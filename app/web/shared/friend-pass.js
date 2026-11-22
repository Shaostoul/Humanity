// ── friend-pass.js ────────────────────────────────────────────────────────
// Friendship passes v3 (v2 2026-10-09, docs/design/blocking-and-safe-mode.md 10b; v3 2026-11-21):
// the words a pass signs and the shape it travels in, shared by the chat
// client (web/chat/crypto.js mints and checks with it) and by Node, where
// scripts/tests/friend-pass.test.js and scripts/pq-kat.mjs hold it to the
// relay's own builder (crates/humanity-protocol/src/pq_crypto.rs `friend_cert_preimage`).
//
//   preimage = "hum/friend/v3\n{audience}\n{issuer}\n{grantee}\n{serial}\n{may}"
//   cert     = {"aud":"...","may":"...","serial":"...","sig":"<base64 Dilithium3>","v":3}
//
// audience: the address this page opened to reach the server (pq.js
// pqAudience, the name a signed request carries), never the did:hum the
// server's identify_challenge claims: a server claiming another's identity
// collected v2 passes good at that other server. The relay takes the pass only
// when the audience is one of its own names; serial: 16 random bytes,
// lowercase hex, which a withdrawal names;
// may: what the friend may do, the sorted, comma-joined, de-duplicated subset
// of FRIEND_PASS_KINDS. No end date: a pass ends when one of the two ends it.
//
// Also here: the note that carries what I chose for each friend between my
// own devices (10n, CTL_CHOICE below).
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load before crypto.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const FRIEND_PASS_DOMAIN = 'hum/friend/v3';
  const FRIEND_PASS_VERSION = 3;
  // Sorted: this order is the canonical one. Must match the Rust list.
  const FRIEND_PASS_KINDS = ['call', 'invite', 'message', 'trade', 'voice_message'];
  // What two new friends give each other: everything but calls (calls come
  // only from people the person chooses).
  const FRIEND_PASS_DEFAULT_MAY = ['invite', 'message', 'trade', 'voice_message'];
  const FRIEND_PASS_SERIAL_BYTES = 16;
  const FRIEND_PASS_MAX_LEN = 8192;

  /** The canonical `may` for these kinds (any order, repeats folded), or null when empty or a kind is unknown. */
  function friendPassMay(words) {
    const kinds = [];
    for (const w of words || []) {
      if (!FRIEND_PASS_KINDS.includes(w)) return null;
      if (!kinds.includes(w)) kinds.push(w);
    }
    if (kinds.length === 0) return null;
    kinds.sort();
    return kinds.join(',');
  }

  /** Is `s` a pass serial: 16 bytes as lowercase hex? */
  function friendPassSerialOk(s) {
    return typeof s === 'string' && s.length === FRIEND_PASS_SERIAL_BYTES * 2 && /^[0-9a-f]+$/.test(s);
  }

  /** The words an issuer signs. Byte for byte the relay's `friend_cert_preimage`. */
  function friendPassPreimage(audience, issuer, grantee, serial, may) {
    return `${FRIEND_PASS_DOMAIN}\n${audience}\n${issuer}\n${grantee}\n${serial}\n${may}`;
  }

  /**
   * Is `a` a server address in its one spelling (the relay's `pass_audience_ok`,
   * `canonical_audience` giving it back unchanged): a lowercase host name of
   * letters, digits, `.` and `-` (no leading `.` or `-`, no trailing `.`, no
   * `..`), or a bracketed IPv6 address, then optionally `:` and a port from 1 to
   * 65535. A did:hum is not one. Shared by the file pass and the household
   * permit's readers.
   */
  function passAudienceOk(a) {
    if (typeof a !== 'string' || a.length === 0 || a.length > 262) return false;
    const m = /^(\[[0-9a-f:.]+\]|[a-z0-9.-]+)(?::(\d{1,5}))?$/.exec(a);
    if (!m) return false;
    const host = m[1];
    if (!host.startsWith('[')) {
      if (host.length > 253 || /^[.-]/.test(host) || host.endsWith('.') || host.includes('..')) return false;
    }
    if (m[2] !== undefined) {
      const port = Number(m[2]);
      // One spelling: no leading zero (the relay writes the port back as a number).
      if (!(port >= 1 && port <= 65535) || String(port) !== m[2]) return false;
    }
    return true;
  }

  /** An id fit to sign: not empty, no line break. */
  function friendPassFieldOk(s) {
    return typeof s === 'string' && s.length > 0 && !/[\r\n]/.test(s);
  }

  /**
   * Read a pass's JSON without checking its signature: {aud, serial, may, sig},
   * or null when it is not a v3 pass in canonical form (the relay refuses the same).
   */
  function friendPassParse(certJson) {
    if (typeof certJson !== 'string' || certJson.length > FRIEND_PASS_MAX_LEN) return null;
    let v;
    try { v = JSON.parse(certJson); } catch { return null; }
    if (!v || v.v !== FRIEND_PASS_VERSION) return null;
    if (typeof v.aud !== 'string' || typeof v.serial !== 'string' || typeof v.may !== 'string' || typeof v.sig !== 'string') return null;
    if (!passAudienceOk(v.aud)) return null;
    if (!friendPassSerialOk(v.serial)) return null;
    if (friendPassMay(v.may.split(',')) !== v.may) return null;
    return { aud: v.aud, serial: v.serial, may: v.may, sig: v.sig };
  }

  /** The JSON a pass travels in: keys sorted, as the relay's builder writes them. */
  function friendPassJson(aud, serial, may, sigB64) {
    return JSON.stringify({ aud, may, serial, sig: sigB64, v: FRIEND_PASS_VERSION });
  }

  // ── The choice for each friend (10n, 2026-10-10) ──
  // What a friend's pass lets them do is the person's CHOICE, kept on every
  // device of theirs: a tick changed on one device reaches the others as a
  // sealed note to myself only (an ordinary signed v2 DM from me to me, sealed
  // to my own DM key, deposited in my own mailbox), whose text is exactly
  //   [[hum:choice:v1]]<friend key>/<may>
  // the friend's identity key in lowercase hex, a slash, and the canonical
  // `may` (the pass's own words, sorted and comma-joined; `invite` alone when
  // nothing is ticked). The note's signed time is when the choice was made:
  // the newest choice wins on every device, and at an equal time the larger
  // `may` text (plain string order), so every device lands on the same one.
  // The marker must match the desktop app's CTL_CHOICE (src/net/dm_pq.rs)
  // exactly; scripts/tests/choice-web.test.js holds this one to the spec's
  // test vector. A text that starts with the marker is never shown.
  const CTL_CHOICE = '[[hum:choice:v1]]';
  // A friend's key in a note: lowercase hex, never empty (the spec's vector
  // uses the short `ab12`), and no longer than any key could be.
  const CHOICE_KEY_RE = /^[0-9a-f]{1,8192}$/;

  /** Does this text start with the choice marker (whatever follows)? Such text is never shown. */
  function isChoiceNoteText(text) {
    return typeof text === 'string' && text.startsWith(CTL_CHOICE);
  }

  /**
   * The note's text for a choice about `friendKey`: `may` as words (any order)
   * or as comma-joined text. Null when the key is not lowercase hex or the
   * words are not a pass's (an unknown one, or none).
   */
  function choiceNoteText(friendKey, may) {
    if (typeof friendKey !== 'string' || !CHOICE_KEY_RE.test(friendKey)) return null;
    const words = Array.isArray(may) ? may : (typeof may === 'string' ? may.split(',') : null);
    const canonical = words ? friendPassMay(words) : null;
    if (!canonical) return null;
    return `${CTL_CHOICE}${friendKey}/${canonical}`;
  }

  /**
   * Read a note's text: {key, may}, or null when it is not exactly the marker,
   * a lowercase hex key, a slash, and a canonical `may` of the pass's words.
   */
  function choiceNoteParse(text) {
    if (!isChoiceNoteText(text)) return null;
    const rest = text.slice(CTL_CHOICE.length);
    const slash = rest.indexOf('/');
    if (slash <= 0) return null;
    const key = rest.slice(0, slash);
    const may = rest.slice(slash + 1);
    if (!CHOICE_KEY_RE.test(key)) return null;
    if (!may || friendPassMay(may.split(',')) !== may) return null;
    return { key, may };
  }

  /**
   * The note an opened, signature-checked DM carries when it is one I sent to
   * myself: from me, to me, about someone other than me. Anything else is null
   * (a choice note from anyone else is dropped unread).
   */
  function choiceNoteFromSelf(inner, me) {
    if (!inner || typeof me !== 'string' || !me) return null;
    const mine = me.toLowerCase();
    if (String(inner.from || '').toLowerCase() !== mine || String(inner.to || '').toLowerCase() !== mine) return null;
    const note = choiceNoteParse(inner.text);
    if (!note || note.key === mine) return null;
    return note;
  }

  /**
   * Does a choice allowing `may`, made at `at`, replace the one kept (`kept`:
   * {may, at}, or null for none)? The newer wins; at an equal time the larger
   * `may` text wins (an empty one, a choice cleared by Unfollow or Block, is
   * the smallest); an older one changes nothing.
   */
  function choiceWins(may, at, kept) {
    if (!kept) return true;
    const a = Number(at) || 0;
    const k = Number(kept.at) || 0;
    if (a !== k) return a > k;
    return String(may || '') > String(kept.may || '');
  }

  const api = {
    FRIEND_PASS_DOMAIN, FRIEND_PASS_VERSION, FRIEND_PASS_KINDS, FRIEND_PASS_DEFAULT_MAY,
    FRIEND_PASS_SERIAL_BYTES, FRIEND_PASS_MAX_LEN,
    friendPassMay, friendPassSerialOk, friendPassPreimage, friendPassFieldOk, friendPassParse, friendPassJson, passAudienceOk,
    CTL_CHOICE, CHOICE_KEY_RE, isChoiceNoteText, choiceNoteText, choiceNoteParse, choiceNoteFromSelf, choiceWins,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);

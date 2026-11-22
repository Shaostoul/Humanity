// ── key-card.js ───────────────────────────────────────────────────────────
// Key cards (2026-10-22, docs/design/key-cards.md): a person's Kyber768
// (ML-KEM-768) key signed by their own Dilithium3 (ML-DSA-65) identity, so the
// app can check that a key it is about to seal something to really is that
// person's, whatever server delivered it. The website's twin of the relay's
// crates/humanity-protocol/src/key_card.rs, held to it byte for byte by
// scripts/tests/key-card-web.test.js (which reads the pinned words out of that
// file) and by the shared fixture scripts/tests/fixtures/key-card-v1.json (made
// by the Rust builder, read by both apps' tests).
//
// THE CARD, as it travels: {"v":1,"key":K,"kyber":Y,"at":T,"sig":S}
//   key   the person's Dilithium3 public key, lowercase hex (3,904 characters)
//   kyber their Kyber768 key, standard base64 with padding (1,580 characters)
//   at    when it was signed, Unix milliseconds
//   sig   Dilithium3 by `key` over the words below, standard base64 (4,412)
// The words signed, as UTF-8:  hum/key-card/v1\n{key}\n{kyber}\n{at}
// No server is named in a card: it is good on every server.
//
// ONE SPELLING: both base64 fields must be the canonical spelling of their
// bytes (decoding and encoding again gives the same text), the key lowercase.
//
// PINNING (the same rule as the desktop app, key_card.rs `compare` and
// net/key_pins.rs): the first good card for a person is pinned. A later good
// card with the same key changes nothing (its time is noted); a different key
// with a newer time is a CHANGE, kept aside, and nothing is sealed to them
// until it is accepted; a different key with an older or equal time is ignored.
// A change waiting is dropped when a newer card brings the pinned key back.
//
// A classic script in the browser (window.HosKeyCard) and a CommonJS module
// under Node (the tests), the way profile-copy.js is. Checking a signature is
// handed a `verify(publicKeyBytes, messageBytes, signatureBytes)` (the chat's
// window.pqVerifyMessage, or noble in the tests), so this file loads nothing.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const CARD_TAG = 'hum/key-card/v1';
  const CARD_VERSION = 1;
  /** A Dilithium3 public key is 1,952 bytes: 3,904 lowercase hex characters. */
  const KEY_HEX_LEN = 1952 * 2;
  /** A Kyber768 key is 1,184 bytes. */
  const KYBER_LEN = 1184;
  /** A Dilithium3 signature is 3,309 bytes. */
  const SIG_LEN = 3309;
  const KYBER_B64_LEN = Math.ceil(KYBER_LEN / 3) * 4;
  const SIG_B64_LEN = Math.ceil(SIG_LEN / 3) * 4;
  /** The most cards one key_card_request asks for. */
  const REQUEST_MAX = 16;
  /** The button under the change notice. */
  const ACCEPT_LABEL = 'Accept the new key';

  /** The words a card's signature covers (sign their UTF-8 bytes). Byte for byte the relay's `card_text`. */
  function cardText(key, kyber, at) {
    return `${CARD_TAG}\n${key}\n${kyber}\n${String(at)}`;
  }

  const b64enc = (bytes) => {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s);
  };

  /** `text` as the bytes it is the canonical base64 of, exactly `len` of them; null otherwise. */
  function canonicalB64(text, len) {
    if (typeof text !== 'string') return null;
    let bytes;
    try {
      bytes = Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
    } catch (e) {
      return null;
    }
    return bytes.length === len && b64enc(bytes) === text ? bytes : null;
  }

  const isLowerHex = (s) => /^[0-9a-f]*$/.test(s);

  /** Is `key` a person's key as cards carry it: 3,904 lowercase hex characters. */
  function isKeyText(key) {
    return typeof key === 'string' && key.length === KEY_HEX_LEN && isLowerHex(key);
  }

  function hexBytes(hex) {
    const out = new Uint8Array(hex.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.substr(i * 2, 2), 16);
    return out;
  }

  /** The one shape a card may have (no signature checked). */
  function shapeOk(card) {
    return !!card && typeof card === 'object'
      && card.v === CARD_VERSION
      && isKeyText(card.key)
      && Number.isSafeInteger(card.at) && card.at > 0
      && typeof card.kyber === 'string' && card.kyber.length === KYBER_B64_LEN
      && typeof card.sig === 'string' && card.sig.length === SIG_B64_LEN
      && !!canonicalB64(card.kyber, KYBER_LEN)
      && !!canonicalB64(card.sig, SIG_LEN);
  }

  /**
   * A card from JSON: an object with exactly a card's five fields, of the right
   * types (the relay's `parse_card`); null for anything else, so nothing
   * unsigned rides along.
   */
  function parseCard(v) {
    if (!v || typeof v !== 'object' || Array.isArray(v)) return null;
    const keys = Object.keys(v);
    if (keys.length !== 5) return null;
    const { v: ver, key, kyber, at, sig } = v;
    if (typeof ver !== 'number' || typeof key !== 'string' || typeof kyber !== 'string'
        || typeof at !== 'number' || typeof sig !== 'string') return null;
    return { v: ver, key, kyber, at, sig };
  }

  /** Is this a good card: the right shape, and signed by the very key it names. */
  async function verifyCard(card, verify) {
    if (!shapeOk(card) || typeof verify !== 'function') return false;
    try {
      const ok = await verify(hexBytes(card.key), new TextEncoder().encode(cardText(card.key, card.kyber, card.at)), canonicalB64(card.sig, SIG_LEN));
      return ok === true;
    } catch (e) {
      return false;
    }
  }

  /** Is this a good card FOR `person`: what the app checks before it seals anything to them. */
  async function cardFor(card, person, verify) {
    return !!card && card.key === person && verifyCard(card, verify);
  }

  /** The pin rule: 'first' | 'same' | 'changed' | 'stale' (the relay's `compare`). `pin` is {kyber, at} or null. */
  function compare(pin, kyber, at) {
    if (!pin) return 'first';
    if (pin.kyber === kyber) return 'same';
    return at > pin.at ? 'changed' : 'stale';
  }

  /**
   * Take a good card (already checked) into `state` = {pins, changes}, both
   * person -> {kyber, at}. Returns {seen, dirty}: what it meant, and whether
   * `state` changed (and should be saved). The desktop app's KeyPins::take.
   */
  function take(state, card) {
    const person = card.key;
    const pins = state.pins, changes = state.changes;
    const pin = Object.prototype.hasOwnProperty.call(pins, person) ? pins[person] : null;
    const change = Object.prototype.hasOwnProperty.call(changes, person) ? changes[person] : null;
    const seen = compare(pin, card.kyber, card.at);
    let dirty = false;
    if (seen === 'first') {
      pins[person] = { kyber: card.kyber, at: card.at };
      dirty = true;
    } else if (seen === 'same') {
      if (card.at > pin.at) { pin.at = card.at; dirty = true; }
      if (change && card.at > change.at) { delete changes[person]; dirty = true; }
    } else if (seen === 'changed') {
      if (!change || card.at > change.at) { changes[person] = { kyber: card.kyber, at: card.at }; dirty = true; }
    }
    return { seen, dirty };
  }

  /** The key to seal to `person`: the pinned one, none while a change waits or when nothing is pinned. */
  function sealingKey(state, person) {
    if (Object.prototype.hasOwnProperty.call(state.changes, person)) return null;
    const pin = Object.prototype.hasOwnProperty.call(state.pins, person) ? state.pins[person] : null;
    return pin ? pin.kyber : null;
  }

  /** Accept `person`'s changed key: it becomes the pin. Returns the new key, or null when none waited. */
  function accept(state, person) {
    if (!Object.prototype.hasOwnProperty.call(state.changes, person)) return null;
    const change = state.changes[person];
    delete state.changes[person];
    state.pins[person] = change;
    return change.kyber;
  }

  /**
   * Why nothing was sealed to someone, in the words both apps show (the relay's
   * `Refusal::text`): kind 'no_card', 'fetching' or 'changed'.
   */
  function refusalText(kind, name) {
    if (kind === 'changed') {
      return `Nothing was sent: ${name}'s encryption key changed. Accept the new key in your conversation with them first.`;
    }
    if (kind === 'fetching') {
      return `Nothing was sent: ${name}'s key card is still on its way from the server. Try again in a moment.`;
    }
    return `Nothing was sent: ${name} has no signed key card on this server, so there is no key we can check is really theirs. It appears once they sign in with a current version of the app.`;
  }

  /** The notice in a conversation with someone whose key changed (the relay's `change_notice`). */
  function changeNotice(name) {
    return `${name}'s encryption key changed. This happens when someone resets their keys. If you did not expect it, check with ${name} in person or on a call you trust before you accept. Until you do, nothing is sent to them.`;
  }

  /**
   * Sign my own card: `sign(messageBytes)` returns the Dilithium3 signature
   * (the chat's pqSignMessage with my secret). `kyberB64` is my Kyber768 key
   * as base64. Returns the card, or null when signing failed.
   */
  async function signCard(keyHex, kyberB64, at, sign) {
    const sig = await sign(new TextEncoder().encode(cardText(keyHex, kyberB64, at)));
    if (!sig) return null;
    return { v: CARD_VERSION, key: keyHex, kyber: kyberB64, at, sig: b64enc(sig) };
  }

  /** The context label a key's fingerprint is derived under (key_card.rs FINGERPRINT_DOMAIN). */
  const FINGERPRINT_DOMAIN = 'hum/key-fingerprint/v1';

  /**
   * The fingerprint people compare to tell a key apart (2026-11-20, the owner branch's second
   * review, W1; key_card.rs `fingerprint`, held to the same known answer by
   * scripts/tests/key-card-web.test.js): BLAKE3 derive_key("hum/key-fingerprint/v1", the key's
   * bytes), its first 12 bytes as six groups of four hex digits separated by spaces. `deriveKey(context,
   * bytes)` is BLAKE3's derive_key with a 32-byte output (noble's blake3, handed in so this file
   * loads nothing). '' for text that is not a key in hex.
   */
  function fingerprint(keyHex, deriveKey) {
    if (typeof keyHex !== 'string' || !keyHex.length || keyHex.length % 2 || !/^[0-9a-fA-F]+$/.test(keyHex)) return '';
    const d = deriveKey(FINGERPRINT_DOMAIN, hexBytes(keyHex.toLowerCase()));
    let hex = '';
    for (let i = 0; i < 12; i++) hex += d[i].toString(16).padStart(2, '0');
    return hex.match(/.{4}/g).join(' ');
  }

  /** How many groups of a fingerprint the owner types for EVERY handover and naming of a successor,
   *  whoever the person (key_card.rs CONFIRM_GROUPS): four, at positions drawn uniformly from the
   *  15 sets each time the page asks. The person reads them from THEIR OWN screen (their own key's
   *  fingerprint, in Settings and on their check prompt); the owner's page names only the positions
   *  and never shows the listed key's groups before they match (the fifth review's H1). A key ground
   *  to match g of the person's groups passes one ask in C(g,4)/15 (none for three or fewer, one in
   *  15 for four, about 2^64 candidates), and a mismatch blocks that key for the session, so it gets
   *  one ask; the arithmetic is in key_card.rs, "WHY FOUR GROUPS AT RANDOM POSITIONS". The typed
   *  groups are the comparison itself. */
  const CONFIRM_GROUPS = 4;
  const FINGERPRINT_GROUPS = 6;

  /** Every set of CONFIRM_GROUPS different group positions of the six, each ascending (15 for four
   *  of six; key_card.rs `confirm_sets`). */
  function confirmSets() {
    const sets = [];
    for (let m = 0; m < (1 << FINGERPRINT_GROUPS); m++) {
      const set = [];
      for (let p = 0; p < FINGERPRINT_GROUPS; p++) if (m & (1 << p)) set.push(p);
      if (set.length === CONFIRM_GROUPS) sets.push(set);
    }
    return sets;
  }
  /** A uniformly random 32-bit number from the browser's generator. */
  function randomU32() {
    const b = new Uint32Array(1);
    globalThis.crypto.getRandomValues(b);
    return b[0];
  }

  /** CONFIRM_GROUPS different group positions (0-based, ascending) drawn for one ask, uniformly
   *  from all 15 sets whatever was asked before (key_card.rs `confirm_positions_with`; the fifth
   *  review's H1 removed the "never the set asked last" rule, which only narrowed a grinder's next
   *  draw). `random` gives uniform 32-bit numbers (the browser's generator unless a test hands one
   *  in). */
  function confirmPositions(random) {
    const rnd = typeof random === 'function' ? random : randomU32;
    const sets = confirmSets();
    const n = sets.length;
    const zone = 0x100000000 - (0x100000000 % n); // rejection sampling: every set equally likely
    for (;;) {
      const r = rnd() >>> 0;
      if (r < zone) return sets[r % n];
    }
  }

  /** The groups of fingerprint `fp` at `positions`, joined by a space (key_card.rs `groups_at`);
   *  '' when a position is past its end or named twice. */
  function groupsAt(fp, positions) {
    const groups = String(fp || '').split(' ').filter((g) => g);
    if (!Array.isArray(positions) || positions.length !== CONFIRM_GROUPS || new Set(positions).size !== positions.length) return '';
    if (positions.some((p) => !Number.isInteger(p) || p < 0 || p >= groups.length)) return '';
    return positions.map((p) => groups[p]).join(' ');
  }

  /** Whether `typed` is the groups of `fp` at `positions`, in order, any case, spaces anywhere
   *  ignored (key_card.rs `groups_match_at`). */
  function groupsMatchAt(fp, positions, typed) {
    const want = groupsAt(fp, positions).replace(/\s+/g, '').toLowerCase();
    return !!want && String(typed == null ? '' : typed).replace(/\s+/g, '').toLowerCase() === want;
  }

  /** Whether `typed` confirms the fingerprint of `keyHex` at the asked `positions`
   *  (key_card.rs `fingerprint_confirmed_at`). */
  function fingerprintConfirmedAt(keyHex, positions, typed, deriveKey) {
    return groupsMatchAt(fingerprint(keyHex, deriveKey), positions, typed);
  }

  /** What the owner typed for one ask, judged against the listed key `keyHex` (key_card.rs
   *  `judge_at`): 'empty' (nothing typed), 'short' (not sixteen characters, spaces ignored: no
   *  judgement yet), 'matched', or 'mismatched' (sixteen characters that are not the listed key's
   *  groups at `positions`: the page warns, logs it and refuses that key for the session). */
  function judgeAt(keyHex, positions, typed, deriveKey) {
    const got = String(typed == null ? '' : typed).replace(/\s+/g, '');
    if (!got) return 'empty';
    if ([...got].length !== CONFIRM_GROUPS * 4) return 'short';
    return fingerprintConfirmedAt(keyHex, positions, typed, deriveKey) ? 'matched' : 'mismatched';
  }

  /** The longest name an honest relay lets a person take (relay.rs identify). */
  const NAME_MAX = 24;
  /** Whether `name` is one an honest relay could list: 1 to 24 letters, digits, _ or - (key_card.rs
   *  `honest_name`, the rule relay.rs identify holds every name to). The owner's picker shows any
   *  other as a refused name in words, never the text itself (the third review's M2). */
  function honestName(name) {
    return typeof name === 'string' && /^[A-Za-z0-9_-]{1,24}$/.test(name) && !reservedName(name);
  }
  /** The app's own voice and the server's names, nobody's to take (key_card.rs `RESERVED_NAMES`, the
   *  owner branch's seventh review, H1 (b)); scripts/tests/key-card-web.test.js holds the two lists
   *  equal. */
  const RESERVED_NAMES = ['System', 'HumanityOS', 'HumanityOSApp', 'Humanity', 'UnitedHumanity', 'FromThisServer', 'ThisServer', 'Server', 'App'];
  const FOLD = { 0: 'o', 1: 'l', i: 'l', 2: 'z', 3: 'e', 4: 'a', 5: 's', 6: 'g', 9: 'g', 7: 't', 8: 'b' };
  /** key_card.rs `fold_name`: lower case, `_` and `-` dropped, the ASCII lookalikes read as the
   *  letter they imitate, then "rn" m and "vv" w. */
  function foldName(name) {
    const mapped = [...String(name)].filter((c) => c !== '_' && c !== '-').map((c) => { const l = c.toLowerCase(); return FOLD[l] || l; }).join('');
    return mapped.split('rn').join('m').split('vv').join('w');
  }
  function reservedName(name) {
    const f = foldName(name);
    return RESERVED_NAMES.some((r) => foldName(r) === f);
  }

  const api = {
    CARD_TAG, CARD_VERSION, KEY_HEX_LEN, KYBER_LEN, SIG_LEN, KYBER_B64_LEN, SIG_B64_LEN, REQUEST_MAX, ACCEPT_LABEL,
    cardText, isKeyText, shapeOk, parseCard, verifyCard, cardFor, compare, take, sealingKey, accept,
    refusalText, changeNotice, signCard, canonicalB64, FINGERPRINT_DOMAIN, fingerprint, CONFIRM_GROUPS, FINGERPRINT_GROUPS, confirmSets, confirmPositions, groupsAt, groupsMatchAt, fingerprintConfirmedAt, judgeAt,
    NAME_MAX, honestName, RESERVED_NAMES, foldName, reservedName,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  if (root) root.HosKeyCard = api;
})(typeof window !== 'undefined' ? window : null);

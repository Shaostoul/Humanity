// ── friend-pass.js ────────────────────────────────────────────────────────
// Friendship passes v2 (2026-10-09, docs/design/blocking-and-safe-mode.md 10b):
// the words a pass signs and the shape it travels in, shared by the chat
// client (web/chat/crypto.js mints and checks with it) and by Node, where
// scripts/tests/friend-pass.test.js and scripts/pq-kat.mjs hold it to the
// relay's own builder (src/relay/core/pq_crypto.rs `friend_cert_preimage`).
//
//   preimage = "hum/friend/v2\n{server}\n{issuer}\n{grantee}\n{serial}\n{may}"
//   cert     = {"v":2,"serial":"...","may":"...","sig":"<base64 Dilithium3>"}
//
// server: the did:hum of the server it is given on (its identify_challenge
// says); serial: 16 random bytes, lowercase hex, which a withdrawal names;
// may: what the friend may do, the sorted, comma-joined, de-duplicated subset
// of FRIEND_PASS_KINDS. No end date: a pass ends when one of the two ends it.
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load before crypto.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const FRIEND_PASS_DOMAIN = 'hum/friend/v2';
  const FRIEND_PASS_VERSION = 2;
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
  function friendPassPreimage(server, issuer, grantee, serial, may) {
    return `${FRIEND_PASS_DOMAIN}\n${server}\n${issuer}\n${grantee}\n${serial}\n${may}`;
  }

  /** An id fit to sign: not empty, no line break. */
  function friendPassFieldOk(s) {
    return typeof s === 'string' && s.length > 0 && !/[\r\n]/.test(s);
  }

  /**
   * Read a pass's JSON without checking its signature: {serial, may, sig}, or
   * null when it is not a v2 pass in canonical form (the relay refuses the same).
   */
  function friendPassParse(certJson) {
    if (typeof certJson !== 'string' || certJson.length > FRIEND_PASS_MAX_LEN) return null;
    let v;
    try { v = JSON.parse(certJson); } catch { return null; }
    if (!v || v.v !== FRIEND_PASS_VERSION) return null;
    if (typeof v.serial !== 'string' || typeof v.may !== 'string' || typeof v.sig !== 'string') return null;
    if (!friendPassSerialOk(v.serial)) return null;
    if (friendPassMay(v.may.split(',')) !== v.may) return null;
    return { serial: v.serial, may: v.may, sig: v.sig };
  }

  /** The JSON a pass travels in. */
  function friendPassJson(serial, may, sigB64) {
    return JSON.stringify({ v: FRIEND_PASS_VERSION, serial, may, sig: sigB64 });
  }

  const api = {
    FRIEND_PASS_DOMAIN, FRIEND_PASS_VERSION, FRIEND_PASS_KINDS, FRIEND_PASS_DEFAULT_MAY,
    FRIEND_PASS_SERIAL_BYTES, FRIEND_PASS_MAX_LEN,
    friendPassMay, friendPassSerialOk, friendPassPreimage, friendPassFieldOk, friendPassParse, friendPassJson,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);

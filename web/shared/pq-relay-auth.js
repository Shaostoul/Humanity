/**
 * pq-relay-auth.js — shared helper for Dilithium-signed relay REST auth
 * from standalone pages (admin dashboard, settings vault-sync, etc.).
 *
 * Backstory: the relay's identity-keyed REST endpoints (vault_sync,
 * admin_stats, listing reviews, etc.) flipped from Ed25519-verify to
 * Dilithium3-verify in v0.266.0 (Inc5c-core). The chat client signs
 * with Dilithium via `pqSignChatMessage`, so it kept working. The
 * STANDALONE pages — which don't share JS state with the chat client
 * — kept calling `crypto.subtle.sign('Ed25519', ...)` and the relay
 * silently rejected every request from them. Inc5c-tail (v0.277.2)
 * routes those pages through this helper.
 *
 * Preimage is identical to the chat client's `purpose\ntimestamp` shape
 * (matches `verify_dilithium_signature` in src/relay/handlers/broadcast.rs).
 * The wire format is also unchanged — `{ key, timestamp, sig }` — only
 * the byte sizes grew: Dilithium pubkey hex is 3904 chars (was 64) and
 * signature hex is ~6618 chars (was 128). For GET endpoints that carry
 * the auth in the query string this puts the URL near ~10 KB; the relay
 * accepts it and nginx's default `large_client_header_buffers` covers
 * it — if a future operator tightens nginx, bump that directive instead
 * of redesigning the auth shape.
 *
 * Requires `/chat/pq.js` (or equivalent) to be loaded first so that
 * `window.pqDeriveIdentity` + `window.pqSignMessage` are available. We
 * intentionally do NOT depend on `crypto.js` so the helper stays light
 * for pages that don't want all of chat's identity scaffolding.
 *
 * Returns null on any failure (no identity in localStorage, wrapped-
 * only key with no plaintext backup, pq.js not loaded, etc.). Callers
 * MUST handle null by showing the user "Sign in via Chat first" — never
 * fall back to Ed25519, which the relay will reject.
 */
(function() {
  'use strict';

  // PKCS8 for Ed25519 is 48 bytes; the seed is the trailing 32 bytes
  // starting at offset 16. Mirrors `extractSeedFromPkcs8` in
  // web/chat/crypto.js verbatim so the two implementations cannot drift.
  function _extractSeedFromPkcs8(pkcs8Buf) {
    const bytes = new Uint8Array(pkcs8Buf);
    if (bytes.length === 48) return bytes.slice(16, 48);
    // Fallback for any non-standard length: take the trailing 32 bytes.
    return bytes.slice(bytes.length - 32);
  }

  function _bufToHex(buf) {
    const bytes = new Uint8Array(buf);
    let hex = '';
    for (let i = 0; i < bytes.length; i++) hex += bytes[i].toString(16).padStart(2, '0');
    return hex;
  }

  /**
   * Load the user's BIP39 seed (32 bytes) from localStorage.
   * Returns a Uint8Array or null if no plaintext backup exists.
   *
   * Wrapped-only users (the `WRAPPED_KEY_LS` blob in crypto.js, no
   * `humanity_key_backup` row) cannot be served from a standalone page
   * without re-prompting for the passphrase, which is out of scope
   * here — return null and let the caller surface the "Sign in via
   * Chat first" message. The chat client unlocks the seed in its tab
   * and that does not propagate cross-tab.
   */
  async function _loadSeed() {
    const backup = localStorage.getItem('humanity_key_backup');
    if (!backup) return null;
    let parsed;
    try {
      parsed = JSON.parse(backup);
    } catch (e) {
      console.warn('pq-relay-auth: humanity_key_backup is not JSON', e);
      return null;
    }
    try {
      if (parsed.privateKeyPkcs8) {
        // Common path: PKCS8 base64 — extract seed directly.
        const pkcs8 = Uint8Array.from(atob(parsed.privateKeyPkcs8), c => c.charCodeAt(0)).buffer;
        return _extractSeedFromPkcs8(pkcs8);
      }
      if (parsed.jwk) {
        // Legacy JWK path: import then re-export as PKCS8 to get the seed.
        // Mark extractable=true so the re-export works.
        const key = await crypto.subtle.importKey('jwk', parsed.jwk, 'Ed25519', true, ['sign']);
        const pkcs8 = await crypto.subtle.exportKey('pkcs8', key);
        return _extractSeedFromPkcs8(pkcs8);
      }
      console.warn('pq-relay-auth: humanity_key_backup has neither privateKeyPkcs8 nor jwk');
      return null;
    } catch (e) {
      console.warn('pq-relay-auth: failed to extract seed from backup', e);
      return null;
    }
  }

  // The last timestamp this page signed with. The relay refuses a second
  // request with the same key, purpose and timestamp as a replay (media.rs
  // check_signed, auth_nonce_fresh), and two requests started in the same
  // millisecond read the same Date.now(): Accept on a category suggestion
  // sent two signed `media_list` reads at once and the relay refused one
  // (the Media page review, 2026-10-11). So every signature from this page
  // takes the next millisecond after the last one, like the desktop app's
  // `next_timestamp`: no two signed requests from one page ever share one.
  let _lastTimestamp = 0;
  function _nextTimestamp() {
    const now = Date.now();
    _lastTimestamp = now > _lastTimestamp ? now : _lastTimestamp + 1;
    return _lastTimestamp;
  }

  /**
   * Build a Dilithium3-signed relay auth payload.
   *
   * @param {string} purpose — domain-separated tag, MUST match the
   *   server-side `format!("{}\n{}", content, timestamp)` preimage
   *   in `verify_dilithium_signature`. Known values today:
   *   "vault_sync", "admin_stats", "review", "review_delete",
   *   "trade_order", "cancel_order", "fill_order", "agent_override".
   * @returns {Promise<{key:string,timestamp:number,sig:string}|null>}
   *   `key` is the 3904-char Dilithium pubkey hex (NOT the Ed25519
   *   pubkey at localStorage['humanity_key'] — the relay keys
   *   accounts by Dilithium post-Inc3, so sending the Ed25519 key
   *   would fail with "unknown user"). Null on any failure.
   */
  async function getPqSignedAuth(purpose) {
    if (typeof purpose !== 'string' || !purpose) {
      console.warn('pq-relay-auth: purpose is required');
      return null;
    }
    if (typeof window.pqDeriveIdentity !== 'function' || typeof window.pqSignMessage !== 'function') {
      console.warn('pq-relay-auth: pq.js helpers not loaded — load /chat/pq.js first');
      return null;
    }

    const seed = await _loadSeed();
    if (!seed) return null;

    const id = await window.pqDeriveIdentity(seed);
    if (!id || !id.dilithiumPublicHex || !id.dilithiumSecret) {
      console.warn('pq-relay-auth: pqDeriveIdentity returned null');
      return null;
    }

    // Taken after the identity is derived, the moment before signing: the
    // next one after the last this page signed (see _nextTimestamp).
    const ts = _nextTimestamp();
    const preimage = new TextEncoder().encode(purpose + '\n' + ts);
    const sigBytes = await window.pqSignMessage(id.dilithiumSecret, preimage);
    if (!sigBytes) {
      console.warn('pq-relay-auth: pqSignMessage returned null');
      return null;
    }

    return {
      key: id.dilithiumPublicHex,
      timestamp: ts,
      sig: _bufToHex(sigBytes),
    };
  }

  /**
   * Load + derive the user's full Dilithium3 identity for standalone pages
   * that need to SIGN OBJECTS (governance votes, vouches, ...) rather than
   * the `purpose\ntimestamp` REST auth above. Same seed source
   * (localStorage `humanity_key_backup`) and the same KAT-locked derivation
   * (`pqDeriveIdentity` from /chat/pq.js), kept here so seed extraction has
   * exactly one implementation.
   *
   * @returns {Promise<{dilithiumPublicHex:string, dilithiumSecret:Uint8Array}|null>}
   *   Null on any failure (no identity in localStorage, wrapped-only key,
   *   pq.js not loaded). Callers MUST show "Sign in via Chat first" on null.
   */
  async function getPqIdentity() {
    if (typeof window.pqDeriveIdentity !== 'function') {
      console.warn('pq-relay-auth: pq.js helpers not loaded — load /chat/pq.js first');
      return null;
    }
    const seed = await _loadSeed();
    if (!seed) return null;
    const id = await window.pqDeriveIdentity(seed);
    if (!id || !id.dilithiumPublicHex || !id.dilithiumSecret) {
      console.warn('pq-relay-auth: pqDeriveIdentity returned null');
      return null;
    }
    return id;
  }

  /**
   * The key Chat's local store (/chat/chat-dm-store.js) is encrypted under,
   * for a standalone page that only READS that store: the Trade page reads the
   * friendship pass a friend gave this person, to present it with a trade
   * request. Derived from the same seed exactly as Chat derives it
   * (getDmStoreKey in /chat/crypto.js): HKDF-SHA-256 with an empty salt and
   * the info "hum/dm-store-web/v1", to a non-extractable AES-256-GCM key.
   * Decrypt only, since a page reading Chat's store must never write to it.
   * scripts/tests/trade-page-web.test.js opens a store that Chat's own code
   * wrote, so the two derivations cannot drift apart unnoticed.
   *
   * @returns {Promise<CryptoKey|null>} null without a readable seed.
   */
  async function getPqDmStoreKey() {
    const seed = await _loadSeed();
    if (!seed) return null;
    try {
      const base = await crypto.subtle.importKey('raw', seed, 'HKDF', false, ['deriveKey']);
      return await crypto.subtle.deriveKey(
        { name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(0),
          info: new TextEncoder().encode('hum/dm-store-web/v1') },
        base, { name: 'AES-GCM', length: 256 }, false, ['decrypt']);
    } catch (e) {
      console.warn('pq-relay-auth: could not derive the local store key', e);
      return null;
    }
  }

  /**
   * The 24 words of the recovery phrase for `seed32`, as a list: BIP39, 256 bits of entropy and an
   * 8-bit SHA-256 checksum, 11 bits a word (the same as mnemonicFromSeed in /chat/crypto.js, which
   * the backup screen shows; scripts/tests/post-in-chat-web.test.js holds the two to each other).
   * `words` is the 2048-word list (/chat/bip39-english.js).
   */
  async function _phraseWords(seed32, words) {
    const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', seed32));
    const bits = [];
    for (const byte of seed32) for (let i = 7; i >= 0; i--) bits.push((byte >> i) & 1);
    for (let i = 7; i >= 0; i--) bits.push((hash[0] >> i) & 1);
    const out = [];
    for (let i = 0; i < 24; i++) {
      let idx = 0;
      for (let j = 0; j < 11; j++) idx = (idx << 1) | bits[i * 11 + j];
      out.push(words[idx]);
    }
    return out;
  }

  /**
   * The recovery-phrase guard for a standalone page that sends something the person wrote (the
   * Media page's "Also post in chat", 2026-10-17): does any of `texts` hold 4 or more words of the
   * person's recovery phrase in a row, in its order? The rule the chat's own guard applies
   * (chat-warnings.js recoveryPhraseIn, over /shared/warnings.js phraseRunFound). The phrase is
   * derived from the seed for this one check and dropped, never kept.
   *
   * @returns {Promise<boolean|null>} null when it cannot be checked here (no readable seed, the
   *   word list or the matcher not loaded): the caller then sends nothing.
   */
  async function pqRecoveryPhraseIn(texts) {
    const list = (Array.isArray(texts) ? texts : [texts]).filter((t) => typeof t === 'string' && t.trim());
    if (!list.length) return false;
    const words = window.BIP39_ENGLISH;
    if (!Array.isArray(words) || words.length !== 2048 || typeof window.phraseRunFound !== 'function') return null;
    const seed = await _loadSeed();
    if (!seed || seed.length !== 32) return null;
    try {
      const phrase = await _phraseWords(seed, words);
      return list.some((t) => window.phraseRunFound(t, phrase));
    } catch (e) {
      console.warn('pq-relay-auth: the recovery phrase could not be derived for the check', e);
      return null;
    }
  }

  window.getPqSignedAuth = getPqSignedAuth;
  window.getPqIdentity = getPqIdentity;
  window.getPqDmStoreKey = getPqDmStoreKey;
  window.pqRecoveryPhraseIn = pqRecoveryPhraseIn;
})();

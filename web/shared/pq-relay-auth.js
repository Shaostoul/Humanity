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

  /**
   * Build a Dilithium3-signed relay auth payload.
   *
   * @param {string} purpose — domain-separated tag, MUST match the
   *   server-side `format!("{}\n{}", content, timestamp)` preimage
   *   in `verify_dilithium_signature`. Known values today:
   *   "vault_sync", "admin_stats", "review", "review_delete",
   *   "trade_order", "cancel_order", "fill_order".
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

    const ts = Date.now();
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

  window.getPqSignedAuth = getPqSignedAuth;
  window.getPqIdentity = getPqIdentity;
})();

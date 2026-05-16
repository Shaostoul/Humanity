/**
 * pq.js — post-quantum (Dilithium3 / ML-DSA-65) identity for the chat
 * client. PQ migration Increment 1 (v0.251).
 *
 * The chat identity's master secret is the 32-byte BIP39 seed (today
 * also the Ed25519 scalar — see crypto.js). From that SAME seed we
 * deterministically derive a Dilithium3 keypair, using the exact
 * derivation the relay expects:
 *
 *   seed32  = BLAKE3.derive_key("hum/dilithium3/v1", masterSeed)[..32]
 *   keypair = ML-DSA-65.keygen(seed32)
 *
 * Verified byte-for-byte against the Rust relay
 * (src/relay/core/pq_crypto.rs::dilithium_cross_language_kat) — if the
 * KAT in scripts/pq-kat.mjs / that Rust test ever fails, DO NOT ship:
 * a mismatch makes every client PQ pubkey unverifiable by the relay.
 *
 * Increment 1 is ADDITIVE: we derive + present the Dilithium public key
 * at identify so the relay can record it alongside the Ed25519 key.
 * Ed25519 stays the canonical identity for now. Every step here is
 * best-effort — ANY failure (no WASM-free env, module load error,
 * etc.) must leave the caller on the exact pre-PQ Ed25519 path. Never
 * throw out of the public helpers.
 *
 * The Dilithium secret key is kept in memory only (window-lifetime).
 * Signing + persistence land in Increment 2; Increment 1 only needs
 * the public key on the wire.
 */

// Same-origin vendored bundle (CSP `script-src 'self'`). No CDN: a
// primary-identity dependency must not rely on a third party.
const _PQ_BUNDLE_URL = '/shared/vendor/noble-pq.bundle.js';
const _PQ_DOMAIN_DILITHIUM = 'hum/dilithium3/v1';

let _pqMod = null;       // resolved noble module (cached)
let _pqLoadTried = false;
let _pqLoadFailed = false;

/** Lazily import the vendored noble bundle. Returns the module or null. */
async function _pqLoad() {
  if (_pqMod) return _pqMod;
  if (_pqLoadFailed) return null;
  _pqLoadTried = true;
  try {
    _pqMod = await import(_PQ_BUNDLE_URL);
    if (!_pqMod || !_pqMod.ml_dsa65 || !_pqMod.blake3) {
      throw new Error('vendored PQ bundle missing exports');
    }
    return _pqMod;
  } catch (e) {
    _pqLoadFailed = true;
    console.warn('PQ disabled — vendored bundle failed to load:', e && e.message);
    return null;
  }
}

/**
 * Derive the Dilithium3 identity from a 32-byte master seed.
 * @param {Uint8Array} seed32 - the BIP39 / Ed25519 seed (32 bytes).
 * @returns {Promise<{dilithiumPublicHex:string, dilithiumSecret:Uint8Array}|null>}
 *          null on ANY failure (caller must degrade to Ed25519-only).
 */
async function pqDeriveIdentity(seed32) {
  try {
    if (!seed32 || seed32.length !== 32) return null;
    const m = await _pqLoad();
    if (!m) return null;
    const ctx = new TextEncoder().encode(_PQ_DOMAIN_DILITHIUM);
    // BLAKE3 derive_key mode — context MUST be the UTF-8 bytes of the
    // domain string (noble@2.x rejects a JS string here; the relay uses
    // blake3::Hasher::new_derive_key(<same string>)).
    const dilSeed = m.blake3.create({ context: ctx, dkLen: 32 })
      .update(seed32)
      .digest();
    const kp = m.ml_dsa65.keygen(dilSeed); // { publicKey:1952B, secretKey }
    const pub = kp.publicKey;
    let hex = '';
    for (let i = 0; i < pub.length; i++) hex += pub[i].toString(16).padStart(2, '0');
    return { dilithiumPublicHex: hex, dilithiumSecret: kp.secretKey };
  } catch (e) {
    console.warn('pqDeriveIdentity failed (continuing Ed25519-only):', e && e.message);
    return null;
  }
}

/**
 * Sign a message with a Dilithium3 secret key. Returns the 3309-byte
 * signature as a Uint8Array, or null on failure. (Used from Increment 2
 * onward; defined here so all PQ primitives live in one file.)
 */
async function pqSignMessage(secretKey, messageBytes) {
  try {
    if (!secretKey || !messageBytes) return null;
    const m = await _pqLoad();
    if (!m) return null;
    // noble @noble/post-quantum 0.6.x: sign(message, secretKey).
    return m.ml_dsa65.sign(messageBytes, secretKey);
  } catch (e) {
    console.warn('pqSignMessage failed:', e && e.message);
    return null;
  }
}

// Exposed globally (the chat client is classic scripts, not modules).
window.pqDeriveIdentity = pqDeriveIdentity;
window.pqSignMessage = pqSignMessage;

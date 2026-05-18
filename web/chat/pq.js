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
const _PQ_DOMAIN_KYBER = 'hum/kyber768/v1';
const _PQ_DOMAIN_DM_AES = 'hum/dm-aes/v1';

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
    if (!_pqMod || !_pqMod.ml_dsa65 || !_pqMod.blake3 || !_pqMod.ml_kem768) {
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

/**
 * Derive the Kyber768 (ML-KEM-768) DM keypair from the 32-byte master
 * seed — the SAME seed the Dilithium identity uses. Byte-identical to
 * the Rust relay (pq_crypto::derive_kyber_seed → KyberKeypair::from_seed)
 * and the native client (net::dm_pq) — locked by the cross-language KAT
 * in scripts/pq-kat.mjs + pq_crypto.rs::kyber_cross_language_kat.
 *
 *   kseed64 = BLAKE3.derive_key("hum/kyber768/v1", masterSeed)[..64]
 *   keypair = ML-KEM-768.keygen(kseed64)
 *
 * This determinism is THE fix for cross-client DMs: web and native
 * derive the same DM keypair from the same seed, so there is no
 * per-device random key and no vault import, ever.
 * @returns {Promise<{kyberPublicBytes:Uint8Array, kyberSecret:Uint8Array}|null>}
 */
async function pqDeriveKyber(seed32) {
  try {
    if (!seed32 || seed32.length !== 32) return null;
    const m = await _pqLoad();
    if (!m) return null;
    const ctx = new TextEncoder().encode(_PQ_DOMAIN_KYBER);
    const kseed = m.blake3.create({ context: ctx, dkLen: 64 })
      .update(seed32)
      .digest();
    const kp = m.ml_kem768.keygen(kseed); // { publicKey:1184B, secretKey }
    return { kyberPublicBytes: kp.publicKey, kyberSecret: kp.secretKey };
  } catch (e) {
    console.warn('pqDeriveKyber failed:', e && e.message);
    return null;
  }
}

const _b64 = {
  enc: (u8) => btoa(String.fromCharCode(...u8)),
  dec: (s) => Uint8Array.from(atob(s.trim()), (c) => c.charCodeAt(0)),
};

/** BLAKE3.derive_key("hum/dm-aes/v1", sharedSecret) → 32-byte AES key.
 *  Identical KDF to net::dm_pq.rs (operator chose BLAKE3 over HKDF —
 *  already vendored both sides). */
async function _dmAesKey(m, sharedSecret) {
  const ctx = new TextEncoder().encode(_PQ_DOMAIN_DM_AES);
  const raw = m.blake3.create({ context: ctx, dkLen: 32 })
    .update(sharedSecret)
    .digest();
  return crypto.subtle.importKey('raw', raw, { name: 'AES-GCM' }, false,
    ['encrypt', 'decrypt']);
}

/**
 * Seal a DM for the holder of `recipientPubB64` (their base64 Kyber768
 * public key). Pure ML-KEM-768 → BLAKE3-KDF → AES-256-GCM. Matches
 * net::dm_pq::seal exactly. Sender needs no keypair (KEM).
 * @returns {Promise<{ek_ct_b64,nonce_b64,ct_b64}|null>}
 */
async function pqDmSeal(recipientPubB64, plaintext) {
  try {
    const m = await _pqLoad();
    if (!m) return null;
    const pub = _b64.dec(recipientPubB64);
    const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(pub);
    const key = await _dmAesKey(m, sharedSecret);
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const body = new Uint8Array(await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv }, key, new TextEncoder().encode(plaintext)));
    return {
      ek_ct_b64: _b64.enc(cipherText),
      nonce_b64: _b64.enc(iv),
      ct_b64: _b64.enc(body),
    };
  } catch (e) {
    console.warn('pqDmSeal failed:', e && e.message);
    return null;
  }
}

/** Open a DM addressed to us. `kyberSecret` from pqDeriveKyber.
 *  Matches net::dm_pq::open. Returns plaintext or null. */
async function pqDmOpen(kyberSecret, ekCtB64, nonceB64, ctB64) {
  try {
    const m = await _pqLoad();
    if (!m || !kyberSecret) return null;
    const ss = m.ml_kem768.decapsulate(_b64.dec(ekCtB64), kyberSecret);
    const key = await _dmAesKey(m, ss);
    const plain = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: _b64.dec(nonceB64) }, key, _b64.dec(ctB64));
    return new TextDecoder().decode(plain);
  } catch (e) {
    console.warn('pqDmOpen failed (wrong key / tampered):', e && e.message);
    return null;
  }
}

// Exposed globally (the chat client is classic scripts, not modules).
window.pqDeriveIdentity = pqDeriveIdentity;
window.pqSignMessage = pqSignMessage;
window.pqDeriveKyber = pqDeriveKyber;
window.pqDmSeal = pqDmSeal;
window.pqDmOpen = pqDmOpen;

/**
 * pq.js, post-quantum (Dilithium3 / ML-DSA-65) identity for the chat
 * client. PQ migration Increment 1 (v0.251).
 *
 * The chat identity's master secret is the 32-byte BIP39 seed (held in
 * an Ed25519 CryptoKey purely as storage, see crypto.js; the wallet's
 * Ed25519 key is derived from it under its own label). From that SAME seed we
 * deterministically derive a Dilithium3 keypair, using the exact
 * derivation the relay expects:
 *
 *   seed32  = BLAKE3.derive_key("hum/dilithium3/v1", masterSeed)[..32]
 *   keypair = ML-DSA-65.keygen(seed32)
 *
 * Verified byte-for-byte against the Rust relay
 * (crates/humanity-protocol/src/pq_crypto.rs::dilithium_cross_language_kat), if the
 * KAT in scripts/pq-kat.mjs / that Rust test ever fails, DO NOT ship:
 * a mismatch makes every client PQ pubkey unverifiable by the relay.
 *
 * Increment 1 is ADDITIVE: we derive + present the Dilithium public key
 * at identify so the relay can record it alongside the Ed25519 key.
 * Ed25519 stays the canonical identity for now. Every step here is
 * best-effort, ANY failure (no WASM-free env, module load error,
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
    console.warn('PQ disabled, vendored bundle failed to load:', e && e.message);
    return null;
  }
}

/**
 * Derive the Dilithium3 identity from a 32-byte master seed.
 * @param {Uint8Array} seed32 - the identity seed, the recovery phrase's 32 bytes.
 * @returns {Promise<{dilithiumPublicHex:string, dilithiumSecret:Uint8Array}|null>}
 *          null on ANY failure (caller must degrade to Ed25519-only).
 */
async function pqDeriveIdentity(seed32) {
  try {
    if (!seed32 || seed32.length !== 32) return null;
    const m = await _pqLoad();
    if (!m) return null;
    const ctx = new TextEncoder().encode(_PQ_DOMAIN_DILITHIUM);
    // BLAKE3 derive_key mode, context MUST be the UTF-8 bytes of the
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
 * Verify a Dilithium3 signature. `publicKey` and `signature` are
 * Uint8Array; `messageBytes` Uint8Array. Returns true/false (never
 * throws). noble 0.6.x: verify(signature, message, publicKey).
 */
async function pqVerifyMessage(publicKey, messageBytes, signature) {
  try {
    if (!publicKey || !messageBytes || !signature) return false;
    const m = await _pqLoad();
    if (!m) return false;
    return !!m.ml_dsa65.verify(signature, messageBytes, publicKey);
  } catch (e) {
    console.warn('pqVerifyMessage failed:', e && e.message);
    return false;
  }
}

/**
 * Derive the Kyber768 (ML-KEM-768) DM keypair from the 32-byte master
 * seed, the SAME seed the Dilithium identity uses. Byte-identical to
 * the Rust relay (pq_crypto::derive_kyber_seed → KyberKeypair::from_seed)
 * and the native client (net::dm_pq), locked by the cross-language KAT
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
 *  Identical KDF to net::dm_pq.rs (operator chose BLAKE3 over HKDF -
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

/**
 * BLAKE3 (plain hash, 32 bytes) of `bytes`, from the vendored bundle: the
 * report's evidence hash (step D, docs/design/blocking-and-safe-mode.md 10e),
 * which the relay recomputes with the Rust blake3 crate. Null on failure.
 */
async function pqBlake3(bytes) {
  try {
    const m = await _pqLoad();
    if (!m || !bytes) return null;
    return m.blake3.create({ dkLen: 32 }).update(bytes).digest();
  } catch (e) {
    console.warn('pqBlake3 failed:', e && e.message);
    return null;
  }
}

// ── Signed requests and sign-ins (2026-11-20; the audience line 2026-11-21) ─────────────────────
// A REQUEST to a relay (an account export, a bug's details, a vault sync, a channel read, an admin
// action) is signed over `hum/request/v2\n{audience}\n{purpose}\n{timestamp}`, never over a chat
// message's `text\ntimestamp`: a request signed in the chat shape was a chat line, so a person who
// posted the line `account_export` handed the room a credential. A SIGN-IN (the answer to a relay's
// identify challenge) is signed over `hum/identify/v2\n{audience}\n{nonce}\n{key}`.
// The AUDIENCE is the address this page talks to (its own location.host for requests, the socket's
// own URL for a sign-in), so a signature made for one server is never good at another: a server
// cannot replay a request it received, nor pass another server's sign-in challenge through to this
// page and sign in there as the person (the review of 2026-11-21). The twin of
// crates/humanity-protocol/src/signed_request.rs; both are held to
// scripts/tests/fixtures/signed-request-v2.json by scripts/tests/signed-request-web.test.js. Every
// page that signs a request or a sign-in loads this file.
const PQ_REQUEST_DOMAIN = 'hum/request/v2';
const PQ_IDENTIFY_DOMAIN = 'hum/identify/v2';

/**
 * The audience for a URL: its host, lowercase, then `:port` when the port is not the scheme's
 * default (80 for http and ws, 443 for https and wss); null for anything that is not an address.
 * A URL with no scheme is read as host[:port]. The same rules as the Rust `audience_of`.
 *
 * One reading (2026-11-21, round 3): the URL is read twice, by this strict hand reading and by the
 * WHATWG URL parser the browser sends with (`new URL`), and null unless both name the same host
 * and port: a backslash ends a special URL's host for the sender, so `https://evil.example\@victim.example/`
 * would be signed for victim.example and sent to evil.example; `127.1` is 127.0.0.1 to the sender;
 * IPv6 has one written form. A trailing dot is a second spelling of a name and gives none.
 */
function pqAudienceOf(url) {
  if (typeof url !== 'string') return null;
  let rest = url.trim();
  let scheme = '';
  const sep = rest.indexOf('://');
  if (sep >= 0) { scheme = rest.slice(0, sep).toLowerCase(); rest = rest.slice(sep + 3); }
  let authority = rest.split(/[/?#]/)[0];
  const at = authority.lastIndexOf('@');
  if (at >= 0) authority = authority.slice(at + 1);
  authority = authority.toLowerCase();
  let host, portText = null;
  if (authority.startsWith('[')) {
    const close = authority.indexOf(']');
    if (close < 0) return null;
    host = authority.slice(0, close + 1);
    const after = authority.slice(close + 1);
    if (after !== '') { if (!after.startsWith(':')) return null; portText = after.slice(1); }
  } else {
    const colon = authority.lastIndexOf(':');
    if (colon >= 0) { host = authority.slice(0, colon); portText = authority.slice(colon + 1); } else host = authority;
  }
  let port = null;
  if (portText !== null) {
    if (!/^[0-9]{1,5}$/.test(portText)) return null;
    port = Number(portText);
    if (port < 1 || port > 65535) return null;
  }
  const inner = host.startsWith('[') && host.endsWith(']') ? host.slice(1, -1) : null;
  const hostOk = inner !== null
    ? (inner.length > 0 && /^[0-9a-f:.]+$/.test(inner))
    : (host.length > 0 && host.length <= 253 && /^[a-z0-9.-]+$/.test(host) && !/^[.-]/.test(host) && !host.endsWith('.') && host.indexOf('..') < 0);
  if (!hostOk) return null;
  const dflt = (scheme === 'http' || scheme === 'ws') ? 80 : ((scheme === 'https' || scheme === 'wss') ? 443 : null);
  // The sender's reading. A URL with no scheme is read as http would read its host.
  let sent;
  try { sent = new URL(scheme ? url.trim() : 'http://' + rest); } catch (_) { return null; }
  const known = { 'http:': 80, 'ws:': 80, 'https:': 443, 'wss:': 443 };
  const sentPort = sent.port ? Number(sent.port) : (sent.protocol in known ? known[sent.protocol] : null);
  const handPort = port !== null ? port : (scheme ? dflt : 80);
  if (sent.hostname !== host || sentPort !== handPort) return null;
  return (port !== null && port !== dflt) ? host + ':' + port : host;
}

/** This page's own audience: the address it was served from (location.host, lowercase). */
function pqAudience() {
  const loc = (typeof location !== 'undefined' && location) ? location : null;
  return loc ? pqAudienceOf((loc.protocol || 'https:') + '//' + loc.host) : null;
}

/** The text signed for a request: the domain line, the audience, the purpose, the time (ms). */
function pqRequestPreimage(audience, purpose, timestamp) {
  return PQ_REQUEST_DOMAIN + '\n' + audience + '\n' + purpose + '\n' + timestamp;
}

/** The text signed to answer a relay's identify challenge. */
function pqIdentifyPreimage(audience, nonce, publicKeyHex) {
  return PQ_IDENTIFY_DOMAIN + '\n' + audience + '\n' + nonce + '\n' + publicKeyHex;
}

/** Whether `nonce` has a relay's challenge shape (64 lowercase hex): nothing else is signed as a sign-in. */
function pqIsIdentifyNonce(nonce) {
  return typeof nonce === 'string' && /^[0-9a-f]{64}$/.test(nonce);
}

/**
 * Whether a chat text begins with a signing domain line (`hum/<name>/v<n>` then a newline). Such a
 * text is never signed as a chat message (its signature would be the bytes of a request, a report
 * or a DM's inner signature) and the relay refuses it.
 */
function pqIsReservedChatText(text) {
  return typeof text === 'string' && /^hum\/[a-z0-9.\-]+\/v[0-9]+\n/.test(text);
}

// The relay takes each (key, purpose, time) once, so two requests started in the same millisecond
// must not share one: every request time is the next millisecond after the last this page handed
// out (the desktop's net::identity::next_request_timestamp).
let _pqLastRequestTs = 0;
/** The time a request is signed at: Date.now(), or one past the last one handed out. */
function pqRequestTimestamp() {
  const now = Date.now();
  _pqLastRequestTs = now > _pqLastRequestTs ? now : _pqLastRequestTs + 1;
  return _pqLastRequestTs;
}

/** Lowercase hex of a byte array. */
function pqHex(bytes) {
  let hex = '';
  for (let i = 0; i < bytes.length; i++) hex += bytes[i].toString(16).padStart(2, '0');
  return hex;
}

/**
 * Sign a request to this page's own server with a Dilithium3 secret: `{timestamp, sig}` (sig in
 * hex), at `timestamp` when given, else at pqRequestTimestamp(). Null when signing is not possible.
 */
async function pqSignRequest(secretKey, purpose, timestamp) {
  if (typeof purpose !== 'string' || !purpose) return null;
  const audience = pqAudience();
  if (!audience) return null;
  const ts = (typeof timestamp === 'number') ? timestamp : pqRequestTimestamp();
  const bytes = await pqSignMessage(secretKey, new TextEncoder().encode(pqRequestPreimage(audience, purpose, ts)));
  if (!bytes) return null;
  return { timestamp: ts, sig: pqHex(bytes) };
}

/**
 * The answer to a relay's identify challenge on the socket opened at `socketUrl` (the socket's own
 * `url`): `{type: 'identify_response', sig_b64, audience}`, or null when the nonce is not a relay's
 * or the URL names no address. The audience comes ONLY from the URL this page opened, never from the
 * server, so a server passing another server's challenge through gets a proof naming itself.
 */
async function pqIdentifyResponse(secretKey, publicKeyHex, socketUrl, nonce) {
  if (!pqIsIdentifyNonce(nonce)) return null;
  const audience = pqAudienceOf(socketUrl);
  if (!audience) return null;
  const bytes = await pqSignMessage(secretKey, new TextEncoder().encode(pqIdentifyPreimage(audience, nonce, publicKeyHex)));
  if (!bytes || !bytes.length) return null;
  let s = '';
  for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
  return { type: 'identify_response', sig_b64: btoa(s), audience };
}

// Exposed globally (the chat client is classic scripts, not modules).
window.pqRequestPreimage = pqRequestPreimage;
window.pqIdentifyPreimage = pqIdentifyPreimage;
window.pqIdentifyResponse = pqIdentifyResponse;
window.pqIsIdentifyNonce = pqIsIdentifyNonce;
window.pqAudienceOf = pqAudienceOf;
window.pqAudience = pqAudience;
window.pqIsReservedChatText = pqIsReservedChatText;
window.pqRequestTimestamp = pqRequestTimestamp;
window.pqSignRequest = pqSignRequest;
window.pqBlake3 = pqBlake3;
window.pqDeriveIdentity = pqDeriveIdentity;
window.pqSignMessage = pqSignMessage;
window.pqVerifyMessage = pqVerifyMessage;
window.pqDeriveKyber = pqDeriveKyber;
window.pqDmSeal = pqDmSeal;
window.pqDmOpen = pqDmOpen;

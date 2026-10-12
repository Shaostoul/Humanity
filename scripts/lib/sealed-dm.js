// sealed-dm: the sealed-sender DM (v2) in Node, for the rigs' scripted players.
//
// WHY THIS EXISTS
// A scripted player (scripts/second-player.js) has to take part in DMs exactly as the desktop app
// and the website do: open what a friend sends it (a friendship pass, a follow, a household
// permit), and send its own (a contact request, a permit). Those are sealed-sender v2 DMs, and the
// desktop app reads them with src/net/dm_pq.rs, so this file is that module's twin, byte for byte
// where bytes are fixed:
//
//   inner = {"from","pad","sig","text","to","ts","v":2}   (plus "cert" for a friendship pass)
//           the JSON a Rust serde_json::Value writes: keys in sorted order (this crate's
//           serde_json has no `preserve_order`, so a json! object is a BTreeMap)
//   sig   = base64( Dilithium3.sign("hum/dm/v2\n{from}\n{to}\n{ts}\n{text}") )
//   pad   = spaces, so the inner's length lands on a bucket (DM_PAD_BUCKETS): the ciphertext's
//           length then says little about the message's
//   seal  = (ek_ct, ss) = ML-KEM-768.encapsulate(their DM key)
//           aes_key     = BLAKE3.derive_key("hum/dm-aes/v1", ss)
//           ct          = AES-256-GCM(aes_key, 12 random bytes, inner)   (the tag at its end)
//   wire  = {"ct_b64","ek_ct_b64","nonce_b64","v":2}       (standard base64, sorted like Rust's)
//
// The post-quantum parts come from the library the website itself ships, the vendored noble bundle
// web/shared/vendor/noble-pq.bundle.js (loaded by second-player.js `loadNoble` and passed in here
// as `noble`), whose output scripts/pq-kat.mjs pins to the Rust relay's. AES-256-GCM is Node's own
// crypto. Nothing is installed.
//
// scripts/tests/sealed-dm.test.js holds this file to dm_pq.rs: the domains, the padding buckets and
// the control markers are read from the Rust source, the AES key is checked against the domain
// read from there (a misspelling on BOTH sides would still round-trip, so the round trip alone
// proves nothing), and a DM the Rust code sealed (scripts/tests/fixtures/permit-note-v1.json, the
// game's own permit note) is opened and its signature checked.

"use strict";

const crypto = require("crypto");

/** BLAKE3 derive_key context for the DM's AES key (dm_pq.rs DM_AES_DOMAIN). */
const DM_AES_DOMAIN = "hum/dm-aes/v1";
/** The domain a DM's inner signature covers (dm_pq.rs DM_SIG_DOMAIN). */
const DM_SIG_DOMAIN = "hum/dm/v2";
/** Sealed plaintext size buckets, bytes (dm_pq.rs DM_PAD_BUCKETS; the web's are the same). */
const DM_PAD_BUCKETS = Object.freeze([256, 1024, 4096, 16384]);
/** What dm_pq.rs allows for the `,"pad":""` it adds, bytes (`bare.len() + 12 <= *b`). */
const DM_PAD_OVERHEAD = 12;
/** AES-GCM nonce and tag, bytes. */
const NONCE_LEN = 12;
const TAG_LEN = 16;
/** ML-KEM-768 sizes (pq_crypto.rs KYBER_EK_LEN, KYBER_CIPHERTEXT_LEN). */
const KYBER_EK_LEN = 1184;
const KYBER_CT_LEN = 1088;
/** ML-DSA-65 sizes (pq_crypto.rs DILITHIUM_PK_LEN, DILITHIUM_SIG_LEN): verify_dilithium refuses others. */
const DILITHIUM_PK_LEN = 1952;
const DILITHIUM_SIG_LEN = 3309;

// The control texts a DM may carry (dm_pq.rs; the web's crypto.js and /shared/*.js mirror them).
// Clients act on these and never show them.
const CTL_FOLLOW = "[[hum:follow]]";
const CTL_UNFOLLOW = "[[hum:unfollow]]";
const CTL_FRIEND_CERT = "[[hum:friend-cert]]";
/** A household permit (ship homes 5b): the marker, then the contract's `Permit` JSON, nothing between. */
const CTL_PERMIT = "[[hum:permit:v1]]";

const enc = new TextEncoder();
const b64 = (bytes) => Buffer.from(bytes).toString("base64");

/**
 * Standard base64, strictly: what Rust's `base64::engine::general_purpose::STANDARD` decodes (the
 * padding required). Node's own decoder skips anything it does not know, so a damaged field would
 * otherwise decode to something and fail later, somewhere less clear. Throws, naming `what`.
 */
function unb64(text, what) {
  const s = String(text ?? "").trim();
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(s)) throw new Error(`${what} is not standard base64`);
  return new Uint8Array(Buffer.from(s, "base64"));
}

/** Lowercase or uppercase hex to bytes, strictly (Rust's `hex::decode`). Throws, naming `what`. */
function unhex(text, what) {
  const s = String(text ?? "").trim();
  if (s.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(s)) throw new Error(`${what} is not hex`);
  return new Uint8Array(Buffer.from(s, "hex"));
}

/**
 * A JSON value as Rust's serde_json writes a `serde_json::Value` in this crate: compact, and every
 * object's keys in sorted order (a BTreeMap: bytewise order, which for the ASCII keys used here is
 * plain string order). Strings and integers are written as JSON.stringify writes them, which is
 * what serde_json writes too (the same escapes: `"`, `\` and the control characters). Pure.
 */
function rustJson(value) {
  return JSON.stringify(value, (_key, v) => {
    if (!v || typeof v !== "object" || Array.isArray(v)) return v;
    const out = {};
    for (const k of Object.keys(v).sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))) out[k] = v[k];
    return out;
  });
}

/** The exact words a DM's inner signature covers (dm_pq.rs `sig_preimage`). Pure. */
function sigPreimage(from, to, ts, text) {
  return `${DM_SIG_DOMAIN}\n${from}\n${to}\n${ts}\n${text}`;
}

/**
 * How many spaces of padding an inner whose bare JSON is `bareBytes` long gets (dm_pq.rs
 * `build_signed_inner_ext`): the first bucket it fits with DM_PAD_OVERHEAD to spare, padded up to
 * it; none when it fits no bucket. Pure.
 */
function padLength(bareBytes) {
  const bucket = DM_PAD_BUCKETS.find((b) => bareBytes + DM_PAD_OVERHEAD <= b) ?? bareBytes + DM_PAD_OVERHEAD;
  return Math.max(0, bucket - (bareBytes + DM_PAD_OVERHEAD));
}

/**
 * The signed, padded inner of one DM, as dm_pq.rs `build_signed_inner_ext` writes it: from
 * `identity` (second-player.js `deriveIdentity`: its `publicKeyHex` and `sign`) to `to` (their
 * Dilithium3 key, hex), signed at `ts` (ms since 1970), carrying `text`, and with `cert` (a
 * friendship pass's JSON, on a `[[hum:friend-cert]]` note) when given. Returns the JSON text.
 */
function signedInner(identity, { to, ts, text, cert = null }) {
  const from = identity.publicKeyHex;
  if (typeof to !== "string" || !to) throw new Error("a DM needs the key it goes to");
  if (!Number.isSafeInteger(ts) || ts < 0) throw new Error(`a DM's time is whole milliseconds, got ${ts}`);
  const sig = identity.sign(enc.encode(sigPreimage(from, to, ts, String(text))));
  const inner = { v: 2, from, to, ts, text: String(text), sig: b64(sig) };
  if (cert !== null && cert !== undefined) inner.cert = String(cert);
  inner.pad = " ".repeat(padLength(Buffer.byteLength(rustJson(inner), "utf8")));
  return rustJson(inner);
}

/** BLAKE3.derive_key(DM_AES_DOMAIN, shared secret): the DM's 32-byte AES key. */
function aesKey(noble, sharedSecret, domain = DM_AES_DOMAIN) {
  return noble.blake3.create({ context: enc.encode(domain), dkLen: 32 }).update(sharedSecret).digest();
}

/**
 * Seal `plaintext` for the holder of `recipientKyberB64` (their base64 ML-KEM-768 key, as the
 * relay's member list carries it): dm_pq.rs `seal`. Returns { ek_ct_b64, nonce_b64, ct_b64 }.
 */
function seal(noble, recipientKyberB64, plaintext) {
  const pub = unb64(recipientKyberB64, "the recipient's DM key");
  if (pub.length !== KYBER_EK_LEN) throw new Error(`the recipient's DM key must be ${KYBER_EK_LEN} bytes, got ${pub.length}`);
  const { cipherText, sharedSecret } = noble.ml_kem768.encapsulate(pub);
  const nonce = crypto.randomBytes(NONCE_LEN);
  const cipher = crypto.createCipheriv("aes-256-gcm", aesKey(noble, sharedSecret), nonce);
  const body = Buffer.concat([cipher.update(Buffer.from(String(plaintext), "utf8")), cipher.final(), cipher.getAuthTag()]);
  return { ek_ct_b64: b64(cipherText), nonce_b64: b64(nonce), ct_b64: b64(body) };
}

/** Seal an inner into the v2 wire envelope (dm_pq.rs `seal_v2`), the JSON text a `dm_put` carries. */
function sealV2(noble, recipientKyberB64, innerJson) {
  return rustJson({ v: 2, ...seal(noble, recipientKyberB64, innerJson) });
}

/**
 * Open a sealed DM with my own ML-KEM-768 secret key (second-player.js `deriveIdentity`'s
 * `kyberSecretKey`): dm_pq.rs `open`. Returns the plaintext; throws when it is not for me or was
 * changed on the way (the GCM tag).
 */
function open(noble, kyberSecretKey, ekCtB64, nonceB64, ctB64) {
  const kyberCt = unb64(ekCtB64, "ek_ct_b64");
  const nonce = unb64(nonceB64, "nonce_b64");
  const body = unb64(ctB64, "ct_b64");
  if (nonce.length !== NONCE_LEN) throw new Error(`the nonce must be ${NONCE_LEN} bytes, got ${nonce.length}`);
  if (kyberCt.length !== KYBER_CT_LEN) throw new Error(`the key capsule must be ${KYBER_CT_LEN} bytes, got ${kyberCt.length}`);
  if (body.length < TAG_LEN) throw new Error("the sealed body is shorter than its tag");
  const ss = noble.ml_kem768.decapsulate(kyberCt, kyberSecretKey);
  const decipher = crypto.createDecipheriv("aes-256-gcm", aesKey(noble, ss), nonce);
  decipher.setAuthTag(body.subarray(body.length - TAG_LEN));
  let plain;
  try {
    plain = Buffer.concat([decipher.update(body.subarray(0, body.length - TAG_LEN)), decipher.final()]);
  } catch {
    throw new Error("the DM does not open with this key (not for me, or changed on the way)");
  }
  return new TextDecoder("utf-8", { fatal: true }).decode(plain);
}

/** Open a v2 wire envelope (dm_pq.rs `open_v2`): its inner JSON text. Throws as `open` does. */
function openV2(noble, kyberSecretKey, envelopeJson) {
  let env;
  try {
    env = JSON.parse(envelopeJson);
  } catch {
    throw new Error("the envelope is not JSON");
  }
  if (!env || env.v !== 2) throw new Error("not a v2 DM envelope");
  if (typeof env.ek_ct_b64 !== "string" || typeof env.nonce_b64 !== "string" || typeof env.ct_b64 !== "string") {
    throw new Error("the envelope is missing a field");
  }
  return open(noble, kyberSecretKey, env.ek_ct_b64, env.nonce_b64, env.ct_b64);
}

/**
 * Read an inner and CHECK its signature against the key it says it is from (dm_pq.rs
 * `parse_verify_inner`): { from, to, ts, text, sig, cert }. Throws for anything that does not
 * check out: an unchecked `from` must never be taken for the sender.
 */
function verifyInner(noble, innerJson) {
  let v;
  try {
    v = JSON.parse(innerJson);
  } catch {
    throw new Error("the inner is not JSON");
  }
  if (!v || v.v !== 2) throw new Error("not a v2 inner");
  const str = (x) => (typeof x === "string" ? x : "");
  const from = str(v.from);
  const to = str(v.to);
  // Rust reads `ts` with as_u64 (0 for anything else): the signature then fails unless 0 was signed.
  const ts = Number.isSafeInteger(v.ts) && v.ts >= 0 ? v.ts : 0;
  const text = str(v.text);
  const sig = str(v.sig);
  if (!from || !to || !sig) throw new Error("the inner is missing from, to or sig");
  const pk = unhex(from, "the sender's key");
  const sigBytes = unb64(sig, "the signature");
  if (pk.length !== DILITHIUM_PK_LEN || sigBytes.length !== DILITHIUM_SIG_LEN) throw new Error("the sender's key or signature is the wrong size");
  if (!noble.ml_dsa65.verify(sigBytes, enc.encode(sigPreimage(from, to, ts, text)), pk)) {
    throw new Error("the sender's signature does not check out (spoofed or changed)");
  }
  return { from, to, ts, text, sig, cert: typeof v.cert === "string" ? v.cert : null };
}

/**
 * The two `dm_put` frames of one DM, as the desktop app's `control_puts` and the website's
 * `pqBuildDmPuts` make them: `theirs`, sealed to `toKyberB64` (with `friend_cert` = the pass THEY
 * gave me when `friendCert` is given, `ref` when given, `contact_request: true` for a contact
 * request), and `ours`, the same inner sealed to my own DM key and put in my own mailbox (my
 * other devices read sent mail from there). Returns { theirs, ours, inner } (inner: its JSON).
 */
function dmPuts(noble, identity, { to, toKyberB64, text, ts = Date.now(), cert = null, friendCert = null, ref = null, contactRequest = false }) {
  const inner = signedInner(identity, { to, ts, text, cert });
  const theirs = { type: "dm_put", to, content: sealV2(noble, toKyberB64, inner) };
  if (friendCert) theirs.friend_cert = friendCert;
  if (contactRequest) theirs.contact_request = true;
  if (ref) theirs.ref = ref;
  const ours = { type: "dm_put", to: identity.publicKeyHex, content: sealV2(noble, identity.kyberPublicB64, inner) };
  return { theirs, ours, inner };
}

module.exports = {
  DM_AES_DOMAIN,
  DM_SIG_DOMAIN,
  DM_PAD_BUCKETS,
  DM_PAD_OVERHEAD,
  NONCE_LEN,
  TAG_LEN,
  KYBER_EK_LEN,
  KYBER_CT_LEN,
  DILITHIUM_PK_LEN,
  DILITHIUM_SIG_LEN,
  CTL_FOLLOW,
  CTL_UNFOLLOW,
  CTL_FRIEND_CERT,
  CTL_PERMIT,
  rustJson,
  sigPreimage,
  padLength,
  signedInner,
  aesKey,
  seal,
  sealV2,
  open,
  openV2,
  verifyInner,
  dmPuts,
};

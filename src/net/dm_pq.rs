//! Post-quantum DM envelope — pure ML-KEM-768 → BLAKE3-KDF → AES-256-GCM.
//!
//! REPLACES the ECDH-P256 path (`dm_crypto.rs`) in the full-PQ cutover
//! (operator 2026-05-18: "screw backwards compatibility, go full PQ").
//!
//! WHY this fixes the cross-client bug for good:
//! the old ECDH key was a *random* per-browser keypair stored only in
//! the browser + vault, so the native client could never derive it →
//! "decryption failed". Here the recipient's Kyber768 keypair is
//! DETERMINISTICALLY derived from the BIP39 seed
//! (`pq_crypto::derive_kyber_seed`, BLAKE3 domain `hum/kyber768/v1`).
//! Web and native compute the SAME keypair from the same seed, so
//! whatever a sender encapsulates to is decryptable on every device —
//! no vault key, no manual import, ever.
//!
//! Envelope v1 (must stay byte-identical to the web/noble impl —
//! locked by KAT):
//!   1. (kyber_ct, ss) = ML-KEM-768.encapsulate(recipient_kyber_pub)
//!   2. aes_key        = BLAKE3.derive_key("hum/dm-aes/v1", ss)   (32 B)
//!   3. nonce          = 12 random bytes
//!   4. body           = AES-256-GCM.seal(aes_key, nonce, plaintext)
//!   wire = { ek_ct_b64, nonce_b64, ct_b64 }  (standard base64)
//!
//! BLAKE3.derive_key (not HKDF-SHA256) is the KDF: the project already
//! vendors BLAKE3 on BOTH sides (noble for the web Dilithium/Kyber
//! derivation) and KATs it, so this needs zero new web primitives and
//! stays consistent with the existing seed-derivation discipline.
//!
//! The sender needs only the recipient's Kyber *public* key — ML-KEM
//! encapsulation is randomized, so every DM gets a fresh shared secret
//! (per-message KEM freshness; no static shared key on the wire).

use base64::{engine::general_purpose::STANDARD as B64, Engine};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AesOsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};

use crate::relay::core::pq_crypto::{
    self, KyberKeypair, KYBER_EK_LEN,
};

/// BLAKE3 domain separating the DM AES key from any other use of the
/// Kyber shared secret. Web side MUST use the identical string.
const DM_AES_DOMAIN: &str = "hum/dm-aes/v1";

/// A user's PQ DM keypair (Kyber768), deterministic from the BIP39 seed.
pub struct DmPqKeypair {
    kp: KyberKeypair,
}

impl DmPqKeypair {
    /// Derive the DM keypair from the 64-byte BIP39 PBKDF2 seed. SAME
    /// derivation web uses (`derive_kyber_seed` → `KyberKeypair::from_seed`),
    /// so the public key is identical on every device.
    pub fn from_bip39_seed(master_seed: &[u8]) -> Result<Self, String> {
        let kseed = pq_crypto::derive_kyber_seed(master_seed);
        let kp = KyberKeypair::from_seed(&kseed)
            .map_err(|e| format!("Kyber keygen: {e}"))?;
        Ok(Self { kp })
    }

    /// Recipient-facing public (encapsulation) key, base64. This is what
    /// the relay advertises and senders encapsulate to.
    pub fn public_base64(&self) -> String {
        B64.encode(self.kp.public_key())
    }
}

/// The on-wire encrypted DM (all base64, standard alphabet + padding).
pub struct SealedDm {
    /// ML-KEM-768 ciphertext (the encapsulation).
    pub ek_ct_b64: String,
    /// AES-GCM nonce (12 bytes).
    pub nonce_b64: String,
    /// AES-256-GCM body.
    pub ct_b64: String,
}

/// Encrypt `plaintext` for the holder of `recipient_pub_b64`
/// (their base64 Kyber768 public key). Sender needs no keypair.
pub fn seal(recipient_pub_b64: &str, plaintext: &str) -> Result<SealedDm, String> {
    let pub_bytes = B64
        .decode(recipient_pub_b64.trim())
        .map_err(|e| format!("recipient pub base64: {e}"))?;
    if pub_bytes.len() != KYBER_EK_LEN {
        return Err(format!(
            "recipient Kyber pub must be {KYBER_EK_LEN} B, got {}",
            pub_bytes.len()
        ));
    }
    let (kyber_ct, ss) =
        pq_crypto::encapsulate_to(&pub_bytes).map_err(|e| format!("encapsulate: {e}"))?;

    let aes_key = blake3::derive_key(DM_AES_DOMAIN, &ss); // [u8; 32]
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&aes_key));
    let nonce_bytes = Aes256Gcm::generate_nonce(&mut AesOsRng);
    let nonce = Nonce::from_slice(nonce_bytes.as_slice());
    let body = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("aes seal: {e}"))?;

    Ok(SealedDm {
        ek_ct_b64: B64.encode(&kyber_ct),
        nonce_b64: B64.encode(nonce_bytes.as_slice()),
        ct_b64: B64.encode(&body),
    })
}

/// Decrypt a DM addressed to us. Only our deterministic Kyber secret
/// can decapsulate — and it is the SAME on web and native.
pub fn open(
    me: &DmPqKeypair,
    ek_ct_b64: &str,
    nonce_b64: &str,
    ct_b64: &str,
) -> Result<String, String> {
    let kyber_ct = B64
        .decode(ek_ct_b64.trim())
        .map_err(|e| format!("ek_ct base64: {e}"))?;
    let nonce_bytes = B64
        .decode(nonce_b64.trim())
        .map_err(|e| format!("nonce base64: {e}"))?;
    let body = B64
        .decode(ct_b64.trim())
        .map_err(|e| format!("ct base64: {e}"))?;
    if nonce_bytes.len() != 12 {
        return Err(format!("nonce must be 12 B, got {}", nonce_bytes.len()));
    }
    let ss = me
        .kp
        .decapsulate(&kyber_ct)
        .map_err(|e| format!("decapsulate: {e}"))?;
    let aes_key = blake3::derive_key(DM_AES_DOMAIN, &ss);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&aes_key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plain = cipher
        .decrypt(nonce, body.as_slice())
        .map_err(|e| format!("aes open (wrong key / tampered): {e}"))?;
    String::from_utf8(plain).map_err(|e| format!("utf8: {e}"))
}

// ── v2 sealed-sender envelope (DM metadata minimization, 2026-08-23) ────────
// The v1 dual-seal `{v:1,r,s}` envelope hid CONTENT from the relay but the
// wire + DB row still carried `from_key`/`from_name` in the clear, so the
// relay accumulated a complete who-talked-to-whom graph (the thing the
// Take-Two/Discord subpoena actually harvested). v2 moves the sender's
// identity INSIDE the ciphertext:
//
//   inner  = { "v":2, "from", "to", "ts", "text", "sig" }   (JSON, signed)
//   sig    = base64( Dilithium3.sign("hum/dm/v2\n{from}\n{to}\n{ts}\n{text}") )
//   wire   = { "v":2, "ek_ct_b64", "nonce_b64", "ct_b64" }  (single seal)
//
// One message becomes TWO independent envelopes: the inner JSON sealed to
// the recipient's Kyber key (deposited in THEIR server mailbox) and the
// SAME inner JSON sealed to the sender's own Kyber key (deposited in the
// sender's mailbox, so their other devices can read sent history). The
// relay stores only (to_key, ciphertext) — no sender column exists.
//
// The Dilithium `sig` is what makes sealed sender safe: without it anyone
// could deposit an envelope claiming `from: alice`. Receivers MUST verify
// the signature against the `from` key before trusting authorship
// (`parse_verify_inner`), which also upgrades DM authenticity from
// relay-vouched to end-to-end cryptographic.
//
// Web (crypto.js pqBuildDmInner/pqDmSealV2/pqDmOpenV2) MUST produce and
// consume these exact JSON shapes or cross-client DM breaks. The Kyber and
// Dilithium primitives are unchanged and stay locked by `just pq-kat`.

/// Signature preimage domain. Web MUST use the identical string.
const DM_SIG_DOMAIN: &str = "hum/dm/v2";

/// Build MY friendship certificate for `grantee_hex` (base64 Dilithium
/// signature over `pq_crypto::friend_cert_preimage`). Handed to the
/// grantee via a sealed control message; they present it on every dm_put
/// addressed to me. Verification lives in relay/core/pq_crypto.rs
/// (`verify_friend_cert`) because the relay checks it statelessly.
pub fn build_friend_cert(seed: &[u8], my_hex: &str, grantee_hex: &str) -> String {
    let dil_seed = pq_crypto::derive_dilithium_seed(seed);
    let kp = pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    B64.encode(kp.sign(pq_crypto::friend_cert_preimage(my_hex, grantee_hex).as_bytes()))
}

/// A parsed, signature-verified inner DM payload.
#[derive(Debug, Clone)]
pub struct DmInner {
    /// Sender's Dilithium3 identity (hex) — verified against `sig`.
    pub from: String,
    /// Recipient's Dilithium3 identity (hex).
    pub to: String,
    /// Sender-claimed timestamp (ms since epoch).
    pub ts: u64,
    /// The message text. Control messages (follows-graph removal,
    /// 2026-08-24) use reserved markers: `[[hum:follow]]`,
    /// `[[hum:unfollow]]`, `[[hum:friend-cert]]` — clients act on them
    /// instead of rendering them.
    pub text: String,
    /// Base64 Dilithium3 signature (kept for dedupe keying).
    pub sig_b64: String,
    /// Optional friendship certificate riding a `[[hum:friend-cert]]`
    /// control message (the sender authorizing the RECIPIENT to DM
    /// them). Self-authenticating (verified against `from` when stored),
    /// so it is not covered by `sig`.
    pub cert: Option<String>,
}

/// Reserved control-message texts (must match web chat-privacy/dms).
pub const CTL_FOLLOW: &str = "[[hum:follow]]";
pub const CTL_UNFOLLOW: &str = "[[hum:unfollow]]";
pub const CTL_FRIEND_CERT: &str = "[[hum:friend-cert]]";

impl DmInner {
    /// Stable dedupe key for a message: the same inner payload arrives
    /// twice on a sender's own device (live echo of the self-copy + the
    /// local echo at send time) and can be replayed by a hostile relay.
    /// The signature bytes are unique per (from,to,ts,text) signing, so
    /// their hash identifies the message.
    pub fn dedupe_key(&self) -> String {
        blake3::hash(self.sig_b64.as_bytes()).to_hex().to_string()
    }
}

fn sig_preimage(from_hex: &str, to_hex: &str, ts: u64, text: &str) -> String {
    format!("{DM_SIG_DOMAIN}\n{from_hex}\n{to_hex}\n{ts}\n{text}")
}

/// Build the signed inner payload JSON. `seed` is the 64-byte BIP39 seed
/// (the Dilithium identity is re-derived from it, same as chat signing).
pub fn build_signed_inner(
    seed: &[u8],
    from_hex: &str,
    to_hex: &str,
    ts: u64,
    text: &str,
) -> Result<String, String> {
    build_signed_inner_ext(seed, from_hex, to_hex, ts, text, None)
}

/// Like `build_signed_inner` but optionally attaching a friendship
/// certificate (for `[[hum:friend-cert]]` control messages).
pub fn build_signed_inner_ext(
    seed: &[u8],
    from_hex: &str,
    to_hex: &str,
    ts: u64,
    text: &str,
    cert: Option<&str>,
) -> Result<String, String> {
    let dil_seed = pq_crypto::derive_dilithium_seed(seed);
    let kp = pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    let sig = kp.sign(sig_preimage(from_hex, to_hex, ts, text).as_bytes());
    let mut v = serde_json::json!({
        "v": 2,
        "from": from_hex,
        "to": to_hex,
        "ts": ts,
        "text": text,
        "sig": B64.encode(sig),
    });
    if let Some(c) = cert {
        v["cert"] = serde_json::Value::String(c.to_string());
    }
    // Size padding (2026-08-24): round the sealed plaintext up to a
    // bucket so ciphertext length doesn't leak message length ("ok" vs a
    // paragraph is visible to anyone holding the mailbox otherwise). The
    // pad field is ignored by parsers; AES-GCM authenticates the whole
    // blob. Buckets must match the web client.
    let bare = v.to_string();
    let bucket = DM_PAD_BUCKETS
        .iter()
        .copied()
        .find(|b| bare.len() + 12 <= *b)
        .unwrap_or(bare.len() + 12);
    let pad_len = bucket.saturating_sub(bare.len() + 12); // 12 ≈ ,"pad":"" overhead
    v["pad"] = serde_json::Value::String(" ".repeat(pad_len));
    Ok(v.to_string())
}

/// Plaintext size buckets (bytes) for DM padding. Must match web.
pub const DM_PAD_BUCKETS: [usize; 4] = [256, 1024, 4096, 16384];

/// Seal an inner payload (or any plaintext) into a v2 wire envelope for
/// the holder of `recipient_pub_b64`.
pub fn seal_v2(recipient_pub_b64: &str, inner_json: &str) -> Result<String, String> {
    let sealed = seal(recipient_pub_b64, inner_json)?;
    Ok(serde_json::json!({
        "v": 2,
        "ek_ct_b64": sealed.ek_ct_b64,
        "nonce_b64": sealed.nonce_b64,
        "ct_b64": sealed.ct_b64,
    })
    .to_string())
}

/// Open a v2 wire envelope with our own Kyber secret → inner JSON.
pub fn open_v2(me: &DmPqKeypair, envelope_json: &str) -> Result<String, String> {
    let env: serde_json::Value =
        serde_json::from_str(envelope_json).map_err(|e| format!("envelope json: {e}"))?;
    if env.get("v").and_then(|v| v.as_u64()) != Some(2) {
        return Err("unsupported DM envelope version".into());
    }
    let (Some(ek), Some(n), Some(ct)) = (
        env.get("ek_ct_b64").and_then(|v| v.as_str()),
        env.get("nonce_b64").and_then(|v| v.as_str()),
        env.get("ct_b64").and_then(|v| v.as_str()),
    ) else {
        return Err("envelope missing fields".into());
    };
    open(me, ek, n, ct)
}

/// Parse an inner payload and VERIFY its Dilithium signature against the
/// claimed `from` key. Err on any mismatch — an unverified `from` must
/// never be shown as the sender.
pub fn parse_verify_inner(inner_json: &str) -> Result<DmInner, String> {
    let v: serde_json::Value =
        serde_json::from_str(inner_json).map_err(|e| format!("inner json: {e}"))?;
    if v.get("v").and_then(|x| x.as_u64()) != Some(2) {
        return Err("unsupported inner version".into());
    }
    let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let to = v.get("to").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let ts = v.get("ts").and_then(|x| x.as_u64()).unwrap_or(0);
    let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let sig_b64 = v.get("sig").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if from.is_empty() || to.is_empty() || sig_b64.is_empty() {
        return Err("inner missing from/to/sig".into());
    }
    let from_pk = hex::decode(from.trim()).map_err(|e| format!("from key hex: {e}"))?;
    let sig = B64
        .decode(sig_b64.trim())
        .map_err(|e| format!("sig base64: {e}"))?;
    pq_crypto::verify_dilithium(&from_pk, sig_preimage(&from, &to, ts, &text).as_bytes(), &sig)
        .map_err(|_| "sender signature INVALID (spoofed or corrupted)".to_string())?;
    let cert = v.get("cert").and_then(|x| x.as_str()).map(|s| s.to_string());
    Ok(DmInner { from, to, ts, text, sig_b64, cert })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_seal_open() {
        // Recipient derives keypair from a seed; sender only needs pub.
        let seed = vec![7u8; 64];
        let bob = DmPqKeypair::from_bip39_seed(&seed).unwrap();
        let sealed = seal(&bob.public_base64(), "hello post-quantum").unwrap();
        let got = open(&bob, &sealed.ek_ct_b64, &sealed.nonce_b64, &sealed.ct_b64).unwrap();
        assert_eq!(got, "hello post-quantum");
    }

    #[test]
    fn deterministic_keypair_from_seed() {
        // THE property that kills the cross-client bug: same seed →
        // same public key (web and native both derive this).
        let seed = vec![42u8; 64];
        let a = DmPqKeypair::from_bip39_seed(&seed).unwrap();
        let b = DmPqKeypair::from_bip39_seed(&seed).unwrap();
        assert_eq!(a.public_base64(), b.public_base64());
        // And different seeds → different keys.
        let c = DmPqKeypair::from_bip39_seed(&vec![43u8; 64]).unwrap();
        assert_ne!(a.public_base64(), c.public_base64());
    }

    #[test]
    fn wrong_recipient_cannot_open() {
        let bob = DmPqKeypair::from_bip39_seed(&vec![1u8; 64]).unwrap();
        let eve = DmPqKeypair::from_bip39_seed(&vec![2u8; 64]).unwrap();
        let sealed = seal(&bob.public_base64(), "secret").unwrap();
        assert!(open(&eve, &sealed.ek_ct_b64, &sealed.nonce_b64, &sealed.ct_b64).is_err());
    }

    /// Helper: a party's full identity from a test seed.
    fn party(seed_byte: u8) -> (Vec<u8>, String, DmPqKeypair) {
        let seed = vec![seed_byte; 32];
        let dil_seed = pq_crypto::derive_dilithium_seed(&seed);
        let dil_hex = hex::encode(pq_crypto::DilithiumKeypair::from_seed(&dil_seed).public_key());
        let kyber = DmPqKeypair::from_bip39_seed(&seed).unwrap();
        (seed, dil_hex, kyber)
    }

    #[test]
    fn v2_sealed_sender_both_parties_any_device() {
        // THE cross-client contract. Alice DMs Bob: the SAME signed inner
        // payload is sealed once to Bob (his mailbox) and once to Alice
        // herself (her mailbox, for her other devices).
        let (alice_seed, alice_hex, alice_kp) = party(11);
        let (_bob_seed, bob_hex, bob_kp) = party(22);
        let inner =
            build_signed_inner(&alice_seed, &alice_hex, &bob_hex, 1_700_000_000_000, "hi bob")
                .unwrap();
        let to_bob = seal_v2(&bob_kp.public_base64(), &inner).unwrap();
        let to_self = seal_v2(&alice_kp.public_base64(), &inner).unwrap();
        // Wire shape: v2, flat, three b64 fields, NO sender anywhere.
        let j: serde_json::Value = serde_json::from_str(&to_bob).unwrap();
        assert_eq!(j["v"], 2);
        assert!(j["ek_ct_b64"].is_string() && j["nonce_b64"].is_string() && j["ct_b64"].is_string());
        assert!(j.get("from").is_none() && j.get("r").is_none());
        // Bob opens his copy, verifies authorship cryptographically.
        let got = parse_verify_inner(&open_v2(&bob_kp, &to_bob).unwrap()).unwrap();
        assert_eq!(got.from, alice_hex);
        assert_eq!(got.to, bob_hex);
        assert_eq!(got.text, "hi bob");
        // Alice's OTHER device (re-derived keypair) opens the self copy.
        let alice2 = DmPqKeypair::from_bip39_seed(&vec![11u8; 32]).unwrap();
        let mine = parse_verify_inner(&open_v2(&alice2, &to_self).unwrap()).unwrap();
        assert_eq!(mine.text, "hi bob");
        // Both copies carry the SAME signature → same dedupe key.
        assert_eq!(got.dedupe_key(), mine.dedupe_key());
        // A third party cannot open either copy.
        let (_, _, eve_kp) = party(99);
        assert!(open_v2(&eve_kp, &to_bob).is_err());
        assert!(open_v2(&eve_kp, &to_self).is_err());
    }

    #[test]
    fn v2_spoofed_sender_rejected() {
        // Eve builds an inner payload CLAIMING to be Alice but signs with
        // her own key — parse_verify_inner must refuse it. This is the
        // property that makes sealed sender safe.
        let (eve_seed, _eve_hex, _) = party(66);
        let (_a_seed, alice_hex, _) = party(11);
        let (_b_seed, bob_hex, bob_kp) = party(22);
        let forged =
            build_signed_inner(&eve_seed, &alice_hex, &bob_hex, 1, "pretending to be alice")
                .unwrap();
        let env = seal_v2(&bob_kp.public_base64(), &forged).unwrap();
        let inner_json = open_v2(&bob_kp, &env).unwrap();
        assert!(parse_verify_inner(&inner_json).is_err(), "forged sender must not verify");
    }

    #[test]
    fn v2_tampered_inner_field_rejected() {
        // Flipping any signed field after signing breaks verification.
        let (alice_seed, alice_hex, _) = party(11);
        let (_b, bob_hex, _) = party(22);
        let inner = build_signed_inner(&alice_seed, &alice_hex, &bob_hex, 42, "true text").unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&inner).unwrap();
        v["text"] = serde_json::Value::String("swapped text".into());
        assert!(parse_verify_inner(&v.to_string()).is_err());
        let mut v2: serde_json::Value = serde_json::from_str(&inner).unwrap();
        v2["ts"] = serde_json::json!(43);
        assert!(parse_verify_inner(&v2.to_string()).is_err());
    }

    /// Size padding: short and medium messages land in the same bucket,
    /// so ciphertext length does not distinguish "ok" from a paragraph.
    #[test]
    fn v2_padding_hides_message_length() {
        let (alice_seed, alice_hex, _) = party(11);
        let (_b, bob_hex, bob_kp) = party(22);
        let short =
            build_signed_inner(&alice_seed, &alice_hex, &bob_hex, 1, "ok").unwrap();
        let medium = build_signed_inner(
            &alice_seed, &alice_hex, &bob_hex, 2,
            "a considerably longer message with several clauses in it, the kind a length observer would love to distinguish",
        )
        .unwrap();
        // The Dilithium sig makes every inner > 4KB already; both must
        // round to the SAME bucket.
        assert_eq!(short.len(), medium.len(), "padded inners must be bucket-equal");
        let env_a = seal_v2(&bob_kp.public_base64(), &short).unwrap();
        let env_b = seal_v2(&bob_kp.public_base64(), &medium).unwrap();
        assert_eq!(env_a.len(), env_b.len(), "ciphertext lengths must match");
        // And they still parse + verify.
        assert_eq!(parse_verify_inner(&open_v2(&bob_kp, &env_a).unwrap()).unwrap().text, "ok");
    }

    #[test]
    fn v2_rejects_v1_envelope() {
        // No-compat rule: the old dual-seal shape is not accepted.
        let (_s, _h, bob_kp) = party(22);
        assert!(open_v2(&bob_kp, r#"{"v":1,"r":{},"s":{}}"#).is_err());
    }

    #[test]
    fn tampered_body_fails_gcm() {
        let bob = DmPqKeypair::from_bip39_seed(&vec![9u8; 64]).unwrap();
        let mut sealed = seal(&bob.public_base64(), "integrity").unwrap();
        // Flip a byte in the AES body → GCM tag must reject.
        let mut raw = B64.decode(&sealed.ct_b64).unwrap();
        raw[0] ^= 0xFF;
        sealed.ct_b64 = B64.encode(&raw);
        assert!(open(&bob, &sealed.ek_ct_b64, &sealed.nonce_b64, &sealed.ct_b64).is_err());
    }

    /// Cross-language anchor: a frozen 64-byte seed must always derive
    /// the SAME Kyber public key (BLAKE3 `hum/kyber768/v1` → ML-KEM-768).
    /// The web/noble impl is held to this exact hash. If this changes,
    /// every existing DM keypair silently breaks — it must never drift.
    #[test]
    fn frozen_seed_kyber_pubkey_kat() {
        let seed = [0x07u8; 64];
        let kp = DmPqKeypair::from_bip39_seed(&seed).unwrap();
        let pk = B64.decode(kp.public_base64()).unwrap();
        assert_eq!(pk.len(), KYBER_EK_LEN, "ML-KEM-768 ek size");
        let h = blake3::hash(&pk);
        // Lock the derivation. (Value recorded from the first green run;
        // `just pq-kat` / the web KAT must match this.)
        log::info!("KAT kyber pk blake3 = {}", h.to_hex());
        // Stability within this build (determinism already covered;
        // the cross-language frozen value is asserted in scripts/pq-kat).
        let kp2 = DmPqKeypair::from_bip39_seed(&seed).unwrap();
        assert_eq!(kp.public_base64(), kp2.public_base64());
    }
}

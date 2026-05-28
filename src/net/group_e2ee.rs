//! P2P group end-to-end encrypted messaging — native side.
//!
//! Mirrors `web/shared/pq-object.js`'s Phase-2 helpers byte-for-byte so a
//! group_epoch_key_v1 / group_msg_v1 produced by either side opens cleanly on
//! the other. Scheme is identical to the DM envelope (`dm_pq.rs`):
//!
//!   1. epoch_key = 32 random bytes (the shared symmetric key for the epoch).
//!   2. For each member, seal epoch_key (base64-encoded) to their Kyber pub via
//!      ML-KEM-768 → BLAKE3-KDF("hum/dm-aes/v1") → AES-256-GCM (= `dm_pq::seal`).
//!      The signed `group_epoch_key_v1` payload bundles the sealed copies.
//!   3. Messages are AES-256-GCM(epoch_key, plaintext) wrapped in a signed
//!      `group_msg_v1` with payload `{epoch, nonce, ct}` (canonical CBOR).
//!
//! The relay stores the ciphertext + serves it through the read endpoints
//! `/api/v2/groups/{id}/epoch` and `/messages`; it never holds a key and
//! cannot decrypt anything (Phase 2 backend in v0.295.0).

use aes_gcm::{
    AeadCore, Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng as AesOsRng},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ciborium::Value;
use rand::RngCore;

use crate::net::dm_pq::{self, DmPqKeypair};
use crate::relay::core::encoding::{cbor_bytes, cbor_int, cbor_map, from_canonical_bytes};
use crate::relay::core::object::ObjectBuilder;

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A group member that an epoch key can be sealed to.
pub struct GroupMemberKey {
    /// 32-char hex fingerprint of the member's Dilithium pubkey
    /// (= first 16 bytes of BLAKE3(pubkey), hex). Matches the relay's
    /// `author_fingerprint` function.
    pub fp: String,
    /// Base64-encoded ML-KEM-768 public key.
    pub kyber_pub_b64: String,
}

/// Parsed `group_epoch_key_v1` payload — the sealed copies of one epoch key.
pub struct EpochKeyPayload {
    pub epoch: u64,
    pub recipients: Vec<RecipientEntry>,
}

/// One member's sealed copy of the epoch key.
pub struct RecipientEntry {
    pub fp: String,
    pub ek_ct_b64: String,
    pub nonce_b64: String,
    pub ct_b64: String,
}

/// Generate a fresh 32-byte AES-256 epoch key.
pub fn random_epoch_key() -> Vec<u8> {
    let mut k = vec![0u8; 32];
    rand::rng().fill_bytes(&mut k);
    k
}

/// Build an unsigned `group_epoch_key_v1` ObjectBuilder. The caller signs +
/// submits it via `api_v2::submit_signed_object`. The relay's
/// `index_group_epoch_key` validates that the author is the group creator
/// (Phase 1) before projecting it.
pub fn build_group_epoch_key_v1(
    group_id: &str,
    epoch: u64,
    epoch_key: &[u8],
    members: &[GroupMemberKey],
) -> Result<ObjectBuilder, String> {
    if epoch_key.len() != 32 {
        return Err(format!("epoch key must be 32 bytes, got {}", epoch_key.len()));
    }
    let epoch_key_b64 = B64.encode(epoch_key);
    let mut recipients_cbor: Vec<Value> = Vec::with_capacity(members.len());
    for m in members {
        let sealed = dm_pq::seal(&m.kyber_pub_b64, &epoch_key_b64)
            .map_err(|e| format!("seal to {}: {e}", &m.fp))?;
        let ek_ct = B64.decode(&sealed.ek_ct_b64).map_err(|e| format!("ek_ct b64: {e}"))?;
        let nonce = B64.decode(&sealed.nonce_b64).map_err(|e| format!("nonce b64: {e}"))?;
        let ct = B64.decode(&sealed.ct_b64).map_err(|e| format!("ct b64: {e}"))?;
        recipients_cbor.push(Value::Map(vec![
            (Value::Text("fp".into()), Value::Text(m.fp.clone())),
            (Value::Text("ek_ct".into()), Value::Bytes(ek_ct)),
            (Value::Text("nonce".into()), Value::Bytes(nonce)),
            (Value::Text("ct".into()), Value::Bytes(ct)),
        ]));
    }
    let payload = cbor_map(vec![
        ("epoch", cbor_int(epoch)),
        ("recipients", Value::Array(recipients_cbor)),
    ]);
    ObjectBuilder::new("group_epoch_key_v1")
        .reference(group_id)
        .created_at(now_millis())
        .payload_cbor(&payload)
        .map_err(|e| format!("payload: {e}"))
}

/// Decode a `group_epoch_key_v1` payload back into structured form so the
/// caller can find their own recipient entry and check coverage.
pub fn parse_group_epoch_key_payload(payload_bytes: &[u8]) -> Result<EpochKeyPayload, String> {
    let v = from_canonical_bytes(payload_bytes).map_err(|e| format!("cbor: {e}"))?;
    let map = match v {
        Value::Map(m) => m,
        _ => return Err("payload not a map".into()),
    };
    let mut epoch: u64 = 0;
    let mut recipients: Vec<RecipientEntry> = Vec::new();
    for (k, val) in map {
        let key = match k { Value::Text(s) => s, _ => continue };
        match key.as_str() {
            "epoch" => {
                if let Value::Integer(i) = val {
                    let raw: i128 = i.into();
                    if (0..=u64::MAX as i128).contains(&raw) {
                        epoch = raw as u64;
                    }
                }
            }
            "recipients" => {
                if let Value::Array(arr) = val {
                    for rcp_val in arr {
                        let rcp_map = match rcp_val { Value::Map(m) => m, _ => continue };
                        let mut fp = String::new();
                        let mut ek_ct: Vec<u8> = Vec::new();
                        let mut nonce: Vec<u8> = Vec::new();
                        let mut ct: Vec<u8> = Vec::new();
                        for (rk, rv) in rcp_map {
                            let rkey = match rk { Value::Text(s) => s, _ => continue };
                            match (rkey.as_str(), rv) {
                                ("fp", Value::Text(s)) => fp = s,
                                ("ek_ct", Value::Bytes(b)) => ek_ct = b,
                                ("nonce", Value::Bytes(b)) => nonce = b,
                                ("ct", Value::Bytes(b)) => ct = b,
                                _ => {}
                            }
                        }
                        recipients.push(RecipientEntry {
                            fp,
                            ek_ct_b64: B64.encode(&ek_ct),
                            nonce_b64: B64.encode(&nonce),
                            ct_b64: B64.encode(&ct),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(EpochKeyPayload { epoch, recipients })
}

/// Find MY recipient entry by `my_fp` and ML-KEM-decapsulate the sealed
/// epoch key with my Kyber secret. Returns `(epoch, 32-byte epoch_key)`.
pub fn open_epoch_key(
    payload: &EpochKeyPayload,
    my_fp: &str,
    me: &DmPqKeypair,
) -> Result<(u64, Vec<u8>), String> {
    let mine = payload
        .recipients
        .iter()
        .find(|r| r.fp == my_fp)
        .ok_or_else(|| "no recipient entry for my fingerprint".to_string())?;
    let plain = dm_pq::open(me, &mine.ek_ct_b64, &mine.nonce_b64, &mine.ct_b64)?;
    let epoch_key = B64
        .decode(plain.trim())
        .map_err(|e| format!("epoch key base64: {e}"))?;
    if epoch_key.len() != 32 {
        return Err(format!("epoch key must be 32 bytes, got {}", epoch_key.len()));
    }
    Ok((payload.epoch, epoch_key))
}

/// AES-256-GCM encrypt a plaintext UTF-8 string under the epoch key.
/// Returns `(nonce, ciphertext)`.
pub fn aes_gcm_encrypt(epoch_key: &[u8], plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    if epoch_key.len() != 32 {
        return Err(format!("epoch key must be 32 bytes, got {}", epoch_key.len()));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(epoch_key));
    let nonce_bytes = Aes256Gcm::generate_nonce(&mut AesOsRng);
    let nonce = Nonce::from_slice(nonce_bytes.as_slice());
    let ct = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("aes encrypt: {e}"))?;
    Ok((nonce_bytes.to_vec(), ct))
}

/// AES-256-GCM decrypt. Returns the plaintext UTF-8 string or an error if the
/// key/nonce/ct doesn't authenticate (wrong key, tampered ciphertext, etc.).
pub fn aes_gcm_decrypt(epoch_key: &[u8], nonce: &[u8], ct: &[u8]) -> Result<String, String> {
    if epoch_key.len() != 32 {
        return Err(format!("epoch key must be 32 bytes, got {}", epoch_key.len()));
    }
    if nonce.len() != 12 {
        return Err(format!("nonce must be 12 bytes, got {}", nonce.len()));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(epoch_key));
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| format!("aes decrypt: {e}"))?;
    String::from_utf8(plain).map_err(|e| format!("utf8: {e}"))
}

/// Build an unsigned `group_msg_v1` ObjectBuilder: AES-GCM ciphertext under
/// `epoch_key`, payload `{epoch, nonce, ct}`.
pub fn build_group_msg_v1(
    group_id: &str,
    epoch: u64,
    epoch_key: &[u8],
    plaintext: &str,
) -> Result<ObjectBuilder, String> {
    let (nonce, ct) = aes_gcm_encrypt(epoch_key, plaintext)?;
    let payload = cbor_map(vec![
        ("epoch", cbor_int(epoch)),
        ("nonce", cbor_bytes(&nonce)),
        ("ct", cbor_bytes(&ct)),
    ]);
    ObjectBuilder::new("group_msg_v1")
        .reference(group_id)
        .created_at(now_millis())
        .payload_cbor(&payload)
        .map_err(|e| format!("payload: {e}"))
}

/// Parse + decrypt a `group_msg_v1` payload. Returns the plaintext string,
/// or an error if the payload is malformed or the key/nonce/ct don't match.
pub fn open_group_msg(payload_bytes: &[u8], epoch_key: &[u8]) -> Result<String, String> {
    let v = from_canonical_bytes(payload_bytes).map_err(|e| format!("cbor: {e}"))?;
    let map = match v {
        Value::Map(m) => m,
        _ => return Err("payload not a map".into()),
    };
    let mut nonce: Vec<u8> = Vec::new();
    let mut ct: Vec<u8> = Vec::new();
    for (k, val) in map {
        let key = match k { Value::Text(s) => s, _ => continue };
        match (key.as_str(), val) {
            ("nonce", Value::Bytes(b)) => nonce = b,
            ("ct", Value::Bytes(b)) => ct = b,
            _ => {}
        }
    }
    aes_gcm_decrypt(epoch_key, &nonce, &ct)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end round-trip: seal epoch key to one recipient, that recipient
    /// opens it, then encrypts a message under it and decrypts it back.
    #[test]
    fn group_e2ee_roundtrip() {
        let bip39_seed = [42u8; 32]; // deterministic test seed
        let me = DmPqKeypair::from_bip39_seed(&bip39_seed).unwrap();
        let my_kyber_pub = me.public_base64();
        let my_fp = "deadbeefcafef00ddeadbeefcafef00d".to_string();

        // Mint epoch key + seal it to ourselves.
        let epoch_key = random_epoch_key();
        let members = vec![GroupMemberKey {
            fp: my_fp.clone(),
            kyber_pub_b64: my_kyber_pub,
        }];
        let builder =
            build_group_epoch_key_v1("grp_test", 1, &epoch_key, &members).unwrap();
        // Sanity: builder produces canonical payload bytes parseable back.
        // (We can't run the full sign cycle here without a Dilithium keypair —
        // that's covered by groups_p2p::group_v1_canonical_kat.)
        let payload_bytes = {
            // Trick: build a dummy keypair just to extract the payload, then drop the sig.
            // ObjectBuilder.payload is private; use the builder's serialization path.
            use crate::relay::core::pq_crypto::DilithiumKeypair;
            let kp = DilithiumKeypair::generate().unwrap();
            let signed = builder.sign(&kp).unwrap();
            signed.payload
        };
        let parsed = parse_group_epoch_key_payload(&payload_bytes).unwrap();
        assert_eq!(parsed.epoch, 1);
        assert_eq!(parsed.recipients.len(), 1);
        assert_eq!(parsed.recipients[0].fp, my_fp);

        // Open our own copy.
        let (epoch, recovered_key) = open_epoch_key(&parsed, &my_fp, &me).unwrap();
        assert_eq!(epoch, 1);
        assert_eq!(recovered_key, epoch_key);

        // Now seal + open a message.
        let msg = "hello, group";
        let msg_builder = build_group_msg_v1("grp_test", 1, &epoch_key, msg).unwrap();
        let msg_payload = {
            use crate::relay::core::pq_crypto::DilithiumKeypair;
            let kp = DilithiumKeypair::generate().unwrap();
            msg_builder.sign(&kp).unwrap().payload
        };
        let plain = open_group_msg(&msg_payload, &epoch_key).unwrap();
        assert_eq!(plain, msg);
    }

    #[test]
    fn wrong_recipient_cannot_open_epoch_key() {
        let me = DmPqKeypair::from_bip39_seed(&[1u8; 32]).unwrap();
        let other = DmPqKeypair::from_bip39_seed(&[2u8; 32]).unwrap();
        let epoch_key = random_epoch_key();
        let members = vec![GroupMemberKey {
            fp: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            kyber_pub_b64: me.public_base64(),
        }];
        let builder = build_group_epoch_key_v1("grp_x", 1, &epoch_key, &members).unwrap();
        let payload = {
            use crate::relay::core::pq_crypto::DilithiumKeypair;
            let kp = DilithiumKeypair::generate().unwrap();
            builder.sign(&kp).unwrap().payload
        };
        let parsed = parse_group_epoch_key_payload(&payload).unwrap();
        // The other keypair has no entry → error.
        assert!(open_epoch_key(&parsed, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", &other).is_err());
        // Wrong key (with my fp but other's Kyber secret) → decap fails.
        assert!(open_epoch_key(&parsed, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", &other).is_err());
    }
}

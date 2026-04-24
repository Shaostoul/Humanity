//! Decentralized Identifier (DID) layer for HumanityOS.
//!
//! Format: `did:hum:<base58(BLAKE3(public_key)[..16])>`
//!
//! - 16-byte fingerprint truncated from BLAKE3 of the full Dilithium3 pubkey
//! - Base58-encoded for ~22 chars total — fits in QR codes, nav UIs, citations
//! - Collision-resistant (2^64 effective security; fine for any human-scale identity space)
//!
//! ## DIDs and rotations (current limitation, Phase 1.5 will lift this)
//!
//! Today the DID is computed from the *current* Dilithium3 public key. When a user
//! rotates their key, their DID also changes. Phase 1.5 introduces a stable DID
//! anchored on the *initial* key with explicit `did_create_v1` registration; the
//! `dids` table will map stable-DID → current-pubkey via the rotation chain.
//!
//! For now (no one has rotated yet), per-pubkey DIDs are sufficient.
//!
//! ## Adapter formats (interop)
//!
//! - `did:hum:` — native HumanityOS PQ identity (this module)
//! - `did:key:` — generic key-as-DID (W3C); we accept Dilithium3 multikey when seen
//! - `did:web:` — domain-controlled identity for institutional issuers (Phase 1+)

use crate::relay::core::error::{Error, Result};

/// The HumanityOS DID method prefix.
pub const DID_HUM_PREFIX: &str = "did:hum:";

/// Length of a DID fingerprint, in bytes.
pub const FINGERPRINT_LEN: usize = 16;

/// A 16-byte DID fingerprint (truncated BLAKE3 of a full public key).
pub type Fingerprint = [u8; FINGERPRINT_LEN];

/// Compute the 16-byte fingerprint of a public key (any size, any crypto suite).
///
/// = BLAKE3(public_key) truncated to 16 bytes.
pub fn fingerprint_of(public_key: &[u8]) -> Fingerprint {
    let h = blake3::hash(public_key);
    let mut fp = [0u8; FINGERPRINT_LEN];
    fp.copy_from_slice(&h.as_bytes()[..FINGERPRINT_LEN]);
    fp
}

/// Convert a 16-byte fingerprint into a `did:hum:<base58>` string.
pub fn fingerprint_to_did(fp: &Fingerprint) -> String {
    format!("{}{}", DID_HUM_PREFIX, bs58::encode(fp).into_string())
}

/// Convenience: compute the DID for a given public key.
pub fn did_for_pubkey(public_key: &[u8]) -> String {
    fingerprint_to_did(&fingerprint_of(public_key))
}

/// Hex encoding of a fingerprint (matches the `author_fp` column in `signed_objects`).
pub fn fingerprint_to_hex(fp: &Fingerprint) -> String {
    fp.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse a `did:hum:<base58>` string into its 16-byte fingerprint.
///
/// Returns Err with a precise reason if the prefix, base58, or length is wrong.
pub fn parse_did_hum(did: &str) -> Result<Fingerprint> {
    let suffix = did.strip_prefix(DID_HUM_PREFIX).ok_or_else(|| Error::InvalidField {
        field: "did".into(),
        reason: format!("expected '{DID_HUM_PREFIX}<base58>' format"),
    })?;
    let bytes = bs58::decode(suffix)
        .into_vec()
        .map_err(|e| Error::InvalidField {
            field: "did".into(),
            reason: format!("base58 decode: {e}"),
        })?;
    if bytes.len() != FINGERPRINT_LEN {
        return Err(Error::InvalidField {
            field: "did".into(),
            reason: format!("expected {FINGERPRINT_LEN} bytes, got {}", bytes.len()),
        });
    }
    let mut fp = [0u8; FINGERPRINT_LEN];
    fp.copy_from_slice(&bytes);
    Ok(fp)
}

/// Get the hex `author_fp` form (for matching against `signed_objects.author_fp`)
/// from a `did:hum:` identifier.
pub fn did_to_author_fp_hex(did: &str) -> Result<String> {
    let fp = parse_did_hum(did)?;
    Ok(fingerprint_to_hex(&fp))
}

/// Validate that a string is a syntactically well-formed DID (any method).
/// Returns Ok with the parsed `(method, identifier)` pair, or Err.
///
/// This is the lightweight syntactic check — full method resolution is the
/// resolver's job.
pub fn split_did(did: &str) -> Result<(String, String)> {
    if !did.starts_with("did:") {
        return Err(Error::InvalidField {
            field: "did".into(),
            reason: "must start with 'did:'".into(),
        });
    }
    let rest = &did["did:".len()..];
    let mut parts = rest.splitn(2, ':');
    let method = parts.next().ok_or_else(|| Error::InvalidField {
        field: "did".into(),
        reason: "missing method".into(),
    })?;
    let id = parts.next().ok_or_else(|| Error::InvalidField {
        field: "did".into(),
        reason: "missing method-specific id".into(),
    })?;
    if method.is_empty() || id.is_empty() {
        return Err(Error::InvalidField {
            field: "did".into(),
            reason: "method and id must be non-empty".into(),
        });
    }
    Ok((method.to_string(), id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_of_dilithium_pubkey_is_16_bytes() {
        let pubkey = vec![0xABu8; 1952];
        let fp = fingerprint_of(&pubkey);
        assert_eq!(fp.len(), 16);
    }

    #[test]
    fn did_round_trip() {
        let pubkey = vec![0x42u8; 1952];
        let did = did_for_pubkey(&pubkey);
        assert!(did.starts_with("did:hum:"));

        let parsed = parse_did_hum(&did).unwrap();
        assert_eq!(parsed, fingerprint_of(&pubkey));
    }

    #[test]
    fn different_pubkeys_yield_different_dids() {
        let pk1 = vec![0x01u8; 1952];
        let pk2 = vec![0x02u8; 1952];
        assert_ne!(did_for_pubkey(&pk1), did_for_pubkey(&pk2));
    }

    #[test]
    fn parse_rejects_wrong_prefix() {
        assert!(parse_did_hum("did:key:abcd").is_err());
        assert!(parse_did_hum("not-a-did").is_err());
    }

    #[test]
    fn parse_rejects_wrong_length() {
        let too_short = format!("did:hum:{}", bs58::encode(&[0u8; 8]).into_string());
        assert!(parse_did_hum(&too_short).is_err());
        let too_long = format!("did:hum:{}", bs58::encode(&[0u8; 32]).into_string());
        assert!(parse_did_hum(&too_long).is_err());
    }

    #[test]
    fn parse_rejects_invalid_base58() {
        assert!(parse_did_hum("did:hum:!!!not-base58!!!").is_err());
    }

    #[test]
    fn split_did_extracts_method_and_id() {
        let (method, id) = split_did("did:hum:abc123").unwrap();
        assert_eq!(method, "hum");
        assert_eq!(id, "abc123");

        let (method, id) = split_did("did:web:example.com").unwrap();
        assert_eq!(method, "web");
        assert_eq!(id, "example.com");
    }

    #[test]
    fn split_did_rejects_malformed() {
        assert!(split_did("not-a-did").is_err());
        assert!(split_did("did:").is_err());
        assert!(split_did("did:hum").is_err());
        assert!(split_did("did::id").is_err());
    }

    #[test]
    fn fingerprint_to_hex_matches_author_fp_format() {
        // The hex form must equal author_fingerprint() from signed_objects storage.
        let pubkey = vec![0x99u8; 1952];
        let fp = fingerprint_of(&pubkey);
        let hex = fingerprint_to_hex(&fp);
        assert_eq!(hex.len(), 32); // 16 bytes * 2 hex chars
    }

    #[test]
    fn did_to_author_fp_hex_round_trip() {
        let pubkey = vec![0xCCu8; 1952];
        let did = did_for_pubkey(&pubkey);
        let fp_hex = did_to_author_fp_hex(&did).unwrap();
        let direct_hex = fingerprint_to_hex(&fingerprint_of(&pubkey));
        assert_eq!(fp_hex, direct_hex);
    }
}

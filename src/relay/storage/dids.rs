//! DID resolution layer.
//!
//! Given a `did:hum:<base58>` identifier, resolves to the current Dilithium3
//! public key by looking up any signed_objects row whose `author_fp` matches.
//!
//! Phase 1 keeps this simple: DID = fingerprint of *current* key. When a key
//! rotation is introduced (Phase 4 social recovery, or earlier voluntary rotation),
//! the rotation's `key_rotation_v1` object will record old_pubkey → new_pubkey
//! and this resolver will follow the chain.

use rusqlite::{OptionalExtension, params};

use super::Storage;
use crate::relay::core::did::{Fingerprint, fingerprint_to_did};

/// Resolved DID → identity record. Sufficient for verifying signatures from this DID.
#[derive(Debug, Clone)]
pub struct DidResolution {
    pub did: String,
    pub current_pubkey: Vec<u8>,
    pub author_fp_hex: String,
    /// First time this server saw an object signed by this key.
    pub first_seen: i64,
    /// Most recent object signed by this key.
    pub last_seen: i64,
    /// Total objects this server has from this DID.
    pub object_count: u64,
}

impl Storage {
    /// Resolve a DID fingerprint (16 bytes) to a `DidResolution` by looking up
    /// any signed_objects row with matching `author_fp`.
    ///
    /// Returns `Ok(None)` if no objects from this DID have been seen.
    pub fn resolve_did_fp(&self, fp: &Fingerprint) -> Result<Option<DidResolution>, rusqlite::Error> {
        let fp_hex = crate::relay::core::did::fingerprint_to_hex(fp);

        self.with_conn(|conn| {
            // Pull pubkey + first/last seen + count in a single pass.
            let row = conn
                .query_row(
                    "SELECT author_pubkey,
                            MIN(received_at) AS first_seen,
                            MAX(received_at) AS last_seen,
                            COUNT(*)         AS object_count
                     FROM signed_objects
                     WHERE author_fp = ?1
                     GROUP BY author_pubkey
                     ORDER BY last_seen DESC
                     LIMIT 1",
                    params![fp_hex],
                    |r| {
                        Ok((
                            r.get::<_, Vec<u8>>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()?;

            Ok(row.map(|(pubkey, first_seen, last_seen, count)| DidResolution {
                did: fingerprint_to_did(fp),
                current_pubkey: pubkey,
                author_fp_hex: fp_hex.clone(),
                first_seen,
                last_seen,
                object_count: count as u64,
            }))
        })
    }

    /// Convenience: resolve from the canonical `did:hum:` string.
    pub fn resolve_did(&self, did: &str) -> Result<Option<DidResolution>, String> {
        let fp = crate::relay::core::did::parse_did_hum(did)
            .map_err(|e| format!("invalid did: {e}"))?;
        self.resolve_did_fp(&fp)
            .map_err(|e| format!("storage: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::did::did_for_pubkey;
    use crate::relay::core::encoding::{cbor_map, cbor_text};
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_did_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    #[test]
    fn resolves_did_from_signed_object() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        let payload = cbor_map(vec![("greeting", cbor_text("hello"))]);
        let obj = ObjectBuilder::new("post")
            .payload_cbor(&payload)
            .unwrap()
            .sign(&kp)
            .unwrap();
        db.put_signed_object(&obj, None).unwrap();

        let res = db.resolve_did(&did).unwrap().expect("resolved");
        assert_eq!(res.did, did);
        assert_eq!(res.current_pubkey, kp.public_key());
        assert_eq!(res.object_count, 1);
    }

    #[test]
    fn unknown_did_returns_none() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());
        // Never store any object for this DID
        let res = db.resolve_did(&did).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn malformed_did_returns_err() {
        let db = make_test_storage();
        assert!(db.resolve_did("not-a-did").is_err());
        assert!(db.resolve_did("did:web:example.com").is_err());
    }

    #[test]
    fn object_count_matches_inserts() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        for i in 0..3 {
            let payload = cbor_map(vec![("idx", cbor_text(&i.to_string()))]);
            let obj = ObjectBuilder::new("post")
                .payload_cbor(&payload)
                .unwrap()
                .sign(&kp)
                .unwrap();
            db.put_signed_object(&obj, None).unwrap();
        }

        let res = db.resolve_did(&did).unwrap().unwrap();
        assert_eq!(res.object_count, 3);
    }
}

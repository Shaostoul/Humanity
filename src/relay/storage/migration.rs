//! Crypto migration handler — bridges legacy Ed25519 identities to Dilithium3.
//!
//! This is a **one-time, per-user** operation invoked when an existing user (one
//! of the ~30 pre-PQ accounts) presents a `crypto_migration_v1` signed object.
//!
//! The migration object has these properties:
//! - **Outer signature:** Dilithium3 signature by the user's NEW key, verifying the
//!   whole canonical object. (Validates ownership of the new key.)
//! - **Inner sig (in payload):** Ed25519 signature by the user's OLD key, signing
//!   the message `new_dilithium_pubkey || b"\n" || timestamp_ascii_decimal`.
//!   (Validates that the old keyholder authorized the rotation.)
//!
//! Together these prove: same human controls both keys, voluntarily upgrading.
//! The legacy Ed25519 key is then archived as audit history; the new Dilithium3
//! key becomes the canonical identity. The existing `key_rotations` table is
//! also populated so legacy lookups via `resolve_current_key` continue to work.
//!
//! After all 30 users migrate (or after a deadline), this handler is dead code
//! and the `crypto_migration_v1` schema should be removed from the registry.

use ciborium::Value;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rusqlite::params;

use super::Storage;
use crate::relay::core::encoding::from_canonical_bytes;
use crate::relay::core::error::{Error, Result as CoreResult};
use crate::relay::core::object::Object;

/// Length of an Ed25519 public key in bytes.
const ED25519_PK_LEN: usize = 32;
/// Length of an Ed25519 signature in bytes.
const ED25519_SIG_LEN: usize = 64;

/// Outcome of a successful migration: the binding to record.
#[derive(Debug, Clone)]
pub struct MigrationOutcome {
    /// Hex-encoded 32-byte Ed25519 legacy public key (sealed audit value).
    pub legacy_pubkey_hex: String,
    /// Hex-encoded 1952-byte Dilithium3 new public key (the active identity going forward).
    pub new_pubkey_hex: String,
    /// Object id of the migration record.
    pub migration_object_id: String,
    /// Migration timestamp as declared by the client (informational).
    pub migration_timestamp: u64,
}

/// Validate the inner Ed25519 signature of a `crypto_migration_v1` object and
/// extract the binding info. Caller is responsible for the outer Dilithium3
/// signature (which is checked by `Object::verify_signature`).
///
/// Returns Err if:
/// - object_type != "crypto_migration_v1"
/// - payload is not canonical CBOR or missing required fields
/// - new_dilithium_pubkey does not match the object's author_public_key
/// - legacy_pubkey is not 32 bytes
/// - legacy_sig is not 64 bytes
/// - Ed25519 signature does not verify
pub fn validate_migration_object(object: &Object) -> CoreResult<MigrationOutcome> {
    if object.object_type != "crypto_migration_v1" {
        return Err(Error::InvalidField {
            field: "object_type".into(),
            reason: format!(
                "validate_migration_object called with type {}",
                object.object_type
            ),
        });
    }

    if object.payload_encoding != "cbor_canonical_v1" {
        return Err(Error::InvalidField {
            field: "payload_encoding".into(),
            reason: "migration payload must be plaintext canonical CBOR".into(),
        });
    }

    let payload_value = from_canonical_bytes(&object.payload)?;
    let map = match payload_value {
        Value::Map(entries) => entries,
        _ => {
            return Err(Error::InvalidField {
                field: "payload".into(),
                reason: "must be CBOR map".into(),
            });
        }
    };

    let mut legacy_pubkey: Option<Vec<u8>> = None;
    let mut new_dilithium_pubkey: Option<Vec<u8>> = None;
    let mut legacy_sig: Option<Vec<u8>> = None;
    let mut timestamp: Option<u64> = None;

    for (k, v) in map {
        let key = match k {
            Value::Text(s) => s,
            _ => continue,
        };
        match key.as_str() {
            "legacy_pubkey" => {
                if let Value::Bytes(b) = v {
                    legacy_pubkey = Some(b);
                }
            }
            "new_dilithium_pubkey" => {
                if let Value::Bytes(b) = v {
                    new_dilithium_pubkey = Some(b);
                }
            }
            "legacy_sig" => {
                if let Value::Bytes(b) = v {
                    legacy_sig = Some(b);
                }
            }
            "timestamp" => {
                if let Value::Integer(i) = v {
                    let raw: i128 = i.into();
                    if raw >= 0 {
                        timestamp = Some(raw as u64);
                    }
                }
            }
            _ => {}
        }
    }

    let legacy_pubkey = legacy_pubkey.ok_or_else(|| Error::MissingField("legacy_pubkey".into()))?;
    let new_pubkey = new_dilithium_pubkey
        .ok_or_else(|| Error::MissingField("new_dilithium_pubkey".into()))?;
    let legacy_sig =
        legacy_sig.ok_or_else(|| Error::MissingField("legacy_sig".into()))?;
    let timestamp =
        timestamp.ok_or_else(|| Error::MissingField("timestamp".into()))?;

    if legacy_pubkey.len() != ED25519_PK_LEN {
        return Err(Error::InvalidField {
            field: "legacy_pubkey".into(),
            reason: format!("must be {ED25519_PK_LEN} bytes (Ed25519)"),
        });
    }
    if legacy_sig.len() != ED25519_SIG_LEN {
        return Err(Error::InvalidField {
            field: "legacy_sig".into(),
            reason: format!("must be {ED25519_SIG_LEN} bytes (Ed25519)"),
        });
    }
    if new_pubkey != object.author_public_key {
        return Err(Error::InvalidField {
            field: "new_dilithium_pubkey".into(),
            reason: "must equal the object's author_public_key".into(),
        });
    }

    // Verify legacy Ed25519 signature over: new_pubkey || b"\n" || timestamp ASCII decimal
    let mut signed_message: Vec<u8> = Vec::with_capacity(new_pubkey.len() + 24);
    signed_message.extend_from_slice(&new_pubkey);
    signed_message.push(b'\n');
    signed_message.extend_from_slice(timestamp.to_string().as_bytes());

    let pk_arr: [u8; ED25519_PK_LEN] = legacy_pubkey
        .as_slice()
        .try_into()
        .map_err(|_| Error::InvalidPublicKey("ed25519 size".into()))?;
    let sig_arr: [u8; ED25519_SIG_LEN] = legacy_sig
        .as_slice()
        .try_into()
        .map_err(|_| Error::InvalidSignature)?;

    let verifying_key = VerifyingKey::from_bytes(&pk_arr)
        .map_err(|e| Error::InvalidPublicKey(e.to_string()))?;
    let signature = Signature::from_bytes(&sig_arr);

    verifying_key
        .verify(&signed_message, &signature)
        .map_err(|_| Error::SignatureVerificationFailed)?;

    let migration_object_id = object.object_id()?.to_hex();

    Ok(MigrationOutcome {
        legacy_pubkey_hex: hex::encode(legacy_pubkey),
        new_pubkey_hex: hex::encode(&object.author_public_key),
        migration_object_id,
        migration_timestamp: timestamp,
    })
}

impl Storage {
    /// Persist a migration outcome: writes both `legacy_ed25519_history` and
    /// the existing `key_rotations` (so legacy `resolve_current_key` returns
    /// the new Dilithium3 key when given the old Ed25519 key).
    ///
    /// Idempotent: if the legacy_pubkey already migrated, this is a no-op
    /// returning `Ok(false)`. Returns `Ok(true)` on a fresh migration.
    pub fn record_crypto_migration(
        &self,
        outcome: &MigrationOutcome,
    ) -> Result<bool, rusqlite::Error> {
        let archived_at = super::now_millis() as i64;

        self.with_conn(|conn| {
            // Check whether this legacy DID already migrated.
            let existing: Option<String> = conn
                .query_row(
                    "SELECT legacy_pubkey FROM legacy_ed25519_history
                     WHERE legacy_pubkey = ?1",
                    params![hex::decode(&outcome.legacy_pubkey_hex).unwrap_or_default()],
                    |row| {
                        let bytes: Vec<u8> = row.get(0)?;
                        Ok(hex::encode(bytes))
                    },
                )
                .ok();

            if existing.is_some() {
                return Ok(false);
            }

            // The DID column stores the new Dilithium3 pubkey hex as the canonical identity going forward.
            // (Phase 1 will replace this with a proper did:hum:<base58(fingerprint)> once the resolver lands.)
            conn.execute(
                "INSERT INTO legacy_ed25519_history (did, legacy_pubkey, migration_object_id, archived_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    outcome.new_pubkey_hex,
                    hex::decode(&outcome.legacy_pubkey_hex).unwrap_or_default(),
                    outcome.migration_object_id,
                    archived_at,
                ],
            )?;

            // Also write a key_rotations entry so existing `resolve_current_key` works.
            // sig fields are placeholders pointing to the migration object — the actual
            // proofs live in the signed_objects table at migration_object_id.
            conn.execute(
                "INSERT OR REPLACE INTO key_rotations
                 (old_key, new_key, sig_by_old, sig_by_new, rotated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    outcome.legacy_pubkey_hex,
                    outcome.new_pubkey_hex,
                    format!("see signed_objects/{}", outcome.migration_object_id),
                    format!("see signed_objects/{}", outcome.migration_object_id),
                    archived_at,
                ],
            )?;

            Ok(true)
        })
    }

    /// Look up a migration by legacy Ed25519 public key (hex).
    /// Returns the new Dilithium3 public key (hex) and migration object id.
    pub fn lookup_migration_by_legacy_key(
        &self,
        legacy_pubkey_hex: &str,
    ) -> Result<Option<(String, String)>, rusqlite::Error> {
        let legacy_bytes = match hex::decode(legacy_pubkey_hex) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };

        self.with_conn(|conn| {
            conn.query_row(
                "SELECT did, migration_object_id FROM legacy_ed25519_history
                 WHERE legacy_pubkey = ?1",
                params![legacy_bytes],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .ok()
            .map(Some)
            .map(Ok)
            .unwrap_or(Ok(None))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::encoding::{cbor_bytes, cbor_int, cbor_map, cbor_text};
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;
    use ed25519_dalek::{Signer, SigningKey};
    use rand_core_06::OsRng;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_mig_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn build_migration_object(
        legacy_signing_key: &SigningKey,
        new_dilithium_kp: &DilithiumKeypair,
        timestamp: u64,
    ) -> Object {
        // Sign the message: new_dilithium_pubkey || b"\n" || timestamp_ascii
        let new_pubkey_bytes = new_dilithium_kp.public_key();
        let mut signed_msg = Vec::with_capacity(new_pubkey_bytes.len() + 32);
        signed_msg.extend_from_slice(&new_pubkey_bytes);
        signed_msg.push(b'\n');
        signed_msg.extend_from_slice(timestamp.to_string().as_bytes());

        let legacy_sig = legacy_signing_key.sign(&signed_msg);
        let legacy_pubkey = legacy_signing_key.verifying_key().to_bytes();

        let payload = cbor_map(vec![
            ("legacy_pubkey", cbor_bytes(&legacy_pubkey)),
            ("legacy_sig", cbor_bytes(&legacy_sig.to_bytes())),
            ("new_dilithium_pubkey", cbor_bytes(&new_pubkey_bytes)),
            ("timestamp", cbor_int(timestamp)),
        ]);

        ObjectBuilder::new("crypto_migration_v1")
            .created_at(timestamp)
            .payload_cbor(&payload)
            .unwrap()
            .sign(new_dilithium_kp)
            .unwrap()
    }

    #[test]
    fn validate_well_formed_migration() {
        let legacy = SigningKey::generate(&mut OsRng);
        let new_kp = DilithiumKeypair::generate().unwrap();
        let obj = build_migration_object(&legacy, &new_kp, 1_700_000_000);

        let outcome = validate_migration_object(&obj).expect("valid migration");
        assert_eq!(
            outcome.legacy_pubkey_hex,
            hex::encode(legacy.verifying_key().to_bytes())
        );
        assert_eq!(outcome.new_pubkey_hex, hex::encode(new_kp.public_key()));
        assert_eq!(outcome.migration_timestamp, 1_700_000_000);
    }

    #[test]
    fn rejects_wrong_object_type() {
        let new_kp = DilithiumKeypair::generate().unwrap();
        let obj = ObjectBuilder::new("not_a_migration")
            .payload_cbor(&cbor_map(vec![("x", cbor_text("y"))]))
            .unwrap()
            .sign(&new_kp)
            .unwrap();
        assert!(validate_migration_object(&obj).is_err());
    }

    #[test]
    fn rejects_mismatched_pubkeys() {
        let legacy = SigningKey::generate(&mut OsRng);
        let new_kp = DilithiumKeypair::generate().unwrap();
        let other_kp = DilithiumKeypair::generate().unwrap();

        // Build with `other_kp` declared in payload but signed by `new_kp` — mismatch.
        let new_pubkey_bytes = other_kp.public_key();
        let timestamp = 100u64;

        let mut signed_msg = new_pubkey_bytes.clone();
        signed_msg.push(b'\n');
        signed_msg.extend_from_slice(timestamp.to_string().as_bytes());
        let legacy_sig = legacy.sign(&signed_msg);

        let payload = cbor_map(vec![
            ("legacy_pubkey", cbor_bytes(&legacy.verifying_key().to_bytes())),
            ("legacy_sig", cbor_bytes(&legacy_sig.to_bytes())),
            ("new_dilithium_pubkey", cbor_bytes(&new_pubkey_bytes)),
            ("timestamp", cbor_int(timestamp)),
        ]);

        let obj = ObjectBuilder::new("crypto_migration_v1")
            .created_at(timestamp)
            .payload_cbor(&payload)
            .unwrap()
            .sign(&new_kp) // signed by new_kp but payload says other_kp
            .unwrap();

        assert!(validate_migration_object(&obj).is_err());
    }

    #[test]
    fn rejects_bad_legacy_signature() {
        let legacy = SigningKey::generate(&mut OsRng);
        let attacker = SigningKey::generate(&mut OsRng);
        let new_kp = DilithiumKeypair::generate().unwrap();
        let timestamp = 100u64;

        // Sign with attacker's key but claim it's the legacy key
        let new_pubkey_bytes = new_kp.public_key();
        let mut signed_msg = new_pubkey_bytes.clone();
        signed_msg.push(b'\n');
        signed_msg.extend_from_slice(timestamp.to_string().as_bytes());
        let attacker_sig = attacker.sign(&signed_msg);

        let payload = cbor_map(vec![
            ("legacy_pubkey", cbor_bytes(&legacy.verifying_key().to_bytes())),
            ("legacy_sig", cbor_bytes(&attacker_sig.to_bytes())),
            ("new_dilithium_pubkey", cbor_bytes(&new_pubkey_bytes)),
            ("timestamp", cbor_int(timestamp)),
        ]);

        let obj = ObjectBuilder::new("crypto_migration_v1")
            .created_at(timestamp)
            .payload_cbor(&payload)
            .unwrap()
            .sign(&new_kp)
            .unwrap();

        assert!(validate_migration_object(&obj).is_err());
    }

    #[test]
    fn record_and_lookup_migration() {
        let db = make_test_storage();
        let legacy = SigningKey::generate(&mut OsRng);
        let new_kp = DilithiumKeypair::generate().unwrap();
        let obj = build_migration_object(&legacy, &new_kp, 200);

        // Persist the object so its id resolves to a row
        db.put_signed_object(&obj, None).unwrap();

        let outcome = validate_migration_object(&obj).unwrap();
        let recorded = db.record_crypto_migration(&outcome).unwrap();
        assert!(recorded);

        // Idempotent
        let recorded_again = db.record_crypto_migration(&outcome).unwrap();
        assert!(!recorded_again);

        // Lookup by legacy key
        let found = db
            .lookup_migration_by_legacy_key(&outcome.legacy_pubkey_hex)
            .unwrap()
            .expect("should find migration");
        assert_eq!(found.0, outcome.new_pubkey_hex);
        assert_eq!(found.1, outcome.migration_object_id);

        // resolve_current_key (existing chain walker) should follow the rotation
        let current = db.resolve_current_key(&outcome.legacy_pubkey_hex);
        assert_eq!(current, outcome.new_pubkey_hex);
    }
}

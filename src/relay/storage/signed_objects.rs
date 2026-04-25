//! Signed object substrate (Phase 0).
//!
//! Generic SQLite-backed store for any post-quantum-signed canonical-CBOR object.
//! The object format is defined in `docs/network/object_format.md` and implemented
//! in `crate::relay::core::object`.
//!
//! Higher-level domains (signed_profiles, vouches, VCs, votes, governance proposals,
//! recovery shares, etc.) are projections of this table — they keep their own
//! fast-read tables while the underlying authority is the `signed_objects` row.
//!
//! Each row is fully self-validating: the canonical bytes can be recomputed from
//! the fields, the BLAKE3 of those bytes equals `object_id`, and the Dilithium3
//! signature verifies against `author_pubkey`.

use rusqlite::{OptionalExtension, params};

use super::Storage;
use crate::relay::core::error::{Error, Result as CoreResult};
use crate::relay::core::object::Object;
use crate::relay::core::pq_crypto::DILITHIUM_PK_LEN;

/// A row from the `signed_objects` table. Mirrors the in-memory `Object`
/// 1:1 plus storage-only fields (`source_server`, `received_at`).
#[derive(Debug, Clone)]
pub struct SignedObjectRecord {
    pub object_id: String,
    pub protocol_version: u64,
    pub object_type: String,
    pub space_id: Option<String>,
    pub channel_id: Option<String>,
    /// 16-byte BLAKE3 fingerprint of `author_pubkey`, hex-encoded — fast index lookup.
    pub author_fp: String,
    pub author_pubkey: Vec<u8>,
    pub created_at: Option<u64>,
    pub payload_schema_version: u64,
    pub payload_encoding: String,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
    /// JSON array of object_id strings referenced by this object.
    pub references_json: String,
    /// Which peer server gossiped this to us (None = locally submitted).
    pub source_server: Option<String>,
    /// When this server received the object (NOT trusted for ordering).
    pub received_at: i64,
}

/// Compute the 32-character hex fingerprint of a Dilithium3 public key.
///
/// = first 16 bytes of BLAKE3(pubkey), hex-encoded = 32 chars.
/// Used for indexed lookups; collision-resistant for this purpose.
pub fn author_fingerprint(pubkey: &[u8]) -> String {
    let h = blake3::hash(pubkey);
    let bytes = h.as_bytes();
    bytes[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// Compute the hex-encoded `object_id` (64-char BLAKE3 hex) for an `Object`.
pub fn compute_object_id(object: &Object) -> CoreResult<String> {
    Ok(object.object_id()?.to_hex())
}

impl Storage {
    /// Insert a signed object after verifying signature, schema basics, and well-formedness.
    /// Returns `Ok(true)` if the row was newly inserted, `Ok(false)` if it already existed
    /// (duplicate by `object_id`).
    pub fn put_signed_object(
        &self,
        object: &Object,
        source_server: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        // Validate signature first — never store an unverifiable object.
        object.verify_signature().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SignedObjectError(format!(
                "signature verification failed: {e}"
            ))))
        })?;

        // Validate pubkey length.
        if object.author_public_key.len() != DILITHIUM_PK_LEN {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                SignedObjectError(format!(
                    "author_pubkey must be {DILITHIUM_PK_LEN} bytes"
                )),
            )));
        }

        let object_id = object.object_id().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SignedObjectError(format!(
                "object_id computation failed: {e}"
            ))))
        })?;
        let object_id_hex = object_id.to_hex();

        let author_fp = author_fingerprint(&object.author_public_key);
        let references_json = serde_json::to_string(&object.references)
            .unwrap_or_else(|_| "[]".to_string());
        let received_at = current_unix_millis();

        let inserted = self.with_conn(|conn| {
            // Idempotent insert; if already present, this is a no-op.
            let rows = conn.execute(
                "INSERT OR IGNORE INTO signed_objects (
                    object_id, protocol_version, object_type, space_id, channel_id,
                    author_fp, author_pubkey, created_at,
                    payload_schema_version, payload_encoding, payload, signature,
                    references_json, source_server, received_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    object_id_hex,
                    object.protocol_version as i64,
                    object.object_type,
                    object.space_id,
                    object.channel_id,
                    author_fp,
                    object.author_public_key,
                    object.created_at.map(|t| t as i64),
                    object.payload_schema_version as i64,
                    object.payload_encoding,
                    object.payload,
                    object.signature,
                    references_json,
                    source_server,
                    received_at,
                ],
            )?;
            Ok::<bool, rusqlite::Error>(rows > 0)
        })?;

        // Side-effects only on a fresh insert (not duplicate).
        if inserted {
            // Auto-index VCs.
            let _ = self.index_credential(object);
            // Auto-index governance proposals + votes.
            let _ = self.index_proposal(object);
            let _ = self.index_vote(object);
            // Revocations: only the issuer of the target VC may revoke it.
            if object.object_type == "revocation_v1" {
                if let Some(target_id) = first_reference(object) {
                    let author_did =
                        crate::relay::core::did::did_for_pubkey(&object.author_public_key);
                    if let Ok(Some(vc)) = self.get_credential(&target_id) {
                        if vc.issuer_did == author_did {
                            let _ = self.revoke_credential(&target_id, &object_id_hex);
                        }
                    }
                }
            }
            // Withdrawals: only the subject of the target VC may withdraw it.
            if object.object_type == "withdrawal_v1" {
                if let Some(target_id) = first_reference(object) {
                    let author_did =
                        crate::relay::core::did::did_for_pubkey(&object.author_public_key);
                    if let Ok(Some(vc)) = self.get_credential(&target_id) {
                        if vc.subject_did == author_did {
                            let _ = self.withdraw_credential(&target_id);
                        }
                    }
                }
            }
        }

        Ok(inserted)
    }

    /// Fetch a signed object by its hex `object_id`.
    pub fn get_signed_object(&self, object_id: &str) -> Result<Option<SignedObjectRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT object_id, protocol_version, object_type, space_id, channel_id,
                        author_fp, author_pubkey, created_at, payload_schema_version,
                        payload_encoding, payload, signature, references_json,
                        source_server, received_at
                 FROM signed_objects WHERE object_id = ?1",
                params![object_id],
                row_to_record,
            )
            .optional()
        })
    }

    /// List signed objects matching the given filters.
    ///
    /// Each filter is optional:
    /// - `object_type`: exact match
    /// - `space_id`: exact match (use empty string for global / no space)
    /// - `author_fp`: short fingerprint (use [`author_fingerprint`] to compute)
    /// - `since_received`: only return rows with `received_at > since_received`
    /// - `limit`: max rows (default 100, cap 1000)
    pub fn list_signed_objects(
        &self,
        object_type: Option<&str>,
        space_id: Option<&str>,
        author_fp: Option<&str>,
        since_received: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SignedObjectRecord>, rusqlite::Error> {
        let limit = limit.unwrap_or(100).min(1000);

        // Build dynamic WHERE clause without string concat of user input — bind every value.
        let mut sql = String::from(
            "SELECT object_id, protocol_version, object_type, space_id, channel_id,
                    author_fp, author_pubkey, created_at, payload_schema_version,
                    payload_encoding, payload, signature, references_json,
                    source_server, received_at
             FROM signed_objects WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(t) = object_type {
            sql.push_str(" AND object_type = ?");
            params.push(Box::new(t.to_string()));
        }
        if let Some(s) = space_id {
            sql.push_str(" AND space_id = ?");
            params.push(Box::new(s.to_string()));
        }
        if let Some(fp) = author_fp {
            sql.push_str(" AND author_fp = ?");
            params.push(Box::new(fp.to_string()));
        }
        if let Some(since) = since_received {
            sql.push_str(" AND received_at > ?");
            params.push(Box::new(since));
        }
        sql.push_str(" ORDER BY received_at DESC LIMIT ?");
        params.push(Box::new(limit as i64));

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|b| b.as_ref()).collect();
            let rows = stmt
                .query_map(rusqlite::params_from_iter(param_refs), row_to_record)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Count signed objects of a given type (or all if `object_type` is None).
    pub fn count_signed_objects(&self, object_type: Option<&str>) -> Result<u64, rusqlite::Error> {
        self.with_conn(|conn| {
            let count: i64 = if let Some(t) = object_type {
                conn.query_row(
                    "SELECT COUNT(*) FROM signed_objects WHERE object_type = ?1",
                    params![t],
                    |row| row.get(0),
                )?
            } else {
                conn.query_row("SELECT COUNT(*) FROM signed_objects", params![], |row| {
                    row.get(0)
                })?
            };
            Ok(count as u64)
        })
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SignedObjectRecord> {
    let created_at_i: Option<i64> = row.get(7)?;
    let proto_v: i64 = row.get(1)?;
    let schema_v: i64 = row.get(8)?;
    Ok(SignedObjectRecord {
        object_id: row.get(0)?,
        protocol_version: proto_v as u64,
        object_type: row.get(2)?,
        space_id: row.get(3)?,
        channel_id: row.get(4)?,
        author_fp: row.get(5)?,
        author_pubkey: row.get(6)?,
        created_at: created_at_i.map(|t| t as u64),
        payload_schema_version: schema_v as u64,
        payload_encoding: row.get(9)?,
        payload: row.get(10)?,
        signature: row.get(11)?,
        references_json: row.get(12)?,
        source_server: row.get(13)?,
        received_at: row.get(14)?,
    })
}

fn current_unix_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Return the first object_id in this object's `references` array, if any.
/// Used by revocation/withdrawal flows to find the VC being targeted.
fn first_reference(object: &Object) -> Option<String> {
    object.references.first().cloned()
}

/// Wrapper error to thread our crypto/encoding errors through rusqlite's Error::ToSqlConversionFailure.
#[derive(Debug)]
struct SignedObjectError(String);

impl std::fmt::Display for SignedObjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "signed_object: {}", self.0)
    }
}

impl std::error::Error for SignedObjectError {}

// Convert a SignedObjectRecord back to an in-memory Object for re-verification or replay.
impl SignedObjectRecord {
    /// Reconstruct an `Object` from this record so the caller can re-verify the signature
    /// or re-compute the canonical bytes.
    pub fn to_object(&self) -> Object {
        let references: Vec<String> =
            serde_json::from_str(&self.references_json).unwrap_or_default();
        Object {
            protocol_version: self.protocol_version,
            object_type: self.object_type.clone(),
            space_id: self.space_id.clone(),
            channel_id: self.channel_id.clone(),
            author_public_key: self.author_pubkey.clone(),
            created_at: self.created_at,
            references,
            payload_schema_version: self.payload_schema_version,
            payload_encoding: self.payload_encoding.clone(),
            payload: self.payload.clone(),
            signature: self.signature.clone(),
        }
    }
}

// Suppress unused-import warning for Error if no caller of this module uses it directly.
#[allow(dead_code)]
fn _import_anchor(e: Error) -> Error {
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;
    use crate::relay::core::encoding::{cbor_map, cbor_text};
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Create a temp-file backed Storage for tests. We can't use Connection::open_in_memory
    /// directly because `Storage::open` requires `&Path` and runs the migration batch.
    fn make_test_storage() -> Storage {
        // Use a unique temp file per test to avoid cross-test interference.
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn make_signed_object(kp: &DilithiumKeypair, object_type: &str, content: &str) -> Object {
        let payload = cbor_map(vec![("content", cbor_text(content))]);
        ObjectBuilder::new(object_type)
            .space_id("test_space")
            .created_at(1000)
            .payload_cbor(&payload)
            .unwrap()
            .sign(kp)
            .unwrap()
    }

    #[test]
    fn put_and_get_round_trip() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let obj = make_signed_object(&kp, "post", "hello");

        let id = obj.object_id().unwrap().to_hex();

        let stored = db.put_signed_object(&obj, None).unwrap();
        assert!(stored, "first put should insert");

        let fetched = db.get_signed_object(&id).unwrap().expect("found");
        assert_eq!(fetched.object_id, id);
        assert_eq!(fetched.object_type, "post");
        assert_eq!(fetched.author_pubkey, kp.public_key());

        // Re-verify signature from stored bytes
        fetched.to_object().verify_signature().expect("re-verify after roundtrip");
    }

    #[test]
    fn put_is_idempotent() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let obj = make_signed_object(&kp, "post", "duplicate me");

        let first = db.put_signed_object(&obj, None).unwrap();
        let second = db.put_signed_object(&obj, None).unwrap();

        assert!(first);
        assert!(!second, "second put with same content should be no-op");
    }

    #[test]
    fn put_rejects_tampered_object() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let mut obj = make_signed_object(&kp, "post", "real");
        // Flip a payload byte; signature now invalid.
        obj.payload[0] ^= 0xFF;

        let result = db.put_signed_object(&obj, None);
        assert!(result.is_err(), "tampered object must be rejected");
    }

    #[test]
    fn list_filters_by_type() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();

        // Distinct content per object — same content + same key = same object_id
        // (deterministic Dilithium signing), which would dedupe via INSERT OR IGNORE.
        for (kind, body) in &[("post", "first"), ("post", "second"), ("comment", "alpha")] {
            let obj = make_signed_object(&kp, kind, body);
            db.put_signed_object(&obj, None).unwrap();
        }

        let posts = db.list_signed_objects(Some("post"), None, None, None, None).unwrap();
        assert_eq!(posts.len(), 2);

        let comments = db.list_signed_objects(Some("comment"), None, None, None, None).unwrap();
        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn list_filters_by_author_fp() {
        let db = make_test_storage();
        let alice = DilithiumKeypair::generate().unwrap();
        let bob = DilithiumKeypair::generate().unwrap();

        db.put_signed_object(&make_signed_object(&alice, "post", "A"), None).unwrap();
        db.put_signed_object(&make_signed_object(&alice, "post", "AA"), None).unwrap();
        db.put_signed_object(&make_signed_object(&bob, "post", "B"), None).unwrap();

        let alice_fp = author_fingerprint(&alice.public_key());
        let alice_posts = db.list_signed_objects(None, None, Some(&alice_fp), None, None).unwrap();
        assert_eq!(alice_posts.len(), 2);
    }

    #[test]
    fn count_signed_objects_matches_inserts() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();

        assert_eq!(db.count_signed_objects(None).unwrap(), 0);

        for i in 0..5 {
            let obj = make_signed_object(&kp, "vote_v1", &format!("vote-{i}"));
            db.put_signed_object(&obj, None).unwrap();
        }

        assert_eq!(db.count_signed_objects(None).unwrap(), 5);
        assert_eq!(db.count_signed_objects(Some("vote_v1")).unwrap(), 5);
        assert_eq!(db.count_signed_objects(Some("nonexistent")).unwrap(), 0);
    }

    #[test]
    fn record_to_object_round_trips_canonical_bytes() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let original = make_signed_object(&kp, "thread_create", "X");

        let original_canonical = original.to_canonical_bytes().unwrap();
        let original_id = original.object_id().unwrap().to_hex();

        db.put_signed_object(&original, None).unwrap();
        let fetched = db.get_signed_object(&original_id).unwrap().unwrap();
        let recovered = fetched.to_object();

        let recovered_canonical = recovered.to_canonical_bytes().unwrap();
        assert_eq!(original_canonical, recovered_canonical);
        assert_eq!(recovered.object_id().unwrap().to_hex(), original_id);
    }

    // Hold a static lock across tests to avoid rusqlite WAL file contention on Windows
    // when the same temp filename pattern collides. Combined with unique timestamps in
    // make_test_storage this should never actually contend, but the lock is cheap.
    #[allow(dead_code)]
    static _SERIALIZE: Mutex<()> = Mutex::new(());
    #[allow(dead_code)]
    fn _path_unused() -> PathBuf { PathBuf::new() }
}

//! Social key recovery — server-side storage of opaque Shamir shares (Phase 4 PR 1).
//!
//! Per the strategic plan (decision 3): freely-chosen guardians with VC-attested
//! badge as a soft signal. The server stores ciphertext only — like vault_sync,
//! it has no ability to decrypt or reconstruct keys.
//!
//! Object types this module indexes:
//!   recovery_share_v1   — encrypted share to a guardian (one per guardian)
//!   recovery_request_v1 — request to use stored shares to recover a lost key
//!
//! The actual Shamir secret sharing (split + combine) happens client-side. The
//! server's only role is to:
//!   1. Persist shares opaquely
//!   2. Track recovery requests + which guardians have submitted
//!   3. When threshold is reached, accept the new key as a successor (key_rotation)
//!
//! Phase 4 PR 1 ships the storage + indexing. Client-side Shamir lib + relay
//! endpoints to coordinate the recovery flow are Phase 4 PR 2 (later).

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Storage;
use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::object::Object;

/// Read a CBOR text payload field.
fn read_text(object: &Object, field: &str) -> Option<String> {
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    if let ciborium::Value::Map(entries) = value {
        for (k, v) in entries {
            if let ciborium::Value::Text(name) = k {
                if name == field {
                    if let ciborium::Value::Text(s) = v {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

/// Read a CBOR bytes payload field.
fn read_bytes(object: &Object, field: &str) -> Option<Vec<u8>> {
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    if let ciborium::Value::Map(entries) = value {
        for (k, v) in entries {
            if let ciborium::Value::Text(name) = k {
                if name == field {
                    if let ciborium::Value::Bytes(b) = v {
                        return Some(b);
                    }
                }
            }
        }
    }
    None
}

/// Read a CBOR integer payload field.
fn read_int(object: &Object, field: &str) -> Option<i64> {
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    if let ciborium::Value::Map(entries) = value {
        for (k, v) in entries {
            if let ciborium::Value::Text(name) = k {
                if name == field {
                    if let ciborium::Value::Integer(i) = v {
                        let raw: i128 = i.into();
                        if raw <= i64::MAX as i128 && raw >= i64::MIN as i128 {
                            return Some(raw as i64);
                        }
                    }
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryShareIndex {
    pub share_object_id: String,
    pub holder_did: String,
    pub guardian_did: String,
    pub threshold: u32,
    pub total_shares: u32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoverySetup {
    pub holder_did: String,
    pub guardians: Vec<String>,
    pub threshold: u32,
    pub total_shares: u32,
}

/// A recovery request — holder lost their key and is asking guardians for shares.
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryRequestRecord {
    pub request_object_id: String,
    pub holder_did: String,
    pub new_pubkey: Vec<u8>,
    pub threshold_required: u32,
    pub approvals_count: u32,
    pub status: String, // "open" | "ready" | "completed" | "expired"
    pub created_at: i64,
}

/// One guardian's approval: confirms they're sending their decrypted share to
/// the holder via an out-of-band channel (typically encrypted DM to new_pubkey).
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryApprovalRecord {
    pub approval_object_id: String,
    pub request_object_id: String,
    pub guardian_did: String,
    pub submitted_at: i64,
}

impl Storage {
    /// Index a `recovery_share_v1` object stored on this server. Idempotent.
    /// Returns Ok(true) if a new share row was inserted.
    pub fn index_recovery_share(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "recovery_share_v1" {
            return Ok(false);
        }
        let holder_did = did_for_pubkey(&object.author_public_key);
        let guardian_did = match read_text(object, "guardian_did") {
            Some(d) => d,
            None => return Ok(false),
        };
        let threshold = read_int(object, "threshold").unwrap_or(0);
        let total_shares = read_int(object, "total_shares").unwrap_or(0);
        if threshold <= 0 || total_shares <= 0 || threshold as u32 > total_shares as u32 {
            return Ok(false);
        }

        let share_object_id = object
            .object_id()
            .map(|h| h.to_hex())
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )))?;
        let now = super::now_millis() as i64;

        self.with_conn(|conn| {
            let rows = conn.execute(
                "INSERT OR IGNORE INTO recovery_shares
                    (share_object_id, holder_did, guardian_did, threshold,
                     total_shares, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    share_object_id,
                    holder_did,
                    guardian_did,
                    threshold as i64,
                    total_shares as i64,
                    now,
                ],
            )?;
            Ok(rows > 0)
        })
    }

    /// Get the current recovery setup for a holder DID — guardians + threshold.
    pub fn get_recovery_setup(&self, holder_did: &str) -> Result<Option<RecoverySetup>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT guardian_did, threshold, total_shares
                 FROM recovery_shares WHERE holder_did = ?1
                 ORDER BY created_at ASC",
            )?;
            let rows: Vec<(String, i64, i64)> = stmt
                .query_map(params![holder_did], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            if rows.is_empty() {
                return Ok(None);
            }
            let threshold = rows[0].1 as u32;
            let total_shares = rows[0].2 as u32;
            let guardians: Vec<String> = rows.into_iter().map(|(g, _, _)| g).collect();
            Ok(Some(RecoverySetup {
                holder_did: holder_did.to_string(),
                guardians,
                threshold,
                total_shares,
            }))
        })
    }

    /// List all share entries for a guardian DID (so they know what they hold).
    pub fn list_shares_held_by(&self, guardian_did: &str) -> Result<Vec<RecoveryShareIndex>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT share_object_id, holder_did, guardian_did, threshold,
                        total_shares, created_at
                 FROM recovery_shares WHERE guardian_did = ?1
                 ORDER BY created_at DESC",
            )?;
            let rows = stmt
                .query_map(params![guardian_did], |row| {
                    Ok(RecoveryShareIndex {
                        share_object_id: row.get(0)?,
                        holder_did: row.get(1)?,
                        guardian_did: row.get(2)?,
                        threshold: row.get::<_, i64>(3)? as u32,
                        total_shares: row.get::<_, i64>(4)? as u32,
                        created_at: row.get(5)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Index a `recovery_request_v1` object: holder begins recovery flow.
    /// The request author MUST be the holder (using their NEW key).
    /// Returns Ok(true) if a new request was inserted.
    pub fn index_recovery_request(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "recovery_request_v1" {
            return Ok(false);
        }
        let holder_did = match read_text(object, "holder_did") {
            Some(d) => d,
            None => return Ok(false),
        };
        let new_pubkey = match read_bytes(object, "new_pubkey") {
            Some(b) => b,
            None => return Ok(false),
        };

        // Threshold required: look up the holder's recovery setup
        let setup = match self.get_recovery_setup(&holder_did)? {
            Some(s) => s,
            None => return Ok(false), // No recovery configured for this DID
        };

        let request_object_id = object
            .object_id()
            .map(|h| h.to_hex())
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )))?;
        let now = super::now_millis() as i64;

        self.with_conn(|conn| {
            let rows = conn.execute(
                "INSERT OR IGNORE INTO recovery_requests
                    (request_object_id, holder_did, new_pubkey, threshold_required,
                     approvals_count, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, 0, 'open', ?5)",
                params![
                    request_object_id,
                    holder_did,
                    new_pubkey,
                    setup.threshold as i64,
                    now,
                ],
            )?;
            Ok(rows > 0)
        })
    }

    /// Index a `recovery_approval_v1` object: a guardian acknowledges the request
    /// and (out of band) ships their share to the holder. Increments the
    /// approvals_count on the referenced request; if the threshold is met, marks
    /// the request as "ready".
    pub fn index_recovery_approval(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "recovery_approval_v1" {
            return Ok(false);
        }
        let request_object_id = match object.references.first() {
            Some(id) => id.clone(),
            None => return Ok(false),
        };
        let guardian_did = crate::relay::core::did::did_for_pubkey(&object.author_public_key);

        // Authorization: the guardian must actually hold a share for this request's holder.
        let request = match self.get_recovery_request(&request_object_id)? {
            Some(r) => r,
            None => return Ok(false),
        };
        let setup = match self.get_recovery_setup(&request.holder_did)? {
            Some(s) => s,
            None => return Ok(false),
        };
        if !setup.guardians.iter().any(|g| g == &guardian_did) {
            // Not an actual guardian — silently ignored
            return Ok(false);
        }

        let approval_object_id = object
            .object_id()
            .map(|h| h.to_hex())
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )))?;
        let now = super::now_millis() as i64;

        self.with_conn(|conn| {
            // Insert approval (UNIQUE on request+guardian prevents duplicate counting)
            let rows = conn.execute(
                "INSERT OR IGNORE INTO recovery_approvals
                    (approval_object_id, request_object_id, guardian_did, submitted_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![approval_object_id, request_object_id, guardian_did, now],
            )?;

            if rows > 0 {
                // Bump approvals_count on the request and check threshold
                let new_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM recovery_approvals WHERE request_object_id = ?1",
                    params![request_object_id],
                    |r| r.get(0),
                )?;
                let new_status = if new_count as u32 >= request.threshold_required {
                    "ready"
                } else {
                    "open"
                };
                conn.execute(
                    "UPDATE recovery_requests
                     SET approvals_count = ?1, status = ?2
                     WHERE request_object_id = ?3",
                    params![new_count, new_status, request_object_id],
                )?;
            }

            Ok(rows > 0)
        })
    }

    /// Get a recovery request by object id.
    pub fn get_recovery_request(
        &self,
        request_object_id: &str,
    ) -> Result<Option<RecoveryRequestRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT request_object_id, holder_did, new_pubkey, threshold_required,
                        approvals_count, status, created_at
                 FROM recovery_requests WHERE request_object_id = ?1",
                params![request_object_id],
                |row| {
                    Ok(RecoveryRequestRecord {
                        request_object_id: row.get(0)?,
                        holder_did: row.get(1)?,
                        new_pubkey: row.get(2)?,
                        threshold_required: row.get::<_, i64>(3)? as u32,
                        approvals_count: row.get::<_, i64>(4)? as u32,
                        status: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()
        })
    }

    /// List all approvals for a recovery request.
    pub fn list_recovery_approvals(
        &self,
        request_object_id: &str,
    ) -> Result<Vec<RecoveryApprovalRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT approval_object_id, request_object_id, guardian_did, submitted_at
                 FROM recovery_approvals WHERE request_object_id = ?1
                 ORDER BY submitted_at ASC",
            )?;
            let rows = stmt
                .query_map(params![request_object_id], |row| {
                    Ok(RecoveryApprovalRecord {
                        approval_object_id: row.get(0)?,
                        request_object_id: row.get(1)?,
                        guardian_did: row.get(2)?,
                        submitted_at: row.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Look up a single share record by object id.
    pub fn get_recovery_share(&self, share_object_id: &str) -> Result<Option<RecoveryShareIndex>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT share_object_id, holder_did, guardian_did, threshold,
                        total_shares, created_at
                 FROM recovery_shares WHERE share_object_id = ?1",
                params![share_object_id],
                |row| {
                    Ok(RecoveryShareIndex {
                        share_object_id: row.get(0)?,
                        holder_did: row.get(1)?,
                        guardian_did: row.get(2)?,
                        threshold: row.get::<_, i64>(3)? as u32,
                        total_shares: row.get::<_, i64>(4)? as u32,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::encoding::{cbor_bytes, cbor_int, cbor_map, cbor_text};
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_recovery_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn make_share(
        holder: &DilithiumKeypair,
        guardian_did: &str,
        threshold: u32,
        total: u32,
        share_payload: &[u8],
    ) -> Object {
        let payload = cbor_map(vec![
            ("guardian_did", cbor_text(guardian_did)),
            ("threshold", cbor_int(threshold as u64)),
            ("total_shares", cbor_int(total as u64)),
            ("ciphertext", cbor_bytes(share_payload)),
        ]);
        ObjectBuilder::new("recovery_share_v1")
            .created_at(super::super::now_millis())
            .payload_cbor(&payload)
            .unwrap()
            .sign(holder)
            .unwrap()
    }

    #[test]
    fn shares_index_after_storage() {
        let db = make_test_storage();
        let holder = DilithiumKeypair::generate().unwrap();
        let holder_did = did_for_pubkey(&holder.public_key());

        let g1 = DilithiumKeypair::generate().unwrap();
        let g2 = DilithiumKeypair::generate().unwrap();
        let g3 = DilithiumKeypair::generate().unwrap();
        let g1_did = did_for_pubkey(&g1.public_key());
        let g2_did = did_for_pubkey(&g2.public_key());
        let g3_did = did_for_pubkey(&g3.public_key());

        for (i, g_did) in [&g1_did, &g2_did, &g3_did].iter().enumerate() {
            let share_bytes = vec![i as u8; 64]; // dummy ciphertext
            let s = make_share(&holder, g_did, 2, 3, &share_bytes);
            db.put_signed_object(&s, None).unwrap();
        }

        let setup = db.get_recovery_setup(&holder_did).unwrap().expect("found");
        assert_eq!(setup.guardians.len(), 3);
        assert_eq!(setup.threshold, 2);
        assert_eq!(setup.total_shares, 3);
    }

    #[test]
    fn guardian_can_list_their_shares() {
        let db = make_test_storage();
        let holder1 = DilithiumKeypair::generate().unwrap();
        let holder2 = DilithiumKeypair::generate().unwrap();
        let guardian = DilithiumKeypair::generate().unwrap();
        let g_did = did_for_pubkey(&guardian.public_key());

        // Same guardian holds shares for two different holders
        let s1 = make_share(&holder1, &g_did, 2, 3, &[1u8; 32]);
        let s2 = make_share(&holder2, &g_did, 3, 5, &[2u8; 32]);
        db.put_signed_object(&s1, None).unwrap();
        db.put_signed_object(&s2, None).unwrap();

        let shares = db.list_shares_held_by(&g_did).unwrap();
        assert_eq!(shares.len(), 2);
    }

    #[test]
    fn malformed_share_is_not_indexed() {
        let db = make_test_storage();
        let holder = DilithiumKeypair::generate().unwrap();

        // Threshold > total — invalid
        let s = make_share(&holder, "did:hum:fake", 5, 3, &[0u8; 32]);
        db.put_signed_object(&s, None).unwrap();

        let setup = db
            .get_recovery_setup(&did_for_pubkey(&holder.public_key()))
            .unwrap();
        assert!(setup.is_none(), "invalid share must not index");
    }

    #[test]
    fn no_recovery_setup_returns_none() {
        let db = make_test_storage();
        let holder = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&holder.public_key());
        assert!(db.get_recovery_setup(&did).unwrap().is_none());
    }
}

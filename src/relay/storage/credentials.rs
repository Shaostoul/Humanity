//! Verifiable Credentials index (Phase 1 PR 2).
//!
//! A VC is any signed_object whose schema declares a subject DID. The credential
//! itself lives in `signed_objects` (canonical authority); this module just adds
//! a fast-lookup index keyed on issuer/subject/schema for queries like
//! "show me all VCs about this DID".
//!
//! Indexing is triggered automatically from `put_signed_object` when a known
//! VC schema is detected. Revocations and withdrawals update the index in place.

use rusqlite::{OptionalExtension, params};

use super::Storage;
use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::object::Object;

/// One row of the VC fast-lookup index.
#[derive(Debug, Clone)]
pub struct CredentialIndex {
    pub vc_object_id: String,
    pub issuer_did: String,
    pub subject_did: String,
    pub schema_id: String,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub revoked_by_object_id: Option<String>,
    pub withdrawn: bool,
}

/// Schema id → `subject_field` extractor function. Hardcoded for Phase 1 PR 2;
/// future versions will load this from `data/identity/schemas.ron`.
///
/// Returns `None` if the schema isn't a credential (no subject), or `Some(did)`
/// where `did` is the credential subject.
pub fn extract_subject_did(object: &Object) -> Option<String> {
    match object.object_type.as_str() {
        // Subject = author (self-issued credentials)
        "member_v1" | "account_age_v1" | "ai_introduction_v1" | "subject_class_v1" => {
            Some(did_for_pubkey(&object.author_public_key))
        }
        // Subject is in payload.subject_did
        "vouch_v1"
        | "verified_human_v1"
        | "skill_endorsement_v1"
        | "graduation_v1"
        | "employment_v1"
        | "controlled_by_v1"
        | "role_v1"
        | "juror_v1"
        | "trust_score_v1"
        | "ai_consent_v1"
        | "attested_session_v1"
        | "liveness_v1" => extract_payload_did_field(object, "subject_did"),
        _ => None,
    }
}

/// Read `payload[field_name]` as a CBOR text string.
fn extract_payload_did_field(object: &Object, field_name: &str) -> Option<String> {
    if object.payload_encoding != "cbor_canonical_v1" {
        return None;
    }
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    let entries = match value {
        ciborium::Value::Map(e) => e,
        _ => return None,
    };
    for (k, v) in entries {
        if let ciborium::Value::Text(key) = k {
            if key == field_name {
                if let ciborium::Value::Text(s) = v {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Read `payload[field_name]` as a CBOR integer.
fn extract_payload_int_field(object: &Object, field_name: &str) -> Option<i64> {
    if object.payload_encoding != "cbor_canonical_v1" {
        return None;
    }
    let value = crate::relay::core::encoding::from_canonical_bytes(&object.payload).ok()?;
    let entries = match value {
        ciborium::Value::Map(e) => e,
        _ => return None,
    };
    for (k, v) in entries {
        if let ciborium::Value::Text(key) = k {
            if key == field_name {
                if let ciborium::Value::Integer(i) = v {
                    let raw: i128 = i.into();
                    if raw <= i64::MAX as i128 && raw >= 0 {
                        return Some(raw as i64);
                    }
                }
            }
        }
    }
    None
}

impl Storage {
    /// Index a freshly-stored signed_object as a credential, if its schema applies.
    /// Idempotent: re-indexing the same object_id is a no-op.
    /// Returns `Ok(true)` if a row was added, `Ok(false)` if not a VC or already indexed.
    pub fn index_credential(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        let subject_did = match extract_subject_did(object) {
            Some(d) => d,
            None => return Ok(false),
        };

        let issuer_did = did_for_pubkey(&object.author_public_key);
        let schema_id = object.object_type.clone();
        let vc_object_id = object
            .object_id()
            .map(|h| h.to_hex())
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )))?;

        let issued_at = object.created_at.map(|t| t as i64).unwrap_or_else(|| {
            super::now_millis() as i64
        });
        let expires_at = extract_payload_int_field(object, "expires_at");

        self.with_conn(|conn| {
            let rows = conn.execute(
                "INSERT OR IGNORE INTO vc_index
                    (vc_object_id, issuer_did, subject_did, schema_id,
                     issued_at, expires_at, revoked_by_object_id, withdrawn)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 0)",
                params![
                    vc_object_id,
                    issuer_did,
                    subject_did,
                    schema_id,
                    issued_at,
                    expires_at,
                ],
            )?;
            Ok(rows > 0)
        })
    }

    /// Mark a credential as revoked by recording the revocation object id.
    /// Returns `Ok(true)` if the credential existed and was newly marked.
    pub fn revoke_credential(
        &self,
        vc_object_id: &str,
        revocation_object_id: &str,
    ) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE vc_index
                 SET revoked_by_object_id = ?2
                 WHERE vc_object_id = ?1 AND revoked_by_object_id IS NULL",
                params![vc_object_id, revocation_object_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Mark a credential as withdrawn-by-subject (Accord consent: subject can hide
    /// their own VC without invalidating it). Returns `Ok(true)` on success.
    pub fn withdraw_credential(
        &self,
        vc_object_id: &str,
    ) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE vc_index SET withdrawn = 1 WHERE vc_object_id = ?1",
                params![vc_object_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// List credentials by filter. All filters optional.
    pub fn list_credentials(
        &self,
        subject_did: Option<&str>,
        issuer_did: Option<&str>,
        schema_id: Option<&str>,
        include_revoked: bool,
        include_withdrawn: bool,
        limit: Option<usize>,
    ) -> Result<Vec<CredentialIndex>, rusqlite::Error> {
        let limit = limit.unwrap_or(100).min(1000);

        let mut sql = String::from(
            "SELECT vc_object_id, issuer_did, subject_did, schema_id,
                    issued_at, expires_at, revoked_by_object_id, withdrawn
             FROM vc_index WHERE 1=1",
        );
        let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(s) = subject_did {
            sql.push_str(" AND subject_did = ?");
            binds.push(Box::new(s.to_string()));
        }
        if let Some(s) = issuer_did {
            sql.push_str(" AND issuer_did = ?");
            binds.push(Box::new(s.to_string()));
        }
        if let Some(s) = schema_id {
            sql.push_str(" AND schema_id = ?");
            binds.push(Box::new(s.to_string()));
        }
        if !include_revoked {
            sql.push_str(" AND revoked_by_object_id IS NULL");
        }
        if !include_withdrawn {
            sql.push_str(" AND withdrawn = 0");
        }
        sql.push_str(" ORDER BY issued_at DESC LIMIT ?");
        binds.push(Box::new(limit as i64));

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                binds.iter().map(|b| b.as_ref()).collect();
            let rows = stmt
                .query_map(rusqlite::params_from_iter(param_refs), |row| {
                    Ok(CredentialIndex {
                        vc_object_id: row.get(0)?,
                        issuer_did: row.get(1)?,
                        subject_did: row.get(2)?,
                        schema_id: row.get(3)?,
                        issued_at: row.get(4)?,
                        expires_at: row.get(5)?,
                        revoked_by_object_id: row.get(6)?,
                        withdrawn: row.get::<_, i64>(7)? != 0,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Get a single credential by its object id.
    pub fn get_credential(
        &self,
        vc_object_id: &str,
    ) -> Result<Option<CredentialIndex>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT vc_object_id, issuer_did, subject_did, schema_id,
                        issued_at, expires_at, revoked_by_object_id, withdrawn
                 FROM vc_index WHERE vc_object_id = ?1",
                params![vc_object_id],
                |row| {
                    Ok(CredentialIndex {
                        vc_object_id: row.get(0)?,
                        issuer_did: row.get(1)?,
                        subject_did: row.get(2)?,
                        schema_id: row.get(3)?,
                        issued_at: row.get(4)?,
                        expires_at: row.get(5)?,
                        revoked_by_object_id: row.get(6)?,
                        withdrawn: row.get::<_, i64>(7)? != 0,
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
    use crate::relay::core::encoding::{cbor_int, cbor_map, cbor_text};
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_vc_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn make_vouch(issuer: &DilithiumKeypair, subject_did: &str, note: &str) -> Object {
        let payload = cbor_map(vec![
            ("subject_did", cbor_text(subject_did)),
            ("vouch_kind", cbor_text("identity")),
            ("note", cbor_text(note)),
            ("timestamp", cbor_int(1_700_000_000)),
        ]);
        ObjectBuilder::new("vouch_v1")
            .created_at(1_700_000_000)
            .payload_cbor(&payload)
            .unwrap()
            .sign(issuer)
            .unwrap()
    }

    #[test]
    fn vouch_indexes_with_correct_subject_and_issuer() {
        let db = make_test_storage();
        let issuer = DilithiumKeypair::generate().unwrap();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        // put_signed_object auto-indexes — no separate call needed.
        let vouch = make_vouch(&issuer, &subject_did, "knows them well");
        db.put_signed_object(&vouch, None).unwrap();

        let listed = db
            .list_credentials(Some(&subject_did), None, None, false, false, None)
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].schema_id, "vouch_v1");
        assert_eq!(listed[0].issuer_did, did_for_pubkey(&issuer.public_key()));
        assert_eq!(listed[0].subject_did, subject_did);
    }

    #[test]
    fn member_v1_has_self_as_subject() {
        let db = make_test_storage();
        let user = DilithiumKeypair::generate().unwrap();
        let user_did = did_for_pubkey(&user.public_key());

        let member = ObjectBuilder::new("member_v1")
            .space_id("test_server")
            .created_at(1)
            .payload_cbor(&cbor_map(vec![
                ("server_id", cbor_text("test_server")),
                ("role", cbor_text("member")),
            ]))
            .unwrap()
            .sign(&user)
            .unwrap();

        db.put_signed_object(&member, None).unwrap();

        let listed = db
            .list_credentials(Some(&user_did), Some(&user_did), Some("member_v1"), false, false, None)
            .unwrap();
        assert_eq!(listed.len(), 1);
    }

    #[test]
    fn unknown_schema_is_not_indexed() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let obj = ObjectBuilder::new("random_post")
            .payload_cbor(&cbor_map(vec![("text", cbor_text("hi"))]))
            .unwrap()
            .sign(&kp)
            .unwrap();
        db.put_signed_object(&obj, None).unwrap();

        // Random non-VC types should not produce a vc_index row.
        let listed = db.list_credentials(None, None, None, true, true, None).unwrap();
        assert_eq!(listed.len(), 0);
    }

    #[test]
    fn revocation_excludes_from_default_list() {
        let db = make_test_storage();
        let issuer = DilithiumKeypair::generate().unwrap();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        let vouch = make_vouch(&issuer, &subject_did, "first");
        db.put_signed_object(&vouch, None).unwrap();
        let vc_id = vouch.object_id().unwrap().to_hex();

        // Default list: visible
        assert_eq!(
            db.list_credentials(Some(&subject_did), None, None, false, false, None).unwrap().len(),
            1
        );

        // Revoke
        let revoked = db.revoke_credential(&vc_id, "fake_revocation_id").unwrap();
        assert!(revoked);

        // Default list: hidden
        assert_eq!(
            db.list_credentials(Some(&subject_did), None, None, false, false, None).unwrap().len(),
            0
        );

        // include_revoked: visible
        assert_eq!(
            db.list_credentials(Some(&subject_did), None, None, true, false, None).unwrap().len(),
            1
        );

        // Idempotent
        let already = db.revoke_credential(&vc_id, "another_revocation").unwrap();
        assert!(!already);
    }

    #[test]
    fn withdrawal_excludes_from_default_list() {
        let db = make_test_storage();
        let issuer = DilithiumKeypair::generate().unwrap();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        let vouch = make_vouch(&issuer, &subject_did, "x");
        db.put_signed_object(&vouch, None).unwrap();
        let vc_id = vouch.object_id().unwrap().to_hex();

        db.withdraw_credential(&vc_id).unwrap();

        let default = db.list_credentials(Some(&subject_did), None, None, false, false, None).unwrap();
        assert_eq!(default.len(), 0);

        let with_withdrawn = db.list_credentials(Some(&subject_did), None, None, false, true, None).unwrap();
        assert_eq!(with_withdrawn.len(), 1);
        assert!(with_withdrawn[0].withdrawn);
    }

    #[test]
    fn revocation_via_signed_object_only_works_for_issuer() {
        let db = make_test_storage();
        let issuer = DilithiumKeypair::generate().unwrap();
        let attacker = DilithiumKeypair::generate().unwrap();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        let vouch = make_vouch(&issuer, &subject_did, "to-revoke");
        db.put_signed_object(&vouch, None).unwrap();
        let vc_id = vouch.object_id().unwrap().to_hex();

        // Attacker tries to revoke — should be silently ignored.
        let attacker_revocation = ObjectBuilder::new("revocation_v1")
            .reference(&vc_id)
            .created_at(1)
            .payload_cbor(&cbor_map(vec![("reason", cbor_text("malicious"))]))
            .unwrap()
            .sign(&attacker)
            .unwrap();
        db.put_signed_object(&attacker_revocation, None).unwrap();

        let still_visible = db
            .list_credentials(Some(&subject_did), None, None, false, false, None)
            .unwrap();
        assert_eq!(still_visible.len(), 1, "attacker revocation must be ignored");

        // Real issuer revokes — should hide the VC.
        let real_revocation = ObjectBuilder::new("revocation_v1")
            .reference(&vc_id)
            .created_at(2)
            .payload_cbor(&cbor_map(vec![("reason", cbor_text("issuer-decision"))]))
            .unwrap()
            .sign(&issuer)
            .unwrap();
        db.put_signed_object(&real_revocation, None).unwrap();

        let after_real = db
            .list_credentials(Some(&subject_did), None, None, false, false, None)
            .unwrap();
        assert_eq!(after_real.len(), 0, "issuer revocation must hide VC");
    }

    #[test]
    fn withdrawal_via_signed_object_only_works_for_subject() {
        let db = make_test_storage();
        let issuer = DilithiumKeypair::generate().unwrap();
        let attacker = DilithiumKeypair::generate().unwrap();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        let vouch = make_vouch(&issuer, &subject_did, "to-withdraw");
        db.put_signed_object(&vouch, None).unwrap();
        let vc_id = vouch.object_id().unwrap().to_hex();

        // Attacker (not subject, not issuer) tries to withdraw — ignored.
        let attacker_w = ObjectBuilder::new("withdrawal_v1")
            .reference(&vc_id)
            .created_at(1)
            .payload_cbor(&cbor_map(vec![("reason", cbor_text("griefing"))]))
            .unwrap()
            .sign(&attacker)
            .unwrap();
        db.put_signed_object(&attacker_w, None).unwrap();

        let still_visible = db
            .list_credentials(Some(&subject_did), None, None, false, false, None)
            .unwrap();
        assert_eq!(still_visible.len(), 1);

        // Subject withdraws — succeeds.
        let real_w = ObjectBuilder::new("withdrawal_v1")
            .reference(&vc_id)
            .created_at(2)
            .payload_cbor(&cbor_map(vec![("reason", cbor_text("not-mine-anymore"))]))
            .unwrap()
            .sign(&subject_kp)
            .unwrap();
        db.put_signed_object(&real_w, None).unwrap();

        let after = db
            .list_credentials(Some(&subject_did), None, None, false, false, None)
            .unwrap();
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn list_filters_combine() {
        let db = make_test_storage();
        let alice = DilithiumKeypair::generate().unwrap();
        let bob = DilithiumKeypair::generate().unwrap();
        let charlie = DilithiumKeypair::generate().unwrap();
        let charlie_did = did_for_pubkey(&charlie.public_key());

        let vouch_a = make_vouch(&alice, &charlie_did, "from-alice");
        let vouch_b = make_vouch(&bob, &charlie_did, "from-bob");
        db.put_signed_object(&vouch_a, None).unwrap();
        db.put_signed_object(&vouch_b, None).unwrap();

        let alice_did = did_for_pubkey(&alice.public_key());
        let only_alice = db
            .list_credentials(Some(&charlie_did), Some(&alice_did), Some("vouch_v1"), false, false, None)
            .unwrap();
        assert_eq!(only_alice.len(), 1);
        assert_eq!(only_alice[0].issuer_did, alice_did);
    }
}

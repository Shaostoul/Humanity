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

/// Where a resolution came from. A membership hit is a real key with NO
/// publication history, and saying so is the difference between "we know this
/// key and nothing else" and three zeros that read as facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DidSource {
    /// Found via signed_objects: the key signed something this server holds.
    Objects,
    /// Found via server_members: the key identified to this server. Just as
    /// authoritative for the key itself, since it was recorded after the
    /// Dilithium nonce challenge at identify, but it carries no history.
    Membership,
}

impl DidSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            DidSource::Objects => "objects",
            DidSource::Membership => "membership",
        }
    }
}

/// Resolved DID → identity record. Sufficient for verifying signatures from this DID.
#[derive(Debug, Clone)]
pub struct DidResolution {
    pub did: String,
    pub current_pubkey: Vec<u8>,
    pub author_fp_hex: String,
    pub source: DidSource,
    /// First time this server saw an object signed by this key. None for a
    /// membership hit: unknown, not zero.
    pub first_seen: Option<i64>,
    /// Most recent object signed by this key. None for a membership hit.
    pub last_seen: Option<i64>,
    /// Total objects this server has from this DID. None for a membership hit.
    pub object_count: Option<u64>,
}

impl Storage {
    /// Resolve a DID fingerprint (16 bytes) to a `DidResolution` by looking up
    /// any signed_objects row with matching `author_fp`.
    ///
    /// Returns `Ok(None)` if no objects from this DID have been seen.
    pub fn resolve_did_fp(&self, fp: &Fingerprint) -> Result<Option<DidResolution>, rusqlite::Error> {
        let fp_hex = crate::relay::core::did::fingerprint_to_hex(fp);

        // Read-only: single aggregate query_row over signed_objects (DID →
        // current pubkey resolution, a signature-verification hot path). Read pool.
        self.with_read_conn(|conn| {
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

            if let Some((pubkey, first_seen, last_seen, count)) = row {
                return Ok(Some(DidResolution {
                    did: fingerprint_to_did(fp),
                    current_pubkey: pubkey,
                    author_fp_hex: fp_hex.clone(),
                    source: DidSource::Objects,
                    first_seen: Some(first_seen),
                    last_seen: Some(last_seen),
                    object_count: Some(count as u64),
                }));
            }

            // FALLBACK: the member roster.
            //
            // A DID is base58 of BLAKE3(public key)[..16], and server_members
            // already holds the full 1952-byte Dilithium key for everyone who
            // has ever identified to this server. So the key needed to resolve
            // almost every DID was always present, one table over, and this
            // resolver was simply not looking: on the live server 3 of 43
            // members resolved, because only 3 had ever published a signed
            // object. The other 40, including the operator's own identity,
            // returned 404 for a key the server had on file.
            //
            // This is not a weaker answer about the KEY. server_members is
            // written by join_server at identify time, which happens after the
            // Dilithium nonce challenge, so the key was proven to be held by
            // whoever presented it. It is a weaker answer about HISTORY, which
            // is why first_seen, last_seen and object_count are None here
            // rather than zero.
            let member = conn
                .query_row(
                    "SELECT public_key FROM server_members WHERE did_fp = ?1 LIMIT 1",
                    params![fp_hex],
                    |r| r.get::<_, String>(0),
                )
                .optional()?;

            let Some(pk_hex) = member else { return Ok(None) };
            let Ok(pubkey) = hex::decode(&pk_hex) else { return Ok(None) };

            Ok(Some(DidResolution {
                did: fingerprint_to_did(fp),
                current_pubkey: pubkey,
                author_fp_hex: fp_hex.clone(),
                source: DidSource::Membership,
                first_seen: None,
                last_seen: None,
                object_count: None,
            }))
        })
    }

    /// Fill in did_fp for members who joined before the column existed.
    ///
    /// Runs once at startup. Bounded by the member count and BLAKE3 is fast, so
    /// even a large roster is a short pause; the alternative is hashing every
    /// member's key on every resolution miss, on a public unauthenticated
    /// endpoint, which is a table scan anyone can request at will.
    pub fn backfill_member_did_fp(&self) -> Result<usize, rusqlite::Error> {
        let rows: Vec<String> = self.with_conn(|conn| {
            let mut stmt = conn
                .prepare("SELECT public_key FROM server_members WHERE did_fp IS NULL")?;
            let v: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            Ok::<Vec<String>, rusqlite::Error>(v)
        })?;
        if rows.is_empty() {
            return Ok(0);
        }
        let mut done = 0usize;
        self.with_conn(|conn| {
            for pk_hex in &rows {
                let Ok(pk) = hex::decode(pk_hex) else { continue };
                let fp_hex = crate::relay::core::did::fingerprint_to_hex(
                    &crate::relay::core::did::fingerprint_of(&pk),
                );
                conn.execute(
                    "UPDATE server_members SET did_fp = ?1 WHERE public_key = ?2",
                    params![fp_hex, pk_hex],
                )?;
                done += 1;
            }
            Ok::<(), rusqlite::Error>(())
        })?;
        Ok(done)
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
        assert_eq!(res.object_count, Some(1));
        assert_eq!(res.source, DidSource::Objects);
    }

    /// THE FIX. A member who has never published anything still resolves,
    /// because server_members already holds their full Dilithium key: it is
    /// written by join_server at identify time, AFTER the nonce challenge, so
    /// the key was proven held by whoever presented it.
    ///
    /// Measured on the live server before this: 3 of 43 members resolved. The
    /// other 40, the operator's own identity among them, returned 404 for a key
    /// the server had on file one table over.
    #[test]
    fn resolves_a_member_who_has_never_published() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());
        let pk_hex = hex::encode(kp.public_key());

        // No signed objects at all: just a member who identified once.
        db.register_name("Quiet", &pk_hex).unwrap();
        db.join_server(&pk_hex, "Quiet").unwrap();

        let res = db.resolve_did(&did).unwrap().expect("a member must resolve");
        assert_eq!(res.did, did);
        assert_eq!(res.current_pubkey, kp.public_key(), "the key must be exact");
        assert_eq!(res.source, DidSource::Membership);
        // History is UNKNOWN, not zero. Reporting zeros here would say "joined
        // at the epoch and never did anything", which is a different claim.
        assert_eq!(res.first_seen, None);
        assert_eq!(res.last_seen, None);
        assert_eq!(res.object_count, None);
    }

    /// Objects win when both exist, because they carry history the roster does
    /// not. Without this the fallback could silently mask the richer answer.
    #[test]
    fn a_published_object_beats_the_membership_row() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());
        let pk_hex = hex::encode(kp.public_key());
        db.register_name("Loud", &pk_hex).unwrap();
        db.join_server(&pk_hex, "Loud").unwrap();

        let payload = cbor_map(vec![("greeting", cbor_text("hi"))]);
        let obj = ObjectBuilder::new("post").payload_cbor(&payload).unwrap().sign(&kp).unwrap();
        db.put_signed_object(&obj, None).unwrap();

        let res = db.resolve_did(&did).unwrap().expect("resolved");
        assert_eq!(res.source, DidSource::Objects);
        assert_eq!(res.object_count, Some(1));
    }

    /// A stranger who never joined and never published still resolves to
    /// nothing. The fallback must not turn every 16-byte string into a hit.
    #[test]
    fn a_non_member_still_resolves_to_nothing() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        assert!(db.resolve_did(&did_for_pubkey(&kp.public_key())).unwrap().is_none());
    }

    /// Members who joined before did_fp existed are backfilled at startup.
    /// Simulated by clearing the column, which is exactly the live-database
    /// shape this migration lands on.
    #[test]
    fn backfill_repairs_members_who_predate_the_column() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());
        let pk_hex = hex::encode(kp.public_key());
        db.register_name("Old", &pk_hex).unwrap();
        db.join_server(&pk_hex, "Old").unwrap();

        // Wind the row back to its pre-migration state.
        db.with_conn(|conn| {
            conn.execute("UPDATE server_members SET did_fp = NULL", [])?;
            Ok::<(), rusqlite::Error>(())
        })
        .unwrap();
        assert!(
            db.resolve_did(&did).unwrap().is_none(),
            "precondition: an un-backfilled member does not resolve"
        );

        assert_eq!(db.backfill_member_did_fp().unwrap(), 1);
        assert!(db.resolve_did(&did).unwrap().is_some(), "and resolves after backfill");
        // Idempotent: running it again finds nothing left to do.
        assert_eq!(db.backfill_member_did_fp().unwrap(), 0);
    }

    /// Opens a database with the PRE-MIGRATION server_members shape and proves
    /// the ALTER lands and resolution works afterwards.
    ///
    /// This test exists because of BUG-046, which took the live relay down for
    /// ~25 minutes: the main schema batch runs BEFORE the ALTER block, so a
    /// fresh database (where CREATE TABLE already includes the new column)
    /// passes every test while the live database aborts at startup. Every unit
    /// test above builds a fresh database, so none of them can catch it. This
    /// one builds the old shape on purpose.
    #[test]
    fn opens_a_pre_did_fp_database_and_migrates_it() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_did_mig_{pid}_{nanos}.db"));

        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());
        let pk_hex = hex::encode(kp.public_key());

        // The live table shape before 2026-09-06: no did_fp, and a member row
        // already in it.
        {
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "CREATE TABLE server_members (
                    public_key TEXT PRIMARY KEY,
                    name       TEXT,
                    role       TEXT NOT NULL DEFAULT 'member',
                    joined_at  TEXT NOT NULL,
                    last_seen  TEXT,
                    hide_presence INTEGER NOT NULL DEFAULT 0
                );",
            )
            .expect("old schema");
            conn.execute(
                "INSERT INTO server_members (public_key, name, role, joined_at)
                 VALUES (?1, 'Veteran', 'member', datetime('now'))",
                params![pk_hex],
            )
            .expect("old row");
        }

        // Opening must migrate rather than abort.
        let db = Storage::open(&path).expect("must open a pre-migration database");

        // The pre-existing member has no fingerprint yet, so does not resolve.
        assert!(
            db.resolve_did(&did).unwrap().is_none(),
            "precondition: the migrated column starts NULL for existing rows"
        );

        // Startup backfill repairs them.
        assert_eq!(db.backfill_member_did_fp().unwrap(), 1);
        let res = db.resolve_did(&did).unwrap().expect("resolves after backfill");
        assert_eq!(res.current_pubkey, kp.public_key());
        assert_eq!(res.source, DidSource::Membership);
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
        assert_eq!(res.object_count, Some(3));
    }
}

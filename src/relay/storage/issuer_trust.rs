//! Per-observer per-issuer trust scoring (Phase 3 PR 2).
//!
//! Each server tracks how trustworthy it considers each issuer DID it has seen.
//! `trust` is a continuous [0, 1] value derived from observed good/bad signals.
//!
//! "Good" events: issuing a VC that survives without dispute, signature validating
//! cleanly, schema compliance.
//!
//! "Bad" events: a successful `dispute_v1` against the issuer, schema violation,
//! claiming AI status without `controlled_by_v1` binding.
//!
//! Containment without single-revocation-point (Accord non-domination):
//! issuers don't get banned. Their trust just drops, which down-weights their
//! VCs in Phase 2 trust scores. Restoration is symmetric — issuers rebuild
//! trust over time by issuing valid undisputed VCs.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Storage;
use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::object::Object;

/// Default neutral trust for an issuer we've just observed. Symmetric (not
/// trusting nor distrusting) so disputes and good behavior have equal weight.
pub const NEUTRAL_TRUST: f64 = 0.5;

/// Maximum delta a single event can apply (caps both rewards and penalties).
pub const MAX_DELTA: f64 = 0.05;

/// One row of the per-observer per-issuer trust matrix.
#[derive(Debug, Clone, Serialize)]
pub struct IssuerTrustRow {
    pub observer_server: String,
    pub issuer_did: String,
    pub trust: f64,
    pub good_count: u64,
    pub bad_count: u64,
    pub last_event_at: i64,
}

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

impl Storage {
    /// Record a positive event for an issuer (e.g. valid VC observed). Idempotent
    /// on (observer, issuer) — updates the row in place.
    pub fn issuer_trust_good(
        &self,
        observer_server: &str,
        issuer_did: &str,
        delta: f64,
    ) -> Result<f64, rusqlite::Error> {
        let delta = delta.clamp(0.0, MAX_DELTA);
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            let existing: Option<(f64, u64, u64)> = conn
                .query_row(
                    "SELECT trust, good_count, bad_count FROM issuer_trust
                     WHERE observer_server = ?1 AND issuer_did = ?2",
                    params![observer_server, issuer_did],
                    |r| {
                        Ok((
                            r.get::<_, f64>(0)?,
                            r.get::<_, i64>(1)? as u64,
                            r.get::<_, i64>(2)? as u64,
                        ))
                    },
                )
                .optional()?;

            let (new_trust, good, bad) = match existing {
                Some((t, g, b)) => ((t + delta).min(1.0), g + 1, b),
                None => ((NEUTRAL_TRUST + delta).min(1.0), 1, 0),
            };

            conn.execute(
                "INSERT INTO issuer_trust
                    (observer_server, issuer_did, trust, good_count, bad_count, last_event_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(observer_server, issuer_did) DO UPDATE SET
                   trust = excluded.trust,
                   good_count = excluded.good_count,
                   bad_count = excluded.bad_count,
                   last_event_at = excluded.last_event_at",
                params![
                    observer_server,
                    issuer_did,
                    new_trust,
                    good as i64,
                    bad as i64,
                    now,
                ],
            )?;
            Ok(new_trust)
        })
    }

    /// Record a negative event (dispute confirmed, schema violation, etc.).
    pub fn issuer_trust_bad(
        &self,
        observer_server: &str,
        issuer_did: &str,
        delta: f64,
    ) -> Result<f64, rusqlite::Error> {
        let delta = delta.clamp(0.0, MAX_DELTA);
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            let existing: Option<(f64, u64, u64)> = conn
                .query_row(
                    "SELECT trust, good_count, bad_count FROM issuer_trust
                     WHERE observer_server = ?1 AND issuer_did = ?2",
                    params![observer_server, issuer_did],
                    |r| {
                        Ok((
                            r.get::<_, f64>(0)?,
                            r.get::<_, i64>(1)? as u64,
                            r.get::<_, i64>(2)? as u64,
                        ))
                    },
                )
                .optional()?;

            let (new_trust, good, bad) = match existing {
                Some((t, g, b)) => ((t - delta).max(0.0), g, b + 1),
                None => ((NEUTRAL_TRUST - delta).max(0.0), 0, 1),
            };

            conn.execute(
                "INSERT INTO issuer_trust
                    (observer_server, issuer_did, trust, good_count, bad_count, last_event_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(observer_server, issuer_did) DO UPDATE SET
                   trust = excluded.trust,
                   good_count = excluded.good_count,
                   bad_count = excluded.bad_count,
                   last_event_at = excluded.last_event_at",
                params![
                    observer_server,
                    issuer_did,
                    new_trust,
                    good as i64,
                    bad as i64,
                    now,
                ],
            )?;
            Ok(new_trust)
        })
    }

    /// Get the current trust value for an issuer (defaults to NEUTRAL_TRUST if
    /// never observed). Caller's perspective via `observer_server`.
    pub fn issuer_trust(&self, observer_server: &str, issuer_did: &str) -> f64 {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT trust FROM issuer_trust
                 WHERE observer_server = ?1 AND issuer_did = ?2",
                params![observer_server, issuer_did],
                |r| r.get::<_, f64>(0),
            )
            .ok()
        })
        .unwrap_or(NEUTRAL_TRUST)
    }

    /// Index a `dispute_v1` object: payload references the target VC/issuer.
    /// Decrements issuer trust for the disputed party.
    pub fn index_dispute(&self, object: &Object, observer_server: &str) -> Result<bool, rusqlite::Error> {
        if object.object_type != "dispute_v1" {
            return Ok(false);
        }
        // Disputes can target either a specific VC (via reference) or an issuer DID
        // directly via payload.target_did.
        let target_did = if let Some(target_id) = object.references.first() {
            // Look up the disputed VC and use its issuer
            match self.get_credential(target_id)? {
                Some(c) => c.issuer_did,
                None => match read_text(object, "target_did") {
                    Some(d) => d,
                    None => return Ok(false),
                },
            }
        } else if let Some(d) = read_text(object, "target_did") {
            d
        } else {
            return Ok(false);
        };

        let disputer_did = did_for_pubkey(&object.author_public_key);
        // Self-disputes are no-ops (can't dispute yourself meaningfully)
        if disputer_did == target_did {
            return Ok(false);
        }

        // Disputes are observed events. Apply a small bad-trust delta;
        // multiple disputes accumulate.
        self.issuer_trust_bad(observer_server, &target_did, 0.02)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::encoding::{cbor_map, cbor_text};
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_itrust_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    #[test]
    fn unknown_issuer_has_neutral_trust() {
        let db = make_test_storage();
        let t = db.issuer_trust("server-A", "did:hum:unknown");
        assert_eq!(t, NEUTRAL_TRUST);
    }

    #[test]
    fn good_event_raises_trust() {
        let db = make_test_storage();
        let issuer = "did:hum:abc";
        let observer = "server-A";

        let t1 = db.issuer_trust_good(observer, issuer, 0.05).unwrap();
        assert!(t1 > NEUTRAL_TRUST);

        let t2 = db.issuer_trust_good(observer, issuer, 0.05).unwrap();
        assert!(t2 > t1);
    }

    #[test]
    fn bad_event_drops_trust() {
        let db = make_test_storage();
        let t1 = db.issuer_trust_bad("server-A", "did:hum:bad", 0.05).unwrap();
        assert!(t1 < NEUTRAL_TRUST);
    }

    #[test]
    fn delta_caps_at_max() {
        let db = make_test_storage();
        // Try to apply oversized delta — should be clamped
        let t = db.issuer_trust_good("server-A", "did:hum:x", 1.0).unwrap();
        assert_eq!(t, (NEUTRAL_TRUST + MAX_DELTA).min(1.0));
    }

    #[test]
    fn trust_clamped_to_unit_interval() {
        let db = make_test_storage();
        let issuer = "did:hum:y";
        let observer = "server-A";
        // Lots of good events should saturate at 1.0
        for _ in 0..100 {
            db.issuer_trust_good(observer, issuer, 0.05).unwrap();
        }
        let t = db.issuer_trust(observer, issuer);
        assert!(t <= 1.0);

        // Lots of bad events should saturate at 0.0
        let issuer2 = "did:hum:z";
        for _ in 0..100 {
            db.issuer_trust_bad(observer, issuer2, 0.05).unwrap();
        }
        let t2 = db.issuer_trust(observer, issuer2);
        assert!(t2 >= 0.0);
    }

    #[test]
    fn dispute_object_lowers_target_trust() {
        let db = make_test_storage();
        let observer = "server-A";

        // Create a vouch first so we have a credential to dispute
        let issuer_kp = DilithiumKeypair::generate().unwrap();
        let issuer_did = did_for_pubkey(&issuer_kp.public_key());
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        let vouch = ObjectBuilder::new("vouch_v1")
            .created_at(1)
            .payload_cbor(&cbor_map(vec![
                ("subject_did", cbor_text(&subject_did)),
                ("vouch_kind", cbor_text("identity")),
            ]))
            .unwrap()
            .sign(&issuer_kp)
            .unwrap();
        db.put_signed_object(&vouch, None).unwrap();
        let vc_id = vouch.object_id().unwrap().to_hex();

        // A third party files a dispute
        let disputer = DilithiumKeypair::generate().unwrap();
        let dispute = ObjectBuilder::new("dispute_v1")
            .reference(&vc_id)
            .created_at(2)
            .payload_cbor(&cbor_map(vec![("reason", cbor_text("fake credential"))]))
            .unwrap()
            .sign(&disputer)
            .unwrap();

        // Note: dispute objects normally come through put_signed_object's side-effect.
        // Test the storage layer directly:
        let indexed = db.index_dispute(&dispute, observer).unwrap();
        assert!(indexed);

        let new_trust = db.issuer_trust(observer, &issuer_did);
        assert!(new_trust < NEUTRAL_TRUST);
    }

    #[test]
    fn self_dispute_is_noop() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        let self_dispute = ObjectBuilder::new("dispute_v1")
            .created_at(1)
            .payload_cbor(&cbor_map(vec![
                ("target_did", cbor_text(&did)),
                ("reason", cbor_text("nope")),
            ]))
            .unwrap()
            .sign(&kp)
            .unwrap();

        let indexed = db.index_dispute(&self_dispute, "server-A").unwrap();
        assert!(!indexed);
        assert_eq!(db.issuer_trust("server-A", &did), NEUTRAL_TRUST);
    }
}

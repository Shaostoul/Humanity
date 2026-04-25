//! Multi-layer trust score (Phase 2 PR 1).
//!
//! Aggregates VC count, vouching graph entropy, activity diversity, account age,
//! and economic stake into a normalized score in [0, 1]. Inputs are always
//! exposed (Accord transparency) — verifiers can recompute the score themselves.
//!
//! Caching: scores are computed lazily on read, cached for 5 minutes via the
//! `trust_scores` table, and re-computed on demand.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::Storage;
use crate::relay::core::did::{Fingerprint, fingerprint_to_hex};

/// 5-minute cache TTL for trust scores.
const CACHE_TTL_MS: i64 = 5 * 60 * 1000;

/// Hardcoded weights for Phase 2. Future revisions will read from
/// `data/identity/trust_weights.ron`.
const W_VCS: f64 = 0.30;
const W_VOUCHING: f64 = 0.25;
const W_ACTIVITY: f64 = 0.15;
const W_AGE: f64 = 0.10;
const W_STAKE: f64 = 0.10;
const W_REPUTATION: f64 = 0.10;

const WEIGHTS_VERSION: i64 = 1;

/// Window for "recent activity" — last 90 days in milliseconds.
const ACTIVITY_WINDOW_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// Sub-score breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubScores {
    pub vcs: f64,
    pub vouching_graph: f64,
    pub activity_diversity: f64,
    pub age: f64,
    pub economic_stake: f64,
    pub reputation: f64,
}

/// Inputs that fed the score (Accord transparency — every score includes these).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustInputs {
    pub vc_count: u64,
    pub distinct_voucher_communities: u64,
    pub distinct_activity_types: u64,
    pub account_age_days: f64,
    pub computed_for_did: String,
}

/// A computed trust score for a DID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScore {
    pub did: String,
    pub total: f64,
    pub sub_scores: SubScores,
    pub inputs: TrustInputs,
    pub weights_version: i64,
    pub computed_at: i64,
    pub cached: bool,
}

impl Storage {
    /// Compute (or fetch from cache) the trust score for a DID.
    /// `force_recompute` skips the cache.
    pub fn get_trust_score(&self, did: &str, force_recompute: bool) -> Result<TrustScore, String> {
        let fp = crate::relay::core::did::parse_did_hum(did)
            .map_err(|e| format!("invalid did: {e}"))?;
        let fp_hex = fingerprint_to_hex(&fp);
        let now = super::now_millis() as i64;

        if !force_recompute {
            if let Some(cached) = self.read_cached_trust_score(did, now)? {
                return Ok(cached);
            }
        }

        let computed = self.compute_trust_score_inner(did, &fp_hex, now)?;
        let _ = self.write_cached_trust_score(&computed);
        Ok(computed)
    }

    fn read_cached_trust_score(
        &self,
        did: &str,
        now: i64,
    ) -> Result<Option<TrustScore>, String> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT total, sub_scores_json, inputs_json, weights_version, computed_at
                 FROM trust_scores
                 WHERE did = ?1 AND computed_at > ?2",
                params![did, now - CACHE_TTL_MS],
                |row| {
                    let total: f64 = row.get(0)?;
                    let sub_scores_json: String = row.get(1)?;
                    let inputs_json: String = row.get(2)?;
                    let weights_version: i64 = row.get(3)?;
                    let computed_at: i64 = row.get(4)?;
                    Ok((total, sub_scores_json, inputs_json, weights_version, computed_at))
                },
            )
            .optional()
            .map_err(|e| format!("cache read: {e}"))
        })
        .and_then(|opt| {
            Ok(opt.and_then(|(total, sub_json, inputs_json, version, computed_at)| {
                let sub_scores: SubScores = serde_json::from_str(&sub_json).ok()?;
                let inputs: TrustInputs = serde_json::from_str(&inputs_json).ok()?;
                Some(TrustScore {
                    did: did.to_string(),
                    total,
                    sub_scores,
                    inputs,
                    weights_version: version,
                    computed_at,
                    cached: true,
                })
            }))
        })
    }

    fn write_cached_trust_score(&self, score: &TrustScore) -> Result<(), rusqlite::Error> {
        let sub_json = serde_json::to_string(&score.sub_scores).unwrap_or_else(|_| "{}".into());
        let inputs_json = serde_json::to_string(&score.inputs).unwrap_or_else(|_| "{}".into());

        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO trust_scores (did, total, sub_scores_json, inputs_json,
                                           weights_version, computed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(did) DO UPDATE SET
                   total = excluded.total,
                   sub_scores_json = excluded.sub_scores_json,
                   inputs_json = excluded.inputs_json,
                   weights_version = excluded.weights_version,
                   computed_at = excluded.computed_at",
                params![
                    score.did,
                    score.total,
                    sub_json,
                    inputs_json,
                    score.weights_version,
                    score.computed_at,
                ],
            )?;
            Ok(())
        })
    }

    fn compute_trust_score_inner(
        &self,
        did: &str,
        fp_hex: &str,
        now: i64,
    ) -> Result<TrustScore, String> {
        // ---- vcs sub-score ----
        let vc_count: u64 = self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM vc_index
                 WHERE subject_did = ?1 AND revoked_by_object_id IS NULL AND withdrawn = 0",
                params![did],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n as u64)
            .unwrap_or(0)
        });
        let vcs_score = sigmoid(vc_count as f64 / 5.0); // 5 VCs ~= 0.5

        // ---- vouching_graph sub-score ----
        // Distinct issuer_dids from vouch_v1 VCs about this subject.
        let voucher_dids: Vec<String> = self.with_conn(|conn| {
            conn.prepare(
                "SELECT DISTINCT issuer_did FROM vc_index
                 WHERE subject_did = ?1 AND schema_id = 'vouch_v1'
                   AND revoked_by_object_id IS NULL AND withdrawn = 0",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![did], |row| row.get::<_, String>(0))
                    .and_then(|r| r.collect::<rusqlite::Result<Vec<_>>>())
            })
            .unwrap_or_default()
        });
        let voucher_communities = voucher_dids.iter().collect::<HashSet<_>>().len() as u64;
        let vouching_score = sigmoid(voucher_communities as f64 / 3.0);

        // ---- activity_diversity sub-score ----
        let cutoff = now - ACTIVITY_WINDOW_MS;
        let activity_types: Vec<String> = self.with_conn(|conn| {
            conn.prepare(
                "SELECT DISTINCT object_type FROM signed_objects
                 WHERE author_fp = ?1 AND received_at > ?2",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![fp_hex, cutoff], |row| row.get::<_, String>(0))
                    .and_then(|r| r.collect::<rusqlite::Result<Vec<_>>>())
            })
            .unwrap_or_default()
        });
        let activity_score = sigmoid(activity_types.len() as f64 / 4.0);

        // ---- age sub-score ----
        let first_seen: Option<i64> = self.with_conn(|conn| {
            conn.query_row(
                "SELECT MIN(received_at) FROM signed_objects WHERE author_fp = ?1",
                params![fp_hex],
                |row| row.get::<_, Option<i64>>(0),
            )
            .ok()
            .flatten()
        });
        let age_days = match first_seen {
            Some(ts) => ((now - ts) as f64 / (24.0 * 60.0 * 60.0 * 1000.0)).max(0.0),
            None => 0.0,
        };
        // 30 days → 0.5, plateauing toward 1.0 by ~365 days
        let age_score = sigmoid(age_days / 60.0);

        // ---- economic_stake (Phase 6 will populate; 0 for now) ----
        let economic_stake_score = 0.0;

        // ---- reputation (legacy events; bridged via did → pubkey hex) ----
        // For now, leave at 0 unless the legacy reputation table happens to be
        // keyed on something we can derive (not the case for fresh PQ DIDs).
        // Phase 2.5 will add a did → legacy_pubkey_hex bridge.
        let reputation_score = 0.0;

        let sub_scores = SubScores {
            vcs: vcs_score,
            vouching_graph: vouching_score,
            activity_diversity: activity_score,
            age: age_score,
            economic_stake: economic_stake_score,
            reputation: reputation_score,
        };

        let total = (W_VCS * sub_scores.vcs
            + W_VOUCHING * sub_scores.vouching_graph
            + W_ACTIVITY * sub_scores.activity_diversity
            + W_AGE * sub_scores.age
            + W_STAKE * sub_scores.economic_stake
            + W_REPUTATION * sub_scores.reputation)
            .clamp(0.0, 1.0);

        let inputs = TrustInputs {
            vc_count,
            distinct_voucher_communities: voucher_communities,
            distinct_activity_types: activity_types.len() as u64,
            account_age_days: age_days,
            computed_for_did: did.to_string(),
        };

        Ok(TrustScore {
            did: did.to_string(),
            total,
            sub_scores,
            inputs,
            weights_version: WEIGHTS_VERSION,
            computed_at: now,
            cached: false,
        })
    }

    /// Convenience: compute (with cache) for a fingerprint instead of DID string.
    pub fn get_trust_score_for_fp(&self, fp: &Fingerprint) -> Result<TrustScore, String> {
        let did = crate::relay::core::did::fingerprint_to_did(fp);
        self.get_trust_score(&did, false)
    }
}

/// Sigmoid that maps [0, ∞) into [0, 1) with smooth growth, ~0.5 at x=1.
fn sigmoid(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else {
        x / (1.0 + x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::did::did_for_pubkey;
    use crate::relay::core::encoding::{cbor_int, cbor_map, cbor_text};
    use crate::relay::core::object::ObjectBuilder;
    use crate::relay::core::pq_crypto::DilithiumKeypair;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_trust_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn make_vouch(issuer: &DilithiumKeypair, subject_did: &str, suffix: &str) -> crate::relay::core::object::Object {
        let payload = cbor_map(vec![
            ("subject_did", cbor_text(subject_did)),
            ("vouch_kind", cbor_text("identity")),
            ("note", cbor_text(suffix)),
            ("timestamp", cbor_int(super::super::now_millis())),
        ]);
        ObjectBuilder::new("vouch_v1")
            .created_at(super::super::now_millis())
            .payload_cbor(&payload)
            .unwrap()
            .sign(issuer)
            .unwrap()
    }

    #[test]
    fn unknown_did_has_zero_total() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        let score = db.get_trust_score(&did, false).unwrap();
        assert_eq!(score.total, 0.0);
        assert_eq!(score.inputs.vc_count, 0);
        assert!(!score.cached); // first compute
    }

    #[test]
    fn vouches_increase_score() {
        let db = make_test_storage();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        let baseline = db.get_trust_score(&subject_did, true).unwrap();

        // 4 distinct vouchers
        for i in 0..4 {
            let voucher = DilithiumKeypair::generate().unwrap();
            let v = make_vouch(&voucher, &subject_did, &i.to_string());
            db.put_signed_object(&v, None).unwrap();
        }

        let with_vouches = db.get_trust_score(&subject_did, true).unwrap();
        assert!(with_vouches.total > baseline.total);
        assert_eq!(with_vouches.inputs.vc_count, 4);
        assert_eq!(with_vouches.inputs.distinct_voucher_communities, 4);
    }

    #[test]
    fn cached_score_returns_cached_flag() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        let first = db.get_trust_score(&did, false).unwrap();
        assert!(!first.cached);

        let second = db.get_trust_score(&did, false).unwrap();
        assert!(second.cached);
        assert_eq!(first.total, second.total);
    }

    #[test]
    fn force_recompute_bypasses_cache() {
        let db = make_test_storage();
        let kp = DilithiumKeypair::generate().unwrap();
        let did = did_for_pubkey(&kp.public_key());

        let _cached = db.get_trust_score(&did, false).unwrap();
        let recomputed = db.get_trust_score(&did, true).unwrap();
        assert!(!recomputed.cached);
    }

    #[test]
    fn invalid_did_returns_err() {
        let db = make_test_storage();
        assert!(db.get_trust_score("not-a-did", false).is_err());
    }

    #[test]
    fn duplicate_vouches_from_same_voucher_dont_inflate() {
        let db = make_test_storage();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        let single_voucher = DilithiumKeypair::generate().unwrap();
        // Three vouches from the SAME issuer for the same subject — distinct content
        // so each becomes a separate VC, but issuer_did is the same.
        for s in &["a", "b", "c"] {
            let v = make_vouch(&single_voucher, &subject_did, s);
            db.put_signed_object(&v, None).unwrap();
        }

        let score = db.get_trust_score(&subject_did, true).unwrap();
        // 3 VCs total but only 1 distinct voucher community → vouching sub-score
        // should reflect that, not the raw VC count.
        assert_eq!(score.inputs.vc_count, 3);
        assert_eq!(score.inputs.distinct_voucher_communities, 1);
    }

    #[test]
    fn total_is_clamped_to_unit_interval() {
        // Even with maximally favorable inputs, total must be in [0, 1].
        let db = make_test_storage();
        let subject_kp = DilithiumKeypair::generate().unwrap();
        let subject_did = did_for_pubkey(&subject_kp.public_key());

        // Pile up many distinct vouches.
        for _ in 0..50 {
            let v_kp = DilithiumKeypair::generate().unwrap();
            let v = make_vouch(&v_kp, &subject_did, &format!("{}", super::super::now_millis()));
            db.put_signed_object(&v, None).unwrap();
        }

        let score = db.get_trust_score(&subject_did, true).unwrap();
        assert!(score.total >= 0.0 && score.total <= 1.0);
    }
}

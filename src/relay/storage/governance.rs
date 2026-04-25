//! Governance: proposals + votes (Phase 5 PR 1).
//!
//! A proposal is a signed_object of type `proposal_v1`. A vote is a signed_object
//! of type `vote_v1` referencing the proposal's object_id and carrying the voter's
//! choice plus their trust score weight at vote time (snapshotted so audit replays
//! are deterministic).
//!
//! Each server runs its own proposal space by default (local scope). Civilization-
//! scope proposals will federate in Phase 3 — for now they're stored locally but
//! marked appropriately.
//!
//! Vote weight is the voter's trust score at vote time, capped at 0.95
//! (power-asymmetry mitigation: no single high-trust user can dominate a vote).

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Storage;
use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::object::Object;

/// Maximum vote weight for any single voter. Accord power-asymmetry mitigation.
pub const MAX_VOTE_WEIGHT: f64 = 0.95;

/// One row of the proposals fast-lookup index.
#[derive(Debug, Clone, Serialize)]
pub struct ProposalIndex {
    pub proposal_object_id: String,
    pub proposer_did: String,
    pub proposal_type: String,
    pub scope: String, // "local" | "civilization"
    pub space_id: Option<String>,
    pub opens_at: i64,
    pub closes_at: i64,
    pub created_at: i64,
}

/// Vote tally for a proposal.
#[derive(Debug, Clone, Serialize)]
pub struct ProposalTally {
    pub proposal_object_id: String,
    pub yes_weight: f64,
    pub no_weight: f64,
    pub abstain_weight: f64,
    pub total_weight: f64,
    pub vote_count: u64,
}

/// Read a CBOR text field from a payload.
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

/// Read a CBOR integer field from a payload.
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

impl Storage {
    /// Index a proposal_v1 object after it has been stored. Idempotent.
    /// Returns Ok(true) if a new proposal row was inserted.
    pub fn index_proposal(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "proposal_v1" {
            return Ok(false);
        }
        let proposer_did = did_for_pubkey(&object.author_public_key);
        let proposal_type = match read_text(object, "proposal_type") {
            Some(t) => t,
            None => return Ok(false),
        };
        let scope = read_text(object, "scope").unwrap_or_else(|| "local".to_string());
        let opens_at = read_int(object, "opens_at").unwrap_or_else(|| super::now_millis() as i64);
        let closes_at = read_int(object, "closes_at").unwrap_or(opens_at + 604_800_000); // 7d default

        let proposal_object_id = object
            .object_id()
            .map(|h| h.to_hex())
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )))?;
        let created_at = super::now_millis() as i64;

        self.with_conn(|conn| {
            let rows = conn.execute(
                "INSERT OR IGNORE INTO proposals
                    (proposal_object_id, proposer_did, proposal_type, scope, space_id,
                     opens_at, closes_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    proposal_object_id,
                    proposer_did,
                    proposal_type,
                    scope,
                    object.space_id,
                    opens_at,
                    closes_at,
                    created_at,
                ],
            )?;
            Ok(rows > 0)
        })
    }

    /// Index a vote_v1 object after it has been stored. Idempotent.
    /// Returns Ok(true) if a new vote row was inserted.
    ///
    /// **AI agents are silently excluded from governance voting** (Accord — votes
    /// require sentient consent; AI authority must not exceed humans). The vote
    /// signed_object is still stored as audit trail, but no row hits the votes
    /// table, so it doesn't affect the tally.
    pub fn index_vote(&self, object: &Object) -> Result<bool, rusqlite::Error> {
        if object.object_type != "vote_v1" {
            return Ok(false);
        }
        let voter_did = did_for_pubkey(&object.author_public_key);
        if self.is_ai_agent(&voter_did) {
            return Ok(false);
        }
        let proposal_object_id = match object.references.first() {
            Some(id) => id.clone(),
            None => return Ok(false),
        };
        let choice = read_text(object, "choice").unwrap_or_else(|| "abstain".to_string());

        // Snapshot the voter's trust score at vote time (capped at MAX_VOTE_WEIGHT).
        let raw_weight = self
            .get_trust_score(&voter_did, false)
            .map(|s| s.total)
            .unwrap_or(0.0);
        let weight = raw_weight.min(MAX_VOTE_WEIGHT).max(0.0);

        let vote_object_id = object
            .object_id()
            .map(|h| h.to_hex())
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )))?;
        let cast_at = super::now_millis() as i64;

        self.with_conn(|conn| {
            // One vote per (proposal, voter); subsequent votes by the same voter are
            // ignored. Future revision (Phase 5.1) may allow vote changes before close.
            let rows = conn.execute(
                "INSERT OR IGNORE INTO votes
                    (vote_object_id, proposal_object_id, voter_did, choice, weight_at_vote, cast_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    vote_object_id,
                    proposal_object_id,
                    voter_did,
                    choice,
                    weight,
                    cast_at,
                ],
            )?;
            Ok(rows > 0)
        })
    }

    /// Compute a deterministic tally of a proposal from all its votes.
    pub fn tally_proposal(
        &self,
        proposal_object_id: &str,
    ) -> Result<ProposalTally, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut yes = 0.0f64;
            let mut no = 0.0f64;
            let mut abstain = 0.0f64;
            let mut count = 0u64;

            let mut stmt = conn.prepare(
                "SELECT choice, weight_at_vote FROM votes WHERE proposal_object_id = ?1",
            )?;
            let rows = stmt.query_map(params![proposal_object_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?;
            for row in rows {
                let (choice, weight) = row?;
                match choice.as_str() {
                    "yes" => yes += weight,
                    "no" => no += weight,
                    _ => abstain += weight,
                }
                count += 1;
            }

            Ok(ProposalTally {
                proposal_object_id: proposal_object_id.to_string(),
                yes_weight: yes,
                no_weight: no,
                abstain_weight: abstain,
                total_weight: yes + no + abstain,
                vote_count: count,
            })
        })
    }

    /// Get a single proposal by object id.
    pub fn get_proposal(
        &self,
        proposal_object_id: &str,
    ) -> Result<Option<ProposalIndex>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT proposal_object_id, proposer_did, proposal_type, scope,
                        space_id, opens_at, closes_at, created_at
                 FROM proposals WHERE proposal_object_id = ?1",
                params![proposal_object_id],
                |row| {
                    Ok(ProposalIndex {
                        proposal_object_id: row.get(0)?,
                        proposer_did: row.get(1)?,
                        proposal_type: row.get(2)?,
                        scope: row.get(3)?,
                        space_id: row.get(4)?,
                        opens_at: row.get(5)?,
                        closes_at: row.get(6)?,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()
        })
    }

    /// List proposals by filter.
    pub fn list_proposals(
        &self,
        scope: Option<&str>,
        proposal_type: Option<&str>,
        space_id: Option<&str>,
        only_open_at: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<ProposalIndex>, rusqlite::Error> {
        let limit = limit.unwrap_or(100).min(1000);

        let mut sql = String::from(
            "SELECT proposal_object_id, proposer_did, proposal_type, scope,
                    space_id, opens_at, closes_at, created_at
             FROM proposals WHERE 1=1",
        );
        let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(s) = scope {
            sql.push_str(" AND scope = ?");
            binds.push(Box::new(s.to_string()));
        }
        if let Some(t) = proposal_type {
            sql.push_str(" AND proposal_type = ?");
            binds.push(Box::new(t.to_string()));
        }
        if let Some(s) = space_id {
            sql.push_str(" AND space_id = ?");
            binds.push(Box::new(s.to_string()));
        }
        if let Some(now) = only_open_at {
            sql.push_str(" AND opens_at <= ? AND closes_at > ?");
            binds.push(Box::new(now));
            binds.push(Box::new(now));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT ?");
        binds.push(Box::new(limit as i64));

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
            let rows = stmt
                .query_map(rusqlite::params_from_iter(param_refs), |row| {
                    Ok(ProposalIndex {
                        proposal_object_id: row.get(0)?,
                        proposer_did: row.get(1)?,
                        proposal_type: row.get(2)?,
                        scope: row.get(3)?,
                        space_id: row.get(4)?,
                        opens_at: row.get(5)?,
                        closes_at: row.get(6)?,
                        created_at: row.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
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
        let path = std::env::temp_dir().join(format!("hum_gov_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    fn make_proposal(
        proposer: &DilithiumKeypair,
        proposal_type: &str,
        scope: &str,
    ) -> Object {
        let now: u64 = super::super::now_millis();
        let payload = cbor_map(vec![
            ("proposal_type", cbor_text(proposal_type)),
            ("scope", cbor_text(scope)),
            ("title", cbor_text("Test proposal")),
            ("body", cbor_text("Body text")),
            ("opens_at", cbor_int(now)),
            ("closes_at", cbor_int(now + 86_400_000)),
        ]);
        ObjectBuilder::new("proposal_v1")
            .space_id("test_server")
            .created_at(now)
            .payload_cbor(&payload)
            .unwrap()
            .sign(proposer)
            .unwrap()
    }

    fn make_vote(voter: &DilithiumKeypair, proposal_id: &str, choice: &str) -> Object {
        let payload = cbor_map(vec![
            ("choice", cbor_text(choice)),
        ]);
        ObjectBuilder::new("vote_v1")
            .reference(proposal_id)
            .created_at(super::super::now_millis())
            .payload_cbor(&payload)
            .unwrap()
            .sign(voter)
            .unwrap()
    }

    #[test]
    fn proposal_indexes_after_storage() {
        let db = make_test_storage();
        let proposer = DilithiumKeypair::generate().unwrap();
        let p = make_proposal(&proposer, "parameter_change", "local");
        db.put_signed_object(&p, None).unwrap();

        let id = p.object_id().unwrap().to_hex();
        let got = db.get_proposal(&id).unwrap().expect("found");
        assert_eq!(got.proposal_type, "parameter_change");
        assert_eq!(got.scope, "local");
    }

    #[test]
    fn votes_tally_correctly() {
        let db = make_test_storage();
        let proposer = DilithiumKeypair::generate().unwrap();
        let p = make_proposal(&proposer, "parameter_change", "local");
        db.put_signed_object(&p, None).unwrap();
        let p_id = p.object_id().unwrap().to_hex();

        // Three voters; their trust scores will be 0 (no prior history) so the
        // tally just counts vote_count, weights are 0.
        for choice in &["yes", "yes", "no"] {
            let voter = DilithiumKeypair::generate().unwrap();
            let v = make_vote(&voter, &p_id, choice);
            db.put_signed_object(&v, None).unwrap();
        }

        let tally = db.tally_proposal(&p_id).unwrap();
        assert_eq!(tally.vote_count, 3);
    }

    #[test]
    fn duplicate_vote_from_same_voter_is_ignored() {
        // Two votes from the SAME voter for the same proposal — only the first
        // is indexed (one vote per voter).
        let db = make_test_storage();
        let proposer = DilithiumKeypair::generate().unwrap();
        let p = make_proposal(&proposer, "parameter_change", "local");
        db.put_signed_object(&p, None).unwrap();
        let p_id = p.object_id().unwrap().to_hex();

        let voter = DilithiumKeypair::generate().unwrap();
        // Make two votes with different choices but same voter+proposal — different
        // canonical bytes (different `choice` payload) so they pass the signed_objects
        // dedupe and both reach index_vote. The vote_index dedupe (one per voter+proposal)
        // ensures only one is recorded.
        let v1 = make_vote(&voter, &p_id, "yes");
        let v2 = make_vote(&voter, &p_id, "no");
        db.put_signed_object(&v1, None).unwrap();
        db.put_signed_object(&v2, None).unwrap();

        let tally = db.tally_proposal(&p_id).unwrap();
        assert_eq!(tally.vote_count, 1, "second vote from same voter must be ignored");
    }

    #[test]
    fn ai_agent_vote_is_excluded() {
        let db = make_test_storage();
        let proposer = DilithiumKeypair::generate().unwrap();
        let p = make_proposal(&proposer, "parameter_change", "local");
        db.put_signed_object(&p, None).unwrap();
        let p_id = p.object_id().unwrap().to_hex();

        // Declare voter as ai_agent
        let ai_voter = DilithiumKeypair::generate().unwrap();
        let class_decl = ObjectBuilder::new("subject_class_v1")
            .created_at(1)
            .payload_cbor(&cbor_map(vec![("class", cbor_text("ai_agent"))]))
            .unwrap()
            .sign(&ai_voter)
            .unwrap();
        db.put_signed_object(&class_decl, None).unwrap();

        // AI tries to vote
        let v = make_vote(&ai_voter, &p_id, "yes");
        db.put_signed_object(&v, None).unwrap();

        let tally = db.tally_proposal(&p_id).unwrap();
        assert_eq!(tally.vote_count, 0, "AI vote must not count");
    }

    #[test]
    fn list_filters_by_scope() {
        let db = make_test_storage();
        let proposer = DilithiumKeypair::generate().unwrap();

        for (kind, scope) in &[
            ("parameter_change", "local"),
            ("local_rule", "local"),
            ("accord_amendment", "civilization"),
        ] {
            let p = make_proposal(&proposer, kind, scope);
            db.put_signed_object(&p, None).unwrap();
        }

        let local = db.list_proposals(Some("local"), None, None, None, None).unwrap();
        assert_eq!(local.len(), 2);

        let civ = db.list_proposals(Some("civilization"), None, None, None, None).unwrap();
        assert_eq!(civ.len(), 1);
        assert_eq!(civ[0].proposal_type, "accord_amendment");
    }
}

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

        // Scheduled re-votes (v0.1302). All three are optional: a proposal that
        // names none of them behaves exactly as proposals did before.
        //
        // `review_cadence_ms` without an explicit `review_after` implies the
        // first review one cadence after the vote closes, which is what a
        // proposer means by "revisit this every year" and saves them computing
        // a timestamp by hand.
        let review_cadence_ms = read_int(object, "review_cadence_ms").filter(|ms| *ms > 0);
        let review_after = read_int(object, "review_after")
            .or_else(|| review_cadence_ms.map(|ms| closes_at.saturating_add(ms)));
        // The proposal this one replaces. Not verified here: the chain is
        // resolved at read time by `standing_decision`, which walks only rows
        // that actually exist, so a dangling id degrades to a chain root
        // rather than corrupting anything.
        let supersedes = read_text(object, "supersedes").filter(|s| !s.is_empty());

        self.with_conn(|conn| {
            let rows = conn.execute(
                "INSERT OR IGNORE INTO proposals
                    (proposal_object_id, proposer_did, proposal_type, scope, space_id,
                     opens_at, closes_at, created_at,
                     review_after, review_cadence_ms, supersedes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    proposal_object_id,
                    proposer_did,
                    proposal_type,
                    scope,
                    object.space_id,
                    opens_at,
                    closes_at,
                    created_at,
                    review_after,
                    review_cadence_ms,
                    supersedes,
                ],
            )?;
            Ok(rows > 0)
        })
    }

    /// Walk a supersession chain to the decision that currently stands.
    ///
    /// A vote is never mutated and a proposal is never edited. When a question
    /// is revisited, a NEW proposal is opened carrying `supersedes` pointing at
    /// the old one, so the whole history stays readable: what was decided, when,
    /// and how many times it has been reaffirmed. This resolves "what is the
    /// answer right now" without losing any of that.
    ///
    /// Given any id in a chain, returns the newest proposal reachable by
    /// following `supersedes` links forward. Returns the input id unchanged if
    /// nothing supersedes it, and `None` if the id names no proposal at all.
    ///
    /// Cycle-safe. `supersedes` arrives inside a signed object from an
    /// untrusted author, so A-supersedes-B-supersedes-A is a thing a hostile
    /// or buggy client can submit; the visited set means that costs a bounded
    /// walk rather than hanging the request.
    pub fn standing_decision(&self, proposal_object_id: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM proposals WHERE proposal_object_id = ?1",
                params![proposal_object_id],
                |r| r.get(0),
            )?;
            if exists == 0 {
                return Ok(None);
            }
            let mut current = proposal_object_id.to_string();
            let mut visited = std::collections::HashSet::new();
            visited.insert(current.clone());
            loop {
                let next: Option<String> = conn
                    .query_row(
                        "SELECT proposal_object_id FROM proposals
                          WHERE supersedes = ?1
                          ORDER BY created_at DESC, proposal_object_id ASC
                          LIMIT 1",
                        params![current],
                        |r| r.get(0),
                    )
                    .optional()?;
                match next {
                    Some(n) if visited.insert(n.clone()) => current = n,
                    // Either nothing supersedes it, or we have looped.
                    _ => return Ok(Some(current)),
                }
            }
        })
    }

    /// Proposals whose scheduled review date has arrived and that nothing has
    /// superseded yet. This is what a scheduler asks for when it opens the
    /// successor proposal.
    ///
    /// The `NOT EXISTS` clause is what makes calling this repeatedly safe: once
    /// a successor exists, the old proposal stops being returned, so a
    /// scheduler that runs twice does not open two successors.
    pub fn proposals_due_for_review(&self, now_ms: i64) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT p.proposal_object_id FROM proposals p
                  WHERE p.review_after IS NOT NULL
                    AND p.review_after <= ?1
                    AND p.closes_at <= ?1
                    AND NOT EXISTS (
                        SELECT 1 FROM proposals s WHERE s.supersedes = p.proposal_object_id
                    )
                  ORDER BY p.review_after ASC",
            )?;
            let rows = stmt.query_map(params![now_ms], |r| r.get::<_, String>(0))?;
            rows.collect()
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

    // ── Scheduled re-votes (v0.1302) ──────────────────────────────────────
    //
    // The operator's design: never mutate a vote, SUPERSEDE a decision. A cast
    // vote stays immutable and final; revisiting a question opens a NEW
    // proposal linked to the old one, and the standing answer is the newest
    // link in the chain. The whole chain stays visible, which is the point:
    // seeing that a rule was reaffirmed three times is itself information.

    /// A proposal carrying extra payload keys on top of the standard shape.
    fn make_proposal_with(
        proposer: &DilithiumKeypair,
        extra: Vec<(&str, ciborium::Value)>,
    ) -> Object {
        let now: u64 = super::super::now_millis();
        let mut fields: Vec<(&str, ciborium::Value)> = vec![
            ("proposal_type", cbor_text("parameter_change")),
            ("scope", cbor_text("local")),
            ("title", cbor_text("Test proposal")),
            ("body", cbor_text("Body text")),
            ("opens_at", cbor_int(now)),
            ("closes_at", cbor_int(now + 86_400_000)),
        ];
        fields.extend(extra);
        ObjectBuilder::new("proposal_v1")
            .space_id("test_server")
            .created_at(now)
            .payload_cbor(&cbor_map(fields))
            .unwrap()
            .sign(proposer)
            .unwrap()
    }

    fn put(db: &Storage, o: &Object) -> String {
        db.put_signed_object(o, None).unwrap();
        o.object_id().unwrap().to_hex()
    }

    /// The core of the feature: given any link in a chain, the standing answer
    /// is the newest one, and every earlier link still resolves to it.
    #[test]
    fn standing_decision_walks_the_supersession_chain() {
        let db = make_test_storage();
        let k = DilithiumKeypair::generate().unwrap();

        let a = put(&db, &make_proposal_with(&k, vec![]));
        let b = put(&db, &make_proposal_with(&k, vec![("supersedes", cbor_text(&a))]));
        let c = put(&db, &make_proposal_with(&k, vec![("supersedes", cbor_text(&b))]));

        // Asking about ANY link gives the same answer.
        assert_eq!(db.standing_decision(&a).unwrap().as_deref(), Some(c.as_str()));
        assert_eq!(db.standing_decision(&b).unwrap().as_deref(), Some(c.as_str()));
        assert_eq!(db.standing_decision(&c).unwrap().as_deref(), Some(c.as_str()));

        // And the history is still there, which is the whole reason for
        // superseding rather than editing.
        assert!(db.get_proposal(&a).unwrap().is_some(), "the superseded decision must survive");
        assert!(db.get_proposal(&b).unwrap().is_some());
    }

    /// `supersedes` arrives inside a signed object from an untrusted author, so
    /// a cycle is something a hostile or buggy client can submit. It must cost
    /// a bounded walk, not a hung request.
    #[test]
    fn standing_decision_survives_a_supersession_cycle() {
        let db = make_test_storage();
        let k = DilithiumKeypair::generate().unwrap();

        let a = put(&db, &make_proposal_with(&k, vec![]));
        let b = put(&db, &make_proposal_with(&k, vec![("supersedes", cbor_text(&a))]));
        // Close the loop by hand: no honest client would, which is the point.
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE proposals SET supersedes = ?1 WHERE proposal_object_id = ?2",
                params![&b, &a],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .unwrap();

        // Terminates, and returns a real member of the cycle.
        let standing = db.standing_decision(&a).unwrap().expect("some answer");
        assert!(standing == a || standing == b, "got {standing}");
    }

    #[test]
    fn standing_decision_is_none_for_an_unknown_proposal() {
        let db = make_test_storage();
        assert!(db.standing_decision("no_such_proposal").unwrap().is_none());
    }

    /// "Revisit this every year" should not require the proposer to compute a
    /// timestamp: a cadence with no explicit date means one cadence after the
    /// vote closes.
    #[test]
    fn review_cadence_implies_a_first_review_after_close() {
        let db = make_test_storage();
        let k = DilithiumKeypair::generate().unwrap();
        let year_ms: u64 = 365 * 24 * 60 * 60 * 1000;

        let id = put(&db, &make_proposal_with(&k, vec![("review_cadence_ms", cbor_int(year_ms))]));

        let (review_after, closes_at): (Option<i64>, i64) = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT review_after, closes_at FROM proposals WHERE proposal_object_id = ?1",
                    params![&id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            })
            .unwrap();
        assert_eq!(
            review_after,
            Some(closes_at + year_ms as i64),
            "a cadence with no explicit date means one cadence after close"
        );
    }

    /// The idempotence guard. A scheduler runs on a timer and WILL run twice;
    /// once a successor exists the old proposal must stop coming back, or every
    /// tick opens another duplicate re-vote.
    #[test]
    fn proposals_due_for_review_stop_once_superseded() {
        let db = make_test_storage();
        let k = DilithiumKeypair::generate().unwrap();

        // Due in the past, and its vote has closed.
        let a = put(&db, &make_proposal_with(&k, vec![("review_after", cbor_int(1))]));
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE proposals SET closes_at = 1 WHERE proposal_object_id = ?1",
                params![&a],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .unwrap();

        let now = 1_000_000_000_i64;
        assert_eq!(db.proposals_due_for_review(now).unwrap(), vec![a.clone()]);

        // The scheduler opens the successor...
        let _b = put(&db, &make_proposal_with(&k, vec![("supersedes", cbor_text(&a))]));

        // ...and the next tick must not open a second one.
        assert!(
            db.proposals_due_for_review(now).unwrap().is_empty(),
            "a superseded proposal must not come due again"
        );
    }

    /// A proposal that names no review date is untouched by any of this, which
    /// is what makes the whole feature additive.
    #[test]
    fn a_proposal_without_a_review_date_is_never_due() {
        let db = make_test_storage();
        let k = DilithiumKeypair::generate().unwrap();
        let _a = put(&db, &make_proposal_with(&k, vec![]));
        assert!(db.proposals_due_for_review(i64::MAX).unwrap().is_empty());
    }

    /// BUG-046 guard. The live relay's proposals table predates these three
    /// columns, and an index over an ALTER-added column placed in the main
    /// schema batch passes every fresh-DB test and aborts startup on the real
    /// database. This opens a DB in the OLD shape, with a row in it, and
    /// requires that startup survives and the new paths work.
    #[test]
    fn opens_a_pre_revote_database_and_migrates_it() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_gov_mig_{pid}_{nanos}.db"));

        {
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "CREATE TABLE proposals (
                    proposal_object_id  TEXT PRIMARY KEY,
                    proposer_did        TEXT NOT NULL,
                    proposal_type       TEXT NOT NULL,
                    scope               TEXT NOT NULL,
                    space_id            TEXT,
                    opens_at            INTEGER NOT NULL,
                    closes_at           INTEGER NOT NULL,
                    created_at          INTEGER NOT NULL
                );
                INSERT INTO proposals
                    (proposal_object_id, proposer_did, proposal_type, scope,
                     space_id, opens_at, closes_at, created_at)
                 VALUES ('old_prop', 'did:hum:someone', 'parameter_change',
                         'local', 'test_server', 1, 2, 1);",
            )
            .expect("seed old-shape table");
        }

        // The line that would have crashed on the live DB.
        let db = Storage::open(&path).expect("startup must survive a pre-revote DB");

        // The old row survived and resolves as its own standing decision.
        assert!(db.get_proposal("old_prop").unwrap().is_some());
        assert_eq!(db.standing_decision("old_prop").unwrap().as_deref(), Some("old_prop"));

        // The new columns exist and the new paths work on the migrated table.
        let k = DilithiumKeypair::generate().unwrap();
        let b = put(&db, &make_proposal_with(&k, vec![("supersedes", cbor_text("old_prop"))]));
        assert_eq!(db.standing_decision("old_prop").unwrap().as_deref(), Some(b.as_str()));
    }
}

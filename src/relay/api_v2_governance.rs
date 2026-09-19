//! HTTP API v2: governance proposals + votes (Phase 5 PR 1).
//!
//! To **propose**: POST a `proposal_v1` signed_object via `POST /api/v2/objects`.
//! To **vote**:    POST a `vote_v1` signed_object referencing the proposal.
//!
//! Routes here:
//! - `GET /api/v2/proposals` — list with optional scope/type/space filters
//! - `GET /api/v2/proposals/{id}` — fetch a proposal
//! - `GET /api/v2/proposals/{id}/tally` — current weighted tally

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::relay::relay::RelayState;

#[derive(Debug, Deserialize)]
pub struct ListProposalsQuery {
    pub scope: Option<String>,
    pub proposal_type: Option<String>,
    pub space_id: Option<String>,
    /// If true, only proposals currently open at the given timestamp (or now).
    pub only_open: Option<bool>,
    pub limit: Option<usize>,
}

/// `GET /api/v2/proposals`
pub async fn list_proposals(
    State(state): State<Arc<RelayState>>,
    Query(q): Query<ListProposalsQuery>,
) -> impl IntoResponse {
    let only_open_at = if q.only_open.unwrap_or(false) {
        Some(crate::relay::storage::now_millis() as i64)
    } else {
        None
    };

    match state.db.list_proposals(
        q.scope.as_deref(),
        q.proposal_type.as_deref(),
        q.space_id.as_deref(),
        only_open_at,
        q.limit,
    ) {
        Ok(rows) => (
            StatusCode::OK,
            Json(serde_json::to_value(rows).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage: {e}")})),
        )
            .into_response(),
    }
}

/// `GET /api/v2/proposals/{id}`
pub async fn get_proposal(
    State(state): State<Arc<RelayState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_proposal(&id) {
        Ok(Some(p)) => (
            StatusCode::OK,
            Json(serde_json::to_value(p).unwrap_or_default()),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage: {e}")})),
        )
            .into_response(),
    }
}

/// `GET /api/v2/proposals/{id}/chain`
///
/// The supersession chain this proposal belongs to, oldest first, plus which
/// link currently stands. Scheduled re-votes never mutate a decision; they open
/// a successor, so "what is the answer right now" and "how did we get here" are
/// two different questions and this endpoint answers both at once.
///
/// A proposal that has never been revisited returns a one-element chain, which
/// is the honest answer rather than an error.
///
/// Each link also carries `question_digest`, plus `auto_opened` and
/// `auto_opened_note` when the server opened that link itself. A chain whose
/// links all share one `question_digest` is a chain where the same question was
/// asked every time, which is exactly what an automatically-opened re-vote must
/// not be able to change.
///
/// **That digest is a convenience, not the authority.** A client that does not
/// want to take this server's word for it fetches each link from
/// `GET /api/v2/objects/{id}`, verifies the Dilithium signature itself, strips
/// the scheduling and machine fields listed in
/// `governance_revotes::SCHEDULE_AND_MACHINE_FIELDS`, re-encodes the rest as
/// canonical CBOR and compares the bytes. That is the same computation this
/// endpoint runs, over objects the server cannot forge.
pub async fn proposal_chain(
    State(state): State<Arc<RelayState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.decision_chain(&id) {
        Ok(chain) if chain.is_empty() => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Ok(chain) => {
            // NOT chain.last(). The newest link is the newest PROPOSAL, which
            // may be open, or closed and defeated. The standing answer is the
            // newest link that actually carried, which is what
            // standing_decision computes. v0.1303.0 conflated the two, so
            // merely opening a review appeared to change the answer.
            let standing = chain
                .first()
                .and_then(|p| state.db.standing_decision(&p.proposal_object_id).ok().flatten());

            // Enrich each link with what the question IS and who opened it.
            // Read straight off the stored signed object, so it reflects the
            // bytes the author signed rather than anything re-derived.
            use crate::relay::governance_revotes as revotes;
            let mut links: Vec<serde_json::Value> = Vec::with_capacity(chain.len());
            let mut digests: Vec<Option<String>> = Vec::with_capacity(chain.len());
            for p in &chain {
                let mut link = serde_json::to_value(p).unwrap_or_default();
                let mut digest = None;
                if let Ok(Some(obj)) = state.db.get_signed_object(&p.proposal_object_id) {
                    digest = revotes::question_digest(&obj.payload);
                    link["question_digest"] = serde_json::json!(digest);
                    if revotes::is_auto_opened(&obj.payload) {
                        link["auto_opened"] = serde_json::json!(true);
                        link["auto_opened_note"] =
                            serde_json::json!(revotes::auto_opened_note(&obj.payload));
                    }
                }
                digests.push(digest);
                links.push(link);
            }
            // True only when every link asks the identical question. A client
            // can show "the same question, asked N times" on the strength of
            // it, or recompute it from the signed objects to be sure. An
            // unreadable link (digest None) makes this false rather than
            // vacuously true.
            let same_question = digests
                .first()
                .map(|first| first.is_some() && digests.iter().all(|d| d == first))
                .unwrap_or(false);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "standing": standing,
                    "revisions": links.len(),
                    "same_question": same_question,
                    "chain": links,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage: {e}")})),
        )
            .into_response(),
    }
}

// ── Proposal-type rules (v0.759, ladder rung 10): vote rules are DATA ──
// data/governance/proposal_types.ron authored quorum/pass rules per proposal
// kind in Phase 5 PR 1, then nothing ever loaded it. The tally endpoint now
// reads it per request (the file header always promised hot-reload) and
// returns the verdicts both clients render.

/// One proposal-type row from data/governance/proposal_types.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct ProposalTypeDef {
    pub id: String,
    pub scope: String,
    #[serde(default)]
    pub description: String,
    /// Min fraction of the electorate that must cast a vote.
    pub quorum_fraction: f64,
    /// Min yes/(yes+no) ratio of decisive votes to pass.
    pub pass_threshold: f64,
    pub default_duration_ms: i64,
}

/// The whole rules file. Loaded per tally request: disk first (hot-editable
/// on the server), embedded copy as the fallback.
#[derive(Debug, Clone, Deserialize)]
pub struct ProposalTypeRegistry {
    pub version: u32,
    pub types: Vec<ProposalTypeDef>,
}

impl ProposalTypeRegistry {
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn load() -> Option<Self> {
        let text = std::fs::read_to_string("data/governance/proposal_types.ron")
            .ok()
            .or_else(|| {
                crate::embedded_data::get_embedded("governance/proposal_types.ron")
                    .map(|s| s.to_string())
            })?;
        Self::from_ron(&text).ok()
    }

    pub fn get(&self, id: &str) -> Option<&ProposalTypeDef> {
        self.types.iter().find(|t| t.id == id)
    }
}

/// Verdict math, pure for tests: quorum counts every CAST vote (abstain
/// included) against the electorate; the pass ratio is yes/(yes+no), so an
/// abstention helps reach quorum without counting as a no.
pub fn tally_verdict(
    rules: &ProposalTypeDef,
    vote_count: u64,
    electorate: i64,
    yes_weight: f64,
    no_weight: f64,
) -> (bool, bool) {
    let quorum_met =
        electorate > 0 && (vote_count as f64 / electorate as f64) >= rules.quorum_fraction;
    let decisive = yes_weight + no_weight;
    let passing = decisive > 0.0 && (yes_weight / decisive) >= rules.pass_threshold;
    (quorum_met, passing)
}

/// `GET /api/v2/proposals/{id}/tally`
pub async fn tally_proposal(
    State(state): State<Arc<RelayState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.tally_proposal(&id) {
        Ok(t) => {
            let mut out = serde_json::to_value(&t).unwrap_or_default();
            // Join the data-driven rules + verdicts (v0.759). Absent rules
            // (unknown type, unreadable file) leave the plain tally intact.
            if let Ok(Some(p)) = state.db.get_proposal(&id) {
                if let Some(rules) =
                    ProposalTypeRegistry::load().and_then(|r| r.get(&p.proposal_type).cloned())
                {
                    let electorate = state.db.get_member_count(None).unwrap_or(0);
                    let (quorum_met, passing) =
                        tally_verdict(&rules, t.vote_count, electorate, t.yes_weight, t.no_weight);
                    out["quorum_fraction"] = serde_json::json!(rules.quorum_fraction);
                    out["pass_threshold"] = serde_json::json!(rules.pass_threshold);
                    out["electorate"] = serde_json::json!(electorate);
                    out["quorum_met"] = serde_json::json!(quorum_met);
                    out["passing"] = serde_json::json!(passing);
                }
            }
            (StatusCode::OK, Json(out)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("tally: {e}")})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod proposal_rules_tests {
    use super::*;

    /// The SHIPPED rules file parses whole, every quorum/pass value is a
    /// sane fraction, ids are unique, and the three types the native form
    /// offers all resolve.
    #[test]
    fn shipped_proposal_types_parse_and_are_sane() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("governance")
            .join("proposal_types.ron");
        let reg =
            ProposalTypeRegistry::from_ron(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(reg.types.len() >= 5, "got {} types", reg.types.len());
        let mut seen = std::collections::HashSet::new();
        for t in &reg.types {
            assert!(seen.insert(t.id.clone()), "duplicate proposal type {}", t.id);
            assert!(
                (0.0..=1.0).contains(&t.quorum_fraction),
                "{}: quorum {} out of range",
                t.id,
                t.quorum_fraction
            );
            assert!(
                (0.0..=1.0).contains(&t.pass_threshold),
                "{}: threshold {} out of range",
                t.id,
                t.pass_threshold
            );
            assert!(t.default_duration_ms > 0, "{}: zero duration", t.id);
            assert!(
                ["local", "civilization"].contains(&t.scope.as_str()),
                "{}: unknown scope {}",
                t.id,
                t.scope
            );
        }
        for form_type in ["local_rule", "parameter_change", "accord_amendment"] {
            assert!(reg.get(form_type).is_some(), "{form_type} missing from rules");
        }
    }

    /// Verdict math: quorum counts abstains, the pass ratio does not, and a
    /// zero electorate can never reach quorum.
    #[test]
    fn tally_verdicts_follow_the_authored_rules() {
        let rules = ProposalTypeDef {
            id: "local_rule".into(),
            scope: "local".into(),
            description: String::new(),
            quorum_fraction: 0.10,
            pass_threshold: 0.66,
            default_duration_ms: 1,
        };
        // 10 members, 1 vote = exactly 10% quorum; 1 yes, 0 no = passing.
        assert_eq!(tally_verdict(&rules, 1, 10, 1.0, 0.0), (true, true));
        // Below quorum.
        assert_eq!(tally_verdict(&rules, 1, 20, 1.0, 0.0), (false, true));
        // Abstains reach quorum but do not dilute the ratio: 3 casts (2
        // abstain + 1 yes) on 10 members = quorum met, ratio 1/1 = passing.
        assert_eq!(tally_verdict(&rules, 3, 10, 1.0, 0.0), (true, true));
        // 65% yes fails a 66% threshold.
        assert_eq!(tally_verdict(&rules, 10, 10, 0.65, 0.35), (true, false));
        // Zero electorate: no quorum, ever.
        assert_eq!(tally_verdict(&rules, 5, 0, 5.0, 0.0), (false, true));
        // No decisive votes: not passing (all abstain).
        assert_eq!(tally_verdict(&rules, 2, 10, 0.0, 0.0), (true, false));
    }
}

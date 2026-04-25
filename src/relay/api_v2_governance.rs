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

/// `GET /api/v2/proposals/{id}/tally`
pub async fn tally_proposal(
    State(state): State<Arc<RelayState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.tally_proposal(&id) {
        Ok(t) => (
            StatusCode::OK,
            Json(serde_json::to_value(t).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("tally: {e}")})),
        )
            .into_response(),
    }
}

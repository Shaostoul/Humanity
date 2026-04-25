//! HTTP API v2: trust score (Phase 2 PR 1).
//!
//! Routes:
//! - `GET /api/v2/trust/{did}` — compute or fetch the trust score for a DID.
//!   Returns the total + sub-score breakdown + raw inputs (Accord transparency).
//!
//! Cached for 5 minutes per DID. To force a fresh compute, pass `?fresh=true`.

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
pub struct TrustQuery {
    pub fresh: Option<bool>,
}

/// `GET /api/v2/trust/{did}`
pub async fn get_trust_score(
    State(state): State<Arc<RelayState>>,
    Path(did): Path<String>,
    Query(q): Query<TrustQuery>,
) -> impl IntoResponse {
    let force = q.fresh.unwrap_or(false);
    match state.db.get_trust_score(&did, force) {
        Ok(score) => {
            (StatusCode::OK, Json(serde_json::to_value(score).unwrap_or_default())).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

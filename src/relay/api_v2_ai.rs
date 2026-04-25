//! HTTP API v2: AI-as-citizen status (Phase 8 PR 1).
//!
//! `GET /api/v2/ai-status/{did}` — returns the subject_class declaration and
//! operator binding (if any) for a DID. Helps clients render the persistent
//! "AI" badge required by the strategic plan.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;

use crate::relay::relay::RelayState;

/// `GET /api/v2/ai-status/{did}`
pub async fn get_ai_status(
    State(state): State<Arc<RelayState>>,
    Path(did): Path<String>,
) -> impl IntoResponse {
    match state.db.get_ai_status(&did) {
        Ok(s) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "did": s.did,
                "subject_class": s.subject_class,
                "operator_did": s.operator_did,
                "compliant": state.db.ai_has_operator(&did),
                "is_ai": s.subject_class == "ai_agent",
                "last_updated": s.last_updated,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage: {e}")})),
        )
            .into_response(),
    }
}

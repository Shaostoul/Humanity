//! HTTP API v2: social key recovery (Phase 4 PR 1).
//!
//! `GET /api/v2/recovery/setup/{holder_did}` — returns the holder's recovery
//! configuration: list of guardian DIDs and (threshold, total_shares).
//! Useful for clients showing recovery status, or guardians confirming their role.
//!
//! `GET /api/v2/recovery/shares-held-by/{guardian_did}` — list all shares this
//! guardian is holding (so they can see which holders have entrusted them).
//!
//! Setting up recovery: POST a `recovery_share_v1` per guardian via
//! `POST /api/v2/objects`. Each is encrypted client-side to the guardian's
//! Kyber768 pubkey before submission; server stores opaque ciphertext.
//!
//! Initiating recovery: post a `recovery_request_v1` (Phase 4 PR 2 spec).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;

use crate::relay::relay::RelayState;

/// `GET /api/v2/recovery/setup/{holder_did}`
pub async fn get_recovery_setup(
    State(state): State<Arc<RelayState>>,
    Path(holder_did): Path<String>,
) -> impl IntoResponse {
    match state.db.get_recovery_setup(&holder_did) {
        Ok(Some(setup)) => (
            StatusCode::OK,
            Json(serde_json::to_value(setup).unwrap_or_default()),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no recovery setup for this DID"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage: {e}")})),
        )
            .into_response(),
    }
}

/// `GET /api/v2/recovery/shares-held-by/{guardian_did}`
pub async fn get_shares_held_by(
    State(state): State<Arc<RelayState>>,
    Path(guardian_did): Path<String>,
) -> impl IntoResponse {
    match state.db.list_shares_held_by(&guardian_did) {
        Ok(shares) => (
            StatusCode::OK,
            Json(serde_json::to_value(shares).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage: {e}")})),
        )
            .into_response(),
    }
}

/// `GET /api/v2/recovery/request/{request_object_id}` — fetch a recovery request
/// with its approval count and status.
pub async fn get_recovery_request(
    State(state): State<Arc<RelayState>>,
    Path(request_object_id): Path<String>,
) -> impl IntoResponse {
    let req = match state.db.get_recovery_request(&request_object_id) {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "recovery request not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("storage: {e}")})),
            )
                .into_response();
        }
    };
    let approvals = state
        .db
        .list_recovery_approvals(&request_object_id)
        .unwrap_or_default();
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let body = serde_json::json!({
        "request_object_id": req.request_object_id,
        "holder_did": req.holder_did,
        "new_pubkey_b64": B64.encode(&req.new_pubkey),
        "threshold_required": req.threshold_required,
        "approvals_count": req.approvals_count,
        "status": req.status,
        "created_at": req.created_at,
        "approvals": approvals,
    });
    (StatusCode::OK, Json(body)).into_response()
}

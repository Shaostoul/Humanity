//! HTTP API v2: DID resolution.
//!
//! Routes:
//! - `GET /api/v2/did/{did}` — resolve a `did:hum:<base58>` to its current
//!   Dilithium3 public key + activity metadata.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::Serialize;
use std::sync::Arc;

use crate::relay::core::did::{Fingerprint, did_for_pubkey, fingerprint_to_hex};
use crate::relay::relay::RelayState;
use crate::relay::storage::DidResolution;

#[derive(Debug, Serialize)]
pub struct DidResolutionResponse {
    pub did: String,
    pub current_pubkey_b64: String,
    pub author_fp_hex: String,
    pub crypto_suite: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub object_count: u64,
}

impl From<DidResolution> for DidResolutionResponse {
    fn from(r: DidResolution) -> Self {
        Self {
            did: r.did,
            current_pubkey_b64: B64.encode(&r.current_pubkey),
            author_fp_hex: r.author_fp_hex,
            crypto_suite: "ml-dsa-65".to_string(),
            first_seen: r.first_seen,
            last_seen: r.last_seen,
            object_count: r.object_count,
        }
    }
}

/// `GET /api/v2/did/{did}`
pub async fn resolve_did(
    State(state): State<Arc<RelayState>>,
    Path(did): Path<String>,
) -> impl IntoResponse {
    match state.db.resolve_did(&did) {
        Ok(Some(res)) => (
            StatusCode::OK,
            Json(serde_json::to_value(DidResolutionResponse::from(res)).unwrap_or_default()),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "did unknown to this server (no signed objects observed)",
                "did": did,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("invalid did: {e}")})),
        )
            .into_response(),
    }
}

// Re-export for convenience: helpers callers may want.
pub use crate::relay::core::did::{did_for_pubkey as compute_did_for_pubkey};
// (the use just makes the symbol visible in `api_v2_did::compute_did_for_pubkey`)
#[allow(dead_code)]
fn _import_anchor(fp: Fingerprint) -> String {
    fingerprint_to_hex(&fp)
}

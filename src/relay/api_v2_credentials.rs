//! HTTP API v2: Verifiable Credentials.
//!
//! Routes:
//! - `GET /api/v2/credentials` — list with optional subject/issuer/schema filters
//! - `GET /api/v2/credentials/{vc_object_id}` — fetch one credential index entry
//!
//! To **issue** a VC: POST a `signed_object` of the appropriate schema (vouch_v1,
//! verified_human_v1, member_v1, etc.) via `POST /api/v2/objects`. The substrate
//! auto-indexes the credential.
//!
//! To **revoke** a VC: POST a `revocation_v1` signed_object referencing the target
//! VC's object_id. The author of the revocation must be the original issuer
//! (enforced by the substrate; non-matching author is silently ignored).
//!
//! To **withdraw** a VC (Accord consent — subject can hide a credential from
//! display without invalidating it): POST a `withdrawal_v1` signed_object
//! referencing the target VC's object_id. The author must be the subject.
//!
//! To verify a credential: fetch its signed_object via `GET /api/v2/objects/{id}`
//! and re-verify the Dilithium3 signature locally.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::relay::RelayState;
use crate::relay::storage::CredentialIndex;

#[derive(Debug, Serialize)]
pub struct CredentialResponse {
    pub vc_object_id: String,
    pub issuer_did: String,
    pub subject_did: String,
    pub schema_id: String,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub revoked: bool,
    pub revoked_by_object_id: Option<String>,
    pub withdrawn: bool,
}

impl From<CredentialIndex> for CredentialResponse {
    fn from(c: CredentialIndex) -> Self {
        Self {
            vc_object_id: c.vc_object_id,
            issuer_did: c.issuer_did,
            subject_did: c.subject_did,
            schema_id: c.schema_id,
            issued_at: c.issued_at,
            expires_at: c.expires_at,
            revoked: c.revoked_by_object_id.is_some(),
            revoked_by_object_id: c.revoked_by_object_id,
            withdrawn: c.withdrawn,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListCredentialsQuery {
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub schema: Option<String>,
    /// Default: false. Set true to include revoked credentials in the result.
    pub include_revoked: Option<bool>,
    /// Default: false. Set true to include subject-withdrawn credentials.
    pub include_withdrawn: Option<bool>,
    pub limit: Option<usize>,
}

/// `GET /api/v2/credentials`
pub async fn list_credentials(
    State(state): State<Arc<RelayState>>,
    Query(q): Query<ListCredentialsQuery>,
) -> impl IntoResponse {
    match state.db.list_credentials(
        q.subject.as_deref(),
        q.issuer.as_deref(),
        q.schema.as_deref(),
        q.include_revoked.unwrap_or(false),
        q.include_withdrawn.unwrap_or(false),
        q.limit,
    ) {
        Ok(rows) => {
            let body: Vec<CredentialResponse> =
                rows.into_iter().map(CredentialResponse::from).collect();
            (StatusCode::OK, Json(serde_json::to_value(body).unwrap_or_default())).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage: {e}")})),
        )
            .into_response(),
    }
}

/// `GET /api/v2/credentials/{vc_object_id}`
pub async fn get_credential(
    State(state): State<Arc<RelayState>>,
    Path(vc_object_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_credential(&vc_object_id) {
        Ok(Some(c)) => (
            StatusCode::OK,
            Json(serde_json::to_value(CredentialResponse::from(c)).unwrap_or_default()),
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

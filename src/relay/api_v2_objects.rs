//! HTTP API v2: signed objects substrate (Phase 0).
//!
//! Routes:
//! - `POST /api/v2/objects` — submit a signed object (validates signature + stores)
//! - `GET  /api/v2/objects/{object_id}` — fetch one object by hex id
//! - `GET  /api/v2/objects` — list objects with filters
//! - `GET  /api/v2/objects/count` — count objects (optionally filtered by type)
//!
//! Object format on the wire: JSON. The fields mirror `crate::relay::core::object::Object`,
//! but binary fields (author_public_key, payload, signature) are base64-encoded for JSON
//! transport. The server reconstructs the canonical CBOR form internally to verify the
//! signature — clients NEVER need to canonicalize CBOR themselves; they sign their canonical
//! bytes locally and submit the resulting fields.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::core::object::Object;
use crate::relay::relay::RelayState;
use crate::relay::storage::{SignedObjectRecord, validate_migration_object};

/// JSON shape submitted by clients to `POST /api/v2/objects`. Binary fields are base64.
#[derive(Debug, Deserialize)]
pub struct SignedObjectSubmission {
    pub protocol_version: u64,
    pub object_type: String,
    pub space_id: Option<String>,
    pub channel_id: Option<String>,
    /// Base64-encoded 1952-byte Dilithium3 public key.
    pub author_public_key_b64: String,
    pub created_at: Option<u64>,
    pub references: Vec<String>,
    pub payload_schema_version: u64,
    pub payload_encoding: String,
    /// Base64-encoded payload bytes.
    pub payload_b64: String,
    /// Base64-encoded 3309-byte Dilithium3 signature.
    pub signature_b64: String,
}

impl SignedObjectSubmission {
    fn to_object(&self) -> Result<Object, String> {
        let author_public_key = B64
            .decode(&self.author_public_key_b64)
            .map_err(|e| format!("author_public_key_b64 not base64: {e}"))?;
        let payload = B64
            .decode(&self.payload_b64)
            .map_err(|e| format!("payload_b64 not base64: {e}"))?;
        let signature = B64
            .decode(&self.signature_b64)
            .map_err(|e| format!("signature_b64 not base64: {e}"))?;
        Ok(Object {
            protocol_version: self.protocol_version,
            object_type: self.object_type.clone(),
            space_id: self.space_id.clone(),
            channel_id: self.channel_id.clone(),
            author_public_key,
            created_at: self.created_at,
            references: self.references.clone(),
            payload_schema_version: self.payload_schema_version,
            payload_encoding: self.payload_encoding.clone(),
            payload,
            signature,
        })
    }
}

/// JSON wire form of a `SignedObjectRecord` returned by GET endpoints.
#[derive(Debug, Serialize)]
pub struct SignedObjectResponse {
    pub object_id: String,
    pub protocol_version: u64,
    pub object_type: String,
    pub space_id: Option<String>,
    pub channel_id: Option<String>,
    pub author_fp: String,
    pub author_public_key_b64: String,
    pub created_at: Option<u64>,
    pub references: Vec<String>,
    pub payload_schema_version: u64,
    pub payload_encoding: String,
    pub payload_b64: String,
    pub signature_b64: String,
    pub source_server: Option<String>,
    pub received_at: i64,
}

impl From<SignedObjectRecord> for SignedObjectResponse {
    fn from(r: SignedObjectRecord) -> Self {
        let references: Vec<String> =
            serde_json::from_str(&r.references_json).unwrap_or_default();
        Self {
            object_id: r.object_id,
            protocol_version: r.protocol_version,
            object_type: r.object_type,
            space_id: r.space_id,
            channel_id: r.channel_id,
            author_fp: r.author_fp,
            author_public_key_b64: B64.encode(&r.author_pubkey),
            created_at: r.created_at,
            references,
            payload_schema_version: r.payload_schema_version,
            payload_encoding: r.payload_encoding,
            payload_b64: B64.encode(&r.payload),
            signature_b64: B64.encode(&r.signature),
            source_server: r.source_server,
            received_at: r.received_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ObjectAcceptResponse {
    pub object_id: String,
    pub stored: bool,
    pub message: String,
}

/// `POST /api/v2/objects`
///
/// Validates signature, computes object_id, stores. Returns the canonical object_id.
/// Idempotent: re-submitting the same object returns `stored: false`.
pub async fn post_object(
    State(state): State<Arc<RelayState>>,
    Json(payload): Json<SignedObjectSubmission>,
) -> impl IntoResponse {
    let object = match payload.to_object() {
        Ok(o) => o,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("malformed submission: {e}")
            })))
                .into_response();
        }
    };

    // Verify signature *before* storage so we can return a precise error.
    if let Err(e) = object.verify_signature() {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "error": format!("signature verification failed: {e}")
        })))
            .into_response();
    }

    let object_id = match object.object_id() {
        Ok(h) => h.to_hex(),
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("object_id computation failed: {e}")
            })))
                .into_response();
        }
    };

    // Special-case: crypto_migration_v1 needs the inner Ed25519 signature validated
    // before persistence — the outer Dilithium3 sig alone doesn't prove the legacy
    // keyholder authorized the rotation.
    if object.object_type == "crypto_migration_v1" {
        let outcome = match validate_migration_object(&object) {
            Ok(o) => o,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("migration validation failed: {e}")
                    })),
                )
                    .into_response();
            }
        };

        // Persist the signed_object first (so its id is queryable), then record the migration.
        let stored = match state.db.put_signed_object(&object, None) {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("storage error: {e}")})),
                )
                    .into_response();
            }
        };
        match state.db.record_crypto_migration(&outcome) {
            Ok(_) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "object_id": object_id,
                    "stored": stored,
                    "migration": {
                        "legacy_pubkey_hex": outcome.legacy_pubkey_hex,
                        "new_pubkey_hex": outcome.new_pubkey_hex,
                        "migration_object_id": outcome.migration_object_id,
                        "timestamp": outcome.migration_timestamp,
                    },
                    "message": "migration recorded; legacy Ed25519 key archived"
                })),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("migration record failed: {e}")})),
            )
                .into_response(),
        }
    } else {
        match state.db.put_signed_object(&object, None) {
            Ok(stored) => (
                StatusCode::OK,
                Json(serde_json::to_value(ObjectAcceptResponse {
                    object_id,
                    stored,
                    message: if stored {
                        "object stored".into()
                    } else {
                        "object already known (no-op)".into()
                    },
                }).unwrap_or_default()),
            )
                .into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("storage error: {e}")
            })))
                .into_response(),
        }
    }
}

/// `GET /api/v2/objects/{object_id}`
pub async fn get_object_by_id(
    State(state): State<Arc<RelayState>>,
    Path(object_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_signed_object(&object_id) {
        Ok(Some(rec)) => (
            StatusCode::OK,
            Json(serde_json::to_value(SignedObjectResponse::from(rec)).unwrap_or_default()),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage error: {e}")})),
        )
            .into_response(),
    }
}

/// Query params for `GET /api/v2/objects`.
#[derive(Debug, Deserialize)]
pub struct ListObjectsQuery {
    pub object_type: Option<String>,
    pub space_id: Option<String>,
    pub author_fp: Option<String>,
    pub since_received: Option<i64>,
    pub limit: Option<usize>,
}

/// `GET /api/v2/objects` — list with filters.
pub async fn list_objects(
    State(state): State<Arc<RelayState>>,
    Query(q): Query<ListObjectsQuery>,
) -> impl IntoResponse {
    match state.db.list_signed_objects(
        q.object_type.as_deref(),
        q.space_id.as_deref(),
        q.author_fp.as_deref(),
        q.since_received,
        q.limit,
    ) {
        Ok(rows) => {
            let body: Vec<SignedObjectResponse> =
                rows.into_iter().map(SignedObjectResponse::from).collect();
            (StatusCode::OK, Json(serde_json::to_value(body).unwrap_or_default())).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage error: {e}")})),
        )
            .into_response(),
    }
}

/// Query params for `GET /api/v2/objects/count`.
#[derive(Debug, Deserialize)]
pub struct CountObjectsQuery {
    pub object_type: Option<String>,
}

/// `GET /api/v2/objects/count`.
pub async fn count_objects(
    State(state): State<Arc<RelayState>>,
    Query(q): Query<CountObjectsQuery>,
) -> impl IntoResponse {
    match state.db.count_signed_objects(q.object_type.as_deref()) {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({"count": n}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage error: {e}")})),
        )
            .into_response(),
    }
}

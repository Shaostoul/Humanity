//! HTTP API v2: zero-knowledge selective-disclosure presentations (Phase 6b).
//!
//! Per the plan trade-off (decision 6 / Phase 6b): drop BBS+ (not PQ-secure due
//! to bilinear pairings) in favor of hash-based zk-STARKs that ARE post-quantum
//! secure and need no trusted setup.
//!
//! This PR ships the **API surface and schema contract**. The actual prover/
//! verifier (Plonky2 / Plonky3 / Risc Zero) is deferred — it requires picking a
//! library, designing circuits per VC schema, and characterizing proof
//! generation cost. Until then, the verify endpoint returns a structured
//! "not yet wired" response so clients can integrate now and the prover can
//! land later without breaking the wire format.
//!
//! The recommended payload shape for a `stark_presentation_v1` signed_object
//! is documented at `GET /api/v2/zk/schema`.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::relay::RelayState;

#[derive(Debug, Serialize, Deserialize)]
pub struct StarkPresentationPayload {
    /// DID of the holder presenting the proof.
    pub holder_did: String,
    /// object_id of the source VC the holder is selectively disclosing from.
    pub vc_object_id: String,
    /// Schema id of the source VC (e.g. "graduation_v1") — verifier picks the right circuit.
    pub source_schema: String,
    /// The publicly-revealed claims (a subset of the VC's full claim set).
    pub public_claims: serde_json::Value,
    /// The zk-STARK proof bytes, base64-encoded. Opaque to the substrate.
    pub proof_b64: String,
    /// Identifier of the circuit/parameter set used (so verifier knows which
    /// trusted-setup-free STARK definition to use).
    pub circuit_id: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    /// object_id of a previously-stored stark_presentation_v1 signed_object.
    pub presentation_object_id: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub presentation_object_id: String,
    pub status: String,
    pub message: String,
}

/// `GET /api/v2/zk/schema` — describes the recommended payload structure for
/// stark_presentation_v1 signed_objects. Useful for client developers building
/// presentations.
pub async fn get_zk_schema(State(_state): State<Arc<RelayState>>) -> impl IntoResponse {
    let body = serde_json::json!({
        "stark_presentation_v1": {
            "purpose": "Selectively disclose claims from a Verifiable Credential without revealing the full credential. PQ-secure (zk-STARK, no trusted setup).",
            "issuer": "Holder of the source VC",
            "subject_field": "holder_did",
            "required_payload_fields": [
                "holder_did",
                "vc_object_id",
                "source_schema",
                "public_claims",
                "proof_b64",
                "circuit_id"
            ],
            "verification": "POST /api/v2/zk/verify with the presentation_object_id",
        },
        "implementation_status": "Phase 6b — wire format and schema stable; prover/verifier (Plonky2 or similar) not yet integrated. See plan trade-off 6.",
        "supported_circuits": [],
        "next_steps": [
            "Pick a hash-based zk-STARK library (Plonky2, Plonky3, or Risc Zero)",
            "Design circuits per VC schema (graduation, age-gate, credential-ownership)",
            "Characterize proof generation cost on user devices",
            "Wire the verify_proof function below"
        ],
    });
    (StatusCode::OK, Json(body)).into_response()
}

/// `POST /api/v2/zk/verify` — verify a previously-submitted stark_presentation_v1.
///
/// Currently returns "pending" status because the verifier isn't wired. Clients
/// can already submit `stark_presentation_v1` signed_objects via
/// `POST /api/v2/objects` and refer back here when the verifier ships.
pub async fn verify_presentation(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<VerifyRequest>,
) -> impl IntoResponse {
    // Confirm the presentation object exists, even if we can't verify it yet.
    let exists = state
        .db
        .get_signed_object(&req.presentation_object_id)
        .ok()
        .flatten();

    let body = if let Some(rec) = exists {
        if rec.object_type != "stark_presentation_v1" {
            VerifyResponse {
                presentation_object_id: req.presentation_object_id,
                status: "error".to_string(),
                message: format!(
                    "object_type is {}, not stark_presentation_v1",
                    rec.object_type
                ),
            }
        } else {
            VerifyResponse {
                presentation_object_id: req.presentation_object_id,
                status: "pending".to_string(),
                message: "STARK verifier not yet integrated. The signed_object exists and its outer Dilithium3 signature was verified on storage. Proof verification awaits Phase 6b PR 2.".to_string(),
            }
        }
    } else {
        VerifyResponse {
            presentation_object_id: req.presentation_object_id,
            status: "not_found".to_string(),
            message: "no signed_object with that id".to_string(),
        }
    };
    (StatusCode::OK, Json(body)).into_response()
}

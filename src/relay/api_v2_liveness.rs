//! HTTP API v2: WebRTC session attestation + liveness signaling (Phase 6c).
//!
//! Anti-deepfake / liveness via the existing VC substrate. The relay doesn't
//! ATTEST liveness itself (that's the accredited verifier's job — Phase 6c
//! mature design); it just proxies and records the session metadata so other
//! services can build on it.
//!
//! `POST /api/v2/liveness/attest` is a thin convenience wrapper. The actual
//! liveness VC issuance happens via `POST /api/v2/objects` with
//! object_type = `attested_session_v1` (per-WebRTC session) or `liveness_v1`
//! (accredited liveness check). This endpoint exists primarily to document
//! the recommended payload shape in code.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::relay::RelayState;

/// Recommended payload shape for an `attested_session_v1` VC.
/// See `data/identity/schemas.ron` for the registered schema metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct AttestedSessionPayload {
    /// DID being attested (one party of the WebRTC session).
    pub subject_did: String,
    /// DID of the OTHER party of the session (counter-party).
    pub counterparty_did: String,
    /// BLAKE3 hash of the WebRTC session descriptor exchange — opaque receipt.
    pub session_hash_hex: String,
    /// Unix epoch ms when the session began.
    pub started_at: u64,
    /// Unix epoch ms when the session ended.
    pub ended_at: u64,
}

/// Schema documentation for clients.
///
/// `GET /api/v2/liveness/schema` — returns the recommended structure for
/// attested_session_v1 and liveness_v1 payloads. Useful so client devs can
/// verify they're constructing correct VCs without poking through the source.
pub async fn get_liveness_schema(
    State(_state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    let body = serde_json::json!({
        "attested_session_v1": {
            "purpose": "Per-WebRTC-session watermark VC, signed by both parties' servers as a deepfake-resistance measure. Hashes the session descriptor exchange.",
            "issuer": "Either party's server (the relay where the call was hosted)",
            "subject_field": "subject_did",
            "required_payload_fields": [
                "subject_did", "counterparty_did", "session_hash_hex",
                "started_at", "ended_at"
            ],
            "issued_via": "POST /api/v2/objects with object_type = attested_session_v1",
        },
        "liveness_v1": {
            "purpose": "Accredited liveness attestation — a third party verifier checked the subject was a live human (or an AI agent transparently disclosed) at a specific moment.",
            "issuer": "Accredited verifier server (Phase 6c v2 — for now any server may issue)",
            "subject_field": "subject_did",
            "required_payload_fields": [
                "subject_did", "checked_at", "method"
            ],
            "method_values": ["video_proof_of_personhood", "in_person_meet", "trusted_attestor"],
            "issued_via": "POST /api/v2/objects with object_type = liveness_v1",
        },
        "notes": [
            "Both schemas are auto-indexed as VCs by the substrate.",
            "Subject can withdraw their own attestation via withdrawal_v1 (Accord consent).",
            "Issuer can revoke via revocation_v1 if the underlying check turns out to be fraudulent.",
            "Trust score weights for these VCs are tunable in data/identity/trust_weights.ron."
        ],
    });
    (StatusCode::OK, Json(body)).into_response()
}

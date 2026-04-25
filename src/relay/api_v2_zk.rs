//! HTTP API v2: zero-knowledge selective-disclosure presentations (Phase 6b).
//!
//! Per the plan trade-off (decision 6 / Phase 6b): drop BBS+ (not PQ-secure due
//! to bilinear pairings) in favor of hash-based proofs that ARE post-quantum
//! secure and need no trusted setup.
//!
//! Two presentation types supported:
//!
//! 1. **`merkle_disclosure_v1`** — Merkle authentication paths over BLAKE3.
//!    Issuer hashes claim fields into a tree, embeds root in the VC. Holder
//!    later proves "this leaf is in this VC" without revealing other leaves.
//!    PQ-secure, no trusted setup, real verification implemented in
//!    `crate::relay::core::merkle_disclosure`. **WIRED** in
//!    `verify_merkle_disclosure` below.
//!
//! 2. **`stark_presentation_v1`** — full zk-STARK proofs with custom AIR per
//!    VC schema (Plonky2 / Plonky3 / Risc Zero). API surface is stable; the
//!    verifier integration ships in a follow-up once specific circuit designs
//!    are picked.
//!
//! The recommended payload shapes are documented at `GET /api/v2/zk/schema`.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ciborium::Value;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::core::encoding::from_canonical_bytes;
use crate::relay::core::merkle_disclosure::{HASH_LEN, PathStep, verify_path};
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

/// `GET /api/v2/zk/schema` — describes the recommended payload structures for
/// the supported selective-disclosure presentation types.
pub async fn get_zk_schema(State(_state): State<Arc<RelayState>>) -> impl IntoResponse {
    let body = serde_json::json!({
        "merkle_disclosure_v1": {
            "purpose": "Selectively disclose specific claim fields from a VC by presenting a BLAKE3 Merkle authentication path. PQ-secure, no trusted setup.",
            "issuer": "Holder of the source VC",
            "subject_field": "holder_did",
            "status": "wired",
            "required_payload_fields": [
                "holder_did",
                "vc_object_id",
                "vc_root_b64",            // 32-byte BLAKE3 root from the source VC
                "field_name",             // e.g. \"degree\", \"age_bucket\"
                "field_value_b64",        // canonical CBOR bytes of the disclosed value
                "path_b64"                // serialized Vec<PathStep>
            ],
            "leaf_hash_rule": "BLAKE3(field_name || 0x00 || canonical_cbor(field_value))",
            "path_format": "concat( for each step: 32-byte sibling || 1 byte (1=is_left, 0=is_right) ) — total 33 bytes per step",
            "verification": "POST /api/v2/zk/verify with the presentation_object_id",
        },
        "stark_presentation_v1": {
            "purpose": "Arbitrary predicate proofs (\"age >= 18\", \"holds active credential of class X\") via zk-STARKs.",
            "issuer": "Holder of the source VC",
            "subject_field": "holder_did",
            "status": "scaffold — verifier integration follows merkle_disclosure_v1",
            "required_payload_fields": [
                "holder_did",
                "vc_object_id",
                "source_schema",
                "public_claims",
                "proof_b64",
                "circuit_id"
            ],
            "verification": "POST /api/v2/zk/verify with the presentation_object_id (returns pending until circuits are wired)",
        },
        "implementation_status": "Phase 6b: merkle_disclosure_v1 fully wired; stark_presentation_v1 awaits circuit design.",
    });
    (StatusCode::OK, Json(body)).into_response()
}

/// Decode a path-bytes blob `concat(32-byte sibling || 1-byte direction)*N` into
/// a Vec<PathStep>.
fn decode_path(path_bytes: &[u8]) -> Option<Vec<PathStep>> {
    const STEP_LEN: usize = HASH_LEN + 1;
    if path_bytes.len() % STEP_LEN != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(path_bytes.len() / STEP_LEN);
    for chunk in path_bytes.chunks(STEP_LEN) {
        let mut sibling = [0u8; HASH_LEN];
        sibling.copy_from_slice(&chunk[..HASH_LEN]);
        let is_left = chunk[HASH_LEN] != 0;
        out.push(PathStep { sibling, is_left });
    }
    Some(out)
}

/// Read a CBOR text/bytes payload field by name.
fn cbor_get<'a>(map: &'a [(Value, Value)], field: &str) -> Option<&'a Value> {
    for (k, v) in map {
        if let Value::Text(name) = k {
            if name == field {
                return Some(v);
            }
        }
    }
    None
}

/// Verify a `merkle_disclosure_v1` payload against its claimed source VC.
/// Returns Ok with a status string, or Err with reason.
fn verify_merkle_disclosure(
    state: &Arc<RelayState>,
    payload: &[u8],
) -> std::result::Result<String, String> {
    let value = from_canonical_bytes(payload).map_err(|e| format!("payload not canonical CBOR: {e}"))?;
    let map = match value {
        Value::Map(m) => m,
        _ => return Err("payload must be CBOR map".into()),
    };

    let vc_object_id = match cbor_get(&map, "vc_object_id") {
        Some(Value::Text(s)) => s.clone(),
        _ => return Err("missing vc_object_id".into()),
    };
    let field_name = match cbor_get(&map, "field_name") {
        Some(Value::Text(s)) => s.clone(),
        _ => return Err("missing field_name".into()),
    };
    let field_value_cbor = match cbor_get(&map, "field_value") {
        Some(Value::Bytes(b)) => b.clone(),
        _ => return Err("missing field_value (must be bytes)".into()),
    };
    let path_bytes = match cbor_get(&map, "path") {
        Some(Value::Bytes(b)) => b.clone(),
        _ => return Err("missing path (must be bytes)".into()),
    };
    let claimed_root = match cbor_get(&map, "vc_root") {
        Some(Value::Bytes(b)) => {
            if b.len() != HASH_LEN {
                return Err("vc_root must be 32 bytes".into());
            }
            let mut arr = [0u8; HASH_LEN];
            arr.copy_from_slice(b);
            arr
        }
        _ => return Err("missing vc_root".into()),
    };

    // The cited VC must exist and embed a vc_root field equal to claimed_root.
    let vc = state
        .db
        .get_signed_object(&vc_object_id)
        .ok()
        .flatten()
        .ok_or_else(|| format!("source VC {vc_object_id} not found"))?;
    // Decode VC payload to read its declared vc_root
    let vc_payload_value = from_canonical_bytes(&vc.payload)
        .map_err(|e| format!("VC payload not canonical CBOR: {e}"))?;
    let vc_map = match vc_payload_value {
        Value::Map(m) => m,
        _ => return Err("VC payload must be CBOR map".into()),
    };
    let actual_root = match cbor_get(&vc_map, "vc_root") {
        Some(Value::Bytes(b)) if b.len() == HASH_LEN => {
            let mut arr = [0u8; HASH_LEN];
            arr.copy_from_slice(b);
            arr
        }
        _ => return Err("source VC has no vc_root commitment field".into()),
    };
    if claimed_root != actual_root {
        return Err("claimed vc_root does not match source VC's commitment".into());
    }

    let path = decode_path(&path_bytes).ok_or_else(|| "malformed path bytes".to_string())?;
    let leaf = crate::relay::core::merkle_disclosure::leaf_hash(&field_name, &field_value_cbor);

    if verify_path(&leaf, &path, &claimed_root) {
        Ok(format!(
            "verified: field '{field_name}' is included in VC {vc_object_id}"
        ))
    } else {
        Err("Merkle path does not verify against root".into())
    }
}

/// `POST /api/v2/zk/verify` — verify a previously-submitted disclosure
/// presentation. Dispatches by object_type:
///
///   - `merkle_disclosure_v1` → real Merkle path verification
///   - `stark_presentation_v1` → returns "pending" until circuits are wired
pub async fn verify_presentation(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<VerifyRequest>,
) -> impl IntoResponse {
    let exists = state
        .db
        .get_signed_object(&req.presentation_object_id)
        .ok()
        .flatten();

    let body = if let Some(rec) = exists {
        match rec.object_type.as_str() {
            "merkle_disclosure_v1" => {
                let object = rec.to_object();
                match verify_merkle_disclosure(&state, &object.payload) {
                    Ok(msg) => VerifyResponse {
                        presentation_object_id: req.presentation_object_id,
                        status: "verified".to_string(),
                        message: msg,
                    },
                    Err(reason) => VerifyResponse {
                        presentation_object_id: req.presentation_object_id,
                        status: "invalid".to_string(),
                        message: reason,
                    },
                }
            }
            "stark_presentation_v1" => VerifyResponse {
                presentation_object_id: req.presentation_object_id,
                status: "pending".to_string(),
                message: "STARK verifier scaffold — circuit design follows merkle_disclosure_v1. Outer Dilithium3 sig was verified on storage.".to_string(),
            },
            other => VerifyResponse {
                presentation_object_id: req.presentation_object_id,
                status: "error".to_string(),
                message: format!(
                    "object_type is {other}, expected merkle_disclosure_v1 or stark_presentation_v1"
                ),
            },
        }
    } else {
        VerifyResponse {
            presentation_object_id: req.presentation_object_id,
            status: "not_found".to_string(),
            message: "no signed_object with that id".to_string(),
        }
    };
    let _ = B64.encode(b""); // silence unused-import lint when build configs vary
    (StatusCode::OK, Json(body)).into_response()
}

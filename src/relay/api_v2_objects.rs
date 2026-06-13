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
use crate::relay::storage::SignedObjectRecord;

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

/// Outcome of the off-executor ingest closure (see [`post_object`]).
///
/// `put_signed_object` runs the (CPU-bound) ML-DSA-65 verify AND the SQLite
/// write, both inside `spawn_blocking`, then returns a `rusqlite::Error` on
/// any failure. We classify that error here so the async handler can map it
/// back to the SAME HTTP status a client saw before this offload existed:
/// a bad signature was a 401, a malformed key a 400, anything else a 500.
enum IngestError {
    /// Signature verification failed inside `put_signed_object` → HTTP 401
    /// (unchanged from the old inline pre-verify behavior).
    BadSignature(String),
    /// `author_pubkey` was the wrong length → HTTP 400 (client input fault).
    BadPublicKey(String),
    /// object_id computation or a genuine storage fault → HTTP 500.
    Storage(String),
}

/// `POST /api/v2/objects`
///
/// Validates signature, computes object_id, stores. Returns the canonical object_id.
/// Idempotent: re-submitting the same object returns `stored: false`.
///
/// Concurrency: the ML-DSA-65 signature verify is CPU-bound and used to run
/// inline on the tokio worker pool (it ran TWICE — once here as a pre-check
/// and again inside `put_signed_object`). We now drop the redundant pre-check
/// and run the single authoritative verify + the DB write together inside
/// `tokio::task::spawn_blocking`, so a burst of object submissions can't starve
/// the async executor. `put_signed_object` itself rejects a bad signature
/// (returns `Err`) before ANY row is written — see
/// `storage/signed_objects.rs` `put_signed_object` and its
/// `put_rejects_tampered_object` test — so removing the pre-check does not
/// weaken the guarantee that an unverifiable object is never stored.
pub async fn post_object(
    State(state): State<Arc<RelayState>>,
    Json(payload): Json<SignedObjectSubmission>,
) -> impl IntoResponse {
    // Per-author submission quota (audit 2026-06-12). The signature is verified
    // below (so the author key can't be spoofed), but a holder of one valid key
    // could otherwise flood the DB with unlimited DISTINCT objects. Cap the rate
    // per author in a sliding window; over the cap → 429. Mirrors the
    // identify_rate pattern (compute under the lock, drop the guard before any
    // await). Keyed by a short hash of the author key so the map keys stay small.
    {
        use std::time::Instant;
        const OBJECT_SUBMIT_WINDOW_SECS: u64 = 60;
        const OBJECT_SUBMIT_MAX: usize = 30; // generous for legit votes/vouches/recovery; kills floods
        const OBJECT_RATE_MAP_CAP: usize = 50_000; // bound distinct-author growth
        let fp = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(payload.author_public_key_b64.as_bytes());
            hex::encode(&h.finalize()[..8])
        };
        let over_limit = {
            let mut rate = state.object_submit_rate.lock().unwrap();
            let now = Instant::now();
            // Occasional global prune so a churn of distinct authors can't grow
            // the map without bound (drop keys whose timestamps are all stale).
            if rate.len() > OBJECT_RATE_MAP_CAP {
                rate.retain(|_, times| {
                    times.retain(|t| now.duration_since(*t).as_secs() < OBJECT_SUBMIT_WINDOW_SECS);
                    !times.is_empty()
                });
            }
            let times = rate.entry(fp).or_default();
            times.retain(|t| now.duration_since(*t).as_secs() < OBJECT_SUBMIT_WINDOW_SECS);
            if times.len() >= OBJECT_SUBMIT_MAX {
                true
            } else {
                times.push(now);
                false
            }
        };
        if over_limit {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": format!(
                        "submission rate limit: max {OBJECT_SUBMIT_MAX} objects per {OBJECT_SUBMIT_WINDOW_SECS}s per author"
                    )
                })),
            )
                .into_response();
        }
    }

    let object = match payload.to_object() {
        Ok(o) => o,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("malformed submission: {e}")
            })))
                .into_response();
        }
    };

    // Run the CPU-bound verify + the DB write off the async executor. The
    // closure must be `'static + Send`, so we clone the `Arc<RelayState>` and
    // move the owned `object` in; the object is handed back out so the
    // (rare) gossip path below can reuse it without an extra clone.
    let state_blocking = state.clone();
    let join = tokio::task::spawn_blocking(move || {
        // Compute the canonical id first (cheap relative to the verify, but
        // also CPU-bound CBOR encoding — keep it off the executor too).
        let object_id = object
            .object_id()
            .map_err(|e| IngestError::Storage(format!("object_id computation failed: {e}")))?
            .to_hex();
        // Single authoritative verify happens INSIDE put_signed_object; a bad
        // signature returns Err here and nothing is written.
        match state_blocking.db.put_signed_object(&object, None) {
            Ok(stored) => Ok((object, object_id, stored)),
            Err(e) => {
                // `put_signed_object` wraps a verify failure as a
                // `ToSqlConversionFailure` whose message begins with
                // "signature verification failed:" (see signed_objects.rs),
                // and a wrong-length key as "author_pubkey must be ... bytes".
                // Classify on those stable, same-crate markers so the client
                // keeps seeing 401 / 400 (not a blanket 500) — the exact
                // status codes the old inline pre-verify produced.
                let msg = e.to_string();
                if msg.contains("signature verification failed") {
                    Err(IngestError::BadSignature(msg))
                } else if msg.contains("author_pubkey must be") {
                    Err(IngestError::BadPublicKey(msg))
                } else {
                    Err(IngestError::Storage(format!("storage error: {e}")))
                }
            }
        }
    });

    let (object, object_id, stored) = match join.await {
        Ok(Ok(triple)) => triple,
        Ok(Err(IngestError::BadSignature(msg))) => {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                "error": format!("signature verification failed: {msg}")
            })))
                .into_response();
        }
        Ok(Err(IngestError::BadPublicKey(msg))) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": msg
            })))
                .into_response();
        }
        Ok(Err(IngestError::Storage(msg))) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": msg
            })))
                .into_response();
        }
        // The blocking task panicked or was cancelled — surface as 500.
        Err(join_err) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("ingest task failed: {join_err}")
            })))
                .into_response();
        }
    };

    // Phase 3 PR 1: gossip newly-stored locally-submitted objects to peers.
    // Already-known objects (stored == false) are not re-gossiped.
    if stored {
        let state_clone = state.clone();
        tokio::spawn(async move {
            crate::relay::handlers::federation::gossip_signed_object(
                &state_clone,
                &object,
                None, // locally submitted — no peer to exclude
            )
            .await;
        });
    }
    (
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
        .into_response()
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

/// Query params for `GET /api/v2/groups`.
#[derive(Debug, Deserialize)]
pub struct MyGroupsQuery {
    /// Hex-encoded Dilithium3 public key of the member.
    pub pubkey: Option<String>,
}

/// A P2P group + its current roster, as seen by the relay's projection.
#[derive(Debug, Serialize)]
pub struct P2pGroupView {
    pub group_id: String,
    pub name: String,
    /// Active member public keys, hex-encoded.
    pub members: Vec<String>,
    /// Whether the requesting `pubkey` is this group's creator — gates the
    /// client's "Disband group" action (everyone can Leave; only the creator
    /// can Disband). Computed server-side so the client needs no extra fetch.
    pub is_creator: bool,
}

/// `GET /api/v2/groups?pubkey=<hex>` — the P2P groups the given member is in,
/// each with its current roster, read from the projection (docs/design/p2p-groups.md
/// Phase 1). Read-only convenience view; the authority is the signed objects.
pub async fn my_p2p_groups(
    State(state): State<Arc<RelayState>>,
    Query(q): Query<MyGroupsQuery>,
) -> impl IntoResponse {
    let pubkey_hex = match q.pubkey {
        Some(p) if !p.is_empty() => p,
        _ => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "pubkey (hex) required"})))
                .into_response();
        }
    };
    // ASCII-hex only (so byte-slicing below can't split a multi-byte char).
    if pubkey_hex.len() % 2 != 0 || !pubkey_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "pubkey not hex"})))
            .into_response();
    }
    let pubkey: Vec<u8> = (0..pubkey_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&pubkey_hex[i..i + 2], 16).unwrap())
        .collect();
    let my_fp = crate::relay::storage::author_fingerprint(&pubkey);
    let groups = state.db.p2p_groups_for_member(&pubkey).unwrap_or_default();
    let out: Vec<P2pGroupView> = groups
        .into_iter()
        .map(|(group_id, name)| {
            let members = state
                .db
                .p2p_group_roster(&group_id)
                .unwrap_or_default()
                .into_iter()
                .map(|m| m.member_pubkey.iter().map(|b| format!("{b:02x}")).collect::<String>())
                .collect();
            let is_creator = state
                .db
                .p2p_group_creator_fp(&group_id)
                .ok()
                .flatten()
                .map(|c| c == my_fp)
                .unwrap_or(false);
            P2pGroupView { group_id, name, members, is_creator }
        })
        .collect();
    (StatusCode::OK, Json(serde_json::json!({"groups": out}))).into_response()
}

/// `GET /api/v2/groups/{group_id}/members` — the roster with each member's
/// Kyber public key (looked up from their registered identity), so a sender can
/// seal the epoch group key to every member. Public keys only — no secrets.
pub async fn group_member_keys(
    State(state): State<Arc<RelayState>>,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let roster = state.db.p2p_group_roster(&group_id).unwrap_or_default();
    let members: Vec<serde_json::Value> = roster
        .into_iter()
        .map(|m| {
            let pubkey_hex: String = m.member_pubkey.iter().map(|b| format!("{b:02x}")).collect();
            // Kyber pub is keyed by the Dilithium identity hex in registered_names.
            let kyber = state.db.get_kyber_public(&pubkey_hex).ok().flatten();
            serde_json::json!({ "pubkey": pubkey_hex, "kyber_public": kyber })
        })
        .collect();
    (StatusCode::OK, Json(serde_json::json!({ "members": members }))).into_response()
}

/// `GET /api/v2/groups/{group_id}/messages` — the group's encrypted messages
/// (full signed objects; the relay cannot decrypt them). Oldest→newest, capped.
pub async fn group_messages(
    State(state): State<Arc<RelayState>>,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let ids = state.db.p2p_group_message_ids(&group_id, 200).unwrap_or_default();
    let mut out: Vec<SignedObjectResponse> = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(Some(rec)) = state.db.get_signed_object(&id) {
            out.push(SignedObjectResponse::from(rec));
        }
    }
    (StatusCode::OK, Json(serde_json::json!({ "messages": out }))).into_response()
}

/// `GET /api/v2/groups/{group_id}/epoch` — the latest `group_epoch_key_v1`
/// object, so a member can unseal the current group key.
pub async fn group_epoch_key(
    State(state): State<Arc<RelayState>>,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    match state.db.p2p_group_latest_epoch_object(&group_id) {
        Ok(Some(oid)) => match state.db.get_signed_object(&oid) {
            Ok(Some(rec)) => (
                StatusCode::OK,
                Json(serde_json::to_value(SignedObjectResponse::from(rec)).unwrap_or_default()),
            )
                .into_response(),
            _ => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "epoch object missing"})))
                .into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "no epoch key yet"})))
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("storage error: {e}")})),
        )
            .into_response(),
    }
}

/// `GET /api/v2/groups/{group_id}/epochs` — ALL `group_epoch_key_v1` objects
/// (oldest→newest), so a member can unseal every epoch key they were sealed
/// into and decrypt the FULL message history. After a re-key, messages from an
/// earlier epoch can only be opened with that epoch's key — the latest key alone
/// (the `/epoch` endpoint) is insufficient, so this returns the whole set. The
/// relay still cannot decrypt anything (these are sealed, signed objects).
pub async fn group_epoch_keys(
    State(state): State<Arc<RelayState>>,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let ids = state.db.p2p_group_all_epoch_objects(&group_id).unwrap_or_default();
    let mut out: Vec<SignedObjectResponse> = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(Some(rec)) = state.db.get_signed_object(&id) {
            out.push(SignedObjectResponse::from(rec));
        }
    }
    (StatusCode::OK, Json(serde_json::json!({ "epochs": out }))).into_response()
}

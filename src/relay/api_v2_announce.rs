//! HTTP API v2: external announcement endpoint (v0.119.0).
//!
//! `POST /api/v2/announce { channel?, content }`
//!
//! Lets external scripts (CI workflows, bump-version.js, custom integrations)
//! post into a system-managed chat channel. Auth: requires `Authorization:
//! Bearer <API_SECRET>` (same pattern as `POST /api/send`).
//!
//! `channel` defaults to "announcements". The relay signs the message with
//! its own Ed25519 keypair (system identity), persists it, and broadcasts
//! to every connected client.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::relay::handlers::announce;
use crate::relay::relay::RelayState;

#[derive(Debug, Deserialize)]
pub struct AnnounceRequest {
    pub content: String,
    #[serde(default)]
    pub channel: Option<String>,
}

fn check_api_auth(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let expected = std::env::var("API_SECRET").unwrap_or_default();
    if expected.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "API_SECRET not configured".into()));
    }
    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if provided.len() != expected.len()
        || !constant_time_eq(provided.as_bytes(), expected.as_bytes())
    {
        return Err((StatusCode::UNAUTHORIZED, "invalid Bearer token".into()));
    }
    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub async fn post_announce(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(req): Json<AnnounceRequest>,
) -> impl IntoResponse {
    if let Err((code, msg)) = check_api_auth(&headers) {
        return (code, Json(serde_json::json!({"error": msg}))).into_response();
    }
    // Global announce rate cap (audit 2026-06-12): system announcements are
    // broadcast to everyone, so bound the flood blast-radius if API_SECRET
    // leaks. Legit use is ~1/deploy.
    {
        use std::time::Instant;
        const ANNOUNCE_WINDOW_SECS: u64 = 60;
        const ANNOUNCE_MAX: usize = 20;
        let over = {
            let mut times = state.announce_rate.lock().unwrap();
            let now = Instant::now();
            times.retain(|t| now.duration_since(*t).as_secs() < ANNOUNCE_WINDOW_SECS);
            if times.len() >= ANNOUNCE_MAX {
                true
            } else {
                times.push(now);
                false
            }
        };
        if over {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": format!("announce rate limit: max {ANNOUNCE_MAX} per {ANNOUNCE_WINDOW_SECS}s")
                })),
            )
                .into_response();
        }
    }
    if req.content.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "content required"}))).into_response();
    }
    let channel = req
        .channel
        .as_deref()
        .unwrap_or(announce::DEFAULT_ANNOUNCEMENT_CHANNEL);
    announce::announce_to(&state, channel, &req.content).await;
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "channel": channel}))).into_response()
}

//! HTTP API v2: Recovery through trusted people (2026-10-21). The protocol is
//! relay/core/recovery.rs, what the server keeps storage/recovery.rs, the design
//! docs/design/recovery-through-trusted-people.md.
//!
//! - `POST /api/v2/objects` with a `recovery_share_v1`: one piece of a setup, signed by the
//!   person (kept in `recovery_pieces`, never in the object store; see signed_objects.rs).
//! - `GET  /api/v2/recovery/setup/{account}`: the setup that stands and the one being sent.
//!   Every piece is served as the signed object it was posted as, so an app checks it itself.
//! - `POST /api/v2/recovery/withdraw` `{key, timestamp, sig}`: the person takes every piece off
//!   this server (Dilithium3 over `recovery_withdraw\n{timestamp}`, files.rs `check_signed`).
//! - `POST /api/v2/recovery/ask` `{holder_did, one_time_key_b64, close_hash}`: a new device asks
//!   for help. Needs no signature (the asker has lost the key); the code it is filed under is
//!   computed here from the account and the one-time key.
//! - `GET  /api/v2/recovery/ask/{code}`: the ask, its one-time key and the answers so far.
//! - `GET  /api/v2/recovery/asks`: every open ask, whose and when, never a code or a key.
//! - `POST /api/v2/recovery/answer` `{code, slot, token_b64, sealed_b64}`: a trusted person's
//!   piece, sealed to the one-time key, with the slot's answer token.
//! - `POST /api/v2/recovery/close` `{code, close_secret_b64}`: the device that asked closes it.
//! - `POST /api/v2/recovery/not-me` `{key, timestamp, sig}`: the person's own app closes every
//!   request naming them ("that was not me", `recovery_not_me\n{timestamp}`).
//!
//! Nothing here can collect the pieces: an answer is sealed to a key only the asking device
//! holds, and the code the trusted person types commits to that key.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::Deserialize;
use std::sync::Arc;

use crate::relay::core::did::did_for_pubkey;
use crate::relay::core::recovery::{NOT_ME_PURPOSE, WITHDRAW_PURPOSE, account_id_ok, normalize_code};
use crate::relay::relay::RelayState;
use crate::relay::storage::RecoveryRefusal;

/// The most open asks the list answers with.
pub const OPEN_ASKS_LISTED: usize = 500;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn say(code: StatusCode, words: impl Into<String>) -> Response {
    (code, Json(serde_json::json!({ "error": words.into() }))).into_response()
}

fn refused(r: RecoveryRefusal) -> Response {
    match r {
        RecoveryRefusal::Bad(w) => say(StatusCode::BAD_REQUEST, w),
        RecoveryRefusal::NotFound(w) => say(StatusCode::NOT_FOUND, w),
        RecoveryRefusal::NotAllowed(w) => say(StatusCode::FORBIDDEN, w),
        RecoveryRefusal::Full(w) => say(StatusCode::TOO_MANY_REQUESTS, w),
        RecoveryRefusal::Db(e) => db(e),
    }
}

fn db(e: rusqlite::Error) -> Response {
    tracing::error!("recovery: storage failed: {e}");
    say(StatusCode::INTERNAL_SERVER_ERROR, "The server could not read or write its records.")
}

fn code_of(typed: &str) -> Result<String, Response> {
    normalize_code(typed).ok_or_else(|| say(StatusCode::BAD_REQUEST, "That is not a code: it is 16 letters and numbers."))
}

fn key_did(key_hex: &str) -> Option<String> {
    hex::decode(key_hex.trim()).ok().filter(|b| !b.is_empty()).map(|b| did_for_pubkey(&b))
}

/// `GET /api/v2/recovery/setup/{account}`
pub async fn get_recovery_setup(State(state): State<Arc<RelayState>>, Path(holder): Path<String>) -> Response {
    if !account_id_ok(&holder) {
        return say(StatusCode::BAD_REQUEST, "That is not an account id.");
    }
    match state.db.recovery_setup(&holder) {
        Ok(Some(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(None) => say(StatusCode::NOT_FOUND, "This account has no trusted people on this server."),
        Err(e) => db(e),
    }
}

/// A request signed by the person's own key: `purpose\ntimestamp` (files.rs `check_signed`).
#[derive(Debug, Deserialize)]
pub struct SignedBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
}

/// `POST /api/v2/recovery/withdraw`
pub async fn withdraw(State(state): State<Arc<RelayState>>, Json(body): Json<SignedBody>) -> Response {
    let Some(holder) = key_did(&body.key) else { return say(StatusCode::BAD_REQUEST, "key is not a public key") };
    if let Err((code, words)) = crate::relay::files::check_signed(&state, &body.key, body.timestamp, &body.sig, WITHDRAW_PURPOSE).await {
        return say(code, words);
    }
    match state.db.recovery_withdraw(&holder, body.timestamp) {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({ "removed": n }))).into_response(),
        Err(e) => db(e),
    }
}

/// `POST /api/v2/recovery/ask` body.
#[derive(Debug, Deserialize)]
pub struct AskBody {
    pub holder_did: String,
    pub one_time_key_b64: String,
    pub close_hash: String,
}

/// `POST /api/v2/recovery/ask`
pub async fn open_ask(State(state): State<Arc<RelayState>>, Json(body): Json<AskBody>) -> Response {
    let Ok(key) = B64.decode(body.one_time_key_b64.trim()) else {
        return say(StatusCode::BAD_REQUEST, "one_time_key_b64 is not base64");
    };
    match state.db.recovery_open_ask(body.holder_did.trim(), &key, body.close_hash.trim(), now_ms()) {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(r) => refused(r),
    }
}

/// `GET /api/v2/recovery/ask/{code}`
pub async fn get_ask(State(state): State<Arc<RelayState>>, Path(code): Path<String>) -> Response {
    let code = match code_of(&code) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match state.db.recovery_ask(&code, now_ms()) {
        Ok(Some(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(None) => say(StatusCode::NOT_FOUND, "There is no open request for help with that code."),
        Err(e) => db(e),
    }
}

/// `GET /api/v2/recovery/asks`
pub async fn list_asks(State(state): State<Arc<RelayState>>) -> Response {
    match state.db.recovery_open_asks(now_ms(), OPEN_ASKS_LISTED) {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(e) => db(e),
    }
}

/// `POST /api/v2/recovery/answer` body.
#[derive(Debug, Deserialize)]
pub struct AnswerBody {
    pub code: String,
    pub slot: usize,
    pub token_b64: String,
    pub sealed_b64: String,
}

/// `POST /api/v2/recovery/answer`
pub async fn answer(State(state): State<Arc<RelayState>>, Json(body): Json<AnswerBody>) -> Response {
    let code = match code_of(&body.code) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (Ok(token), Ok(sealed)) = (B64.decode(body.token_b64.trim()), B64.decode(body.sealed_b64.trim())) else {
        return say(StatusCode::BAD_REQUEST, "token_b64 and sealed_b64 are base64");
    };
    match state.db.recovery_answer(&code, body.slot, &token, &sealed, now_ms()) {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({ "answered": n }))).into_response(),
        Err(r) => refused(r),
    }
}

/// `POST /api/v2/recovery/close` body: the secret the asking device kept.
#[derive(Debug, Deserialize)]
pub struct CloseBody {
    pub code: String,
    pub close_secret_b64: String,
}

/// `POST /api/v2/recovery/close`
pub async fn close(State(state): State<Arc<RelayState>>, Json(body): Json<CloseBody>) -> Response {
    let code = match code_of(&body.code) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(secret) = B64.decode(body.close_secret_b64.trim()) else { return say(StatusCode::BAD_REQUEST, "close_secret_b64 is not base64") };
    match state.db.recovery_close_with_secret(&code, &secret) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "closed": true }))).into_response(),
        Err(r) => refused(r),
    }
}

/// `POST /api/v2/recovery/not-me`
pub async fn not_me(State(state): State<Arc<RelayState>>, Json(body): Json<SignedBody>) -> Response {
    let Some(holder) = key_did(&body.key) else { return say(StatusCode::BAD_REQUEST, "key is not a public key") };
    if let Err((status, words)) = crate::relay::files::check_signed(&state, &body.key, body.timestamp, &body.sig, NOT_ME_PURPOSE).await {
        return say(status, words);
    }
    match state.db.recovery_close_all_for(&holder) {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({ "closed": n }))).into_response(),
        Err(e) => db(e),
    }
}

#[cfg(test)]
#[path = "api_v2_recovery_tests.rs"]
mod tests;

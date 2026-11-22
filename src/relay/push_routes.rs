//! The Web Push subscription routes, POST /api/push/subscribe and /api/push/unsubscribe, signed
//! over everything they act on (2026-11-06, BUG-255; moved out of api.rs).
//!
//! WHAT CHANGED, AND WHY. The subscribe signature covered only `push_subscribe\n{timestamp}`, so a
//! signed request seen once could be sent again within its five minutes with ANOTHER endpoint (and
//! a timestamp in the future was never refused, so such a request stayed good for ever); it was
//! not checked against replays; and unsubscribe needed nothing at all, so anyone holding an
//! endpoint could remove it. Now:
//! - subscribe signs `push_subscribe\n{endpoint}\n{p256dh}\n{auth}` as a request's purpose
//!   (`hum/request/v2`, this server's address, the purpose, the time: `signed_request.rs`, since 2026-11-20): the endpoint
//!   and both of the browser's keys, so none can be swapped;
//! - unsubscribe signs `push_unsubscribe\n{endpoint}` and removes the subscription only when the
//!   signer is the one it belongs to;
//! - both are fresh (at most five minutes old, 30 seconds ahead: `signed_request::SIGNED_WINDOW_MS`
//!   and `SIGNED_AHEAD_MS`) and used once (`RelayState::auth_nonce_fresh`, keyed by the route's own
//!   short tag so a long endpoint never lands in the replay memory);
//! - the endpoint must be somewhere a push may go (`outbound`: https, a public name, at most
//!   2,048 characters), checked before the signature (cheap, and nothing is looked up for an
//!   unsigned request) and its name looked up only after it.
//!
//! The words signed are pinned in `scripts/tests/fixtures/push-signing-v1.json`, which the
//! website's test (`scripts/tests/push-subscribe-web.test.js`) holds its two requests to. Rows
//! kept before this change stay valid: only new requests are signed this way. A refusal answers
//! `{ "reason", "error" }`: `error` in the relay's words, `reason` a code the website shows its
//! own words for, by key (`push.refused_*`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use base64ct::{Base64UrlUnpadded, Encoding};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::relay::outbound::{Refusal, Schemes, MAX_URL_LEN};
use crate::relay::relay::RelayState;

type Refused = (StatusCode, Json<Value>);

/// The words a subscribe request signs, before `\n{timestamp}`.
pub fn subscribe_purpose(endpoint: &str, p256dh: &str, auth: &str) -> String {
    format!("push_subscribe\n{endpoint}\n{p256dh}\n{auth}")
}

/// The words an unsubscribe request signs, before `\n{timestamp}`.
pub fn unsubscribe_purpose(endpoint: &str) -> String {
    format!("push_unsubscribe\n{endpoint}")
}

#[derive(Debug, Deserialize)]
pub struct PushSubscribeRequest {
    /// The subscriber's Dilithium3 public key (hex): whose notices go to this endpoint.
    pub key: String,
    /// The push service's endpoint, from the browser's PushSubscription.
    pub endpoint: String,
    /// PushSubscription.keys.p256dh: the browser's P-256 public key (base64url, 65 bytes).
    pub p256dh: String,
    /// PushSubscription.keys.auth: the browser's auth secret (base64url, 16 bytes).
    pub auth: String,
    /// Unix milliseconds.
    pub timestamp: u64,
    /// Dilithium3 over `subscribe_purpose(..)\n{timestamp}`, hex.
    pub sig: String,
}

#[derive(Debug, Deserialize)]
pub struct PushUnsubscribeRequest {
    /// The Dilithium3 public key (hex) the subscription belongs to.
    pub key: String,
    /// The endpoint to forget.
    pub endpoint: String,
    /// Unix milliseconds.
    pub timestamp: u64,
    /// Dilithium3 over `unsubscribe_purpose(..)\n{timestamp}`, hex.
    pub sig: String,
}

fn refused(status: StatusCode, reason: &str, words: &str) -> Refused {
    (status, Json(json!({ "reason": reason, "error": words })))
}

fn endpoint_refused(why: Refusal) -> Refused {
    refused(StatusCode::BAD_REQUEST, why.reason(), why.words())
}

/// The browser's two keys as a push will use them (`web_push::build_request`): a P-256 public
/// key and a 16-byte auth secret, both unpadded base64url.
fn keys_ok(p256dh: &str, auth: &str) -> bool {
    match (Base64UrlUnpadded::decode_vec(p256dh), Base64UrlUnpadded::decode_vec(auth)) {
        (Ok(p), Ok(a)) => a.len() == 16 && p256::PublicKey::from_sec1_bytes(&p).is_ok(),
        _ => false,
    }
}

/// Fresh, signed as a request over `purpose` (never a chat line's shape), and used once (the
/// replay memory keyed by `tag`, the route's name): `signed_request::check`.
async fn check_signature(state: &RelayState, key: &str, timestamp: u64, sig: &str, purpose: String, tag: &'static str) -> Result<(), Refused> {
    use crate::relay::signed_request::SignedRefusal;
    crate::relay::signed_request::check(state, key, timestamp, sig, &purpose, tag).await.map_err(|why| match why {
        // A replay answers 409, as it always has here.
        SignedRefusal::Replay => refused(StatusCode::CONFLICT, why.code(), &why.words()),
        // No key or signature at all is a signature that does not match.
        SignedRefusal::Unsigned => refused(StatusCode::UNAUTHORIZED, "signature", &SignedRefusal::Signature.words()),
        other => refused(other.status(), other.code(), &other.words()),
    })
}

/// POST /api/push/subscribe: keep a browser's push subscription for the key that signed it.
pub async fn push_subscribe(state: State<Arc<RelayState>>, Json(body): Json<PushSubscribeRequest>) -> Result<Json<Value>, Refused> {
    // What is to be kept, first: cheap, and refused before any signature work.
    state.outbound.shape(&body.endpoint, Schemes::HttpsOnly).map_err(endpoint_refused)?;
    if !keys_ok(&body.p256dh, &body.auth) {
        return Err(refused(StatusCode::BAD_REQUEST, "keys", "The browser's notification keys are not in the expected form."));
    }
    let purpose = subscribe_purpose(&body.endpoint, &body.p256dh, &body.auth);
    check_signature(&state, &body.key, body.timestamp, &body.sig, purpose, "push_subscribe").await?;
    // Only for a signed request is the name looked up: every address it has must be public.
    state.outbound.check(&body.endpoint, Schemes::HttpsOnly).await.map_err(endpoint_refused)?;

    state
        .db
        .save_push_subscription(&body.key, &body.endpoint, &body.p256dh, &body.auth)
        .map_err(|e| refused(StatusCode::INTERNAL_SERVER_ERROR, "storage", &format!("DB error: {e}")))?;
    tracing::info!("Push subscription registered for key {}", body.key.get(..8).unwrap_or(""));
    Ok(Json(json!({ "status": "subscribed" })))
}

/// POST /api/push/unsubscribe: forget a push subscription, signed by the key it belongs to. The
/// endpoint's shape is not checked here (only its length), so a subscription kept before the
/// endpoint rules can still be removed by its owner.
pub async fn push_unsubscribe(state: State<Arc<RelayState>>, Json(body): Json<PushUnsubscribeRequest>) -> Result<Json<Value>, Refused> {
    if body.endpoint.is_empty() {
        return Err(endpoint_refused(Refusal::NotAWebAddress));
    }
    if body.endpoint.len() > MAX_URL_LEN {
        return Err(endpoint_refused(Refusal::TooLong));
    }
    let purpose = unsubscribe_purpose(&body.endpoint);
    check_signature(&state, &body.key, body.timestamp, &body.sig, purpose, "push_unsubscribe").await?;
    let removed = state
        .db
        .remove_own_push_subscription(&body.key, &body.endpoint)
        .map_err(|e| refused(StatusCode::INTERNAL_SERVER_ERROR, "storage", &format!("DB error: {e}")))?;
    if removed == 0 {
        // The same answer whether the endpoint is unknown or someone else's: it tells the signer
        // nothing about other people's subscriptions.
        return Err(refused(StatusCode::NOT_FOUND, "not_found", "This server holds no push subscription of yours at that address."));
    }
    Ok(Json(json!({ "status": "unsubscribed" })))
}

#[cfg(test)]
#[path = "push_routes_tests.rs"]
mod tests;

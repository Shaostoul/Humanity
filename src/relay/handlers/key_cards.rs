//! Key cards on the relay (2026-10-22, docs/design/key-cards.md; the card itself is
//! core/key_card.rs, what is kept storage/key_cards.rs).
//!
//! A key card is a person's Kyber768 key signed by their own Dilithium3 identity. The server keeps
//! and serves cards and checks each one before it keeps it, but it is never trusted for one: every
//! app checks every card itself before it seals anything with the key it carries. What this file
//! does:
//! - keeps the card an app sends with its sign-in (`identify`'s `key_card`), when it checks out
//!   and names the key that sign-in just proved (`keep_from_sign_in`);
//! - answers `key_card_request {keys}` with the cards held, to the asker alone (`handle_request`),
//!   and `GET /api/key-card/{key}` for pages and threads with no socket (`api_get_key_card`);
//! - passes a newly kept card on to linked servers, keeps one a linked server passes on by the
//!   same rule, and sends this server's accounts' cards when a link comes up (the way profile
//!   copies travel, handlers/federation.rs).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::federation::key_in_log;
use crate::relay::core::key_card::{self, KeyCard, Keep, REQUEST_MAX};
use crate::relay::relay::{FederatedConnection, RelayMessage, RelayState};

/// The cards in one `key_cards` answer, behind an `Arc`: every socket's task receives its own
/// clone of each broadcast message and drops those not for it, and a card is 6 KB, so the cards
/// are shared rather than copied once per socket.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SharedCards(pub Arc<Vec<KeyCard>>);

impl Serialize for SharedCards {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.as_slice().serialize(s)
    }
}

impl<'de> Deserialize<'de> for SharedCards {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Vec::<KeyCard>::deserialize(d).map(|v| SharedCards(Arc::new(v)))
    }
}

/// What a person is told when the card their app sent with its sign-in is refused. It does not
/// stop the sign-in: only that nobody can seal anything to them with that card.
pub const REFUSED_SENTENCE: &str = "This server could not keep your app's key card (it did not check out as yours), so people cannot send you private messages here until it can. Updating the app usually fixes this.";

/// Does `card` check out as `person`'s, run off the async pool (one Dilithium3 verify). A
/// panicked or cancelled check counts as a failure.
pub async fn checks_out(card: &KeyCard, person: &str) -> bool {
    let (c, p) = (card.clone(), person.to_string());
    tokio::task::spawn_blocking(move || key_card::card_for(&c, &p)).await.unwrap_or(false)
}

/// The card an app sent with its sign-in, once the sign-in is proven for `signed_in` (relay.rs,
/// after `identify_response`): kept by the keep rule when it checks out and names that very key,
/// and passed on to the linked servers when kept. A card that does not check out (or names
/// somebody else) is refused and the person told; the sign-in goes on. Returns what was done with
/// it (None: refused).
pub async fn keep_from_sign_in(state: &Arc<RelayState>, signed_in: &str, value: serde_json::Value) -> Option<Keep> {
    let card = key_card::parse_card(&value);
    let good = match &card {
        Some(c) => checks_out(c, signed_in).await,
        None => false,
    };
    let Some(card) = card.filter(|_| good) else {
        tracing::warn!("Key cards: refused the card sent with the sign-in of {}: it did not check out as theirs", key_in_log(signed_in));
        let _ = state.broadcast_tx.send(RelayMessage::Private { to: signed_in.to_string(), message: REFUSED_SENTENCE.to_string() });
        return None;
    };
    let keep = match state.db.store_key_card(&card) {
        Ok(keep) => keep,
        Err(e) => {
            tracing::error!("Key cards: could not keep the card of {}: {e}", key_in_log(signed_in));
            return None;
        }
    };
    if keep.stored() {
        tracing::info!("Key cards: kept the card of {} ({keep:?})", key_in_log(signed_in));
        gossip_card(state, &card, None).await;
    }
    Some(keep)
}

/// A card a linked server passed on (`key_card_gossip`): kept when it checks out as the card of
/// the key it names, by the same keep rule. A card newly kept for one of THIS server's accounts
/// goes on to the other links, never back to `source`; one for someone with no account here is
/// kept but not passed on (their own server sends it). It cannot loop: a card that is not kept goes
/// no further. Returns whether it was kept.
pub async fn keep_from_link(state: &Arc<RelayState>, source: &str, card: KeyCard) -> bool {
    let key = card.key.clone();
    if !checks_out(&card, &key).await {
        tracing::warn!("Federation: refused a key card for {} from {}: it did not check out", key_in_log(&key), key_in_log(source));
        return false;
    }
    let keep = match state.db.store_key_card(&card) {
        Ok(keep) => keep,
        Err(e) => {
            tracing::error!("Federation: could not keep a key card for {}: {e}", key_in_log(&key));
            return false;
        }
    };
    if !keep.stored() {
        tracing::debug!("Federation: key card for {} from {}: not kept ({keep:?})", key_in_log(&key), key_in_log(source));
        return false;
    }
    let ours = state.db.name_for_key(&key).ok().flatten().is_some();
    if ours {
        gossip_card(state, &card, Some(source)).await;
    }
    true
}

/// The `key_card_gossip` frame carrying one card.
pub(crate) fn gossip_text(card: &KeyCard) -> Option<String> {
    serde_json::to_string(&RelayMessage::KeyCardGossip { card: card.clone() }).ok()
}

/// Send `card` to every linked server but `exclude` (the one it came from). Numbered in the
/// outbox like a profile copy (federation_log.rs `publish`, kind "key_card"), so a link that is
/// down or paused gets it when it catches up, applied once and in order; `exclude` is sent a skip.
pub async fn gossip_card(state: &Arc<RelayState>, card: &KeyCard, exclude: Option<&str>) {
    let Some(json) = gossip_text(card) else { return };
    let Ok(event) = serde_json::from_str::<serde_json::Value>(&json) else { return };
    crate::relay::handlers::federation_log::publish(state, "key_card", "", 0, exclude.unwrap_or(""), &event).await;
}

/// The catch-up when a link comes up, queued on that link's own channel: the cards of this
/// server's own accounts, newest first, at most 1,000 (`key_cards_of_accounts_here`). Called by
/// handlers/federation.rs `send_own_copies`, once per link, beside the profile copies. Returns how
/// many were queued.
pub(crate) fn queue_own_cards(state: &RelayState, conn: &FederatedConnection) -> usize {
    let cards = match state.db.key_cards_of_accounts_here() {
        Ok(cards) => cards,
        Err(e) => {
            tracing::error!("Federation: could not read the key cards to send {} on link up: {e}", key_in_log(&conn.server_id));
            return 0;
        }
    };
    let mut sent = 0;
    for card in &cards {
        let Some(text) = gossip_text(card) else { continue };
        if conn.tx.send(text).is_err() {
            break;
        }
        sent += 1;
    }
    sent
}

/// The keys of a `key_card_request`: its `keys`, each a string, at most [`REQUEST_MAX`] (the
/// rest are ignored), each once.
fn requested_keys(raw: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for k in raw.get("keys").and_then(|v| v.as_array()).into_iter().flatten() {
        let Some(k) = k.as_str() else { continue };
        if keys.len() >= REQUEST_MAX {
            break;
        }
        if !keys.iter().any(|have| have == k) {
            keys.push(k.to_string());
        }
    }
    keys
}

/// The cards held for `keys` and the keys with none (a text that is not a key counts as one
/// with none, and is not looked up).
pub fn cards_for(state: &RelayState, keys: &[String]) -> (Vec<KeyCard>, Vec<String>) {
    let mut cards = Vec::new();
    let mut missing = Vec::new();
    for k in keys {
        match key_card::is_key_text(k).then(|| state.db.get_key_card(k).ok().flatten()).flatten() {
            Some(card) => cards.push(card),
            None => missing.push(k.clone()),
        }
    }
    (cards, missing)
}

/// `key_card_request {keys}` from a signed-in socket: the cards held, to the asker alone.
pub fn handle_request(state: &Arc<RelayState>, my_key: &str, raw: &serde_json::Value) {
    let keys = requested_keys(raw);
    if keys.is_empty() {
        return;
    }
    let (cards, missing) = cards_for(state, &keys);
    let _ = state.broadcast_tx.send(RelayMessage::KeyCards { to: my_key.to_string(), cards: SharedCards(Arc::new(cards)), missing });
}

/// `GET /api/key-card/{key}`: the card held for `key`, exactly as its owner signed it, or 404.
/// For the website's standalone pages and the desktop app's work on threads, which have no socket.
/// Anyone may ask: a card holds only public keys, and checking it is the asker's job.
pub async fn api_get_key_card(State(state): State<Arc<RelayState>>, Path(key): Path<String>) -> impl IntoResponse {
    if !key_card::is_key_text(&key) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "That is not a person's key." }))).into_response();
    }
    match state.db.get_key_card(&key) {
        Ok(Some(card)) => (StatusCode::OK, Json(key_card::card_json(&card))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "This server holds no key card for that person." }))).into_response(),
        Err(e) => {
            tracing::error!("Key cards: could not read a card: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "The server could not read its key cards." }))).into_response()
        }
    }
}

#[cfg(test)]
#[path = "key_cards_tests.rs"]
mod tests;

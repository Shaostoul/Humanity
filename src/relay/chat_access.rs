//! Chat access levels on the server (2026-11-02, docs/design/chat-access.md): who a key is here,
//! whether its socket may be sent a channel's lines, the check every channel line meets, and the
//! routes a channel's history, pins and reactions are read through.
//!
//! The levels and the rules themselves are `humanity_protocol::chat_access`, shared with the
//! desktop app. This file is what needs the database.
//!
//! THE ROUTES. A request without a member's signature (`GET /api/messages`, `/api/pins`,
//! `/api/reactions`) is answered for a channel that reads at Anyone only, with only what was posted
//! after the channel opened to Anyone and with no poster's key (the design's section 4.1). Any other
//! channel answers 401 and the level it needs. A member reads with `POST` on the same routes, signed
//! with their Dilithium3 key over [`access::read_purpose`] then the time (fresh for five minutes,
//! used once): the whole history, keys included, when their rank admits the channel's read level
//! (403 when it does not). `GET /api/channels` lists the Anyone channels for the website's read-only
//! view. Every refusal is `{"error": words, "read_level": level}`.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use humanity_protocol::chat_access::{self as access, PostLevel, PostRefusal, Rank, ReadLevel};

use crate::relay::api::{MessagesResponse, PinEntry, PinsResponse, ReactionEntry, ReactionsResponse};
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::Storage;

/// The rank `key` holds on this server. A bot (`bot_*`) signed in with the server's own bot secret
/// (L-1), so it is the operator's and trusted as before; anyone else needs a chosen name here to be
/// more than a visitor, and a banned key is a visitor whatever its role.
pub fn rank_of(db: &Storage, key: &str) -> Rank {
    if key.is_empty() {
        return Rank::Visitor;
    }
    if key.starts_with("bot_") {
        return Rank::Admin;
    }
    if db.name_for_key(key).ok().flatten().is_none() || db.is_banned(key).unwrap_or(false) {
        return Rank::Visitor;
    }
    let role = db.get_role(key).unwrap_or_default();
    // The built-in names decide by themselves; only a role the admins made needs its trust level.
    let trust = match role.as_str() {
        "" | "member" | "unverified" | "muted" | "verified" | "donor" | "mod" | "moderator" | "admin" | "owner" => 0,
        custom => db.role_def(custom).trust_level,
    };
    access::rank_for_member(&role, trust)
}

/// A channel's read and post levels and when it opened to Anyone. A channel this server does not
/// have reads and posts at Members, the safe default.
pub fn levels(db: &Storage, channel: &str) -> (ReadLevel, PostLevel, i64) {
    db.channel_access(channel).unwrap_or((ReadLevel::Members, PostLevel::Members, 0))
}

/// Set a channel's levels from the words an admin's app sent (`channel_update`, the registry's
/// `channels.access`): a level not given, or a word naming none, keeps what the channel has.
/// Returns false when there is no such channel.
pub fn set_levels(db: &Storage, channel: &str, read: Option<&str>, post: Option<&str>) -> bool {
    let Some((now_read, now_post, _)) = db.channel_access(channel) else { return false };
    let read = read.and_then(ReadLevel::parse).unwrap_or(now_read);
    let post = post.and_then(PostLevel::parse).unwrap_or(now_post);
    db.set_channel_access(channel, read, post).unwrap_or(false)
}

/// Whether the socket of `key` may be sent `channel`'s lines. An Anyone channel asks nothing more,
/// so the common case is one read.
pub fn may_see(db: &Storage, key: &str, channel: &str) -> bool {
    let (read, _, _) = levels(db, channel);
    read == ReadLevel::Anyone || read.admits(rank_of(db, key))
}

/// The channel whose words, reactions or pins a message to the sockets carries, if it carries any.
/// The send loop (relay.rs) passes a message on only to sockets that [`may_see`] it.
pub fn line_channel(msg: &RelayMessage) -> Option<&str> {
    match msg {
        RelayMessage::Chat { channel, .. }
        | RelayMessage::FederatedChat { channel, .. }
        | RelayMessage::Edit { channel, .. }
        | RelayMessage::Reaction { channel, .. }
        | RelayMessage::PinsSync { channel, .. }
        | RelayMessage::PinAdded { channel, .. }
        | RelayMessage::PinRemoved { channel, .. }
        | RelayMessage::LinkPreviews { channel, .. }
        | RelayMessage::MessageDeleted { channel, .. } => Some(channel.as_str()),
        _ => None,
    }
}

/// Whether a channel's new lines may travel to linked servers: only a channel anyone may read (a
/// peer shows what it receives to its own visitors), whatever its Federated tick says.
pub fn travels(db: &Storage, channel: &str) -> bool {
    levels(db, channel).0 == ReadLevel::Anyone
}

/// The words a refused line's sender is told.
fn refusal_words(why: PostRefusal, channel: &str) -> String {
    match why {
        PostRefusal::CannotRead(ReadLevel::Verified) | PostRefusal::PostLevel(PostLevel::Verified) => format!(
            "Not sent: only verified people can post in #{channel}. An admin of this server verifies people."
        ),
        PostRefusal::CannotRead(_) => format!("Not sent: only people with a name on this server can post in #{channel}."),
        PostRefusal::PostLevel(PostLevel::Mods) => format!("Not sent: only moderators and admins can post in #{channel}."),
        PostRefusal::PostLevel(PostLevel::Members) => format!("Not sent: choose a name on this server to post in #{channel}."),
        PostRefusal::MassMention => "Not sent: @everyone and @here are for this server's moderators and admins.".to_string(),
        PostRefusal::TooManyLinks { links } => format!(
            "Not sent: one link a message until an admin of this server verifies you (this one has {links})."
        ),
        PostRefusal::Attachment => {
            "Not sent: pictures and files need a verified account. An admin of this server verifies people.".to_string()
        }
    }
}

/// Whether `key`'s line `content` in `channel` is refused: the channel's levels, then for a member
/// below Verified one link and no attachment. When it is, the sender has been told why, in words
/// and as `chat_refused`, so their own copy is marked "Not sent". Asked after the slash commands
/// (a command is not a post).
pub fn post_refused(state: &Arc<RelayState>, key: &str, channel: &str, content: &str, timestamp: u64) -> bool {
    let (read, post, _) = levels(&state.db, channel);
    let Some(why) = access::post_refusal(read, post, rank_of(&state.db, key), content) else {
        return false;
    };
    let words = refusal_words(why, channel);
    let _ = state.broadcast_tx.send(RelayMessage::Private { to: key.to_string(), message: words });
    let _ = state.broadcast_tx.send(RelayMessage::ChatRefused {
        to: key.to_string(),
        timestamp,
        reason: why.reason().to_string(),
        limit: None,
    });
    true
}

/// #general's reactions for the socket of `key` at sign-in, or none when it may not read #general.
pub fn general_reactions(db: &Storage, key: &str) -> Result<Vec<crate::relay::storage::ReactionRecord>, rusqlite::Error> {
    if may_see(db, key, "general") { db.load_channel_reactions("general", 500) } else { Ok(Vec::new()) }
}

/// #general's pins for the socket of `key` at sign-in, or none when it may not read #general.
pub fn general_pins(db: &Storage, key: &str) -> Result<Vec<crate::relay::storage::PinnedMessageRecord>, rusqlite::Error> {
    if may_see(db, key, "general") { db.get_pinned_messages("general") } else { Ok(Vec::new()) }
}

// ── The routes ──

/// A refusal: the status, the words, and the level the channel reads at.
fn refuse(status: StatusCode, words: &str, read: ReadLevel) -> Response {
    (status, Json(json!({ "error": words, "read_level": read.as_str() }))).into_response()
}

/// For a request without a member's signature: when `channel` reads at Anyone, the time it opened
/// (only posts after it are answered); otherwise the refusal.
fn open_to_visitors(db: &Storage, channel: &str) -> Result<i64, Response> {
    let (read, _, since) = levels(db, channel);
    if read == ReadLevel::Anyone {
        Ok(since)
    } else {
        Err(refuse(
            StatusCode::UNAUTHORIZED,
            "This channel is for people with a name on this server: sign in to the chat to read it.",
            read,
        ))
    }
}

/// A member's signed read of one channel: the routes' POST body.
#[derive(Debug, Deserialize)]
pub struct SignedRead {
    /// The reader's Dilithium3 public key (hex): who they are.
    pub key: String,
    /// When they signed, in milliseconds.
    pub timestamp: u64,
    /// Their Dilithium3 signature (hex) over [`access::read_purpose`] then `\n{timestamp}`.
    pub sig: String,
    /// The channel (default #general).
    #[serde(default)]
    pub channel: Option<String>,
    /// For messages: only those after this row id.
    #[serde(default)]
    pub after: Option<usize>,
    /// How many at most.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Whether a signed read of `what` in `channel` is answered: a signature from `req.key` over the
/// read's purpose, fresh (five minutes, and not from the future), not used before, from a key whose
/// rank admits the channel's read level. The refusal otherwise.
async fn signed_reader(state: &RelayState, req: &SignedRead, what: &str, channel: &str) -> Result<(), Response> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;
    let (read, _, _) = levels(&state.db, channel);
    if req.key.is_empty() || req.sig.is_empty() {
        return Err(refuse(StatusCode::UNAUTHORIZED, "A signed read needs a key and a signature.", read));
    }
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    if now_ms.saturating_sub(req.timestamp) > 5 * 60 * 1000 || req.timestamp > now_ms + 60_000 {
        return Err(refuse(StatusCode::BAD_REQUEST, "The request is too old or from the future: check this computer's clock.", read));
    }
    let purpose = access::read_purpose(what, channel);
    let (vk, vsig, vpurpose, vts) = (req.key.clone(), req.sig.clone(), purpose.clone(), req.timestamp);
    // CPU-bound: off the async executor, failing closed if the check panics.
    let ok = tokio::task::spawn_blocking(move || verify_dilithium_signature(&vk, &vpurpose, vts, &vsig)).await.unwrap_or(false);
    if !ok {
        return Err(refuse(StatusCode::UNAUTHORIZED, "The signature does not match this request.", read));
    }
    if !state.auth_nonce_fresh(&req.key, &purpose, req.timestamp) {
        return Err(refuse(StatusCode::UNAUTHORIZED, "That request was already used: try again.", read));
    }
    if !read.admits(rank_of(&state.db, &req.key)) {
        let words = if read == ReadLevel::Verified {
            "This channel is for verified people on this server. An admin of this server verifies people."
        } else {
            "This channel is for people with a name on this server."
        };
        return Err(refuse(StatusCode::FORBIDDEN, words, read));
    }
    Ok(())
}

/// A channel's messages after `after`, at most `limit` (200 at most), posted at or after `since`;
/// for a visitor, without keys.
fn messages(db: &Storage, channel: &str, after: Option<usize>, limit: Option<usize>, since: i64, visitor: bool) -> Response {
    let limit = limit.unwrap_or(50).min(200);
    match db.load_channel_messages_after(channel, after.unwrap_or(0) as i64, limit, since) {
        Ok((messages, cursor)) => {
            let messages = if visitor { messages.into_iter().map(for_visitors).collect() } else { messages };
            Json(MessagesResponse { messages, cursor: cursor as usize }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to load messages: {e}");
            (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "The messages could not be read just now." }))).into_response()
        }
    }
}

/// A line as a visitor gets it: no poster's key (a name is enough to follow a conversation; a key
/// ties a person across servers), and no signature either. A signature over the line's words is
/// checked against a key, so anyone holding a list of keys (another server's members, the key
/// cards) could try each one and find whose line it is: the key would be back in all but name.
fn for_visitors(mut msg: RelayMessage) -> RelayMessage {
    match &mut msg {
        RelayMessage::Chat { from, reply_to, signature, .. } => {
            from.clear();
            *signature = None;
            if let Some(r) = reply_to.as_mut() {
                r.from.clear();
            }
        }
        RelayMessage::FederatedChat { from_key, signature, .. } => {
            *from_key = None;
            *signature = None;
        }
        _ => {}
    }
    msg
}

/// GET /api/messages: an Anyone channel's posts since it opened, without keys (module docs).
pub async fn get_messages(State(state): State<Arc<RelayState>>, Query(q): Query<crate::relay::api::MessagesQuery>) -> Response {
    let channel = q.channel.as_deref().unwrap_or("general");
    match open_to_visitors(&state.db, channel) {
        Ok(since) => messages(&state.db, channel, q.after, q.limit, since, true),
        Err(refusal) => refusal,
    }
}

/// POST /api/messages: a member's signed read of a channel's history (module docs).
pub async fn post_messages(State(state): State<Arc<RelayState>>, Json(req): Json<SignedRead>) -> Response {
    let channel = req.channel.clone().unwrap_or_else(|| "general".to_string());
    if let Err(refusal) = signed_reader(&state, &req, "messages", &channel).await {
        return refusal;
    }
    messages(&state.db, &channel, req.after, req.limit, 0, false)
}

/// A channel's pins, those pinned from posts made at or after `since`.
fn pins(db: &Storage, channel: &str, since: i64, visitor: bool) -> Response {
    match db.get_pinned_messages(channel) {
        Ok(records) => {
            let pins = records
                .into_iter()
                .filter(|r| r.original_timestamp as i64 >= since)
                .map(|r| PinEntry {
                    from_key: if visitor { String::new() } else { r.from_key },
                    from_name: r.from_name,
                    content: r.content,
                    original_timestamp: r.original_timestamp,
                    pinned_by: if visitor { String::new() } else { r.pinned_by },
                    pinned_at: r.pinned_at,
                })
                .collect();
            Json(PinsResponse { pins }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to load pins: {e}");
            Json(PinsResponse { pins: vec![] }).into_response()
        }
    }
}

/// GET /api/pins: an Anyone channel's pins of posts made since it opened, without keys.
pub async fn get_pins(State(state): State<Arc<RelayState>>, Query(q): Query<crate::relay::api::PinsQuery>) -> Response {
    let channel = q.channel.as_deref().unwrap_or("general");
    match open_to_visitors(&state.db, channel) {
        Ok(since) => pins(&state.db, channel, since, true),
        Err(refusal) => refusal,
    }
}

/// POST /api/pins: a member's signed read of a channel's pins.
pub async fn post_pins(State(state): State<Arc<RelayState>>, Json(req): Json<SignedRead>) -> Response {
    let channel = req.channel.clone().unwrap_or_else(|| "general".to_string());
    if let Err(refusal) = signed_reader(&state, &req, "pins", &channel).await {
        return refusal;
    }
    pins(&state.db, &channel, 0, false)
}

/// A channel's reactions (at most `limit`, 1,000 at most), on posts made at or after `since`.
fn reactions(db: &Storage, channel: &str, limit: Option<usize>, since: i64, visitor: bool) -> Response {
    let limit = limit.unwrap_or(500).min(1000);
    match db.load_channel_reactions(channel, limit) {
        Ok(records) => {
            let reactions = records
                .into_iter()
                .filter(|r| r.target_timestamp as i64 >= since)
                .map(|r| ReactionEntry {
                    target_from: if visitor { String::new() } else { r.target_from },
                    target_timestamp: r.target_timestamp,
                    emoji: r.emoji,
                    reactor_key: if visitor { String::new() } else { r.reactor_key },
                    reactor_name: r.reactor_name,
                })
                .collect();
            Json(ReactionsResponse { reactions }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to load reactions: {e}");
            Json(ReactionsResponse { reactions: vec![] }).into_response()
        }
    }
}

/// GET /api/reactions: an Anyone channel's reactions on posts made since it opened, without keys.
pub async fn get_reactions(State(state): State<Arc<RelayState>>, Query(q): Query<crate::relay::api::ReactionsQuery>) -> Response {
    let channel = q.channel.as_deref().unwrap_or("general");
    match open_to_visitors(&state.db, channel) {
        Ok(since) => reactions(&state.db, channel, q.limit, since, true),
        Err(refusal) => refusal,
    }
}

/// POST /api/reactions: a member's signed read of a channel's reactions.
pub async fn post_reactions(State(state): State<Arc<RelayState>>, Json(req): Json<SignedRead>) -> Response {
    let channel = req.channel.clone().unwrap_or_else(|| "general".to_string());
    if let Err(refusal) = signed_reader(&state, &req, "reactions", &channel).await {
        return refusal;
    }
    reactions(&state.db, &channel, req.limit, 0, false)
}

/// GET /api/channels: the channels anyone may read, for the website's read-only view
/// (`web/chat/read.html`): each one's id, name and description, in the sidebar's order.
pub async fn get_channels(State(state): State<Arc<RelayState>>) -> Response {
    let channels: Vec<serde_json::Value> = state
        .db
        .list_channels()
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, _, _, read)| *read == ReadLevel::Anyone)
        .map(|(id, name, description, _)| json!({ "id": id, "name": name, "description": description.unwrap_or_default() }))
        .collect();
    Json(json!({ "channels": channels })).into_response()
}

#[cfg(test)]
#[path = "chat_access_tests.rs"]
mod tests;

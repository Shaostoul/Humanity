//! A file posted in a members-only room is served only to people who may read that room
//! (BUG-282, 2026-11-07).
//!
//! WHY. The relay served `data/uploads` to anyone (`nest_service("/uploads", ServeDir)`, nginx
//! proxying `/uploads/` to it), so a picture posted in a room that reads at Members, Verified or
//! staff only opened for anyone who was given, or guessed, its address; and the address was the
//! upload time in milliseconds and the original file name (`upload_rules.rs` `stored_name`), with
//! nothing random in it. Members-only Files items were checked (`files.rs`); chat attachments were
//! not.
//!
//! WHAT.
//! - A stored name carries 128 random bits (`upload_rules.rs` `stored_name`), so nobody can guess one.
//! - When a chat line is kept in a room, every upload of the poster's own that the line links to is
//!   bound to that room ([`note_posted`]): to the first room it is posted in, and to a room that
//!   reads at Anyone whenever it is posted in one (then it is public, as the room is). The binding
//!   is the column `user_uploads.room`; a file never posted in a room (a Files item, a sealed
//!   private file) has none.
//! - [`gate`] sits in front of `/uploads`: a file bound to a room that does not read at Anyone NOW
//!   (the levels are read at each request, so an admin opening or closing the room is followed) is
//!   served only to someone who may read that room (`chat_access::may_see`), proven by the
//!   session's upload token in the `X-Upload-Token` header (the desktop app, which fetches every
//!   file itself) or by a room ticket in the address, `?t=` (the website, whose pictures, players
//!   and Download links cannot send a header). Anything else is refused in words (401).
//! - A ROOM TICKET ([`room_ticket`], `POST /api/uploads/room-ticket` with the upload token in the
//!   header) opens the files of one room for one person for [`TICKET_LIFE_SECS`], and is checked
//!   again at every request against that person's right to read the room, so a ban or a room closed
//!   to them ends it at once. It is stateless: the room, a fingerprint of the person and the time
//!   it runs out, under a keyed BLAKE3 with a key drawn at each start (a restart ends every ticket,
//!   which only means a page asks for a new one). The person's fingerprint is the one
//!   `server_members.did_fp` holds; no key is in the address.
//!
//! The website's own Files page items keep their own route and tickets (`files.rs`,
//! `files_ticket.rs`); this is the chat's.

use std::sync::{Arc, OnceLock};

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;

/// How long a room ticket opens the room's files.
pub const TICKET_LIFE_SECS: u64 = 3600;

/// The words a refused file answers with.
pub const REFUSED: &str = "This file was posted in a members-only room. Open it from that room, signed in.";

/// The stored names an upload line links to: what follows each `/uploads/` up to a space, a quote,
/// a bracket, `?` or `#`, when it is a plain file name (no folder).
pub fn linked_names(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = content;
    while let Some(at) = rest.find("/uploads/") {
        rest = &rest[at + "/uploads/".len()..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | '(' | '[' | ']' | '?' | '#'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if !name.is_empty() && !name.contains('/') && !name.contains('\\') && !name.starts_with('.') && !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// Bind the uploads of `poster` that `content` links to to `room`, the room the line was kept in:
/// a file not yet bound takes the room, and a room that reads at Anyone takes it whatever it had.
pub fn note_posted(db: &Storage, poster: &str, content: &str, room: &str) {
    let names = linked_names(content);
    if names.is_empty() || room.is_empty() {
        return;
    }
    let public = crate::relay::chat_access::levels(db, room).0 == humanity_protocol::chat_access::ReadLevel::Anyone;
    for name in names {
        if let Err(e) = db.bind_upload_room(poster, &name, room, public) {
            tracing::warn!("upload rooms: could not bind a file to its room: {e}");
        }
    }
}

/// The room a stored file is bound to when that room does not read at Anyone now: the files only
/// its readers may open.
pub fn members_only_room(db: &Storage, filename: &str) -> Option<String> {
    let room = db.upload_room(filename)?;
    let (read, _, _) = crate::relay::chat_access::levels(db, &room);
    (read != humanity_protocol::chat_access::ReadLevel::Anyone).then_some(room)
}

/// The key this process signs its room tickets with, drawn at the first use after each start.
fn ticket_key() -> &'static [u8; 32] {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| {
        let mut k = [0u8; 32];
        getrandom::getrandom(&mut k).expect("the operating system's random source");
        k
    })
}

/// The fingerprint of a key that `server_members.did_fp` holds: hex of BLAKE3(key)[..16].
fn fingerprint(key: &str) -> Option<String> {
    let pk = hex::decode(key).ok()?;
    Some(crate::relay::storage::author_fingerprint(&pk))
}

fn mac(room: &str, fp: &str, until: u64) -> String {
    let mut h = blake3::Hasher::new_keyed(ticket_key());
    h.update(b"hum/room-ticket/v1\n");
    h.update(room.as_bytes());
    h.update(b"\n");
    h.update(fp.as_bytes());
    h.update(b"\n");
    h.update(until.to_string().as_bytes());
    h.finalize().to_hex().to_string()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// A ticket that opens `room`'s files for `key` until `now + TICKET_LIFE_SECS`.
pub fn mint_ticket(room: &str, key: &str, now: u64) -> Option<String> {
    let fp = fingerprint(key)?;
    let until = now + TICKET_LIFE_SECS;
    Some(format!("{until}.{fp}.{}", mac(room, &fp, until)))
}

/// The key of the person `ticket` was given to, when it is for `room`, has not run out at `now`,
/// and its fingerprint is a member's here.
pub fn ticket_holder(db: &Storage, ticket: &str, room: &str, now: u64) -> Option<String> {
    let mut parts = ticket.splitn(3, '.');
    let (until, fp, given) = (parts.next()?, parts.next()?, parts.next()?);
    let until: u64 = until.parse().ok()?;
    if until < now || fp.len() != 32 || !fp.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let want = mac(room, fp, until);
    // Compared in constant time, so a guess learns nothing from how long the answer took.
    if !subtle_eq(want.as_bytes(), given.as_bytes()) {
        return None;
    }
    db.member_key_by_fingerprint(fp)
}

fn subtle_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The `?t=` of an address, if it has one.
fn ticket_in(query: Option<&str>) -> Option<String> {
    query?.split('&').find_map(|kv| kv.strip_prefix("t=")).map(|t| t.to_string())
}

/// May the request open the file at `path` (under `/uploads`)? A file bound to a members-only room
/// needs the upload token of someone who may read it, or that room's ticket.
pub async fn admitted(state: &RelayState, path: &str, query: Option<&str>, headers: &HeaderMap) -> bool {
    let name = path.trim_start_matches('/');
    // A video's poster picture is `<file>.poster.jpg`: it follows its video.
    let file = name.strip_suffix(".poster.jpg").unwrap_or(name);
    let Some(room) = members_only_room(&state.db, file) else { return true };
    let key = match crate::relay::files::header_token(headers) {
        Some(token) => state.upload_tokens.read().await.get(&token).cloned(),
        None => ticket_in(query).and_then(|t| ticket_holder(&state.db, &t, &room, now_secs())),
    };
    key.is_some_and(|k| crate::relay::chat_access::may_see(&state.db, &k, &room))
}

/// The layer in front of `/uploads` ([`admitted`]): passes the request on to the files, or refuses.
pub async fn gate(State(state): State<Arc<RelayState>>, req: Request, next: Next) -> Response {
    let ok = admitted(&state, req.uri().path(), req.uri().query(), req.headers()).await;
    if !ok {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": REFUSED }))).into_response();
    }
    next.run(req).await
}

#[derive(Debug, Deserialize)]
pub struct RoomTicketBody {
    pub channel: String,
}

/// `POST /api/uploads/room-ticket` with the session's upload token in `X-Upload-Token` and
/// `{channel}`: a ticket that opens that room's files for this person ([`TICKET_LIFE_SECS`]), when
/// they may read the room (403 otherwise).
pub async fn room_ticket(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(b): Json<RoomTicketBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let key = crate::relay::upload_rules::token_key(&state, crate::relay::files::header_token(&headers).as_deref()).await?;
    if !crate::relay::chat_access::may_see(&state.db, &key, &b.channel) {
        return Err((StatusCode::FORBIDDEN, "You may not read that room.".into()));
    }
    let ticket = mint_ticket(&b.channel, &key, now_secs()).ok_or((StatusCode::BAD_REQUEST, "No ticket for this key.".to_string()))?;
    Ok(Json(json!({ "ticket": ticket, "expires_in_s": TICKET_LIFE_SECS })))
}

#[cfg(test)]
#[path = "upload_rooms_tests.rs"]
mod tests;

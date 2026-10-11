//! Tickets for a members-only Media file (R3-8, 2026-10-11; docs/design/media-showcase.md,
//! "Round 3: fixes from the round 2 review").
//!
//! WHY. A members-only item's file is served only to a member, proven by the session's upload
//! token (media.rs `file_core`). The apps send that token in a header and show a picture from
//! memory. A video cannot be shown that way: a player streams it from an address with range
//! requests, and a big download is saved from an address too. Putting the token in such an address
//! would put the key to every upload of the session in a browser's history, its downloads list and
//! the server's logs. A ticket is an address for ONE file that works for ten minutes and says
//! nothing about the session.
//!
//! THE ROUTE. `POST /api/media/ticket` with the session's upload token in `X-Upload-Token` (never
//! in an address) and the body `{id, filename}`: the item's number and its stored file name, as
//! every write names an item (W5, media.rs `same_item`). For a member who is not banned it answers
//! `{url: "/api/media/file/<id>?ticket=<ticket>", expires_in_s: 600}`, and the file route then
//! serves that one item's file for that ticket, range requests included, until it runs out.
//!
//! A TICKET is 32 random bytes in hex (256 bits, from the operating system's random source), bound
//! to the item's number, the item's file name and the member it was given to. It is refused after
//! ten minutes, for any other item, when the item's file is no longer that file, and once that
//! person is no longer a member or is banned. Kept in memory only (`RelayState::media_tickets`): a
//! restart ends every ticket, which only means an app asks for a new one.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::relay::media::{header_token, is_member, same_item, stored_name};
use crate::relay::relay::RelayState;
use crate::relay::storage::media::MediaItem;

type Refused = (StatusCode, String);

/// How long a ticket opens its file.
pub const TICKET_LIFE: Duration = Duration::from_secs(600);
/// The most tickets one person may hold at once: an app asks for one each time a video is played
/// or a big file is downloaded, far fewer than this in ten minutes.
pub const TICKETS_PER_PERSON: usize = 256;
/// The most tickets the relay holds at once, everyone's: a bound on the memory they take.
pub const TICKETS_MAX: usize = 50_000;

/// One ticket: the item it opens, that item's file name, whom it was given to, and until when.
#[derive(Debug, Clone)]
struct Ticket {
    item: i64,
    filename: String,
    key: String,
    until: Instant,
}

/// Every live ticket, by its random value. In memory only.
#[derive(Debug, Default)]
pub struct Tickets {
    live: std::sync::Mutex<HashMap<String, Ticket>>,
}

impl Tickets {
    /// A new ticket for `key` to open item `item`, whose file is `filename`, given at `now`.
    /// Tickets that have run out are dropped first. Refused when the person already holds
    /// [`TICKETS_PER_PERSON`], or the relay [`TICKETS_MAX`].
    pub fn issue(&self, key: &str, item: i64, filename: &str, now: Instant) -> Result<String, Refused> {
        let mut live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        live.retain(|_, t| t.until > now);
        if live.values().filter(|t| t.key == key).count() >= TICKETS_PER_PERSON || live.len() >= TICKETS_MAX {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Too many members-only files have been opened from this account in the last ten minutes. Wait a few minutes and open it again.".to_string(),
            ));
        }
        let mut raw = [0u8; 32];
        getrandom::getrandom(&mut raw).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to make a ticket: {e}")))?;
        let ticket = hex::encode(raw);
        live.insert(ticket.clone(), Ticket { item, filename: filename.to_string(), key: key.to_string(), until: now + TICKET_LIFE });
        Ok(ticket)
    }

    /// What `ticket` was given for, when it is still live at `now` and was given for item `item`:
    /// the item's file name then, and whom it was given to.
    fn lookup(&self, ticket: &str, item: i64, now: Instant) -> Option<(String, String)> {
        let live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        live.get(ticket).filter(|t| t.until > now && t.item == item).map(|t| (t.filename.clone(), t.key.clone()))
    }

    /// How many tickets are live (tests).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.live.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

/// What a refused ticket is told: one sentence, the same whatever was wrong with it, so a guess
/// learns nothing.
pub const TICKET_REFUSED: &str = "That link to a members-only file has run out (links last ten minutes) or is not for this file. Open the file again from the Media page.";

/// The item `ticket` opens at `now`, if it does: live, given for item `id`, that item's file still
/// the one it was given for, and the person it was given to still a member who is not banned.
pub fn admitted(state: &RelayState, ticket: &str, id: i64, now: Instant) -> Result<MediaItem, Refused> {
    let refused = || (StatusCode::FORBIDDEN, TICKET_REFUSED.to_string());
    let (filename, key) = state.media_tickets.lookup(ticket, id, now).ok_or_else(refused)?;
    if !is_member(&state.db, &key) {
        return Err(refused());
    }
    let item = state
        .db
        .media_item(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("storage: {e}")))?
        .ok_or_else(refused)?;
    if item.upload_filename != filename {
        return Err(refused());
    }
    Ok(item)
}

/// `POST /api/media/ticket`'s body: the item's number and its stored file name (W5).
#[derive(Debug, Default, Deserialize)]
pub struct TicketBody {
    pub id: i64,
    #[serde(default)]
    pub filename: String,
}

pub async fn ticket(State(state): State<Arc<RelayState>>, headers: HeaderMap, Json(b): Json<TicketBody>) -> Result<Json<Value>, Refused> {
    let token = header_token(&headers).ok_or_else(|| {
        (StatusCode::UNAUTHORIZED, "Ask for a ticket with this session's upload token in the X-Upload-Token header.".to_string())
    })?;
    let key = state
        .upload_tokens
        .read()
        .await
        .get(&token)
        .cloned()
        .ok_or_else(|| (StatusCode::FORBIDDEN, "That upload token is not a session signed in to this server.".to_string()))?;
    ticket_core(&state, &key, b.id, &b.filename, Instant::now())
}

/// A ticket for `key` to open item `id` (whose file `filename` must name, W5), given at `now`: only
/// to a member of this server who is not banned, the one rule for opening members-only files.
pub fn ticket_core(state: &RelayState, key: &str, id: i64, filename: &str, now: Instant) -> Result<Json<Value>, Refused> {
    if !is_member(&state.db, key) {
        return Err((StatusCode::FORBIDDEN, "Only members of this server who are not banned can open members-only Media files.".to_string()));
    }
    same_item(&state.db, id, filename)?;
    let ticket = state.media_tickets.issue(key, id, stored_name(filename)?, now)?;
    Ok(Json(json!({ "url": format!("/api/media/file/{id}?ticket={ticket}"), "expires_in_s": TICKET_LIFE.as_secs() })))
}

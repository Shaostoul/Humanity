//! Tickets for a members-only Media file (R3-8, 2026-10-11; docs/design/files-showcase.md,
//! "Round 3: fixes from the round 2 review"; the pool and its memory, R4-1 in round 4).
//!
//! WHY. A members-only item's file is served only to a member, proven by the session's upload
//! token (media.rs `file_core`). The apps send that token in a header and show a picture from
//! memory. A video cannot be shown that way: a player streams it from an address with range
//! requests, and a big download is saved from an address too. Putting the token in such an address
//! would put the key to every upload of the session in a browser's history, its downloads list and
//! the server's logs. A ticket is an address for ONE file that works for ten minutes and says
//! nothing about the session.
//!
//! THE ROUTE. `POST /api/files/ticket` with the session's upload token in `X-Upload-Token` (never
//! in an address) and the body `{id, filename, file, download}`: the item's number and its own
//! stored file name, as every write names an item (W5, media.rs `same_item`), which of its files
//! (round 5, 2026-10-12: 0, the default, is the item's own; any other an added file's or the
//! cover's number), and whether the address is the counted download route. For a member who is not
//! banned it answers `{url: "/api/files/file/<id>?file=<n>&ticket=<ticket>", expires_in_s: 600}`
//! (or `/api/files/download/...` with `download`), and the route then serves that one file for that
//! ticket, range requests included, until it runs out.
//!
//! A TICKET is 32 random bytes in hex (256 bits, from the operating system's random source), bound
//! to the item's number, the stored name of the one file it opens and the member it was given to.
//! It is refused after ten minutes, for any other item or file, when that file is no longer the
//! one it was given for, and once that person is no longer a member or is banned. Kept in memory
//! only (`RelayState::media_tickets`): a
//! restart ends every ticket, which only means an app asks for a new one.
//!
//! THE POOL (R4-1, 2026-10-11). One person holds at most [`TICKETS_PER_PERSON`] at once. The relay
//! holds at most [`TICKETS_MAX`], and when that is full it drops its OLDEST ticket rather than
//! refusing anyone, so no number of accounts can lock the others out. A ticket keeps a short
//! fingerprint of the person's key and their member row, never the key: 128 bytes a ticket
//! ([`BYTES_PER_TICKET`]), so even a full pool of 500,000 is about 140 MB with the tables' spare
//! room, where round 3's 3,904-character keys made its pool of 50,000 about 200 MB.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::relay::files::{header_token, is_member, same_item};
use crate::relay::relay::RelayState;
use crate::relay::storage::files::FilesItem;

type Refused = (StatusCode, String);

/// How long a ticket opens its file.
pub const TICKET_LIFE: Duration = Duration::from_secs(600);
/// The most tickets one person may hold at once (R4-1, 2026-10-11): an app asks for one each time
/// a video is played or a big file is downloaded, far fewer than this in ten minutes. Round 3 let
/// one person hold 256, so about 196 accounts filled the old pool of 50,000.
pub const TICKETS_PER_PERSON: usize = 16;
/// The most tickets the relay holds at once, everyone's (R4-1): a bound on the memory they take.
/// When it is full the OLDEST ticket is dropped to make room, so a full pool never refuses anyone:
/// no number of accounts asking for tickets can lock other members out of members-only video and
/// downloads. The person whose old ticket went asks for a new one, as after a restart.
pub const TICKETS_MAX: usize = 500_000;

/// A short fingerprint: the first 16 bytes of a BLAKE3 hash. Enough to tell one key, or one file
/// name, from another, by accident or on purpose; it is never sent anywhere.
type Print = [u8; 16];

/// The fingerprint of `text`, under `what` (a key and a file name never share one).
fn print(what: &str, text: &str) -> Print {
    let mut h = blake3::Hasher::new();
    h.update(b"hum/media-ticket/v1\n");
    h.update(what.as_bytes());
    h.update(b"\n");
    h.update(text.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

/// One ticket, 64 bytes and nothing on the heap (R4-1): the item it opens, a fingerprint of that
/// item's file name, the member it was given to (their member row and a fingerprint of their key)
/// and until when. Round 3 kept the whole key in every ticket, 3,904 characters of Dilithium3
/// public key in hex: about 200 MB with its pool full.
#[derive(Debug, Clone, Copy)]
struct Ticket {
    item: i64,
    file: Print,
    member_row: i64,
    person: Print,
    until: Instant,
}

/// What one ticket takes in memory, in bytes (R4-1): its value as the map's key, the ticket, and
/// its value again in the oldest-first queue; 128. The tables' spare room comes on top (a hash
/// table holding 500,000 has room for about a million), so a full pool is about 140 MB at most,
/// most of it the map of tickets.
pub const BYTES_PER_TICKET: usize = 2 * std::mem::size_of::<[u8; 32]>() + std::mem::size_of::<Ticket>();

/// The live tickets: by value, oldest first, and how many each person holds.
#[derive(Debug, Default)]
struct Live {
    by_value: HashMap<[u8; 32], Ticket>,
    /// Every ticket's value in the order given. A ticket lives exactly [`TICKET_LIFE`] and the
    /// relay's clock only goes forward, so this is also the order they run out in.
    oldest_first: VecDeque<[u8; 32]>,
    /// How many each person holds, by the fingerprint of their key (at most [`TICKETS_PER_PERSON`]).
    per_person: HashMap<Print, u16>,
}

impl Live {
    /// Forget the oldest ticket. Whether there was one.
    fn drop_oldest(&mut self) -> bool {
        let Some(value) = self.oldest_first.pop_front() else { return false };
        if let Some(t) = self.by_value.remove(&value) {
            if let Some(n) = self.per_person.get_mut(&t.person) {
                *n -= 1;
                if *n == 0 {
                    self.per_person.remove(&t.person);
                }
            }
        }
        true
    }

    /// Forget every ticket that has run out by `now`, from the oldest.
    fn drop_run_out(&mut self, now: Instant) {
        while let Some(value) = self.oldest_first.front() {
            if self.by_value.get(value).is_some_and(|t| t.until > now) {
                break;
            }
            self.drop_oldest();
        }
    }
}

/// Every live ticket, by its random value. In memory only.
#[derive(Debug)]
pub struct Tickets {
    live: std::sync::Mutex<Live>,
    /// The most held at once: [`TICKETS_MAX`] (a test sets a smaller pool).
    room: usize,
}

impl Default for Tickets {
    fn default() -> Self {
        Tickets::with_room(TICKETS_MAX)
    }
}

impl Tickets {
    /// A pool that holds at most `room` tickets at once.
    pub fn with_room(room: usize) -> Self {
        Tickets { live: Default::default(), room: room.max(1) }
    }

    /// A new ticket for the member `key` (whose member row is `member_row`) to open item `item`,
    /// whose file is `filename`, given at `now`. Tickets that have run out are dropped first.
    /// Refused (429) when the person already holds [`TICKETS_PER_PERSON`]; a full pool drops its
    /// oldest ticket instead of refusing (R4-1).
    pub fn issue(&self, key: &str, member_row: i64, item: i64, filename: &str, now: Instant) -> Result<String, Refused> {
        let person = print("key", key);
        let mut live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        live.drop_run_out(now);
        if usize::from(live.per_person.get(&person).copied().unwrap_or(0)) >= TICKETS_PER_PERSON {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Too many members-only files have been opened from this account in the last ten minutes. Wait a few minutes and open it again.".to_string(),
            ));
        }
        while live.by_value.len() >= self.room && live.drop_oldest() {}
        let mut raw = [0u8; 32];
        getrandom::getrandom(&mut raw).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to make a ticket: {e}")))?;
        let ticket = Ticket { item, file: print("file", filename), member_row, person, until: now + TICKET_LIFE };
        live.by_value.insert(raw, ticket);
        live.oldest_first.push_back(raw);
        *live.per_person.entry(person).or_insert(0) += 1;
        Ok(hex::encode(raw))
    }

    /// What `ticket` was given for, when it is still live at `now` and was given for item `item`.
    fn lookup(&self, ticket: &str, item: i64, now: Instant) -> Option<Ticket> {
        let mut value = [0u8; 32];
        hex::decode_to_slice(ticket, &mut value).ok()?;
        let live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        live.by_value.get(&value).filter(|t| t.until > now && t.item == item).copied()
    }

    /// How many tickets are held (tests).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.live.lock().unwrap_or_else(|p| p.into_inner()).by_value.len()
    }

    /// Whether `ticket` is still held, not dropped (tests).
    #[cfg(test)]
    pub(crate) fn holds(&self, ticket: &str) -> bool {
        let mut value = [0u8; 32];
        hex::decode_to_slice(ticket, &mut value).is_ok() && self.live.lock().unwrap_or_else(|p| p.into_inner()).by_value.contains_key(&value)
    }
}

// A ticket holds nothing on the heap (R4-1): it is `Copy`, which no type owning heap memory (a
// String, a Vec, a Box) can be. Checked when the relay compiles.
const _: fn() = || {
    fn copy<T: Copy>() {}
    copy::<Ticket>();
};

/// What a refused ticket is told: one sentence, the same whatever was wrong with it, so a guess
/// learns nothing.
pub const TICKET_REFUSED: &str = "That link to a members-only file has run out (links last ten minutes) or is not for this file. Open the file again from the Files page.";

/// The item `ticket` opens at `now` as its file `file`, if it does: live, given for item `id`,
/// that file's stored name still the one the ticket was given for (round 5: a ticket opens the one
/// file it was given for, the item's own or an added one), and the person it was given to still a
/// member who is not banned. The ticket holds no key (R4-1): the person is found by their member
/// row, and the key there must have the ticket's fingerprint, so a row number given to someone else
/// later never inherits a ticket.
pub fn admitted(state: &RelayState, ticket: &str, id: i64, file: i64, now: Instant) -> Result<FilesItem, Refused> {
    admitted_as(state, ticket, id, file, now).map(|(item, _key)| item)
}

/// [`admitted`], and the key of the member the ticket was given to: the counted download route
/// counts a members-only download by that key, once a day (relay/files_counts.rs, Media round 5).
/// The key is looked up from the member row, as above; the ticket itself still holds none.
pub fn admitted_as(state: &RelayState, ticket: &str, id: i64, file: i64, now: Instant) -> Result<(FilesItem, String), Refused> {
    let refused = || (StatusCode::FORBIDDEN, TICKET_REFUSED.to_string());
    let t = state.media_tickets.lookup(ticket, id, now).ok_or_else(refused)?;
    let key = state.db.member_key_at_row(t.member_row).ok_or_else(refused)?;
    if print("key", &key) != t.person || !is_member(&state.db, &key) {
        return Err(refused());
    }
    let item = state
        .db
        .media_item(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("storage: {e}")))?
        .ok_or_else(refused)?;
    match item.file_name(file) {
        Some(name) if print("file", name) == t.file => Ok((item, key)),
        _ => Err(refused()),
    }
}

/// `POST /api/files/ticket`'s body: the item's number and its own stored file name (W5), which of
/// its files the ticket opens (round 5; 0, the item's own, when left out), and whether the address
/// is the counted download route instead of the members' file route.
#[derive(Debug, Default, Deserialize)]
pub struct TicketBody {
    pub id: i64,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub file: i64,
    #[serde(default)]
    pub download: bool,
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
    ticket_core(&state, &key, &b, Instant::now())
}

/// A ticket for `key` to open file `b.file` of item `b.id` (whose own file `b.filename` must name,
/// W5), given at `now`: only to a member of this server who is not banned, the one rule for opening
/// members-only files. The ticket is bound to that one file's stored name. Its address is the
/// members' file route, `/api/files/file/<id>?file=<n>&ticket=...`, or with `download` the counted
/// download route, `/api/files/download/<id>?file=<n>&ticket=...` (round 5).
pub fn ticket_core(state: &RelayState, key: &str, b: &TicketBody, now: Instant) -> Result<Json<Value>, Refused> {
    let not_member = || (StatusCode::FORBIDDEN, "Only members of this server who are not banned can open members-only files on the Files page.".to_string());
    if !is_member(&state.db, key) {
        return Err(not_member());
    }
    let member_row = state.db.member_row(key).ok_or_else(not_member)?;
    let id = b.id;
    same_item(&state.db, id, &b.filename)?;
    let item = state
        .db
        .media_item(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("storage: {e}")))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "There is no item with that number on the Files page.".to_string()))?;
    let name = item.file_name(b.file).ok_or_else(|| (StatusCode::NOT_FOUND, "That item has no file with that number.".to_string()))?;
    let ticket = state.media_tickets.issue(key, member_row, id, name, now)?;
    let route = if b.download { "download" } else { "file" };
    Ok(Json(json!({ "url": format!("/api/files/{route}/{id}?file={}&ticket={ticket}", b.file), "expires_in_s": TICKET_LIFE.as_secs() })))
}

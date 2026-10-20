//! Views and downloads on the Files page, counted once a person a UTC day (Media round 5,
//! 2026-10-12; docs/design/media-round-5-plan.md 3.5, decisions 4 and 5).
//!
//! WHO A PERSON IS, AND WHAT IS KEPT. A person is their key (a ticket's, or a token's), else their
//! network address. Whether they were counted today is a MARK: the first 16 bytes of BLAKE3, keyed
//! with a secret drawn at random for each UTC day and held only in memory, over
//! `hum/media-count/v1\n<view|download>\n<item>\n<m:key | a:address>`. At UTC midnight the marks
//! are forgotten and the secret is replaced, so nothing about who looked outlives the day, and
//! only the numbers (`media_items.views`, `.downloads`) are written to disk. A restart forgets the
//! marks too, so a person can be counted twice on the day of a restart: an overcount by one, never
//! a record of who. At most [`MARKS_PER_DAY`] marks a day (about 32 MB); past that nothing more is
//! counted that day, with one log line.
//!
//! THE ADDRESS is the `X-Real-IP` header, which nginx sets from the connection itself
//! (`proxy_set_header X-Real-IP $remote_addr`, scripts/nginx/humanity.conf and the self-hosting
//! guide). It is NEVER `X-Forwarded-For`: nginx appends to that one whatever the client already
//! wrote in it, so anyone could make themselves a new person on every request. A request without a
//! usable `X-Real-IP` counts as one shared "unknown" person, which can only undercount. An IPv6
//! address counts as its /64 network, since one home is usually handed a whole /64 and could
//! otherwise count again from every address in it ([`address_of`]).
//!
//! THE ROUTES (lane 1B):
//! - `GET /api/files/download/{id}?file=<n>[&ticket=]`: file `n` of item `id` (0, the default, is
//!   the item's own; any other an added file's or the cover's number) as an attachment named by
//!   the name it had on its uploader's device ([`content_disposition`]). Anyone may download a
//!   public item on a public Files page; a members-only one takes a ticket (relay/files_ticket.rs,
//!   asked with `download: true`) or the session's upload token in `X-Upload-Token`. Everything
//!   else gets the same 404 words ([`NO_SUCH_FILE`]), so a guess learns nothing. It counts one
//!   download when the request starts at the file's first byte (no Range, or a Range from byte 0,
//!   so a video player's later pieces and a resumed download count nothing) and this person was
//!   not counted for that item today.
//! - `POST /api/files/viewed` with `{id}`: one view, counted the same way, for an item the asker
//!   may see (a members-only one needs the session's upload token in `X-Upload-Token`). It always
//!   answers `{ok: true}`, so it never says whether an item exists.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;

use crate::relay::files::{header_token, is_member, media_settings, serve_item_file, Folders, TokenQuery};
use crate::relay::relay::RelayState;
use crate::relay::storage::files::FilesItem;
use crate::relay::storage::ServerSettings;

/// The most marks kept in one day, everyone's: a bound on their memory.
pub const MARKS_PER_DAY: usize = 500_000;

/// What is counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Count {
    View,
    Download,
}

impl Count {
    fn word(self) -> &'static str {
        match self {
            Count::View => "view",
            Count::Download => "download",
        }
    }
}

/// Who is counted: a key (a member, through a ticket or the session's token), else a network
/// address (the `X-Real-IP` nginx sets; never `X-Forwarded-For`, which a client can write). A
/// request with neither counts as one shared unknown address: an undercount, never an overcount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who<'a> {
    Key(&'a str),
    Address(&'a str),
}

/// One day's marks: the day (days since 1970, UTC), its secret, and the marks.
struct Day {
    day: i64,
    secret: [u8; 32],
    seen: HashSet<[u8; 16]>,
    /// The day's "full" line was logged.
    full_said: bool,
}

/// The day's marks, in memory only (`RelayState::media_marks`).
pub struct Marks {
    today: std::sync::Mutex<Option<Day>>,
    cap: usize,
}

impl Default for Marks {
    fn default() -> Self {
        Marks::with_cap(MARKS_PER_DAY)
    }
}

impl std::fmt::Debug for Marks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Marks").field("cap", &self.cap).finish_non_exhaustive() // never the secret
    }
}

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

impl Marks {
    /// Marks that hold at most `cap` a day (a test sets a small one).
    pub fn with_cap(cap: usize) -> Self {
        Marks { today: std::sync::Mutex::new(None), cap: cap.max(1) }
    }

    /// Whether `who` is counted for `what` on item `item` at `now_ms` (Unix ms): true the first time
    /// today, false after that, and false for everyone new once the day holds its cap. A new UTC day
    /// forgets every mark and draws a new secret first.
    pub fn first_today(&self, what: Count, item: i64, who: Who, now_ms: i64) -> bool {
        let day = now_ms.div_euclid(DAY_MS);
        let mut today = self.today.lock().unwrap_or_else(|p| p.into_inner());
        if today.as_ref().is_none_or(|d| d.day != day) {
            let mut secret = [0u8; 32];
            if getrandom::getrandom(&mut secret).is_err() {
                // No secret, no mark: count nothing rather than keep marks anyone could work out.
                tracing::error!("Files counts: no random secret for the day; nothing is counted");
                return false;
            }
            *today = Some(Day { day, secret, seen: HashSet::new(), full_said: false });
        }
        let Some(d) = today.as_mut() else { return false };
        let mark = mark_of(&d.secret, what, item, who);
        if d.seen.contains(&mark) {
            return false;
        }
        if d.seen.len() >= self.cap {
            if !d.full_said {
                d.full_said = true;
                tracing::warn!("Files counts: {} marks today, the most a day; nothing more is counted until midnight UTC", self.cap);
            }
            return false;
        }
        d.seen.insert(mark)
    }

    /// How many marks today holds (tests).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.today.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map_or(0, |d| d.seen.len())
    }
}

/// The mark of `who` for `what` on `item` under the day's `secret`.
fn mark_of(secret: &[u8; 32], what: Count, item: i64, who: Who) -> [u8; 16] {
    let mut h = blake3::Hasher::new_keyed(secret);
    h.update(b"hum/media-count/v1\n");
    h.update(what.word().as_bytes());
    h.update(b"\n");
    h.update(item.to_string().as_bytes());
    h.update(b"\n");
    match who {
        Who::Key(k) => {
            h.update(b"m:");
            h.update(k.as_bytes());
        }
        Who::Address(a) => {
            h.update(b"a:");
            h.update(a.as_bytes());
        }
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

// ── Who a person is ──

/// The header nginx sets from the connection: `proxy_set_header X-Real-IP $remote_addr;`.
pub const REAL_IP_HEADER: &str = "x-real-ip";

/// What a request without a usable address counts as: one person, shared by every such request.
pub const UNKNOWN_ADDRESS: &str = "unknown";

/// The address a request counts by: its `X-Real-IP` header and nothing else (never
/// `X-Forwarded-For`, which the client can write). Written the one way Rust writes an address, so
/// `203.0.113.7` and ` 203.0.113.7 ` are one person. An IPv6 address counts as its /64 network
/// (`2001:db8:1:2::/64`), and an IPv4 address written as IPv6 (`::ffff:203.0.113.7`) as the IPv4
/// address. Missing, or not an address: [`UNKNOWN_ADDRESS`].
pub fn address_of(headers: &HeaderMap) -> String {
    let given = headers.get(REAL_IP_HEADER).and_then(|v| v.to_str().ok()).map(str::trim);
    let Some(ip) = given.and_then(|t| t.parse::<IpAddr>().ok()) else {
        return UNKNOWN_ADDRESS.to_string();
    };
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
    }
}

/// Whether everyone may see `item`: it is public, and so is the server's Files page (anything but
/// "public" counts as members only, as everywhere on the Files page). The download route, the
/// view count, an item's own page and its preview picture all ask this one question
/// (relay/files_page.rs too).
pub(crate) fn shown_to_everyone(item: &FilesItem, s: &ServerSettings) -> bool {
    item.visibility == "public" && s.media_visibility == "public"
}

/// The member a request's `X-Upload-Token` header names, when it names a session signed in to this
/// server whose person is a member who is not banned.
async fn member_of(state: &RelayState, headers: &HeaderMap) -> Option<String> {
    let token = header_token(headers)?;
    let key = state.upload_tokens.read().await.get(&token).cloned()?;
    is_member(&state.db, &key).then_some(key)
}

/// Item `id`, or none: a storage error is logged and counts as no item, so the answer is the same.
fn item_or_none(state: &RelayState, id: i64) -> Option<FilesItem> {
    state.db.media_item(id).unwrap_or_else(|e| {
        tracing::error!("Files counts: item {id} could not be read ({e})");
        None
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

// ── GET /api/files/download/{id}?file=<n>[&ticket=] ──

/// What every refused download is told, whatever was wrong: no such item, no such file, a
/// members-only item without a ticket or a token, a ticket that ran out. One sentence, so a guess
/// learns nothing about which.
pub const NO_SUCH_FILE: &str = "There is no such file on the Files page, or it is not yours to open.";

fn no_such_file() -> Response {
    (StatusCode::NOT_FOUND, NO_SUCH_FILE).into_response()
}

/// `GET /api/files/download/{id}?file=<n>[&ticket=]`: file `n` of item `id` as an attachment named
/// by its original name, counted once a person a day.
pub async fn download(State(state): State<Arc<RelayState>>, Path(id): Path<i64>, Query(q): Query<TokenQuery>, req: axum::extract::Request) -> Response {
    download_from(&state, Folders::live(), id, q, req, now_ms()).await
}

/// [`download`] with the folders and the time named (the tests' way in).
///
/// Who may have the file, in order: a ticket given for that very file (relay/files_ticket.rs); a
/// refused ticket falls back to the session's token when the request has one, as the members' file
/// route does. Then the session's upload token in `X-Upload-Token`, for a member who is not banned:
/// any item. Then anyone: a public item on a public Files page. A ticket and a token count by the
/// member's key; anyone else by their address. `?token=` in the address is not read here: a token
/// in an address ends up in browser histories and server logs (W3b).
pub async fn download_from(state: &RelayState, folders: Folders<'_>, id: i64, q: TokenQuery, req: axum::extract::Request, now_ms: i64) -> Response {
    let file_no = q.file.unwrap_or(0);
    let token_holder = member_of(state, req.headers()).await;
    let opened: Option<(FilesItem, Option<String>)> = match q.ticket.as_deref().filter(|t| !t.is_empty()) {
        Some(ticket) => match crate::relay::files_ticket::admitted_as(state, ticket, id, file_no, std::time::Instant::now()) {
            Ok((item, key)) => Some((item, Some(key))),
            Err(_) => None,
        },
        None => None,
    };
    let opened = match opened {
        Some(o) => Some(o),
        None => match token_holder {
            // A member: any item, members-only ones included.
            Some(key) => item_or_none(state, id).map(|item| (item, Some(key))),
            // Anyone else: only a public item on a public page, and only without a ticket that was
            // refused (a bad ticket with no token is simply refused, not served as the public).
            None if q.ticket.as_deref().is_none_or(str::is_empty) => {
                let s = media_settings(&state.db);
                item_or_none(state, id).filter(|i| shown_to_everyone(i, &s)).map(|item| (item, None))
            }
            None => None,
        },
    };
    let Some((item, key)) = opened else { return no_such_file() };

    // Read what decides the count before the request goes to the file server.
    let counts = req.method() == Method::GET && from_the_first_byte(req.headers());
    let address = address_of(req.headers());
    let disposition = content_disposition(&download_name(&item, file_no));
    let res = match serve_item_file(folders, &item, file_no, req, disposition).await {
        Ok(res) => res,
        // No file with that number, or its file is missing: the same words as everything else.
        Err(_) => return no_such_file(),
    };
    // Counted only when the file is actually being given (200, or 206 for a range from byte 0),
    // never for a 304, a 416 or a HEAD.
    if counts && res.status().is_success() {
        let who = match key.as_deref() {
            Some(k) => Who::Key(k),
            None => Who::Address(&address),
        };
        if state.media_marks.first_today(Count::Download, item.id, who, now_ms) {
            if let Err(e) = state.db.add_media_download(item.id) {
                tracing::error!("Files counts: a download of item {} could not be counted ({e})", item.id);
            }
        }
    }
    res
}

/// Whether a request asks for a file from its first byte: it has no `Range` header, or the first
/// range it names starts at byte 0 (`bytes=0-`, `bytes=0-499`). A range from anywhere else
/// (`bytes=500-`), a suffix (`bytes=-500`, the last 500 bytes) or a header that cannot be read is
/// a later piece of something already counted: a video player seeking, a download resuming.
pub fn from_the_first_byte(headers: &HeaderMap) -> bool {
    let Some(value) = headers.get(header::RANGE) else { return true };
    let Ok(text) = value.to_str() else { return false };
    let text = text.trim();
    // The unit is case-insensitive (RFC 9110 14.1).
    let Some(spec) = text.get(..6).filter(|u| u.eq_ignore_ascii_case("bytes=")).map(|_| &text[6..]) else {
        return false;
    };
    let first = spec.split(',').next().unwrap_or("").trim();
    let start = first.split('-').next().unwrap_or("").trim();
    !start.is_empty() && start.bytes().all(|b| b == b'0')
}

/// The name file `file_no` of `item` is saved under: the name it had on its uploader's device, else
/// its stored name, else "download".
pub fn download_name(item: &FilesItem, file_no: i64) -> String {
    let original = if file_no == 0 {
        item.original_name.as_str()
    } else {
        item.files.iter().chain(item.cover.iter()).find(|f| f.file == file_no).map_or("", |f| f.original_name.as_str())
    };
    [original, item.file_name(file_no).unwrap_or("")]
        .into_iter()
        .map(clean_name)
        .find(|n| !n.is_empty())
        .unwrap_or_else(|| "download".to_string())
}

/// A file name made safe to save under: no control characters (a line break, a tab, a NUL) and no
/// folder separators, its ends trimmed, at most 200 characters.
fn clean_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| !c.is_control()).map(|c| if c == '/' || c == '\\' { '_' } else { c }).collect();
    kept.trim().chars().take(200).collect()
}

/// The `Content-Disposition` a download is answered with (RFC 6266): an attachment, with the name
/// twice. `filename="..."` is a plain ASCII copy for old browsers, where every character that is
/// not printable ASCII, and every quote, backslash and percent sign, is an underscore;
/// `filename*=UTF-8''...` is the whole name, every byte outside RFC 8187's short list of safe
/// characters percent-encoded. So a quote or a line break in someone's file name can never end the
/// name early or start a header of its own: it never reaches the header as itself.
pub fn content_disposition(name: &str) -> HeaderValue {
    let name = clean_name(name);
    let fallback: String = name.chars().map(|c| if (c.is_ascii_graphic() || c == ' ') && !matches!(c, '"' | '\\' | '%') { c } else { '_' }).collect();
    let mut encoded = String::with_capacity(name.len() * 3);
    for b in name.bytes() {
        // RFC 8187 attr-char: letters, digits and !#$&+-.^_`|~ go as themselves.
        if b.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    HeaderValue::from_str(&format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

// ── POST /api/files/viewed ──

/// `POST /api/files/viewed`'s body: the item's number. Unsigned; the session's upload token in
/// `X-Upload-Token` is needed only to count a members-only item. The answer is always
/// `{ok: true}`, so it never says whether an item exists.
#[derive(Debug, Default, Deserialize)]
pub struct ViewedBody {
    pub id: i64,
}

/// `POST /api/files/viewed`: one view of an item's page, once a person a day.
pub async fn viewed(State(state): State<Arc<RelayState>>, headers: HeaderMap, Json(b): Json<ViewedBody>) -> Response {
    viewed_core(&state, &headers, b.id, now_ms()).await
}

/// [`viewed`] with the time named. A member (the session's upload token) is counted by their key,
/// for any item; anyone else by their address, for a public item on a public page only. Nothing
/// else is counted, and the answer is the same `{ok: true}` whatever happened.
pub async fn viewed_core(state: &RelayState, headers: &HeaderMap, id: i64, now_ms: i64) -> Response {
    let member = member_of(state, headers).await;
    if let Some(item) = item_or_none(state, id) {
        let address = address_of(headers);
        let who = match member.as_deref() {
            Some(k) => Some(Who::Key(k)),
            None if shown_to_everyone(&item, &media_settings(&state.db)) => Some(Who::Address(&address)),
            None => None,
        };
        if let Some(who) = who {
            if state.media_marks.first_today(Count::View, item.id, who, now_ms) {
                if let Err(e) = state.db.add_media_view(item.id) {
                    tracing::error!("Files counts: a view of item {} could not be counted ({e})", item.id);
                }
            }
        }
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

#[cfg(test)]
#[path = "files_counts_tests.rs"]
mod tests;

//! Resumable uploads, the relay half (2026-10-11; docs/design/media-showcase.md, "Resumable
//! uploads, exact"). The operator asked "What if I want to upload a multi-GB file?" One request
//! carries at most 1 GB, and a connection that drops at 900 MB loses all of it. So a Media file
//! can instead be sent in pieces: each piece is written and counted before the next is sent, and
//! after a drop the app asks how far the server got and carries on from there. The approach is the
//! tus protocol's (an open, published protocol for exactly this); the wire format is our own small
//! version of it, so neither app needs a library.
//!
//! THE ROUTES (registered in relay/mod.rs; all under `/api/upload`, so the Uploads switch in
//! relay/features.rs covers them):
//! - `POST /api/upload/resumable?token=` with JSON `{filename, size, content_type, media: 1,
//!   blake3?, keep_metadata?}` starts one: 201 `{id, offset: 0, size, chunk_max}`.
//!   `keep_metadata: 1` (B4, 2026-10-11) is the person's choice to keep the file's hidden
//!   information: the finish skips the picture and video strip.
//! - `GET /api/upload/resumable/{id}?token=`: `{offset, size, chunk_max}`, how far it got, and once
//!   the file is stored `done`: the finished upload's answer (R2: an app whose connection dropped
//!   during the finish finds the stored name here; the row stays until the idle expiry).
//! - `PATCH /api/upload/resumable/{id}?token=&offset=<n>` with the next piece as the body:
//!   `{offset, size}`, or for the last piece the finished upload, the same JSON as
//!   `POST /api/upload` (`{url, filename, size, type}`), which the app then publishes with
//!   `POST /api/media` as before. A PATCH on a finished upload answers that same JSON and does
//!   nothing.
//! - `DELETE /api/upload/resumable/{id}?token=` cancels, and nothing stays (R4-2): `{ok: true}`
//!   for a session still arriving; `{ok: true, removed_stored: true}` for a finished one whose
//!   file was not published (that file is removed too); `{ok: true, published: true}` for one
//!   whose file was published (the item stays); 409 `{busy: true}` while a piece or the finish is
//!   being written ([`cancel_core`]).
//!
//! Every route takes the session's upload token in the `X-Upload-Token` header as well as in
//! `?token=` (R3-9, 2026-10-11; [`session_token`]): the apps send the header.
//!
//! THE ALLOWANCES (B3, 2026-10-11): a start is refused when the person's role has used its hour's
//! uploads or its day's bytes (429 JSON `{error, allowance: "hour"|"day", resets_at, resets_in_s,
//! left_bytes}`); a piece that would pass the day's bytes is refused (429 JSON, the same plus
//! `offset` and `size`), nothing of it written and the session kept, so the rest comes after the
//! reset. `resets_in_s` is the seconds until the reset by the relay's own clock, and `left_bytes`
//! what the day still allows (0 for the hour's, and for a day used up): a piece of that size, or
//! any piece smaller than `chunk_max`, is taken (R3-6). Each piece's bytes are checked and counted
//! together under the room lock when they arrive, and the start's upload likewise (R3-1). A role's
//! largest Media file (B1) holds at the start.
//!
//! WHAT IS CHECKED, AND WHEN. At the start, before anything is written: the token names a person;
//! their role may upload; the same type rules and sharing switches as the single request
//! (upload_rules.rs, one copy shared by both); a picture is at most `PICTURE_MAX_BYTES`; there is
//! room in the Media budget and the person's share counting the uploads already in progress
//! (media.rs `media_room`), so two that fit alone but not together cannot both start; and at most
//! [`OPEN_SESSIONS_MAX`] at once per person. At the end, the role and the switches again (a long
//! upload can outlive a change), the hash if one was given, a picture's magic bytes and metadata
//! strip, and the room once more under its lock (media.rs `store_upload`), which is the check that
//! counts.
//!
//! WHO CAN CARRY ON. A session belongs to the person (the key), not the connection: tokens change
//! on every reconnect, and any current token of the same key continues it. Another person's token
//! gets 404, the same answer as a session that does not exist, so nobody learns that one does.
//!
//! WHEN THE ROOM HAS GONE BY THE END (an admin lowered the budget, or a single request took the
//! space while the pieces arrived): the finish is refused with the room words (507) and the
//! session is KEPT, file and all, for a day. Sending an empty piece at `offset = size` tries the
//! finish again, so a person who makes room does not send gigabytes twice.
//!
//! Everything that touches a file (the piece's append and sync, the hash, the picture's read and
//! strip, the move into place) runs off the async executor, in `spawn_blocking`. Each route's work
//! is a `*_core` function over a relay state and named folders, which is what the tests call
//! (resumable_tests.rs): nothing a test does lands in the checkout's data folder.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::relay::media::{self, Folders, NewUpload, Reserved};
use crate::relay::relay::RelayState;
use crate::relay::storage::media::{OwnedFolders, MEDIA_SHELF};
use crate::relay::storage::media::{MediaError, Refusal};
use crate::relay::storage::upload_sessions::{part_path, remove_session_files, restore_staged_part, valid_id, UploadSession};
use crate::relay::storage::{RoleDef, Storage};
use crate::relay::upload_limits::{picture_too_large, UploadBody, PICTURE_MAX_BYTES};
use crate::relay::upload_rules::{
    allowance_at_start, allowance_for_piece, check_type, count_upload, file_type, finish_file_as, finish_picture_as, media_file_limit,
    media_file_too_large, sharing_check, stored_name, token_key, upload_answer, uploader_role,
};

/// The largest piece the server takes, stated in its answers so neither app hardcodes it.
pub const CHUNK_MAX: usize = 8 * 1024 * 1024;
/// The body limit on the piece route (relay/mod.rs): a full piece plus a little.
pub const PIECE_BODY_LIMIT: usize = CHUNK_MAX + 64 * 1024;
/// A sanity ceiling on one file: the real limit is the room in the Media budget and the person's
/// share, which an admin sets; this only stops a number nobody could mean.
pub const SIZE_CEILING: u64 = 64 * 1024 * 1024 * 1024;
/// The most uploads one person may have in progress at once.
pub const OPEN_SESSIONS_MAX: usize = 3;
/// How long a file's own name and declared type may be when they are kept in the session row.
const NAME_MAX: usize = 255;
const CONTENT_TYPE_MAX: usize = 127;

/// What a refusal adds when the finish was refused for room and the file is kept.
pub const KEPT_WORDS: &str = " The file you sent is kept here for a day, so once there is room it can be finished without sending it again.";

/// A refusal: plain words, as every upload refusal is, or, for a piece that cannot be taken where
/// it was sent (409), JSON saying why and, when it was the wrong place, where the server is.
#[derive(Debug)]
pub enum Refused {
    Words(StatusCode, String),
    Json(StatusCode, Value),
}

impl Refused {
    pub fn status(&self) -> StatusCode {
        match self {
            Refused::Words(code, _) | Refused::Json(code, _) => *code,
        }
    }
    /// The words a person reads: the text, or the JSON's `error`.
    pub fn words(&self) -> String {
        match self {
            Refused::Words(_, words) => words.clone(),
            Refused::Json(_, body) => body["error"].as_str().unwrap_or_default().to_string(),
        }
    }
}

impl From<(StatusCode, String)> for Refused {
    fn from((code, words): (StatusCode, String)) -> Self {
        Refused::Words(code, words)
    }
}

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        match self {
            Refused::Words(code, words) => (code, words).into_response(),
            Refused::Json(code, body) => (code, Json(body)).into_response(),
        }
    }
}

fn words(code: StatusCode, w: impl Into<String>) -> Refused {
    Refused::Words(code, w.into())
}

fn db_err(e: rusqlite::Error) -> Refused {
    words(StatusCode::INTERNAL_SERVER_ERROR, format!("storage: {e}"))
}

fn task_err(e: tokio::task::JoinError) -> Refused {
    words(StatusCode::INTERNAL_SERVER_ERROR, format!("Upload task failed: {e}"))
}

fn not_found() -> Refused {
    words(StatusCode::NOT_FOUND, "There is no upload in progress with that id.")
}

fn now_ms() -> i64 {
    crate::relay::storage::now_millis() as i64
}

// ── One piece at a time per session ──

/// The sessions a piece (or a finish, or a cancel) is being written for right now. A second
/// request for the same session while one is at work gets 409: two pieces written at once could
/// interleave. A handful of entries at most, so a list is enough.
static WRITING: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Holds session `id` as being written until it is dropped.
struct Writing(String);

impl Writing {
    fn begin(id: &str) -> Option<Writing> {
        let mut busy = WRITING.lock().unwrap_or_else(|p| p.into_inner());
        if busy.iter().any(|b| b == id) {
            return None;
        }
        busy.push(id.to_string());
        Some(Writing(id.to_string()))
    }
}

impl Drop for Writing {
    fn drop(&mut self) {
        WRITING.lock().unwrap_or_else(|p| p.into_inner()).retain(|b| b != &self.0);
    }
}

fn busy() -> Refused {
    Refused::Json(
        StatusCode::CONFLICT,
        json!({ "error": "Another piece of this upload is being written. Wait for its answer, then ask where the upload is (GET) and carry on from there.", "busy": true }),
    )
}

// ── POST /api/upload/resumable ──

/// `?token=` (every route) and `?offset=` (a piece).
#[derive(Debug, Default, Deserialize)]
pub struct SessionQuery {
    pub token: Option<String>,
    pub offset: Option<u64>,
}

/// The session's upload token: from the `X-Upload-Token` header when the request has one, else
/// from `?token=` (R3-9, 2026-10-11: an app sends it in the header, so it is never in an address
/// a proxy or a log keeps; the query still works).
pub fn session_token(headers: &HeaderMap, q: &SessionQuery) -> Option<String> {
    media::header_token(headers).or_else(|| q.token.clone())
}

/// What the app says about the file it is about to send.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct StartBody {
    #[serde(default)]
    pub filename: String,
    pub size: u64,
    #[serde(default)]
    pub content_type: Option<String>,
    /// Must be 1: resumable uploads are for the Media page.
    #[serde(default)]
    pub media: Option<u8>,
    /// The BLAKE3 hash of the whole file in hex, if the app has it: the finished file must match.
    #[serde(default)]
    pub blake3: Option<String>,
    /// 1: the person chose to keep the file's hidden information (B4, 2026-10-11): the finish
    /// skips the picture and video strip (a picture is still checked to be what it says).
    #[serde(default)]
    pub keep_metadata: Option<u8>,
}

/// `text` trimmed and held to `max` characters.
fn clipped(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

/// Start a session. Everything is checked before anything is written (the module docs list what);
/// the answer is 201 `{id, offset: 0, size, chunk_max}`.
pub async fn start_core(state: &Arc<RelayState>, folders: Folders<'_>, token: Option<&str>, b: StartBody) -> Result<(StatusCode, Json<Value>), Refused> {
    let key = token_key(state, token).await?;
    if b.media != Some(1) {
        return Err(words(
            StatusCode::BAD_REQUEST,
            "A resumable upload is for the Media page: send media: 1. Chat and Shared Files files go in one request (POST /api/upload).",
        ));
    }
    let settings = state.db.get_server_settings().unwrap_or_default();
    let role = uploader_role(&state.db, &key)?;

    let filename = match clipped(&b.filename, NAME_MAX) {
        n if n.is_empty() => "upload".to_string(),
        n => n,
    };
    let content_type = match clipped(b.content_type.as_deref().unwrap_or(""), CONTENT_TYPE_MAX) {
        t if t.is_empty() => "application/octet-stream".to_string(),
        t => t,
    };
    // The same type rules and sharing switches as the single request (upload_rules.rs).
    let kind = check_type(&filename, &content_type, false)?;
    sharing_check(&settings, &role, &content_type, false)?;

    if b.size == 0 {
        return Err(words(StatusCode::BAD_REQUEST, "The file is empty: there is nothing to upload."));
    }
    if b.size > SIZE_CEILING {
        return Err(words(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("File too large ({:.1} GB; one file can be at most {} GB here).", b.size as f64 / (1u64 << 30) as f64, SIZE_CEILING >> 30),
        ));
    }
    // A picture is read into memory at the end for its magic check and metadata strip, so it is
    // held to the same size as in the single request.
    if kind.picture(&content_type, false).is_some() && b.size > PICTURE_MAX_BYTES as u64 {
        return Err(picture_too_large(b.size).into());
    }
    // The role's own largest Media file, when it has one (B1, 2026-10-11; 0 is up to the share).
    if media_file_limit(&role).is_some_and(|max| b.size > max) {
        return Err(media_file_too_large(&role).into());
    }
    let blake3 = match b.blake3.as_deref().map(|h| h.trim().to_lowercase()) {
        None => String::new(),
        Some(h) if h.is_empty() => String::new(),
        Some(h) if h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit()) => h,
        Some(_) => return Err(words(StatusCode::BAD_REQUEST, "blake3 must be the file's BLAKE3 hash: 64 hexadecimal characters.")),
    };

    let size = b.size;
    let keep_metadata = b.keep_metadata == Some(1);
    let state = state.clone();
    let owned = OwnedFolders::of(folders);
    let id = tokio::task::spawn_blocking(move || -> Result<String, Refused> {
        let folders = owned.get();
        let db = &state.db;
        // The room check and the new row under the room lock (media.rs), so two starts (or a
        // start and a single request's store) cannot both pass on the same room.
        let _room = media::room_lock();
        // The hour's count of uploads, and a day's allowance already used up (B3), checked and
        // counted under the same lock at this time (R3-1), so starts sent at once cannot all pass:
        // answered in JSON with when it resets, so an app can wait and start then. A file bigger
        // than what is left of today starts: it is stopped at the piece that would pass the
        // allowance, and carries on after the reset.
        let now = state.upload_now_ms();
        allowance_at_start(db, &key, &role, now).map_err(|over| Refused::Json(StatusCode::TOO_MANY_REQUESTS, over.json()))?;
        // Finished sessions only keep their answer (R2): their files are stored and counted.
        let all: Vec<UploadSession> = db.all_upload_sessions().map_err(db_err)?.into_iter().filter(|s| !s.finished()).collect();
        let mine: Vec<&UploadSession> = all.iter().filter(|s| s.public_key == key).collect();
        if mine.len() >= OPEN_SESSIONS_MAX {
            return Err(words(
                StatusCode::TOO_MANY_REQUESTS,
                format!("You have {} uploads in progress on this server, the most at once. Finish or cancel one first.", mine.len()),
            ));
        }
        let reserved = Reserved { all: all.iter().map(|s| s.size).sum(), mine: mine.iter().map(|s| s.size).sum() };
        media::media_room(db, &settings, &key, size, reserved)?;

        let mut raw = [0u8; 16];
        getrandom::getrandom(&mut raw).map_err(|e| words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to name the upload: {e}")))?;
        let id = hex::encode(raw);
        let path = part_path(folders.resume, &id).expect("a hex id is a session id");
        std::fs::create_dir_all(folders.resume)
            .and_then(|_| std::fs::File::create(&path))
            .map_err(|e| words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start the upload: {e}")))?;
        let session = UploadSession {
            id: id.clone(),
            public_key: key.clone(),
            filename,
            content_type,
            size: size as i64,
            offset: 0,
            shelf: MEDIA_SHELF,
            blake3,
            created_at: now,
            touched_at: now,
            keep_metadata,
            hash_checked: false,
            done_answer: String::new(),
        };
        if let Err(e) = db.insert_upload_session(&session) {
            let _ = std::fs::remove_file(&path);
            return Err(db_err(e));
        }
        // One upload started, against the hour's allowance (B3); its bytes count as they arrive.
        count_upload(db, &key, now, 0, true);
        Ok(id)
    })
    .await
    .map_err(task_err)??;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "offset": 0, "size": size, "chunk_max": CHUNK_MAX }))))
}

pub async fn start(State(state): State<Arc<RelayState>>, Query(q): Query<SessionQuery>, headers: HeaderMap, Json(b): Json<StartBody>) -> Response {
    match start_core(&state, Folders::live(), session_token(&headers, &q).as_deref(), b).await {
        Ok(answer) => answer.into_response(),
        Err(e) => e.into_response(),
    }
}

// ── The person's own session ──

/// Session `id`, if it is the token holder's: anyone else's, and one that does not exist, are the
/// same 404.
async fn own_session(state: &RelayState, token: Option<&str>, id: &str) -> Result<UploadSession, Refused> {
    let key = token_key(state, token).await?;
    if !valid_id(id) {
        return Err(not_found());
    }
    match state.db.upload_session(id).map_err(db_err)? {
        Some(s) if s.public_key == key => Ok(s),
        _ => Err(not_found()),
    }
}

// ── GET /api/upload/resumable/{id} ──

/// How far the server got: `{offset, size, chunk_max}`, and once the file is stored `done`, the
/// finished upload's answer (R2: an app that lost its connection during the finish learns the
/// stored name here).
pub async fn status_core(state: &RelayState, id: &str, token: Option<&str>) -> Result<Json<Value>, Refused> {
    let s = own_session(state, token, id).await?;
    let mut answer = json!({ "offset": s.offset, "size": s.size, "chunk_max": CHUNK_MAX });
    if let Some(done) = done_answer(&s) {
        answer["done"] = done;
    }
    Ok(Json(answer))
}

/// A finished session's answer (R2), or None while the file is not stored yet.
fn done_answer(s: &UploadSession) -> Option<Value> {
    s.finished().then(|| serde_json::from_str(&s.done_answer).unwrap_or(Value::Null))
}

pub async fn status(State(state): State<Arc<RelayState>>, Path(id): Path<String>, Query(q): Query<SessionQuery>, headers: HeaderMap) -> Response {
    let token = session_token(&headers, &q);
    status_core(&state, &id, token.as_deref()).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
}

// ── DELETE /api/upload/resumable/{id} ──

/// Cancel. A cancel means nothing stays (R4-2, 2026-10-11, the decision at the top of the design's
/// round 4):
/// - while a piece or the finish is being written: 409 `{busy: true}`, nothing done; the app asks
///   again until it is not busy;
/// - a session still arriving: its row and its pieces file go, wherever the file is (the resume
///   folder, or staged by a finish that could not move it back, R3-5): `{ok: true}`;
/// - a finished session whose file is not published: that stored file goes from the Media waiting
///   folder with its upload row, and the session's row: `{ok: true, removed_stored: true}` (also
///   when the file was already gone, by its own Remove or the day's expiry: nothing stays either
///   way);
/// - a finished session whose file was already published: the item stays, since publishing it
///   was the person's own choice; the session's row goes: `{ok: true, published: true}`.
///
/// Round 3 removed only a finished session's row and answered `{ok: true}`, so a cancel that
/// raced the finish (asked again after the busy answer, the finish done meanwhile) left the stored
/// file waiting with nothing to say so.
pub async fn cancel_core(state: &Arc<RelayState>, folders: Folders<'_>, id: &str, token: Option<&str>) -> Result<Json<Value>, Refused> {
    own_session(state, token, id).await?;
    let writing = Writing::begin(id).ok_or_else(busy)?;
    // Read again now that this request holds the session: a finish that ended a moment ago made
    // it a finished one.
    let s = own_session(state, token, id).await?;
    let state = state.clone();
    let owned = OwnedFolders::of(folders);
    tokio::task::spawn_blocking(move || {
        let _writing = writing;
        cancel_session(&state.db, owned.get(), &s)
    })
    .await
    .map_err(task_err)?
    .map(Json)
}

/// [`cancel_core`]'s work on session `s`, which the caller holds as being written. BLOCKING: a
/// finished session's part runs under the room lock, as a publish does, so a publish of its file
/// either came first (the item stays) or finds nothing left to publish.
fn cancel_session(db: &Storage, folders: Folders, s: &UploadSession) -> Result<Value, Refused> {
    if !s.finished() {
        db.delete_upload_session(&s.id).map_err(db_err)?;
        remove_session_files(folders, &s.id);
        return Ok(json!({ "ok": true }));
    }
    let answer: Value = serde_json::from_str(&s.done_answer).unwrap_or(Value::Null);
    let name = answer["filename"].as_str().and_then(|n| media::stored_name(n).ok()).map(str::to_string);
    let _room = media::room_lock();
    let Some(name) = name else {
        // A finished row whose answer names no file (only a row written by hand): it goes.
        db.delete_upload_session(&s.id).map_err(db_err)?;
        return Ok(json!({ "ok": true, "removed_stored": true }));
    };
    if db.media_item_for_upload(&name).map_err(db_err)?.is_some() {
        db.delete_upload_session(&s.id).map_err(db_err)?;
        return Ok(json!({ "ok": true, "published": true }));
    }
    // The file first: one that cannot be removed keeps its rows, so no file is left that no row
    // knows of (the day's expiry of unpublished uploads works from the rows), and the app is told.
    match std::fs::remove_file(folders.waiting.join(&name)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(words(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("The uploaded file could not be removed from the server ({e}). Try again in a moment."),
            ))
        }
    }
    match db.remove_unpublished_media(&name, &s.public_key, false) {
        Ok(()) | Err(MediaError::Refused(Refusal::NotFound, _)) => {}
        Err(MediaError::Db(e)) => return Err(db_err(e)),
        Err(MediaError::Refused(_, w) | MediaError::Files(w)) => return Err(words(StatusCode::INTERNAL_SERVER_ERROR, w)),
    }
    db.delete_upload_session(&s.id).map_err(db_err)?;
    Ok(json!({ "ok": true, "removed_stored": true }))
}

pub async fn cancel(State(state): State<Arc<RelayState>>, Path(id): Path<String>, Query(q): Query<SessionQuery>, headers: HeaderMap) -> Response {
    let token = session_token(&headers, &q);
    cancel_core(&state, Folders::live(), &id, token.as_deref()).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
}

// ── PATCH /api/upload/resumable/{id} ──

/// Write `piece` at `offset` in the file at `path`: anything past `offset` first goes (bytes of a
/// write that was never counted), then the piece is written and synced to disk. On a failure the
/// file is cut back to `offset`, so the stored offset stays true.
fn write_piece(path: &std::path::Path, offset: u64, piece: &[u8]) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    let mut f = std::fs::OpenOptions::new().write(true).open(path)?;
    f.set_len(offset)?;
    f.seek(SeekFrom::Start(offset))?;
    let wrote = f.write_all(piece).and_then(|_| f.sync_data());
    if wrote.is_err() {
        let _ = f.set_len(offset);
    }
    wrote
}

/// One piece of session `s` (whose owner's role is `role`) written at the session's offset in its
/// pieces file at `path`, and the offset stored. BLOCKING, and run whole once started, even if
/// the app's connection drops (R3-1, 2026-10-11):
///
/// 1. Under the room lock, at the time the piece arrives: the day's allowance checked (B3) and the
///    piece's bytes counted, together, so pieces of several uploads sent at once cannot all pass on
///    the same allowance (the single request's store does the same, media.rs `store_upload`). A
///    piece that would pass it is refused (429 JSON: which allowance, when it resets, in seconds
///    too, what is left of it, where the upload is; R3-6), nothing of it written, and the session
///    kept so the rest can come after the reset, or a piece cut to `left_bytes` now. The count is
///    written on the session too, in the same transaction (R4-3, storage/upload_sessions.rs
///    `count_upload_piece`).
/// 2. The piece written and synced, then the offset stored, which clears the session's count. A
///    write that fails, or a session cancelled or expired meanwhile, takes the piece's bytes back
///    off the count. A relay that stops before the offset is stored leaves the count on the
///    session, and its next start gives the bytes back to the day they were counted in (R4-3), as
///    it cuts the piece off the file: the piece sent again is counted once, not twice.
fn write_counted(state: &RelayState, path: &std::path::Path, s: &UploadSession, role: &RoleDef, piece: &[u8]) -> Result<(), Refused> {
    let (db, id, key, at) = (&state.db, s.id.as_str(), s.public_key.as_str(), s.offset);
    let len = piece.len() as u64;
    let (now, counted) = {
        let _room = media::room_lock();
        let now = state.upload_now_ms();
        if let Err(over) = allowance_for_piece(db, key, role, now, len) {
            let mut body = over.json();
            body["offset"] = json!(at);
            body["size"] = json!(s.size);
            return Err(Refused::Json(StatusCode::TOO_MANY_REQUESTS, body));
        }
        let counted = match db.count_upload_piece(id, key, now, len as i64) {
            Ok(true) => true,
            // Cancelled or expired before it was counted: nothing counted, nothing written.
            Ok(false) => return Err(not_found()),
            // A count that cannot be written is logged: the upload is not stopped for the relay's
            // own fault (upload_rules.rs `count_upload`). Nothing was counted, so nothing is
            // taken back below.
            Err(e) => {
                tracing::error!("Upload allowance: could not count {len} bytes of upload {id} ({e})");
                false
            }
        };
        (now, counted)
    };
    let take_back = || {
        if counted {
            take_back_piece(db, id, key, now, len);
        }
    };
    // A test can hold a piece here, its allowance decided and not yet written (R3-1), by the
    // name `piece:<file name>`, apart from the store's hold on the file name itself.
    #[cfg(test)]
    media::hold::wait_at(&format!("piece:{}", s.filename));
    if let Err(e) = write_piece(path, at as u64, piece) {
        take_back();
        if e.kind() == std::io::ErrorKind::NotFound {
            // The pieces' file is gone (removed by hand): nothing can be resumed.
            let _ = db.delete_upload_session(id);
            return Err(words(StatusCode::GONE, "This upload's pieces were lost on the server. Start it again."));
        }
        return Err(words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write the piece: {e}")));
    }
    // A test can stop here, as a relay that stops between the write and the offset does (R4-3),
    // by the name `written:<file name>`: nothing after this line runs.
    #[cfg(test)]
    if media::hold::stops_at(&format!("written:{}", s.filename)) {
        return Err(words(StatusCode::INTERNAL_SERVER_ERROR, "A test stopped the relay here."));
    }
    match db.set_upload_session_offset(id, at + len as i64, now) {
        Ok(true) => Ok(()),
        Ok(false) => {
            // Cancelled or expired while the piece was written.
            take_back();
            let _ = std::fs::remove_file(path);
            Err(not_found())
        }
        Err(e) => {
            take_back();
            Err(db_err(e))
        }
    }
}

/// A piece of session `id` counted at `counted_at` and not stored: its bytes go back off `key`'s
/// day and the session holds no count (R4-3). A failure is logged; a session that is still there
/// keeps its count, which the next start gives back.
fn take_back_piece(db: &Storage, id: &str, key: &str, counted_at: i64, len: u64) {
    if let Err(e) = db.take_back_upload_piece(id, key, counted_at, len as i64) {
        tracing::error!("Upload allowance: could not take back {len} bytes of upload {id} ({e})");
    }
}

/// The next piece. `offset` must be the server's offset (409 with the right one otherwise), the
/// piece at most [`CHUNK_MAX`] and not past the file's size. The piece is appended and synced,
/// then the offset is stored: `{offset, size}`. The piece that makes the file whole finishes it
/// ([`finish`]) and the answer is the finished upload.
pub async fn piece_core(state: &Arc<RelayState>, folders: Folders<'_>, id: &str, token: Option<&str>, offset: Option<u64>, piece: Bytes) -> Result<Json<Value>, Refused> {
    own_session(state, token, id).await?;
    let offset = offset.ok_or_else(|| words(StatusCode::BAD_REQUEST, "Say where this piece starts: ?offset=<bytes already sent>."))?;
    // The marker is shared with the blocking work below (R2, 2026-10-11): if the app's connection
    // drops while a piece is written or the file is finished, this request's future is dropped
    // but the work goes on, and the marker stays until it is done, so no second request can
    // write beside it or start a second finish.
    let writing = Arc::new(Writing::begin(id).ok_or_else(busy)?);
    // Read again now that this request holds the session: a piece that finished a moment ago
    // moved the offset.
    let s = own_session(state, token, id).await?;
    // Already stored (R2): the same answer again, and nothing done.
    if let Some(done) = done_answer(&s) {
        return Ok(Json(done));
    }
    if offset != s.offset as u64 {
        return Err(Refused::Json(
            StatusCode::CONFLICT,
            json!({
                "error": format!("This piece starts at byte {offset}, but the server has {} bytes of the file. Send from there.", s.offset),
                "offset": s.offset,
                "size": s.size,
            }),
        ));
    }
    if piece.len() > CHUNK_MAX {
        return Err(words(StatusCode::PAYLOAD_TOO_LARGE, format!("A piece can be at most {} MB; this one is {:.1} MB.", CHUNK_MAX >> 20, piece.len() as f64 / (1u64 << 20) as f64)));
    }
    let end = s.offset + piece.len() as i64;
    if end > s.size {
        return Err(words(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("This piece goes past the end of the file: the file is {} bytes and the piece would end at byte {end}.", s.size),
        ));
    }
    // The session is in use: the expiry pass leaves it alone while the piece is written (and, for
    // a piece past the day's allowance, for a day from now, which is past the reset).
    state.db.touch_upload_session(id, state.upload_now_ms()).map_err(db_err)?;
    let path = part_path(folders.resume, id).ok_or_else(not_found)?;
    if !piece.is_empty() {
        // The allowance, the write and the offset, off the executor and whole even if the app's
        // connection drops now (R3-1): see `write_counted`.
        let role = state.db.role_def(&state.db.get_role(&s.public_key).unwrap_or_default());
        let (st, session, held) = (state.clone(), s.clone(), writing.clone());
        tokio::task::spawn_blocking(move || {
            let _held = held;
            write_counted(&st, &path, &session, &role, &piece)
        })
        .await
        .map_err(task_err)??;
    }
    if end < s.size {
        return Ok(Json(json!({ "offset": end, "size": s.size })));
    }
    let done = UploadSession { offset: end, ..s };
    let state = state.clone();
    let owned = OwnedFolders::of(folders);
    let held = writing.clone();
    tokio::task::spawn_blocking(move || {
        let _held = held;
        finish(&state, owned.get(), &done)
    })
    .await
    .map_err(task_err)?
    .map(Json)
}

pub async fn piece(State(state): State<Arc<RelayState>>, Path(id): Path<String>, Query(q): Query<SessionQuery>, headers: HeaderMap, body: Bytes) -> Response {
    let token = session_token(&headers, &q);
    piece_core(&state, Folders::live(), &id, token.as_deref(), q.offset, body).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
}

// ── The finish ──

/// The BLAKE3 hash of the file at `path`, in lowercase hex, read a megabyte at a time.
fn hash_file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// The file is whole: check it and store it, as the single request does. BLOCKING (it hashes the
/// whole file and may read a picture into memory), so it runs in `spawn_blocking`.
///
/// Refused and the session removed (the file is not one that can ever be stored): the role or the
/// sharing switches no longer allow it, a hash that does not match, a picture that is not what it
/// says, a video or a HEIC or AVIF photo that cannot be read through (422). Refused and the session KEPT:
/// no room (507; an empty piece at its end tries again), and a failure to read, write or move the
/// file, which may pass (R3, R5). Stored: the session's row stays, holding the answer (the single
/// request's JSON), until the idle expiry (R2); the caller holds the one-piece-at-a-time marker
/// for the whole of it.
fn finish(state: &RelayState, folders: Folders, s: &UploadSession) -> Result<Value, Refused> {
    let db = &state.db;
    let part = part_path(folders.resume, &s.id).ok_or_else(not_found)?;
    let drop_session = |why: Refused| -> Refused {
        let _ = db.delete_upload_session(&s.id);
        let _ = std::fs::remove_file(&part);
        why
    };
    // A test can hold one named finish at its start (relay/resumable_tests.rs, R2).
    #[cfg(test)]
    let _held = (s.filename == tests::HELD_FINISH_NAME).then(|| tests::FINISH_HOLD.lock().unwrap_or_else(|p| p.into_inner()));
    // A finish that could not move the pieces file back after a refusal left it staged under its
    // session's name (R3-5): it comes back first. Only a file that is nowhere is lost.
    if !part.is_file() && !restore_staged_part(folders, &s.id) {
        let staged = crate::relay::storage::upload_sessions::staged_part_name(&s.id);
        if folders.waiting.join(&staged).is_file() || folders.uploads.join(&staged).is_file() {
            // Staged, and it cannot come back yet: kept, for another try.
            return Err(words(StatusCode::INTERNAL_SERVER_ERROR, "This upload's file could not be put back in place on the server yet. Try again in a moment."));
        }
        // The pieces' file is gone (removed by hand): nothing can be finished.
        return Err(drop_session(words(StatusCode::GONE, "This upload's pieces were lost on the server. Start it again.")));
    }

    // The rules again: a long upload can outlive a change to the role or the switches.
    let settings = db.get_server_settings().unwrap_or_default();
    let role = uploader_role(db, &s.public_key).map_err(|e| drop_session(e.into()))?;
    let kind = check_type(&s.filename, &s.content_type, false).map_err(|e| drop_session(e.into()))?;
    sharing_check(&settings, &role, &s.content_type, false).map_err(|e| drop_session(e.into()))?;

    // The hash the app gave at the start: if the file does not match, a resume went wrong and the
    // file is not the one the app has. Checked ONCE, before anything below changes the file, and
    // the check is recorded on the row (R1, 2026-10-11): the location strip rewrites boxes in
    // place, so a finish tried again after a 507 or a crash would hash the stripped file, find it
    // different, and delete a good upload.
    if !s.blake3.is_empty() && !s.hash_checked {
        let got = hash_file(&part).map_err(|e| words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read the upload back: {e}")))?;
        if got != s.blake3 {
            return Err(drop_session(words(
                StatusCode::UNPROCESSABLE_ENTITY,
                "The file that arrived does not match the file you sent (its BLAKE3 hash differs), so nothing was kept. Send it again from the start.",
            )));
        }
        db.mark_upload_hash_checked(&s.id).map_err(db_err)?;
    }

    // A picture: its magic bytes and the metadata strip, as in the single request (the strip
    // skipped when the person keeps the picture's hidden information, B4).
    let picture = kind.picture(&s.content_type, false);
    let body = match picture {
        Some(p) => {
            let bytes = std::fs::read(&part).map_err(|e| words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read the upload back: {e}")))?;
            UploadBody::Bytes(finish_picture_as(p, bytes, s.keep_metadata).map_err(|e| drop_session(e.into()))?.into())
        }
        None => {
            // A video's or a HEIC or AVIF photo's hidden location removed in place, as in the
            // single request (BUG-190, the HEIF strip). Only a file that can never be stored removes
            // the session (422: it cannot be read through, so it cannot be cleaned with certainty);
            // a failure to read or write the file may pass, so the session is kept for another
            // try (R3, 2026-10-11).
            finish_file_as(&part, s.keep_metadata)
                .map_err(|e| if e.0 == StatusCode::UNPROCESSABLE_ENTITY { drop_session(e.into()) } else { e.into() })?;
            UploadBody::Part(part.clone())
        }
    };
    let len = match &body {
        UploadBody::Bytes(b) => b.len() as u64,
        UploadBody::Part(_) => s.size as u64,
    };
    let ext = kind.stored_ext(&s.content_type, false);
    let kind_word = file_type(&s.content_type, &ext, false);
    let name = stored_name(&s.filename, &ext);

    // No allowance at the store: the pieces' bytes were counted as they arrived, the upload at its
    // start (R3-1). The pieces file waits for the room lock under its session's name (R3-5).
    let new = NewUpload {
        key: &s.public_key,
        shelf: s.shelf,
        name: &name,
        original_name: &s.filename,
        len,
        keep: role.max_uploads_kept.max(1),
        private_held_limit: None,
        allowance: None,
        session: Some(&s.id),
    };
    match media::store_upload(db, &settings, folders, &new, |path| body.put(path), |path| body.unstage(path)) {
        Ok(old_files) => {
            media::delete_pruned(folders, &old_files);
            // A picture was stored from its stripped bytes, a copy from another disk left the
            // pieces' file behind: either way it is done with.
            let _ = std::fs::remove_file(&part);
            let answer = upload_answer(&name, len, kind_word);
            // The row stays with the answer until the idle expiry (R2): an app whose connection
            // dropped during this finish asks (GET) and is told the stored name.
            if let Err(e) = db.finish_upload_session(&s.id, &answer.to_string(), now_ms()) {
                tracing::error!("Resumable upload {}: stored, but its answer could not be kept ({e})", s.id);
            }
            Ok(answer)
        }
        // No room: nothing was moved, so the session and its file stay for another try.
        Err((StatusCode::INSUFFICIENT_STORAGE, room)) => Err(words(StatusCode::INSUFFICIENT_STORAGE, room + KEPT_WORDS)),
        // A failure to put the file in place: store_upload undid it (the pieces' file is back, or
        // kept staged under its session's name when it could not go back: the next try or the
        // next start brings it back, R3-5), so the session stays for another try (R5).
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
#[path = "resumable_tests.rs"]
mod tests;

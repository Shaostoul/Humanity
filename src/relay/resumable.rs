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
//!   blake3?}` starts one: 201 `{id, offset: 0, size, chunk_max}`.
//! - `GET /api/upload/resumable/{id}?token=`: `{offset, size, chunk_max}`, how far it got.
//! - `PATCH /api/upload/resumable/{id}?token=&offset=<n>` with the next piece as the body:
//!   `{offset, size}`, or for the last piece the finished upload, the same JSON as
//!   `POST /api/upload` (`{url, filename, size, type}`), which the app then publishes with
//!   `POST /api/media` as before.
//! - `DELETE /api/upload/resumable/{id}?token=` cancels: `{ok: true}`.
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
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

use crate::relay::media::{self, Folders, NewUpload, Reserved};
use crate::relay::relay::RelayState;
use crate::relay::storage::media::MEDIA_SHELF;
use crate::relay::storage::upload_sessions::{part_path, valid_id, UploadSession};
use crate::relay::upload_limits::{picture_too_large, UploadBody, PICTURE_MAX_BYTES};
use crate::relay::upload_rules::{check_type, file_type, finish_picture, sharing_check, stored_name, token_key, upload_answer, uploader_role};

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

// ── The folders, owned, for work off the executor ──

/// [`Folders`] with its paths owned, so they can move into `spawn_blocking`.
#[derive(Debug, Clone)]
struct OwnedFolders {
    uploads: PathBuf,
    members: PathBuf,
    resume: PathBuf,
}

impl OwnedFolders {
    fn of(f: Folders) -> Self {
        OwnedFolders { uploads: f.uploads.to_path_buf(), members: f.members.to_path_buf(), resume: f.resume.to_path_buf() }
    }
    fn get(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume }
    }
}

// ── POST /api/upload/resumable ──

/// `?token=` (every route) and `?offset=` (a piece).
#[derive(Debug, Default, Deserialize)]
pub struct SessionQuery {
    pub token: Option<String>,
    pub offset: Option<u64>,
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
    let blake3 = match b.blake3.as_deref().map(|h| h.trim().to_lowercase()) {
        None => String::new(),
        Some(h) if h.is_empty() => String::new(),
        Some(h) if h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit()) => h,
        Some(_) => return Err(words(StatusCode::BAD_REQUEST, "blake3 must be the file's BLAKE3 hash: 64 hexadecimal characters.")),
    };

    let size = b.size;
    let state = state.clone();
    let owned = OwnedFolders::of(folders);
    let id = tokio::task::spawn_blocking(move || -> Result<String, Refused> {
        let folders = owned.get();
        let db = &state.db;
        // The room check and the new row under the room lock (media.rs), so two starts (or a
        // start and a single request's store) cannot both pass on the same room.
        let _room = media::room_lock();
        let all = db.all_upload_sessions().map_err(db_err)?;
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
        let now = now_ms();
        let session = UploadSession {
            id: id.clone(),
            public_key: key,
            filename,
            content_type,
            size: size as i64,
            offset: 0,
            shelf: MEDIA_SHELF,
            blake3,
            created_at: now,
            touched_at: now,
        };
        if let Err(e) = db.insert_upload_session(&session) {
            let _ = std::fs::remove_file(&path);
            return Err(db_err(e));
        }
        Ok(id)
    })
    .await
    .map_err(task_err)??;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "offset": 0, "size": size, "chunk_max": CHUNK_MAX }))))
}

pub async fn start(State(state): State<Arc<RelayState>>, Query(q): Query<SessionQuery>, Json(b): Json<StartBody>) -> Response {
    match start_core(&state, Folders::live(), q.token.as_deref(), b).await {
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

/// How far the server got: `{offset, size, chunk_max}`.
pub async fn status_core(state: &RelayState, id: &str, token: Option<&str>) -> Result<Json<Value>, Refused> {
    let s = own_session(state, token, id).await?;
    Ok(Json(json!({ "offset": s.offset, "size": s.size, "chunk_max": CHUNK_MAX })))
}

pub async fn status(State(state): State<Arc<RelayState>>, Path(id): Path<String>, Query(q): Query<SessionQuery>) -> Response {
    status_core(&state, &id, q.token.as_deref()).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
}

// ── DELETE /api/upload/resumable/{id} ──

/// Cancel: the row and the file go. Not while a piece is being written (409).
pub async fn cancel_core(state: &RelayState, folders: Folders<'_>, id: &str, token: Option<&str>) -> Result<Json<Value>, Refused> {
    own_session(state, token, id).await?;
    let _writing = Writing::begin(id).ok_or_else(busy)?;
    state.db.delete_upload_session(id).map_err(db_err)?;
    if let Some(path) = part_path(folders.resume, id) {
        let _ = std::fs::remove_file(path);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn cancel(State(state): State<Arc<RelayState>>, Path(id): Path<String>, Query(q): Query<SessionQuery>) -> Response {
    cancel_core(&state, Folders::live(), &id, q.token.as_deref()).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
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

/// The next piece. `offset` must be the server's offset (409 with the right one otherwise), the
/// piece at most [`CHUNK_MAX`] and not past the file's size. The piece is appended and synced,
/// then the offset is stored: `{offset, size}`. The piece that makes the file whole finishes it
/// ([`finish`]) and the answer is the finished upload.
pub async fn piece_core(state: &Arc<RelayState>, folders: Folders<'_>, id: &str, token: Option<&str>, offset: Option<u64>, piece: Bytes) -> Result<Json<Value>, Refused> {
    own_session(state, token, id).await?;
    let offset = offset.ok_or_else(|| words(StatusCode::BAD_REQUEST, "Say where this piece starts: ?offset=<bytes already sent>."))?;
    let _writing = Writing::begin(id).ok_or_else(busy)?;
    // Read again now that this request holds the session: a piece that finished a moment ago
    // moved the offset.
    let s = own_session(state, token, id).await?;
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
    // The session is in use: the expiry pass leaves it alone while the piece is written.
    state.db.touch_upload_session(id, now_ms()).map_err(db_err)?;
    let path = part_path(folders.resume, id).ok_or_else(not_found)?;
    if !piece.is_empty() {
        let (to, at) = (path.clone(), s.offset as u64);
        let wrote = tokio::task::spawn_blocking(move || write_piece(&to, at, &piece)).await.map_err(task_err)?;
        if let Err(e) = wrote {
            if e.kind() == std::io::ErrorKind::NotFound {
                // The pieces' file is gone (removed by hand): nothing can be resumed.
                let _ = state.db.delete_upload_session(id);
                return Err(words(StatusCode::GONE, "This upload's pieces were lost on the server. Start it again."));
            }
            return Err(words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write the piece: {e}")));
        }
        if !state.db.set_upload_session_offset(id, end, now_ms()).map_err(db_err)? {
            // Cancelled or expired while the piece was written.
            let _ = std::fs::remove_file(&path);
            return Err(not_found());
        }
    }
    if end < s.size {
        return Ok(Json(json!({ "offset": end, "size": s.size })));
    }
    let done = UploadSession { offset: end, ..s };
    let state = state.clone();
    let owned = OwnedFolders::of(folders);
    tokio::task::spawn_blocking(move || finish(&state, owned.get(), &done)).await.map_err(task_err)?.map(Json)
}

pub async fn piece(State(state): State<Arc<RelayState>>, Path(id): Path<String>, Query(q): Query<SessionQuery>, body: Bytes) -> Response {
    piece_core(&state, Folders::live(), &id, q.token.as_deref(), q.offset, body).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
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
/// says. Refused and the session KEPT: no room (507; an empty piece at its end tries again), and a
/// failure to read or make a folder, which may pass. Stored: the session goes and the answer is
/// the single request's JSON.
fn finish(state: &RelayState, folders: Folders, s: &UploadSession) -> Result<Value, Refused> {
    let db = &state.db;
    let part = part_path(folders.resume, &s.id).ok_or_else(not_found)?;
    let drop_session = |why: Refused| -> Refused {
        let _ = db.delete_upload_session(&s.id);
        let _ = std::fs::remove_file(&part);
        why
    };

    // The rules again: a long upload can outlive a change to the role or the switches.
    let settings = db.get_server_settings().unwrap_or_default();
    let role = uploader_role(db, &s.public_key).map_err(|e| drop_session(e.into()))?;
    let kind = check_type(&s.filename, &s.content_type, false).map_err(|e| drop_session(e.into()))?;
    sharing_check(&settings, &role, &s.content_type, false).map_err(|e| drop_session(e.into()))?;

    // The hash the app gave at the start: if the file does not match, a resume went wrong and the
    // file is not the one the app has.
    if !s.blake3.is_empty() {
        let got = hash_file(&part).map_err(|e| words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read the upload back: {e}")))?;
        if got != s.blake3 {
            return Err(drop_session(words(
                StatusCode::UNPROCESSABLE_ENTITY,
                "The file that arrived does not match the file you sent (its BLAKE3 hash differs), so nothing was kept. Send it again from the start.",
            )));
        }
    }

    // A picture: its magic bytes and the metadata strip, as in the single request.
    let picture = kind.picture(&s.content_type, false);
    let body = match picture {
        Some(p) => {
            let bytes = std::fs::read(&part).map_err(|e| words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read the upload back: {e}")))?;
            UploadBody::Bytes(finish_picture(p, bytes).map_err(|e| drop_session(e.into()))?.into())
        }
        None => UploadBody::Part(part.clone()),
    };
    let len = match &body {
        UploadBody::Bytes(b) => b.len() as u64,
        UploadBody::Part(_) => s.size as u64,
    };
    let ext = kind.stored_ext(&s.content_type, false);
    let kind_word = file_type(&s.content_type, &ext, false);
    let name = stored_name(&s.filename, &ext);

    std::fs::create_dir_all(folders.uploads).map_err(|e| words(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create upload dir: {e}")))?;
    let new = NewUpload { key: &s.public_key, shelf: s.shelf, name: &name, original_name: &s.filename, len, keep: role.max_uploads_kept.max(1) };
    match media::store_upload(db, &settings, folders, &new, |path| body.put(path)) {
        Ok(old_files) => {
            media::delete_pruned(folders, &old_files);
            let _ = db.delete_upload_session(&s.id);
            // A picture was stored from its stripped bytes; the pieces' file is done with.
            let _ = std::fs::remove_file(&part);
            Ok(upload_answer(&name, len, kind_word))
        }
        // No room: nothing was moved, so the session and its file stay for another try.
        Err((StatusCode::INSUFFICIENT_STORAGE, room)) => Err(words(StatusCode::INSUFFICIENT_STORAGE, room + KEPT_WORDS)),
        Err(e) => Err(drop_session(e.into())),
    }
}

#[cfg(test)]
#[path = "resumable_tests.rs"]
mod tests;

//! The Media page's routes (2026-10-11; docs/design/media-showcase.md, "Increment 1, exact").
//! Storage is storage/media.rs; the routes are registered in relay/mod.rs `build_router` and
//! gated with the uploads in relay/features.rs (`/api/media` belongs to `Feature::Uploads`).
//!
//! WHO SEES WHAT. `GET /api/media` gives public items to anyone while the server's
//! `media_visibility` is "public"; members-only items, and everything on a members-only server
//! (its categories and labels too), only to a signed request from a member (purpose
//! `media_list`). A signed read can ride in the query or, through `POST /api/media/list`, in a
//! body: a Dilithium key and signature make a URL of about 10 KB, which a proxy refuses (the
//! admin-stats 414, api.rs), so the list's answer carries the categories and labels too. The same
//! two forms serve the admins' suggestions and usage reads. A settings read that fails, or a
//! visibility word the relay does not know, counts as members only (the review's F7).
//!
//! "MEMBER" is anyone signed in to this server who is not banned, as everywhere else on the
//! relay; on an open server anyone can become one, which Server Settings says under the switch
//! (`storage::MEDIA_MEMBERS_NOTE`). A members-only item's file is not in the public uploads
//! folder: it is in `data/media-members/`, served only by `GET /api/media/file/{id}` to a member
//! with the session's upload token, in the `X-Upload-Token` header (W3b, 2026-10-11) or, for
//! now, `?token=` (F3). A Media upload not yet published waits in `data/media-waiting/`, which
//! nothing serves (R6).
//!
//! EVERY WRITE IS SIGNED: Dilithium over `purpose\ntimestamp`, as `delete_shared_upload` is,
//! plus what that route does not have: a timestamp at most five minutes old and at most 30
//! seconds ahead (the relay's replay memory covers the whole window), and a (key, purpose,
//! timestamp) the relay has not seen, so a captured request cannot be sent twice. Each purpose
//! names its target, so a signature is good for one thing (F5): `media_publish:<file name>`,
//! `media_remove:<item id or file name>`, `media_pick:<item id>` (pick and unpick),
//! `media_feature:<item id>`, `media_visibility:<item id>:<public|members>`,
//! `media_suggestions:<suggestion id>:<accept|decline>`, `media_categories:<BLAKE3 hex of the
//! request's category list as sent>` and `media_keep:<item id>:<on|off>` (A7, 2026-10-11). A new
//! category suggestion keeps plain `media_suggest` (the design names no target for it), and the
//! reads keep `media_list` (the list, and one person's picks by POST, A3), `media_usage`,
//! `media_suggestions` and `media_mine` (A4). The role checks happen after the signature, on the
//! proven key. A write that names an item by number also names its file (`filename`), and is
//! refused when that is not the item's file, so a page that is out of date never acts on another
//! item (W5, [`same_item`]); item numbers are never reused.
//!
//! Each route's work after the signature is a plain function over the storage (`*_core`), which
//! is what the tests call: no relay, no socket, and no file written outside a temp folder (the
//! `*_core` functions that touch files take the folders, `storage::media::Folders`).
//!
//! THE LABELS (spoiler, flashing lights, graphic) are data: data/media/labels.json, read at start
//! ([`labels`]). Adult content is not among them and is refused by name: it stays off the Media
//! page until it is researched and the operator decides (the design's "Labels").

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::relay::relay::{RelayMessage, RelayState};
pub use crate::relay::storage::media::{Folders, MEMBERS_DIR, UPLOADS_DIR};
use crate::relay::storage::media::{
    self as store, bare_name, CategoryEdit, MediaError, MediaItem, MediaQuery, Publish, Refusal, Spot, KINDS, MEDIA_SHELF,
};
use crate::relay::storage::{media_visibility_of, ServerSettings, Storage};

/// How old a signed request's timestamp may be.
pub const SIGNED_WINDOW_MS: u64 = 5 * 60 * 1000;
/// How far ahead of the relay's clock a signed request's timestamp may be (F5): small, so a
/// request is never accepted after the replay memory (six minutes from first sight) forgets it.
pub const SIGNED_AHEAD_MS: u64 = 30 * 1000;
const MIB: i64 = 1024 * 1024;

type Refused = (StatusCode, String);
type Answer = Result<Json<Value>, Refused>;

// ── Labels (data/media/labels.json) ──

/// One content label: the id the relay stores, the name the apps show, the line under it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
pub struct MediaLabel {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub desc: String,
}

#[derive(Deserialize)]
struct LabelsFile {
    labels: Vec<MediaLabel>,
}

/// The labels file's text: refused when it is not the file's shape, lists none, or has an id
/// that is empty, not lower-case a to z and underscore (a comma would break the stored list),
/// repeated, or without a name.
pub fn parse_labels(text: &str) -> Result<Vec<MediaLabel>, String> {
    let file: LabelsFile = serde_json::from_str(text).map_err(|e| format!("not a labels file: {e}"))?;
    if file.labels.is_empty() {
        return Err("it lists no labels".into());
    }
    let mut seen = std::collections::HashSet::new();
    for l in &file.labels {
        if l.id.is_empty() || !l.id.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Err(format!("the id \"{}\" is not lower-case a to z and underscore", l.id));
        }
        if !seen.insert(l.id.clone()) {
            return Err(format!("the id \"{}\" is listed twice", l.id));
        }
        if l.label.trim().is_empty() {
            return Err(format!("the label \"{}\" has no name", l.id));
        }
    }
    Ok(file.labels)
}

/// The labels from `path`, else the copy built into the exe (and the log says so, as for every
/// relay data file), else none (then no label is accepted, and the log says that too).
pub fn load_labels(path: &std::path::Path) -> Vec<MediaLabel> {
    let shown = path.display().to_string();
    let from_disk = match std::fs::read_to_string(path) {
        Ok(text) => parse_labels(&text).map_err(|e| format!("{shown} does not parse ({e})")),
        Err(e) => Err(format!("{shown} could not be read ({e})")),
    };
    from_disk.unwrap_or_else(|why| {
        crate::embedded_data::note_builtin_copy("media/labels.json", why);
        // Declared after the disk read, so the compiled-in check sees this file read disk first.
        const BUILT_IN: &str = include_str!("../../data/media/labels.json");
        parse_labels(BUILT_IN).unwrap_or_else(|e| {
            tracing::error!("The built-in media/labels.json does not parse either ({e}); no content label is accepted");
            Vec::new()
        })
    })
}

/// The labels this relay accepts, read once from the data folder (`crate::data_dir`, which is
/// "data" for the headless relay), at start (relay/mod.rs `run_relay`).
pub fn labels() -> &'static [MediaLabel] {
    static LABELS: std::sync::OnceLock<Vec<MediaLabel>> = std::sync::OnceLock::new();
    LABELS.get_or_init(|| load_labels(&crate::data_dir().join("media").join("labels.json")))
}

/// The labels someone chose, checked against `known` and joined as they are stored: ids in the
/// labels file's order, each once. An unknown one is refused with the list; adult content by
/// name, since it is not allowed on the Media page.
pub fn normalize_labels(given: &[String], known: &[MediaLabel]) -> Result<String, String> {
    let given: Vec<String> = given.iter().map(|g| g.trim().to_ascii_lowercase()).filter(|g| !g.is_empty()).collect();
    for g in &given {
        if matches!(g.as_str(), "nsfw" | "adult" | "explicit" | "porn") {
            return Err("Adult content is not allowed on the Media page of this server.".into());
        }
        if !known.iter().any(|l| &l.id == g) {
            let list: Vec<String> = known.iter().map(|l| format!("{} ({})", l.label, l.id)).collect();
            return Err(format!("\"{g}\" is not a content label here. The labels are: {}.", list.join(", ")));
        }
    }
    Ok(known.iter().filter(|l| given.contains(&l.id)).map(|l| l.id.as_str()).collect::<Vec<_>>().join(","))
}

// ── Signatures and roles ──

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Prove the caller holds `key`: the timestamp at most [`SIGNED_WINDOW_MS`] old and at most
/// [`SIGNED_AHEAD_MS`] ahead of the relay's clock, the Dilithium signature over
/// `purpose\ntimestamp` (off the async executor; a panic fails closed), and a (key, purpose,
/// timestamp) the relay has not seen in the window. A write's `purpose` names its target (F5).
pub async fn check_signed(state: &RelayState, key: &str, timestamp: u64, sig: &str, purpose: &str) -> Result<(), Refused> {
    let now = now_ms();
    if now.saturating_sub(timestamp) > SIGNED_WINDOW_MS {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }
    if timestamp.saturating_sub(now) > SIGNED_AHEAD_MS {
        return Err((StatusCode::BAD_REQUEST, "Timestamp is in the future: check this device's clock.".into()));
    }
    let (k, s, p) = (key.to_string(), sig.to_string(), purpose.to_string());
    let ok = tokio::task::spawn_blocking(move || {
        crate::relay::handlers::broadcast::verify_dilithium_signature(&k, &p, timestamp, &s)
    })
    .await
    .unwrap_or(false);
    if !ok {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }
    if !state.auth_nonce_fresh(key, purpose, timestamp) {
        return Err((StatusCode::CONFLICT, "Duplicate request rejected (replay).".into()));
    }
    Ok(())
}

/// The server settings as the Media page reads them: a read that fails counts as members only,
/// never as a public page (fail closed, the review's F7). A stored visibility word the relay
/// does not know already reads as members only (storage/server_settings.rs).
pub fn media_settings(db: &Storage) -> ServerSettings {
    db.get_server_settings().unwrap_or_else(|e| {
        tracing::error!("Media: the server settings could not be read ({e}); the Media page counts as members only");
        ServerSettings { media_visibility: "members".into(), ..ServerSettings::default() }
    })
}

/// Whether the server's whole Media page is members only: anything but "public" is.
fn members_only_server(s: &ServerSettings) -> bool {
    s.media_visibility != "public"
}

fn is_admin(db: &Storage, key: &str) -> bool {
    matches!(db.get_role(key).unwrap_or_default().as_str(), "admin" | "owner")
}

/// A member of this server who is not banned from it.
fn is_member(db: &Storage, key: &str) -> bool {
    db.is_member(key) && !db.is_banned(key).unwrap_or(true)
}

fn refusal(e: MediaError) -> Refused {
    match e {
        MediaError::Refused(why, words) => (
            match why {
                Refusal::Bad => StatusCode::BAD_REQUEST,
                Refusal::NotAllowed => StatusCode::FORBIDDEN,
                Refusal::NotFound => StatusCode::NOT_FOUND,
                Refusal::Conflict => StatusCode::CONFLICT,
                Refusal::Full => StatusCode::INSUFFICIENT_STORAGE,
            },
            words,
        ),
        MediaError::Db(e) => db_err(e),
        MediaError::Files(words) => (StatusCode::INTERNAL_SERVER_ERROR, words),
    }
}

fn db_err(e: rusqlite::Error) -> Refused {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("storage: {e}"))
}

fn forbidden(words: &str) -> Refused {
    (StatusCode::FORBIDDEN, words.to_string())
}

/// Where an item's file is fetched: its `/uploads/` address while public; a members-only one's
/// is the members' file route, which the app calls with `?token=` and its session's upload token.
pub fn item_url(i: &MediaItem) -> String {
    if i.visibility == "public" {
        format!("/uploads/{}", i.upload_filename)
    } else {
        format!("/api/media/file/{}", i.id)
    }
}

/// One item as every route gives it.
pub fn item_json(i: &MediaItem) -> Value {
    json!({
        "id": i.id,
        "url": item_url(i),
        "title": i.title,
        "description": i.description,
        "category": { "id": i.category_id, "name": i.category_name },
        "kind": i.kind,
        "labels": i.labels,
        "visibility": i.visibility,
        "featured": i.featured,
        "size_bytes": i.size_bytes,
        "views": i.views,
        "uploaded_at": i.uploaded_at,
        "published_at": i.published_at,
        "uploader_key": i.uploader_key,
        "uploader_name": i.uploader_name,
        "picks": i.picks,
        // A1 and A7 (2026-10-11): the uploader's own name for the file, whether Remove keeps the
        // file (an upload from chat or Shared Files) or deletes it (one made for Media), and
        // whether the server keeps the item.
        "original_name": i.original_name,
        "keeps_file": i.keeps_file,
        "kept": i.kept,
    })
}

fn items_json(items: &[MediaItem]) -> Vec<Value> {
    items.iter().map(item_json).collect()
}

// ── Uploads: the shelf and the room (called by api.rs `upload_file` and resumable.rs) ──

/// Which shelf an upload goes on from its query: 2 for `?media=1` (the Media budget, out of the
/// keep-N FIFO and the Shared Files list until it is published), 1 for `?share=1`, 0 for chat
/// media. An encrypted file can never be Media.
pub fn upload_shelf(share: Option<u8>, media: Option<u8>, encrypted: Option<u8>) -> Result<i64, Refused> {
    if media == Some(1) {
        if encrypted == Some(1) {
            return Err((
                StatusCode::BAD_REQUEST,
                "An encrypted file is for a private conversation and can never go on the Media page.".into(),
            ));
        }
        return Ok(MEDIA_SHELF);
    }
    Ok(if share == Some(1) { 1 } else { 0 })
}

/// One lock over every upload's room check, its move into place and its row, and over every
/// publish's budget check and its row (F4): without it two uploads finishing together each
/// passed the check before either was counted. Process-wide, because the folders are.
static ROOM: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The room lock, also taken by a resumable upload's start (relay/resumable.rs), so its room
/// check and its session row go in together and two starts cannot both pass.
pub(crate) fn room_lock() -> std::sync::MutexGuard<'static, ()> {
    ROOM.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What the stored files take on disk: the public folder, the members-only one and the Media
/// waiting folder, leaving out copies still arriving (storage/media.rs `folder_bytes`).
pub fn stored_bytes(folders: Folders) -> u64 {
    store::folder_bytes(folders.uploads) + store::folder_bytes(folders.members) + store::folder_bytes(folders.waiting)
}

/// A fresh name for a file being copied into a folder (R5): [`store::STAGED_PREFIX`], 16 random
/// hex characters and a number this relay never repeats, so two copies never share a name.
fn staged_name() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut raw = [0u8; 8];
    let _ = getrandom::getrandom(&mut raw);
    format!("{}{}-{}", store::STAGED_PREFIX, hex::encode(raw), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// An upload the route has received, to be stored by [`store_upload`].
pub struct NewUpload<'a> {
    pub key: &'a str,
    /// 0 chat, 1 Shared Files, 2 Media ([`upload_shelf`]).
    pub shelf: i64,
    /// The stored file name (the upload route makes it).
    pub name: &'a str,
    pub original_name: &'a str,
    pub len: u64,
    /// The keep-N FIFO's count for the uploader's role.
    pub keep: i64,
    /// For a file from a private conversation, the most its uploader may hold in such files at
    /// once (their role's `max_private_held_mb`, B5, 2026-10-11), checked with the room; None for
    /// any other file, or a role with no limit.
    pub private_held_limit: Option<u64>,
}

/// "10 November 2026": a Unix-ms time as a date people read (UTC).
fn date_words(ms: i64) -> String {
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let (y, m, d) = crate::relay::epoch_days_to_ymd(ms.div_euclid(24 * 60 * 60 * 1000));
    format!("{d} {} {y}", MONTHS[(m as usize).clamp(1, 12) - 1])
}

/// Whether `key` may hold a `len`-byte private file more (B5): what they hold of files from
/// private conversations plus this one, against their role's `limit`. The words say it is the
/// person's OWN room that is full, not the server's, and when their oldest leaves.
pub fn private_room(db: &Storage, s: &ServerSettings, key: &str, limit: u64, len: u64) -> Result<(), Refused> {
    let (held, oldest) = db.private_files_held(key).map_err(db_err)?;
    if held.max(0) as u64 + len <= limit {
        return Ok(());
    }
    let words = match oldest {
        Some(t) => format!(
            "Your private files on this server are full ({} of {}). The oldest leaves on {}, or remove some from Settings > Data.",
            store::mb(held),
            store::mb(limit as i64),
            date_words(crate::relay::storage::private_files::leaves_at(t, s.dm_mailbox_ttl_days))
        ),
        None => format!(
            "Your private files on this server can be {} in all, and this file is {}.",
            store::mb(limit as i64),
            store::mb(len as i64)
        ),
    };
    Err((StatusCode::INSUFFICIENT_STORAGE, words))
}

/// The room checks an upload is held to, in one place for [`store_upload`]'s two looks: the
/// shelf's room ([`upload_room`]) and, for a private file, the person's own ([`private_room`]).
fn room_for(db: &Storage, s: &ServerSettings, folders: Folders, u: &NewUpload) -> Result<(), Refused> {
    upload_room(db, s, u.key, u.shelf, stored_bytes(folders), u.len)?;
    match u.private_held_limit {
        Some(limit) => private_room(db, s, u.key, limit, u.len),
        None => Ok(()),
    }
}

/// Store an upload: a chat or Shared Files file in the public folder, a Media upload in the
/// waiting folder that nothing serves until it is published (R6, 2026-10-11). Returns the files
/// the keep-N FIFO let go, for the caller to delete. A row that cannot be written is logged, as
/// before.
///
/// THE ORDER (R5, 2026-10-11). A quick room check first, under the room lock, so a file that
/// cannot fit is never copied. Then, OUTSIDE the lock, `put` brings the file into the destination
/// folder under a hidden name ([`store::STAGED_PREFIX`]: never served, never counted, cleared at
/// start): a rename when it can, a copy of up to 64 GiB when the file is on another disk, which
/// must not hold every other upload waiting. Then under the lock (F4) the room check that counts,
/// a rename within the folder, and the row. On any refusal or failure after `put`, `unstage` is
/// handed the hidden file to undo it (move it back where it came from, or delete the copy), so
/// nothing half-stored is ever left at a public address; a failed `put` has its partial copy
/// removed here.
pub fn store_upload(
    db: &Storage,
    s: &ServerSettings,
    folders: Folders,
    u: &NewUpload,
    put: impl FnOnce(&std::path::Path) -> std::io::Result<()>,
    unstage: impl FnOnce(&std::path::Path),
) -> Result<Vec<String>, Refused> {
    {
        let _room = room_lock();
        room_for(db, s, folders, u)?;
    }
    let dest = if u.shelf == MEDIA_SHELF { folders.waiting } else { folders.uploads };
    std::fs::create_dir_all(dest).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create upload dir: {e}")))?;
    let staged = dest.join(staged_name());
    if let Err(e) = put(&staged) {
        let _ = std::fs::remove_file(&staged);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {e}")));
    }
    let _room = room_lock();
    if let Err(refused) = room_for(db, s, folders, u) {
        unstage(&staged);
        return Err(refused);
    }
    if let Err(e) = std::fs::rename(&staged, dest.join(u.name)) {
        unstage(&staged);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {e}")));
    }
    match db.record_upload(u.key, u.name, u.keep, u.shelf, u.original_name, u.len as i64) {
        Ok(old) => Ok(old),
        Err(e) => {
            tracing::error!("Failed to record upload: {e}");
            Ok(Vec::new())
        }
    }
}

/// Delete the files the keep-N FIFO let go ([`store_upload`]'s answer) from the public folder,
/// logging each. Blocking: called on a worker thread.
pub fn delete_pruned(folders: Folders, old_files: &[String]) {
    for old_file in old_files {
        if let Err(e) = std::fs::remove_file(folders.uploads.join(old_file)) {
            tracing::warn!("Failed to delete old upload {}: {e}", old_file);
        } else {
            tracing::info!("FIFO cleanup: deleted old upload {}", old_file);
        }
    }
}

/// Whether there is room for a `len`-byte upload on `shelf` by `key`, with `dir_total` bytes in
/// the upload folders now (the public one and the members-only one). A Media upload is held to
/// the Media budget and the person's share of it, with both numbers in the words ([`media_room`]);
/// anything else to the general cap, `max_total_upload_mb`, counting what is in the folders apart
/// from Media's own bytes (the cap stays for chat files).
pub fn upload_room(db: &Storage, s: &ServerSettings, key: &str, shelf: i64, dir_total: u64, len: u64) -> Result<(), Refused> {
    if shelf == MEDIA_SHELF {
        return media_room(db, s, key, len, Reserved::default());
    }
    let len = len as i64;
    let media_used = db.media_bytes(None);
    let cap = s.max_total_upload_mb.max(1) * MIB;
    let chat = (dir_total as i64 - media_used).max(0);
    if chat + len > cap {
        return Err((
            StatusCode::INSUFFICIENT_STORAGE,
            format!(
                "Upload storage full ({:.1} MB / {:.0} MB). Please try again later.",
                chat as f64 / MIB as f64,
                cap as f64 / MIB as f64
            ),
        ));
    }
    Ok(())
}

/// Bytes promised to Media uploads still arriving in pieces (resumable sessions,
/// relay/resumable.rs), which [`media_room`] counts as already used when a new one starts, so two
/// uploads that fit alone but not together cannot both start (and both send gigabytes, only for
/// one to be refused at the end). Nothing is promised to a single request, and the check when a
/// file is stored ([`store_upload`]) counts only what is stored: that one is the one that counts.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Reserved {
    /// Every open session's full size, everyone's: held against the Media budget.
    pub all: i64,
    /// The person's own open sessions: held against their share.
    pub mine: i64,
}

/// Whether the Media space has room for a `len`-byte file by `key`: the Media budget, then the
/// person's share of it, counting what is stored there and what is `reserved` for uploads in
/// progress, with both numbers in the words (and the part that is still arriving, when there is
/// one). With nothing reserved this is exactly the Media half of [`upload_room`].
pub fn media_room(db: &Storage, s: &ServerSettings, key: &str, len: u64, reserved: Reserved) -> Result<(), Refused> {
    let len = len as i64;
    // What the words add when part of "used" is uploads still arriving, so the number makes sense.
    let in_progress = |n: i64| if n > 0 { format!(" That counts {} of uploads still in progress.", store::mb(n)) } else { String::new() };
    let used = db.media_bytes(None) + reserved.all;
    let budget = s.media_budget_mb.max(1) * MIB;
    if used + len > budget {
        return Err((StatusCode::INSUFFICIENT_STORAGE, store::budget_full_words(used, budget, len) + &in_progress(reserved.all)));
    }
    let mine = db.media_bytes(Some(key)) + reserved.mine;
    let share = s.media_per_person_mb.max(1) * MIB;
    if mine + len > share {
        return Err((StatusCode::INSUFFICIENT_STORAGE, store::share_full_words(mine, share, len) + &in_progress(reserved.mine)));
    }
    Ok(())
}

// ── GET /api/media, POST /api/media/list, GET /api/media/picks/{key} ──

/// The list's filters, and an optional signature (`media_list`) for the members' view.
#[derive(Debug, Default, Deserialize)]
pub struct ListParams {
    pub category: Option<i64>,
    pub kind: Option<String>,
    pub by: Option<String>,
    pub q: Option<String>,
    pub before: Option<i64>,
    pub limit: Option<i64>,
    pub key: Option<String>,
    pub timestamp: Option<u64>,
    pub sig: Option<String>,
    /// `featured=1`: featured items only, so the strip shows every one (A2, 2026-10-11).
    pub featured: Option<u8>,
    /// One item by its number, if the viewer may see it (the admins' Reports page opens a
    /// reported item this way, A5).
    pub id: Option<i64>,
    /// Whose picks, for `POST /api/media/picks` (A3); the GET form names them in its path.
    pub of: Option<String>,
}

/// Whether the request gets the members' view, as the member's key: no signature, no; a signature
/// from a member, yes; a bad or partial signature is refused rather than quietly served the
/// public view.
async fn members_view(state: &RelayState, p: &ListParams) -> Result<Option<String>, Refused> {
    match (p.key.as_deref().filter(|k| !k.is_empty()), p.timestamp, p.sig.as_deref().filter(|s| !s.is_empty())) {
        (None, None, None) => Ok(None),
        (Some(key), Some(ts), Some(sig)) => {
            check_signed(state, key, ts, sig, "media_list").await?;
            Ok(is_member(&state.db, key).then(|| key.to_string()))
        }
        _ => Err((StatusCode::BAD_REQUEST, "A signed request needs key, timestamp and sig together.".into())),
    }
}

/// The list for a viewer with the members' view (`member`, their key) or without it, optionally
/// one person's picks. On a members-only server the public gets no items, and is told so. In the
/// members' view every item also says whether that member picked it (`picked_by_me`, A3).
pub fn list_core(db: &Storage, s: &ServerSettings, p: &ListParams, member: Option<&str>, picked_by: Option<String>) -> Answer {
    let members = member.is_some();
    if let Some(kind) = p.kind.as_deref() {
        if !KINDS.contains(&kind) {
            return Err((StatusCode::BAD_REQUEST, format!("\"{kind}\" is not a kind. The kinds are: {}.", KINDS.join(", "))));
        }
    }
    let members_only = members_only_server(s);
    let shut = members_only && !members;
    let items = if shut {
        Vec::new()
    } else {
        db.list_media(&MediaQuery {
            category: p.category,
            kind: p.kind.clone(),
            by: p.by.clone(),
            q: p.q.clone(),
            before: p.before,
            limit: p.limit,
            members,
            picked_by,
            featured: p.featured == Some(1),
            id: p.id,
        })
        .map_err(db_err)?
    };
    let limit = p.limit.unwrap_or(store::MEDIA_LIST_DEFAULT).clamp(1, store::MEDIA_LIST_MAX) as usize;
    let next_before = if items.len() == limit { items.last().map(|i| i.id) } else { None };
    let (categories, labels) = categories_and_labels(db, s, members)?;
    let mut shown = items_json(&items);
    if let Some(me) = member {
        let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        let picked = db.picked_among(me, &ids).map_err(db_err)?;
        for (v, i) in shown.iter_mut().zip(&items) {
            v["picked_by_me"] = Value::Bool(picked.contains(&i.id));
        }
    }
    Ok(Json(json!({
        "items": shown,
        "next_before": next_before,
        "members_view": members,
        "media_visibility": s.media_visibility,
        "members_only_for_you": shut,
        "categories": categories,
        "labels": labels,
    })))
}

/// The categories and the labels, for a viewer with or without the members' view: on a
/// members-only server, none for anyone else (F3).
fn categories_and_labels(db: &Storage, s: &ServerSettings, members: bool) -> Result<(Value, Value), Refused> {
    if members_only_server(s) && !members {
        return Ok((json!([]), json!([])));
    }
    Ok((json!(db.media_categories().map_err(db_err)?), json!(labels())))
}

pub async fn list(State(state): State<Arc<RelayState>>, Query(p): Query<ListParams>) -> Answer {
    let member = members_view(&state, &p).await?;
    list_core(&state.db, &media_settings(&state.db), &p, member.as_deref(), None)
}

pub async fn list_post(State(state): State<Arc<RelayState>>, Json(p): Json<ListParams>) -> Answer {
    let member = members_view(&state, &p).await?;
    list_core(&state.db, &media_settings(&state.db), &p, member.as_deref(), None)
}

/// One person's picks, as the list (the same filters and the same rule about who sees what).
pub async fn picks_of(State(state): State<Arc<RelayState>>, Path(key): Path<String>, Query(p): Query<ListParams>) -> Answer {
    let member = members_view(&state, &p).await?;
    list_core(&state.db, &media_settings(&state.db), &p, member.as_deref(), Some(key))
}

/// `POST /api/media/picks` (A3, 2026-10-11): one person's picks (`of`) with the list's signed body
/// (purpose `media_list`), so a member's view, members-only items included, never needs the
/// signature in a URL (the 414 lesson above). Answered like the list.
pub async fn picks_post(State(state): State<Arc<RelayState>>, Json(p): Json<ListParams>) -> Answer {
    let of = p
        .of
        .clone()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Say whose picks: `of`, their key.".to_string()))?;
    let member = members_view(&state, &p).await?;
    list_core(&state.db, &media_settings(&state.db), &p, member.as_deref(), Some(of))
}

// ── GET /api/media/categories ──

/// The categories and the labels. On a members-only server only a signed request from a member
/// gets them (the list's signature, `media_list`, in the query; the list's answer carries them
/// too, for the POST form).
pub async fn categories(State(state): State<Arc<RelayState>>, Query(p): Query<ListParams>) -> Answer {
    let member = members_view(&state, &p).await?;
    category_list_core(&state.db, &media_settings(&state.db), member.is_some())
}

/// [`categories`] for a viewer with or without the members' view.
pub fn category_list_core(db: &Storage, s: &ServerSettings, members: bool) -> Answer {
    let (categories, labels) = categories_and_labels(db, s, members)?;
    Ok(Json(json!({ "categories": categories, "labels": labels, "members_only_for_you": members_only_server(s) && !members })))
}

// ── POST /api/media (publish) ──

#[derive(Debug, Default, Deserialize)]
pub struct PublishBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    /// The stored file name, or its `/uploads/...` address.
    pub filename: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub labels: Vec<String>,
    /// "public" (the default) or "members".
    #[serde(default)]
    pub visibility: Option<String>,
}

/// The stored file name a publish or a removal names: as sent, or its `/uploads/...` address.
/// Refused when it is not a bare name.
pub fn stored_name(given: &str) -> Result<&str, Refused> {
    let name = given.trim().trim_start_matches("/uploads/");
    if !bare_name(name) {
        return Err((StatusCode::BAD_REQUEST, "Bad file name.".into()));
    }
    Ok(name)
}

pub async fn publish(State(state): State<Arc<RelayState>>, Json(b): Json<PublishBody>) -> Answer {
    let purpose = format!("media_publish:{}", stored_name(&b.filename)?);
    check_signed(&state, &b.key, b.timestamp, &b.sig, &purpose).await?;
    // Off the executor: the publish waits for the room lock, which an upload holds while its
    // file moves into place (F4).
    tokio::task::spawn_blocking(move || publish_core(&state.db, &media_settings(&state.db), labels(), Folders::live(), &b))
        .await
        .unwrap_or_else(|e| Err((StatusCode::INTERNAL_SERVER_ERROR, format!("The publish failed: {e}"))))
}

/// Publish for the proven `b.key` (a member, not banned): one of their own uploads, never an
/// encrypted one, within the Media budget and their share of it, under the room lock (F4). A
/// members-only item's file moves out of the public folder before the item is committed (F3).
pub fn publish_core(db: &Storage, s: &ServerSettings, known: &[MediaLabel], folders: Folders, b: &PublishBody) -> Answer {
    if !is_member(db, &b.key) {
        return Err(forbidden("Only members of this server can put things on its Media page."));
    }
    let filename = stored_name(&b.filename)?;
    if b.title.chars().count() > store::MEDIA_TITLE_MAX {
        return Err((StatusCode::BAD_REQUEST, format!("A title can be at most {} characters.", store::MEDIA_TITLE_MAX)));
    }
    if b.description.chars().count() > store::MEDIA_DESCRIPTION_MAX {
        return Err((StatusCode::BAD_REQUEST, format!("A description can be at most {} characters.", store::MEDIA_DESCRIPTION_MAX)));
    }
    let visibility = match b.visibility.as_deref() {
        None => "public",
        Some(v) => media_visibility_of(v)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Visibility is \"public\" or \"members\".".to_string()))?,
    };
    let labels = normalize_labels(&b.labels, known).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let _room = room_lock();
    let disk_size = folders.find(filename).and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len() as i64);
    // Where the file was before this publish moved it (R6: a Media upload waits in a folder
    // nothing serves and moves to the public folder or the members-only one now), so a refused
    // publish puts it back exactly there.
    let moved: std::cell::Cell<Option<Spot>> = std::cell::Cell::new(None);
    let published = db.publish_media_placing(
        &b.key,
        &Publish {
            filename,
            title: &b.title,
            description: &b.description,
            category_id: b.category_id,
            labels: &labels,
            visibility,
        },
        s.media_budget_mb.max(1) * MIB,
        s.media_per_person_mb.max(1) * MIB,
        disk_size,
        |members| {
            let from = folders.move_to(filename, if members { Spot::Members } else { Spot::Public })?;
            moved.set(from);
            Ok(from.is_some())
        },
    );
    match published {
        Ok(item) => Ok(Json(json!({ "ok": true, "item": item_json(&item) }))),
        Err(e) => {
            // Nothing was committed: a file this publish moved goes back where it was.
            if let Some(from) = moved.get() {
                if let Err(err) = folders.move_to(filename, from) {
                    tracing::error!("Media: a refused publish could not put its file back ({err})");
                }
            }
            Err(refusal(e))
        }
    }
}

// ── POST /api/media/remove, /pick, /unpick, /feature, /keep ──

/// What a request that names an item by number must also say, and is refused without (W5).
pub const NAME_THE_FILE: &str = "Name the item's file too (`filename`), so a page that is out of date never acts on another item.";

/// W5 (2026-10-11): the item `id` names must be the one whose file `filename` names, so a tile
/// left on a page that is out of date never acts on another item. Item numbers are never reused
/// (`media_items.id` is AUTOINCREMENT), and the file name says which item the person saw. Refused:
/// no file name (400), no such item (404), another item's file (409).
pub fn same_item(db: &Storage, id: i64, filename: &str) -> Result<(), Refused> {
    if filename.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, NAME_THE_FILE.to_string()));
    }
    let name = stored_name(filename)?;
    match db.media_item(id).map_err(db_err)? {
        None => Err((StatusCode::NOT_FOUND, "There is no Media item with that number.".to_string())),
        Some(item) if item.upload_filename != name => Err((
            StatusCode::CONFLICT,
            "That is not the Media item you were looking at: the page is out of date. Load it again.".to_string(),
        )),
        Some(_) => Ok(()),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ItemBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    /// The item's file name, as the list gave it (`url` without `/uploads/`, or the item's own),
    /// required (W5): [`same_item`].
    #[serde(default)]
    pub filename: String,
}

/// What `POST /api/media/remove` takes down: an item by its `id`, or by its upload's file name;
/// a Media upload not yet published by its file name (F1).
#[derive(Debug, Default, Deserialize)]
pub struct RemoveBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub filename: Option<String>,
}

/// The two things a removal can name.
#[derive(Debug, Clone, PartialEq)]
pub enum RemoveTarget {
    Item(i64),
    Upload(String),
}

impl RemoveBody {
    /// The target, and the purpose its signature is over: `media_remove:<item id or file name>`.
    /// An item named by its id must carry its file name too (W5, checked by [`same_item`]); an
    /// upload not yet published is named by its file name alone.
    pub fn target(&self) -> Result<(RemoveTarget, String), Refused> {
        match (self.id, self.filename.as_deref()) {
            (Some(id), Some(_)) => Ok((RemoveTarget::Item(id), format!("media_remove:{id}"))),
            (Some(_), None) => Err((StatusCode::BAD_REQUEST, NAME_THE_FILE.to_string())),
            (None, Some(name)) => {
                let name = stored_name(name)?;
                Ok((RemoveTarget::Upload(name.to_string()), format!("media_remove:{name}")))
            }
            (None, None) => Err((StatusCode::BAD_REQUEST, "Name the item by its id and file name, or the upload by its file name.".into())),
        }
    }
}

pub async fn remove(State(state): State<Arc<RelayState>>, Json(b): Json<RemoveBody>) -> Answer {
    let (target, purpose) = b.target()?;
    check_signed(&state, &b.key, b.timestamp, &b.sig, &purpose).await?;
    if let RemoveTarget::Item(id) = target {
        same_item(&state.db, id, b.filename.as_deref().unwrap_or(""))?;
    }
    remove_core(&state.db, Folders::live(), &target, &b.key)
}

/// The uploader or an admin takes something down. An item: it and its picks go, and its upload
/// goes back where it came from, keeping its file (from chat or Shared Files, back in the public
/// folder if it was members only), or loses its file (uploaded for Media) (F2). A Media upload
/// not yet published, named by its file name: its row and file go (F1). A file name that is a
/// published item's removes that item.
pub fn remove_core(db: &Storage, folders: Folders, target: &RemoveTarget, requester: &str) -> Answer {
    let admin = is_admin(db, requester);
    let id = match target {
        RemoveTarget::Item(id) => *id,
        RemoveTarget::Upload(name) => match db.media_item_for_upload(stored_name(name)?).map_err(db_err)? {
            Some(id) => id,
            None => {
                let name = stored_name(name)?;
                db.remove_unpublished_media(name, requester, admin).map_err(refusal)?;
                let file_removed = folders.delete(name);
                tracing::info!("A Media upload waiting to be published was removed (file removed: {file_removed})");
                return Ok(Json(json!({ "ok": true, "removed_upload": name, "file_removed": file_removed, "file_kept": false })));
            }
        },
    };
    let removed = db.remove_media(id, requester, admin).map_err(refusal)?;
    let (file_removed, file_kept) = if removed.keeps_file() {
        // Back to its public address, where its chat message or the Shared Files list finds it.
        if let Err(e) = folders.place(&removed.filename, false) {
            tracing::error!("Media: a removed item's file could not go back to the public folder ({e})");
        }
        (false, true)
    } else {
        (folders.delete(&removed.filename), false)
    };
    tracing::info!("A Media item was removed (file removed: {file_removed}, kept: {file_kept})");
    Ok(Json(json!({ "ok": true, "removed": id, "file_removed": file_removed, "file_kept": file_kept })))
}

pub async fn pick(State(state): State<Arc<RelayState>>, Json(b): Json<ItemBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, &format!("media_pick:{}", b.id)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    pick_core(&state.db, &b.key, b.id, true)
}

pub async fn unpick(State(state): State<Arc<RelayState>>, Json(b): Json<ItemBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, &format!("media_pick:{}", b.id)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    pick_core(&state.db, &b.key, b.id, false)
}

/// A member picks item `id` (`on`) or takes the pick back; answers with its pick count.
pub fn pick_core(db: &Storage, key: &str, id: i64, on: bool) -> Answer {
    if !is_member(db, key) {
        return Err(forbidden("Only members of this server can pick Media items."));
    }
    let picks = if on { db.pick_media(key, id) } else { db.unpick_media(key, id) }.map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "id": id, "picked": on, "picks": picks })))
}

fn yes() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
pub struct FeatureBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    /// True to feature it (the default), false to take it off the featured row.
    #[serde(default = "yes")]
    pub featured: bool,
    /// The item's file name, required (W5): [`same_item`].
    #[serde(default)]
    pub filename: String,
}

pub async fn feature(State(state): State<Arc<RelayState>>, Json(b): Json<FeatureBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, &format!("media_feature:{}", b.id)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    feature_core(&state.db, &b.key, b.id, b.featured)
}

/// `POST /api/media/keep` (A7, 2026-10-11; the operator's answers, number 1): an admin or a
/// moderator keeps an item on the server, or stops keeping it.
#[derive(Debug, Default, Deserialize)]
pub struct KeepBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    /// True to keep it (the default), false to stop keeping it.
    #[serde(default = "yes")]
    pub kept: bool,
    /// The item's file name, required (W5): [`same_item`].
    #[serde(default)]
    pub filename: String,
}

/// The purpose a keep is signed over: `media_keep:<item id>:<on|off>`.
pub fn keep_purpose(id: i64, kept: bool) -> String {
    format!("media_keep:{id}:{}", if kept { "on" } else { "off" })
}

pub async fn keep(State(state): State<Arc<RelayState>>, Json(b): Json<KeepBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, &keep_purpose(b.id, b.kept)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    keep_core(&state.db, &b.key, b.id, b.kept)
}

/// Admins and moderators only. A kept item counts against the Media budget but not against its
/// uploader's share, and is in no prune list; stopping puts it back in the uploader's share even
/// past it (never deleted for that: the share stops only new uploads). Its uploader can still
/// remove it.
pub fn keep_core(db: &Storage, key: &str, id: i64, kept: bool) -> Answer {
    if !matches!(db.get_role(key).unwrap_or_default().as_str(), "admin" | "owner" | "mod" | "moderator") {
        return Err(forbidden("Only an admin or a moderator can keep Media items on the server."));
    }
    db.set_media_kept(id, kept.then_some(key), now_ms() as i64).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "id": id, "kept": kept })))
}

/// Admins only.
pub fn feature_core(db: &Storage, key: &str, id: i64, featured: bool) -> Answer {
    if !is_admin(db, key) {
        return Err(forbidden("Only an admin can feature Media items."));
    }
    db.set_media_featured(id, featured).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "id": id, "featured": featured })))
}

// ── POST /api/media/visibility ──

#[derive(Debug, Default, Deserialize)]
pub struct VisibilityBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    /// "public" or "members".
    pub visibility: String,
    /// The item's file name, required (W5): [`same_item`].
    #[serde(default)]
    pub filename: String,
}

pub async fn visibility(State(state): State<Arc<RelayState>>, Json(b): Json<VisibilityBody>) -> Answer {
    let to = media_visibility_of(&b.visibility)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Visibility is \"public\" or \"members\".".to_string()))?;
    check_signed(&state, &b.key, b.timestamp, &b.sig, &format!("media_visibility:{}:{to}", b.id)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    visibility_core(&state.db, Folders::live(), b.id, &b.key, to)
}

/// The uploader or an admin makes item `id` public or members only, and its file moves with it:
/// out of the public folder for members only, back to its `/uploads/` address for public, before
/// the change is committed, and back again when it cannot be (F3).
pub fn visibility_core(db: &Storage, folders: Folders, id: i64, requester: &str, to: &str) -> Answer {
    let moved: std::cell::Cell<Option<(String, bool)>> = std::cell::Cell::new(None);
    let changed = db.set_media_visibility(id, requester, is_admin(db, requester), to, |name, members| {
        let m = folders.place(name, members)?;
        if m {
            moved.set(Some((name.to_string(), members)));
        }
        Ok(m)
    });
    match changed {
        Ok(item) => Ok(Json(json!({ "ok": true, "item": item_json(&item) }))),
        Err(e) => {
            if let Some((name, members)) = moved.take() {
                if let Err(err) = folders.place(&name, !members) {
                    tracing::error!("Media: a refused visibility change could not put its file back ({err})");
                }
            }
            Err(refusal(e))
        }
    }
}

// ── Category suggestions ──

#[derive(Debug, Default, Deserialize)]
pub struct SuggestBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub name: String,
    #[serde(default)]
    pub note: String,
}

pub async fn suggest(State(state): State<Arc<RelayState>>, Json(b): Json<SuggestBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_suggest").await?;
    suggest_core(&state.db, &b.key, &b.name, &b.note)
}

/// A member asks the admins for a category.
pub fn suggest_core(db: &Storage, key: &str, name: &str, note: &str) -> Answer {
    if !is_member(db, key) {
        return Err(forbidden("Only members of this server can suggest a category."));
    }
    if note.chars().count() > store::SUGGESTION_NOTE_MAX {
        return Err((StatusCode::BAD_REQUEST, format!("A note can be at most {} characters.", store::SUGGESTION_NOTE_MAX)));
    }
    let s = db.suggest_media_category(key, name, note).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "suggestion": s })))
}

/// The signature alone, for the signed reads (in the query, or in a POST body).
#[derive(Debug, Default, Deserialize)]
pub struct SignedOnly {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
}

pub async fn suggestions(State(state): State<Arc<RelayState>>, Query(b): Query<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_suggestions").await?;
    suggestions_core(&state.db, &b.key)
}

pub async fn suggestions_post(State(state): State<Arc<RelayState>>, Json(b): Json<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_suggestions").await?;
    suggestions_core(&state.db, &b.key)
}

/// An admin gets every suggestion; a member their own (so someone who was away when theirs was
/// decided still finds out); anyone else nothing.
pub fn suggestions_core(db: &Storage, key: &str) -> Answer {
    let admin = is_admin(db, key);
    if !admin && !is_member(db, key) {
        return Err(forbidden("Only members of this server can see category suggestions."));
    }
    let list = db.media_suggestions(if admin { None } else { Some(key) }).map_err(db_err)?;
    Ok(Json(json!({ "suggestions": list, "all": admin })))
}

#[derive(Debug, Default, Deserialize)]
pub struct DecideBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    pub accept: bool,
}

/// The purpose a decision is signed over: `media_suggestions:<suggestion id>:<accept|decline>`.
pub fn decide_purpose(id: i64, accept: bool) -> String {
    format!("media_suggestions:{id}:{}", if accept { "accept" } else { "decline" })
}

pub async fn decide(State(state): State<Arc<RelayState>>, Json(b): Json<DecideBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, &decide_purpose(b.id, b.accept)).await?;
    let (answer, (to, message)) = decide_core(&state.db, &b.key, b.id, b.accept)?;
    // The suggester is told, on every client of theirs signed in now; the suggestions list keeps
    // the decision for when they were away.
    let _ = state.broadcast_tx.send(RelayMessage::Private { to, message });
    Ok(answer)
}

/// An admin accepts (the category is added) or declines a suggestion. Also returns who to tell
/// and the words.
pub fn decide_core(db: &Storage, key: &str, id: i64, accept: bool) -> Result<(Json<Value>, (String, String)), Refused> {
    if !is_admin(db, key) {
        return Err(forbidden("Only an admin can decide category suggestions."));
    }
    let (s, category) = db.decide_media_suggestion(id, accept).map_err(refusal)?;
    let words = if accept {
        format!("Your category suggestion \"{}\" was accepted: it is now a category on the Media page.", s.name)
    } else {
        format!("Your category suggestion \"{}\" was declined by an admin.", s.name)
    };
    Ok((Json(json!({ "ok": true, "suggestion": s, "category": category })), (s.suggester_key.clone(), words)))
}

// ── POST /api/media/categories (the whole list) ──

#[derive(Debug, Default, Deserialize)]
pub struct CategoriesBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub categories: Vec<CategoryEdit>,
}

/// The body and the purpose its signature is over: `media_categories:<BLAKE3 hex of the
/// request's category list as sent>`, the `categories` member's JSON text exactly as it arrived
/// (handlers/reports.rs `raw_member`), so the signature holds that list and no other (F5).
pub fn categories_request(text: &str) -> Result<(CategoriesBody, String), Refused> {
    let b: CategoriesBody =
        serde_json::from_str(text).map_err(|e| (StatusCode::BAD_REQUEST, format!("Not a category list: {e}")))?;
    let sent = crate::relay::handlers::reports::raw_member(text, "categories")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "The request has no category list.".to_string()))?;
    Ok((b, format!("media_categories:{}", blake3::hash(sent.as_bytes()).to_hex())))
}

pub async fn set_categories(State(state): State<Arc<RelayState>>, text: String) -> Answer {
    let (b, purpose) = categories_request(&text)?;
    check_signed(&state, &b.key, b.timestamp, &b.sig, &purpose).await?;
    categories_core(&state.db, &b.key, &b.categories)
}

/// Admins replace the whole list (add, rename, remove, reorder); a removed category's items move
/// to General.
pub fn categories_core(db: &Storage, key: &str, list: &[CategoryEdit]) -> Answer {
    if !is_admin(db, key) {
        return Err(forbidden("Only an admin can change the Media categories."));
    }
    let list = db.replace_media_categories(list).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "categories": list })))
}

// ── GET or POST /api/media/usage ──

pub async fn usage(State(state): State<Arc<RelayState>>, Query(b): Query<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_usage").await?;
    usage_core(&state.db, &media_settings(&state.db), &b.key)
}

pub async fn usage_post(State(state): State<Arc<RelayState>>, Json(b): Json<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_usage").await?;
    usage_core(&state.db, &media_settings(&state.db), &b.key)
}

/// Admins: the meter ("12.4 MB of 2048.0 MB used"), each person's use, the largest, oldest and
/// least viewed items for pruning, and the uploads waiting to be published, with their sizes and
/// when the expiry pass deletes them (F1).
pub fn usage_core(db: &Storage, s: &ServerSettings, key: &str) -> Answer {
    if !is_admin(db, key) {
        return Err(forbidden("Only an admin can see how the Media space is used."));
    }
    let u = db.media_usage().map_err(db_err)?;
    let budget = s.media_budget_mb.max(1) * MIB;
    Ok(Json(json!({
        "used_bytes": u.used_bytes,
        "budget_bytes": budget,
        "per_person_bytes": s.media_per_person_mb.max(1) * MIB,
        "meter": format!("{} of {} used", store::mb(u.used_bytes), store::mb(budget)),
        "items": u.items,
        "people": u.people,
        "largest": items_json(&u.largest),
        "oldest": items_json(&u.oldest),
        "least_viewed": items_json(&u.least_viewed),
        "waiting": u.waiting,
        // A7: the items the server keeps, inside used_bytes, in nobody's share.
        "kept_bytes": u.kept_bytes,
    })))
}

// ── GET or POST /api/media/mine ──

pub async fn mine(State(state): State<Arc<RelayState>>, Query(b): Query<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_mine").await?;
    mine_core(&state.db, &media_settings(&state.db), &b.key)
}

pub async fn mine_post(State(state): State<Arc<RelayState>>, Json(b): Json<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_mine").await?;
    mine_core(&state.db, &media_settings(&state.db), &b.key)
}

/// A member's own use of the Media space (A4, 2026-10-11), counted exactly as the room check
/// counts it ([`media_room`]: `media_bytes`), so "You have X of your Y left" is the relay's own
/// number: `used_bytes` (their items and their uploads waiting to be published; items the server
/// keeps are not in it), `share_bytes`, `budget_left_bytes` (what the whole Media budget has
/// left), `in_progress_bytes` (their resumable uploads still arriving, which a new resumable
/// upload's start counts as used), `file_limit_bytes` (their role's largest Media file, B1; 0 is
/// none: up to the share) and `waiting` (their uploads not yet published, with the names they had
/// on their device).
pub fn mine_core(db: &Storage, s: &ServerSettings, key: &str) -> Answer {
    if !is_member(db, key) {
        return Err(forbidden("Only members of this server have a share of its Media space."));
    }
    let budget = s.media_budget_mb.max(1) * MIB;
    let in_progress: i64 = db.upload_sessions_of(key).map_err(db_err)?.iter().filter(|s| !s.finished()).map(|s| s.size).sum();
    let role = db.role_def(&db.get_role(key).unwrap_or_default());
    Ok(Json(json!({
        "used_bytes": db.media_bytes(Some(key)),
        "share_bytes": s.media_per_person_mb.max(1) * MIB,
        "budget_left_bytes": (budget - db.media_bytes(None)).max(0),
        "in_progress_bytes": in_progress,
        "file_limit_bytes": crate::relay::upload_rules::media_file_limit(&role).unwrap_or(0),
        "waiting": db.media_waiting_of(key).map_err(db_err)?,
    })))
}

// ── GET /api/media/file/{id}?token= ──

/// The session's upload token, the one `POST /api/upload` takes.
#[derive(Debug, Default, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

/// The header the members' file route also takes the session's upload token in (W3b,
/// 2026-10-11): an app fetches with it and shows the file through an address of its own, so the
/// token is never in an `<img src>`, a Download link, a browser's downloads list or a server log.
/// `?token=` still works for now; the apps move to the header.
pub const UPLOAD_TOKEN_HEADER: &str = "x-upload-token";

pub async fn file(
    State(state): State<Arc<RelayState>>,
    Path(id): Path<i64>,
    Query(q): Query<TokenQuery>,
    req: axum::extract::Request,
) -> Response {
    file_from(&state, Folders::live(), id, q, req).await
}

/// [`file`] with the folders named: the token from the [`UPLOAD_TOKEN_HEADER`] header when the
/// request has one, else from `?token=`.
pub async fn file_from(state: &RelayState, folders: Folders<'_>, id: i64, q: TokenQuery, req: axum::extract::Request) -> Response {
    let from_header = req.headers().get(UPLOAD_TOKEN_HEADER).and_then(|v| v.to_str().ok()).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    let token = from_header.or(q.token);
    file_core(state, folders, id, token.as_deref(), req).await.unwrap_or_else(IntoResponse::into_response)
}

/// A Media item's file, to a member of this server who is not banned, proven by the session's
/// upload token (F3): the way a members-only item's file is fetched, since it is not in the
/// public folder. Ranges work (a video seeks), and like `/uploads` it is sent as an attachment
/// (relay/mod.rs `security_headers`) and kept out of shared caches.
pub async fn file_core(state: &RelayState, folders: Folders<'_>, id: i64, token: Option<&str>, req: axum::extract::Request) -> Result<Response, Refused> {
    let token = token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "A members-only file needs this session's upload token (?token=).".to_string()))?;
    let key = state
        .upload_tokens
        .read()
        .await
        .get(token)
        .cloned()
        .ok_or_else(|| forbidden("That upload token is not a session signed in to this server."))?;
    if !is_member(&state.db, &key) {
        return Err(forbidden("Only members of this server who are not banned can open members-only Media files."));
    }
    let item = state.db.media_item(id).map_err(db_err)?.ok_or_else(|| (StatusCode::NOT_FOUND, "There is no Media item with that number.".to_string()))?;
    let path = folders
        .find(&item.upload_filename)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "That Media item's file is missing.".to_string()))?;
    let res = tower_http::services::ServeFile::new(path)
        .try_call(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("The file could not be read: {e}")))?;
    let mut res = res.map(axum::body::Body::new);
    res.headers_mut().insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    Ok(res)
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;

// The increment 2 follow-ups' tests (2026-10-11): A1 to A7, W3b, W5, R5, R6.
#[cfg(test)]
#[path = "media_follow_ups_tests.rs"]
mod follow_ups_tests;

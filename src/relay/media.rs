//! The Media page's routes (2026-10-11; docs/design/media-showcase.md, "Increment 1, exact").
//! Storage is storage/media.rs; the routes are registered in relay/mod.rs `build_router` and
//! gated with the uploads in relay/features.rs (`/api/media` belongs to `Feature::Uploads`).
//!
//! WHO SEES WHAT. `GET /api/media` gives public items to anyone while the server's
//! `media_visibility` is "public"; members-only items, and everything on a members-only server,
//! only to a signed request from a member (purpose `media_list`). A signed read can ride in the
//! query or, through `POST /api/media/list`, in a body: a Dilithium key and signature make a URL
//! of about 10 KB, which a proxy refuses (the admin-stats 414, api.rs). The same two forms serve
//! the admins' suggestions and usage reads.
//!
//! EVERY WRITE IS SIGNED the way `delete_shared_upload` is (Dilithium over `purpose\ntimestamp`,
//! five minutes either side of the relay's clock, and a nonce the relay has not seen in that
//! window, so a captured request cannot be sent twice), with the purpose named per route:
//! `media_publish`, `media_remove`, `media_pick` (pick and unpick), `media_feature`,
//! `media_suggest`, `media_suggestions` (the list and the decisions), `media_categories`,
//! `media_usage`. The role checks happen after the signature, on the proven key.
//!
//! Each route's work after the signature is a plain function over the storage (`*_core`), which
//! is what the tests call: no relay, no socket, and no file written outside a temp folder
//! (`remove_core` takes the uploads folder).
//!
//! THE LABELS (spoiler, flashing lights, graphic) are data: data/media/labels.json, read at start
//! ([`labels`]). Adult content is not among them and is refused by name: it stays off the Media
//! page until it is researched and the operator decides (the design's "Labels").

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::media::{
    self as store, CategoryEdit, MediaError, MediaItem, MediaQuery, Publish, Refusal, KINDS, MEDIA_SHELF,
};
use crate::relay::storage::{media_visibility_of, ServerSettings, Storage};

/// Where uploads are kept and served from (`/uploads`, relay/mod.rs).
pub const UPLOADS_DIR: &str = "data/uploads";
/// How far a signed request's timestamp may be from the relay's clock, either way.
pub const SIGNED_WINDOW_MS: u64 = 5 * 60 * 1000;
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
    const BUILT_IN: &str = include_str!("../../data/media/labels.json");
    let shown = path.display().to_string();
    let from_disk = match std::fs::read_to_string(path) {
        Ok(text) => parse_labels(&text).map_err(|e| format!("{shown} does not parse ({e})")),
        Err(e) => Err(format!("{shown} could not be read ({e})")),
    };
    from_disk.unwrap_or_else(|why| {
        crate::embedded_data::note_builtin_copy("media/labels.json", why);
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

/// Prove the caller holds `key`: the timestamp within [`SIGNED_WINDOW_MS`] of the relay's clock,
/// the Dilithium signature over `purpose\ntimestamp` (off the async executor; a panic fails
/// closed), and a (key, purpose, timestamp) the relay has not seen in the window.
pub async fn check_signed(state: &RelayState, key: &str, timestamp: u64, sig: &str, purpose: &'static str) -> Result<(), Refused> {
    let now = now_ms();
    if now.saturating_sub(timestamp) > SIGNED_WINDOW_MS {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }
    if timestamp.saturating_sub(now) > SIGNED_WINDOW_MS {
        return Err((StatusCode::BAD_REQUEST, "Timestamp is in the future: check this device's clock.".into()));
    }
    let (k, s) = (key.to_string(), sig.to_string());
    let ok = tokio::task::spawn_blocking(move || {
        crate::relay::handlers::broadcast::verify_dilithium_signature(&k, purpose, timestamp, &s)
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
    }
}

fn db_err(e: rusqlite::Error) -> Refused {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("storage: {e}"))
}

fn forbidden(words: &str) -> Refused {
    (StatusCode::FORBIDDEN, words.to_string())
}

/// One item as every route gives it.
pub fn item_json(i: &MediaItem) -> Value {
    json!({
        "id": i.id,
        "url": format!("/uploads/{}", i.upload_filename),
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
    })
}

fn items_json(items: &[MediaItem]) -> Vec<Value> {
    items.iter().map(item_json).collect()
}

// ── Uploads: the shelf and the room (called by api.rs `upload_file`) ──

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

/// Whether there is room for a `len`-byte upload on `shelf` by `key`, with `dir_total` bytes in
/// the uploads folder now. A Media upload is held to the Media budget and the person's share of
/// it, with both numbers in the words; anything else to the general cap, `max_total_upload_mb`,
/// counting what is in the folder apart from Media's own bytes (the cap stays for chat files).
pub fn upload_room(db: &Storage, s: &ServerSettings, key: &str, shelf: i64, dir_total: u64, len: u64) -> Result<(), Refused> {
    let len = len as i64;
    let media_used = db.media_bytes(None);
    if shelf == MEDIA_SHELF {
        let budget = s.media_budget_mb.max(1) * MIB;
        if media_used + len > budget {
            return Err((StatusCode::INSUFFICIENT_STORAGE, store::budget_full_words(media_used, budget, len)));
        }
        let mine = db.media_bytes(Some(key));
        let share = s.media_per_person_mb.max(1) * MIB;
        if mine + len > share {
            return Err((StatusCode::INSUFFICIENT_STORAGE, store::share_full_words(mine, share, len)));
        }
        return Ok(());
    }
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
}

/// Whether the request gets the members' view: no signature, no; a signature from a member, yes;
/// a bad or partial signature is refused rather than quietly served the public view.
async fn members_view(state: &RelayState, p: &ListParams) -> Result<bool, Refused> {
    match (p.key.as_deref().filter(|k| !k.is_empty()), p.timestamp, p.sig.as_deref().filter(|s| !s.is_empty())) {
        (None, None, None) => Ok(false),
        (Some(key), Some(ts), Some(sig)) => {
            check_signed(state, key, ts, sig, "media_list").await?;
            Ok(is_member(&state.db, key))
        }
        _ => Err((StatusCode::BAD_REQUEST, "A signed request needs key, timestamp and sig together.".into())),
    }
}

/// The list for a viewer with or without the members' view (`members`), optionally one person's
/// picks. On a members-only server the public gets no items, and is told so.
pub fn list_core(db: &Storage, s: &ServerSettings, p: &ListParams, members: bool, picked_by: Option<String>) -> Answer {
    if let Some(kind) = p.kind.as_deref() {
        if !KINDS.contains(&kind) {
            return Err((StatusCode::BAD_REQUEST, format!("\"{kind}\" is not a kind. The kinds are: {}.", KINDS.join(", "))));
        }
    }
    let members_only = s.media_visibility == "members";
    let items = if members_only && !members {
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
        })
        .map_err(db_err)?
    };
    let limit = p.limit.unwrap_or(store::MEDIA_LIST_DEFAULT).clamp(1, store::MEDIA_LIST_MAX) as usize;
    let next_before = if items.len() == limit { items.last().map(|i| i.id) } else { None };
    Ok(Json(json!({
        "items": items_json(&items),
        "next_before": next_before,
        "members_view": members,
        "media_visibility": s.media_visibility,
        "members_only_for_you": members_only && !members,
    })))
}

pub async fn list(State(state): State<Arc<RelayState>>, Query(p): Query<ListParams>) -> Answer {
    let members = members_view(&state, &p).await?;
    list_core(&state.db, &state.db.get_server_settings().unwrap_or_default(), &p, members, None)
}

pub async fn list_post(State(state): State<Arc<RelayState>>, Json(p): Json<ListParams>) -> Answer {
    let members = members_view(&state, &p).await?;
    list_core(&state.db, &state.db.get_server_settings().unwrap_or_default(), &p, members, None)
}

/// One person's picks, as the list (the same filters and the same rule about who sees what).
pub async fn picks_of(State(state): State<Arc<RelayState>>, Path(key): Path<String>, Query(p): Query<ListParams>) -> Answer {
    let members = members_view(&state, &p).await?;
    list_core(&state.db, &state.db.get_server_settings().unwrap_or_default(), &p, members, Some(key))
}

// ── GET /api/media/categories ──

pub async fn categories(State(state): State<Arc<RelayState>>) -> Answer {
    let list = state.db.media_categories().map_err(db_err)?;
    Ok(Json(json!({ "categories": list, "labels": labels() })))
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

pub async fn publish(State(state): State<Arc<RelayState>>, Json(b): Json<PublishBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_publish").await?;
    let settings = state.db.get_server_settings().unwrap_or_default();
    publish_core(&state.db, &settings, labels(), std::path::Path::new(UPLOADS_DIR), &b)
}

/// Publish for the proven `b.key` (a member, not banned): one of their own uploads, never an
/// encrypted one, within the Media budget and their share of it.
pub fn publish_core(db: &Storage, s: &ServerSettings, known: &[MediaLabel], uploads: &std::path::Path, b: &PublishBody) -> Answer {
    if !is_member(db, &b.key) {
        return Err(forbidden("Only members of this server can put things on its Media page."));
    }
    let filename = b.filename.trim().trim_start_matches("/uploads/");
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err((StatusCode::BAD_REQUEST, "Bad file name.".into()));
    }
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
    let disk_size = std::fs::metadata(uploads.join(filename)).ok().map(|m| m.len() as i64);
    let item = db
        .publish_media(
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
        )
        .map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "item": item_json(&item) })))
}

// ── POST /api/media/remove, /pick, /unpick, /feature ──

#[derive(Debug, Default, Deserialize)]
pub struct ItemBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
}

pub async fn remove(State(state): State<Arc<RelayState>>, Json(b): Json<ItemBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_remove").await?;
    remove_core(&state.db, std::path::Path::new(UPLOADS_DIR), b.id, &b.key)
}

/// The uploader or an admin takes item `id` down: the item, its picks, its upload row, and the
/// file in `uploads`.
pub fn remove_core(db: &Storage, uploads: &std::path::Path, id: i64, requester: &str) -> Answer {
    let filename = db.remove_media(id, requester, is_admin(db, requester)).map_err(refusal)?;
    // The stored name is the upload route's own, but it is never joined unless it is a bare name.
    let bare = !(filename.contains('/') || filename.contains('\\') || filename.contains(".."));
    let file_removed = bare && std::fs::remove_file(uploads.join(&filename)).is_ok();
    tracing::info!("A Media item was removed (file removed: {file_removed})");
    Ok(Json(json!({ "ok": true, "removed": id, "file_removed": file_removed })))
}

pub async fn pick(State(state): State<Arc<RelayState>>, Json(b): Json<ItemBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_pick").await?;
    pick_core(&state.db, &b.key, b.id, true)
}

pub async fn unpick(State(state): State<Arc<RelayState>>, Json(b): Json<ItemBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_pick").await?;
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
}

pub async fn feature(State(state): State<Arc<RelayState>>, Json(b): Json<FeatureBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_feature").await?;
    feature_core(&state.db, &b.key, b.id, b.featured)
}

/// Admins only.
pub fn feature_core(db: &Storage, key: &str, id: i64, featured: bool) -> Answer {
    if !is_admin(db, key) {
        return Err(forbidden("Only an admin can feature Media items."));
    }
    db.set_media_featured(id, featured).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "id": id, "featured": featured })))
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

pub async fn decide(State(state): State<Arc<RelayState>>, Json(b): Json<DecideBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_suggestions").await?;
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

pub async fn set_categories(State(state): State<Arc<RelayState>>, Json(b): Json<CategoriesBody>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_categories").await?;
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
    usage_core(&state.db, &state.db.get_server_settings().unwrap_or_default(), &b.key)
}

pub async fn usage_post(State(state): State<Arc<RelayState>>, Json(b): Json<SignedOnly>) -> Answer {
    check_signed(&state, &b.key, b.timestamp, &b.sig, "media_usage").await?;
    usage_core(&state.db, &state.db.get_server_settings().unwrap_or_default(), &b.key)
}

/// Admins: the meter ("12.4 MB of 2048.0 MB used"), each person's use, and the largest, oldest
/// and least viewed items for pruning.
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
    })))
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;

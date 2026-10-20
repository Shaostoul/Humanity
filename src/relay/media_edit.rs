//! Editing a Media item where it is, and its other files (Media round 5, 2026-10-12;
//! docs/design/media-round-5-plan.md 3.4, with the orchestrator's decisions in its section 9).
//! Storage is storage/media_files.rs.
//!
//! - `POST /api/media/edit`: the title, description, category, labels, licence and the
//!   game tick (`project_use`, "Offer this for the game"), every one sent (the form sends what it
//!   shows). Signed over
//!   `media_edit:<id>:<BLAKE3 hex of the edit member's JSON text exactly as sent>` ([`edit_request`],
//!   the categories route's pattern), so the signature holds that edit and no other. The uploader
//!   or an admin; an admin never changes the licence or the game tick (decision 1: a licence is
//!   the rights holder's grant), and a moderator edits nothing. Answer: `{ok: true, item}`.
//! - `POST /api/media/files/add`: one of the uploader's own uploads waiting on the Media shelf,
//!   added as a file or as the cover. Signed over `media_file_add:<id>:<upload>:<file|cover>`.
//!   Under the room lock, the file moves from the waiting folder into the item's folder (the
//!   public one, or the members' one for a members-only item) before anything is committed. A new
//!   cover replaces the old one, whose file goes. The uploader only. Answer: `{ok: true, item}`.
//! - `POST /api/media/files/remove`: an added file or the cover taken off (never file 0). Signed
//!   over `media_file_remove:<id>:<file number>`. The uploader or an admin. The file was uploaded
//!   for Media, so its row and its file go. Answer: `{ok: true, item, file_removed}`.
//!
//! - `POST /api/media/files/rename` (2026-10-17): a file's shown name changed, the item's own
//!   (file 0) included; it keeps its stored ending. Signed over
//!   `media_file_rename:<id>:<file number>:<BLAKE3 hex of the new name as sent>`. The uploader or an
//!   admin. Answer: `{ok: true, item}`.
//! - `POST /api/media/files/cover` (2026-10-17): a picture already on the item made its cover; the
//!   old cover becomes one of its files. Signed over `media_file_cover:<id>:<file number>`. The
//!   uploader only. Answer: `{ok: true, item}`.
//!
//! Each names the item's own file too (`filename`, the relay's W5, [`same_item`]), and the file
//! remove names the file's own stored name too (`file_name`, the same rule for the file). The
//! work after the signature is a plain function over the storage (`*_core`), which the tests call
//! (media_edit_tests.rs).

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::relay::media::{
    check_signed, db_err, forbidden, is_admin, is_member, item_json, labels, normalize_labels, refusal, room_lock, same_item, stored_name,
    MediaLabel,
};
use crate::relay::media_licences::{licences, LicencesFile};
use crate::relay::relay::RelayState;
use crate::relay::storage::media::{self as store, Edit, Folders, OwnedFolders, Signed, Spot};
use crate::relay::storage::Storage;

type Refused = (StatusCode, String);
type Answer = Result<Json<Value>, Refused>;

/// What someone who is not a member of this server (or is banned from it) is told when they try
/// to change a Media item or add to one.
pub const MEMBERS_ONLY: &str = "Only members of this server can change items on the Files page.";

/// What an edit sets: every field, required.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EditFields {
    pub title: String,
    pub description: String,
    pub category_id: i64,
    pub labels: Vec<String>,
    pub licence: String,
    pub project_use: bool,
}

/// `POST /api/media/edit`'s body.
#[derive(Debug, Clone, Deserialize)]
pub struct EditBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    /// The item's own file name (W5).
    #[serde(default)]
    pub filename: String,
    pub edit: EditFields,
}

/// The purpose an edit of item `id` is signed over: `media_edit:<id>:<BLAKE3 hex of edit_text>`,
/// where `edit_text` is the `edit` member's JSON text exactly as it is sent.
pub fn edit_purpose(id: i64, edit_text: &str) -> String {
    format!("media_edit:{id}:{}", blake3::hash(edit_text.as_bytes()).to_hex())
}

/// The body of an edit request, its `edit` member's text exactly as it arrived, and the purpose
/// its signature must be over (handlers/reports.rs `raw_member`): a body whose edit was changed
/// after it was signed never matches.
pub fn edit_request(text: &str) -> Result<(EditBody, String, String), Refused> {
    let b: EditBody = serde_json::from_str(text).map_err(|e| (StatusCode::BAD_REQUEST, format!("Not an edit: {e}")))?;
    let sent = crate::relay::handlers::reports::raw_member(text, "edit")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "The request has no edit.".to_string()))?;
    let purpose = edit_purpose(b.id, sent);
    Ok((b, sent.to_string(), purpose))
}

/// `POST /api/media/edit`.
pub async fn edit(State(state): State<Arc<RelayState>>, text: String) -> Response {
    edit_signed(&state, &text).await.into_response()
}

/// [`edit`] over the request's text: the signature checked over the edit as sent, the item named
/// by its own file (W5), then [`edit_core`] with the relay's labels and licences, and the signature
/// kept with a licence choice.
pub async fn edit_signed(state: &RelayState, text: &str) -> Answer {
    let (b, sent, purpose) = edit_request(text)?;
    check_signed(state, &b.key, b.timestamp, &b.sig, &purpose).await?;
    same_item(&state.db, b.id, &b.filename)?;
    let signed = Signed { key: &b.key, purpose: &purpose, body: &sent, ts: b.timestamp as i64, sig: &b.sig };
    edit_core(&state.db, labels(), licences(), &b, &signed)
}

/// The edit for the proven `b.key`: an admin, or a member (not banned) whose item it is (storage
/// refuses anyone else, and an admin's change of the licence or the game tick). The publish's own
/// words for the lengths and the labels, the category checked by storage. The licence and the game
/// tick are checked against what this server offers (`offered`, data/media/licences.json) only
/// when they change: an item keeps the licence it was given even if this server stops offering
/// it, so its words can still be edited.
pub fn edit_core(db: &Storage, known: &[MediaLabel], offered: &LicencesFile, b: &EditBody, signed: &Signed) -> Answer {
    let admin = is_admin(db, &b.key);
    if !admin && !is_member(db, &b.key) {
        return Err(forbidden(MEMBERS_ONLY));
    }
    let e = &b.edit;
    if e.title.chars().count() > store::MEDIA_TITLE_MAX {
        return Err((StatusCode::BAD_REQUEST, format!("A title can be at most {} characters.", store::MEDIA_TITLE_MAX)));
    }
    if e.description.chars().count() > store::MEDIA_DESCRIPTION_MAX {
        return Err((StatusCode::BAD_REQUEST, format!("A description can be at most {} characters.", store::MEDIA_DESCRIPTION_MAX)));
    }
    let labels = normalize_labels(&e.labels, known).map_err(|w| (StatusCode::BAD_REQUEST, w))?;
    let now = db.media_item(b.id).map_err(db_err)?.ok_or_else(|| (StatusCode::NOT_FOUND, "There is no item with that number on the Files page.".to_string()))?;
    if e.licence != now.licence || e.project_use != now.project_use {
        offered.check_choice(&e.licence, e.project_use).map_err(|w| (StatusCode::BAD_REQUEST, w))?;
    }
    let edit = Edit { title: &e.title, description: &e.description, category_id: e.category_id, labels: &labels, licence: &e.licence, project_use: e.project_use };
    let item = db.edit_media(&b.key, admin, b.id, &edit, signed).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "item": item_json(&item) })))
}

/// `POST /api/media/files/add`'s body: the item, its own file name, the upload to add, and as what.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileAddBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    #[serde(default)]
    pub filename: String,
    /// The stored name of the upload to add.
    pub add: String,
    /// `file` or `cover`.
    #[serde(rename = "as")]
    pub role: String,
}

/// The purpose adding `add` to item `id` as `role` is signed over.
pub fn file_add_purpose(id: i64, add: &str, role: &str) -> String {
    format!("media_file_add:{id}:{add}:{role}")
}

/// `POST /api/media/files/add`.
pub async fn files_add(State(state): State<Arc<RelayState>>, Json(b): Json<FileAddBody>) -> Response {
    files_add_in(state, OwnedFolders::of(Folders::live()), b).await.into_response()
}

/// [`files_add`] with the folders named: the signature over the upload's stored name and its role,
/// the item named by its own file (W5), then [`files_add_core`] off the executor (it waits for the
/// room lock, which an upload holds while its file moves into place).
pub async fn files_add_in(state: Arc<RelayState>, folders: OwnedFolders, b: FileAddBody) -> Answer {
    let purpose = file_add_purpose(b.id, stored_name(&b.add)?, &b.role);
    check_signed(&state, &b.key, b.timestamp, &b.sig, &purpose).await?;
    same_item(&state.db, b.id, &b.filename)?;
    tokio::task::spawn_blocking(move || files_add_core(&state.db, folders.get(), &b))
        .await
        .unwrap_or_else(|e| Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Adding the file failed: {e}"))))
}

/// The proven `b.key`, a member who is not banned, adds their upload `b.add` to their item `b.id`
/// as a file or its cover (storage/media_files.rs `attach_media_file` holds every rule: their own
/// upload, waiting on the Media shelf and on no item; their role's largest Media file; at most 20
/// files; a cover only a picture). Under the room lock, the file moves out of the waiting folder
/// into the item's folder before anything is committed, and back where it was when nothing is. A
/// cover it replaced loses its file.
pub fn files_add_core(db: &Storage, folders: Folders, b: &FileAddBody) -> Answer {
    if !is_member(db, &b.key) {
        return Err(forbidden(MEMBERS_ONLY));
    }
    let add = stored_name(&b.add)?;
    let max_file_mb = db.role_def(&db.get_role(&b.key).unwrap_or_default()).max_media_file_mb;
    let _room = room_lock();
    let moved: std::cell::Cell<Option<Spot>> = std::cell::Cell::new(None);
    let attached = db.attach_media_file(&b.key, b.id, add, &b.role, max_file_mb, |members| {
        let from = folders.move_to(add, if members { Spot::Members } else { Spot::Public })?;
        moved.set(from);
        Ok(from.is_some())
    });
    match attached {
        Ok(a) => {
            if let Some(old) = a.replaced.as_deref() {
                let gone = folders.delete(old);
                tracing::info!("A Media item's cover was replaced (the old file removed: {gone})");
            }
            Ok(Json(json!({ "ok": true, "item": item_json(&a.item) })))
        }
        Err(e) => {
            // Nothing was committed: a file this moved goes back where it was.
            if let Some(from) = moved.get() {
                if let Err(err) = folders.move_to(add, from) {
                    tracing::error!("Media: a refused file add could not put its file back ({err})");
                }
            }
            Err(refusal(e))
        }
    }
}

/// `POST /api/media/files/remove`'s body: the item, its own file name, the file's number and that
/// file's stored name (W5 for the file).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileRemoveBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    #[serde(default)]
    pub filename: String,
    pub file: i64,
    #[serde(default)]
    pub file_name: String,
}

/// The purpose taking file `file` off item `id` is signed over.
pub fn file_remove_purpose(id: i64, file: i64) -> String {
    format!("media_file_remove:{id}:{file}")
}

/// `POST /api/media/files/remove`.
pub async fn files_remove(State(state): State<Arc<RelayState>>, Json(b): Json<FileRemoveBody>) -> Response {
    files_remove_in(&state, Folders::live(), &b).await.into_response()
}

/// [`files_remove`] with the folders named: the signature, the item named by its own file (W5),
/// then [`files_remove_core`].
pub async fn files_remove_in(state: &RelayState, folders: Folders<'_>, b: &FileRemoveBody) -> Answer {
    check_signed(state, &b.key, b.timestamp, &b.sig, &file_remove_purpose(b.id, b.file)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    files_remove_core(&state.db, folders, b)
}

/// The proven `b.key`, the item's uploader or an admin, takes file `b.file` off item `b.id`: never
/// file 0 (the item itself: remove the item instead), and only when `b.file_name` is that file's
/// stored name (W5). The file was uploaded for Media, so its row goes and its file is deleted.
pub fn files_remove_core(db: &Storage, folders: Folders, b: &FileRemoveBody) -> Answer {
    let (item, name) = db.detach_media_file(&b.key, is_admin(db, &b.key), b.id, b.file, &b.file_name).map_err(refusal)?;
    let file_removed = folders.delete(&name);
    Ok(Json(json!({ "ok": true, "item": item_json(&item), "file_removed": file_removed })))
}

/// `POST /api/media/files/rename`'s body: the item, its own file name, the file's number, that
/// file's stored name (W5), and the name it is to have.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileRenameBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    #[serde(default)]
    pub filename: String,
    pub file: i64,
    #[serde(default)]
    pub file_name: String,
    pub name: String,
}

/// The purpose renaming file `file` of item `id` to `name` is signed over: the name as its BLAKE3
/// hex, so the signature holds that name and no other.
pub fn file_rename_purpose(id: i64, file: i64, name: &str) -> String {
    format!("media_file_rename:{id}:{file}:{}", blake3::hash(name.as_bytes()).to_hex())
}

/// `POST /api/media/files/rename`.
pub async fn files_rename(State(state): State<Arc<RelayState>>, Json(b): Json<FileRenameBody>) -> Response {
    files_rename_in(&state, &b).await.into_response()
}

/// [`files_rename`]: the signature, the item named by its own file (W5), then [`files_rename_core`].
pub async fn files_rename_in(state: &RelayState, b: &FileRenameBody) -> Answer {
    check_signed(state, &b.key, b.timestamp, &b.sig, &file_rename_purpose(b.id, b.file, &b.name)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    files_rename_core(&state.db, b)
}

/// The proven `b.key`, the item's uploader or an admin, names file `b.file` of item `b.id` `b.name`
/// (storage/media_files.rs `rename_media_file` holds the rules).
pub fn files_rename_core(db: &Storage, b: &FileRenameBody) -> Answer {
    let item = db.rename_media_file(&b.key, is_admin(db, &b.key), b.id, b.file, &b.file_name, &b.name).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "item": item_json(&item) })))
}

/// `POST /api/media/files/cover`'s body: the item, its own file name, the picture's number and its
/// stored name (W5).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileCoverBody {
    pub key: String,
    pub timestamp: u64,
    pub sig: String,
    pub id: i64,
    #[serde(default)]
    pub filename: String,
    pub file: i64,
    #[serde(default)]
    pub file_name: String,
}

/// The purpose making file `file` item `id`'s cover is signed over.
pub fn file_cover_purpose(id: i64, file: i64) -> String {
    format!("media_file_cover:{id}:{file}")
}

/// `POST /api/media/files/cover`.
pub async fn files_cover(State(state): State<Arc<RelayState>>, Json(b): Json<FileCoverBody>) -> Response {
    files_cover_in(&state, &b).await.into_response()
}

/// [`files_cover`]: the signature, the item named by its own file (W5), then [`files_cover_core`].
pub async fn files_cover_in(state: &RelayState, b: &FileCoverBody) -> Answer {
    check_signed(state, &b.key, b.timestamp, &b.sig, &file_cover_purpose(b.id, b.file)).await?;
    same_item(&state.db, b.id, &b.filename)?;
    files_cover_core(&state.db, b)
}

/// The proven `b.key`, the item's uploader, makes picture file `b.file` the cover of item `b.id`
/// (storage/media_files.rs `cover_from_file` holds the rules).
pub fn files_cover_core(db: &Storage, b: &FileCoverBody) -> Answer {
    let item = db.cover_from_file(&b.key, b.id, b.file, &b.file_name).map_err(refusal)?;
    Ok(Json(json!({ "ok": true, "item": item_json(&item) })))
}

#[cfg(test)]
#[path = "media_edit_tests.rs"]
mod tests;

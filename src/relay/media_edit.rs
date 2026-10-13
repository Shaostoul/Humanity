//! Editing a Media item where it is, and its other files (Media round 5, 2026-10-12;
//! docs/design/media-round-5-plan.md 3.4). Storage is storage/media_files.rs.
//!
//! - `POST /api/media/edit`: the title, description, category, labels, licence and the
//!   game tick (`project_use`, "Offer this for the game"), every one sent (the form sends what it
//!   shows). Signed over
//!   `media_edit:<id>:<BLAKE3 hex of the edit member's JSON text exactly as sent>` ([`edit_request`],
//!   the categories route's pattern), so the signature holds that edit and no other. The uploader
//!   or an admin; an admin never changes the licence or the game tick.
//! - `POST /api/media/files/add`: one of the uploader's own uploads waiting on the Media shelf,
//!   added as a file or as the cover. Signed over `media_file_add:<id>:<upload>:<file|cover>`.
//! - `POST /api/media/files/remove`: an added file or the cover taken off (never file 0). Signed
//!   over `media_file_remove:<id>:<file number>`.
//!
//! Each names the item's own file too (`filename`, the relay's W5). The routes are lane 1A's:
//! until it lands they answer 501 "Not built yet.", with the signatures, bodies and purposes they
//! keep, which both apps already build to (the shared fixture, scripts/tests/fixtures/
//! media-round5-v1.json).

use axum::{extract::State, http::StatusCode, response::Response, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::relay::relay::RelayState;

type Refused = (StatusCode, String);

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

/// `POST /api/media/edit` (lane 1A).
pub async fn edit(State(_state): State<Arc<RelayState>>, _text: String) -> Response {
    crate::relay::media_counts::not_built()
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

/// `POST /api/media/files/add` (lane 1A).
pub async fn files_add(State(_state): State<Arc<RelayState>>, Json(_b): Json<FileAddBody>) -> Response {
    crate::relay::media_counts::not_built()
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

/// `POST /api/media/files/remove` (lane 1A).
pub async fn files_remove(State(_state): State<Arc<RelayState>>, Json(_b): Json<FileRemoveBody>) -> Response {
    crate::relay::media_counts::not_built()
}

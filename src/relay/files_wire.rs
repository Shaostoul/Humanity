//! The Files page's wire, round 5 (2026-10-12; docs/design/media-round-5-plan.md 3.2 to 3.4): one
//! item's files as the wire carries them, the list's sort and cursor, the licences an answer
//! carries, and the purposes a publish and a feature are signed over. A child of relay/files.rs
//! (`#[path]`), so it reads that file's private helpers; kept apart so media.rs stays inside its
//! budget (tests/file_size_ratchet.rs) for the lanes that fill its routes.

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::{members_only_server, ListParams, Refused};
use crate::relay::storage::files::{self as store, Featured, ItemFile, FilesItem, Sort};
use crate::relay::storage::ServerSettings;

/// Where file `file` of item `i` (stored as `filename`) is fetched: its `/uploads/` address while
/// the item is public; for a members-only item, the members' file route with `?file=<n>` (round 5).
pub fn file_url(i: &FilesItem, file: i64, filename: &str) -> String {
    if i.visibility == "public" {
        format!("/uploads/{filename}")
    } else {
        format!("/api/files/file/{}?file={file}", i.id)
    }
}

/// One of an item's files as the wire carries it (round 5, plan 3.2).
pub(super) fn file_json(i: &FilesItem, f: &ItemFile) -> Value {
    json!({
        "file": f.file,
        "filename": f.filename,
        "original_name": f.original_name,
        "kind": f.kind,
        "size_bytes": f.size_bytes,
        "url": file_url(i, f.file, &f.filename),
    })
}

/// What a cursor this server did not give is told (round 5).
pub const BAD_CURSOR: &str = "That cursor is not one this server gave.";

/// The list's order and the page after the cursor, from the request: 400 for a sort that is not
/// `new`, `picks` or `downloads`, or a cursor this server did not give (`"<value>.<id>"`; for the
/// newest-first order the value is the id itself).
pub fn sort_and_cursor(p: &ListParams) -> Result<(Sort, Option<(i64, i64)>), Refused> {
    let sort = match p.sort.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Sort::New,
        Some(word) => Sort::parse(word)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Sort is \"new\", \"picks\" or \"downloads\".".to_string()))?,
    };
    let cursor = match p.cursor.as_deref().filter(|c| !c.is_empty()) {
        None => None,
        Some(text) => {
            let (value, id) = store::parse_cursor(text).ok_or_else(|| (StatusCode::BAD_REQUEST, BAD_CURSOR.to_string()))?;
            if sort == Sort::New && value != id {
                return Err((StatusCode::BAD_REQUEST, BAD_CURSOR.to_string()));
            }
            Some((value, id))
        }
    };
    Ok((sort, cursor))
}

/// The licences this server offers (relay/files_licences.rs, plan 3.3), for a viewer with or
/// without the members' view: on a members-only server, nothing for anyone else.
pub(super) fn licences_for(s: &ServerSettings, members: bool) -> Value {
    if members_only_server(s) && !members {
        return Value::Null;
    }
    crate::relay::files_licences::licences().answer()
}

/// The purpose a publish is signed over (round 5): `media_publish:<file name>:<licence id>:<0|1>`,
/// so the licence and the game tick are signed with the publish.
pub fn publish_purpose(filename: &str, licence: &str, project_use: bool) -> String {
    format!("media_publish:{filename}:{licence}:{}", project_use as u8)
}

/// The purpose featuring item `id` at `to` is signed over: `media_feature:<id>:<main|category|none>`.
pub fn feature_purpose(id: i64, to: Featured) -> String {
    format!("media_feature:{id}:{}", to.word())
}

//! Restore, the handler (2026-11-20, docs/design/hide-and-keep.md rung 5). A child of
//! handlers/held.rs (#[path]); the storage half is storage/held_restore.rs.
//!
//! The owner signs `held_restore` and sends it as a `held_act` (held.rs records it: a grant of
//! [`super::EXPORT_GRANT_MS`]). Their device then fetches each sealed part under that grant, opens
//! it, and sends each file back here in pieces (`PUT /api/held/{id}/{part}?grant=&offset=`, at most
//! `RESTORE_PIECE_MAX` each, under nginx's 1 MB limit on `/api/`), with the restore key part 0
//! carried in the `x-held-restore-key` header. Finally `POST /api/held/{id}/restore?grant=` (the
//! same header) puts the post back. The key opens the restore plan the relay sealed when it held
//! the post; every byte is checked against it, and the words, the names and the folders come from
//! it, never from the device. The grant alone does nothing: every request is signed by the owner's
//! own key (held.rs `signed_caller`, the owner review's finding 1), and without the restore key
//! nothing is taken. The answer to anything else is the same 404 the export uses.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use super::{caller_purpose, granted_item, handle_list, now_ms, room, signed_caller, tell, GrantQuery};
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::files::{Folders, OwnedFolders};
use crate::relay::storage::held::restore::{expected, open_plan, take_piece, Piece, PieceRefused};
use crate::relay::storage::held::HeldRow;

/// The header the restore key travels in.
pub const KEY_HEADER: &str = "x-held-restore-key";

#[derive(serde::Deserialize)]
pub struct PieceQuery {
    #[serde(default)]
    pub grant: String,
    #[serde(default)]
    pub offset: u64,
}

fn refuse(code: StatusCode, words: impl Into<String>) -> Response {
    (code, Json(json!({ "error": words.into() }))).into_response()
}

/// Held item `id` under a RESTORE grant for `caller` (the key the request's signature proved), or
/// the same 404 as every other refusal.
fn restore_item(state: &RelayState, id: i64, grant: &str, caller: &str, now: i64) -> Result<HeldRow, Response> {
    match granted_item(state, id, grant, caller, now) {
        Some((row, action)) if action == "held_restore" => Ok(row),
        _ => Err(StatusCode::NOT_FOUND.into_response()),
    }
}

fn key_of(headers: &HeaderMap) -> String {
    headers.get(KEY_HEADER).and_then(|v| v.to_str().ok()).unwrap_or("").to_string()
}

/// One piece of part `part` of held item `id` (the module's header).
pub fn piece_in(state: &RelayState, id: i64, part: i64, q: &PieceQuery, caller: &str, key: &str, body: &[u8], folders: Folders, now: i64) -> Response {
    let row = match restore_item(state, id, &q.grant, caller, now) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let plan = match open_plan(&row, folders, key) {
        Ok(p) => p,
        Err(why) => return refuse(StatusCode::FORBIDDEN, why),
    };
    let Some((part, (size, hash))) = usize::try_from(part).ok().and_then(|p| expected(&plan, p).map(|e| (p, e))) else {
        return refuse(StatusCode::NOT_FOUND, "This held item holds no file in that part.");
    };
    match take_piece(&row, folders, part, size, &hash, q.offset, body) {
        Ok(Piece::More(n)) => Json(json!({ "offset": n, "size": size })).into_response(),
        Ok(Piece::Done) => Json(json!({ "done": true, "size": size })).into_response(),
        Err(PieceRefused::WrongOffset(have)) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": format!("This piece starts at byte {}, but the server has {have} bytes of this file. Send from there.", q.offset), "offset": have })),
        )
            .into_response(),
        Err(PieceRefused::TooLarge) => refuse(StatusCode::PAYLOAD_TOO_LARGE, "This piece is larger than a piece may be, or goes past the end of the file that was held."),
        Err(PieceRefused::Mismatch) => refuse(StatusCode::BAD_REQUEST, "This file is not exactly the one that was held, so it was not taken. Nothing was put back."),
        Err(PieceRefused::Disk(e)) => {
            tracing::error!("Held items: a restore's piece of #{id} could not be written: {e}");
            refuse(StatusCode::INTERNAL_SERVER_ERROR, "The server could not write this piece. Nothing was put back.")
        }
    }
}

/// `PUT /api/held/{id}/{part}?grant=&offset=`, signed by the owner (held.rs `signed_caller`): one
/// piece of a file the owner's device opened.
pub async fn api_restore_piece(
    State(state): State<Arc<RelayState>>,
    Path((id, part)): Path<(i64, i64)>,
    Query(q): Query<PieceQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(caller) = signed_caller(&state, &headers, &caller_purpose("restore", &q.grant, id, part, q.offset)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let key = key_of(&headers);
    let owned = OwnedFolders::of(Folders::live());
    tokio::task::spawn_blocking(move || piece_in(&state, id, part, &q, &caller, &key, &body, owned.get(), now_ms()))
        .await
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Put held item `id` back (the module's header): the words for the owner, or the refusal.
pub fn finish_in(state: &RelayState, id: i64, grant: &str, caller: &str, key: &str, folders: Folders, now: i64) -> Response {
    let row = match restore_item(state, id, grant, caller, now) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let plan = match open_plan(&row, folders, key) {
        Ok(p) => p,
        Err(why) => return refuse(StatusCode::FORBIDDEN, why),
    };
    let owner = row.sealed_to.clone();
    let done = match state.db.held_restore_in(&row, &plan, folders, now) {
        Ok(d) => d,
        Err(why) => return refuse(StatusCode::CONFLICT, why),
    };
    // Every client that may read the room gets the post again (the broadcast loop filters by room).
    let _ = state.broadcast_tx.send(RelayMessage::Chat {
        from: done.author.clone(),
        from_name: done.from_name.clone(),
        content: done.words.clone(),
        timestamp: done.stamp,
        signature: None,
        channel: room(&done.channel).to_string(),
        reply_to: None,
        thread_count: None,
        message_id: Some(done.message_id),
    });
    let words = format!(
        "Held item #{id} is restored: the post and its {} file(s) are back where they were, on this server only, and the held copy is deleted. If its author was banned, lift the ban with Unban.",
        done.files
    );
    tell(state, &owner, words.clone());
    handle_list(state, &owner);
    Json(json!({ "ok": true, "words": words })).into_response()
}

/// `POST /api/held/{id}/restore?grant=`, signed by the owner: put the post back once every file
/// arrived.
pub async fn api_restore_finish(State(state): State<Arc<RelayState>>, Path(id): Path<i64>, Query(q): Query<GrantQuery>, headers: HeaderMap) -> Response {
    let Some(caller) = signed_caller(&state, &headers, &caller_purpose("finish", &q.grant, id, 0, 0)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let key = key_of(&headers);
    let owned = OwnedFolders::of(Folders::live());
    tokio::task::spawn_blocking(move || finish_in(&state, id, &q.grant, &caller, &key, owned.get(), now_ms()))
        .await
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
#[path = "held_restore_tests.rs"]
mod tests;

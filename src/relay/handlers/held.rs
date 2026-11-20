//! Hide and keep, the decision (2026-11-20, docs/design/hide-and-keep.md). A report's
//! `hide_and_keep` decision (handlers/reports.rs `handle_decide`) lands here; the storage half is
//! storage/held.rs.
//!
//! WHO MAY. An admin or a moderator, by the same moderation rules as every other decision
//! (msg_handlers.rs `mod_action_refusal`: only an admin acts on an admin). Refused, with nothing
//! changed, when the server cannot seal a copy to its owner ([`crate::relay::storage::held::OwnerCard`]):
//! the admin is told why in words and what works instead ("Delete the post", which keeps nothing).
//!
//! WHAT IT DOES, for each of the reported person's posts the report names: the post leaves every
//! view (storage/held.rs `hide_post_in`), every client is told to drop it (the `Delete` broadcast),
//! a pinned copy's room has its pins sent again, and the delete goes to linked servers and the
//! words leave the federation outbox (federation_log.rs `after_delete`).
//!
//! SIGNED (rung 4). Every action here carries the actor's own signed `mod_action_v1` under
//! `action` (the JSON `POST /api/v2/objects` takes), checked by storage/moderation_held.rs and
//! recorded once carried out; a frame without one is refused ([`UNSIGNED`]). The owner's `held_act`
//! frame carries a legal hold, destroy now, export or restore; an export or a restore grants the
//! owner's device [`EXPORT_GRANT_MS`] to fetch the sealed parts from `GET /api/held/{id}/{part}?
//! grant=<object id>` (ciphertext only that device opens), and a restore to send each file back
//! (`PUT` of that address, then `POST /api/held/{id}/restore`, held_restore.rs: every byte checked
//! against the restore plan sealed when the post was held). `held_list` shows admins the held
//! items (no contents, no view), the log and the counts; `GET /api/held-count` shows anyone the
//! counts alone.

use std::sync::Arc;

use crate::relay::handlers::federation_log;
use crate::relay::handlers::msg_handlers::mod_action_refusal;
use crate::relay::relay::{PinData, RelayMessage, RelayState};
use crate::relay::storage::files::Folders;
use crate::relay::storage::held::{legal_hold_refusal, Keeper};
use crate::relay::storage::moderation::held::HeldAction;
use crate::relay::api_v2_objects::SignedObjectSubmission;
use humanity_protocol::object::Object;
use serde_json::Value;
use crate::relay::storage::reports::ReportRow;

/// The main room's name when a post's room is stored empty.
fn room(channel: &str) -> &str {
    if channel.is_empty() {
        humanity_protocol::chat_access::MAIN_CHANNEL
    } else {
        channel
    }
}

/// Send the pins of `channel` again, to everyone who may read it (chat_access.rs filters by room).
fn resend_pins(state: &RelayState, channel: &str) {
    let pins = state.db.get_pinned_messages(channel).unwrap_or_default();
    let pins = pins
        .into_iter()
        .map(|p| PinData {
            from_key: p.from_key,
            from_name: p.from_name,
            content: p.content,
            original_timestamp: p.original_timestamp,
            pinned_by: p.pinned_by,
            pinned_at: p.pinned_at,
        })
        .collect();
    let _ = state.broadcast_tx.send(RelayMessage::PinsSync { channel: channel.to_string(), pins });
}

/// Hide and keep the posts of `report` (by `me`, an admin or moderator), with the uploads in
/// `folders`: the words for `me` on success, or why nothing was done. `posts` are the report's
/// posts by the reported person (reports.rs `posts_of`).
pub async fn hide_and_keep_in(state: &Arc<RelayState>, me: &str, report: &ReportRow, posts: Vec<(String, u64)>, raw: &Value, folders: Folders<'_>) -> Result<String, String> {
    let target = report.target_key.as_str();
    if let Some(why) = mod_action_refusal(state, me, "hide_and_keep", target, "") {
        return Err(why);
    }
    let (signed, json) = signed_action(state, me, raw, "hide_and_keep", report.id)?;
    if posts.is_empty() {
        return Err("it has no post of theirs to hide.".to_string());
    }
    let card = state.db.owner_card();
    if let Some(why) = card.refusal() {
        return Err(why.to_string());
    }
    let crate::relay::storage::held::OwnerCard::Ready { owner, kyber, card_at } = &card else {
        return Err(crate::relay::storage::held::NO_CARD.to_string());
    };
    let keeper = Keeper { owner, kyber, card_at: *card_at, held_by: me, now_ms: crate::relay::storage::now_millis() as i64 };
    let (mut hidden, mut files, mut left) = (0usize, 0usize, Vec::new());
    for (author, stamp) in posts {
        match state.db.hide_post_in(report.id, &author, stamp, &keeper, folders) {
            Ok(Some(h)) => {
                hidden += 1;
                files += h.files;
                left.extend(h.left_in_view);
                let _ = state.broadcast_tx.send(RelayMessage::Delete { from: author.clone(), timestamp: stamp });
                if h.unpinned {
                    resend_pins(state, room(&h.channel));
                }
                federation_log::after_delete(state, &author, stamp, Some(room(&h.channel).to_string())).await;
            }
            Ok(None) => {} // already gone: its author deleted it, or it expired
            Err(e) => {
                tracing::error!("Hide and keep: report #{}: a post could not be hidden: {e}", report.id);
                return Err(format!("a post could not be hidden: {e}. Posts hidden before it stay hidden."));
            }
        }
    }
    if hidden == 0 {
        return Err("its posts are no longer on this server (deleted by their author, or expired), so there was nothing to hide.".to_string());
    }
    record(state, me, &signed, &json);
    let mut words = format!(
        "{hidden} post(s) and {files} file(s) taken out of view and kept sealed to the owner's key card. They are deleted in {} days unless the owner places a legal hold.",
        crate::relay::storage::held::HELD_KEEP_DAYS
    );
    if !left.is_empty() {
        words.push_str(&format!(" {} file(s) could not be taken out of the uploads folder; the server's log names them.", left.len()));
    }
    Ok(words)
}

// ── The signed actions (rung 4) and the held items list ──

/// How long after a recorded Export its parts may be fetched.
pub const EXPORT_GRANT_MS: i64 = 10 * 60 * 1000;

/// What a refused frame without a signed action is told.
pub const UNSIGNED: &str = "This action must be signed by your own key, and this app did not sign it. Update the app and try again. Nothing was done.";

fn now_ms() -> i64 {
    crate::relay::storage::now_millis() as i64
}

/// The signed action a frame carries under `action` (the JSON `POST /api/v2/objects` takes),
/// parsed, with its JSON text to record.
fn action_of(raw: &Value) -> Result<(Object, String), String> {
    let Some(v) = raw.get("action").filter(|v| v.is_object()) else { return Err(UNSIGNED.to_string()) };
    let sub: SignedObjectSubmission = serde_json::from_value(v.clone()).map_err(|_| UNSIGNED.to_string())?;
    let object = sub.to_object().map_err(|_| UNSIGNED.to_string())?;
    Ok((object, v.to_string()))
}

/// Check the signed action `raw` carries for `me`: it must be `want` naming `target`.
pub(crate) fn signed_action(state: &RelayState, me: &str, raw: &Value, want: &str, target: i64) -> Result<(HeldAction, String), String> {
    let (object, json) = action_of(raw)?;
    let a = state.db.check_held_action(&object, me, now_ms())?;
    if a.action != want || a.target != target {
        return Err(format!("the signed action is {} of #{}, not {want} of #{target}", a.action, a.target));
    }
    Ok((a, json))
}

/// Record a carried-out action; a failure is logged and said, never undoes what was done.
fn record(state: &RelayState, me: &str, a: &HeldAction, json: &str) {
    if let Err(e) = state.db.record_held_action(a, json, now_ms()) {
        tracing::error!("Held items: {} of #{} was carried out but could not be recorded: {e}", a.action, a.target);
        tell(state, me, format!("The {} was carried out, but the server could not record it in the held items log.", a.action));
    }
}

fn tell(state: &RelayState, who: &str, message: String) {
    let _ = state.broadcast_tx.send(RelayMessage::Private { to: who.to_string(), message });
}

fn is_admin(state: &RelayState, key: &str) -> bool {
    matches!(state.db.get_role(key).unwrap_or_default().as_str(), "admin" | "owner")
}

fn name_or_key(state: &RelayState, key: &str) -> String {
    match state.db.name_for_key(key).ok().flatten() {
        Some(n) if !n.is_empty() => n,
        _ => key.chars().take(8).collect(),
    }
}

/// `held_list` from `me`, an admin or the owner: the held items (how many, each one's times, size
/// and legal hold; nothing of what it holds), the signed actions log, the counts, and whether
/// this server can keep a copy at all (and if not, why, in words).
pub fn handle_list(state: &RelayState, me: &str) {
    if !is_admin(state, me) {
        return tell(state, me, "Only admins can see held items.".to_string());
    }
    let card = state.db.owner_card();
    let owner = match &card {
        crate::relay::storage::held::OwnerCard::Ready { owner, .. } => owner.clone(),
        _ => String::new(),
    };
    let items = state
        .db
        .held_items()
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let mut item = serde_json::json!({
                "id": r.id, "report_id": r.report_id, "file_count": r.file_count, "parts": r.parts,
                "sealed_bytes": r.sealed_bytes, "held_at": r.held_at, "held_by_name": name_or_key(state, &r.held_by),
                "delete_after": r.delete_after, "legal_hold_until": r.legal_hold_until, "deleted_at": r.deleted_at(),
                "exported_count": r.exported_count, "sealed_to_owner": !owner.is_empty() && r.sealed_to == owner,
            });
            // The package id goes to the owner whose key it is sealed to, and to nobody else: their
            // device needs it to open the parts (each part is bound to it). Never drawn.
            if !owner.is_empty() && owner == me && r.sealed_to == me {
                item["package"] = serde_json::Value::String(r.package.clone());
            }
            item
        })
        .collect();
    let actions = state
        .db
        .held_actions(200)
        .unwrap_or_default()
        .into_iter()
        .map(|a| {
            serde_json::json!({
                "action": a.action, "target": a.target, "signer": a.signer_key, "signer_name": name_or_key(state, &a.signer_key),
                "reason": a.reason, "until": a.until_ms, "signed_at": a.signed_at, "object": a.object,
            })
        })
        .collect();
    let counts = serde_json::to_value(state.db.held_counts().unwrap_or_default()).unwrap_or_default();
    let _ = state.broadcast_tx.send(RelayMessage::HeldItems {
        to: me.to_string(),
        can_keep: card.refusal().is_none(),
        cannot_keep_why: card.refusal().unwrap_or("").to_string(),
        i_am_owner: !owner.is_empty() && owner == me,
        items,
        actions,
        counts,
    });
}

/// `held_act {action}` from `me`: the owner's legal hold, destroy now, export and restore, each a
/// signed action (the module's header); `hide_and_keep` is decided on its report.
pub async fn handle_act_in(state: &Arc<RelayState>, me: &str, raw: &Value, folders: Folders<'_>) {
    let (object, json) = match action_of(raw) {
        Ok(x) => x,
        Err(why) => return tell(state, me, why),
    };
    let a = match state.db.check_held_action(&object, me, now_ms()) {
        Ok(a) => a,
        Err(why) => return tell(state, me, format!("Nothing was done: {why}.")),
    };
    let Ok(Some(item)) = state.db.held_item(a.target) else {
        return tell(state, me, format!("There is no held item #{}.", a.target));
    };
    let done = match a.action.as_str() {
        "held_legal_hold" => {
            if let Some(why) = legal_hold_refusal(a.until, now_ms()) {
                return tell(state, me, format!("Nothing was done: {why}"));
            }
            match state.db.held_set_legal_hold(item.id, a.until) {
                Ok(true) if a.until == 0 => format!("The legal hold on held item #{} is lifted.", item.id),
                Ok(true) => format!("Held item #{} is under a legal hold.", item.id),
                Ok(false) => return tell(state, me, format!("There is no held item #{}.", item.id)),
                Err(e) => return tell(state, me, format!("Nothing was done: the server could not save the hold ({e}).")),
            }
        }
        "held_destroy" => match state.db.held_destroy_in(item.id, folders) {
            Ok(_) => format!("Held item #{} is destroyed: its sealed copy and its row are deleted.", item.id),
            Err(e) => return tell(state, me, format!("Held item #{} could not be destroyed ({e}); it is kept.", item.id)),
        },
        "held_export" => {
            if item.sealed_to != me {
                return tell(state, me, format!("Held item #{} is sealed to an earlier owner's key card, so only that key can open it. Nothing was done.", item.id));
            }
            if let Err(e) = state.db.held_count_export(item.id) {
                tracing::error!("Held items: could not count an export of #{}: {e}", item.id);
            }
            format!("Export of held item #{} granted for {} minutes.", item.id, EXPORT_GRANT_MS / 60_000)
        }
        // Restore (rung 5): the owner's device fetches the parts, opens them and sends each file
        // back within the grant (held_restore.rs); the relay checks every byte against the plan.
        "held_restore" => {
            if item.sealed_to != me {
                return tell(state, me, format!("Held item #{} is sealed to an earlier owner's key card, so only that key can open it. Nothing was done.", item.id));
            }
            format!("Restore of held item #{} granted for {} minutes: your device is putting it back.", item.id, EXPORT_GRANT_MS / 60_000)
        }
        _ => return tell(state, me, "A post is hidden and kept from its report. Nothing was done.".to_string()),
    };
    record(state, me, &a, &json);
    tell(state, me, done);
    handle_list(state, me);
}

/// The frames relay.rs hands here: `held_list` and `held_act`.
pub async fn handle(state: &Arc<RelayState>, me: &str, kind: &str, raw: &Value) {
    match kind {
        "held_list" => handle_list(state, me),
        "held_act" => handle_act_in(state, me, raw, Folders::live()).await,
        _ => {}
    }
}

/// Held item `id` under `grant` (the object id of the owner's recorded export or restore, at most
/// [`EXPORT_GRANT_MS`] old, by the key the item is sealed to, who is the owner now), with which of
/// the two the grant is; None for anything else.
pub(crate) fn granted_item(state: &RelayState, id: i64, grant: &str, now: i64) -> Option<(crate::relay::storage::held::HeldRow, String)> {
    let (signer, at, action) = state.db.held_grant(grant, id).ok().flatten()?;
    let item = state.db.held_item(id).ok().flatten()?;
    let fresh = now - at <= EXPORT_GRANT_MS;
    let owner_now = state.db.owner_keys().unwrap_or_default() == [signer.clone()];
    (fresh && owner_now && item.sealed_to == signer).then_some((item, action))
}

/// Where part `part` of held item `id` is, for the export or restore `grant`
/// ([`granted_item`]). Every refusal is the same 404, so a guess learns nothing.
pub fn export_part_path(state: &RelayState, id: i64, part: i64, grant: &str, folders: Folders, now: i64) -> Option<std::path::PathBuf> {
    let (item, _) = granted_item(state, id, grant, now)?;
    if part < 0 || part >= item.parts {
        return None;
    }
    Some(crate::relay::storage::held::held_dir_beside(folders).join(&item.package).join(crate::relay::storage::held::part_file(part as usize)))
}

#[derive(serde::Deserialize)]
pub struct GrantQuery {
    #[serde(default)]
    pub grant: String,
}

/// `GET /api/held/{id}/{part}?grant=<object id>`: one sealed part, ciphertext only the owner's
/// device opens ([`export_part_path`]); 404 for anything else.
pub async fn api_export_part(
    axum::extract::State(state): axum::extract::State<Arc<RelayState>>,
    axum::extract::Path((id, part)): axum::extract::Path<(i64, i64)>,
    axum::extract::Query(q): axum::extract::Query<GrantQuery>,
    req: axum::extract::Request,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match export_part_path(&state, id, part, &q.grant, Folders::live(), now_ms()) {
        Some(path) => match tower_http::services::ServeFile::new(path).try_call(req).await {
            Ok(r) => r.into_response(),
            Err(_) => axum::http::StatusCode::NOT_FOUND.into_response(),
        },
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /api/held-count`: how many held actions this server carried out, ever, with no details
/// (guarantee 5's public transparency line).
pub async fn api_count(axum::extract::State(state): axum::extract::State<Arc<RelayState>>) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::to_value(state.db.held_counts().unwrap_or_default()).unwrap_or_default())
}

// Restore (rung 5): the files sent back and the post put back.
#[path = "held_restore.rs"]
mod restore;
pub use restore::{api_restore_finish, api_restore_piece};

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;

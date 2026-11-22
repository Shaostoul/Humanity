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
//! ONE TASK, WHATEVER HAPPENS TO THE MODERATOR'S CONNECTION (the sixth review's M1, 2026-11-21):
//! reports.rs runs the whole hide (the move, the seal, the record, the broadcast, the federation
//! and the report's decision) as a task of its own that owns the sealing count, so a dropped socket
//! no longer cancels the await of a seal already running and loses everything after it, nor stops
//! the hide being counted as sealing while it still seals. The seal reads its recipients under the
//! held folder's lock at sealing time (storage/held.rs `hide_post_in`).
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
use crate::relay::storage::files::{Folders, OwnedFolders};
use crate::relay::storage::held::legal_hold_refusal;
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
    // Sealing now, or waiting (the owner branch's third review, S3): while an owner change is
    // pending or being carried out, the post leaves every view at once as always, but its sealed copy
    // WAITS and is made right after the change is carried out or rolled back, to whoever the
    // recipients are then (owner.rs `seal_waiting`). Otherwise it is sealed now, counted as sealing
    // so no change begins meanwhile, and the owner's card is read inside that count.
    let sealing = crate::relay::handlers::owner::SealingNow::start(state);
    let card = state.db.owner_card();
    if let Some(why) = card.refusal() {
        return Err(why.to_string());
    }
    if !matches!(card, crate::relay::storage::held::OwnerCard::Ready { .. }) {
        return Err(crate::relay::storage::held::NO_CARD.to_string());
    }
    let now = crate::relay::storage::now_millis() as i64;
    let waits = sealing.is_none();
    let (mut hidden, mut files, mut left) = (0usize, 0usize, Vec::new());
    for (author, stamp) in posts {
        // Hiding a post seals its files (whole files read and written): on a blocking thread, never
        // on the async runtime (the fifth review's L4). The card is read again inside, under the
        // held folder's lock (the sixth review's M1); the one read above only says why not, early.
        let (st, by, author2, owned, report_id) = (state.clone(), me.to_string(), author.clone(), OwnedFolders::of(folders), report.id);
        let done = tokio::task::spawn_blocking(move || {
            let folders = owned.get();
            if waits {
                return st.db.hide_post_waiting_in(report_id, &author2, stamp, &by, now, folders);
            }
            st.db.hide_post_in(report_id, &author2, stamp, &by, now, folders)
        })
        .await
        .unwrap_or_else(|e| Err(crate::relay::storage::held::HideError::Seal(format!("the server stopped while sealing it ({e})"))));
        match done {
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
    drop(sealing);
    if hidden == 0 {
        return Err("its posts are no longer on this server (deleted by their author, or expired), so there was nothing to hide.".to_string());
    }
    record(state, me, &signed, &json);
    let mut words = if waits {
        // Said on the moderator's screen (S3): the copy is not sealed yet, and why.
        format!(
            "{hidden} post(s) and {files} file(s) taken out of view. Their sealed copy WAITS for the owner change in progress: it is made as soon as that change is carried out or rolled back, sealed to whoever then holds the role, and kept {} days from now unless the owner places a legal hold. Until then it waits on the server unsealed, out of every view.",
            crate::relay::storage::held::HELD_KEEP_DAYS
        )
    } else {
        format!(
            "{hidden} post(s) and {files} file(s) taken out of view and kept sealed to the owner's key card. They are deleted in {} days unless the owner places a legal hold.",
            crate::relay::storage::held::HELD_KEEP_DAYS
        )
    };
    // The change may have ended while this hide waited: its copy is sealed now, not at the next one
    // (on a blocking thread: the fifth review's L4).
    if waits {
        let (st, owned) = (state.clone(), OwnedFolders::of(folders));
        if let Err(e) = tokio::task::spawn_blocking(move || crate::relay::handlers::owner::seal_waiting(&st, owned.get())).await {
            tracing::error!("Hide and keep: sealing the copies that waited stopped ({e}); the owner's minute seals them.");
        }
    }
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
pub(crate) fn action_of(raw: &Value) -> Result<(Object, String), String> {
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
/// The decision's note for a hide completed at a start, when its signed record landed before the stop.
pub const UNFINISHED_HIDE_NOTE: &str = "Completed when the server started again: it had stopped part-way through this hide and keep. The copy is held and the post out of view, as decided.";
/// The same, when the moderator's signed record of it was lost with the stop.
pub const UNFINISHED_HIDE_NOTE_NO_RECORD: &str = "Completed when the server started again: it had stopped part-way through this hide and keep. The copy is held and the post out of view, as decided, but the moderator's signed record of it was lost with the stop and cannot be signed again.";

/// At start (the owner branch's seventh review, L2): every hide and keep a stop cut short, its copy
/// sealed and its post out of view but its report still open, is completed: the report's decision
/// is recorded under the moderator who held it, with words saying it was completed at this start
/// (and that the signed record was lost, when it was: it cannot be signed again), the log says so,
/// and the linked servers' delete is queued again for each of its posts. Graceful shutdown waits for
/// hides in flight (owner.rs `wait_for_hides`), so this is for a crash or a kill. Returns how many
/// reports it completed.
pub async fn complete_unfinished_hides(state: &Arc<RelayState>) -> usize {
    let rows = match state.db.held_unfinished_hides() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Hide and keep: could not look for hides a stop cut short: {e}");
            return 0;
        }
    };
    let mut completed = 0;
    for h in rows {
        let note = if h.recorded { UNFINISHED_HIDE_NOTE } else { UNFINISHED_HIDE_NOTE_NO_RECORD };
        match state.db.report_record_decision(h.report_id, &h.held_by, "hide_and_keep", note, now_ms()) {
            Ok(true) => {
                completed += 1;
                tracing::warn!("Hide and keep: report #{}: {note}", h.report_id);
            }
            Ok(false) => {}
            Err(e) => tracing::error!("Hide and keep: report #{}: its decision could not be recorded at start: {e}", h.report_id),
        }
        federation_log::after_delete(state, &h.author, h.stamp, Some(room(&h.channel).to_string())).await;
    }
    completed
}

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
    crate::relay::handlers::msg_handlers::is_admin_role(&state.db.get_role(key).unwrap_or_default())
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
    handle_list_in(state, me, Folders::live());
}

/// [`handle_list`] with the uploads (and the held folder beside them) in `folders`.
pub fn handle_list_in(state: &RelayState, me: &str, folders: Folders) {
    if !is_admin(state, me) {
        return tell(state, me, "Only admins can see held items.".to_string());
    }
    let card = state.db.owner_card();
    let owner = match &card {
        crate::relay::storage::held::OwnerCard::Ready { owner, .. } => owner.clone(),
        _ => String::new(),
    };
    // The successor the owner named (every admin sees who it is), and the owner log: each claim,
    // handover and change of successor (handlers/owner.rs).
    let successor = state.db.successor().unwrap_or_default();
    let successor_json = if successor.is_empty() { Value::Null } else { serde_json::json!({ "key": successor, "name": name_or_key(state, &successor) }) };
    let owner_log: Vec<Value> = state
        .db
        .owner_log(50)
        .unwrap_or_default()
        .into_iter()
        .map(|l| serde_json::json!({ "kind": l.kind, "old_name": if l.old_key.is_empty() { String::new() } else { name_or_key(state, &l.old_key) }, "new_name": name_or_key(state, &l.new_key), "reason": l.reason, "at": l.at, "object": l.object }))
        .collect();
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
                "exported_count": r.exported_count, "sealed_to_owner": !owner.is_empty() && r.opens_for(&owner),
                "sealed_to_successor": !successor.is_empty() && r.also_sealed_to == successor,
                // The post's author and time, shown, so the owner tells apart the posts of one report
                // and the app checks that what an Export or a Restore opens is that very post (the
                // fourth review's finding 8). The report already showed the admins both.
                "author": r.author_key, "author_name": name_or_key(state, &r.author_key), "stamp": r.stamp,
            });
            // The package id goes to the owner whose key it is sealed to, and to nobody else: their
            // device needs it to open the parts (each part is bound to it). Never drawn.
            if !owner.is_empty() && owner == me && r.opens_for(me) {
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
            // An export's or a restore's signed object names its grant (its object id), so only
            // the owner whose grant it is gets it; every other admin sees who, what, when and why.
            // (The part routes also check the caller's own signature, so a grant alone fetches
            // nothing: the owner review's finding 1, 2026-11-20.)
            let grant_bearing = matches!(a.action.as_str(), "held_export" | "held_restore");
            let object = if grant_bearing && !(owner == me && a.signer_key == me) { Value::Null } else { Value::String(a.object.clone()) };
            serde_json::json!({
                "action": a.action, "target": a.target, "signer": a.signer_key, "signer_name": name_or_key(state, &a.signer_key),
                "reason": a.reason, "until": a.until_ms, "signed_at": a.signed_at, "object": object,
            })
        })
        .collect();
    // The hides waiting for their sealed copy, and any waiting folder a stop left with no note and no
    // post to name it, told to every admin in words (the fourth review's finding 5a: never silent).
    let mut counted = state.db.held_counts().unwrap_or_default();
    let (waiting, unidentified) = state.db.held_waiting_counts_in(folders);
    (counted.waiting, counted.unidentified) = (waiting as i64, unidentified as i64);
    let counts = serde_json::to_value(counted).unwrap_or_default();
    let _ = state.broadcast_tx.send(RelayMessage::HeldItems {
        to: me.to_string(),
        can_keep: card.refusal().is_none(),
        cannot_keep_why: card.refusal().unwrap_or("").to_string(),
        i_am_owner: !owner.is_empty() && owner == me,
        items,
        actions,
        counts,
        successor: successor_json,
        owner_log,
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
        // On a blocking thread (the owner branch's seventh review, L1): the destroy takes the held
        // folder's one lock, which a hide holds across its whole seal, so taken here on the async
        // runtime it parked a worker for as long as the longest seal.
        "held_destroy" => {
            let (st, owned, id) = (state.clone(), OwnedFolders::of(folders), item.id);
            let destroyed = tokio::task::spawn_blocking(move || st.db.held_destroy_in(id, owned.get()).map_err(|e| e.to_string()))
                .await
                .unwrap_or_else(|e| Err(format!("the server stopped while destroying it ({e})")));
            match destroyed {
                Ok(_) => format!("Held item #{} is destroyed: its sealed copy and its row are deleted.", item.id),
                Err(e) => return tell(state, me, format!("Held item #{} could not be destroyed ({e}); it is kept.", item.id)),
            }
        }
        "held_export" => {
            if !item.opens_for(me) {
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
            if !item.opens_for(me) {
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

// ── Who may fetch or send a part (the owner review's finding 1, 2026-11-20) ──
//
// A grant (a signed action's object id) is in the held items log, and a part is sealed to the
// owner AND the successor, so a grant alone must never hand out a part: an admin who is the
// successor, or anyone an admin passed the grant to, would open it unlogged. Every part route
// therefore proves its CALLER's key with the project's signed-request pattern (files.rs
// `check_signed`: Dilithium3 over `purpose\ntimestamp`, five minutes fresh, used once; the purpose
// starts with core held_seal's CALLER_DOMAIN line), and the caller must be the key the grant
// belongs to: the owner who signed the export or restore. An owner change has routes of its own,
// which serve headers alone (handlers/owner.rs, owner_accept.rs).
// NOT YET CLOSED, said plainly (the owner branch's third review, S6, 2026-11-21): the domain line alone
// does not keep a signed chat line out today, because a chat message's content may hold a
// newline, so a chat line whose content is this very purpose is signed over the same
// `purpose\ntime` and broadcast to everyone, who could replay it within the five minutes. What
// closes it is the relay refusing every chat line that begins with a `hum/` domain line, which
// arrives with the signed-requests branch (src/relay/signed_request.rs, round 2); once both
// branches are merged, the held caller moves onto that request scheme. NOTE to whoever merges the
// second of the two: make that move then (it is not done here).

// The headers and the purpose are the protocol's, shared with the desktop app (core held_seal).
pub use humanity_protocol::held_seal::{caller_purpose, CALLER_KEY, CALLER_SIG, CALLER_TIME};

/// The key that signed this request for `purpose` ([`CALLER_KEY`] and the rest), or None for a
/// request with no signature, a bad one, a stale one or a replay.
pub async fn signed_caller(state: &RelayState, headers: &axum::http::HeaderMap, purpose: &str) -> Option<String> {
    let h = |n: &str| headers.get(n).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let (key, sig) = (h(CALLER_KEY), h(CALLER_SIG));
    let ts: u64 = h(CALLER_TIME).parse().ok()?;
    if key.is_empty() || sig.is_empty() {
        return None;
    }
    crate::relay::files::check_signed(state, &key, ts, &sig, purpose).await.ok()?;
    Some(key)
}

/// Held item `id` under `grant` for `caller` (the key a signed request proved): the object id of
/// the owner's recorded export or restore, at most [`EXPORT_GRANT_MS`] old, signed by the key the
/// item is sealed to, who is the owner now and is the caller. With which it is; None for anything
/// else, a grant in other hands above all. An owner CHANGE's grant is not one: a change is served
/// headers alone (handlers/owner.rs), never a body, so beginning a change, fetching under its grant
/// and cancelling reads nothing (the second review, R3: it served every original part, unlogged).
pub(crate) fn granted_item(state: &RelayState, id: i64, grant: &str, caller: &str, now: i64) -> Option<(crate::relay::storage::held::HeldRow, String)> {
    if caller.is_empty() {
        return None;
    }
    let (signer, at, action) = state.db.held_grant(grant, id).ok().flatten()?;
    if signer != caller {
        return None;
    }
    let item = state.db.held_item(id).ok().flatten()?;
    let fresh = now - at <= EXPORT_GRANT_MS;
    let owner_now = state.db.owner_keys().unwrap_or_default() == [signer.clone()];
    (fresh && owner_now && item.opens_for(&signer)).then_some((item, action))
}

/// Where part `part` of held item `id` is, for the export or restore `grant` and its `caller`
/// ([`granted_item`]). Every refusal is the same 404, so a guess learns nothing.
pub fn export_part_path(state: &RelayState, id: i64, part: i64, grant: &str, caller: &str, folders: Folders, now: i64) -> Option<std::path::PathBuf> {
    let (item, _) = granted_item(state, id, grant, caller, now)?;
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

/// `GET /api/held/{id}/{part}?grant=<object id>`, signed by the owner ([`signed_caller`]): one
/// sealed part, ciphertext only the owner's device opens ([`export_part_path`]); 404 for anything
/// else.
pub async fn api_export_part(
    axum::extract::State(state): axum::extract::State<Arc<RelayState>>,
    axum::extract::Path((id, part)): axum::extract::Path<(i64, i64)>,
    axum::extract::Query(q): axum::extract::Query<GrantQuery>,
    req: axum::extract::Request,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let Some(caller) = signed_caller(&state, req.headers(), &caller_purpose("part", &q.grant, id, part, 0)).await else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };
    match export_part_path(&state, id, part, &q.grant, &caller, Folders::live(), now_ms()) {
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
pub(crate) mod tests;

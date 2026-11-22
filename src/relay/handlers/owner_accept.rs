//! An owner change's second phase: each recipient checks the new headers on their own device and
//! accepts (2026-11-20, the owner review's finding 5, re-done after the second review; handlers/
//! owner.rs has the whole flow). A child of handlers/owner.rs (#[path]).
//!
//! WHY. The relay cannot open a slot, so it cannot tell whether a new header opens for the person it
//! is meant for: a sealing bug, or an owner who wanted the copies gone, could hand it a header no
//! recipient can open, and carrying that out destroys every copy while the log says the change went
//! through. So the originals and the role stay exactly as they were until every RECIPIENT of the new
//! seal has opened their own slot of every new header and signed an acceptance of what they checked
//! ([`accept_in`]); a slot that does not open for them is a decline ([`decline`]), and a change
//! nobody accepts in time rolls back (owner.rs `expire_pending`).
//!
//! WHAT IS CHECKED (format v4, core `held_seal::check_rewrapped_package`). The recipient's device
//! fetches each new header alone ([`api_rewrapped_header`]), checks it names as many people as the
//! owner's signed action implies, opens ONLY its own slot (its place in that order), and checks the
//! key it finds against the header's key commitment. The relay already refused any header whose
//! commitment is not the body's, so a slot that checks out opens the very body that was held. No
//! body is served, no restore key is sent, and nobody but the owner opens content during a change
//! (the second review, R2 and W3: the re-seal's check handed a successor-to-be every copy). The
//! device signs `owner_change_accept` naming the change (its object id) with the reason `checked
//! <digest>` (core `held_seal::acceptance_digest` over every new header's hash and the change's
//! ROUND), and the relay works out the same digest from the headers it staged
//! (storage/owner_handover.rs `expected_acceptance`). An acceptance of another round, or of other
//! headers, does not match (the second review, R1 and R4).
//!
//! THE RESIDUAL, SAID PLAINLY. When the owner is the only person who checks a change (a successor
//! removal), a modified owner's app could make the owner's own new slot unopenable and accept
//! anyway, which is the same as destroying the copies, and the owner can destroy them openly. The
//! bodies themselves survive byte for byte. The owner log marks such a change "checked by the owner
//! alone".

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use super::{covered_rows, now_ms, reply, reply_kind, roll_back_if, slot, HeaderQuery, PendingHandover, ACCEPT_WINDOW_MS};
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::files::Folders;
use crate::relay::storage::held::HeldRow;
use crate::relay::storage::owner::handover::{expected_acceptance, new_header, ChangeKind, OwnerChange};

fn name(state: &RelayState, key: &str) -> String {
    state.db.name_for_key(key).ok().flatten().filter(|n| !n.is_empty()).unwrap_or_else(|| key.chars().take(8).collect())
}

/// The `owner_change_check` frame for `to`, a recipient of `p` who has not accepted: the owner's
/// signed action (the device checks it was signed by the owner and names what the frame says), the
/// round, and every held item.
fn check_frame(state: &RelayState, p: &PendingHandover, rows: &[HeldRow], to: &str) -> RelayMessage {
    let c = &p.change;
    RelayMessage::OwnerChangeCheck {
        to: to.to_string(),
        grant: c.object_id.clone(),
        round: p.round.clone(),
        action: c.kind.action().to_string(),
        object: serde_json::from_str(&p.object_json).unwrap_or(Value::Null),
        owner: c.owner.clone(),
        owner_name: name(state, &c.owner),
        target: c.target.clone(),
        target_name: name(state, &c.target),
        items: rows.iter().map(|r| json!({ "id": r.id, "package": r.package, "parts": r.parts })).collect(),
        until: p.checked_at.unwrap_or(0) + ACCEPT_WINDOW_MS,
    }
}

/// Phase 2 begins: ask every recipient of `p` to check the new headers of `rows` on their device,
/// and tell the owner whom it waits for.
pub(crate) fn ask_recipients(state: &RelayState, p: &PendingHandover, rows: &[HeldRow]) {
    for key in p.waiting_on() {
        let _ = state.broadcast_tx.send(check_frame(state, p, rows, &key));
    }
    // Said in the chat too, so a recipient who has no settings page open learns of it.
    let what = if p.change.kind == ChangeKind::Handover { "asks you to accept this server's owner role" } else { "named you this server's successor" };
    for key in p.waiting_on().into_iter().filter(|k| *k != p.change.owner) {
        let message = format!(
            "{} {what}. Nothing changes until your device checks that the held copies are sealed to you: in the desktop app, open Server Settings (Overview); on the website, answer the line in the chat. Your device opens no copy to check it. Within {} minutes, or it is rolled back.",
            name(state, &p.change.owner),
            ACCEPT_WINDOW_MS / 60_000
        );
        let _ = state.broadcast_tx.send(RelayMessage::Private { to: key, message });
    }
    let others: Vec<String> = p.waiting_on().into_iter().filter(|k| *k != p.change.owner).map(|k| name(state, &k)).collect();
    let words = if others.is_empty() {
        "Every copy is re-wrapped. Your device is now checking that each opens for you; nothing changes until it has.".to_string()
    } else {
        format!(
            "Every copy is re-wrapped. Nothing changes until {} checks on their own device that every copy is sealed to them and accepts, within {} minutes. They must be signed in; if they do not accept in time, it is rolled back.",
            others.join(" and "),
            ACCEPT_WINDOW_MS / 60_000
        )
    };
    reply(state, &p.change.owner, reply_kind(p.change.kind), true, words);
}

/// When `key` signs in: ask them again if a change waits on their check.
pub fn on_connect(state: &RelayState, key: &str) {
    let Some(p) = slot(state).pending.clone() else { return };
    if p.checked_at.is_none() || !p.live(now_ms()) || !p.waiting_on().iter().any(|k| k == key) {
        return;
    }
    if let Ok(rows) = covered_rows(state, &p) {
        let _ = state.broadcast_tx.send(check_frame(state, &p, &rows, key));
    }
}

/// Where a test steps into [`accept_in`] between its checks and the carry-out (the second review's
/// R4 race test): `Checked` once the digest matched and before the acceptance is recorded,
/// `Carrying` once the change left its slot and before its folders move.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Checked,
    Carrying,
}

#[cfg(test)]
thread_local! {
    /// The test's hook, called at each [`Step`] on the thread running [`accept_in`].
    pub(crate) static HOOK: std::cell::RefCell<Option<Box<dyn FnMut(Step)>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn step(at: Step) {
    HOOK.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f(at);
        }
    });
}

/// `owner_change_accept {action}` from `me`: check the signed acceptance against the headers the
/// relay staged (the module's header). The change carried out when `me` was the last to accept;
/// None while others still must; why not, in words. A digest that does not match rolls the change
/// back: the headers cannot be what the recipient checked.
pub fn accept_in(state: &RelayState, me: &str, raw: &Value, folders: Folders, now: i64) -> Result<Option<OwnerChange>, String> {
    let (object, json) = crate::relay::handlers::held::action_of(raw)?;
    let Some(p) = slot(state).pending.clone().filter(|p| p.live(now) && !p.uploading()) else {
        return Err("No change is waiting for your check (it may have been cancelled or run out of time). Nothing was changed.".into());
    };
    if !p.waiting_on().iter().any(|k| k == me) {
        return Err("This change is not waiting for your check. Nothing was changed.".into());
    }
    let a = state.db.check_owner_accept(&object, me, now, &p.change, &json).map_err(|why| format!("Nothing was changed: {why}."))?;
    let round = p.round.clone();
    let rows = match covered_rows(state, &p) {
        Ok(r) => r,
        Err(why) => {
            roll_back_if(state, folders, |q| q.round == round, &format!("It was rolled back: {why}. Nothing was changed."), Some(&format!("The owner's change was rolled back: {why}. Nothing was changed.")));
            return Err(format!("Nothing was changed: {why}."));
        }
    };
    let want = expected_acceptance(&p.round, &rows, folders);
    if want.as_deref() != Ok(a.digest.as_str()) {
        let why = match want {
            Err(why) => why,
            Ok(_) => "what their device checked is not what the server holds for this change".into(),
        };
        let who = name(state, me);
        roll_back_if(state, folders, |q| q.round == round, &format!("{who}'s check of the re-wrapped copies failed ({why}), so the change was rolled back and every original kept. Nothing was changed."), None);
        return Err(format!("The copies did not check out ({why}), so the change was rolled back. Nothing was changed."));
    }
    #[cfg(test)]
    step(Step::Checked);
    // Recorded, under the lock, in the round it was checked for; the last acceptance takes the
    // change out of its slot and names its round as being carried out, so nothing rolls it back,
    // cancels it or begins another until it is done.
    let last = {
        let mut s = slot(state);
        let Some(q) = s.pending.as_mut().filter(|q| q.round == round) else {
            return Err("The change was cancelled or begun again while your acceptance was checked. Nothing was changed.".into());
        };
        q.accepted.push(a);
        if q.waiting_on().is_empty() {
            // Named as carried out under the same lock it leaves `pending` by (`CarryingNow`
            // clears it however the carry-out ends: the fifth review's L1).
            let done = s.pending.take();
            s.carrying = Some(round.clone());
            done
        } else {
            None
        }
    };
    let Some(done) = last else {
        reply(state, me, "accept", true, "Your device checked every copy and the server agrees. Waiting for the others it needs.".into());
        return Ok(None);
    };
    let carrying = super::CarryingNow::adopt(state);
    #[cfg(test)]
    step(Step::Carrying);
    let c = &done.change;
    // A panic inside the carry-out (the fifth review's L1) puts the held folder right at once, as
    // the next start would (finished if the change was recorded, undone if not), and is answered
    // like any other failure, instead of leaving folders swapped part-way until a restart.
    let carried = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.db.owner_change_in(c, &done.round, &rows, &done.ids, &done.object_json, &done.accepted, now, folders))).unwrap_or_else(|_| {
        tracing::error!("Owner change: carrying out {} by {} stopped with a panic; the held folder is put right now, as the next start would.", c.kind.action(), c.owner);
        state.db.owner_handover_recover_in(folders);
        if state.db.owner_handover_used(&c.object_id).unwrap_or(false) {
            Ok(())
        } else {
            Err("the server stopped part-way while carrying it out, and put every copy back as it was".to_string())
        }
    });
    if carried.is_err() {
        // The change's own new parts go (it never committed: owner_change_in put every folder
        // back), while it is still named as carried, so no new begin's are cleared.
        state.db.owner_handover_clear_staging_in(folders);
    }
    drop(carrying);
    // A hide that waited for this change is sealed now, to whoever the recipients are now (S3).
    super::seal_waiting(state, folders);
    if let Err(why) = carried {
        reply(state, &c.owner, reply_kind(c.kind), false, format!("Every check passed, but it failed: {why}."));
        for (key, _) in c.recipients.iter().filter(|(k, _)| *k != c.owner) {
            reply(state, key, "accept", false, format!("The owner's change failed: {why}."));
        }
        return Err(format!("It failed: {why}."));
    }
    tracing::info!(
        "Owner change: {} by {} naming {} at {now} ms ({} held item(s) re-wrapped, checked and accepted by {}{}).",
        c.kind.action(),
        c.owner,
        c.target,
        rows.len(),
        c.recipients.len(),
        if c.owner_alone() { ", the owner alone" } else { "" }
    );
    if me != c.owner {
        reply(state, me, "accept", true, "Your device checked every copy and accepted.".into());
    }
    Ok(Some(c.clone()))
}

/// `owner_change_decline {grant, why}` from a recipient whose device could not check the copies (or
/// who does not want the role): the change rolls back, and the owner is told why. Too late once the
/// change is being carried out: the decliner is told so.
pub fn decline(state: &RelayState, me: &str, raw: &Value, folders: Folders) {
    let grant = raw.get("grant").and_then(Value::as_str).unwrap_or("").to_string();
    let why: String = raw.get("why").and_then(Value::as_str).unwrap_or("").chars().take(300).collect();
    let who = name(state, me);
    let (owner, carrying) = {
        let s = slot(state);
        (s.pending.as_ref().map(|p| p.change.owner.clone()), s.carrying.is_some())
    };
    let words = match (owner.as_deref() == Some(me), why.trim().is_empty()) {
        (true, false) => format!("Your device's check of the re-wrapped copies failed ({why}), so the change was rolled back and every original kept. Nothing was changed."),
        (true, true) => "Your device's check of the re-wrapped copies failed, so the change was rolled back and every original kept. Nothing was changed.".to_string(),
        (false, false) => format!("{who} declined, or their device's check of the copies failed ({why}). It was rolled back and every original kept. Nothing was changed."),
        (false, true) => format!("{who} declined. It was rolled back and every original kept. Nothing was changed."),
    };
    let rolled = roll_back_if(state, folders, |p| p.change.object_id == grant && !p.uploading() && p.change.recipients.iter().any(|(k, _)| k == me), &words, None);
    if rolled && owner.as_deref() != Some(me) {
        reply(state, me, "accept", false, "Declined. Nothing was changed.".into());
    } else if !rolled && carrying {
        reply(state, me, "accept", false, "Too late to decline: every check had passed and the change was already being carried out.".into());
    }
}

/// The NEW header of part `part` of held item `id`, for `caller` (the key the request's signature
/// proved) checking the change `grant`: only a recipient who has not accepted yet, while the
/// recipients check. A header alone, never a body; every refusal is None (a 404).
pub fn rewrapped_header_in(state: &RelayState, id: i64, part: i64, grant: &str, caller: &str, folders: Folders, now: i64) -> Option<Vec<u8>> {
    let p = super::pending_for(state, grant, now).filter(|p| !p.uploading() && p.ids.contains(&id))?;
    if caller.is_empty() || !p.waiting_on().iter().any(|k| k == caller) {
        return None;
    }
    let row = state.db.held_item(id).ok().flatten()?;
    let part = usize::try_from(part).ok().filter(|p| (*p as i64) < row.parts)?;
    new_header(&row, folders, part).ok()
}

/// `GET /api/held/{id}/{part}/rewrapped?grant=`, signed by a recipient (held.rs `signed_caller`):
/// one new header for their device to check; 404 for anything else.
pub async fn api_rewrapped_header(State(state): State<Arc<RelayState>>, Path((id, part)): Path<(i64, i64)>, Query(q): Query<HeaderQuery>, headers: axum::http::HeaderMap) -> Response {
    use crate::relay::handlers::held::{caller_purpose, signed_caller};
    let Some(caller) = signed_caller(&state, &headers, &caller_purpose("rewrapped", &q.grant, id, part, 0)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match rewrapped_header_in(&state, id, part, &q.grant, &caller, Folders::live(), now_ms()) {
        Some(h) => ([(axum::http::header::CONTENT_TYPE, "application/octet-stream")], h).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

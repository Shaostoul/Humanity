//! A report about a group reaches the group's creator (section 10j of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10): the desktop app's half that touches the app
//! state, the socket and the server. The rules themselves (the marker, the dialog's choices, the
//! creator's checks) are src/net/group_report.rs; the removal that changes the group key first is
//! src/net/group_remove.rs; the dialog is src/gui/pages/chat/report_dialog.rs and the creator's
//! list src/gui/pages/safety_group_reports.rs. The web client built this first
//! (web/chat/chat-reports.js, chat-groups-p2p.js) and this file follows its order.
//!
//! THE REPORTER. A P2P group message's Report opens the dialog with the group, the message (named
//! by its signed-object id, which the group's rows now carry) and a lookup of who created the
//! group, read from the group's own signed `group_v1` record off the frame (`start_finding`).
//! Until it answers the dialog says "Finding out who created this group..." and Send waits. Send
//! builds everything first (`prepare`: the sealed DM to the creator, the signed `report_v2` to the
//! admins, or both) and only then sends, so a report never goes half. The DM to the creator is one
//! `dm_put` flagged `group_report`, with no self-copy: my other devices have nothing to do with a
//! report I sent. A `reach_refused` for the creator within a minute is about the report
//! (`refused`), so it is said as such rather than offering a contact request.
//!
//! THE CREATOR. Such a DM is taken before the "show as a request" rule (a group's members need
//! not be my friends), never stored or shown as a message, and kept in the DM store as waiting
//! (`ingest`, or `take_into` on a parked server's store). `pump` checks each waiting report off
//! the frame against this server's group list and my own copy of the group, then keeps it or
//! drops it; a kept one is announced once. The three actions are `remove` (a new group key for
//! everyone else, then the remove), `block_them` and `dismiss`.
//!
//! `pump` runs once a frame from lib.rs, beside the background connections.

use std::sync::mpsc::{channel, TryRecvError};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::gui::GuiState;
use crate::net::dm_pq::DmInner;
use crate::net::dm_store::DmStore;
use crate::net::group_remove::RemoveError;
use crate::net::group_report::{self as gr, CheckOutcome, Dest, GroupTarget, PendingReport};
use crate::net::report::{DraftProblem, ReportDialog};

/// How long a check that could not run waits before it is tried again.
const RETRY_AFTER: Duration = Duration::from_secs(30);

// ── The reporter ─────────────────────────────────────────────────────────────────────────────

/// The dialog's group part for a group message's Report: its group (named as this app's list
/// names it), the message as an item (None for a file, or a row whose signed-object id is not
/// known), and the creator still to be found. None for a message that is not a group's.
pub(crate) fn target_for(gs: &GuiState, msg: &crate::gui::ChatMessage) -> Option<GroupTarget> {
    let gid = msg.channel.strip_prefix("p2pgroup:")?;
    let name = gs.p2p_groups.iter().find(|g| g.group_id == gid).map(|g| g.name.clone()).unwrap_or_default();
    Some(GroupTarget {
        id: gid.to_string(),
        name,
        item: gr::item(&msg.group_object_id, &msg.sender_key, msg.timestamp_ms, &msg.content),
        creator: None,
        // No id to look up: the creator cannot be found out, and only the admins remain.
        finding: gr::id_norm(gid).is_some(),
        send_to: None,
    })
}

/// Find the open dialog's group creator: known already (a group's creator never changes), or
/// asked of the server off the frame, its answer applied by `pump`.
pub(crate) fn start_finding(gs: &mut GuiState) {
    let Some(gid) = gs.reports.dialog.as_ref().and_then(|d| d.group.as_ref()).filter(|g| g.finding).map(|g| g.id.clone()) else {
        return;
    };
    if let Some(known) = gs.reports.group.creators.get(&gid).cloned() {
        creator_found(gs, &gid, Some(known));
        return;
    }
    let server = gs.server_url.clone();
    let (tx, rx) = channel();
    let id = gid.clone();
    std::thread::spawn(move || {
        let creator = crate::net::api_v2::fetch_signed_object(&server, &id)
            .ok()
            .flatten()
            .and_then(|obj| gr::creator_from_group_object(&obj, &id));
        let _ = tx.send(creator);
    });
    gs.reports.group.finding = Some((gid, rx));
}

/// The lookup answered (None: the group's record did not load, or did not check). A creator read
/// from the record is remembered. When it could not be found out, a group this app's list says I
/// created is mine (the web's fallback), and otherwise only the admins remain.
pub(crate) fn creator_found(gs: &mut GuiState, group_id: &str, creator: Option<String>) {
    if let Some(c) = &creator {
        gs.reports.group.creators.insert(group_id.to_string(), c.clone());
    }
    let creator = creator.or_else(|| {
        gs.p2p_groups.iter().any(|g| g.group_id == group_id && g.is_creator).then(|| gs.profile_public_key.to_ascii_lowercase())
    });
    if let Some(g) = gs.reports.dialog.as_mut().and_then(|d| d.group.as_mut()).filter(|g| g.id == group_id) {
        g.found(creator);
    }
}

/// The sealed DM to the creator for the open dialog: the creator's key and the `dm_put`, flagged
/// `group_report`, sealed to them alone (no self-copy). Err is the sentence the dialog shows.
pub(crate) fn creator_put(gs: &GuiState, d: &ReportDialog) -> Result<(String, Value), String> {
    let g = d.group.as_ref().ok_or(gr::CREATOR_UNKNOWN)?;
    let creator = g.creator.clone().ok_or(gr::CREATOR_UNKNOWN)?;
    let fields = gr::Report {
        group_id: g.id.clone(),
        group_name: g.name.clone(),
        target: d.target.clone(),
        reason: d.reason.clone(),
        note: d.note.clone(),
        items: g.item.clone().into_iter().collect(),
    };
    let text = gr::build_text(&fields)?;
    if gs.private_key_bytes.is_none() {
        return Err(DraftProblem::Locked.sentence().to_string());
    }
    if !gs.peer_kyber_keys.contains_key(&creator) {
        return Err(gr::CANNOT_SEAL.to_string());
    }
    // The same signed, sealed v2 DM a control message is (engine/dm.rs `control_puts`), so it
    // carries the creator's pass when I hold one. Its self-copy is not sent.
    let (mut put, _self_copy) = crate::engine::dm::control_puts(gs, &creator, &text, None).ok_or(gr::CANNOT_SEAL)?;
    put["group_report"] = Value::Bool(true);
    Ok((creator, put))
}

/// What Send puts on the wire, all of it built before any of it is sent.
#[derive(Debug)]
pub(crate) struct Outgoing {
    /// The creator's key and the sealed DM to them.
    pub to_creator: Option<(String, Value)>,
    /// The signed `report_v2` frame for this server's admins.
    pub to_admins: Option<String>,
    /// Who to block as it goes ("Also block them").
    pub block: Option<String>,
}

/// Build the open dialog's report for where it goes (10j): to the admins as step D always did, to
/// the group's creator, or to both. Err is the sentence the dialog shows; nothing is half built.
pub(crate) fn prepare(gs: &GuiState, ts: u64) -> Result<Outgoing, String> {
    let d = gs.reports.dialog.as_ref().ok_or(DraftProblem::NoTarget.sentence())?;
    let dest = match &d.group {
        None => Dest::Admins,
        Some(g) => g.destination(&gs.profile_public_key, &d.target).ok_or(gr::STILL_FINDING)?,
    };
    let to_creator = if dest.to_creator() { Some(creator_put(gs, d)?) } else { None };
    let (to_admins, block) = if dest.to_admins() {
        let (frame, block) = crate::engine::report::prepare_send(gs, ts).map_err(|p| p.sentence().to_string())?;
        (Some(frame), block)
    } else {
        (None, d.also_block.then(|| d.target.clone()))
    };
    Ok(Outgoing { to_creator, to_admins, block })
}

/// A report to the creator went out: said once, and its time kept so a refusal can be told apart.
pub(crate) fn note_sent(gs: &mut GuiState, creator: &str) {
    gs.reports.group.sent.insert(creator.to_ascii_lowercase(), Instant::now());
    gs.pending_notices.push(gr::SENT_LINE.to_string());
}

/// A `reach_refused` for a message to `to`: when a report went to them within the last minute,
/// it was the report the server did not let through, and the reporter is told so (true). Anything
/// else is an ordinary refusal (false).
pub(crate) fn refused(gs: &mut GuiState, to: &str) -> bool {
    let to = to.to_ascii_lowercase();
    match gs.reports.group.sent.get(&to) {
        Some(at) if at.elapsed() <= gr::REFUSAL_WINDOW => {}
        _ => return false,
    }
    gs.reports.group.sent.remove(&to);
    gs.pending_notices.push(gr::REFUSED_LINE.to_string());
    true
}

// ── The creator ──────────────────────────────────────────────────────────────────────────────

/// A verified DM on the active server whose text is a report about a group: kept waiting for its
/// check when it reads as one and is not my own. True whenever the text was a report, so the
/// caller never stores or shows it as a message.
pub(crate) fn ingest(gs: &mut GuiState, inner: &DmInner) -> bool {
    if !gr::is_report_text(&inner.text) {
        return false;
    }
    let me = gs.profile_public_key.clone();
    if let Some(store) = gs.dm_store.as_mut() {
        if take_into(store, &me, inner) {
            store.save();
        }
    }
    true
}

/// The same on any server's store (a parked server's is checked once it is the active one); the
/// caller saves. True when a new report now waits.
pub(crate) fn take_into(store: &mut DmStore, me: &str, inner: &DmInner) -> bool {
    gr::pending_from(me, inner).is_some_and(|p| store.add_pending_group_report(p))
}

/// The server the loaded DM store belongs to (the store is per server, and so are the groups).
fn current_server(gs: &GuiState) -> Option<String> {
    gs.dial_address().map(crate::gui::pages::chat::norm_server_url)
}

/// Once a frame: the dialog's creator lookup, checks and removals that finished, and checks to
/// start for reports waiting on this server.
pub(crate) fn pump(gs: &mut GuiState) {
    if let Some((gid, rx)) = gs.reports.group.finding.as_ref() {
        let answer = match rx.try_recv() {
            Ok(creator) => Some(creator),
            Err(TryRecvError::Disconnected) => Some(None),
            Err(TryRecvError::Empty) => None,
        };
        if let Some(creator) = answer {
            let gid = gid.clone();
            gs.reports.group.finding = None;
            creator_found(gs, &gid, creator);
        }
    }
    let mut checked = Vec::new();
    gs.reports.group.checking.retain(|(server, id, rx)| match rx.try_recv() {
        Ok(outcome) => {
            checked.push((server.clone(), id.clone(), outcome));
            false
        }
        Err(TryRecvError::Disconnected) => false,
        Err(TryRecvError::Empty) => true,
    });
    for (server, id, outcome) in checked {
        apply_check(gs, &server, &id, outcome);
    }
    let mut removed = Vec::new();
    gs.reports.group.removing.retain(|(id, rx)| match rx.try_recv() {
        Ok(result) => {
            removed.push((id.clone(), result));
            false
        }
        Err(TryRecvError::Disconnected) => {
            removed.push((id.clone(), Err(RemoveError::NoKey("the worker stopped".into()))));
            false
        }
        Err(TryRecvError::Empty) => true,
    });
    for (id, result) in removed {
        removal_done(gs, &id, result);
    }
    start_checks(gs);
}

/// Start a check, off the frame, for each report waiting on this server that is not being checked.
fn start_checks(gs: &mut GuiState) {
    let Some(store) = gs.dm_store.as_ref() else { return };
    if store.pending_group_reports().is_empty() {
        return;
    }
    let (Some(seed), Some(server)) = (gs.private_key_bytes.clone(), current_server(gs)) else { return };
    let waiting: Vec<PendingReport> = store.pending_group_reports().to_vec();
    let me = gs.profile_public_key.clone();
    // The list as this app last had it, for when the server's cannot be read now (the web's
    // "the list as it was"); none when no list was ever fetched.
    let known = gs.p2p_groups_last_fetch.is_some().then(|| gs.p2p_groups.clone());
    let now = Instant::now();
    for p in waiting {
        let busy = gs.reports.group.checking.iter().any(|(s, id, _)| *s == server && *id == p.id);
        if busy || gs.reports.group.retry_at.get(&p.id).is_some_and(|at| *at > now) {
            continue;
        }
        let (tx, rx) = channel();
        let (http, seed, me, known, id) = (gs.server_url.clone(), seed.clone(), me.clone(), known.clone(), p.id.clone());
        std::thread::spawn(move || {
            let _ = tx.send(check(&http, &seed, &me, &p, known));
        });
        gs.reports.group.checking.push((server.clone(), id, rx));
    }
}

/// One report's check, on a worker thread: this server's group list (or the one known), then my
/// own copy of the group the report names.
fn check(http: &str, seed: &[u8], me: &str, p: &PendingReport, known: Option<Vec<crate::net::api_v2::P2pGroupInfo>>) -> CheckOutcome {
    let Some(groups) = crate::net::api_v2::fetch_p2p_groups(http, me).ok().or(known) else { return CheckOutcome::Retry };
    if let Err(why) = gr::accepts(me, &p.from, &p.to, &p.report, &groups) {
        return CheckOutcome::Dropped(why);
    }
    // A copy that cannot be read leaves every item "Not found in your copy": shown, never dropped.
    let gid = &p.report.group_id;
    let objects = crate::net::api_v2::fetch_group_message_objects(http, gid).unwrap_or_default();
    let keys = crate::net::api_v2::fetch_all_epoch_keys(http, seed, gid).unwrap_or_default();
    match gr::resolve(me, p, &groups, &objects, &keys) {
        Ok(kept) => CheckOutcome::Kept(kept),
        Err(why) => CheckOutcome::Dropped(why),
    }
}

/// A check finished: the report is kept (and announced once) or dropped, or tried again later.
/// An answer for another server than the one loaded now waits: its report stays in that server's
/// store and is checked again when that server is the active one.
pub(crate) fn apply_check(gs: &mut GuiState, server: &str, id: &str, outcome: CheckOutcome) {
    if current_server(gs).as_deref() != Some(server) {
        return;
    }
    let Some(store) = gs.dm_store.as_mut() else { return };
    let kept = match outcome {
        CheckOutcome::Retry => {
            gs.reports.group.retry_at.insert(id.to_string(), Instant::now() + RETRY_AFTER);
            return;
        }
        CheckOutcome::Dropped(why) => {
            log::info!("a report about a group was not kept ({why:?})");
            store.settle_group_report(id, None);
            store.save();
            return;
        }
        CheckOutcome::Kept(k) => k,
    };
    let (from, target, group) = (kept.from.clone(), kept.target.clone(), kept.group_name.clone());
    let new = store.settle_group_report(id, Some(kept));
    store.save();
    gs.reports.group.retry_at.remove(id);
    if new {
        let line = gr::arrived_line(
            &crate::engine::dm::dm_display_name(gs, &from),
            &crate::engine::dm::dm_display_name(gs, &target),
            &group,
        );
        gs.pending_notices.push(line);
    }
}

/// How many kept reports are about this group (the count on it in the group list).
pub(crate) fn count_for(gs: &GuiState, group_id: &str) -> usize {
    gs.dm_store.as_ref().map(|s| s.group_report_count(group_id)).unwrap_or(0)
}

/// Is the person a report is about in its group now, as this app's group list says?
pub(crate) fn in_group(gs: &GuiState, group_id: &str, key: &str) -> bool {
    gs.p2p_groups.iter().any(|g| g.group_id == group_id && g.members.iter().any(|m| m.eq_ignore_ascii_case(key)))
}

/// "Remove them from the group", once confirmed: a new group key for everyone else, then the
/// signed remove (net/group_remove.rs), off the frame; `removal_done` says how it went.
pub(crate) fn remove(gs: &mut GuiState, id: &str) {
    gs.reports.group.confirm_remove = None;
    let Some(rec) = gs.dm_store.as_ref().and_then(|s| s.group_report(id)).cloned() else { return };
    if gs.reports.group.removing.iter().any(|(r, _)| r == id) {
        return;
    }
    let Some(seed) = gs.private_key_bytes.clone() else {
        gs.pending_notices.push(DraftProblem::Locked.sentence().to_string());
        return;
    };
    let server = gs.server_url.clone();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::net::group_remove::remove_member(&mut crate::net::group_remove::Http(&server), &seed, &rec.group_id, &rec.target));
    });
    gs.reports.group.removing.push((id.to_string(), rx));
}

/// A removal finished. Done: the report says "Removed from the group.", an open view of the group
/// sends under the new key at once, and the group list is fetched again. Not done: why, in the
/// web's words.
pub(crate) fn removal_done(gs: &mut GuiState, id: &str, result: Result<(u64, Vec<u8>), RemoveError>) {
    let Some(rec) = gs.dm_store.as_ref().and_then(|s| s.group_report(id)).cloned() else { return };
    let name = crate::engine::dm::dm_display_name(gs, &rec.target);
    match result {
        Ok((epoch, key)) => {
            if let Some(store) = gs.dm_store.as_mut() {
                store.set_group_report_removed(id);
                store.save();
            }
            if gs.p2p_group_active_id == rec.group_id {
                gs.p2p_group_chat_epoch = epoch;
                gs.p2p_group_chat_epoch_key = Some(key);
            }
            gs.pending_notices.push(gr::removed_line(&name, &rec.group_name));
            crate::gui::pages::chat::spawn_groups_list_refresh(gs);
        }
        Err(e) => {
            log::warn!("removing {name} from {} did not go through: {e:?}", rec.group_name);
            gs.pending_notices.push(e.sentence(&name, &rec.group_name));
        }
    }
}

/// "Block them": step C's Block of the person the report is about. The report stays until
/// dismissed.
pub(crate) fn block_them(gs: &mut GuiState, id: &str) {
    if let Some(target) = gs.dm_store.as_ref().and_then(|s| s.group_report(id)).map(|r| r.target.clone()) {
        crate::engine::block::block(gs, &target);
    }
}

/// "Dismiss": the report is gone from this device. Nobody is told.
pub(crate) fn dismiss(gs: &mut GuiState, id: &str) -> bool {
    if gs.reports.group.confirm_remove.as_deref() == Some(id) {
        gs.reports.group.confirm_remove = None;
    }
    let Some(store) = gs.dm_store.as_mut() else { return false };
    let gone = store.remove_group_report(id);
    if gone {
        store.save();
    }
    gone
}

#[cfg(test)]
#[path = "group_report_tests.rs"]
mod tests;

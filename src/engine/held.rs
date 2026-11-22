//! Hide and keep's frames and jobs in the desktop app (2026-11-20, docs/design/hide-and-keep.md
//! rung 5): the GUI's side of src/net/held.rs. The screens are src/gui/pages/server_settings/held.rs
//! (Held copies) and server_settings/reports.rs (the "Hide and keep" decision).
//!
//! The owner's Export and Restore run on a thread of their own once the relay has recorded the
//! signed action that grants them (the `held_items` list it sends straight after): the thread
//! fetches the sealed parts, opens them with the owner's Kyber768 key, and writes them into the
//! .zip file the owner chose (Export) or sends them back to the server (Restore). The page only
//! ever sees words about it ("opened into <file>"), never what it held.

use crate::gui::GuiState;
use crate::net::held::{self, Confirm, HeldItem, JobKind, Pending};
use humanity_core::i18n::{t, tf};

/// How long a sent Export or Restore waits for the relay's answer before saying there was none.
const ANSWER_WAIT_MS: i64 = 15_000;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn connected(gs: &GuiState) -> bool {
    gs.ws_client.as_ref().is_some_and(|c| c.is_connected())
}

fn send(gs: &GuiState, frame: &str) {
    if let Some(c) = gs.ws_client.as_ref() {
        c.send(frame);
    }
}

/// Sign `action` on `target` as me, for this server; the words to show when it cannot be signed.
fn sign(gs: &GuiState, action: &str, target: i64, reason: &str, until: u64) -> Result<held::Signed, String> {
    let Some(seed) = gs.private_key_bytes.as_ref() else { return Err(t("held.need_key").to_string()) };
    let Some(did) = gs.dm_store.as_ref().and_then(|s| s.pass_server()) else { return Err(t("held.no_server_did").to_string()) };
    held::sign(seed, did, action, target, reason, until, now_ms() as u64).map_err(|e| tf("held.sign_failed", &[("why", &e)]))
}

/// Ask the server for Held copies.
pub(crate) fn request_list(gs: &mut GuiState) {
    if !connected(gs) {
        gs.reports.held.status = t("held.not_connected").to_string();
        gs.reports.held.requested = false;
        return;
    }
    send(gs, &held::list_frame());
    gs.reports.held.requested = true;
}

/// The relay's `held_items`: kept for the page, and a granted Export or Restore started.
pub(crate) fn on_frame(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    let Some(list) = held::parse_list(frame) else { return false };
    gs.reports.held.keep = Some((list.can_keep, list.cannot_keep_why.clone()));
    gs.reports.held.list = Some(list);
    start_pending(gs);
    true
}

/// The Reports list carries whether hide and keep can seal a copy here.
pub(crate) fn note_reports_frame(gs: &mut GuiState, frame: &serde_json::Value) {
    if let Some(can) = frame.get("can_keep").and_then(|v| v.as_bool()) {
        let why = frame.get("cannot_keep_why").and_then(|v| v.as_str()).unwrap_or("").to_string();
        gs.reports.held.keep = Some((can, why));
    }
}

/// A `private` line from the relay about held items (its answer to an action, or a refusal) is
/// also shown on Held copies.
pub(crate) fn on_private(gs: &mut GuiState, message: &str) {
    // The relay's answer to a report decision is shown on the Reports page too, where it was made
    // (the third review's S3: a hide during an owner change says there that its sealed copy waits).
    if message.starts_with("Report #") {
        gs.reports.status = message.to_string();
    }
    if held::is_held_words(message) {
        gs.reports.held.status = message.to_string();
        if message.starts_with("Nothing was done") || message.contains("could not") {
            gs.reports.held.pending = None; // refused: no grant is coming
        }
    }
}

/// Decide report `id` with hide and keep, signed by me. The reason signed is the decision's note,
/// else "Report <id>: <reason>".
pub(crate) fn hide_and_keep(gs: &mut GuiState, id: &serde_json::Value, note: &str, reason_label: &str) {
    if !connected(gs) {
        gs.reports.status = t("held.not_connected").to_string();
        return;
    }
    let number = id.as_i64().or_else(|| id.as_str().and_then(|s| s.parse().ok())).unwrap_or(0);
    let reason = if note.trim().is_empty() { tf("held.decide_reason", &[("id", &number), ("reason", &reason_label)]) } else { note.trim().to_string() };
    match sign(gs, "hide_and_keep", number, &reason, 0) {
        Ok(s) => {
            send(gs, &held::decide_frame(id, note.trim(), &s));
            gs.reports.status = tf("held.decide_sent", &[("id", &number)]);
            crate::engine::report::request_list(gs);
            request_list(gs);
        }
        Err(why) => gs.reports.status = why,
    }
}

/// The reason typed for `item`, or the words asking for one.
fn reason_of(gs: &GuiState, item: i64) -> Result<String, String> {
    let r = gs.reports.held.reasons.get(&item).map(|s| s.trim().to_string()).unwrap_or_default();
    if r.is_empty() {
        Err(t("held.reason_needed").to_string())
    } else {
        Ok(r)
    }
}

/// Send the owner's signed `action` on `item`; the signed action, or why not (in the status).
fn act(gs: &mut GuiState, item: i64, action: &str, until: u64) -> Option<held::Signed> {
    if !connected(gs) {
        gs.reports.held.status = t("held.not_connected").to_string();
        return None;
    }
    let signed = match reason_of(gs, item).and_then(|r| sign(gs, action, item, &r, until)) {
        Ok(s) => s,
        Err(why) => {
            gs.reports.held.status = why;
            return None;
        }
    };
    send(gs, &held::act_frame(&signed));
    Some(signed)
}

/// Place a legal hold for `days` from today on `item` (0 lifts it).
pub(crate) fn legal_hold(gs: &mut GuiState, item: i64, days: i64) {
    let Some(until) = held::hold_until(days, now_ms()) else {
        gs.reports.held.status = t("held.hold_days_range").to_string();
        return;
    };
    if act(gs, item, "held_legal_hold", until).is_some() {
        gs.reports.held.status = tf(if until == 0 { "held.hold_lift_sent" } else { "held.hold_sent" }, &[("id", &item)]);
    }
}

/// Destroy `item` now (the page asked first).
pub(crate) fn destroy(gs: &mut GuiState, item: i64) {
    gs.reports.held.confirm = None;
    if act(gs, item, "held_destroy", 0).is_some() {
        gs.reports.held.status = tf("held.destroy_sent", &[("id", &item)]);
    }
}

/// Export `item` into a new .zip file in `folder`: signed now, run once the relay granted it.
pub(crate) fn export(gs: &mut GuiState, item: &HeldItem, folder: &std::path::Path) {
    let Some(signed) = act(gs, item.id, "held_export", 0) else { return };
    let file = held::export_path(folder, item.id);
    gs.reports.held.status = tf("held.exporting", &[("id", &item.id)]);
    gs.reports.held.pending = Some(Pending { kind: JobKind::Export, item: item.clone(), grant: signed.object_id, file: Some(file), sent_at_ms: now_ms(), started: false });
}

/// Restore `item` (the page asked first): signed now, run once the relay granted it.
pub(crate) fn restore(gs: &mut GuiState, item: &HeldItem) {
    gs.reports.held.confirm = None;
    let Some(signed) = act(gs, item.id, "held_restore", 0) else { return };
    gs.reports.held.status = tf("held.restoring", &[("id", &item.id)]);
    gs.reports.held.pending = Some(Pending { kind: JobKind::Restore, item: item.clone(), grant: signed.object_id, file: None, sent_at_ms: now_ms(), started: false });
}

/// Start the granted job on a thread of its own: it fetches, opens and writes or sends; the page
/// learns only its words.
fn start_pending(gs: &mut GuiState) {
    let Some(p) = gs.reports.held.pending.as_mut().filter(|p| !p.started) else { return };
    let Some(seed) = gs.private_key_bytes.clone() else { return };
    p.started = true;
    let p = p.clone();
    let me = gs.profile_public_key.clone();
    let base = crate::gui::widgets::image_cache::http_base(&gs.server_url);
    let (tx, rx) = std::sync::mpsc::channel();
    gs.reports.held.running = Some(rx);
    std::thread::spawn(move || {
        let _ = tx.send(run(&seed, &me, &base, &p));
    });
}

/// The job itself (on its own thread).
fn run(seed: &[u8], me: &str, base: &str, p: &Pending) -> Result<String, String> {
    use humanity_protocol::pq_crypto::{derive_kyber_seed, KyberKeypair};
    let kyber = KyberKeypair::from_seed(&derive_kyber_seed(seed)).map_err(|e| e.to_string())?;
    let (id, grant) = (p.item.id, p.grant.as_str());
    // Every request signed by this app's own key (the owner review's finding 1).
    let caller = held::Caller::new(me, seed);
    let fetch = |part: usize| held::fetch_part(base, id, part, grant, &caller);
    // What opens must be the very item chosen: its package, report, author and post time as the list
    // showed them (the fourth review's finding 8).
    let chosen = humanity_protocol::held_seal::Chosen { package: &p.item.package, report_id: p.item.report_id, author: &p.item.author, stamp: p.item.stamp.max(0) as u64 };
    match p.kind {
        JobKind::Export => {
            let file = p.file.clone().ok_or_else(|| t("held.no_file").to_string())?;
            let done = held::export_with(&kyber, &chosen, &file, now_ms(), fetch).map_err(|e| tf("held.export_failed", &[("why", &e)]))?;
            Ok(tf("held.export_done", &[("id", &id), ("path", &done.path.display())]))
        }
        JobKind::Restore => humanity_protocol::held_seal::restore_with(
            &kyber,
            &chosen,
            fetch,
            |part, key, src| held::upload_part(base, id, part, grant, key, &caller, src),
            |key| held::finish_restore(base, id, grant, key, &caller),
        )
        .map_err(|e| tf("held.restore_failed", &[("why", &e)])),
    }
}

/// Every frame the page is drawn: a finished job's words, or an answer that never came.
pub(crate) fn poll(gs: &mut GuiState) {
    if let Some(rx) = gs.reports.held.running.as_ref() {
        match rx.try_recv() {
            Ok(result) => {
                gs.reports.held.status = result.unwrap_or_else(|e| e);
                gs.reports.held.running = None;
                gs.reports.held.pending = None;
                request_list(gs);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                gs.reports.held.running = None;
                gs.reports.held.pending = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    if gs.reports.held.pending.as_ref().is_some_and(|p| !p.started && now_ms() - p.sent_at_ms > ANSWER_WAIT_MS) {
        gs.reports.held.pending = None;
        gs.reports.held.status = t("held.no_answer").to_string();
    }
}

/// Ask before Destroy now or Restore.
pub(crate) fn ask(gs: &mut GuiState, item: i64, what: Confirm) {
    gs.reports.held.confirm = Some((item, what));
}

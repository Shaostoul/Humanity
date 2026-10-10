//! Reports the admins can check (step D of docs/design/blocking-and-safe-mode.md, section 10e,
//! 2026-10-09): the native client's half that touches the app state and the socket. The rules
//! themselves (the signed report, its evidence, the picker's choice, reading the admins' list)
//! are src/net/report.rs; the Report dialog is src/gui/pages/chat/report_dialog.rs and the
//! admins' Reports list src/gui/pages/server_settings/reports.rs.
//!
//! A report goes to the server the app is on, over the signed-in socket, as `report_v2`. Its
//! answer is `report_received` (a short confirmation here) or a Private notice saying why it was
//! refused, which the chat shows like every other Private notice. With "Also block them" ticked
//! (the default for a DM report), step C's Block runs as the report goes.
//!
//! Opening a dialog also loads the help outside this server (10e-ii, src/net/outside_help.rs)
//! and starts it on the country last picked on this device. That country is never sent.
//!
//! The message pump (frame_ws_poll.rs) reaches this file through `on_frame`, called from its
//! catch-all arm: the pump sits at its file-size budget, so nothing more goes there.

use crate::gui::GuiState;
use crate::net::outside_help;
use crate::net::report::{self, DraftProblem, Evidence, ReportContext, ReportDialog, ReportDraft};

/// The confirmation `report_received` shows.
pub(crate) const RECEIVED_LINE: &str =
    "Report received. This server's admins and moderators will look at it. The person is not told who reported them.";

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Load the reasons (`data/safety/report_reasons.json`) the first time a dialog opens, and try
/// again on the next one if they could not be read.
pub(crate) fn ensure_reasons(gs: &mut GuiState) {
    if !gs.reports.reasons.is_empty() {
        return;
    }
    match report::load_reasons(&crate::data_dir()) {
        Ok(reasons) => {
            gs.reports.reasons = reasons;
            gs.reports.reasons_error = None;
        }
        Err(e) => {
            log::warn!("{e}");
            gs.reports.reasons_error = Some(e);
        }
    }
}

/// Load the help outside this server (`data/safety/outside_help.json`, 10e-ii) the first time a
/// dialog opens, and try again on the next one if it could not be read. Without it the dialog
/// simply has no such block: a report never waits on this file.
pub(crate) fn ensure_outside_help(gs: &mut GuiState) {
    if gs.reports.outside_help.is_some() {
        return;
    }
    match outside_help::load(&crate::data_dir()) {
        Ok(help) => gs.reports.outside_help = Some(help),
        Err(e) => log::warn!("{e}"),
    }
}

// ── Opening the dialog ──────────────────────────────────────────────────────────────────────

/// A message's menu: a DM opens a DM report with that message ticked; a P2P group message
/// reports its text (unproven); anything else is a public post, reported by author and time.
pub(crate) fn open_for_message(gs: &mut GuiState, msg: &crate::gui::ChatMessage) {
    if let Some(peer) = msg.channel.strip_prefix("dm:") {
        open_for_dm(gs, peer, Some(msg.timestamp_ms));
        return;
    }
    // A group message whose author the roster has not resolved yet carries their fingerprint in
    // place of their key (chat/p2p_groups.rs), and a report must name a key.
    let full_key = msg.sender_key.len() == 2 * crate::relay::core::pq_crypto::DILITHIUM_PK_LEN
        && msg.sender_key.bytes().all(|b| b.is_ascii_hexdigit());
    if msg.channel.starts_with("p2pgroup:") && !full_key {
        gs.pending_notices.push("This group member is not known yet, so they cannot be reported from here. Try again in a moment.".to_string());
        return;
    }
    let (context, item) = if msg.channel.starts_with("p2pgroup:") {
        // A group message with a file carries the key that opens it, so it is never evidence
        // (net/report.rs `carries_file`): the report goes without it, and the dialog says why.
        let item = (!report::carries_file(&msg.content))
            .then(|| Evidence::GroupText { from: msg.sender_key.clone(), ts: msg.timestamp_ms, text: msg.content.clone() });
        (ReportContext::Group, item)
    } else {
        (ReportContext::Post, Some(Evidence::Post { from: msg.sender_key.clone(), timestamp: msg.timestamp_ms }))
    };
    let fixed_text = if item.is_some() { msg.content.clone() } else { String::new() };
    // A group message can also go to the group's creator (10j): its group, the message by its
    // signed-object id, and a lookup of who created the group, started as the dialog opens.
    let group = crate::engine::group_report::target_for(gs, msg);
    open(
        gs,
        ReportDialog {
            target: msg.sender_key.clone(),
            target_name: msg.sender_name.clone(),
            context: Some(context),
            fixed: item,
            fixed_text,
            group,
            ..Default::default()
        },
    );
    crate::engine::group_report::start_finding(gs);
}

/// A DM conversation (its header, or one of its messages): that person's messages to us,
/// `pressed_ts` (or else the most recent) ticked, and "Also block them" ticked.
pub(crate) fn open_for_dm(gs: &mut GuiState, peer: &str, pressed_ts: Option<u64>) {
    crate::engine::dm::ensure_dm_store(gs);
    let me = gs.profile_public_key.clone();
    let candidates = gs
        .dm_store
        .as_ref()
        .map(|s| report::dm_candidates(s.conversation(peer), peer, &me, pressed_ts))
        .unwrap_or_default();
    let target_name = crate::engine::dm::dm_display_name(gs, peer);
    open(
        gs,
        ReportDialog {
            target: peer.to_string(),
            target_name,
            context: Some(ReportContext::Dm),
            candidates,
            also_block: true,
            ..Default::default()
        },
    );
}

/// A person's entry in the member list, or their profile: no evidence.
pub(crate) fn open_for_person(gs: &mut GuiState, key: &str) {
    let target_name = crate::engine::dm::dm_display_name(gs, key);
    open(gs, ReportDialog { target: key.to_string(), target_name, context: Some(ReportContext::Profile), ..Default::default() });
}

fn open(gs: &mut GuiState, mut dialog: ReportDialog) {
    if dialog.target.is_empty() || dialog.target == gs.profile_public_key {
        return;
    }
    ensure_reasons(gs);
    // The help outside this server starts on the country last picked on this device, else
    // Another country (10e-ii; this app reads no OS locale, so there is no language step). The
    // person's location is never looked up.
    ensure_outside_help(gs);
    if let Some(help) = gs.reports.outside_help.as_ref() {
        dialog.country = outside_help::first_country(help, &gs.settings.outside_help_country);
    }
    gs.reports.dialog = Some(dialog);
}

// ── Sending ─────────────────────────────────────────────────────────────────────────────────

/// The draft the open dialog describes: a DM report's ticked messages, or the one post or
/// group message it was opened on.
pub(crate) fn draft_of(d: &ReportDialog, ts: u64) -> ReportDraft {
    let context = d.context.unwrap_or(ReportContext::Profile);
    let evidence = match context {
        ReportContext::Dm => report::chosen(&d.candidates),
        _ => d.fixed.clone().into_iter().collect(),
    };
    ReportDraft { target: d.target.clone(), context, reason: d.reason.clone(), note: d.note.trim().to_string(), evidence, ts }
}

/// The signed `report_v2` frame for the open dialog, and who to block as it goes (None unless
/// "Also block them" is ticked).
pub(crate) fn prepare_send(gs: &GuiState, ts: u64) -> Result<(String, Option<String>), DraftProblem> {
    let d = gs.reports.dialog.as_ref().ok_or(DraftProblem::NoTarget)?;
    let seed = gs.private_key_bytes.as_deref().unwrap_or_default();
    let frame = report::build_report_frame(seed, &gs.profile_public_key, &draft_of(d, ts))?;
    Ok((frame, d.also_block.then(|| d.target.clone())))
}

/// The dialog's Send: the report goes to this server (or, for a group message, to the group's
/// creator, or both: 10j, engine/group_report.rs, everything built before anything is sent) and
/// the dialog closes, and with "Also block them" ticked the person is blocked too. When it cannot
/// go, the dialog stays open and says why.
pub(crate) fn send(gs: &mut GuiState) {
    if !gs.ws_client.as_ref().is_some_and(|c| c.is_connected()) {
        if let Some(d) = gs.reports.dialog.as_mut() {
            d.problem = "Not connected to the server, so nothing was sent. Connect and send it again.".to_string();
        }
        return;
    }
    // Step F's recovery-phrase guard on the note we wrote (the evidence is the other person's).
    let note = gs.reports.dialog.as_ref().map(|d| d.note.clone()).unwrap_or_default();
    if crate::engine::warnings::holds_own_phrase(gs, &[&note]) {
        if let Some(d) = gs.reports.dialog.as_mut() {
            d.problem = crate::net::warnings::GUARD_LINE.to_string();
        }
        return;
    }
    match crate::engine::group_report::prepare(gs, now_ms()) {
        Ok(out) => {
            if let (Some((creator, put)), Some(client)) = (out.to_creator.as_ref(), gs.ws_client.as_ref()) {
                client.send(&put.to_string());
                crate::engine::group_report::note_sent(gs, creator);
            }
            if let (Some(frame), Some(client)) = (out.to_admins.as_ref(), gs.ws_client.as_ref()) {
                client.send(frame);
            }
            gs.reports.dialog = None;
            if let Some(key) = out.block {
                crate::engine::block::block(gs, &key); // step C: nothing is sent to them
            }
        }
        Err(problem) => {
            if let Some(d) = gs.reports.dialog.as_mut() {
                d.problem = problem;
            }
        }
    }
}

// ── The relay's answers ─────────────────────────────────────────────────────────────────────

/// Step D's frames from the message pump: `report_received` (a short confirmation) and
/// `reports` (the admins' list). False for any other frame, which the pump then logs as
/// unhandled the way it always has.
pub(crate) fn on_frame(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    match frame.get("type").and_then(|t| t.as_str()) {
        Some("report_received") => {
            gs.pending_notices.push(RECEIVED_LINE.to_string());
            true
        }
        Some("reports") => {
            gs.reports.list = report::parse_reports(frame);
            gs.reports.list_is_decided = match frame.get("state").and_then(|s| s.as_str()) {
                Some(state) => state == "decided",
                None => gs.reports.asked_decided,
            };
            true
        }
        _ => false,
    }
}

// ── The admins' list ────────────────────────────────────────────────────────────────────────

/// Ask this server for its open or decided reports (as `show_decided` says).
pub(crate) fn request_list(gs: &mut GuiState) {
    let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) else {
        gs.reports.status = "Not connected to the server.".to_string();
        gs.reports.requested = false; // the page asks again once a socket is up
        return;
    };
    client.send(&report::list_frame(gs.reports.show_decided));
    gs.reports.status.clear();
    gs.reports.requested = true;
    gs.reports.asked_decided = gs.reports.show_decided;
}

/// A decision on a report, carried out by the relay through its moderation path; then the list
/// is asked for again, so the report moves to Decided once the relay has recorded it.
pub(crate) fn decide(gs: &mut GuiState, id: &serde_json::Value, decision: report::Decision, note: &str) {
    let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) else {
        gs.reports.status = "Not connected to the server, so no decision was sent.".to_string();
        return;
    };
    if crate::engine::warnings::holds_own_phrase(gs, &[note]) {
        gs.reports.status = crate::net::warnings::GUARD_LINE.to_string(); // step F's guard
        return;
    }
    client.send(&report::decide_frame(id, decision, note.trim()));
    request_list(gs);
    gs.reports.status = format!("Sent: {}.", decision.label());
}

/// Each test was seen red once on purpose, recorded at the test.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::dm_pq;
    use crate::net::dm_store::DmStore;
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, verify_dilithium, DilithiumKeypair};
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    fn identity(n: u8) -> (Vec<u8>, String) {
        let seed = vec![n; 32];
        (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
    }

    /// An app signed in as `me`, with a DM store held in memory (never saved by these tests).
    fn app(me: &str, seed: &[u8], tag: &str) -> GuiState {
        let mut gs = GuiState::default();
        gs.profile_public_key = me.to_string();
        gs.private_key_bytes = Some(seed.to_vec());
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        gs.dm_store = Some(DmStore::load(seed, me, &format!("wss://{tag}-{nanos}.example")));
        gs.block_list = Some(crate::net::block_list::BlockList::in_temp(seed, me, tag));
        gs.reports.reasons = report::parse_reasons(br#"[{"id":"harassment","label":"Harassment","help":"h"}]"#).unwrap();
        gs
    }

    /// A message `from` sends `to`, signed and verified the way it arrives, kept in the store.
    fn receive(gs: &mut GuiState, from_seed: &[u8], from: &str, to: &str, ts: u64, text: &str) {
        let inner = dm_pq::build_signed_inner(from_seed, from, to, ts, text).unwrap();
        let verified = dm_pq::parse_verify_inner(&inner).unwrap();
        assert!(gs.dm_store.as_mut().unwrap().insert(&verified));
    }

    /// A DM report end to end, short of the socket: the store keeps each message's signature,
    /// the dialog offers the person's messages with the right one ticked and "Also block them"
    /// ticked, and every item sent verifies over the DM v2 preimage against THEIR key, from them
    /// to us: exactly what the relay checks before it marks an item `checked`. Seen red
    /// 2026-10-09 by taking `sig: inner.sig_b64.clone()` out of `DmStore::insert`: with no
    /// signature kept, the dialog had nothing to offer.
    #[test]
    fn a_dm_report_carries_their_signed_messages_that_verify_against_their_key() {
        let (seed, me) = identity(51);
        let (dana_seed, dana) = identity(52);
        let mut gs = app(&me, &seed, "report-dm");
        receive(&mut gs, &dana_seed, &dana, &me, 1_000, "you will regret this");
        receive(&mut gs, &seed, &me, &dana, 1_500, "please stop");
        receive(&mut gs, &dana_seed, &dana, &me, 2_000, "I know where you live");

        open_for_dm(&mut gs, &dana, None);
        let d = gs.reports.dialog.as_ref().expect("the dialog is open");
        assert_eq!((d.context, d.also_block), (Some(ReportContext::Dm), true), "a DM report blocks them too by default");
        assert_eq!(d.candidates.iter().map(|c| (c.text.as_str(), c.ticked)).collect::<Vec<_>>(), [("I know where you live", true), ("you will regret this", false)]);

        gs.reports.dialog.as_mut().unwrap().candidates[1].ticked = true;
        gs.reports.dialog.as_mut().unwrap().reason = "harassment".into();
        let (frame, block) = prepare_send(&gs, 3_000).expect("a frame");
        assert_eq!(block.as_deref(), Some(dana.as_str()));
        let v: serde_json::Value = serde_json::from_str(&frame).unwrap();
        let items = v["evidence"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        let dana_key = hex::decode(&dana).unwrap();
        for item in items {
            assert_eq!((item["kind"].as_str(), item["from"].as_str(), item["to"].as_str()), (Some("dm"), Some(dana.as_str()), Some(me.as_str())));
            let preimage = dm_pq::sig_preimage(&dana, &me, item["ts"].as_u64().unwrap(), item["text"].as_str().unwrap());
            let sig = B64.decode(item["sig"].as_str().unwrap()).unwrap();
            assert!(verify_dilithium(&dana_key, preimage.as_bytes(), &sig).is_ok(), "the item proves Dana wrote it to us");
        }
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// A message's menu opens the right kind of report. Seen red 2026-10-09 three times: by
    /// dropping the `dm:` branch of `open_for_message`, a DM's Report became a post report with
    /// no picker; by dropping its `carries_file` check, a group message with a file went as
    /// evidence; by switching off its full-key check, a group author known only by fingerprint
    /// was reported under the fingerprint.
    #[test]
    fn a_messages_menu_opens_a_dm_post_or_group_report() {
        let (seed, me) = identity(53);
        let (dana_seed, dana) = identity(54);
        let mut gs = app(&me, &seed, "report-menu");
        receive(&mut gs, &dana_seed, &dana, &me, 10, "older");
        receive(&mut gs, &dana_seed, &dana, &me, 20, "newer");
        let msg = |channel: &str, ts: u64, text: &str| crate::gui::ChatMessage {
            sender_key: dana.clone(),
            sender_name: "Dana".into(),
            content: text.into(),
            timestamp_ms: ts,
            channel: channel.into(),
            ..Default::default()
        };

        open_for_message(&mut gs, &msg(&format!("dm:{dana}"), 10, "older"));
        let d = gs.reports.dialog.take().unwrap();
        assert_eq!(d.context, Some(ReportContext::Dm));
        assert_eq!(d.candidates.iter().map(|c| c.ticked).collect::<Vec<_>>(), [false, true], "the message pressed on is ticked");

        open_for_message(&mut gs, &msg("general", 30, "buy now"));
        let d = gs.reports.dialog.take().unwrap();
        assert_eq!((d.context, d.also_block), (Some(ReportContext::Post), false));
        assert_eq!(d.fixed, Some(Evidence::Post { from: dana.clone(), timestamp: 30 }));

        open_for_message(&mut gs, &msg("p2pgroup:g1", 40, "in the group"));
        let d = gs.reports.dialog.take().unwrap();
        assert_eq!(d.context, Some(ReportContext::Group));
        assert_eq!(d.fixed, Some(Evidence::GroupText { from: dana.clone(), ts: 40, text: "in the group".into() }));

        // A group message with a file: reported without it (its text holds the file's key).
        let att = dm_pq::DmAttachment { url: "u".into(), k: "k".into(), n: "n".into(), name: "a.jpg".into(), mime: "image/jpeg".into(), size: 1 };
        open_for_message(&mut gs, &msg("p2pgroup:g1", 50, &dm_pq::build_file_marker(&att)));
        let d = gs.reports.dialog.take().unwrap();
        assert_eq!((d.context, d.fixed, d.fixed_text.as_str()), (Some(ReportContext::Group), None, ""), "the file's key is not handed over");

        // A group author known only by fingerprint so far: no report, a notice instead.
        let unresolved = crate::gui::ChatMessage { sender_key: "ab12cd34ef56".into(), ..msg("p2pgroup:g1", 60, "hi") };
        open_for_message(&mut gs, &unresolved);
        assert!(gs.reports.dialog.is_none() && gs.pending_notices.len() == 1, "a fingerprint is not a key to report");

        open_for_person(&mut gs, &me);
        assert!(gs.reports.dialog.is_none(), "never a report of ourselves");
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// The help outside this server (10e-ii) opens on the country last picked on this device, else
    /// on Another country, and the country never goes in the report: the frame carries exactly
    /// 10e's fields. Seen red 2026-10-10 with `open` leaving `country` empty: "the saved pick".
    #[test]
    fn a_dialog_opens_on_the_country_last_picked_and_the_report_never_carries_it() {
        let (seed, me) = identity(55);
        let (_, dana) = identity(56);
        let mut gs = app(&me, &seed, "report-country");
        let help = outside_help::parse(crate::embedded_data::OUTSIDE_HELP_JSON.as_bytes()).expect("the shipped file");
        let listed = help.countries[0].clone();
        gs.settings.outside_help_country = listed.code.clone();
        open_for_person(&mut gs, &dana);
        assert!(gs.reports.outside_help.is_some(), "the file loads with the dialog");
        assert_eq!(gs.reports.dialog.as_ref().map(|d| d.country.as_str()), Some(listed.code.as_str()), "the saved pick");

        gs.reports.dialog = None;
        gs.settings.outside_help_country.clear();
        open_for_person(&mut gs, &dana);
        assert_eq!(gs.reports.dialog.as_ref().map(|d| d.country.as_str()), Some(outside_help::OTHER), "nothing picked: Another country");

        let d = gs.reports.dialog.as_mut().unwrap();
        d.country = listed.code.clone();
        d.reason = "child_danger".into();
        let (frame, _) = prepare_send(&gs, 4_000).expect("a frame");
        let v: serde_json::Value = serde_json::from_str(&frame).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["context", "evidence", "note", "reason", "sig", "target", "ts", "type"], "10e's fields and nothing else");
        assert!(!frame.contains(&listed.name), "the country is not in the report");
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// The relay's two answers. Seen red 2026-10-09 with the `reports` arm removed from
    /// `on_frame`: the frame fell through as unhandled and the list stayed empty.
    #[test]
    fn the_relays_answers_confirm_and_fill_the_list() {
        let mut gs = GuiState::default();
        assert!(on_frame(&mut gs, &serde_json::json!({ "type": "report_received", "id": 3 })));
        assert_eq!(gs.pending_notices, [RECEIVED_LINE]);
        gs.reports.asked_decided = true;
        assert!(on_frame(&mut gs, &serde_json::json!({ "type": "reports", "items": [ { "id": 3, "target": "t", "reason": "spam" } ] })));
        assert_eq!((gs.reports.list.len(), gs.reports.list_is_decided), (1, true));
        assert!(!on_frame(&mut gs, &serde_json::json!({ "type": "something_else" })), "other frames go on to the pump's log");
    }
}

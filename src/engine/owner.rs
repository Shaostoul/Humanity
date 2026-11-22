//! The server's owner in the desktop app (2026-11-20, docs/design/hide-and-keep.md, "How the owner
//! role is set"): the frames and the jobs, the GUI's side of src/net/owner.rs. The screen is
//! src/gui/pages/server_settings/owner.rs.
//!
//! - Claim ownership sends the pasted code (`owner_claim`); the relay's `owner_reply` (kind claim)
//!   is shown beside it, and `owner_state` says whether to show it at all.
//! - Choosing a member fetches their key card on a thread of its own and checks it on this device
//!   (signed by their own key and naming them, and the key pinned for them if one is).
//! - Hand over ownership, Name as successor and Remove successor sign the owner's change and send
//!   it (`owner_change_begin`). On the relay's `owner_change_ready` (checked against what was
//!   signed, net/owner.rs `check_ready`) a thread fetches each recipient's card again and checks it,
//!   then fetches every part's HEADER alone, opens its own slot with the owner's Kyber768 key, wraps
//!   the same part key for the recipients and sends the new header, telling the page how far it has
//!   come; the page then sends `owner_change_finish` (no key travels). Cancel sends
//!   `owner_change_cancel` and stops the thread at its next part. No body is ever fetched, and the
//!   page only ever sees counts and words.
//! - A typed `/claim` line (and an edit that is one) goes to the server alone or shows the claim
//!   help, never to a person ([`typed_claim`], net/owner.rs `typed_claim`).

use std::collections::HashMap;

use crate::gui::GuiState;
use crate::net::owner::{self, CardCheck, Change, JobMsg, PendingChange};
use humanity_core::i18n::{t, tf};

/// How long a sent change waits for the relay's answer before saying there was none.
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

/// Whether I am this server's owner: the held items list says so, or my role does.
pub(crate) fn i_am_owner(gs: &GuiState) -> bool {
    gs.reports.held.list.as_ref().is_some_and(|l| l.i_am_owner) || gs.chat_users.iter().any(|u| u.public_key == gs.profile_public_key && crate::net::roles::is_owner_role(&u.role))
}

/// This server's address as the refusals are kept under (per server and key: the sixth review's L1).
fn server_of(gs: &GuiState) -> String {
    crate::gui::pages::chat::norm_server_url(&gs.server_url)
}

/// Whether `key` is refused on this server: its typed groups did not match here, and the app has
/// not restarted since (whatever servers it switched between meanwhile).
pub(crate) fn is_refused(gs: &GuiState, key: &str) -> bool {
    gs.reports.owner.refused.contains(&server_of(gs), key)
}

/// Every key the member list shows with the owner role, once each (this device's own key too,
/// when it is the owner): the owner as this device knows it.
fn listed_owners(gs: &GuiState) -> Vec<String> {
    let mut owners: Vec<String> = gs.chat_users.iter().filter(|u| crate::net::roles::is_owner_role(&u.role)).map(|u| u.public_key.clone()).collect();
    owners.sort();
    owners.dedup();
    owners
}

/// The member list came (or changed): a check that waited for it is taken again.
pub(crate) fn on_users(gs: &mut GuiState) {
    if let Some(check) = gs.reports.owner.waiting_check.take() {
        on_check(gs, check);
    }
}

/// A person's name as the member list shows it, else the start of their key.
pub(crate) fn name_of(gs: &GuiState, key: &str) -> String {
    gs.chat_users.iter().find(|u| u.public_key == key).map(|u| u.name.clone()).unwrap_or_else(|| key.chars().take(12).collect())
}

/// The relay's owner frames: `owner_state`, `owner_reply`, `owner_change_ready`. False for any
/// other frame.
pub(crate) fn on_frame(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    if let Some(has_owner) = owner::parse_state(frame) {
        gs.reports.owner.has_owner = Some(has_owner);
        return true;
    }
    if let Some(reply) = owner::parse_reply(frame) {
        on_reply(gs, reply);
        return true;
    }
    if let Some(ready) = owner::parse_ready(frame) {
        on_ready(gs, ready);
        return true;
    }
    if let Some(check) = owner::parse_check(frame) {
        on_check(gs, check);
        return true;
    }
    false
}

/// A change waits for this device's check of the new headers (the owner review's finding 5). The
/// owner's own change, known from its signed action being signed by this device's own key
/// (net/owner.rs `is_own_change`: it survives a reload or a second window, the second review's W6),
/// is checked at once; anyone else is asked first (Server Settings, Overview: `draw_check`). A
/// re-ask after reconnecting is answered the same way, and a re-ask of a change and round this
/// device already answered is answered AGAIN while the relay has not acknowledged that answer (the
/// third review's W5: the same decline, or the check run and signed afresh), never once it has.
///
/// DRAWN ONLY ONCE ITS SIGNED ACTION VERIFIES, SIGNED BY THE OWNER THIS DEVICE KNOWS (the sixth
/// review's H1 (c), net/owner.rs `gate_check`): a frame with no signed action, a bad one, or one
/// whose signer is not the owner the member list shows, draws nothing and is logged; one that
/// comes before the member list waits for it ([`on_users`]).
fn on_check(gs: &mut GuiState, check: owner::Check) {
    match owner::gate_check(&check, &listed_owners(gs)) {
        owner::CheckGate::Draw => {}
        owner::CheckGate::Wait => {
            log::info!("owner: a check waits until the member list says who this server's owner is");
            gs.reports.owner.waiting_check = Some(check);
            return;
        }
        owner::CheckGate::Refuse(why) => {
            log::warn!("owner: a check was not drawn: {why} (change {}...)", humanity_core::text::prefix(&check.grant, 16));
            return;
        }
    }
    let mine = owner::is_own_change(&check, &gs.profile_public_key);
    if let Some(prev) = gs.reports.owner.sent_answer.clone().filter(|a| a.grant == check.grant && a.round == check.round) {
        if prev.acked {
            return;
        }
        if !prev.accepted {
            if connected(gs) {
                send(gs, &owner::decline_frame(&check.grant, &prev.why));
            }
            return;
        }
        gs.reports.owner.sent_answer = None;
        start_check(gs, check);
        return;
    }
    gs.reports.owner.check_words.clear();
    if mine {
        gs.reports.owner.status = t("owner.check_own").to_string();
        start_check(gs, check);
    } else {
        gs.reports.owner.check = Some(check);
    }
}

/// Check and accept (the prompt's button, or the owner's own change): on a thread of its own, check
/// the owner's signed action, then fetch every new header and open this device's own slot of it
/// (never a body); `poll` signs and sends the acceptance, or the decline with why.
pub(crate) fn start_check(gs: &mut GuiState, check: owner::Check) {
    let Some(seed) = gs.private_key_bytes.clone() else {
        gs.reports.owner.check_words = t("held.need_key").to_string();
        return;
    };
    let me = gs.profile_public_key.clone();
    let base = crate::gui::widgets::image_cache::http_base(&gs.server_url);
    let (tx, rx) = std::sync::mpsc::channel();
    gs.reports.owner.check_job = Some(rx);
    gs.reports.owner.check = Some(check.clone());
    gs.reports.owner.check_words = tf("owner.checking", &[("done", &0), ("items", &check.items.len())]);
    std::thread::spawn(move || {
        use humanity_protocol::pq_crypto::{derive_kyber_seed, KyberKeypair};
        let result = KyberKeypair::from_seed(&derive_kyber_seed(&seed)).map_err(|e| e.to_string()).and_then(|kyber| {
            let caller = crate::net::held::Caller::new(&me, &seed);
            owner::check_all(&kyber, &me, &check, |id, part| owner::fetch_rewrapped(&base, id, part, &check.grant, &caller), |n| {
                let _ = tx.send(owner::CheckMsg::Progress(n));
            })
        });
        let _ = tx.send(owner::CheckMsg::Done(result));
    });
}

/// Decline the change the prompt shows: it is rolled back, everything kept as it was.
pub(crate) fn decline_check(gs: &mut GuiState) {
    let Some(check) = gs.reports.owner.check.take() else { return };
    if connected(gs) {
        send(gs, &owner::decline_frame(&check.grant, ""));
        gs.reports.owner.sent_answer = Some(sent(&check, false, "", false));
    }
    gs.reports.owner.check_job = None;
    gs.reports.owner.check_words = t("owner.declined").to_string();
}

/// The check thread's progress and end: sign and send the acceptance, or the decline with why.
fn poll_check(gs: &mut GuiState) {
    let mut done = None;
    if let Some(rx) = gs.reports.owner.check_job.as_ref() {
        for msg in rx.try_iter() {
            match msg {
                owner::CheckMsg::Progress(n) => {
                    let items = gs.reports.owner.check.as_ref().map(|c| c.items.len()).unwrap_or(0);
                    gs.reports.owner.check_words = tf("owner.checking", &[("done", &n), ("items", &items)]);
                }
                owner::CheckMsg::Done(r) => done = Some(r),
            }
        }
    }
    let Some(result) = done else { return };
    gs.reports.owner.check_job = None;
    let Some(check) = gs.reports.owner.check.take() else { return };
    let accepted = result.and_then(|digest| {
        let seed = gs.private_key_bytes.as_ref().ok_or_else(|| t("held.need_key").to_string())?;
        let did = gs.dm_store.as_ref().and_then(|s| s.pass_server()).ok_or_else(|| t("held.no_server_did").to_string())?;
        owner::sign_accept(seed, did, &check.grant, &digest, now_ms() as u64).map(|signed| owner::accept_frame(&signed))
    });
    if !connected(gs) {
        // Not answered: the check stays, and the relay asks again when this device reconnects (the
        // second review, W6: a dropped socket used to mark it answered).
        gs.reports.owner.check = Some(check);
        gs.reports.owner.check_words = t("held.not_connected").to_string();
        return;
    }
    let mine = owner::is_own_change(&check, &gs.profile_public_key);
    match accepted {
        Ok(frame) => {
            send(gs, &frame);
            gs.reports.owner.sent_answer = Some(sent(&check, true, "", mine));
            gs.reports.owner.check_words = t("owner.check_sent").to_string();
        }
        Err(why) => {
            send(gs, &owner::decline_frame(&check.grant, &why));
            gs.reports.owner.sent_answer = Some(sent(&check, false, &why, mine));
            gs.reports.owner.check_words = tf("owner.check_failed", &[("why", &why.trim_end().trim_end_matches('.'))]);
        }
    }
}

/// The answer just sent to `check`, not yet acknowledged.
fn sent(check: &owner::Check, accepted: bool, why: &str, mine: bool) -> owner::SentAnswer {
    owner::SentAnswer { grant: check.grant.clone(), round: check.round.clone(), accepted, why: why.to_string(), mine, acked: false }
}

fn on_reply(gs: &mut GuiState, reply: owner::Reply) {
    if reply.kind == "accept" {
        // The relay's answer to this device's acceptance or decline: only now is it answered (W5).
        let o = &mut gs.reports.owner;
        if let Some(a) = o.sent_answer.as_mut() {
            a.acked = true;
        }
        // No text the relay wrote is drawn in a prompt (the sixth review's H1 (c)): the prompt says
        // in the app's own words whether the server took the answer, and the server's own words go
        // to the chat under the sender "From this server".
        o.check_words = t(if reply.ok { "owner.check_answer_ok" } else { "owner.check_answer_refused" }).to_string();
        if !reply.message.is_empty() {
            crate::engine::friend_code::server_line(gs, &reply.message);
        }
        return;
    }
    let o = &mut gs.reports.owner;
    // The owner's own check, when it was the last acceptance, is acknowledged by the change's outcome.
    if reply.kind != "claim" {
        if let Some(a) = o.sent_answer.as_mut().filter(|a| a.mine) {
            a.acked = true;
        }
    }
    if reply.kind == "claim" {
        o.claim_words = reply.message;
        o.claim_ok = reply.ok;
        if reply.ok {
            o.code.clear();
            o.has_owner = Some(true);
            gs.reports.held.requested = false; // the owner's Held copies, asked for again
        }
        return;
    }
    // A handover or a change of successor: made (the relay sends a fresh Held copies list), or
    // refused or cancelled with nothing changed. Either way nothing is in flight any more, and a
    // job still running stops at its next part (its messages have nowhere to go).
    o.status = reply.message;
    o.pending = None;
    o.job = None;
    o.progress = None;
    if reply.ok {
        o.picked = None;
        o.card = None;
        o.card_job = None;
        o.confirm.clear();
        o.reason.clear();
        o.search.clear();
    }
}

/// The relay checked the change: check its answer against what this device signed, then start
/// the re-seal on a thread of its own.
fn on_ready(gs: &mut GuiState, ready: owner::Ready) {
    let me = gs.profile_public_key.clone();
    let Some(p) = gs.reports.owner.pending.clone().filter(|p| !p.started) else { return };
    if let Err(why) = owner::check_ready(&ready, p.change, &me, &p.target, &p.grant) {
        return stop(gs, &why);
    }
    let Some(seed) = gs.private_key_bytes.clone() else { return stop(gs, t("held.need_key")) };
    if let Some(p) = gs.reports.owner.pending.as_mut() {
        p.started = true;
    }
    // Each recipient's name for the words, and the key this device pinned for them, if any.
    let names: HashMap<String, String> = ready.recipients.iter().map(|r| (r.key.clone(), name_of(gs, &r.key))).collect();
    crate::engine::key_cards::ensure_pins(gs);
    let pins: HashMap<String, String> = ready
        .recipients
        .iter()
        .filter_map(|r| gs.key_cards.pins.as_ref().and_then(|p| p.pinned(&r.key)).map(|pin| (r.key.clone(), pin.kyber.clone())))
        .collect();
    let base = crate::gui::widgets::image_cache::http_base(&gs.server_url);
    let (tx, rx) = std::sync::mpsc::channel();
    gs.reports.owner.job = Some(rx);
    gs.reports.owner.status = t("owner.waiting").to_string();
    std::thread::spawn(move || {
        let result = run(&seed, &base, &me, &ready, &names, &pins, &tx);
        let _ = tx.send(JobMsg::Done(result));
    });
}

/// The re-wrap itself (on its own thread): every recipient's card checked here, then every part's
/// header fetched, re-wrapped and sent back. The words why it stopped, if it did.
fn run(
    seed: &[u8],
    base: &str,
    me: &str,
    ready: &owner::Ready,
    names: &HashMap<String, String>,
    pins: &HashMap<String, String>,
    tx: &std::sync::mpsc::Sender<JobMsg>,
) -> Result<(), String> {
    use humanity_protocol::pq_crypto::{derive_kyber_seed, KyberKeypair};
    let kyber = KyberKeypair::from_seed(&derive_kyber_seed(seed)).map_err(|e| e.to_string())?;
    let name = |k: &str| names.get(k).cloned().unwrap_or_else(|| k.chars().take(12).collect());
    let keys = owner::recipient_keys(me, &kyber.public_key(), &ready.recipients, |k| owner::fetch_card(base, k), |k| pins.get(k).cloned(), name)?;
    // Every request signed by this app's own key (the owner review's finding 1).
    let caller = crate::net::held::Caller::new(me, seed);
    // The page stopped listening (Cancel, or the relay's answer): stop at the next part.
    let stopped = std::cell::Cell::new(false);
    owner::rewrap_all(
        &kyber,
        &keys,
        &ready.items,
        |id, part| owner::fetch_header(base, id, part, &ready.grant, &caller),
        |id, part, header| if stopped.get() { Err(t("owner.cancelled").to_string()) } else { owner::upload_header(base, id, part, &ready.grant, &caller, header) },
        |progress| {
            if tx.send(JobMsg::Progress(progress)).is_err() {
                stopped.set(true);
            }
        },
    )
}

/// Stop the change in flight with `why`, telling the relay so it keeps everything as it was.
fn stop(gs: &mut GuiState, why: &str) {
    if connected(gs) {
        send(gs, &owner::cancel_frame());
    }
    let o = &mut gs.reports.owner;
    // The words end with "Nothing was changed.", so a sentence's own full stop is dropped first.
    o.status = tf("owner.failed", &[("why", &why.trim_end().trim_end_matches('.'))]);
    o.pending = None;
    o.job = None;
    o.progress = None;
}

/// A composer line or an edit that starts with `/claim` (net/owner.rs `typed_claim`): a well-formed
/// one goes to the server alone, anything else shows the claim help in the chat; never a message to
/// a person or a group. Whether the line was one (and so must not be sent on).
pub(crate) fn typed_claim(gs: &mut GuiState, line: &str) -> bool {
    match owner::typed_claim(line) {
        Some(owner::TypedClaim::Code(code)) => {
            if let Some(c) = gs.ws_client.as_ref() {
                c.send(&owner::typed_claim_frame(&code));
            }
            true
        }
        Some(owner::TypedClaim::Help) => {
            crate::engine::friend_code::system_line(gs, t("owner.claim_help"));
            true
        }
        None => false,
    }
}

/// Claim ownership with the pasted code.
pub(crate) fn claim(gs: &mut GuiState) {
    if !connected(gs) {
        gs.reports.owner.claim_words = t("held.not_connected").to_string();
        return;
    }
    let code = gs.reports.owner.code.trim().to_string();
    if code.is_empty() {
        gs.reports.owner.claim_words = t("owner.claim_code_needed").to_string();
        return;
    }
    send(gs, &owner::claim_frame(&code));
    gs.reports.owner.claim_words = t("owner.claim_sent").to_string();
    gs.reports.owner.claim_ok = false;
}

/// Choose `key` (named `name`) and check their key card on this device, on a thread of its own.
pub(crate) fn pick(gs: &mut GuiState, key: &str, name: &str) {
    let o = &mut gs.reports.owner;
    o.picked = Some((key.to_string(), name.to_string()));
    o.card = None;
    o.confirm.clear();
    o.fp_confirm.clear();
    crate::engine::key_cards::ensure_pins(gs);
    // Four groups are asked, at positions drawn now. Every time, for every key (the fourth review's
    // finding 1, 2026-11-21): a friend given a pass is no exemption (a mutual follow is one "Follow
    // back" on a name the relay listed, and the passes then go by themselves), nor a pinned card
    // (the third review's W1), nor a key confirmed before. A key whose groups did not match this
    // session gets no fresh draw (the fifth review's H1 (c)): the picker shows the warning instead.
    if is_refused(gs, key) {
        gs.reports.owner.asked = None;
        gs.reports.owner.fp_ok = None;
        gs.reports.owner.status = mismatch_words(name);
    } else {
        ask_groups(gs, key);
    }
    let pinned = gs.key_cards.pins.as_ref().and_then(|p| p.changed(key).or(p.pinned(key))).map(|pin| pin.kyber.clone());
    if gs.key_cards.pins.as_ref().is_some_and(|p| p.changed(key).is_some()) {
        gs.reports.owner.card = Some(CardCheck::Changed);
        gs.reports.owner.card_job = None;
        return;
    }
    let base = crate::gui::widgets::image_cache::http_base(&gs.server_url);
    let (tx, rx) = std::sync::mpsc::channel();
    gs.reports.owner.card_job = Some((key.to_string(), rx));
    let key = key.to_string();
    std::thread::spawn(move || {
        let check = match owner::fetch_card(&base, &key) {
            Ok(card) => owner::against_pin(owner::check_card(card.as_ref(), &key), pinned.as_deref()),
            Err(why) => CardCheck::Failed(why),
        };
        let _ = tx.send(check);
    });
}

/// Draw the four groups to ask for `key` afresh (core `key_card::confirm_positions`, uniformly from
/// all 15 sets) and clear what was typed: on every pick, and after every change is signed, so the
/// next handover or naming asks again. Never for a refused key (the caller checks).
fn ask_groups(gs: &mut GuiState, key: &str) {
    debug_assert!(!is_refused(gs, key), "a refused key gets no fresh draw");
    let o = &mut gs.reports.owner;
    o.asked = Some(humanity_protocol::key_card::confirm_positions());
    o.fp_confirm.clear();
    o.fp_ok = None;
}

/// The words asking for the groups `asked`, for the member `name` (`owner.fp_needed`).
pub(crate) fn fp_needed(name: &str, asked: Option<humanity_protocol::key_card::Asked>) -> String {
    let [a, b, c, d] = asked.map(humanity_protocol::key_card::asked_words).unwrap_or_default();
    tf("owner.fp_needed", &[("name", &name), ("a", &a), ("b", &b), ("c", &c), ("d", &d)])
}

/// The warning for a key whose typed groups did not match, and that it is refused on this server
/// until the app restarts.
pub(crate) fn mismatch_words(name: &str) -> String {
    let name = owner::shown_name(name);
    format!("{} {}", tf("owner.fp_mismatch", &[("name", &name)]), tf("owner.fp_blocked", &[("name", &name)]))
}

/// Judge the groups typed for the chosen member (Check these groups, and every change before it is
/// signed): None when they match; else the words why not. A full set of four that does not match
/// refuses that key on this server until the app restarts (per server and key, kept across a
/// switch: the sixth review's L1), logged, with the warning in words (the fifth review's H1 (c)): a
/// wrong set never reads like an empty one, and no fresh draw follows.
pub(crate) fn check_groups(gs: &mut GuiState) -> Option<String> {
    use humanity_protocol::key_card::Typed;
    let Some((key, name)) = gs.reports.owner.picked.clone() else { return Some(t("owner.need_pick").to_string()) };
    let server = server_of(gs);
    let o = &mut gs.reports.owner;
    if o.refused.contains(&server, &key) {
        return Some(mismatch_words(&name));
    }
    match owner::judge(&key, o.asked, &o.fp_confirm) {
        Typed::Matched => {
            o.fp_ok = Some(key);
            None
        }
        Typed::Empty => Some(fp_needed(&owner::shown_name(&name), o.asked)),
        Typed::Short => Some(tf("owner.fp_short", &[("name", &owner::shown_name(&name))])),
        Typed::Mismatched => {
            log::warn!(
                "owner: the fingerprint groups typed for the key listed as {:?} ({}...) do not match it; that key is refused on {server} until the app restarts",
                owner::shown_name(&name),
                humanity_core::text::prefix(&key, 16)
            );
            o.refused.insert(&server, &key);
            o.asked = None;
            o.fp_ok = None;
            o.fp_confirm.clear();
            Some(mismatch_words(&name))
        }
    }
}

/// Check these groups: judge them now and show the answer (the whole fingerprint once they match).
pub(crate) fn check_groups_now(gs: &mut GuiState) {
    gs.reports.owner.status = match check_groups(gs) {
        None => String::new(),
        Some(why) => why,
    };
}

/// Why the change can not be signed yet, in words; None when it can. `change` names the chosen
/// member except Remove successor, which names the successor.
fn refusal(gs: &mut GuiState, change: Change) -> Option<String> {
    let server = server_of(gs);
    let o = &gs.reports.owner;
    if !connected(gs) {
        return Some(t("held.not_connected").to_string());
    }
    if o.pending.is_some() {
        return Some(t("owner.waiting").to_string());
    }
    if change != Change::SuccessorRemove {
        let Some((key, name)) = o.picked.as_ref() else { return Some(t("owner.need_pick").to_string()) };
        // A key whose groups did not match this session is refused with that warning first,
        // whatever else is still being checked (the fifth review's H1 (c)).
        if o.refused.contains(&server, key) {
            return Some(mismatch_words(name));
        }
        match o.card.as_ref() {
            Some(CardCheck::Good { .. }) => {}
            Some(other) => return owner::card_words(other, name),
            None => return Some(tf("owner.need_card", &[("name", name)])),
        }
        // A name an honest relay could not list is no name to type (the third review's M2).
        if !humanity_protocol::key_card::honest_name(name) {
            return Some(t("owner.name_refused_handover").to_string());
        }
        if o.confirm.trim() != name.as_str() {
            return Some(tf("owner.confirm_mismatch", &[("name", name)]));
        }
        // The person told apart by key, not the relay's name (the second review, W1), from what
        // they read off their own screen (the fifth review's H1).
        if let Some(why) = check_groups(gs) {
            return Some(why);
        }
    }
    let o = &gs.reports.owner;
    if o.reason.trim().is_empty() {
        return Some(t("owner.reason_needed").to_string());
    }
    None
}

/// Sign `change` and send it; the page shows the words why not.
pub(crate) fn begin(gs: &mut GuiState, change: Change) {
    if let Some(why) = refusal(gs, change) {
        gs.reports.owner.status = why;
        return;
    }
    let (target, target_name) = if change == Change::SuccessorRemove {
        match gs.reports.held.list.as_ref().and_then(|l| l.successor.clone()) {
            Some(s) => (s.key, s.name),
            None => {
                gs.reports.owner.status = t("owner.no_successor_to_remove").to_string();
                return;
            }
        }
    } else {
        gs.reports.owner.picked.clone().unwrap_or_default()
    };
    let Some(seed) = gs.private_key_bytes.as_ref() else {
        gs.reports.owner.status = t("held.need_key").to_string();
        return;
    };
    let Some(did) = gs.dm_store.as_ref().and_then(|s| s.pass_server()) else {
        gs.reports.owner.status = t("held.no_server_did").to_string();
        return;
    };
    let signed = match owner::sign(seed, did, change, &target, &gs.reports.owner.reason, now_ms() as u64) {
        Ok(s) => s,
        Err(e) => {
            gs.reports.owner.status = tf("held.sign_failed", &[("why", &e)]);
            return;
        }
    };
    send(gs, &owner::begin_frame(&signed));
    // The groups typed served this change alone: the next handover or naming asks four afresh,
    // even for the same member (the fourth review's finding 1).
    if change != Change::SuccessorRemove {
        ask_groups(gs, &target);
    }
    let o = &mut gs.reports.owner;
    o.status = t("owner.waiting").to_string();
    o.progress = None;
    o.pending = Some(PendingChange { change, target, target_name, grant: signed.object_id, sent_at_ms: now_ms(), started: false });
}

/// Cancel the change in flight: the relay keeps everything as it was, and the job stops.
pub(crate) fn cancel(gs: &mut GuiState) {
    if connected(gs) {
        send(gs, &owner::cancel_frame());
    }
    let o = &mut gs.reports.owner;
    o.pending = None;
    o.job = None;
    o.progress = None;
    o.status = t("owner.cancelled").to_string();
}

/// Every frame the panel is drawn: a card check's answer, the job's progress and its end (the
/// finish sent), or an answer that never came.
pub(crate) fn poll(gs: &mut GuiState) {
    poll_check(gs);
    if let Some((key, rx)) = gs.reports.owner.card_job.as_ref() {
        match rx.try_recv() {
            Ok(check) => {
                if gs.reports.owner.picked.as_ref().is_some_and(|(k, _)| k == key) {
                    gs.reports.owner.card = Some(check);
                }
                gs.reports.owner.card_job = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => gs.reports.owner.card_job = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    let mut done = None;
    if let Some(rx) = gs.reports.owner.job.as_ref() {
        for msg in rx.try_iter() {
            match msg {
                JobMsg::Progress(p) => {
                    gs.reports.owner.progress = Some(p);
                    gs.reports.owner.status = tf("owner.sealing", &[("items_done", &p.items_done), ("items", &p.items), ("parts_done", &p.parts_done), ("parts", &p.parts)]);
                }
                JobMsg::Done(result) => done = Some(result),
            }
        }
    }
    match done {
        Some(Ok(())) => {
            gs.reports.owner.job = None;
            let grant = gs.reports.owner.pending.as_ref().map(|p| p.grant.clone()).unwrap_or_default();
            if connected(gs) {
                send(gs, &owner::finish_frame(&grant));
                gs.reports.owner.status = t("owner.finishing").to_string();
            } else {
                stop(gs, t("held.not_connected"));
            }
        }
        Some(Err(why)) => stop(gs, &why),
        None => {}
    }
    if gs.reports.owner.pending.as_ref().is_some_and(|p| !p.started && now_ms() - p.sent_at_ms > ANSWER_WAIT_MS) {
        gs.reports.owner.pending = None;
        gs.reports.owner.status = t("held.no_answer").to_string();
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
pub(crate) mod tests;

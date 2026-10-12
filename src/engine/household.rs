//! Household permits travel between devices (ship homes increment 5b, lane 1A, 2026-10-12;
//! docs/design/ship-homes-increment-5b-plan.md sections 3.1 to 3.4 and 3.6, with the
//! orchestrator's amendments in its section 8, which win where they differ). The rules with no
//! socket and no app state live in src/net/household.rs; what is stored is the DM store's
//! (`permits_held`, `permits_given`); this file is the part that touches the app and the socket.
//!
//! GIVING ([`give`], the Household card's Give). A permit is minted with a fresh serial for the
//! plot this session's welcome gave me, sealed as a DM note (`[[hum:permit:v1]]` and the permit's
//! JSON, so the DM's own signature covers it) and sent to the friend with the pass THEY gave me
//! (so it rides past their "who can reach me" gate) and a random `ref`. It counts as given only
//! once the server answers `dm_put_ok` (10l, [`on_answer`]); only then does the self-copy go to my
//! own mailbox, so my other devices never list a permit the friend never got. A refusal or 30
//! seconds of silence says so on the row ([`expire`]), and no self-copy goes. These sends are
//! held in `GuiState::household`, never in `pending_puts`, so they never hold up the pass sweep.
//!
//! ONE PERMIT PER FRIEND AND PLOT. When the server took a new permit (a renewal, or Give again
//! after no answer), every older permit to that friend for that plot that might still stand is
//! taken back (amendment A1). [`renew`] is Give again for the same length, counted from today.
//!
//! TAKING BACK ([`take_back`], the card's Take back; and Block). Each serial goes to the relay as
//! `cert_revoke {serial}` through the friendship passes' own resend list (the DM store's
//! `withdrawals_pending`, resent on every member list until the relay answers `cert_revoked`), and
//! the row says so at once. My other devices learn from a note to myself (the self-copy pattern):
//! `[[hum:permit:v1]]{"taken_back":[...]}`, which starts with the permit marker so the website
//! swallows it like any permit note. Block takes back every permit given to the blocked person in
//! the same step that withdraws their passes ([`take_back_from_blocked`], run first by engine/dm.rs
//! `send_pending_withdrawals`, which every Block path calls right after it withdraws the passes).
//!
//! RECEIVING ([`ingest`], called by engine/dm.rs `ingest_dm` after block, choice and the controls,
//! before contact requests, the reach screen and `store.insert`; [`screens_dm_parked`] is the same
//! for a parked server's store). Every permit note is consumed, never stored or shown:
//! - mine, to a friend, echoed from my other device: listed as given, after the same check;
//! - a note to myself from my other device saying what it took back: those rows say so here;
//! - to me from a friend: kept only when `net::household::judge_arrival` says it is theirs, for me,
//!   on this server and checks out; a notice says so;
//! - from anyone else: dropped with one log line naming 12 characters of the key.
//!
//! The relay keeps no permit. Whether a held permit works now (its giver still holds the plot, and
//! has not taken it back) is the relay's `game_permit_check`, asked by the game's glue (lane 2A,
//! the second half of this file):
//! - [`session_began`] / [`session_ended`]: a welcome names my plot and every held permit is
//!   asked about afresh; leaving forgets what the relay said, never a send on its way;
//! - [`tick`] / [`on_status`]: the checks and their answers (engine/shared_build.rs routes both);
//! - [`open_for`] / [`apply_open`]: the plots this game may walk into (`ShipStructure::open`);
//! - [`plot_permit`]: what the build gate reads (engine/shared_build.rs `gate`);
//! - [`probe_json`]: what the dev IPC reports (engine/ipc.rs).
//!
//! The protected setup (net/protected.rs `permit_allowed`) refuses Give and Renew outright while it
//! is on: a permit lets someone into the home. Take back is never refused.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::gui::GuiState;
use crate::net::dm_pq::{DmInner, CTL_PERMIT};
use crate::net::dm_store::DmStore;
use crate::net::household::{self as hh, Arrival, GivenPermit, GivenState, HeldPermit, Kept, PermitPut, Verdict};
use crate::net::put_answers::{Answer, ANSWER_WAIT};
use crate::relay::storage::plot_owner_id;
use crate::systems::construction::shared::{Permit, Why, PERMIT_MAX_DAYS};

/// Seconds in a day.
const DAY: u64 = 86_400;

/// At most this many taken-back rows ride in one note to myself (each names the friend's key, about
/// 4 KB, so a note stays far under the relay's envelope limit).
const ROWS_PER_NOTE: usize = 4;

// ── The words ───────────────────────────────────────────────────────────────────────────────────
// Plain words only: never certificate, signature, key, token, revoke or verify (plan 3.9).

/// Give and Take back while the identity is locked.
pub const LOCKED: &str = "Unlock your identity to give or take back household permits.";
/// Give while not connected.
pub const NOT_CONNECTED: &str = "Connect to a server to give a household permit.";
/// Give before the server said who it is (its `identify_challenge`; not seen in practice).
pub const NO_SERVER_YET: &str = "The server has not said who it is yet; try again in a moment.";
/// Give before this session's welcome named my plot, or as a guest.
pub const NO_PLOT: &str =
    "Step into this server's shared world first: the server says which plot is yours when you arrive.";
/// Give for a length outside 1 to 90 days.
pub const BAD_LENGTH: &str = "A household permit lasts 1 to 90 days.";
/// The permit could not be made or sealed (no randomness, a malformed plot id).
pub const NOT_MADE: &str = "The permit could not be made; try again.";
/// Give and Renew while the protected setup is on (net/protected.rs `permit_allowed`): refused
/// outright, the PIN does not let it through.
pub const PROTECTED: &str =
    "The protected setup is on, so this device does not give household permits: a permit lets someone into your home. You can still take one back.";

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn now_s() -> u64 {
    now_ms() / 1000
}

/// The first 12 characters of a key, for a log line (never the whole key).
fn key12(key: &str) -> &str {
    &key[..12.min(key.len())]
}

/// How many days a permit given at `at_ms` and running out at `expiry` was given for: what Renew
/// gives again (plan 3.3), 1 to 90.
pub fn length_days(g: &GivenPermit) -> u64 {
    let secs = g.expiry.saturating_sub(g.at / 1000);
    ((secs + DAY / 2) / DAY).clamp(1, PERMIT_MAX_DAYS)
}

/// Whole days left before `expiry` at `now_s`, counting a part day as one ("29 days left").
pub fn days_left(expiry: u64, now_s: u64) -> u64 {
    expiry.saturating_sub(now_s).div_ceil(DAY)
}

/// Save `store` after dropping permits that ended more than the kept days ago (plan 3.2: "on every
/// save that touches permits"). The kept days are data (data/ship/household.ron).
fn save_pruned(store: &mut DmStore) {
    let kept = hh::load_lengths(&crate::data_dir()).ended_kept_days;
    store.prune_permits(now_s(), kept);
    store.save();
}

// ── The note to myself that says what I took back ──────────────────────────────────────────────

/// One permit I took back, as the note to my other devices names it: enough for a device that
/// never saw the permit to list it as taken back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TakenBackRow {
    to: String,
    plot: String,
    serial: String,
    expiry: u64,
    at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TakenBackNote {
    taken_back: Vec<TakenBackRow>,
}

/// The text of the note to myself saying I took back `rows`: the permit marker, then
/// `{"taken_back":[{"to","plot","serial","expiry","at"},...]}`. It starts with the permit marker so
/// the website, which swallows every text that does, never shows it.
pub fn take_back_note_text(rows: &[GivenPermit]) -> String {
    let note = TakenBackNote {
        taken_back: rows
            .iter()
            .map(|g| TakenBackRow { to: g.to.clone(), plot: g.plot.clone(), serial: g.serial.clone(), expiry: g.expiry, at: g.at })
            .collect(),
    };
    format!("{CTL_PERMIT}{}", serde_json::to_string(&note).unwrap_or_default())
}

/// The rows a take-back note names, or None when `text` is not one (a permit, or not a note).
fn parse_take_back(text: &str) -> Option<Vec<TakenBackRow>> {
    let body = text.strip_prefix(CTL_PERMIT)?;
    if body.len() > ROWS_PER_NOTE * 2 * hh::NOTE_BODY_MAX_LEN {
        return None;
    }
    let note: TakenBackNote = serde_json::from_str(body).ok()?;
    let fits = |r: &TakenBackRow| {
        !r.to.is_empty()
            && r.to.len() <= hh::ISSUER_MAX_LEN
            && r.to.bytes().all(|b| b.is_ascii_hexdigit())
            && !r.plot.is_empty()
            && r.plot.len() <= crate::relay::core::pq_crypto::PLOT_PERMIT_FIELD_MAX_LEN
            && crate::relay::core::pq_crypto::friend_cert_serial_ok(&r.serial)
    };
    (note.taken_back.iter().all(fits) && note.taken_back.len() <= ROWS_PER_NOTE).then_some(note.taken_back)
}

// ── Receiving ──────────────────────────────────────────────────────────────────────────────────

/// Why a permit note was dropped (logged, never shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// It does not read as a permit or a take-back note.
    Unreadable,
    /// Addressed to someone else (its signed `to` is not me).
    NotToMe,
    /// From someone who is not a friend (a mutual follow) on this server.
    NotAFriend,
    /// A friend's permit that is not kept, for this reason.
    Refused(Arrival),
    /// My own permit, echoed, that does not check out as one I gave the friend it went to.
    NotMine(Arrival),
}

/// What a permit note did to a store.
#[derive(Debug, Clone, PartialEq)]
pub enum Arrived {
    /// A friend's permit, checked and kept (`Kept::New`, `Kept::Replaced`), or one already held.
    Held(HeldPermit, Kept),
    /// My own permit to `to`, echoed from my other device, listed as given; `new` when the row is
    /// new here; `blocked` when I have since blocked them, so it was taken back at once.
    Given { to: String, plot: String, new: bool, blocked: bool },
    /// A note from my other device saying what it took back: the rows it named.
    TakenBack(usize),
    /// Dropped.
    Dropped(Dropped),
}

/// Apply one permit note (`hh::is_note(&inner.text)`) to `store`, the store of the server it came
/// by: the rules of plan 3.1 and amendment A1. `me` is my key, `now_s` my clock, `to_blocked`
/// whether I blocked `inner.to` (for my own echo: the block wins, as for passes). The caller saves.
pub fn take_note(store: &mut DmStore, me: &str, inner: &DmInner, now_s: u64, to_blocked: bool) -> Arrived {
    let from_me = inner.from == me;
    if from_me && inner.to == me {
        // A note to myself, so only my own devices can have written it.
        let Some(rows) = parse_take_back(&inner.text) else { return Arrived::Dropped(Dropped::Unreadable) };
        for r in &rows {
            let row = GivenPermit { to: r.to.clone(), plot: r.plot.clone(), serial: r.serial.clone(), expiry: r.expiry, at: r.at, state: GivenState::TakenBack };
            store.record_given(row, now_s);
        }
        return Arrived::TakenBack(rows.len());
    }
    let Some(p) = hh::parse_note(&inner.text) else { return Arrived::Dropped(Dropped::Unreadable) };
    if from_me {
        // My own permit to the friend `inner.to`, from my other device: the same check as a
        // friend's, against the server this store holds, the friend it went to and the time it
        // was given (the DM's signed time), so a forged one is never listed as given.
        if let Err(why) = hh::judge_arrival(&p, me, store.pass_server(), &plot_owner_id(&inner.to), inner.ts / 1000) {
            return Arrived::Dropped(Dropped::NotMine(why));
        }
        let row = GivenPermit { to: inner.to.clone(), plot: p.plot.clone(), serial: p.serial.clone(), expiry: p.expiry, at: inner.ts, state: GivenState::Taken };
        let new = store.record_given(row, now_s);
        if to_blocked {
            store.take_back_permits(&inner.to, |_| false);
        }
        return Arrived::Given { to: inner.to.clone(), plot: p.plot, new, blocked: to_blocked };
    }
    if inner.to != me {
        return Arrived::Dropped(Dropped::NotToMe);
    }
    if !store.is_friend(&inner.from) {
        return Arrived::Dropped(Dropped::NotAFriend);
    }
    if let Err(why) = hh::judge_arrival(&p, &inner.from, store.pass_server(), &plot_owner_id(me), now_s) {
        return Arrived::Dropped(Dropped::Refused(why));
    }
    let held = HeldPermit { from: inner.from.clone(), permit: p, at: inner.ts };
    let kept = store.keep_permit(held.clone());
    Arrived::Held(held, kept)
}

fn log_dropped(inner: &DmInner, why: Dropped) {
    log::info!("Household: a permit note from {}… was dropped ({why:?}); nothing kept or shown", key12(&inner.from));
}

/// A verified DM (engine/dm.rs `ingest_dm`, after block, choice and the controls): a permit note is
/// acted on here and never stored, listed or shown. True when it was one (every case, a dropped
/// one too, so a malformed note is never shown as a message).
pub fn ingest(gs: &mut GuiState, inner: &DmInner) -> bool {
    if !hh::is_note(&inner.text) {
        return false;
    }
    let me = gs.profile_public_key.clone();
    let to_blocked = crate::engine::block::is_blocked(gs, &inner.to);
    let Some(store) = gs.dm_store.as_mut() else { return true };
    let arrived = take_note(store, &me, inner, now_s(), to_blocked);
    match &arrived {
        Arrived::Dropped(why) => {
            log_dropped(inner, *why);
            return true;
        }
        Arrived::Held(_, Kept::Unchanged) | Arrived::TakenBack(0) => return true,
        _ => save_pruned(store),
    }
    match arrived {
        Arrived::Held(h, _) => {
            let name = crate::engine::dm::dm_display_name(gs, &h.from);
            gs.pending_notices.push(format!(
                "{name} gave you a household permit for plot {} until {}. Home > Household lists it.",
                h.permit.plot,
                hh::date_words(h.permit.expiry)
            ));
            // Ask the server about it at once when in the shared world (plan 3.4, lane 2A's checks).
            gs.household.next_check_at = 0.0;
        }
        Arrived::Given { to, blocked: true, .. } => {
            log::info!("Household: my other device gave {}… a permit after I blocked them; taken back", key12(&to));
            gs.household.on_their_way.retain(|p| p.peer != to);
            crate::engine::dm::send_pending_withdrawals(gs);
        }
        _ => {}
    }
    true
}

/// [`ingest`] on a parked server's store (engine/bg_connections.rs `bg_screen`): the permit is kept
/// in, listed as given in, or taken back in THAT server's store, with no notice and nothing sent
/// (a withdrawal it queues goes when that server is the active one). True when the DM was a
/// permit note (taken; the caller saves and stores nothing as a message).
pub fn screens_dm_parked(gs: &mut GuiState, store: &mut DmStore, inner: &DmInner) -> bool {
    if !hh::is_note(&inner.text) {
        return false;
    }
    let to_blocked = crate::engine::block::is_blocked(gs, &inner.to);
    if let Arrived::Dropped(why) = take_note(store, &gs.profile_public_key.clone(), inner, now_s(), to_blocked) {
        log_dropped(inner, why);
    }
    true
}

// ── Giving ─────────────────────────────────────────────────────────────────────────────────────

/// Give `peer` a household permit for my plot on this server for `days` days (the card's Give).
/// Err with the sentence the card shows when it cannot go now (plan 3.3). Ok once it is on its way:
/// it counts as given only on the server's `dm_put_ok` ([`on_answer`]).
pub fn give(gs: &mut GuiState, peer: &str, days: u64) -> Result<(), String> {
    if !gs.protected.setup.permit_allowed() {
        return Err(PROTECTED.into());
    }
    let expiry = now_s() + days.saturating_mul(DAY);
    give_until(gs, peer, days, expiry)
}

/// [`give`] with its end date worked out: `days` is the length (checked and remembered for Renew),
/// `expiry` when it runs out.
fn give_until(gs: &mut GuiState, peer: &str, days: u64, expiry: u64) -> Result<(), String> {
    let name = crate::engine::dm::dm_display_name(gs, peer);
    if gs.private_key_bytes.is_none() || gs.profile_public_key.is_empty() {
        return Err(LOCKED.into());
    }
    if !gs.ws_client.as_ref().is_some_and(|c| c.is_connected()) || !crate::engine::dm::ensure_dm_store(gs) {
        return Err(NOT_CONNECTED.into());
    }
    let Some(server) = gs.dm_store.as_ref().and_then(|s| s.pass_server()).map(str::to_string) else { return Err(NO_SERVER_YET.into()) };
    let Some(plot) = gs.household.my_plot.clone() else { return Err(NO_PLOT.into()) };
    if crate::engine::block::is_blocked(gs, peer) {
        return Err(format!("You blocked {name}. Unblock them first, in Settings > Safety > Blocked people."));
    }
    if !gs.dm_store.as_ref().is_some_and(|s| s.is_friend(peer)) {
        return Err(format!("{name} is not a friend here: a permit goes only to people you follow who follow you back."));
    }
    if !gs.peer_kyber_keys.contains_key(peer) {
        return Err(format!(
            "{name} has not been online on this server since you started the app, so a permit cannot be sealed for them yet."
        ));
    }
    if gs.household.on_their_way.iter().any(|p| p.peer == peer) {
        return Err(format!("A permit for {name} is still on its way; wait for the server's answer."));
    }
    if !(1..=PERMIT_MAX_DAYS).contains(&days) {
        return Err(BAD_LENGTH.into());
    }
    let at = now_ms();
    let now = at / 1000;
    let seed = gs.private_key_bytes.clone().unwrap_or_default();
    let Some(serial) = crate::relay::core::pq_crypto::new_friend_cert_serial() else { return Err(NOT_MADE.into()) };
    let grantee = plot_owner_id(peer);
    let sig = crate::relay::core::pq_crypto::build_plot_permit(&seed, &server, &plot, &grantee, &serial, expiry, now).map_err(|e| {
        log::warn!("Household: a permit for {plot} was not made ({e:?})");
        NOT_MADE.to_string()
    })?;
    let permit = Permit { issuer: gs.profile_public_key.clone(), server, plot: plot.clone(), grantee, serial: serial.clone(), expiry, sig };
    // Theirs carries the pass they gave me, so it rides the friend lane past their gate; ours,
    // the self-copy, waits for the server's answer.
    let Some((mut theirs, ours)) = crate::engine::dm::control_puts_at(gs, peer, &hh::note_text(&permit), None, at) else {
        return Err(NOT_MADE.into());
    };
    let Some(reference) = crate::net::put_answers::new_ref() else { return Err(NOT_MADE.into()) };
    theirs["ref"] = serde_json::Value::String(reference.clone());
    let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) else { return Err(NOT_CONNECTED.into()) };
    client.send(&theirs.to_string());
    if let Some(store) = gs.dm_store.as_mut() {
        let row = GivenPermit { to: peer.to_string(), plot: plot.clone(), serial: serial.clone(), expiry, at, state: GivenState::OnItsWay { reference: reference.clone() } };
        store.record_given(row, now);
        save_pruned(store);
    }
    gs.household.on_their_way.push(PermitPut {
        reference,
        peer: peer.to_string(),
        plot: plot.clone(),
        serial,
        expiry,
        days,
        self_copy: ours,
        at: std::time::Instant::now(),
    });
    gs.household.line = format!("Sending {name} a permit for your plot {plot} until {}...", hh::date_words(expiry));
    Ok(())
}

/// Renew the permit given to `peer` (the card's Renew): a new permit for the same length, counted
/// from today, with a new serial; once the server took it, the old one is taken back
/// ([`on_answer`]), so one permit per friend and plot is ever live. Refused with a sentence when
/// nothing was given to them, or when it was for a plot that is no longer mine here.
pub fn renew(gs: &mut GuiState, peer: &str) -> Result<(), String> {
    if !gs.protected.setup.permit_allowed() {
        return Err(PROTECTED.into());
    }
    let name = crate::engine::dm::dm_display_name(gs, peer);
    let Some(row) = gs.dm_store.as_ref().and_then(|s| s.permits_given().iter().filter(|g| g.to == peer).max_by_key(|g| g.at).cloned()) else {
        return Err(format!("You have not given {name} a permit here yet."));
    };
    if let Some(mine) = gs.household.my_plot.as_deref() {
        if row.plot != mine {
            return Err(format!(
                "{name}'s permit is for plot {}, which is no longer yours here, so it cannot be renewed. Give a new one for {mine}.",
                row.plot
            ));
        }
    }
    let days = length_days(&row);
    let expiry = now_s() + days * DAY;
    give_until(gs, peer, days, expiry)
}

// ── The server's answers ──────────────────────────────────────────────────────────────────────

/// `dm_put_ok` / `dm_put_refused` (engine/put_answer.rs `on_frame`, first): true when it answers a
/// permit's send, handled here.
/// - Taken: the row says so, the self-copy goes to my own mailbox, and every older permit to that
///   friend for that plot that might still stand is taken back (amendment A1). A permit taken back
///   while on its way stays taken back, and its self-copy never goes.
/// - Refused: the row says why; no self-copy goes (the friend never got it).
pub fn on_answer(gs: &mut GuiState, answer: &Answer) -> bool {
    let Some(i) = gs.household.on_their_way.iter().position(|p| p.reference == answer.reference()) else { return false };
    let put = gs.household.on_their_way.remove(i);
    let name = crate::engine::dm::dm_display_name(gs, &put.peer);
    let Some(store) = gs.dm_store.as_mut() else { return true };
    match answer {
        Answer::Taken(_) => {
            let recorded = store.set_given_state(&put.serial, GivenState::Taken);
            save_pruned(store);
            if !recorded {
                return true; // taken back on its way (Take back, Block): nobody else hears of it
            }
            if let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) {
                client.send(&put.self_copy.to_string());
            }
            gs.household.line = format!("{name} has a permit for your plot {} until {}.", put.plot, hh::date_words(put.expiry));
            let older = take_back_rows(gs, &put.peer, |g| g.serial == put.serial || g.plot != put.plot);
            if !older.is_empty() {
                tell_my_devices(gs, &older);
            }
            crate::engine::dm::send_pending_withdrawals(gs);
        }
        Answer::Refused(_, reason) => {
            store.set_given_state(&put.serial, GivenState::Refused { reason: reason.clone() });
            save_pruned(store);
            gs.household.line = refused_words(&name, reason);
        }
    }
    true
}

/// What the card says of a permit the server did not take, by its reason (plan 3.9).
pub fn refused_words(name: &str, reason: &str) -> String {
    match reason {
        "reach" => format!("{name}'s settings do not take messages from you right now."),
        "rate" => "The server is busy; try again in a moment.".to_string(),
        _ => "The server did not take it; try again.".to_string(),
    }
}

/// Permit sends that have waited [`ANSWER_WAIT`] with no answer (engine/put_answer.rs `expire`,
/// every frame): the row says unconfirmed (the friend may or may not have it), and no self-copy
/// goes.
pub fn expire(gs: &mut GuiState, now: std::time::Instant) {
    if !gs.household.on_their_way.iter().any(|p| now.duration_since(p.at) >= ANSWER_WAIT) {
        return;
    }
    let (late, waiting): (Vec<PermitPut>, Vec<PermitPut>) =
        std::mem::take(&mut gs.household.on_their_way).into_iter().partition(|p| now.duration_since(p.at) >= ANSWER_WAIT);
    gs.household.on_their_way = waiting;
    let names: Vec<String> = late.iter().map(|p| crate::engine::dm::dm_display_name(gs, &p.peer)).collect();
    if let Some(store) = gs.dm_store.as_mut() {
        for put in &late {
            store.set_given_state(&put.serial, GivenState::Unconfirmed);
        }
        save_pruned(store);
    }
    if let Some(name) = names.last() {
        gs.household.line = format!("The server did not say whether {name} got the permit. Give it again to be sure.");
    }
}

// ── Taking back ───────────────────────────────────────────────────────────────────────────────

/// Mark taken back every permit given to `peer` that might stand and that `keep` does not keep,
/// queue their serials for the relay (the passes' resend list), and save. Returns those rows.
fn take_back_rows(gs: &mut GuiState, peer: &str, keep: impl Fn(&GivenPermit) -> bool) -> Vec<GivenPermit> {
    let Some(store) = gs.dm_store.as_mut() else { return Vec::new() };
    let serials = store.take_back_permits(peer, keep);
    if serials.is_empty() {
        return Vec::new();
    }
    let rows: Vec<GivenPermit> = store.permits_given().iter().filter(|g| serials.contains(&g.serial)).cloned().collect();
    save_pruned(store);
    rows
}

/// Tell my other devices on this server that I took back `rows`: one note to myself per few rows,
/// to my own mailbox on the server I am on (permits are per server). Nothing when not connected:
/// the withdrawals themselves still wait for the next connection, and the relay's `cert_revoked`
/// then reaches every device of mine that is online.
fn tell_my_devices(gs: &GuiState, rows: &[GivenPermit]) {
    let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) else { return };
    for chunk in rows.chunks(ROWS_PER_NOTE) {
        if let Some((put, _id)) = crate::engine::block::note_put_text(gs, &take_back_note_text(chunk), now_ms()) {
            client.send(&put.to_string());
        }
    }
}

/// Take back every permit given to `peer` (the card's Take back, after it asked first). At once
/// and whether connected or not: each row says taken back, each serial waits among the
/// withdrawals resent to the relay until it confirms (`cert_revoke` now when connected, on every
/// member list until `cert_revoked`), and my other devices are told. A permit to them still on
/// its way is taken back with the rest and its self-copy never goes. Ok(how many), or Err with the
/// card's sentence.
pub fn take_back(gs: &mut GuiState, peer: &str) -> Result<usize, String> {
    let name = crate::engine::dm::dm_display_name(gs, peer);
    if gs.private_key_bytes.is_none() || !crate::engine::dm::ensure_dm_store(gs) {
        return Err(LOCKED.into());
    }
    gs.household.on_their_way.retain(|p| p.peer != peer);
    let rows = take_back_rows(gs, peer, |_| false);
    if rows.is_empty() {
        return Err(format!("{name} holds no permit from you to take back."));
    }
    tell_my_devices(gs, &rows);
    crate::engine::dm::send_pending_withdrawals(gs);
    gs.household.line = format!("Taking back {name}'s permit...");
    Ok(rows.len())
}

/// BLOCK TAKES BACK EVERY PERMIT GIVEN TO THE BLOCKED PERSON (amendment A1), in the same step that
/// withdraws their passes: engine/dm.rs `send_pending_withdrawals` runs this first, and every
/// Block path calls it right after the passes are withdrawn (engine/block.rs `take_back` for a
/// Block here or a block note from my other device, `sweep` for a block made elsewhere, on every
/// member list). Runs whether connected or not; the serials wait with the passes'. Returns how many
/// permits it took back.
pub fn take_back_from_blocked(gs: &mut GuiState) -> usize {
    let Some(store) = gs.dm_store.as_ref() else { return 0 };
    let blocked: BTreeSet<String> = store
        .permits_given()
        .iter()
        .filter(|g| g.state.may_stand() && crate::engine::block::is_blocked(gs, &g.to))
        .map(|g| g.to.clone())
        .collect();
    if blocked.is_empty() {
        return 0;
    }
    gs.household.on_their_way.retain(|p| !blocked.contains(&p.peer));
    let Some(store) = gs.dm_store.as_mut() else { return 0 };
    let taken: usize = blocked.iter().map(|to| store.take_back_permits(to, |_| false).len()).sum();
    save_pruned(store);
    log::info!("Household: Block took back {taken} permit(s)");
    taken
}

// ── What the card shows ───────────────────────────────────────────────────────────────────────

/// Where the permit for my plot given to one person stands, as the card's row says it (plan 3.9,
/// amendment A1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Nothing given (or nothing left on the lists): the lengths and Give.
    None,
    /// On its way this run: "Sending the permit...".
    Sending { expiry: u64 },
    /// The server took it and it has not run out: until `expiry`; Renew gives `days` again.
    Stands { expiry: u64, days: u64 },
    /// No answer came: the friend may or may not have it ("Give it again to be sure").
    Unconfirmed { expiry: u64 },
    /// The server did not take it, for `reason` (`reach`, `rate`, ...).
    Refused { reason: String },
    /// It ran out.
    RanOut { expiry: u64 },
    /// Taken back, and the server has not confirmed it yet: "Taking it back...".
    TakingBack { expiry: u64 },
    /// Taken back and confirmed.
    TakenBack { expiry: u64 },
}

/// The [`Standing`] of the permits `rows` (all given to one person, for one plot), at `now_s`:
/// one on its way this run first, then the latest-ending one the server took, then one never
/// answered, then whatever the latest given says. `pending` is the DM store's withdrawals the relay
/// has not confirmed.
pub fn standing(household: &hh::Household, rows: &[&GivenPermit], pending: &[String], now_s: u64) -> Standing {
    let state = |g: &GivenPermit| household.given_state(g);
    if let Some(g) = rows.iter().filter(|g| matches!(state(g), GivenState::OnItsWay { .. })).max_by_key(|g| g.at) {
        return Standing::Sending { expiry: g.expiry };
    }
    if let Some(g) = rows.iter().filter(|g| g.state == GivenState::Taken && g.expiry > now_s).max_by_key(|g| g.expiry) {
        return Standing::Stands { expiry: g.expiry, days: length_days(g) };
    }
    if let Some(g) = rows.iter().filter(|g| state(g) == GivenState::Unconfirmed && g.expiry > now_s).max_by_key(|g| g.at) {
        return Standing::Unconfirmed { expiry: g.expiry };
    }
    let Some(g) = rows.iter().max_by_key(|g| g.at) else { return Standing::None };
    match state(g) {
        GivenState::Refused { reason } => Standing::Refused { reason },
        GivenState::TakenBack if pending.contains(&g.serial) => Standing::TakingBack { expiry: g.expiry },
        GivenState::TakenBack => Standing::TakenBack { expiry: g.expiry },
        _ => Standing::RanOut { expiry: g.expiry },
    }
}

/// One person on the card's "Your plot" list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendRow {
    pub key: String,
    /// Their name as the chat shows it (`dm_display_name`).
    pub name: String,
    /// A friend here now (a mutual follow): only friends can be given a permit or a renewal.
    pub friend: bool,
    /// Someone I blocked.
    pub blocked: bool,
    /// Their DM key is known this run, so a permit can be sealed for them.
    pub sealable: bool,
    /// The permit for my plot this session (any plot before the welcome names one).
    pub permit: Standing,
    /// Permits that might still stand for plots that are no longer mine here: (plot, expiry).
    pub other_plots: Vec<(String, u64)>,
}

/// The card's "Your plot" rows (plan 3.9): every friend on this server (mutual follows in the DM
/// store, so offline friends too), and anyone else I gave a permit to that is still listed (an old
/// friend, someone blocked), sorted by name.
pub fn friends(gs: &GuiState) -> Vec<FriendRow> {
    let Some(store) = gs.dm_store.as_ref() else { return Vec::new() };
    let now = now_s();
    let my_plot = gs.household.my_plot.as_deref();
    let mut keys: BTreeSet<String> = store.following().iter().filter(|k| store.is_follower(k)).cloned().collect();
    keys.extend(store.permits_given().iter().map(|g| g.to.clone()));
    let mut rows: Vec<FriendRow> = keys
        .into_iter()
        .map(|key| {
            let given: Vec<&GivenPermit> = store.permits_given().iter().filter(|g| g.to == key).collect();
            let for_mine: Vec<&GivenPermit> = given.iter().filter(|g| my_plot.is_none_or(|p| g.plot == p)).copied().collect();
            let other_plots = given
                .iter()
                .filter(|g| my_plot.is_some_and(|p| g.plot != p) && gs.household.given_state(g).may_stand() && g.expiry > now)
                .map(|g| (g.plot.clone(), g.expiry))
                .collect();
            FriendRow {
                name: crate::engine::dm::dm_display_name(gs, &key),
                friend: store.is_friend(&key),
                blocked: crate::engine::block::is_blocked(gs, &key),
                sealable: gs.peer_kyber_keys.contains_key(&key),
                permit: standing(&gs.household, &for_mine, store.pending_withdrawals(), now),
                other_plots,
                key,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.key.cmp(&b.key)));
    rows
}

/// One permit on the card's "Plots you may build on" list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldRow {
    /// The giver's key and name.
    pub from: String,
    pub name: String,
    pub plot: String,
    pub expiry: u64,
    /// It has run out by my clock.
    pub ran_out: bool,
    /// What the relay said of it this session, if it was asked.
    pub verdict: Option<Verdict>,
}

/// The card's "Plots you may build on" rows: each permit held on this server, by giver name then
/// plot. Whether its plot is on this ship is the card's question (it has the ship).
pub fn held_rows(gs: &GuiState) -> Vec<HeldRow> {
    let Some(store) = gs.dm_store.as_ref() else { return Vec::new() };
    let now = now_s();
    let mut rows: Vec<HeldRow> = store
        .permits_held()
        .iter()
        .map(|h| HeldRow {
            from: h.from.clone(),
            name: crate::engine::dm::dm_display_name(gs, &h.from),
            plot: h.permit.plot.clone(),
            expiry: h.permit.expiry,
            ran_out: h.permit.expiry <= now,
            verdict: gs.household.verdict(h),
        })
        .collect();
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.plot.cmp(&b.plot)));
    rows
}

/// The permit to show with a build on, or a take-down of my own piece on, `plot` (lane 2A's
/// `send_next`): the held permit for that plot that has not run out, one the relay said works this
/// session first, then the latest-ending. None when I hold none.
pub fn permit_for(gs: &GuiState, plot: &str) -> Option<Permit> {
    permit_for_at(gs, plot, now_s())
}

fn permit_for_at(gs: &GuiState, plot: &str, now_s: u64) -> Option<Permit> {
    held_for_at(gs, plot, now_s).map(|h| h.permit.clone())
}

/// [`permit_for`] as the held permit, so a build's refusal can be filed against the permit it
/// carried (engine/shared_build.rs `send_next`, plan 3.4 item 3).
pub(crate) fn held_for<'a>(gs: &'a GuiState, plot: &str) -> Option<&'a HeldPermit> {
    held_for_at(gs, plot, now_s())
}

fn held_for_at<'a>(gs: &'a GuiState, plot: &str, now_s: u64) -> Option<&'a HeldPermit> {
    let store = gs.dm_store.as_ref()?;
    store
        .permits_held()
        .iter()
        .filter(|h| h.permit.plot == plot && h.permit.expiry > now_s)
        .max_by_key(|h| (gs.household.verdict(h) == Some(Verdict::Works), h.permit.expiry))
}

// ── This session in the shared world (lane 2A, the game's glue) ───────────────────────────────
//
// WHEN A HELD PERMIT IS ASKED ABOUT (plan 3.4 and 3.5). Only while the game is in a server's shared
// world and welcomed (engine/shared_build.rs `tick_core` calls [`tick`]): every permit held for a
// neighbour's plot of this ship, not run out, right after each welcome ([`session_began`]); again
// `RECHECK_S` after its last answer; at once when a new one arrives (`ingest` sets
// `next_check_at` to 0); one unanswered for `UNANSWERED_S` is asked again, and one the relay could
// not judge (`again`) after `AGAIN_S`. One check at a time, at most one each `CHECK_GAP_S` (the
// relay turns away a second within 200 ms). A permit the relay refused for good (it ran out, it
// was taken back, it does not check out, it runs too long) is not asked about again: a renewal is
// a new permit, with a new serial, and is asked about as one.
//
// WHAT OPENS (plan 3.7, [`open_for`], run every frame by [`apply_open`]). A plot this game may
// build on now (`Household::usable_plots`: the relay said works this session, not run out, a
// neighbour's plot of this ship) is walkable; so is the plot the player stands in, or in the
// corridor of, while it was walkable the frame before, so nobody is ever walled in when a permit
// stops working or the session ends. The walkable plot the player stands in is lit.

/// After its last answer, a permit is asked about again this many seconds later (plan 3.4).
pub const RECHECK_S: f64 = 300.0;
/// A check with no answer this long is asked again, seconds (plan 3.5).
pub const UNANSWERED_S: f64 = 10.0;
/// A check the relay could not judge (`again`) is asked again this many seconds later (plan 3.5).
pub const AGAIN_S: f64 = 5.0;
/// At most one check each this many seconds (shared.rs `SEND_INTERVAL_MS`; the relay's own limit
/// for permit checks is 200 ms per player).
pub const CHECK_GAP_S: f64 = crate::systems::construction::shared::SEND_INTERVAL_MS as f64 / 1000.0;

/// The crosshair's words while a held permit for the plot aimed at is still being checked with the
/// server (plan 3.8), after "Placing Wood Wall: ".
pub const CHECKING_WORDS: &str = "your household permit for this plot is being checked with the server; try again in a moment";

/// A welcome to a server's shared world (engine/shared_build.rs `welcome`): this session's plot is
/// the welcome's `home_plot` (None for a guest), and every held permit is asked about afresh
/// (`next_check_at` 0). What the relay said in an earlier session says nothing about this one.
/// What the player may walk out of (`walkable`, `lit`) is left to [`apply_open`].
pub(crate) fn session_began(gs: &mut GuiState, welcome: &serde_json::Value) {
    let plot = welcome
        .get("home_plot")
        .filter(|v| !v.is_null())
        .and_then(|hp| hp.get("id"))
        .and_then(|i| i.as_str())
        .map(str::to_string);
    let hh = &mut gs.household;
    hh.my_plot = plot;
    hh.welcomed = true;
    hh.verdicts.clear();
    hh.usable.clear();
    hh.asked.clear();
    hh.next_check_at = 0.0;
}

/// The session ended (left, dropped, or a new join waiting for its welcome; engine/shared_build.rs
/// `leave`): no plot of this server is mine any more, and nothing the relay said of a permit counts.
/// KEPT: the permits on their way to a friend and the card's last line (DM sends, answered on their
/// own, not this session's), and what the player may walk out of (`walkable`, `lit`), which
/// [`apply_open`] lets go as they step off: so nobody is walled in on the plot they stand in.
pub(crate) fn session_ended(gs: &mut GuiState) {
    let hh = &mut gs.household;
    hh.my_plot = None;
    hh.welcomed = false;
    hh.verdicts.clear();
    hh.usable.clear();
    hh.asked.clear();
    hh.next_check_at = 0.0;
}

/// A refusal no later check can change for this permit: it ran out, it was taken back, it does not
/// check out, it runs too long. (Not from the plot's holder can change: the plot may come back.)
fn refused_for_good(why: Why) -> bool {
    matches!(why, Why::PermitExpired | Why::PermitWithdrawn | Why::PermitBad | Why::PermitTooLong)
}

/// When a permit judged `judged` is next asked about (the game's clock), or None for never. Not yet
/// asked, or asked and its check let go: now.
fn due_at(judged: Option<&hh::Judged>) -> Option<f64> {
    let Some(j) = judged else { return Some(0.0) };
    match j.verdict {
        Verdict::Checking => Some(0.0),
        Verdict::Later => Some(j.at + AGAIN_S),
        Verdict::Works => Some(j.at + RECHECK_S),
        Verdict::Refused(w) if refused_for_good(w) => None,
        Verdict::Refused(_) => Some(j.at + RECHECK_S),
    }
}

/// The checks (plan 3.5), once a frame while in the shared world and welcomed: the next permit due,
/// as a `game_permit_check` into `outbox`, at most one each [`CHECK_GAP_S`] (`last_sent`, the game's
/// clock, kept by the caller). `now` is the game's clock, seconds (`SharedBuild`'s). A permit is
/// asked about only when it is for a neighbour's plot of this ship (never my own), has not run out
/// and is not being asked about already. Until the next one is due, nothing is looked at
/// (`next_check_at`).
pub(crate) fn tick(gs: &mut GuiState, now: f64, last_sent: &mut Option<f64>, outbox: &mut Vec<String>) {
    if now < gs.household.next_check_at {
        return;
    }
    // A check unanswered this long was lost: its permit is due again.
    gs.household.asked.retain(|_, (_, at)| now - *at < UNANSWERED_S);
    if last_sent.is_some_and(|t| now - t < CHECK_GAP_S) {
        return;
    }
    let (Some(store), Some(ship)) = (gs.dm_store.as_ref(), gs.ship_structure.as_ref()) else { return };
    let unix = now_s();
    let neighbours: BTreeSet<&str> = ship.neighbour_plots().map(|p| p.id.as_str()).collect();
    let house = &mut gs.household;
    let asking: BTreeSet<&hh::PermitKey> = house.asked.values().map(|(k, _)| k).collect();
    let mut next: Option<(f64, &HeldPermit)> = None;
    for h in store.permits_held() {
        let plot = h.permit.plot.as_str();
        if h.permit.expiry <= unix || !neighbours.contains(plot) || house.my_plot.as_deref() == Some(plot) {
            continue;
        }
        let key = hh::PermitKey::of(h);
        if asking.contains(&key) {
            continue;
        }
        if let Some(due) = due_at(house.verdicts.get(&key)) {
            if next.is_none_or(|(d, _)| due < d) {
                next = Some((due, h));
            }
        }
    }
    let reask = house.asked.values().map(|(_, at)| at + UNANSWERED_S).fold(f64::INFINITY, f64::min);
    match next {
        Some((due, h)) if due <= now => {
            house.last_req = house.last_req.wrapping_add(1).max(1);
            let req_id = house.last_req;
            let key = hh::PermitKey::of(h);
            let check = crate::systems::construction::shared::ToRelay::PermitCheck { req_id, permit: h.permit.clone() };
            outbox.push(serde_json::to_string(&check).unwrap_or_default());
            house.asked.insert(req_id, (key.clone(), now));
            // A permit asked about again keeps what the relay said last until it answers, so a
            // door that is open stays open while it is asked.
            house.verdicts.entry(key).or_insert(hh::Judged { verdict: Verdict::Checking, at: now });
            *last_sent = Some(now);
        }
        other => house.next_check_at = other.map_or(f64::INFINITY, |(d, _)| d).min(reask),
    }
}

/// `game_permit_status` (engine/shared_build.rs `receive`): the relay's word on check `req_id`.
/// Works, refused with its why, or `again` (asked again after [`AGAIN_S`]). An answer to no check
/// this session asked (an earlier session's, a duplicate) changes nothing. A permit that worked and
/// now does not says so in a notice (plan 3.10); one refused from the start is shown on the card.
/// True when it answered a check of ours.
pub(crate) fn on_status(gs: &mut GuiState, req_id: u32, plot: &str, ok: bool, why: Option<Why>, again: bool, now: f64) -> bool {
    let Some((key, _)) = gs.household.asked.remove(&req_id) else { return false };
    if key.plot != plot {
        log::warn!("Household: the server answered check {req_id} about plot {plot}, asked about {}", key.plot);
    }
    let verdict = if ok {
        Verdict::Works
    } else if again {
        Verdict::Later
    } else {
        Verdict::Refused(why.unwrap_or(Why::Unknown))
    };
    log::info!("Household: the server says the permit for {} is {verdict:?}", key.plot);
    let was = gs.household.verdicts.insert(key.clone(), hh::Judged { verdict, at: now }).map(|j| j.verdict);
    gs.household.next_check_at = 0.0;
    if let (Some(Verdict::Works), Verdict::Refused(w)) = (was, verdict) {
        let notice = no_longer_works(gs, &key, w);
        gs.pending_notices.push(notice);
    }
    true
}

/// A build or take-down the relay refused with a `permit_*` why (engine/shared_build.rs `refused`,
/// plan 3.4 item 3): the permit it carried, `key`, is refused for that reason from now on, so its
/// plot closes as a check's refusal closes it.
pub(crate) fn refused_by_server(gs: &mut GuiState, key: hh::PermitKey, why: Why, now: f64) {
    let was = gs.household.verdicts.insert(key.clone(), hh::Judged { verdict: Verdict::Refused(why), at: now }).map(|j| j.verdict);
    gs.household.next_check_at = 0.0;
    if was == Some(Verdict::Works) {
        log::info!("Household: a build on {} was refused ({}); that plot closes", key.plot, why.as_str());
    }
}

/// The notice when a permit that worked stops working: "Your household permit for plot p1 no
/// longer works: Ada took it back."
fn no_longer_works(gs: &GuiState, key: &hh::PermitKey, why: Why) -> String {
    use crate::systems::construction::shared::{why_words, Action, Reason};
    let name = crate::engine::dm::dm_display_name(gs, &key.from);
    let reason = match why {
        Why::PermitNotFromHolder => format!("{name} does not hold that plot any more"),
        Why::PermitWithdrawn => format!("{name} took it back"),
        Why::PermitExpired => "it ran out".to_string(),
        w => why_words(Action::Build, Reason::NotAllowed, Some(w)),
    };
    format!("Your household permit for plot {} no longer works: {reason}.", key.plot)
}

/// What this game's permits say of building on `plot` now (the build gate, engine/shared_build.rs
/// `gate`, plan 3.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlotPermit {
    /// No permit held for it.
    None,
    /// A permit the relay said works this session ([`Household::usable`](hh::Household)).
    Usable,
    /// A permit not yet answered for, or one the relay could not judge yet.
    Checking,
    /// Every permit held for it ran out, or the relay refused it, for this reason.
    Refused(Why),
}

/// [`PlotPermit`] for `plot` from the session's state and the permits `held`, at `now_s` (Unix
/// seconds). A usable one first; then one still being checked (or not yet asked); then why the
/// latest-ending one does not work. Pure.
pub(crate) fn plot_permit(household: &hh::Household, held: &[HeldPermit], plot: &str, now_s: u64) -> PlotPermit {
    if household.usable.contains(plot) {
        return PlotPermit::Usable;
    }
    let mine: Vec<&HeldPermit> = held.iter().filter(|h| h.permit.plot == plot).collect();
    if mine.is_empty() {
        return PlotPermit::None;
    }
    let asking = |h: &HeldPermit| matches!(household.verdict(h), None | Some(Verdict::Checking | Verdict::Later));
    if mine.iter().any(|h| h.permit.expiry > now_s && asking(h)) {
        return PlotPermit::Checking;
    }
    let latest = mine.iter().max_by_key(|h| h.permit.expiry).copied();
    match latest.map(|h| (h.permit.expiry > now_s, household.verdict(h))) {
        Some((true, Some(Verdict::Refused(w)))) => PlotPermit::Refused(w),
        // A permit that worked and has since run out by this game's clock (the relay rules on a
        // build by its own clock; this only says so before anything is spent).
        _ => PlotPermit::Refused(Why::PermitExpired),
    }
}

/// THE OPEN PLOTS NOW (plan 3.7; [`apply_open`]'s core, pure on its parts). Sets `household.usable`
/// from the permits `held` (works this session, not run out at `now_s`, a neighbour's plot of
/// `ship`), then `walkable` = usable, plus the plot the player stands in or in the corridor of
/// (`standing`, ship metres; None while not aboard) when it was walkable already, and `lit` = that
/// plot when walkable. Returns what `ship.open` should be.
pub(crate) fn open_for(
    household: &mut hh::Household,
    held: &[HeldPermit],
    ship: &crate::ship::ship_structure::ShipStructure,
    standing: Option<glam::Vec3>,
    now_s: u64,
) -> crate::ship::ship_structure::OpenPlots {
    let neighbours: BTreeSet<&str> = ship.neighbour_plots().map(|p| p.id.as_str()).collect();
    household.usable = household.usable_plots(held, |p| neighbours.contains(p), now_s);
    // Where the player stands matters only when something is or was open.
    let here: Option<String> = if household.usable.is_empty() && household.walkable.is_empty() {
        None
    } else {
        standing.and_then(|p| crate::engine::home_plot::plot_ground_of(ship, p)).map(|p| p.id.clone())
    };
    let mut walkable = household.usable.clone();
    if let Some(h) = here.as_ref().filter(|h| household.walkable.contains(*h)) {
        walkable.insert(h.clone());
    }
    household.lit = here.filter(|h| walkable.contains(h));
    household.walkable = walkable;
    crate::ship::ship_structure::OpenPlots { plots: household.walkable.clone(), lit: household.lit.clone() }
}

/// Make `ship.open` `want`; true when it changed. Compared with the LIVE ship each frame, so a ship
/// rebuilt since (which starts with nothing open) is opened again.
pub(crate) fn set_open(ship: &mut crate::ship::ship_structure::ShipStructure, want: crate::ship::ship_structure::OpenPlots) -> bool {
    if ship.open == want {
        return false;
    }
    ship.open = want;
    true
}

/// Once a frame (engine/shared_build.rs `tick`, joined or not): the plots this game may walk into
/// ([`open_for`]), from where the camera stands while aboard; when they changed, the ship takes
/// them and the colliders, doors and lights that read them are redone
/// (engine/home_meshes.rs `open_plots_changed`).
pub(crate) fn apply_open(state: &mut crate::engine::state::EngineState) {
    let standing = crate::engine::home_plot::aboard(state).then_some(state.camera.position);
    let gs = &mut state.gui_state;
    let Some(ship) = gs.ship_structure.as_ref() else { return };
    let held: &[HeldPermit] = gs.dm_store.as_ref().map_or(&[], |s| s.permits_held());
    let want = open_for(&mut gs.household, held, ship, standing, now_s());
    let Some(ship) = gs.ship_structure.as_mut() else { return };
    if set_open(ship, want) {
        crate::engine::home_meshes::open_plots_changed(state);
    }
}

// ── What the dev IPC reports (engine/ipc.rs, the recorder's `household`) ───────────────────────

/// The probe's `household` (engine/ipc.rs's recorder, done JSON; scripts/verify-copresence.js
/// --household reads it).
pub(crate) fn probe_json(state: &crate::engine::state::EngineState) -> serde_json::Value {
    probe_core(&state.gui_state, &state.door_panels)
}

/// [`probe_json`] on its parts: `doors` are the door panels with their open fractions
/// (`EngineState::door_panels`). Fields:
/// - `me` (my full key), `me_did` (my id as the relay holds plots under it), `server` (the server's
///   own `did:hum:` this DM store holds), `my_plot` (this session's);
/// - `friends`: the card's "Your plot" rows, each with its permit's standing;
/// - `given` and `held`: every permit given and held on this server, `given` with its state as the
///   card reads it, `held` with what the relay said this session (`verdict`: works, checking,
///   later, refused, or null; `why` for refused);
/// - `usable`, `walkable`, `lit`, `open_plots` (what the ship has open now) and `open_lights` (how
///   many lights the lit open plot adds);
/// - `mouths`: each neighbour plot's corridor door mouths, both ends, with `others_only` (true:
///   shut to this player) and `open` (the doors' largest open fraction);
/// - `checks_waiting`, `sending`, `withdrawals_waiting` and `line`.
pub(crate) fn probe_core(gs: &GuiState, doors: &[(crate::ship::door_panels::PanelPlacement, f32)]) -> serde_json::Value {
    use serde_json::json;
    let store = gs.dm_store.as_ref();
    let household = &gs.household;
    let me = gs.profile_public_key.as_str();
    let friend_rows: Vec<serde_json::Value> = friends(gs)
        .iter()
        .map(|r| {
            json!({
                "key": r.key,
                "name": r.name,
                "friend": r.friend,
                "blocked": r.blocked,
                "sealable": r.sealable,
                "permit": standing_json(&r.permit),
                "other_plots": r.other_plots,
            })
        })
        .collect();
    let given: Vec<serde_json::Value> = store
        .map(|s| {
            s.permits_given()
                .iter()
                .map(|g| {
                    let row = GivenPermit { state: household.given_state(g), ..g.clone() };
                    serde_json::to_value(row).unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let held: Vec<serde_json::Value> = store
        .map(|s| {
            s.permits_held()
                .iter()
                .map(|h| {
                    let (verdict, why) = match household.verdict(h) {
                        None => (None, None),
                        Some(Verdict::Works) => (Some("works"), None),
                        Some(Verdict::Checking) => (Some("checking"), None),
                        Some(Verdict::Later) => (Some("later"), None),
                        Some(Verdict::Refused(w)) => (Some("refused"), Some(w.as_str())),
                    };
                    json!({
                        "from": h.from,
                        "plot": h.permit.plot,
                        "serial": h.permit.serial,
                        "expiry": h.permit.expiry,
                        "at": h.at,
                        "verdict": verdict,
                        "why": why,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let ship = gs.ship_structure.as_ref();
    json!({
        "me": me,
        "me_did": (!me.is_empty()).then(|| plot_owner_id(me)),
        "server": store.and_then(|s| s.pass_server()),
        "my_plot": household.my_plot,
        "friends": friend_rows,
        "given": given,
        "held": held,
        "usable": household.usable,
        "walkable": household.walkable,
        "lit": household.lit,
        "open_plots": ship.map(|s| s.open.plots.clone()).unwrap_or_default(),
        "open_lights": ship.map_or(0, |s| crate::engine::home_meshes::open_plot_lights(s).len()),
        "mouths": ship.map_or_else(|| json!({}), |s| mouths_json(s, doors)),
        "checks_waiting": household.asked.len(),
        "sending": household.on_their_way.len(),
        "withdrawals_waiting": store.map_or(0, |s| s.pending_withdrawals().len()),
        "line": household.line,
    })
}

/// A [`Standing`] as the probe writes it: `{"state": "stands", "expiry": ..., "days": ...}`.
fn standing_json(s: &Standing) -> serde_json::Value {
    use serde_json::json;
    match s {
        Standing::None => json!({ "state": "none" }),
        Standing::Sending { expiry } => json!({ "state": "sending", "expiry": expiry }),
        Standing::Stands { expiry, days } => json!({ "state": "stands", "expiry": expiry, "days": days }),
        Standing::Unconfirmed { expiry } => json!({ "state": "unconfirmed", "expiry": expiry }),
        Standing::Refused { reason } => json!({ "state": "refused", "reason": reason }),
        Standing::RanOut { expiry } => json!({ "state": "ran_out", "expiry": expiry }),
        Standing::TakingBack { expiry } => json!({ "state": "taking_back", "expiry": expiry }),
        Standing::TakenBack { expiry } => json!({ "state": "taken_back", "expiry": expiry }),
    }
}

/// Each neighbour plot's corridor door mouths (`NeighbourView::mouths_by_plot`, both ends of its
/// corridor), keyed by plot: where the mouth is (its aperture's floor centre, ship metres), how many
/// door halves stand in it, whether they open only for the other players (`others_only`, null with
/// none), and how far open they are (the larger fraction). The halves are found by where they stand
/// among the neighbours' doors, which `ship_panel_placements` puts last.
fn mouths_json(ship: &crate::ship::ship_structure::ShipStructure, doors: &[(crate::ship::door_panels::PanelPlacement, f32)]) -> serde_json::Value {
    use crate::ship::ship_structure::CorridorAxis;
    let view = crate::ship::neighbours::neighbour_view(ship);
    let tail_len = crate::ship::door_panels::neighbour_panel_placements(ship).len();
    let tail = &doors[doors.len().saturating_sub(tail_len)..];
    let mut out: std::collections::BTreeMap<String, Vec<serde_json::Value>> = std::collections::BTreeMap::new();
    for (plot, m) in view.mouths_by_plot(ship) {
        let (x, z) = match m.axis {
            CorridorAxis::X => (m.plane, m.lat),
            CorridorAxis::Z => (m.lat, m.plane),
        };
        let in_mouth = |c: glam::Vec3| {
            let (along, across) = match m.axis {
                CorridorAxis::X => (c.x, c.z),
                CorridorAxis::Z => (c.z, c.x),
            };
            (along - m.plane).abs() < 0.05 && (across - m.lat).abs() <= m.door.0 * 0.5 + 0.01 && (c.y - m.floor_y).abs() < 0.05
        };
        let halves: Vec<&(crate::ship::door_panels::PanelPlacement, f32)> = tail.iter().filter(|(p, _)| in_mouth(p.center)).collect();
        let others_only = (!halves.is_empty()).then(|| halves.iter().all(|(p, _)| p.others_only));
        let open = halves.iter().map(|(_, f)| *f).fold(0.0f32, f32::max);
        out.entry(plot.to_string()).or_default().push(serde_json::json!({
            "at": [x, m.floor_y, z],
            "doors": halves.len(),
            "others_only": others_only,
            "open": open,
        }));
    }
    serde_json::to_value(out).unwrap_or_default()
}

#[cfg(test)]
#[path = "household_tests.rs"]
mod tests;

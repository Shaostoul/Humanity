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
//! has not taken it back) is the relay's `game_permit_check`, asked by the game's glue (lane 2A).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::gui::GuiState;
use crate::net::dm_pq::{DmInner, CTL_PERMIT};
use crate::net::dm_store::DmStore;
use crate::net::household::{self as hh, Arrival, GivenPermit, GivenState, HeldPermit, Kept, PermitPut, Verdict};
use crate::net::put_answers::{Answer, ANSWER_WAIT};
use crate::relay::storage::plot_owner_id;
use crate::systems::construction::shared::{Permit, PERMIT_MAX_DAYS};

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
    let store = gs.dm_store.as_ref()?;
    store
        .permits_held()
        .iter()
        .filter(|h| h.permit.plot == plot && h.permit.expiry > now_s)
        .max_by_key(|h| (gs.household.verdict(h) == Some(Verdict::Works), h.permit.expiry))
        .map(|h| h.permit.clone())
}

#[cfg(test)]
#[path = "household_tests.rs"]
mod tests;

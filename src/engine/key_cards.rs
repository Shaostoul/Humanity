//! Key cards in the desktop app (2026-10-22, docs/design/key-cards.md): signing our own card,
//! asking servers for other people's, checking every one, pinning, and the change notice.
//!
//! THE ONE RULE: `GuiState::peer_kyber_keys` holds only keys from key cards this app checked
//! itself (signed by the person's own identity key, naming that person) and pinned
//! (net/key_pins.rs), never a key a server merely said. Every seal to a person reads that map
//! (engine/dm.rs `control_puts`, the Chat page's DM send, household permits, file passes, reports
//! to a group's creator, recovery pieces through `pinned_for_recovery`), so none of them can be
//! handed a server's key. A group's key is sealed with cards checked the same way
//! (net/api_v2.rs `group_member_keys`).
//!
//! HOW CARDS ARRIVE. The member lists carry only each person's card TIME (`key_card_at`). This
//! app asks for a card (`key_card_request`, at most 16 keys a frame) when it has none pinned for
//! someone it may seal to (the people it has conversations with, follows, is followed by, has a
//! request from, or has open in Chat), or when a list shows a card newer than the pinned one. The
//! answer (`key_cards`) is checked card by card here, from whichever server sent it.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::gui::GuiState;
use crate::net::key_pins::KeyPins;
use crate::relay::core::key_card::{self, Refusal, Seen, REQUEST_MAX};

/// How long a card asked for may take before it is asked for again.
const ASK_AGAIN: Duration = Duration::from_secs(60);
/// How often the people this app may seal to are looked over for cards it lacks.
const SWEEP_EVERY: Duration = Duration::from_secs(30);
/// The most request frames sent in one frame of the app.
const FRAMES_PER_PUMP: usize = 4;

/// The app's key-card bookkeeping (in `GuiState::key_cards`).
#[derive(Default)]
pub struct KeyCardsState {
    /// This identity's pins (net/key_pins.rs), loaded with the identity.
    pub pins: Option<KeyPins>,
    /// Cards to ask for, in order, each once.
    queue: Vec<String>,
    /// When each card was last asked for.
    asked: HashMap<String, Instant>,
    /// People a server said it holds no card for, or sent one that did not check out.
    no_card: HashSet<String>,
    /// When the people this app may seal to were last looked over.
    last_sweep: Option<Instant>,
}

impl KeyCardsState {
    /// Has a card for `person` been asked for and not answered yet.
    pub fn asking(&self, person: &str) -> bool {
        self.queue.iter().any(|k| k == person) || self.asked.get(person).is_some_and(|t| t.elapsed() < ASK_AGAIN)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ── Our own card ──

/// Sign our own key card from the seed in memory (`own_key_card`, the JSON sent with every
/// sign-in). Nothing when the seed is locked.
pub fn sign_own_card(gs: &mut GuiState) {
    let Some(seed) = gs.private_key_bytes.as_ref() else { return };
    match key_card::sign_card(seed, now_ms()) {
        Ok(card) => gs.own_key_card = key_card::card_json(&card).to_string(),
        Err(e) => log::error!("key cards: could not sign our own card: {e}"),
    }
}

/// A new identity is in memory (`apply_pq_identity`): sign its card, and load ITS pins, so no key
/// pinned under another identity seals anything for this one.
pub fn on_identity(gs: &mut GuiState) {
    sign_own_card(gs);
    gs.peer_kyber_keys.clear();
    gs.key_cards = KeyCardsState::default();
    ensure_pins(gs);
}

/// Make sure the pins of the identity unlocked now are loaded, and their keys are in
/// `peer_kyber_keys`. False while there is no identity to key them by.
pub fn ensure_pins(gs: &mut GuiState) -> bool {
    if gs.profile_public_key.is_empty() {
        return false;
    }
    if gs.key_cards.pins.as_ref().is_some_and(|p| p.identity() == gs.profile_public_key) {
        return true;
    }
    let Some(seed) = gs.private_key_bytes.as_ref() else { return false };
    #[cfg(not(test))]
    let pins = KeyPins::load(seed, &gs.profile_public_key);
    // Tests never read or write the real pins folder: a test identity's pins from an earlier run
    // would change what this one sees.
    #[cfg(test)]
    let pins = KeyPins::in_temp(seed, &gs.profile_public_key, "engine");
    for (person, kyber) in pins.sealable() {
        gs.peer_kyber_keys.insert(person, kyber);
    }
    gs.key_cards.pins = Some(pins);
    true
}

// ── Asking ──

/// Ask for `person`'s card (unless it was asked for within a minute). `update`: ask even when a
/// key for them is pinned (a list showed a newer card).
pub fn want(gs: &mut GuiState, person: &str, update: bool) {
    if person.is_empty() || person == gs.profile_public_key || !key_card::is_key_text(person) {
        return;
    }
    if !update && gs.peer_kyber_keys.contains_key(person) {
        return;
    }
    if gs.key_cards.asking(person) {
        return;
    }
    gs.key_cards.queue.push(person.to_string());
}

/// The people this app may seal things to: those it has a conversation with, follows, is
/// followed by, or has a request from on the server it is on, and the conversation open in Chat.
fn people_to_seal_to(gs: &GuiState) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(store) = gs.dm_store.as_ref() {
        out.extend(store.conversations().into_iter().map(|c| c.peer));
        out.extend(store.following().iter().cloned());
        out.extend(store.followers().iter().cloned());
        out.extend(store.requests().iter().map(|r| r.key.clone()));
    }
    if let Some(partner) = open_partner(gs) {
        out.push(partner.to_string());
    }
    out.sort();
    out.dedup();
    out
}

/// The person whose conversation is open in Chat, if one is.
fn open_partner(gs: &GuiState) -> Option<&str> {
    gs.chat_active_channel.strip_prefix("dm:").filter(|k| !k.is_empty())
}

/// Is `person` someone this app may seal things to (see `people_to_seal_to`).
fn may_seal_to(gs: &GuiState, person: &str) -> bool {
    open_partner(gs) == Some(person)
        || gs.dm_store.as_ref().is_some_and(|s| {
            s.is_following(person) || s.is_follower(person) || !s.conversation(person).is_empty() || s.requests().iter().any(|r| r.key == person)
        })
}

/// Ask for the cards this app lacks of everyone it may seal to. Not again for someone the server
/// said it holds no card for: a list showing a card time for them asks again (`note_listed`).
pub fn want_people(gs: &mut GuiState) {
    for person in people_to_seal_to(gs) {
        if !gs.key_cards.no_card.contains(&person) {
            want(gs, &person, false);
        }
    }
    gs.key_cards.last_sweep = Some(Instant::now());
}

/// A member list named `person` with the time of the card their server holds (None: it holds
/// none). Asks for the card when it is newer than the pinned one, or when none is pinned and this
/// app may seal to them. No key ever comes from a list.
pub fn note_listed(gs: &mut GuiState, person: &str, at: Option<u64>) {
    if person.is_empty() || person == gs.profile_public_key {
        return;
    }
    let Some(at) = at else {
        // This server holds no card for them (others may): keep what is pinned, ask nothing here.
        if !gs.peer_kyber_keys.contains_key(person) {
            gs.key_cards.no_card.insert(person.to_string());
        }
        return;
    };
    ensure_pins(gs);
    let known = gs.key_cards.pins.as_ref().map(|p| {
        let newest = p.changed(person).or(p.pinned(person)).map(|pin| pin.at);
        newest.is_some_and(|seen| at <= seen)
    });
    match known {
        Some(true) => {}
        _ if gs.key_cards.pins.as_ref().is_some_and(|p| p.pinned(person).is_some()) => want(gs, person, true),
        _ if may_seal_to(gs, person) => {
            gs.key_cards.no_card.remove(person);
            want(gs, person, false)
        }
        _ => {}
    }
}

/// A new socket signed in. The first time on a socket, requests in flight on the old one are
/// forgotten (their answers are lost) and the people this app may seal to are looked over.
pub fn on_signed_in(gs: &mut GuiState, first_on_socket: bool) {
    if first_on_socket {
        gs.key_cards.asked.clear();
        ensure_pins(gs);
        // The people come from this server's DM store: load it now rather than wait for the
        // next sweep (it is loaded lazily, engine/dm.rs).
        crate::engine::dm::ensure_dm_store(gs);
        want_people(gs);
    }
}

/// Every frame: send what is queued, a batch of [`REQUEST_MAX`] per request frame, on the socket
/// of the server the app is on (or, when it has none, the first signed-in link: a card is good
/// from any server), and look the people over every [`SWEEP_EVERY`].
pub fn pump(gs: &mut GuiState) {
    if gs.key_cards.last_sweep.is_none_or(|t| t.elapsed() >= SWEEP_EVERY) && gs.ws_identified {
        ensure_pins(gs);
        want_people(gs);
    }
    // The conversation open in Chat: its card is wanted at once, so it is here by the time the
    // person has typed a message (asked at most once a minute).
    if let Some(partner) = open_partner(gs).map(str::to_string) {
        if !gs.peer_kyber_keys.contains_key(&partner) && !gs.key_cards.no_card.contains(&partner) {
            want(gs, &partner, false);
        }
    }
    if gs.key_cards.queue.is_empty() {
        return;
    }
    let primary = gs.ws_identified && gs.ws_client.as_ref().is_some_and(|c| c.is_connected());
    let link = if primary { None } else { gs.connections.iter().position(|c| c.identified && c.ws.as_ref().is_some_and(|w| w.is_connected())) };
    if !primary && link.is_none() {
        return;
    }
    for _ in 0..FRAMES_PER_PUMP {
        if gs.key_cards.queue.is_empty() {
            break;
        }
        let n = gs.key_cards.queue.len().min(REQUEST_MAX);
        let batch: Vec<String> = gs.key_cards.queue.drain(..n).collect();
        let frame = serde_json::json!({ "type": "key_card_request", "keys": batch }).to_string();
        if primary {
            if let Some(c) = gs.ws_client.as_ref() {
                c.send(&frame);
            }
        } else if let Some(c) = link.and_then(|i| gs.connections.get(i)).and_then(|c| c.ws.as_ref()) {
            c.send(&frame);
        }
        let now = Instant::now();
        for k in batch {
            gs.key_cards.asked.insert(k, now);
        }
    }
}

// ── Answers ──

/// A `key_cards` answer from any server: every card checked and taken by the pin rule; the keys
/// it holds no card for noted.
pub fn on_key_cards_frame(gs: &mut GuiState, val: &serde_json::Value) {
    for card in val.get("cards").and_then(|v| v.as_array()).into_iter().flatten() {
        ingest(gs, card);
    }
    for k in val.get("missing").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str()) {
        if !gs.peer_kyber_keys.contains_key(k) {
            gs.key_cards.no_card.insert(k.to_string());
        }
    }
}

/// Check one card and take it by the pin rule. A card that does not check out (a server-swapped
/// key, the wrong signer, the wrong shape) gives no key. Returns what it meant (None: refused).
pub fn ingest(gs: &mut GuiState, value: &serde_json::Value) -> Option<Seen> {
    let card = key_card::parse_card(value)?;
    gs.key_cards.asked.remove(&card.key);
    if !key_card::card_for(&card, &card.key) {
        log::warn!("key cards: refused a card for {}… that does not check out as theirs", &card.key[..12.min(card.key.len())]);
        if !gs.peer_kyber_keys.contains_key(&card.key) {
            gs.key_cards.no_card.insert(card.key.clone());
        }
        return None;
    }
    if card.key == gs.profile_public_key || !ensure_pins(gs) {
        return None;
    }
    let pins = gs.key_cards.pins.as_mut()?;
    let seen = pins.take(&card);
    match pins.sealing_key(&card.key) {
        Some(kyber) => {
            gs.peer_kyber_keys.insert(card.key.clone(), kyber.to_string());
        }
        None => {
            gs.peer_kyber_keys.remove(&card.key);
        }
    }
    gs.key_cards.no_card.remove(&card.key);
    if seen == Seen::Changed {
        log::warn!("key cards: the key of {}… changed; nothing is sealed to them until it is accepted", &card.key[..12]);
    }
    Some(seen)
}

/// The person accepted `person`'s changed key: it is sealed to from now on.
pub fn accept(gs: &mut GuiState, person: &str) -> bool {
    let Some(kyber) = gs.key_cards.pins.as_mut().and_then(|p| p.accept(person)) else { return false };
    gs.peer_kyber_keys.insert(person.to_string(), kyber);
    true
}

// ── What a person reads ──

/// Why nothing can be sealed to `person` now, or None when something can.
pub fn refusal(gs: &GuiState, person: &str) -> Option<Refusal> {
    if gs.peer_kyber_keys.contains_key(person) {
        return None;
    }
    if gs.key_cards.pins.as_ref().is_some_and(|p| p.changed(person).is_some()) {
        return Some(Refusal::Changed);
    }
    if gs.key_cards.asking(person) {
        return Some(Refusal::Fetching);
    }
    if gs.key_cards.no_card.contains(person) {
        return Some(Refusal::NoCard);
    }
    Some(Refusal::Fetching)
}

/// The sentence for [`refusal`], naming `person` the way the app does. Asks for their card when
/// it is still to come, so trying again in a moment works.
pub fn refusal_text(gs: &mut GuiState, person: &str) -> Option<String> {
    let why = refusal(gs, person)?;
    if why == Refusal::Fetching {
        want(gs, person, false);
    }
    let name = crate::engine::dm::dm_display_name(gs, person);
    Some(why.text(&name))
}

/// The change notice for `person`, when their key changed and is waiting to be accepted.
pub fn change_notice(gs: &GuiState, person: &str) -> Option<String> {
    gs.key_cards.pins.as_ref()?.changed(person)?;
    Some(key_card::change_notice(&crate::engine::dm::dm_display_name(gs, person)))
}

/// What the recovery section may seal a trusted person's piece to (gui/pages/settings/recovery.rs,
/// on its thread): the pinned key when there is one; an error naming why when their key changed
/// (accept it first); None when nothing is pinned yet, so the thread fetches their card and checks
/// it itself (`settings/recovery_net.rs`).
pub fn pinned_for_recovery(gs: &GuiState, person: &str) -> Result<Option<String>, String> {
    if gs.key_cards.pins.as_ref().is_some_and(|p| p.changed(person).is_some()) {
        return Err(Refusal::Changed.text(&crate::engine::dm::dm_display_name(gs, person)));
    }
    Ok(gs.peer_kyber_keys.get(person).cloned())
}

#[cfg(test)]
#[path = "key_cards_tests.rs"]
mod tests;

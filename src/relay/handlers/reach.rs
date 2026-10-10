//! "Who can reach me" at the relay (step B, 2026-10-09, docs/design/blocking-and-safe-mode.md
//! 10c, which is the specification: the message names and shapes below are what both clients
//! build against).
//!
//! Each person chooses, for each kind of contact, who may use it, from a short ladder
//! (narrowest first): `nobody`, `chosen` (people whose friendship pass from them allows that
//! kind), `friends` (anyone holding a valid pass from them), `groups` (friends, plus people who
//! share a P2P group with them) and `anyone`. A kind with no saved choice is at its safe
//! default: messages from friends, calls from people they choose, trades from friends. The
//! relay keeps no friends list to decide this: the pass the sender presents (checked by
//! handlers/friend_passes.rs `friend_pass` against the relay's own facts and the withdrawal
//! list) says whether they are a friend and what the friend may do.
//!
//! Where it is enforced, each door a person can be reached through:
//! - `message`: `dm_put` (msg_handlers.rs `handle_dm_put`, through [`dm_gate`] and [`pay`]).
//! - `call`: a `voice_call` ring and the call's own `webrtc_signal`s ([`call_may_pass`],
//!   [`signal_may_pass`]). A ring that is let through opens the call between the two people, so
//!   the callee's accept and both sides' offers, answers and candidates pass whatever the
//!   CALLER'S own call setting is (the callee is answering, not reaching out).
//! - `trade`: `trade_request` (msg_handlers.rs `handle_trade_request`, through [`admit`]).
//! - `dc_offer` (a direct connection, which hands over a network address): only from someone
//!   holding a valid pass from the target, sharing a P2P group with them or in the same voice
//!   room, or from the target's own key (their other devices) ([`dc_offer_may_pass`]). This
//!   backs up the clients' own gate (BUG-171, BUG-173).
//!
//! What a refused sender learns: a refused message or trade request gets
//! `{"type":"reach_refused","kind":"message"|"trade","to":"<target key>"}`, the same for
//! everyone refused, so it singles nobody out (4.7). Nothing is stored or delivered. A refused
//! call and a refused `dc_offer` get nothing at all: they ring out, which also says nothing
//! about the target's presence (BUG-172).
//!
//! Admins and moderators are bound like everyone (10a, question 5): no role is exempt at these
//! gates. (They still skip the stranger's daily knock budget once let in, as they always have:
//! the audience binds them, the budget never did.)
//!
//! Contact requests (as amended in review, 2026-10-09): a sender refused for `message` may send
//! `dm_put` with `"contact_request": true`. It is an ordinary signed, sealed v2 DM, with the
//! ordinary size limits, carrying the requester's own pass for the recipient (inside the sealed
//! part, where the relay cannot see it). It is let through whatever the recipient's `message`
//! audience, unless that is `nobody`, when the sender has one of today's
//! [`CONTACT_REQUESTS_PER_DAY`] left (a budget of its own, separate from the knocks). The relay
//! stores nothing about who asked whom: the accepter's reply gets through the requester's own
//! gate because it presents the requester's pass as its `friend_cert`, which [`dm_gate`] checks
//! like any other pass. The recipient's client shows only the sender's registered name.
//!
//! Group reports (2026-10-10, 10j): a report about a P2P group's message goes to the group's
//! creator as an ordinary signed, sealed v2 DM sent with `"group_report": true`, because only the
//! creator can read the group's messages and remove someone from it. It is let through the
//! creator's `message` audience, unless that is `nobody`, when the sender and the recipient are
//! both active members of some group the recipient created, and the sender has one of today's
//! [`GROUP_REPORTS_PER_DAY`] left (a budget of its own, separate from the knocks and the contact
//! requests). A friend's report is free, as a friend's DM is: the exception only matters where
//! the ordinary gate would refuse, or charge a knock. Anything else carrying the flag is treated
//! exactly as the same DM without it. The relay keeps nothing about who reported whom: the
//! budget counts per sender, in memory.
//!
//! The settings: `{"type":"reach_set","settings":{"message":"...","call":"...","trade":"..."}}`
//! from the person's own signed-in socket saves any subset of kinds ([`handle_reach_set`]); the
//! relay answers `{"type":"reach_settings","settings":{...}}` with all three kinds filled in, to
//! every socket of theirs, after every `reach_set` and after a successful identify. Both are
//! always on, whatever features the owner switches off (features.rs `WS_ALWAYS_ON`).
//!
//! The kinds and audiences are protocol words, signed into passes and read by both clients, so
//! they live here as code (as `FRIEND_PASS_KINDS` does in pq_crypto.rs), not in a data file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::relay::core::pq_crypto::FriendPass;
use crate::relay::handlers::friend_passes::friend_pass;
use crate::relay::relay::{RelayMessage, RelayState};

/// A kind of contact the relay enforces now (10c). `invite` and `voice_message` are valid words
/// in a pass but not enforced yet: invitations travel as tickets inside messages (so the message
/// rule covers them) and no voice messages exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Message,
    Call,
    Trade,
}

impl Kind {
    /// Every kind, in the order the settings list them.
    pub const ALL: [Kind; 3] = [Kind::Message, Kind::Call, Kind::Trade];

    /// The word on the wire, in a saved row and in a pass's `may` (pq_crypto `FRIEND_PASS_KINDS`).
    pub fn word(self) -> &'static str {
        match self {
            Kind::Message => "message",
            Kind::Call => "call",
            Kind::Trade => "trade",
        }
    }

    pub fn from_word(word: &str) -> Option<Kind> {
        Self::ALL.into_iter().find(|k| k.word() == word)
    }

    /// The audience of a person who saved nothing for this kind: the safe defaults (10c).
    pub fn default_audience(self) -> Audience {
        match self {
            Kind::Message => Audience::Friends,
            Kind::Call => Audience::Chosen,
            Kind::Trade => Audience::Friends,
        }
    }
}

/// Who may use a kind of contact, narrowest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    /// No one.
    Nobody,
    /// Holders of a valid pass from the person whose `may` includes this kind.
    Chosen,
    /// Holders of any valid pass from the person.
    Friends,
    /// Friends, plus people who share a P2P group with the person.
    Groups,
    /// Everyone (strangers still spend the daily knock budget where the path has one).
    Anyone,
}

impl Audience {
    pub const ALL: [Audience; 5] = [Audience::Nobody, Audience::Chosen, Audience::Friends, Audience::Groups, Audience::Anyone];

    pub fn word(self) -> &'static str {
        match self {
            Audience::Nobody => "nobody",
            Audience::Chosen => "chosen",
            Audience::Friends => "friends",
            Audience::Groups => "groups",
            Audience::Anyone => "anyone",
        }
    }

    pub fn from_word(word: &str) -> Option<Audience> {
        Self::ALL.into_iter().find(|a| a.word() == word)
    }
}

/// The `settings` object of `reach_settings`: every kind's audience word, defaults filled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReachSettings {
    pub message: String,
    pub call: String,
    pub trade: String,
}

/// Cert-less "knocks" allowed per sender per day (follows-graph removal, 2026-08-24). A stranger
/// let in without a friendship pass (an `anyone` or `groups` audience) can still reach out, but
/// only this many times a day across ALL recipients, so nobody can flood. Deliberately
/// sender-scoped, never per pair: a per-pair counter would be a social graph again. One budget
/// for DMs and trade requests together (2026-10-09, 3.7.5).
pub const DM_KNOCKS_PER_DAY: u32 = 20;

/// Contact requests one sender may send a day, across all recipients (10c), separate from the
/// knocks.
pub const CONTACT_REQUESTS_PER_DAY: u32 = 5;

/// Reports about a group one sender may send its creators a day, across all groups (10j),
/// separate from the knocks and the contact requests. Low on purpose: a report is rare, and the
/// exception reaches someone who may have chosen to hear from no strangers.
pub const GROUP_REPORTS_PER_DAY: u32 = 3;

/// How long a call that was let through stays open with no ring or signal between its two
/// people. Each one refreshes it, so a long call stays open; an abandoned one closes by itself.
const CALL_IDLE: Duration = Duration::from_secs(2 * 60 * 60);

/// The most open calls the relay remembers at once (past it, the oldest is forgotten first,
/// which only means its next signal is checked against the callee's setting again).
const OPEN_CALLS_MAX: usize = 10_000;

/// What "who can reach me" keeps in memory (never on disk): the calls it let through, and the
/// contact requests and group reports each sender sent today. Lost on a restart, which only
/// closes open calls (their next signal is checked against the setting again) and refills the
/// day's budgets.
#[derive(Default)]
pub struct ReachState {
    /// Calls let through: a one-way hash of the two keys (sorted, so either may be first) to the
    /// time of the last ring or signal between them. A hash, not the keys: 32 bytes a call, and
    /// nothing readable about who is calling whom.
    calls: Mutex<HashMap<[u8; 32], Instant>>,
    /// Contact requests sent today: sender key -> (Unix day, count). Sender-scoped like the knocks.
    contact_requests: Mutex<HashMap<String, (i64, u32)>>,
    /// Group reports sent today: sender key -> (Unix day, count). Sender-scoped, never per pair,
    /// so it says nothing about whom anyone reported (10j: the relay stores nothing of that).
    group_reports: Mutex<HashMap<String, (i64, u32)>>,
}

/// The id of the call between `a` and `b`, the same whichever of them is first.
fn call_id(a: &str, b: &str) -> [u8; 32] {
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    let mut h = blake3::Hasher::new();
    h.update(b"hum/reach-call/v1\n");
    h.update(x.as_bytes());
    h.update(b"\n");
    h.update(y.as_bytes());
    *h.finalize().as_bytes()
}

impl ReachState {
    /// A ring was let through: the call between `a` and `b` is open from now.
    fn open_call(&self, a: &str, b: &str) {
        let now = Instant::now();
        let mut calls = self.calls.lock().unwrap_or_else(|p| p.into_inner());
        if calls.len() >= OPEN_CALLS_MAX {
            calls.retain(|_, last| now.duration_since(*last) < CALL_IDLE);
        }
        if calls.len() >= OPEN_CALLS_MAX {
            if let Some(oldest) = calls.iter().min_by_key(|(_, last)| **last).map(|(id, _)| *id) {
                calls.remove(&oldest);
            }
        }
        calls.insert(call_id(a, b), now);
    }

    /// Is a call between `a` and `b` open? Refreshes it when it is, forgets it when it went idle.
    fn in_call(&self, a: &str, b: &str) -> bool {
        let id = call_id(a, b);
        let now = Instant::now();
        let mut calls = self.calls.lock().unwrap_or_else(|p| p.into_inner());
        match calls.get_mut(&id) {
            Some(last) if now.duration_since(*last) < CALL_IDLE => {
                *last = now;
                true
            }
            Some(_) => {
                calls.remove(&id);
                false
            }
            None => false,
        }
    }

    /// The call between `a` and `b` ended (rejected or hung up).
    fn end_call(&self, a: &str, b: &str) {
        self.calls.lock().unwrap_or_else(|p| p.into_inner()).remove(&call_id(a, b));
    }
}

/// Today, in Unix days.
fn unix_day() -> i64 {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() / 86_400) as i64
}

/// A notice to `who` alone, as a system line.
fn tell(state: &RelayState, who: &str, message: String) {
    let _ = state.broadcast_tx.send(RelayMessage::Private { to: who.to_string(), message });
}

/// The audience `key` chose for `kind`, or its default (also for a saved word this relay does
/// not know, which only a hand-edited database could hold).
pub fn audience(state: &RelayState, key: &str, kind: Kind) -> Audience {
    state
        .db
        .reach_audience_of(key, kind.word())
        .as_deref()
        .and_then(Audience::from_word)
        .unwrap_or(kind.default_audience())
}

/// `key`'s settings for every kind, defaults filled in.
pub fn settings_of(state: &RelayState, key: &str) -> ReachSettings {
    let saved = state.db.reach_settings_of(key);
    let pick = |kind: Kind| {
        saved
            .iter()
            .find(|(k, _)| k == kind.word())
            .and_then(|(_, a)| Audience::from_word(a))
            .unwrap_or(kind.default_audience())
            .word()
            .to_string()
    };
    ReachSettings { message: pick(Kind::Message), call: pick(Kind::Call), trade: pick(Kind::Trade) }
}

/// The `reach_settings` message for `key`'s own sockets.
pub fn settings_message(state: &RelayState, key: &str) -> RelayMessage {
    RelayMessage::ReachSettings { to: key.to_string(), settings: settings_of(state, key) }
}

/// Do `a` and `b` share a P2P group: are both active members of one group that is not
/// disbanded (`p2p_groups_for_member` on both keys)? A key that is not hex (a bot) is in none.
fn share_a_group(state: &RelayState, a: &str, b: &str) -> bool {
    let groups_of = |key: &str| -> Vec<String> {
        hex::decode(key)
            .ok()
            .and_then(|pk| state.db.p2p_groups_for_member(&pk).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    };
    let first = groups_of(a);
    !first.is_empty() && groups_of(b).iter().any(|g| first.contains(g))
}

/// Did `creator` create a P2P group (not disbanded) that both they and `member` are active
/// members of now? The group-report exception's condition (10j). Sharing a group is not enough:
/// only the creator holds the group's messages to check a report against and can remove anyone.
/// A key that is not hex (a bot) created nothing.
fn created_a_group_with(state: &RelayState, creator: &str, member: &str) -> bool {
    match (hex::decode(creator), hex::decode(member)) {
        (Ok(c), Ok(m)) => state.db.p2p_group_created_by_with(&c, &m).unwrap_or(false),
        _ => false,
    }
}

/// Are `a` and `b` both in one voice room right now?
async fn in_same_voice_room(state: &RelayState, a: &str, b: &str) -> bool {
    state.voice_rooms.read().await.values().any(|room| {
        room.participants.iter().any(|(k, _)| k == a) && room.participants.iter().any(|(k, _)| k == b)
    })
}

/// May `sender` reach `target` by `kind`? `pass` is the pass `target` gave `sender`, already
/// checked (`friend_pass`: None for no pass, a forged one, one given on another server or to
/// someone else, or one withdrawn). No role is exempt. Everyone may reach themselves (their own
/// other devices).
pub fn allowed(state: &RelayState, target: &str, sender: &str, kind: Kind, pass: Option<&FriendPass>) -> bool {
    if target == sender {
        return true;
    }
    match audience(state, target, kind) {
        Audience::Nobody => false,
        Audience::Chosen => pass.is_some_and(|p| p.may.allows(kind.word())),
        Audience::Friends => pass.is_some(),
        Audience::Groups => pass.is_some() || share_a_group(state, target, sender),
        Audience::Anyone => true,
    }
}

/// A message or trade request that was let in, with the pass its sender presented (None: let in
/// without one, a stranger, who spends a knock where the path has a budget).
pub struct Admitted {
    pub pass: Option<FriendPass>,
}

/// The gate for a trade request from `sender` to `target`, carrying the pass `cert`: let in, or
/// refused with `reach_refused {kind, to}` sent to the sender alone. A message makes the same
/// check in [`dm_gate`], which has a contact request's and a group report's exceptions beside it.
pub fn admit(state: &Arc<RelayState>, target: &str, sender: &str, kind: Kind, cert: Option<&str>) -> Option<Admitted> {
    let pass = friend_pass(state, target, sender, cert);
    if allowed(state, target, sender, kind, pass.as_ref()) {
        return Some(Admitted { pass });
    }
    let _ = state.broadcast_tx.send(RelayMessage::ReachRefused {
        sender: sender.to_string(),
        kind: kind.word().to_string(),
        to: target.to_string(),
    });
    None
}

/// What a `dm_put` that was let in costs its sender, paid by [`pay`] once the rate limiter has
/// let the send through (so a send the limiter slows down spends nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cost {
    /// A friend, an admin or moderator let in, or the sender's own self-copy.
    Free,
    /// A stranger let in: one of today's [`DM_KNOCKS_PER_DAY`].
    Knock,
    /// A contact request: one of today's [`CONTACT_REQUESTS_PER_DAY`].
    ContactRequest,
    /// A report about a group, let in by its own exception: one of today's
    /// [`GROUP_REPORTS_PER_DAY`].
    GroupReport,
}

/// What a `dm_put` asks of the gate, read from its flags: an ordinary message, a contact request
/// (`contact_request: true`, 10c) or a report about a group to its creator (`group_report: true`,
/// 10j).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmAsk {
    Ordinary,
    ContactRequest,
    GroupReport,
}

impl DmAsk {
    /// The ask a `dm_put`'s two flags make. No client sets both; if one does, it is a contact
    /// request, which is let through more widely than a group report anyway, so a put carrying
    /// both gains nothing over the contact-request rule and spends that budget.
    pub fn from_flags(contact_request: bool, group_report: bool) -> DmAsk {
        if contact_request {
            DmAsk::ContactRequest
        } else if group_report {
            DmAsk::GroupReport
        } else {
            DmAsk::Ordinary
        }
    }
}

/// The "who can reach me" gate for a `dm_put` from `sender` to `to`, someone other than
/// themselves: the recipient's `message` audience, or for a contact request or a group report its
/// own rule (the envelope's shape and size were already checked like any DM's). `role` is the
/// sender's. Returns what the send costs, or None when it is refused (the sender has been told,
/// with the refusal everyone gets).
///
/// A group report is first checked as the same DM without the flag would be: a friend (or an
/// admin or moderator) the audience lets in sends it free, as they would any DM, so a friend's
/// fourth report in a day still arrives. Only where the ordinary gate would refuse, or would
/// charge a stranger's knock, does the exception apply, and then it costs a group report. Where
/// the exception does not apply either, the ordinary answer stands: a knock, or the refusal.
pub fn dm_gate(state: &Arc<RelayState>, sender: &str, to: &str, cert: Option<&str>, ask: DmAsk, role: &str) -> Option<Cost> {
    let refuse = || {
        let _ = state.broadcast_tx.send(RelayMessage::ReachRefused {
            sender: sender.to_string(),
            kind: Kind::Message.word().to_string(),
            to: to.to_string(),
        });
        None
    };
    if ask == DmAsk::ContactRequest {
        if audience(state, to, Kind::Message) == Audience::Nobody {
            return refuse();
        }
        return Some(Cost::ContactRequest);
    }
    let pass = friend_pass(state, to, sender, cert);
    let let_in = allowed(state, to, sender, Kind::Message, pass.as_ref());
    let free = pass.is_some() || role == "admin" || role == "mod";
    if let_in && free {
        return Some(Cost::Free);
    }
    if ask == DmAsk::GroupReport
        && audience(state, to, Kind::Message) != Audience::Nobody
        && created_a_group_with(state, to, sender)
    {
        return Some(Cost::GroupReport);
    }
    if let_in {
        return Some(Cost::Knock);
    }
    refuse()
}

/// Pay what [`dm_gate`] said a send costs. False (and the sender told why) when today's budget
/// for it is spent.
pub async fn pay(state: &Arc<RelayState>, sender: &str, cost: Cost) -> bool {
    match cost {
        Cost::Free => true,
        Cost::Knock => {
            if spend_knock(state, sender).await {
                return true;
            }
            tell(
                state,
                sender,
                format!(
                    "🔒 Daily limit for messaging people who haven't befriended you yet ({DM_KNOCKS_PER_DAY}/day, shared with trade requests). Once they add you as a friend, messages are unlimited."
                ),
            );
            false
        }
        Cost::ContactRequest => {
            if spend_daily(&state.reach.contact_requests, sender, CONTACT_REQUESTS_PER_DAY) {
                return true;
            }
            tell(
                state,
                sender,
                format!("You have sent today's {CONTACT_REQUESTS_PER_DAY} contact requests. You can send more tomorrow."),
            );
            false
        }
        Cost::GroupReport => {
            if spend_daily(&state.reach.group_reports, sender, GROUP_REPORTS_PER_DAY) {
                return true;
            }
            tell(
                state,
                sender,
                format!("You have sent today's {GROUP_REPORTS_PER_DAY} reports to group creators. You can send more tomorrow."),
            );
            false
        }
    }
}

/// Spend one of `sender`'s knocks for today. False when today's budget is already used up
/// (nothing is spent then). One budget, two doors: a DM and a trade request to someone who has
/// not given you a pass (2026-10-09, 3.7.5); before, a trade request was a way around the DM
/// budget.
pub(crate) async fn spend_knock(state: &RelayState, sender: &str) -> bool {
    let today = unix_day();
    let mut knocks = state.dm_knocks.write().await;
    let entry = knocks.entry(sender.to_string()).or_insert((today, 0));
    if entry.0 != today {
        *entry = (today, 0);
    }
    if entry.1 >= DM_KNOCKS_PER_DAY {
        return false;
    }
    entry.1 += 1;
    true
}

/// Spend one of `sender`'s sends for today from a per-sender daily budget of `limit` (the contact
/// requests, the group reports). False when they are used up (nothing is spent then).
fn spend_daily(budget: &Mutex<HashMap<String, (i64, u32)>>, sender: &str, limit: u32) -> bool {
    let today = unix_day();
    let mut sent = budget.lock().unwrap_or_else(|p| p.into_inner());
    let entry = sent.entry(sender.to_string()).or_insert((today, 0));
    if entry.0 != today {
        *entry = (today, 0);
    }
    if entry.1 >= limit {
        return false;
    }
    entry.1 += 1;
    true
}

/// May this `voice_call` from `caller` to `callee` go on? A `ring` is checked against the
/// callee's `call` audience with the pass it carries, and when let through opens the call
/// between the two. Any other action (`accept`, `reject`, `hangup`) goes through within an open
/// call, or else is checked as a ring would be, without opening one; a reject or hangup closes
/// the call. A refusal sends nothing back, whatever the callee's presence.
pub fn call_may_pass(state: &Arc<RelayState>, caller: &str, callee: &str, action: &str, cert: Option<&str>) -> bool {
    let by_setting = || allowed(state, callee, caller, Kind::Call, friend_pass(state, callee, caller, cert).as_ref());
    if action == "ring" {
        if !by_setting() {
            return false;
        }
        state.reach.open_call(caller, callee);
        return true;
    }
    let ok = state.reach.in_call(caller, callee) || by_setting();
    if ok && (action == "reject" || action == "hangup") {
        state.reach.end_call(caller, callee);
    }
    ok
}

/// Is a call between `a` and `b` open (a ring let through, not ended, not idle)? The call
/// forwarder's credentials are given for an open call (call_credentials.rs). Refreshes it when it
/// is, as a signal between the two would.
pub fn call_is_open(state: &RelayState, a: &str, b: &str) -> bool {
    a != b && state.reach.in_call(a, b)
}

/// May this `webrtc_signal` from `sender` to `to` go on? A `dc_offer` meets
/// [`dc_offer_may_pass`]. Its answer and candidates (`dc_answer`, `dc_ice`) go through as they
/// always have: they answer an offer, and one to an offer that was never let through finds no
/// connection to join. Every other kind is a call's (`offer`, `answer`, `ice`) and goes through
/// within an open call, or else when the recipient's `call` audience lets the sender in.
/// Refusals are silent.
pub async fn signal_may_pass(state: &Arc<RelayState>, sender: &str, to: &str, signal_type: &str, cert: Option<&str>) -> bool {
    match signal_type {
        "dc_offer" => dc_offer_may_pass(state, sender, to, cert).await,
        "dc_answer" | "dc_ice" => true,
        _ => state.reach.in_call(sender, to) || allowed(state, to, sender, Kind::Call, friend_pass(state, to, sender, cert).as_ref()),
    }
}

/// May `sender` offer `to` a direct connection (which hands over a network address)? Only from
/// `to`'s own key (their other devices), a holder of a valid pass from `to`, someone sharing a
/// P2P group with them, or someone in the same voice room (10c).
pub async fn dc_offer_may_pass(state: &Arc<RelayState>, sender: &str, to: &str, cert: Option<&str>) -> bool {
    sender == to
        || friend_pass(state, to, sender, cert).is_some()
        || share_a_group(state, sender, to)
        || in_same_voice_room(state, sender, to).await
}

/// The longest piece of a refused word repeated back in a notice.
const ECHO_MAX_CHARS: usize = 40;

/// The kinds and audiences a `reach_set` names, checked: each key a kind the relay enforces,
/// each value one of the five audiences. Err says why, for the notice.
fn parse_settings(settings: Option<&serde_json::Value>) -> Result<Vec<(&'static str, &'static str)>, String> {
    let echo = |s: &str| s.chars().take(ECHO_MAX_CHARS).collect::<String>();
    let Some(object) = settings.and_then(|v| v.as_object()) else {
        return Err("the settings were missing, or not kinds of contact with their audiences.".to_string());
    };
    object
        .iter()
        .map(|(k, v)| {
            let kind = Kind::from_word(k)
                .ok_or_else(|| format!("\"{}\" is not a kind of contact this server sets (message, call, trade).", echo(k)))?;
            let audience = v.as_str().and_then(Audience::from_word).ok_or_else(|| {
                format!("\"{}\" is not one of nobody, chosen, friends, groups, anyone.", echo(&v.to_string()))
            })?;
            Ok((kind.word(), audience.word()))
        })
        .collect()
}

/// `reach_set {settings}` from the signed-in `my_key`: save the kinds it names. An unknown kind
/// or audience refuses the whole set with a notice and changes nothing. Every `reach_set` is
/// answered with `reach_settings` to all of the person's sockets, whether it changed anything or
/// not, so every device shows what the relay will enforce.
pub async fn handle_reach_set(state: &Arc<RelayState>, my_key: &str, raw: &serde_json::Value) {
    match parse_settings(raw.get("settings")) {
        Ok(rows) => {
            if let Err(e) = state.db.set_reach_settings(my_key, &rows) {
                tracing::error!("reach settings: could not save: {e}");
                tell(state, my_key, "Who can reach you was not saved: the server could not write it. Try again.".to_string());
            }
        }
        Err(why) => tell(state, my_key, format!("Who can reach you was not changed: {why}")),
    }
    let _ = state.broadcast_tx.send(settings_message(state, my_key));
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;

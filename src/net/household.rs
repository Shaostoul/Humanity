//! Household permits on the desktop (ship homes increment 5b, 2026-10-12;
//! docs/design/ship-homes-increment-5b-plan.md, with the orchestrator's amendments in its section
//! 8): the permit note, what is kept of permits held and given, the rules for keeping them, the
//! session's state, and the lengths a permit may be given for. No socket and no GUI, so every
//! rule has a unit test. What acts on the app is src/engine/household.rs; what is stored is the DM
//! store's (net/dm_store.rs `permits_held`, `permits_given`); the contract is
//! systems/construction/shared.rs `Permit`, and the signed words are
//! relay/core/pq_crypto.rs `plot_permit_preimage`.
//!
//! WHAT A PERMIT IS. A plot's holder lets one friend build on their plot on one server, and walk
//! into their home there, until a date at most 90 days ahead. It names a random serial, so the
//! holder can take it back sooner (amendment A1: `cert_revoke {serial}`, the withdrawal list
//! friendship passes use; Block takes back every permit given to the blocked person). The relay
//! keeps no permit: the grantee keeps it, on their devices, and shows it with each build there.
//!
//! HOW ONE TRAVELS. As a sealed DM whose text is [`CTL_PERMIT`] followed by the permit's JSON
//! ([`note_text`]), so the DM's own signature covers it. A copy also goes to the giver's own
//! mailbox, so their other devices list it as given. The website has no game: it swallows the
//! note and shows nothing.
//!
//! WHO A PERMIT IS KEPT FROM ([`judge_arrival`]): only from the friend it names as its issuer, the
//! DM's checked sender (a permit forwarded by someone else is dropped), only for this server
//! (the DM store's server identity), only when it names me as grantee, and only when its words
//! check out against those facts and the clock. The relay checks it again, by its own facts and
//! against who holds the plot now, before any door opens (`game_permit_check`).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::dm_pq::CTL_PERMIT;
use crate::relay::core::pq_crypto::{friend_cert_serial_ok, verify_plot_permit, PlotPermitError, PLOT_PERMIT_FIELD_MAX_LEN};
use crate::build_permission::{Permit, Why, PERMIT_MAX_DAYS};

/// Permits held (from friends) kept per server at most. Past it, the earliest-ended go first.
pub const HELD_MAX: usize = 64;
/// Permits given kept per server at most. Past it, the earliest-ended go first, and one that
/// still stands is taken back as it goes, so no permit is ever out that this device forgot.
pub const GIVEN_MAX: usize = 256;
/// The longest issuer a permit note may carry, characters: a Dilithium3 key is 3,904 in hex.
pub const ISSUER_MAX_LEN: usize = 8_192;
/// The longest signature a permit note may carry, characters: a Dilithium3 signature is about
/// 4,400 in base64.
pub const SIG_MAX_LEN: usize = 8_192;
/// The longest permit JSON a note may carry after its marker, bytes. A real one is about 8.4 KB.
pub const NOTE_BODY_MAX_LEN: usize = 16_384;
/// Where the lengths a permit may be given for live, under data/.
pub const LENGTHS_FILE: &str = "ship/household.ron";

// ── The note ──────────────────────────────────────────────────────────────

/// The text of the DM that gives `p`: the marker, then the permit's JSON in its field order
/// (issuer, server, plot, grantee, serial, expiry, sig), nothing between.
pub fn note_text(p: &Permit) -> String {
    format!("{CTL_PERMIT}{}", serde_json::to_string(p).unwrap_or_default())
}

/// Is `text` a permit note? Only "starts with the marker", so one that does not read is still a
/// note: it is consumed, never stored or shown as a message.
pub fn is_note(text: &str) -> bool {
    text.starts_with(CTL_PERMIT)
}

/// The permit a note carries, or None when it does not read as one: not the JSON of a permit, a
/// body over [`NOTE_BODY_MAX_LEN`], an issuer that is not hex or over [`ISSUER_MAX_LEN`], a
/// signature empty or over [`SIG_MAX_LEN`], a server, plot or grantee id that is empty, over
/// `PLOT_PERMIT_FIELD_MAX_LEN` or holds a line break, or a serial that is not 16 bytes of
/// lowercase hex. Whether it checks out is [`judge_arrival`]'s question.
pub fn parse_note(text: &str) -> Option<Permit> {
    let body = text.strip_prefix(CTL_PERMIT)?;
    if body.len() > NOTE_BODY_MAX_LEN {
        return None;
    }
    let p: Permit = serde_json::from_str(body).ok()?;
    let id_ok = |s: &str| !s.is_empty() && s.len() <= PLOT_PERMIT_FIELD_MAX_LEN && !s.contains(['\n', '\r']);
    let issuer_ok = !p.issuer.is_empty() && p.issuer.len() <= ISSUER_MAX_LEN && p.issuer.bytes().all(|b| b.is_ascii_hexdigit());
    let fits = issuer_ok
        && !p.sig.is_empty()
        && p.sig.len() <= SIG_MAX_LEN
        && [&p.server, &p.plot, &p.grantee].into_iter().all(|s| id_ok(s))
        && friend_cert_serial_ok(&p.serial);
    fits.then_some(p)
}

// ── Who a permit is kept from ─────────────────────────────────────────────

/// Why a permit that arrived is not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// Its issuer is not the DM's sender: someone forwarded a permit another person gave.
    NotFromItsIssuer,
    /// This device does not know the server's identity yet (mail is fetched after the server's
    /// `identify_challenge` names it, so this is not seen in practice).
    NoServerYet,
    /// It was given on another server.
    OtherServer,
    /// It names someone else as grantee.
    NotForMe,
    /// It has run out.
    RanOut,
    /// It runs more than 90 days ahead.
    TooLong,
    /// Its signature, or its serial, does not check out.
    DoesNotCheckOut,
}

/// Keep `p`, which arrived in a DM from `sender` (the DM's checked sender, a Dilithium3 key in
/// hex)? `pass_server` is the server identity this DM store holds (`DmStore::pass_server`),
/// `my_did` my own `did:hum:` as the relay holds plots under it (`plot_owner_id` of my key), and
/// `now_s` my clock. The checks, in order: the issuer is the sender, the server is known and is
/// this one, I am the grantee, and the words check out (the signature is checked against the
/// SENDER's key and these facts, never anything else the permit claims).
pub fn judge_arrival(p: &Permit, sender: &str, pass_server: Option<&str>, my_did: &str, now_s: u64) -> Result<(), Arrival> {
    if !p.issuer.eq_ignore_ascii_case(sender) {
        return Err(Arrival::NotFromItsIssuer);
    }
    let Some(server) = pass_server.filter(|s| !s.is_empty()) else { return Err(Arrival::NoServerYet) };
    if p.server != server {
        return Err(Arrival::OtherServer);
    }
    if p.grantee != my_did {
        return Err(Arrival::NotForMe);
    }
    verify_plot_permit(sender, server, &p.plot, my_did, &p.serial, p.expiry, &p.sig, now_s).map_err(|e| match e {
        PlotPermitError::Expired => Arrival::RanOut,
        PlotPermitError::TooLong => Arrival::TooLong,
        PlotPermitError::Malformed | PlotPermitError::BadSerial | PlotPermitError::BadSignature => Arrival::DoesNotCheckOut,
    })
}

// ── What is kept ──────────────────────────────────────────────────────────

/// A permit a friend gave me, as the DM store keeps it (per server).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldPermit {
    /// The giver's key, hex: the DM's checked sender, which is the permit's issuer.
    pub from: String,
    pub permit: Permit,
    /// When it was given: the DM's signed time, ms.
    pub at: u64,
}

/// A permit I gave a friend, as the DM store keeps it (per server), one row per serial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GivenPermit {
    /// The friend's key, hex.
    pub to: String,
    /// My plot it is for, by its ship-file id.
    pub plot: String,
    /// Its serial: what taking it back names.
    pub serial: String,
    /// When it runs out, Unix seconds.
    pub expiry: u64,
    /// When it was given, ms.
    pub at: u64,
    /// Where it stands.
    #[serde(flatten)]
    pub state: GivenState,
}

/// Where a permit I gave stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GivenState {
    /// Sent, under `reference` (the `ref` its `dm_put` carries); no answer yet. One read after a
    /// restart, when that send is no longer in memory, is [`GivenState::Unconfirmed`]
    /// ([`Household::given_state`]).
    OnItsWay { reference: String },
    /// The server took it (`dm_put_ok`): it waits in the friend's mailbox, or they have it.
    Taken,
    /// No answer came: the friend may or may not have it.
    Unconfirmed,
    /// The server did not take it (`dm_put_refused {reason}`): the friend never got it.
    Refused { reason: String },
    /// I took it back (`cert_revoke`), here or on another of my devices. Whether the relay has
    /// confirmed it yet is whether its serial is still among the store's waiting withdrawals.
    TakenBack,
}

impl GivenState {
    /// Might the friend hold this permit now, so that it must be taken back to end it?
    pub fn may_stand(&self) -> bool {
        matches!(self, GivenState::OnItsWay { .. } | GivenState::Taken | GivenState::Unconfirmed)
    }
}

/// What [`keep_held`] did with a permit that arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// The first from that giver for that plot.
    New,
    /// It replaced an earlier one from that giver for that plot.
    Replaced,
    /// An earlier (or the same) one: nothing changed.
    Unchanged,
}

/// Is `a` given later than `b`: the later signed time, then the later end date, then the larger
/// serial (so two devices order any pair the same way)?
fn later(a: &HeldPermit, b: &HeldPermit) -> bool {
    (a.at, a.permit.expiry, &a.permit.serial) > (b.at, b.permit.expiry, &b.permit.serial)
}

/// Keep `h` in `held`: one per (giver, plot), the one given LATER winning, so a renewal replaces
/// the permit it renews and an older note read late (from the mailbox after a live one) changes
/// nothing. Past [`HELD_MAX`] the earliest-ended go.
pub fn keep_held(held: &mut Vec<HeldPermit>, h: HeldPermit) -> Kept {
    let kept = match held.iter_mut().find(|k| k.from.eq_ignore_ascii_case(&h.from) && k.permit.plot == h.permit.plot) {
        Some(k) if later(&h, k) => {
            *k = h;
            Kept::Replaced
        }
        Some(_) => Kept::Unchanged,
        None => {
            held.push(h);
            Kept::New
        }
    };
    while held.len() > HELD_MAX {
        let Some(first) = held.iter().enumerate().min_by_key(|(_, k)| k.permit.expiry).map(|(i, _)| i) else { break };
        held.remove(first);
    }
    kept
}

/// Record `g` in `given`, one row per serial: a row already there with that serial takes `g`'s
/// state (an answer, or the echo from my other device), except that a permit taken back stays
/// taken back. Past [`GIVEN_MAX`] the earliest-ended go first; the serials of any that might
/// still stand at `now_s` are returned, for the caller to take back. Returns (new row, serials to
/// take back).
pub fn record_given(given: &mut Vec<GivenPermit>, g: GivenPermit, now_s: u64) -> (bool, Vec<String>) {
    if let Some(row) = given.iter_mut().find(|r| r.serial == g.serial) {
        if row.state != GivenState::TakenBack {
            *row = g;
        }
        return (false, Vec::new());
    }
    given.push(g);
    let mut gone = Vec::new();
    while given.len() > GIVEN_MAX {
        // Rows that no longer stand go before any that might, then the earliest-ended.
        let Some(first) = given.iter().enumerate().min_by_key(|(_, r)| (r.state.may_stand() && r.expiry > now_s, r.expiry)).map(|(i, _)| i) else {
            break;
        };
        let row = given.remove(first);
        if row.state.may_stand() && row.expiry > now_s {
            gone.push(row.serial);
        }
    }
    (true, gone)
}

/// Set the row with `serial` to `state`. False when there is none, or when it was taken back (a
/// late answer never brings a permit back).
pub fn set_given_state(given: &mut [GivenPermit], serial: &str, state: GivenState) -> bool {
    match given.iter_mut().find(|r| r.serial == serial) {
        Some(row) if row.state != GivenState::TakenBack || state == GivenState::TakenBack => {
            row.state = state;
            true
        }
        _ => false,
    }
}

/// Take back every permit given to `to` that might stand and that `keep` does not keep (Take
/// back: keep the other plots; Block: keep none; a renewal the server took: keep the new serial):
/// each row becomes [`GivenState::TakenBack`], and their serials are returned for the relay.
pub fn take_back(given: &mut [GivenPermit], to: &str, keep: impl Fn(&GivenPermit) -> bool) -> Vec<String> {
    let mut serials = Vec::new();
    for row in given.iter_mut().filter(|r| r.to.eq_ignore_ascii_case(to) && r.state.may_stand()) {
        if !keep(row) {
            row.state = GivenState::TakenBack;
            serials.push(row.serial.clone());
        }
    }
    serials
}

/// Drop every permit, held or given, that ended more than `kept_days` days before `now_s` (the
/// lists say "ran out on <date>" until then).
pub fn prune(held: &mut Vec<HeldPermit>, given: &mut Vec<GivenPermit>, now_s: u64, kept_days: u64) {
    let cutoff = now_s.saturating_sub(kept_days.saturating_mul(86_400));
    held.retain(|h| h.permit.expiry >= cutoff);
    given.retain(|g| g.expiry >= cutoff);
}

// ── This session ──────────────────────────────────────────────────────────

/// Which held permit a verdict is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PermitKey {
    pub from: String,
    pub plot: String,
    pub serial: String,
}

impl PermitKey {
    pub fn of(h: &HeldPermit) -> PermitKey {
        PermitKey { from: h.from.clone(), plot: h.permit.plot.clone(), serial: h.permit.serial.clone() }
    }
}

/// What the relay said of a held permit this session (`game_permit_status`, or a build's
/// refusal with a `permit_*` why).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Asked; no answer yet.
    Checking,
    /// It lets me build on that plot now.
    Works,
    /// It does not, for this reason.
    Refused(Why),
    /// The relay could not judge it this time; asked again later.
    Later,
}

/// A verdict, and when it was given (the game's clock, seconds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Judged {
    pub verdict: Verdict,
    pub at: f64,
}

/// A permit on its way to a friend: its `dm_put` went out under `reference`, and the self-copy
/// for my other devices waits here until the server took it (the 10l rule).
#[derive(Debug, Clone)]
pub struct PermitPut {
    pub reference: String,
    pub peer: String,
    pub plot: String,
    pub serial: String,
    pub expiry: u64,
    /// The length it was given for, days (a renewal gives the same length again).
    pub days: u64,
    /// The `dm_put` of the copy to my own mailbox, sent once the server took theirs.
    pub self_copy: serde_json::Value,
    pub at: std::time::Instant,
}

/// The household's state this session, never saved (`GuiState::household`): which plot the
/// server gave me, what it said of each permit I hold, which plots are open to me, and the sends
/// on their way. A server switch or leaving the shared world clears it.
#[derive(Debug, Clone, Default)]
pub struct Household {
    /// The plot this session's welcome gave me; None before one, or as a guest.
    pub my_plot: Option<String>,
    /// This session's welcome has arrived, with a plot or without one (2026-10-12): until it
    /// does, the Household card says it is joining rather than that the player is a guest.
    pub welcomed: bool,
    /// What the relay said of each held permit this session.
    pub verdicts: BTreeMap<PermitKey, Judged>,
    /// Plots I may build on now: [`Household::usable_plots`].
    pub usable: BTreeSet<String>,
    /// Plots I may walk into now (the usable ones, and one I still stand in).
    pub walkable: BTreeSet<String>,
    /// The open plot I stand in, whose lights are on.
    pub lit: Option<String>,
    /// Checks asked and not answered: `req_id` to the permit and when (the game's clock, seconds).
    pub asked: BTreeMap<u32, (PermitKey, f64)>,
    /// The last `req_id` used.
    pub last_req: u32,
    /// When the next round of checks is due (the game's clock, seconds).
    pub next_check_at: f64,
    /// Permits sent and not yet answered.
    pub on_their_way: Vec<PermitPut>,
    /// The page's last result, in words.
    pub line: String,
}

impl Household {
    /// What the relay said this session of `h`, if anything.
    pub fn verdict(&self, h: &HeldPermit) -> Option<Verdict> {
        self.verdicts.get(&PermitKey::of(h)).map(|j| j.verdict)
    }

    /// The plots `held` lets me build on now: each permit whose plot is a plot of this ship
    /// (`is_ship_plot`) and not my own, that has not run out at `now_s`, and that the relay said
    /// works this session.
    pub fn usable_plots(&self, held: &[HeldPermit], is_ship_plot: impl Fn(&str) -> bool, now_s: u64) -> BTreeSet<String> {
        held.iter()
            .filter(|h| self.my_plot.as_deref() != Some(h.permit.plot.as_str()))
            .filter(|h| is_ship_plot(&h.permit.plot) && h.permit.expiry > now_s)
            .filter(|h| self.verdict(h) == Some(Verdict::Works))
            .map(|h| h.permit.plot.clone())
            .collect()
    }

    /// Where `g` stands as the page shows it: one on its way whose send is no longer in memory
    /// (the app was closed before the answer) is unconfirmed.
    pub fn given_state(&self, g: &GivenPermit) -> GivenState {
        match &g.state {
            GivenState::OnItsWay { reference } if !self.on_their_way.iter().any(|p| &p.reference == reference) => GivenState::Unconfirmed,
            other => other.clone(),
        }
    }
}

// ── Dates ─────────────────────────────────────────────────────────────────

/// The month names a date is written with (English, as every other word on the page is).
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// A Unix time as the page writes a date: "10 November 2026" (UTC).
pub fn date_words(unix_s: u64) -> String {
    let (y, m, d) = crate::civil_date::epoch_days_to_ymd((unix_s / 86_400) as i64);
    format!("{d} {} {y}", MONTHS[(m as usize).clamp(1, 12) - 1])
}

// ── The lengths a permit may be given for ─────────────────────────────────

/// data/ship/household.ron: the lengths the page offers, the one picked first, and how long an
/// ended permit stays listed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Lengths {
    /// The choices, days, smallest first: each 1 to 90.
    pub lengths_days: Vec<u64>,
    /// The one picked first: one of `lengths_days`.
    pub default_days: u64,
    /// How long a permit that ran out (or was taken back) stays on the lists, days.
    pub ended_kept_days: u64,
}

impl Lengths {
    /// Read the file's text, refusing one that breaks the rules ([`Lengths::check`]), so the
    /// copy built into the exe is used instead (embedded_data `load_data_or_embedded`).
    pub fn parse(bytes: &[u8]) -> Result<Lengths, String> {
        let l: Lengths = ron::de::from_bytes(bytes).map_err(|e| e.to_string())?;
        l.check()?;
        Ok(l)
    }

    /// The rules: at least one length, each 1 to [`PERMIT_MAX_DAYS`] (no permit runs longer),
    /// smallest first with none twice, the default one of them, and ended permits kept 1 to 365
    /// days.
    pub fn check(&self) -> Result<(), String> {
        if self.lengths_days.is_empty() {
            return Err("lengths_days lists no length".into());
        }
        if let Some(d) = self.lengths_days.iter().find(|d| !(1..=PERMIT_MAX_DAYS).contains(*d)) {
            return Err(format!("a length of {d} days is not 1 to {PERMIT_MAX_DAYS}: no permit runs longer than {PERMIT_MAX_DAYS} days"));
        }
        if self.lengths_days.windows(2).any(|w| w[0] >= w[1]) {
            return Err(format!("lengths_days {:?} is not smallest first with none twice", self.lengths_days));
        }
        if !self.lengths_days.contains(&self.default_days) {
            return Err(format!("default_days {} is not one of lengths_days {:?}", self.default_days, self.lengths_days));
        }
        if !(1..=365).contains(&self.ended_kept_days) {
            return Err(format!("ended_kept_days {} is not 1 to 365", self.ended_kept_days));
        }
        Ok(())
    }
}

/// The lengths from `data_dir` (data/ship/household.ron), else the copy built into the exe.
pub fn load_lengths(data_dir: &std::path::Path) -> Lengths {
    crate::embedded_data::load_data_or_embedded(data_dir, LENGTHS_FILE, Lengths::parse).unwrap_or_else(|e| {
        // Not reached: the built-in copy is held to the rules by a test. One length, the most
        // often given, rather than none.
        log::error!("Household: {e}; permits are offered for 30 days only");
        Lengths { lengths_days: vec![30], default_days: 30, ended_kept_days: 30 }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::pq_crypto::{build_plot_permit, derive_dilithium_seed, plot_permit_preimage, DilithiumKeypair};
    use crate::relay::storage::plot_owner_id;

    /// "Now" for these tests, Unix seconds.
    const NOW: u64 = 1_760_000_000;
    const DAY: u64 = 86_400;
    /// The server these permits are given on.
    const SERVER: &str = "did:hum:ThisServer11111111111";

    /// The key (Dilithium3, hex) of the person whose seed is `[n; 32]`.
    fn key(n: u8) -> String {
        hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[n; 32])).public_key())
    }

    /// Their `did:hum:`, as the relay holds plots under it.
    fn did(n: u8) -> String {
        plot_owner_id(&key(n))
    }

    /// The permit `giver` mints on `server` for `grantee` on `plot`, running out at `expiry`,
    /// minted at `minted`, named `serial`.
    fn permit_on(server: &str, giver: u8, plot: &str, grantee: u8, serial: &str, expiry: u64, minted: u64) -> Permit {
        let sig = build_plot_permit(&[giver; 32], server, plot, &did(grantee), serial, expiry, minted).expect("minted");
        Permit { issuer: key(giver), server: server.into(), plot: plot.into(), grantee: did(grantee), serial: serial.into(), expiry, sig }
    }

    const SERIAL: &str = "00112233445566778899aabbccddeeff";

    /// [`permit_on`] this server, minted now, named [`SERIAL`].
    fn permit(giver: u8, plot: &str, grantee: u8, expiry: u64) -> Permit {
        permit_on(SERVER, giver, plot, grantee, SERIAL, expiry, NOW)
    }

    /// A permit for the keeping rules, which never look at its signature.
    fn unsigned(from: &str, plot: &str, serial: &str, expiry: u64, at: u64) -> HeldPermit {
        let permit = Permit {
            issuer: from.into(),
            server: SERVER.into(),
            plot: plot.into(),
            grantee: "did:hum:me".into(),
            serial: serial.into(),
            expiry,
            sig: "c2ln".into(),
        };
        HeldPermit { from: from.into(), permit, at }
    }

    /// A given row for the keeping rules.
    fn given(to: &str, plot: &str, serial: &str, expiry: u64, state: GivenState) -> GivenPermit {
        GivenPermit { to: to.into(), plot: plot.into(), serial: serial.into(), expiry, at: 1, state }
    }

    /// THE PERMIT NOTE (plan section 3.1). Its text is exactly the marker `[[hum:permit:v1]]`
    /// followed by the permit's JSON in the order its words are signed; it reads back as the same
    /// permit, about 8.6 KB of it (padded to the 16,384 bucket). Each malformed one (not JSON,
    /// not a permit, an issuer not hex or over the cap, a signature over the cap, an empty plot, a
    /// line break in an id, a grantee over the cap, a serial in capitals, a body over the cap) is
    /// read as no permit, yet is still a note, so it is never stored or shown as a message. A
    /// text that does not START with the marker is not a note.
    /// Seen red 2026-10-12 with the marker spelt `[[hum:permit]]` in dm_pq.rs: "the marker, then
    /// the permit's JSON" failed on `"[[hum:permit]]{\"issuer\":\"da0280f47df7e04"`.
    #[test]
    fn the_permit_note_round_trips_and_a_bad_one_is_still_a_note() {
        let p = permit(1, "p1", 2, NOW + 30 * DAY);
        let text = note_text(&p);
        assert!(text.starts_with("[[hum:permit:v1]]{\"issuer\":\""), "the marker, then the permit's JSON: {:?}", &text[..40]);
        let order: Vec<usize> = ["\"issuer\":", "\"server\":", "\"plot\":", "\"grantee\":", "\"serial\":", "\"expiry\":", "\"sig\":"]
            .iter()
            .map(|k| text.find(k).unwrap_or_else(|| panic!("{k} in the note")))
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "the fields in the order their words are signed: {order:?}");
        assert!((8_000..NOTE_BODY_MAX_LEN).contains(&text.len()), "a real note is about 8.4 KB: {}", text.len());
        assert!(is_note(&text));
        assert_eq!(parse_note(&text), Some(p.clone()), "it reads back as the permit");

        let with = |f: &dyn Fn(&mut Permit)| {
            let mut q = p.clone();
            f(&mut q);
            note_text(&q)
        };
        let bad: Vec<(&str, String)> = vec![
            ("not JSON", format!("{CTL_PERMIT}{{not json")),
            ("JSON that is not a permit", format!("{CTL_PERMIT}{{\"issuer\":\"aa\"}}")),
            ("an issuer that is not hex", with(&|q| q.issuer = format!("zz{}", &q.issuer[2..]))),
            ("an issuer over the cap", with(&|q| q.issuer = "a".repeat(ISSUER_MAX_LEN + 1))),
            ("a signature over the cap", with(&|q| q.sig = "A".repeat(SIG_MAX_LEN + 1))),
            ("an empty plot", with(&|q| q.plot = String::new())),
            ("a line break in the plot", with(&|q| q.plot = "p1\np2".into())),
            ("a grantee over the cap", with(&|q| q.grantee = "d".repeat(PLOT_PERMIT_FIELD_MAX_LEN + 1))),
            ("a serial in capitals", with(&|q| q.serial = SERIAL.to_uppercase())),
            ("a body over the cap", with(&|q| {
                q.issuer = "a".repeat(ISSUER_MAX_LEN);
                q.sig = "A".repeat(SIG_MAX_LEN);
            })),
        ];
        for (what, text) in &bad {
            assert!(is_note(text), "{what}: still a note");
            assert_eq!(parse_note(text), None, "{what}: no permit");
        }
        for not in ["hello", " [[hum:permit:v1]]{}", "[[hum:choice:v1]]abc/invite", "[[hum:permit:v2]]{}"] {
            assert!(!is_note(not) && parse_note(not).is_none(), "{not:?} is not a permit note");
        }
    }

    /// WHO A PERMIT IS KEPT FROM (plan section 3.1). I am Bo; Ada gives me a 30-day permit for
    /// her p1 on this server, and it is kept. Not kept, each for its own reason: the same permit
    /// forwarded by Cy (`NotFromItsIssuer`), one Ada gave on another server (`OtherServer`), one
    /// she gave Cy (`NotForMe`), one that ran out (`RanOut`), one signed for 92 days (`TooLong`),
    /// one whose signature is not one and one whose serial is not the one signed
    /// (`DoesNotCheckOut`), and any permit before this device knows the server's identity
    /// (`NoServerYet`).
    /// Seen red 2026-10-12 with the issuer-equals-sender line removed from `judge_arrival`:
    /// "a permit Cy forwarded from Ada: Err(DoesNotCheckOut), expected Err(NotFromItsIssuer)" (the
    /// signature, checked against the sender's key, still refused it).
    #[test]
    fn a_permit_is_kept_only_from_its_giver_for_me_on_this_server() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let (ada_key, me, cy_key) = (key(1), did(2), key(3));
        let (ada, cy) = (ada_key.as_str(), cy_key.as_str());
        let good = permit(1, "p1", 2, NOW + 30 * DAY);
        let too_long = {
            // Signed by hand, as a modified app could: the minting never makes one past 90 days.
            let mut p = good.clone();
            p.expiry = NOW + 92 * DAY;
            let signer = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[1; 32]));
            p.sig = B64.encode(signer.sign(plot_permit_preimage(SERVER, "p1", &me, SERIAL, p.expiry).as_bytes()));
            p
        };
        let rows: Vec<(&str, Permit, &str, Option<&str>, Result<(), Arrival>)> = vec![
            ("Ada's permit, from Ada", good.clone(), ada, Some(SERVER), Ok(())),
            ("a permit Cy forwarded from Ada", good.clone(), cy, Some(SERVER), Err(Arrival::NotFromItsIssuer)),
            (
                "a permit Ada gave on another server",
                permit_on("did:hum:AnotherServer1111111", 1, "p1", 2, SERIAL, NOW + DAY, NOW),
                ada,
                Some(SERVER),
                Err(Arrival::OtherServer),
            ),
            ("a permit Ada gave Cy", permit(1, "p1", 3, NOW + DAY), ada, Some(SERVER), Err(Arrival::NotForMe)),
            ("a permit that ran out", permit_on(SERVER, 1, "p1", 2, SERIAL, NOW - DAY, NOW - 10 * DAY), ada, Some(SERVER), Err(Arrival::RanOut)),
            ("a permit signed for 92 days", too_long, ada, Some(SERVER), Err(Arrival::TooLong)),
            ("a permit whose signature is not one", Permit { sig: "bm90LWEtc2ln".into(), ..good.clone() }, ada, Some(SERVER), Err(Arrival::DoesNotCheckOut)),
            (
                "a permit whose serial is not the one signed",
                Permit { serial: "ffeeddccbbaa99887766554433221100".into(), ..good.clone() },
                ada,
                Some(SERVER),
                Err(Arrival::DoesNotCheckOut),
            ),
            ("Ada's permit before the server is known", good, ada, None, Err(Arrival::NoServerYet)),
        ];
        let mut wrong = Vec::new();
        for (what, p, sender, server, want) in &rows {
            let got = judge_arrival(p, sender, *server, &me, NOW);
            if got != *want {
                wrong.push(format!("{what}: {got:?}, expected {want:?}"));
            }
        }
        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }

    /// ONE PERMIT PER GIVER AND PLOT, THE ONE GIVEN LATER WINNING (plan section 3.2). Ada's
    /// first permit for p1 is new; one she gave EARLIER, read late from the mailbox, changes
    /// nothing even though it runs longer; her renewal replaces the first; the renewal again
    /// changes nothing; her p2 and Cy's p1 each get a row of their own. Past 64 rows the
    /// earliest-ended go.
    /// Seen red 2026-10-12 with the comparison in `keep_held` reversed (`later(k, &h)`): "an
    /// earlier permit read late" left `Replaced`, right `Unchanged`.
    #[test]
    fn a_later_permit_replaces_and_an_earlier_one_changes_nothing() {
        let mut held = Vec::new();
        let first = unsigned("ada", "p1", "11111111111111111111111111111111", NOW + 30 * DAY, 2_000);
        assert_eq!(keep_held(&mut held, first.clone()), Kept::New, "the first");
        let earlier = unsigned("ada", "p1", "22222222222222222222222222222222", NOW + 90 * DAY, 1_000);
        assert_eq!(keep_held(&mut held, earlier), Kept::Unchanged, "an earlier permit read late");
        assert_eq!(held, vec![first.clone()], "the first still stands");
        let renewal = unsigned("ada", "p1", "33333333333333333333333333333333", NOW + 60 * DAY, 3_000);
        assert_eq!(keep_held(&mut held, renewal.clone()), Kept::Replaced, "the renewal");
        assert_eq!(keep_held(&mut held, renewal.clone()), Kept::Unchanged, "the renewal again");
        assert_eq!(held, vec![renewal.clone()]);
        assert_eq!(keep_held(&mut held, unsigned("ada", "p2", SERIAL, NOW + DAY, 4_000)), Kept::New, "her other plot");
        assert_eq!(keep_held(&mut held, unsigned("cy", "p1", SERIAL, NOW + DAY, 4_000)), Kept::New, "Cy's p1");
        assert_eq!(held.len(), 3);

        let mut many = Vec::new();
        for i in 0..(HELD_MAX as u64 + 6) {
            keep_held(&mut many, unsigned(&format!("giver{i}"), "p1", SERIAL, NOW + (i + 1) * DAY, 1));
        }
        assert_eq!(many.len(), HELD_MAX);
        assert!(many.iter().all(|h| h.permit.expiry > NOW + 6 * DAY), "the earliest-ended went");
    }

    /// A PERMIT TAKEN BACK STAYS TAKEN BACK (amendment A1). Given rows are one per serial: an
    /// answer for a row updates it, a renewal under a new serial is a row of its own, and taking
    /// back the permits to Bo (keeping the renewal) marks the old one taken back and names its
    /// serial for the relay, leaving the refused one alone (it never reached him). Afterwards a
    /// late answer or an echo of the old one changes nothing. Past 256 rows the earliest-ended go
    /// first, ended ones before any that may stand, and one that may still stand is named for
    /// taking back as it goes.
    /// Seen red 2026-10-12 with the taken-back guard removed from `record_given`: "a late echo of
    /// the old permit" left `Taken`, right `TakenBack`.
    #[test]
    fn a_permit_taken_back_stays_taken_back() {
        let mut list = Vec::new();
        let old = "11111111111111111111111111111111";
        let renewal = "22222222222222222222222222222222";
        let refused = "33333333333333333333333333333333";
        let on_its_way = GivenState::OnItsWay { reference: "r1".into() };
        assert_eq!(record_given(&mut list, given("bo", "p2", old, NOW + DAY, on_its_way), NOW), (true, vec![]));
        assert_eq!(record_given(&mut list, given("bo", "p2", old, NOW + DAY, GivenState::Taken), NOW), (false, vec![]), "its answer");
        assert!(set_given_state(&mut list, old, GivenState::Taken));
        record_given(&mut list, given("bo", "p2", renewal, NOW + 30 * DAY, GivenState::Taken), NOW);
        record_given(&mut list, given("bo", "p2", refused, NOW + 30 * DAY, GivenState::Refused { reason: "reach".into() }), NOW);
        assert_eq!(list.len(), 3, "one row per serial");

        assert_eq!(take_back(&mut list, "bo", |g| g.serial == renewal), vec![old.to_string()], "the old one, not the refused one");
        let state = |l: &[GivenPermit], s: &str| l.iter().find(|g| g.serial == s).map(|g| g.state.clone());
        assert_eq!(state(&list, old), Some(GivenState::TakenBack));
        assert_eq!(state(&list, renewal), Some(GivenState::Taken), "the renewal is kept");
        assert!(!set_given_state(&mut list, old, GivenState::Taken), "a late answer is refused");
        record_given(&mut list, given("bo", "p2", old, NOW + DAY, GivenState::Taken), NOW);
        assert_eq!(state(&list, old), Some(GivenState::TakenBack), "a late echo of the old permit");
        assert!(take_back(&mut list, "bo", |_| false).contains(&renewal.to_string()), "Block takes back the rest");

        let mut full = Vec::new();
        let serial = |i: usize| format!("{i:032x}");
        // One ended long ago, one standing that ends soonest, then many standing.
        record_given(&mut full, given("ann", "p2", &serial(0), NOW - 5 * DAY, GivenState::Taken), NOW);
        record_given(&mut full, given("ann", "p2", &serial(1), NOW + DAY, GivenState::Taken), NOW);
        for i in 2..GIVEN_MAX {
            record_given(&mut full, given("ann", "p2", &serial(i), NOW + 10 * DAY + i as u64, GivenState::Taken), NOW);
        }
        let (_, gone) = record_given(&mut full, given("ann", "p2", &serial(GIVEN_MAX), NOW + 90 * DAY, GivenState::Taken), NOW);
        assert_eq!((full.len(), gone.clone()), (GIVEN_MAX, vec![]), "the ended one went first, nothing to take back");
        let (_, gone) = record_given(&mut full, given("ann", "p2", &serial(GIVEN_MAX + 1), NOW + 90 * DAY, GivenState::Taken), NOW);
        assert_eq!(gone, vec![serial(1)], "then the standing one that ends soonest, named for taking back");
    }

    /// AN ENDED PERMIT STAYS LISTED FOR THE KEPT DAYS, THEN GOES (plan section 3.2), from both
    /// lists: with 30 days kept, one that ended 31 days ago goes, one that ended 29 days ago
    /// stays ("ran out on <date>"), and a standing one stays.
    /// Seen red 2026-10-12 with `prune` skipping the given list: "given" left `["31", "29",
    /// "live"]`, right `["29", "live"]`.
    #[test]
    fn ended_permits_leave_both_lists_after_the_kept_days() {
        let ends = [("31", NOW - 31 * DAY), ("29", NOW - 29 * DAY), ("live", NOW + DAY)];
        let mut held: Vec<HeldPermit> = ends.iter().map(|(from, e)| unsigned(from, "p1", SERIAL, *e, 1)).collect();
        let mut gave: Vec<GivenPermit> = ends.iter().map(|(to, e)| given(to, "p2", &format!("{:0>32}", to.len()), *e, GivenState::Taken)).collect();
        prune(&mut held, &mut gave, NOW, 30);
        assert_eq!(held.iter().map(|h| h.from.as_str()).collect::<Vec<_>>(), ["29", "live"], "held");
        assert_eq!(gave.iter().map(|g| g.to.as_str()).collect::<Vec<_>>(), ["29", "live"], "given");
    }

    /// THE LENGTHS ARE DATA, AND NONE RUNS PAST 90 DAYS (plan section 3.2). The data folder's
    /// data/ship/household.ron and the copy built into the exe both read, and are the same; each
    /// length is 1 to 90 days, smallest first, and the default is one of them. A copy with a
    /// 120-day length is refused, and a game whose data folder holds one offers the built-in
    /// lengths instead.
    /// Seen red 2026-10-12 with 120 added to data/ship/household.ron's lengths: "the data
    /// folder's copy: a length of 120 days is not 1 to 90: no permit runs longer than 90 days".
    #[test]
    fn the_lengths_are_data_and_none_runs_past_90_days() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let disk = Lengths::parse(&std::fs::read(data.join(LENGTHS_FILE)).expect("the data file"))
            .unwrap_or_else(|e| panic!("the data folder's copy: {e}"));
        let built_in = Lengths::parse(crate::embedded_data::get_embedded(LENGTHS_FILE).expect("built in").as_bytes())
            .unwrap_or_else(|e| panic!("the built-in copy: {e}"));
        assert_eq!(disk, built_in, "the built-in copy is the data folder's");
        assert!(disk.lengths_days.iter().all(|d| (1..=PERMIT_MAX_DAYS).contains(d)), "{:?}", disk.lengths_days);
        assert!(disk.lengths_days.windows(2).all(|w| w[0] < w[1]), "smallest first: {:?}", disk.lengths_days);
        assert!(disk.lengths_days.contains(&disk.default_days), "the default is one of them");
        assert_eq!(load_lengths(&data), disk, "loaded from the data folder");

        let dir = crate::test_temp::dir("household_lengths");
        let text = std::fs::read_to_string(data.join(LENGTHS_FILE)).unwrap();
        let longer = text.replacen("lengths_days: [", "lengths_days: [120, ", 1);
        assert_ne!(longer, text, "the copy really has a 120-day length");
        std::fs::create_dir_all(dir.join("ship")).unwrap();
        std::fs::write(dir.join(LENGTHS_FILE), &longer).unwrap();
        let refused = Lengths::parse(longer.as_bytes()).expect_err("a 120-day length is refused");
        assert!(refused.contains("120"), "{refused}");
        assert_eq!(load_lengths(&dir), built_in, "and the built-in lengths are offered instead");
    }

    /// A DATE AS THE PAGE WRITES IT: day, month name, year, in UTC.
    /// Seen red 2026-10-12 with the month taken one too far (`MONTHS[(m as usize).clamp(0, 11)]`):
    /// left "1 February 1970", right "1 January 1970".
    #[test]
    fn date_words_write_the_day_the_month_and_the_year() {
        assert_eq!(date_words(0), "1 January 1970");
        assert_eq!(date_words(1_767_225_600), "1 January 2026");
        assert_eq!(date_words(1_794_268_800 + 86_399), "10 November 2026", "the last second of the day");
        assert_eq!(date_words(1_794_268_800 - 1), "9 November 2026");
    }

    /// WHAT THIS SESSION MAY USE (plan sections 3.2 and 3.7). Of the permits held, only one the
    /// relay said works, for a plot of this ship that is not my own, and not run out, is usable:
    /// here p1. Not p3 (still being checked), p4 (refused), p5 (ran out since the check), p2 (my
    /// own plot) or p99 (not on this ship). And a given permit sent before a restart, whose send
    /// is no longer in memory, reads as unconfirmed; one still on its way reads as on its way.
    /// Seen red 2026-10-12 with the verdict filter removed from `usable_plots`: left `{"p1",
    /// "p3", "p4", "p6"}`, right `{"p1"}`.
    #[test]
    fn only_a_working_permit_for_a_plot_of_this_ship_is_usable() {
        let mut hh = Household { my_plot: Some("p2".into()), ..Default::default() };
        let rows = [
            ("p1", NOW + DAY, Some(Verdict::Works)),
            ("p3", NOW + DAY, Some(Verdict::Checking)),
            ("p4", NOW + DAY, Some(Verdict::Refused(Why::PermitNotFromHolder))),
            ("p5", NOW - 1, Some(Verdict::Works)),
            ("p2", NOW + DAY, Some(Verdict::Works)),
            ("p99", NOW + DAY, Some(Verdict::Works)),
            ("p6", NOW + DAY, None),
        ];
        let held: Vec<HeldPermit> = rows.iter().map(|(plot, expiry, _)| unsigned("ada", plot, SERIAL, *expiry, 1)).collect();
        for (h, (_, _, v)) in held.iter().zip(rows.iter()) {
            if let Some(v) = v {
                hh.verdicts.insert(PermitKey::of(h), Judged { verdict: *v, at: 0.0 });
            }
        }
        let on_ship = |p: &str| p != "p99";
        let usable = hh.usable_plots(&held, on_ship, NOW);
        assert_eq!(usable, BTreeSet::from(["p1".to_string()]));

        let gone = given("bo", "p2", SERIAL, NOW + DAY, GivenState::OnItsWay { reference: "before-restart".into() });
        assert_eq!(hh.given_state(&gone), GivenState::Unconfirmed, "its send is not in memory any more");
        hh.on_their_way.push(PermitPut {
            reference: "now".into(),
            peer: "bo".into(),
            plot: "p2".into(),
            serial: SERIAL.into(),
            expiry: NOW + DAY,
            days: 1,
            self_copy: serde_json::Value::Null,
            at: std::time::Instant::now(),
        });
        let sent = given("bo", "p2", SERIAL, NOW + DAY, GivenState::OnItsWay { reference: "now".into() });
        assert_eq!(hh.given_state(&sent), sent.state, "still on its way");
    }
}

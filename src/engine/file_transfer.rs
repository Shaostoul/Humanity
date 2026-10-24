//! Device-to-device big files, the app's half (docs/design/files-showcase.md, "Device-to-device
//! big files, exact" and "...: the shapes, exact", parts 3, 4 and 6; the operator's answer 10 of
//! 2026-10-11): offering a file, the notes as they arrive, the receiver's Accept and Decline,
//! asking the server for the forwarder's credentials, the one connection per person that carries
//! every transfer with them (net/file_link.rs), Pause, Resume and Cancel, and what each row says.
//!
//! THE FLOW, sender S, receiver R:
//! 1. S picks a file over their private file limit in a DM; the attach step offers to send it
//!    straight to R's device. On Send this app cleans it (the attach step's switch, as every file
//!    leaving the device is cleaned) and reads it for its hashes on a worker ([`offer`]), then
//!    mints the file pass and sends `[[hum:offer:v1]]` to R, with a self-copy for S's other
//!    devices ([`send_offer`]). The row says "Waiting for R to accept".
//! 2. R's app reads the note ([`screens_dm`]): an offer whose pass does not check is not listed.
//!    The row applies "Files in messages" first, then asks (net/file_offer.rs `question`). Nothing
//!    is sent back for an offer left unanswered: S's row ends "R did not answer in time".
//! 3. R accepts ([`accept`]): the part file is made in R's downloads folder and
//!    `[[hum:offer-answer:v1]]{"accept":true}` goes back (and to R's other devices, which say
//!    "Answered on your other device"). Decline sends `accept:false`.
//! 4. Whenever R has a transfer from S accepted and not paused, S is online and no connection
//!    stands, R's app dials: it asks for one transfer's forwarder credentials with its pass
//!    (`{"type":"call_credentials","file","peer","file_pass"}`) and starts the connection, whose
//!    `dc_offer` carries that pass ([`pump`]).
//! 5. The relay hands R's `dc_offer` to S with the pass's id as `file` (rule 7 below). S's app
//!    ([`on_signal`]) asks for that one transfer's credentials and answers; each side holds ONE
//!    allocation for the connection. Every transfer between the two then rides that one
//!    connection, each on its own channel (`file:<id>`), opened by its receiver. If both dial at
//!    once, the larger key wins.
//!
//! Either side may Cancel (on the channel when it is open, and always with the cancel note, so the
//! other side's row says so even when no connection stands), Pause and Resume.
//!
//! THE RULES SETTLED BY THE FIRST REVIEW (files-showcase.md, 2026-10-12) that live here:
//! - Rule 3: while S waits for its credentials to answer R's dial, R's `dc_ice` are kept for R
//!   ([`Pair::early_ice`]) and handed to the connection, which applies them once its allocation
//!   exists; none is dropped or handed to the call code ([`on_signal`]).
//! - Rule 4: R's answer and cancel notes carry the offer's file pass on their `dm_put` to S
//!   (`file_pass`), so they reach a sender who takes messages only from friends ([`send_note`]).
//!
//! AND BY THE SECOND REVIEW (2026-10-12):
//! - Rule 7, a dial names its transfer: only a `dc_offer` carrying `file` (which the relay sets on
//!   a dial it let in by a file pass, and never passes on from a client) is a file dial. S asks for
//!   that transfer's credentials alone, so one allocation serves the connection; a dial naming a
//!   transfer that is not one of this device's live offers to R is refused quietly ([`on_signal`]).
//! - Rule 8, an end is always said: a failure on either side sends `cancel` on the channel (the
//!   connection's protocol does) and the cancel note with the receiver's pass ([`on_end`]); a
//!   channel R closes before `done`, without a word, ends S's transfer ("R's side stopped it") 15
//!   seconds on if R is still on the connection and did not open it again, and is otherwise a drop
//!   that waits for R's next dial; one S closes is opened again by R on the same connection
//!   (net/file_link.rs). Either way the row stops saying it is open or paused at once
//!   (`LinkEvent::TransferClosed`). From the review of the merged code (2026-10-12): a cancel or
//!   failure note that cannot go (the chat offline) is kept with the transfer's record and sent
//!   once the server is connected again ([`say_note`], [`flush_notes`]); a dial naming an offer
//!   that ended here as cancelled or failed, and a channel opened for a transfer that ended here
//!   other than done, both mean the note never arrived, so it goes again once in this run of the
//!   app ([`say_cancel_again`]), the channel answered with `cancel` too (`Cmd::AddEnded`).
//! - Rule 9, a dial is an answer: S, its record still waiting for R's answer note, takes the
//!   transfer's channel opening as Accept and `done` as Sent ([`pump_link`], file_offer.rs
//!   `advance`), so a lost answer note never leaves the row waiting.
//!
//! The runtime state (connections, progress) is [`Transfers`], kept in the chat's file state
//! (`GuiState::files_flow.transfers`); what lasts is the DM store's record of each transfer.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use crate::gui::GuiState;
use crate::net::dm_pq::DmInner;
use crate::net::dm_store::DmStore;
use crate::net::file_link::{self, Cmd, FileLink, LinkEvent, Relay, Setup, Side, Transfer};
use crate::net::file_offer::{self as fo, Applied, FileHashes, FilePass, Happened, Offer, OfferAnswer, OfferCancel, OfferRecord, Stage};
use crate::net::file_transfer::{End, Receiving, Sending};

/// How long the app waits for its `call_credentials` answers before it starts with what came, or
/// says the server is not set up (the call's own wait).
pub const CREDENTIALS_WAIT: Duration = crate::net::call_relay::CREDENTIALS_WAIT;
/// After a connection dropped, how long before the receiver dials again.
pub const REDIAL_AFTER: Duration = Duration::from_secs(5);
/// After the server said it is not set up, how long before the receiver tries again by itself.
pub const RETRY_NOT_SET_UP: Duration = Duration::from_secs(60);

/// The forwarder's words when it is not running (spec part 6).
pub const NOT_SET_UP: &str = "This server is not set up for that yet";

/// The words when an accepted transfer's pass ran out before the file went across.
pub const RAN_OUT: &str = "This offer ran out before the file went across (an offer lasts 7 days).";

/// The words on a receiver's other device once one of them answered.
pub const ANSWERED_ELSEWHERE: &str = "Answered on your other device";

/// While connected, how long before kept notes that could not go (one that cannot be sealed yet:
/// no DM key for them) are tried again.
pub const NOTES_RETRY: Duration = Duration::from_secs(30);

/// Where the connection with one person stands right now (not kept: a restart starts from the
/// records).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No connection, none being made.
    Idle,
    /// The credentials were asked for at this moment.
    Asking(Instant),
    /// The connection is being made through the forwarder.
    Connecting,
    /// A channel is open.
    Open,
    /// The forwarder could not be had (at this moment).
    NotSetUp(Instant),
    /// The connection dropped (at this moment); the receiver dials again after [`REDIAL_AFTER`].
    Dropped(Instant),
}

/// The connection with one person.
pub struct Pair {
    pub link: Option<FileLink>,
    pub phase: Phase,
    /// This side dialled (or is asking to).
    pub dialer: bool,
    /// The transfer whose forwarder credentials were asked for this connection, while the answer
    /// is awaited: dialling, the oldest transfer accepted here; answering, the one the dial named.
    /// One credential, so one allocation, per connection (rule 7 of the second review).
    pub asked: Option<String>,
    /// Dialling: the pass of the transfer whose credentials it dials with.
    pub dial_pass: Option<FilePass>,
    /// Answering: the other side's WebRTC offer, waiting for this side's credentials.
    pub their_offer: Option<String>,
    /// Answering: the other side's `dc_ice` that came while this side waited for its credentials
    /// (each signal's data, its JSON text), handed to the connection when it starts (rule 3 of the
    /// first review: a candidate that arrives early is kept, never dropped or given to the call
    /// code).
    pub early_ice: Vec<String>,
    /// Both dialled at once and theirs was set aside (this key is the larger): its candidates are
    /// taken here and dropped until this side's own connection stands, not given to the call code.
    pub their_dial_set_aside: bool,
    /// The transfers handed to the connection.
    pub on_link: HashSet<String>,
}

impl Default for Pair {
    fn default() -> Self {
        Pair {
            link: None,
            phase: Phase::Idle,
            dialer: false,
            asked: None,
            dial_pass: None,
            their_offer: None,
            early_ice: Vec::new(),
            their_dial_set_aside: false,
            on_link: HashSet::new(),
        }
    }
}

/// One transfer's live state on this device.
#[derive(Default)]
pub struct Live {
    /// Pieces in (receiver) or sent (sender), of the offer's `pieces`.
    pub progress: u64,
    pub paused_them: bool,
    /// Its channel is open.
    pub open: bool,
    /// The sender: the piece hashes, from preparing the offer (worked out again otherwise).
    pub hashes: Option<Arc<Vec<[u8; 32]>>>,
    /// The receiver: the part file was made in this run and nothing has been written to it, so its
    /// first connection need not check it piece by piece.
    pub fresh: bool,
}

/// A file being made ready to offer: cleaned and read for its hashes on a worker.
pub struct Preparing {
    pub id: String,
    pub peer: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub done: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<Prepared, String>>,
}

/// What the worker made ready.
pub struct Prepared {
    /// The file that goes: the person's own, or a cleaned copy this app made.
    pub path: PathBuf,
    pub own_copy: bool,
    pub hashes: FileHashes,
}

/// The chat's transfers on this device (`GuiState::files_flow.transfers`).
#[derive(Default)]
pub struct Transfers {
    /// By the other person's key.
    pub pairs: HashMap<String, Pair>,
    /// By transfer id.
    pub live: HashMap<String, Live>,
    pub preparing: Vec<Preparing>,
    /// Where connections bind their sockets: all interfaces, or 127.0.0.1 in the tests.
    pub bind: Option<String>,
    /// The transfers whose cancel note was said again in this run of the app ([`say_cancel_again`]):
    /// once each, so a receiver dialling every few seconds gets one note, not one per dial.
    pub said_again: HashSet<String>,
    /// When kept notes that could not all go are tried again ([`flush_notes`]); None: at once,
    /// which is how every new connection starts.
    pub notes_retry: Option<Instant>,
}

impl Transfers {
    /// The live state of transfer `id` (made when first needed).
    pub fn entry(&mut self, id: &str) -> &mut Live {
        self.live.entry(id.to_string()).or_default()
    }

    /// The connection with `peer` (made when first needed).
    pub fn pair(&mut self, peer: &str) -> &mut Pair {
        self.pairs.entry(peer.to_string()).or_default()
    }

    /// Where the connection with `peer` stands.
    pub fn phase(&self, peer: &str) -> Phase {
        self.pairs.get(peer).map(|p| p.phase).unwrap_or(Phase::Idle)
    }
}

/// The folder this app's cleaned copies of offered files go in (deleted when their transfer ends).
pub fn offered_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("hum_test_offered_files_{}", std::process::id()));
    }
    let dms = DmStore::store_dir();
    dms.parent().map(|p| p.join("offered-files")).unwrap_or_else(|| PathBuf::from("offered-files"))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn connected(gs: &GuiState) -> bool {
    gs.ws_client.as_ref().is_some_and(|c| c.is_connected())
}

fn send_frame(gs: &GuiState, frame: &str) -> bool {
    match gs.ws_client.as_ref().filter(|c| c.is_connected()) {
        Some(c) => {
            c.send(frame);
            true
        }
        None => false,
    }
}

/// Send one note to `peer` and its self-copy (for this person's other devices). False when it
/// cannot go now (not connected, locked, or no DM key for them yet).
///
/// `file_pass`: the offer's pass, which the RECEIVER's answer and cancel notes carry on the
/// `dm_put` to the sender (rule 4 of the first review, files-showcase.md): the relay lets such a
/// put through the sender's "who can reach me" gate when the pass verifies with the sender as its
/// issuer and this person as its grantee, so the answer reaches a sender who takes messages only
/// from friends. The self-copy never carries it.
fn send_note(gs: &GuiState, peer: &str, text: &str, file_pass: Option<&FilePass>) -> bool {
    if !connected(gs) {
        return false;
    }
    let Some((mut theirs, ours)) = crate::engine::dm::control_puts(gs, peer, text, None) else { return false };
    if let Some(pass) = file_pass {
        theirs["file_pass"] = serde_json::to_value(pass).unwrap_or(serde_json::Value::Null);
    }
    send_frame(gs, &theirs.to_string()) && send_frame(gs, &ours.to_string())
}

/// The pass a note about `r` carries: the offer's, when this device is its receiver (rule 4).
fn note_pass(r: &OfferRecord) -> Option<&FilePass> {
    (!r.outgoing).then_some(&r.offer.pass)
}

/// Whether a note can go now: the chat is connected, and signed in with this connection's mailbox
/// read (the mailbox is fetched only on a signed-in socket, and a `dm_put` before that is refused).
fn can_say(gs: &GuiState) -> bool {
    connected(gs) && crate::engine::dm::mailbox_read(gs)
}

/// RULE 8, AN END IS ALWAYS SAID, even offline (the review of 2026-10-12): send a note about
/// transfer `id` now, or, when it cannot go ([`can_say`], or it cannot be sealed yet), keep it
/// with the transfer's record (`OfferRecord::unsaid`, saved with the DM store, so a restart keeps
/// it) to send when the server is connected again ([`flush_notes`]). One kept already waits ahead
/// of it, in order. The website does the same (`pendingNotes`, at most 8 a transfer). Before this a
/// cancel made with the chat offline was lost, and the other side waited for the file until its
/// pass ran out, up to 7 days, the sender's other files with them waiting behind it.
fn say_note(gs: &mut GuiState, id: &str, text: &str) {
    let Some(r) = record(gs, id).cloned() else { return };
    if r.unsaid.is_empty() && can_say(gs) && send_note(gs, &r.peer, text, note_pass(&r)) {
        return;
    }
    with_record(gs, id, |r| {
        if r.unsaid.len() < fo::NOTES_KEPT && !r.unsaid.iter().any(|t| t == text) {
            r.unsaid.push(text.to_string());
        }
    });
}

/// Send the notes kept for transfer `id`, in order, as far as they go, when a note can go now
/// ([`can_say`]). True when none is left. A note to someone this person has since blocked is
/// dropped instead (step C: nothing is sent to them).
fn flush_record(gs: &mut GuiState, id: &str) -> bool {
    let Some(r) = record(gs, id).cloned() else { return true };
    if r.unsaid.is_empty() {
        return true;
    }
    if crate::engine::block::is_blocked(gs, &r.peer) {
        with_record(gs, id, |r| r.unsaid.clear());
        return true;
    }
    if !can_say(gs) {
        return false;
    }
    let sent = r.unsaid.iter().take_while(|text| send_note(gs, &r.peer, text, note_pass(&r))).count();
    if sent > 0 {
        with_record(gs, id, |r| {
            let n = sent.min(r.unsaid.len());
            r.unsaid.drain(..n);
        });
    }
    sent == r.unsaid.len()
}

/// Once a frame ([`pump`]): the notes kept because they could not go are sent once the server is
/// connected and signed in again, at once on each new connection, then (any that still cannot go)
/// every [`NOTES_RETRY`].
fn flush_notes(gs: &mut GuiState, now: Instant) {
    if !connected(gs) {
        gs.files_flow.transfers.notes_retry = None;
        return;
    }
    if !can_say(gs) || gs.files_flow.transfers.notes_retry.is_some_and(|t| now < t) {
        return;
    }
    let waiting: Vec<String> = gs.dm_store.as_ref().map(|s| s.file_offers().iter().filter(|r| !r.unsaid.is_empty()).map(|r| r.offer.id.clone()).collect()).unwrap_or_default();
    let mut all_went = true;
    for id in waiting {
        all_went &= flush_record(gs, &id);
    }
    gs.files_flow.transfers.notes_retry = (!all_went).then(|| now + NOTES_RETRY);
}

/// THE BACKSTOP OF RULE 8 (the review of 2026-10-12): the other side acts as if transfer `r` goes
/// on (it dials for it, or opens its channel) although it ended on this device as cancelled or
/// failed, so the cancel note never reached them. It goes again, once in this run of the app for
/// that transfer ([`Transfers::said_again`]). A transfer that ended done, declined or expired sends
/// nothing: no cancel note went for it. The website does the same once in a page's life.
fn say_cancel_again(gs: &mut GuiState, r: &OfferRecord) {
    if !r.here || !matches!(r.stage, Stage::Cancelled { .. } | Stage::Failed(_)) || crate::engine::block::is_blocked(gs, &r.peer) {
        return;
    }
    if gs.files_flow.transfers.said_again.insert(r.offer.id.clone()) {
        say_note(gs, &r.offer.id, &OfferCancel { id: r.offer.id.clone() }.text());
    }
}

fn record<'a>(gs: &'a GuiState, id: &str) -> Option<&'a OfferRecord> {
    gs.dm_store.as_ref()?.file_offers().iter().find(|r| r.offer.id == id)
}

fn records_with<'a>(gs: &'a GuiState, peer: &'a str) -> impl Iterator<Item = &'a OfferRecord> + 'a {
    gs.dm_store.as_ref().map(|s| s.file_offers()).unwrap_or(&[]).iter().filter(move |r| r.peer == peer)
}

fn with_record(gs: &mut GuiState, id: &str, f: impl FnOnce(&mut OfferRecord)) -> bool {
    let Some(store) = gs.dm_store.as_mut() else { return false };
    let Some(r) = store.file_offers_mut().iter_mut().find(|r| r.offer.id == id) else { return false };
    f(r);
    store.save();
    true
}

// ── Offering (the sender) ───────────────────────────────────────────────────────────────────

/// Why a file cannot be offered to `peer` now, in words, or None when it can.
pub fn cannot_offer(gs: &GuiState, peer: &str) -> Option<&'static str> {
    if gs.private_key_bytes.is_none() {
        return Some("unlock your identity first.");
    }
    if !connected(gs) {
        return Some("this server is not connected.");
    }
    if gs.dm_store.as_ref().and_then(|s| s.pass_server()).is_none() {
        return Some("this server has not finished signing you in yet.");
    }
    if !gs.peer_kyber_keys.contains_key(peer) {
        return Some("there is no key card for them here that this app could check yet, so nothing can be sealed to them.");
    }
    None
}

/// Offer the file at `path` (named `name`, of type `mime`) to `peer`: cleaned first when `clean`
/// (the attach step's switch; `allow_unchecked` is its Send anyway), then read for its hashes, all
/// on a worker. The offer goes from [`pump`] once that is done.
pub fn offer(gs: &mut GuiState, peer: &str, path: &Path, name: &str, mime: &str, clean: bool, allow_unchecked: bool) {
    if let Some(why) = cannot_offer(gs, peer) {
        gs.pending_notices.push(format!("{name} was not offered: {why}"));
        return;
    }
    let Some(id) = fo::new_id() else {
        gs.pending_notices.push(format!("{name} was not offered: this device could not make a random id."));
        return;
    };
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let done = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let (path, name_w, d, s) = (path.to_path_buf(), name.to_string(), done.clone(), stop.clone());
    std::thread::spawn(move || {
        let _ = tx.send(prepare(&path, &name_w, clean, allow_unchecked, &d, &s));
    });
    gs.files_flow.transfers.preparing.push(Preparing { id, peer: peer.to_string(), name: name.to_string(), mime: mime.to_string(), size, done, stop, rx });
}

/// The worker: a cleaned copy when the switch removes details and there are some (net/file_clean.rs
/// `clean_copy`, never the person's own file), then the hashes of what goes.
fn prepare(path: &Path, name: &str, clean: bool, allow_unchecked: bool, done: &AtomicU64, stop: &AtomicBool) -> Result<Prepared, String> {
    let (send_path, own_copy) = if clean {
        match crate::net::file_clean::clean_copy(path, name, &offered_dir()) {
            Ok((Some(copy), _)) => (copy, true),
            Ok((None, c)) if c.unchecked && !allow_unchecked => {
                return Err(crate::gui::pages::chat::attach_send::NOT_CHECKED.to_string());
            }
            Ok((None, _)) => (path.to_path_buf(), false),
            Err(e) => return Err(format!("it could not be read ({e}).")),
        }
    } else {
        (path.to_path_buf(), false)
    };
    let drop_copy = || {
        if own_copy {
            let _ = std::fs::remove_file(&send_path);
        }
    };
    match fo::hash_file(&send_path, done, stop) {
        Ok(hashes) if hashes.size > 0 => Ok(Prepared { path: send_path.clone(), own_copy, hashes }),
        Ok(_) => {
            drop_copy();
            Err("it is empty.".to_string())
        }
        Err(e) => {
            drop_copy();
            Err(format!("it could not be read ({e})."))
        }
    }
}

/// Stop preparing `id` (the row's Cancel while it is being read).
pub fn stop_preparing(gs: &mut GuiState, id: &str) {
    if let Some(p) = gs.files_flow.transfers.preparing.iter().find(|p| p.id == id) {
        p.stop.store(true, Ordering::Relaxed);
    }
}

/// The file is ready: mint the pass, build the offer, send the note and its self-copy, keep the
/// record. On anything that stops it, the person is told and the cleaned copy goes.
pub fn send_offer(gs: &mut GuiState, prep: &Preparing, ready: Prepared) -> bool {
    let fail = |gs: &mut GuiState, why: &str| {
        if ready.own_copy {
            let _ = std::fs::remove_file(&ready.path);
        }
        gs.pending_notices.push(format!("{} was not offered: {why}", prep.name));
        false
    };
    if let Some(why) = cannot_offer(gs, &prep.peer) {
        return fail(gs, why);
    }
    let me = gs.profile_public_key.clone();
    let (Some(seed), Some(server)) = (gs.private_key_bytes.clone(), gs.dm_store.as_ref().and_then(|s| s.pass_server()).map(str::to_string)) else {
        return fail(gs, "this server has not finished signing you in yet.");
    };
    let now = fo::now_secs();
    let Ok(pass) = fo::mint_file_pass(&seed, &server, &me, &prep.peer, &prep.id, now + fo::FILE_PASS_MAX_SECS, now) else {
        return fail(gs, "its pass could not be signed.");
    };
    let offer = Offer {
        id: prep.id.clone(),
        name: prep.name.clone(),
        size: ready.hashes.size,
        mime: prep.mime.clone(),
        blake3: ready.hashes.whole.clone(),
        piece: fo::PIECE_SIZE,
        pieces: ready.hashes.pieces.len() as u64,
        hashes: fo::hashes_digest(&ready.hashes.pieces),
        pass,
    };
    if offer.check().is_err() {
        return fail(gs, "it is past the size this app sends.");
    }
    if !send_note(gs, &prep.peer, &offer.text(), None) {
        return fail(gs, "the offer could not be sent (check the connection and try again).");
    }
    let rec = OfferRecord {
        offer,
        peer: prep.peer.clone(),
        outgoing: true,
        here: true,
        stage: Stage::Offered,
        path: ready.path.to_string_lossy().into_owned(),
        own_copy: ready.own_copy,
        let_in: false,
        paused: false,
        at: now_ms(),
        unsaid: Vec::new(),
    };
    if let Some(store) = gs.dm_store.as_mut() {
        store.file_offers_mut().push(rec);
        store.save();
    }
    gs.files_flow.transfers.entry(&prep.id).hashes = Some(Arc::new(ready.hashes.pieces));
    true
}

// ── Notes as they arrive ────────────────────────────────────────────────────────────────────

/// A verified DM on the server we are on (engine/dm.rs `ingest_dm`): an offer, answer or cancel
/// note is applied and never shown; true when it was one (taken here, readable or not).
pub fn screens_dm(gs: &mut GuiState, inner: &DmInner) -> bool {
    if !fo::is_note(&inner.text) {
        return false;
    }
    let me = gs.profile_public_key.clone();
    let Some(store) = gs.dm_store.as_mut() else { return true };
    let applied = apply_in(store, &me, inner);
    store.save();
    match applied {
        Applied::Listed(id) => {
            let incoming = record(gs, &id).filter(|r| !r.outgoing).map(|r| (r.peer.clone(), r.offer.name.clone(), r.offer.size));
            if let Some((peer, name, size)) = incoming {
                let who = crate::engine::dm::dm_display_name(gs, &peer);
                gs.pending_notices.push(format!("{who} wants to send you {name} ({}). Open your conversation with them to answer.", fo::size_words(size)));
                crate::engine::dm::rebuild_dm_sidebar(gs); // so that conversation is in the list
            }
        }
        Applied::Moved(id, stage) if stage.ended() => ended(gs, &id),
        _ => {}
    }
    true
}

/// The same on a parked server's store (engine/bg_connections.rs `bg_screen`): the note is applied
/// to that server's records, shown once it is the server in use. The caller saves.
pub fn screens_dm_parked(gs: &GuiState, store: &mut DmStore, inner: &DmInner) -> bool {
    if !fo::is_note(&inner.text) {
        return false;
    }
    apply_in(store, &gs.profile_public_key, inner);
    true
}

/// Apply a note to one server's records: an offer's pass is checked against that server's did:hum,
/// with its sender as issuer and this person as grantee.
pub fn apply_in(store: &mut DmStore, me: &str, inner: &DmInner) -> Applied {
    let Some(note) = fo::parse_note(&inner.text) else {
        log::info!("a file note that does not read was dropped");
        return Applied::Nothing;
    };
    let server = store.pass_server().unwrap_or("").to_string();
    let now = fo::now_secs();
    let from = inner.from.clone();
    let pass_ok = |o: &Offer| fo::verify_file_pass(&from, &server, me, &o.pass, now).is_ok();
    fo::apply_note(store.file_offers_mut(), me, &inner.from, &inner.to, inner.ts, note, pass_ok)
}

/// A transfer ended (by a note, the channel, or this person): it leaves its connection (with
/// `cancel` said if still open), a receiver's unfinished part file is deleted, and a sender's
/// cleaned copy too.
fn ended(gs: &mut GuiState, id: &str) {
    let Some(r) = record(gs, id).cloned() else { return };
    if let Some(pair) = gs.files_flow.transfers.pairs.get(&r.peer) {
        if let Some(link) = pair.link.as_ref().filter(|_| pair.on_link.contains(id)) {
            link.send(Cmd::Cancel(id.to_string())); // a transfer already ended there ignores it
        }
    }
    if let Some(live) = gs.files_flow.transfers.live.get_mut(id) {
        live.open = false;
    }
    if r.here && !r.path.is_empty() {
        if r.outgoing && r.own_copy {
            let _ = std::fs::remove_file(&r.path);
        } else if !r.outgoing && r.stage != Stage::Done {
            let _ = std::fs::remove_file(&r.path); // an unfinished part file
        }
    }
}

// ── The receiver's answers ──────────────────────────────────────────────────────────────────

/// The part file of a transfer: `<name>.<first 12 of the id>.part`, beside where it will be saved.
pub fn part_path(dir: &Path, safe_name: &str, id: &str) -> PathBuf {
    dir.join(format!("{safe_name}.{}.part", &id[..12.min(id.len())]))
}

/// The name a finished file gets, from its part file's name.
pub fn final_name(part: &Path, id: &str) -> Option<String> {
    let name = part.file_name()?.to_str()?;
    name.strip_suffix(&format!(".{}.part", &id[..12.min(id.len())])).map(str::to_string)
}

/// "Accept this one" on the Files in messages card: the question is asked next.
pub fn let_in(gs: &mut GuiState, id: &str) {
    with_record(gs, id, |r| r.let_in = true);
}

/// Accept `id`: the part file is made in `dir` at the file's full size (so too little room says so
/// now, not halfway), the answer goes to the sender and to this person's other devices, and the
/// transfer starts whenever both are online. False, with the person told, when it cannot.
pub fn accept(gs: &mut GuiState, id: &str, dir: &Path, safe_name: &str) -> bool {
    let Some(r) = record(gs, id).cloned() else { return false };
    if r.outgoing || r.stage != Stage::Offered {
        return false;
    }
    if !connected(gs) {
        gs.pending_notices.push("Not connected: accept again once this server is connected.".to_string());
        return false;
    }
    let part = part_path(dir, safe_name, id);
    let made = std::fs::create_dir_all(dir)
        .and_then(|_| std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&part))
        .and_then(|f| f.set_len(r.offer.size));
    if let Err(e) = made {
        let _ = std::fs::remove_file(&part);
        gs.pending_notices.push(format!("{} could not be made ready on this device ({e}).", r.offer.name));
        return false;
    }
    if !send_note(gs, &r.peer, &OfferAnswer { id: id.to_string(), accept: true }.text(), note_pass(&r)) {
        let _ = std::fs::remove_file(&part);
        gs.pending_notices.push("The answer could not be sent: try again in a moment.".to_string());
        return false;
    }
    with_record(gs, id, |r| {
        r.here = true;
        r.path = part.to_string_lossy().into_owned();
        r.stage = Stage::Accepted;
    });
    gs.files_flow.transfers.entry(id).fresh = true;
    true
}

/// Decline `id`: the answer `accept: false` goes to the sender (and this person's other devices).
pub fn decline(gs: &mut GuiState, id: &str) -> bool {
    let Some(r) = record(gs, id).cloned() else { return false };
    if r.outgoing || r.stage != Stage::Offered {
        return false;
    }
    if !send_note(gs, &r.peer, &OfferAnswer { id: id.to_string(), accept: false }.text(), note_pass(&r)) {
        gs.pending_notices.push("The answer could not be sent: try again in a moment.".to_string());
        return false;
    }
    with_record(gs, id, |r| {
        r.here = true; // answered on this device
        r.stage = Stage::Declined;
    });
    true
}

// ── Either side ─────────────────────────────────────────────────────────────────────────────

/// The connection with `peer`, when it carries transfer `id`.
fn link_for<'a>(gs: &'a GuiState, peer: &str, id: &str) -> Option<&'a FileLink> {
    let pair = gs.files_flow.transfers.pairs.get(peer)?;
    pair.link.as_ref().filter(|_| pair.on_link.contains(id))
}

/// Cancel `id`: `cancel` on its channel when it is open, the cancel note always (so the other
/// side's row says so even without a connection; kept to send later when the chat is offline),
/// and the record ends here.
pub fn cancel(gs: &mut GuiState, id: &str) {
    let Some(r) = record(gs, id).cloned() else { return };
    if r.stage.ended() {
        return;
    }
    if let Some(link) = link_for(gs, &r.peer, id) {
        link.send(Cmd::Cancel(id.to_string()));
    }
    if !crate::engine::block::is_blocked(gs, &r.peer) {
        say_note(gs, id, &OfferCancel { id: id.to_string() }.text());
    }
    with_record(gs, id, |r| r.stage = Stage::Cancelled { by_me: true });
    ended(gs, id);
}

/// Pause `id` here: kept (so it stays paused after a restart), and said on its channel when open.
pub fn pause(gs: &mut GuiState, id: &str) {
    let Some(peer) = record(gs, id).map(|r| r.peer.clone()) else { return };
    with_record(gs, id, |r| r.paused = true);
    if let Some(link) = link_for(gs, &peer, id) {
        link.send(Cmd::Pause(id.to_string()));
    }
}

/// Resume `id` here. With no connection standing, the receiver dials again on the next [`pump`]
/// (Resume is also the "Try again" after "not set up").
pub fn resume(gs: &mut GuiState, id: &str) {
    let Some(peer) = record(gs, id).map(|r| r.peer.clone()) else { return };
    with_record(gs, id, |r| r.paused = false);
    if let Some(link) = link_for(gs, &peer, id) {
        link.send(Cmd::Resume(id.to_string()));
        return;
    }
    let pair = gs.files_flow.transfers.pair(&peer);
    if pair.link.is_none() && matches!(pair.phase, Phase::NotSetUp(_) | Phase::Dropped(_)) {
        pair.phase = Phase::Idle;
    }
}

/// Take an ended transfer off the list, or an offer to this person that is left unanswered (one the
/// protected setup left out): nothing is said to the other person. One whose cancel note has not
/// gone yet (made while offline) stays until it has: the record is what keeps the note, and
/// without it the other person would wait for the file until its pass ran out.
pub fn remove(gs: &mut GuiState, id: &str) {
    if record(gs, id).is_some_and(|r| !r.unsaid.is_empty()) {
        flush_record(gs, id); // sent now when it can be (dropped for someone since blocked)
        if let Some(r) = record(gs, id).filter(|r| !r.unsaid.is_empty()).cloned() {
            let name = crate::engine::dm::dm_display_name(gs, &r.peer);
            gs.pending_notices.push(format!("{} stays on the list until this server is connected again, so {name} hears that it ended.", r.offer.name));
            return;
        }
    }
    if let Some(store) = gs.dm_store.as_mut() {
        store.file_offers_mut().retain(|r| r.offer.id != id || !(r.stage.ended() || (!r.outgoing && r.stage == Stage::Offered)));
        store.save();
    }
    if record(gs, id).is_none() {
        gs.files_flow.transfers.live.remove(id);
    }
}

// ── The connection ──────────────────────────────────────────────────────────────────────────

/// `{"type":"call_credentials","file":"<id>","peer":"<key>","file_pass":{...}}`: the forwarder's
/// credentials for this transfer, which the relay gives only with a pass naming the two people.
pub fn credentials_request(id: &str, peer: &str, pass: &FilePass) -> String {
    serde_json::json!({ "type": "call_credentials", "file": id, "peer": peer, "file_pass": pass }).to_string()
}

/// A `call_credentials` answer, read for a transfer (the relay echoes `file`): its id and, when
/// granted, the forwarder and the credentials (None: refused, or missing a UDP `turn:` address, a
/// username or a credential).
pub fn read_credentials(frame: &serde_json::Value) -> Option<(String, Option<Relay>)> {
    if frame.get("type").and_then(|t| t.as_str()) != Some("call_credentials") {
        return None;
    }
    let id = frame.get("file").and_then(|f| f.as_str()).filter(|f| fo::is_id(f))?.to_string();
    let text = |k: &str| frame.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(str::to_string);
    let refused = frame.get("refused").and_then(|v| v.as_bool()) == Some(true);
    let server = frame.get("urls").and_then(|u| u.as_array()).and_then(|urls| urls.iter().filter_map(|u| u.as_str()).find_map(crate::net::call_relay::udp_turn_server));
    let relay = match (refused, server, text("username"), text("credential")) {
        (false, Some(server), Some(username), Some(credential)) => Some(Relay { server, username, credential }),
        _ => None,
    };
    Some((id, relay))
}

/// Ask for the credentials of transfer `id` (with `peer`), the one credential of a connection this
/// side dials (`dialer`) or answers (rule 7: one allocation per connection, whatever it carries).
fn ask_credentials(gs: &mut GuiState, peer: &str, id: &str, dialer: bool) {
    let pass = record(gs, id).map(|r| r.offer.pass.clone());
    let sent = pass.as_ref().is_some_and(|p| send_frame(gs, &credentials_request(id, peer, p)));
    let pair = gs.files_flow.transfers.pair(peer);
    pair.dialer = dialer;
    pair.dial_pass = if dialer { pass } else { None };
    pair.asked = sent.then(|| id.to_string());
    pair.phase = if sent { Phase::Asking(Instant::now()) } else { Phase::Dropped(Instant::now()) };
}

/// A frame the message pump did not handle (engine/call_relay.rs `on_frame` passes it here first):
/// true when it was a transfer's `call_credentials` answer.
pub fn on_credentials(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    let Some((id, relay)) = read_credentials(frame) else { return false };
    let peer = gs.files_flow.transfers.pairs.iter().find(|(_, p)| matches!(p.phase, Phase::Asking(_)) && p.asked.as_deref() == Some(id.as_str())).map(|(k, _)| k.clone());
    let Some(peer) = peer else { return true }; // not awaited (given up on, or another device's)
    start_link(gs, &peer, relay);
    true
}

/// The transfers with `peer` a connection carries: those this device sends (offered, not ended)
/// and those it receives (accepted here, not ended).
fn transfers_for(gs: &mut GuiState, peer: &str) -> Vec<Transfer> {
    let name = crate::engine::dm::dm_display_name(gs, peer);
    let records: Vec<OfferRecord> = records_with(gs, peer).filter(|r| r.here && !r.stage.ended()).cloned().collect();
    let mut out = Vec::new();
    for r in records {
        if let Some(t) = transfer_of(gs, &r, &name) {
            out.push(t);
        }
    }
    out
}

/// One record as a transfer for the connection.
fn transfer_of(gs: &mut GuiState, r: &OfferRecord, name: &str) -> Option<Transfer> {
    let live = gs.files_flow.transfers.entry(&r.offer.id);
    let side = if r.outgoing {
        let mut proto = Sending::new(r.offer.clone(), live.hashes.clone());
        proto.set_paused(r.paused);
        Side::Send { proto, path: PathBuf::from(&r.path), receiver: name.to_string() }
    } else {
        if r.stage != Stage::Accepted {
            return None;
        }
        let part = PathBuf::from(&r.path);
        let final_name = final_name(&part, &r.offer.id)?;
        let dir = part.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut proto = Receiving::new(r.offer.clone(), name, live.fresh, Instant::now());
        proto.set_paused(r.paused);
        live.fresh = false;
        Side::Receive { proto, part, dir, name: final_name, sender: name.to_string() }
    };
    Some(Transfer { id: r.offer.id.clone(), side })
}

/// A transfer that ended here, as the connection holds it to answer a channel opened for it
/// ([`Cmd::AddEnded`]): never served, so no file is named and its protocol never runs.
fn ended_transfer(r: &OfferRecord, name: &str) -> Transfer {
    let side = if r.outgoing {
        Side::Send { proto: Sending::new(r.offer.clone(), None), path: PathBuf::new(), receiver: name.to_string() }
    } else {
        let proto = Receiving::new(r.offer.clone(), name, false, Instant::now());
        Side::Receive { proto, part: PathBuf::new(), dir: PathBuf::new(), name: String::new(), sender: name.to_string() }
    };
    Transfer { id: r.offer.id.clone(), side }
}

/// Nothing more is awaited for the connection with `peer`: it is at `phase`, with no offer of
/// theirs to answer, no candidates kept and no credentials asked.
fn give_up(pair: &mut Pair, phase: Phase) {
    pair.phase = phase;
    pair.their_offer = None;
    pair.early_ice.clear();
    pair.asked = None;
}

/// The credential came (or the wait ran out: None): start the connection with it, or say the
/// server is not set up.
fn start_link(gs: &mut GuiState, peer: &str, relay: Option<Relay>) {
    let Some(relay) = relay else {
        give_up(gs.files_flow.transfers.pair(peer), Phase::NotSetUp(Instant::now()));
        log::info!("file transfer: the server gave no forwarder credentials");
        return;
    };
    let pair = gs.files_flow.transfers.pair(peer);
    if !pair.dialer && pair.their_offer.is_none() {
        give_up(pair, Phase::Idle);
        return;
    }
    let transfers = transfers_for(gs, peer);
    if transfers.is_empty() {
        give_up(gs.files_flow.transfers.pair(peer), Phase::Idle);
        return;
    }
    let ids: HashSet<String> = transfers.iter().map(|t| t.id.clone()).collect();
    let me = gs.profile_public_key.clone();
    let bind = gs.files_flow.transfers.bind.clone().unwrap_or_else(|| "0.0.0.0:0".to_string());
    let pair = gs.files_flow.transfers.pair(peer);
    let setup = Setup {
        me,
        peer: peer.to_string(),
        dial_pass: if pair.dialer { pair.dial_pass.clone() } else { None },
        their_offer: if pair.dialer { None } else { pair.their_offer.take() },
        relay,
        transfers,
        bind,
        // The candidates that came while this side waited (rule 3), applied by the connection
        // once its allocation exists.
        early_ice: std::mem::take(&mut pair.early_ice),
    };
    pair.link = Some(FileLink::start(setup));
    pair.on_link = ids;
    pair.asked = None;
    pair.their_dial_set_aside = false; // from here on their candidates go to the connection
    pair.phase = Phase::Connecting;
}

/// Whether transfer `id` is one of the live offers this device makes to `peer` (its file is here
/// and the transfer has not ended): the only transfers a dial from `peer` may name (rule 7).
fn is_live_offer_to(gs: &GuiState, peer: &str, id: &str) -> bool {
    records_with(gs, peer).any(|r| r.offer.id == id && r.outgoing && r.here && !r.stage.ended())
}

/// Let go of the connection standing with `peer` (its thread stops when its handle is dropped):
/// the transfers it carried are no longer open or paused by them.
fn let_go(gs: &mut GuiState, peer: &str) {
    let pair = gs.files_flow.transfers.pair(peer);
    pair.link = None;
    let was: Vec<String> = pair.on_link.drain().collect();
    for id in was {
        let live = gs.files_flow.transfers.entry(&id);
        live.open = false;
        live.paused_them = false;
    }
}

/// A `webrtc_signal` (frame_ws_poll.rs): true when it is for the file connection with its sender,
/// taken here whatever becomes of it:
/// - a `dc_offer` naming its transfer (`file`, which the relay sets on a dial it let in by a file
///   pass and never passes on from a client: rule 7). Naming one of this device's live offers to
///   its sender, it is answered as their file connection with that one transfer's credentials;
///   naming anything else, it is refused quietly (nothing is sent and nothing shown: another of
///   this person's devices may hold that file), and never given to the call code;
/// - a `dc_answer` or `dc_ice` from someone a file connection stands with;
/// - a `dc_ice` from someone whose dial this side is answering but has no credentials for yet
///   (kept, rule 3), and a `dc_ice` of a dial set aside when both dialled.
///
/// Anything else, a `dc_offer` naming no transfer included, is left for the app's WebRTC manager.
pub fn on_signal(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    let from = frame.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let signal_type = frame.get("signal_type").and_then(|v| v.as_str()).unwrap_or("");
    let data = frame.get("data").cloned().unwrap_or(serde_json::Value::Null);
    if from.is_empty() || crate::engine::block::is_blocked(gs, &from) {
        return false;
    }
    let text = file_link::signal_text(&data);
    match signal_type {
        "dc_offer" => {
            let Some(id) = frame.get("file").and_then(|v| v.as_str()).filter(|f| fo::is_id(f)).map(str::to_string) else {
                return false; // not a file dial: a friend's direct connection, the manager's
            };
            if !is_live_offer_to(gs, &from, &id) {
                // THE BACKSTOP OF RULE 8: a dial for an offer of this device's to them that ended
                // here as cancelled or failed means the cancel note never reached them, and their
                // other files from this person wait behind this dial (they dial with their oldest
                // accepted one). Still not answered, but the cancel note goes again, once in this
                // run of the app.
                let mine = records_with(gs, &from).find(|r| r.offer.id == id && r.outgoing).cloned();
                if let Some(r) = mine {
                    say_cancel_again(gs, &r);
                }
                log::info!("file transfer: a dial for a transfer this device is not offering them was not answered");
                return true;
            }
            let me = gs.profile_public_key.clone();
            let pair = gs.files_flow.transfers.pair(&from);
            let dialling = pair.dialer && (pair.link.is_some() || matches!(pair.phase, Phase::Asking(_)));
            if dialling && me > from {
                // Both dialled at once: the larger key's dial stands, theirs is dropped (and so
                // are its candidates, below).
                pair.their_dial_set_aside = true;
                return true;
            }
            // Theirs stands: any attempt or connection of ours is let go, and this one answered
            // with the credentials of the transfer it names, the connection's one allocation.
            let_go(gs, &from);
            let pair = gs.files_flow.transfers.pair(&from);
            pair.their_offer = Some(text);
            pair.early_ice.clear();
            pair.their_dial_set_aside = false;
            ask_credentials(gs, &from, &id, false);
            true
        }
        "dc_answer" | "dc_ice" => {
            let Some(pair) = gs.files_flow.transfers.pairs.get_mut(&from) else { return false };
            if let Some(link) = pair.link.as_ref() {
                if signal_type == "dc_answer" && !pair.dialer {
                    return false;
                }
                link.send(Cmd::Signal { signal_type: signal_type.to_string(), data: text });
                return true;
            }
            if signal_type != "dc_ice" {
                return false;
            }
            // RULE 3 OF THE FIRST REVIEW: answering their dial, still waiting for this side's
            // credentials, so no connection stands to take it yet. Kept, and handed to the
            // connection when it starts (`start_link`), which applies it once its allocation
            // exists. Before this, it fell through to the call code and was lost, and the forwarder
            // then dropped the dialler's packets for want of a permission toward its address.
            if pair.their_offer.is_some() {
                if pair.early_ice.len() < file_link::MAX_EARLY_ICE {
                    pair.early_ice.push(text);
                }
                return true;
            }
            // A candidate of the dial set aside when both dialled: the file connection's, so it
            // is not the call code's either.
            pair.their_dial_set_aside
        }
        _ => false,
    }
}

/// Once a frame: offers made ready go out; each connection's news is applied; credentials not all
/// answered in time start with what came (or say "not set up"); transfers with someone blocked,
/// and offers that ran out, end; transfers accepted or offered while a connection stands join it;
/// and with no connection, a receiver with a transfer accepted and not paused dials whenever the
/// sender is online.
pub fn pump(gs: &mut GuiState) {
    pump_preparing(gs);
    if gs.dm_store.is_none() {
        return;
    }
    let peers: Vec<String> = gs.files_flow.transfers.pairs.keys().cloned().collect();
    for peer in &peers {
        pump_link(gs, peer);
    }
    let now = Instant::now();
    flush_notes(gs, now);
    let records: Vec<OfferRecord> = gs.dm_store.as_ref().map(|s| s.file_offers().to_vec()).unwrap_or_default();
    let mut peers_live: Vec<String> = Vec::new();
    for r in &records {
        let id = r.offer.id.clone();
        if r.stage.ended() {
            continue;
        }
        // Someone this person blocked: ended here, nothing sent to them (step C).
        if crate::engine::block::is_blocked(gs, &r.peer) {
            with_record(gs, &id, |r| r.stage = Stage::Cancelled { by_me: true });
            ended(gs, &id);
            continue;
        }
        if r.offer.pass.expiry <= fo::now_secs() {
            let stage = if r.stage == Stage::Offered { Stage::Expired } else { Stage::Failed(RAN_OUT.to_string()) };
            with_record(gs, &id, |r| r.stage = stage);
            ended(gs, &id);
            continue;
        }
        if r.here && !peers_live.contains(&r.peer) {
            peers_live.push(r.peer.clone());
        }
    }
    for peer in peers_live {
        // The credential not answered in time: the server is not set up for it.
        if let Phase::Asking(t) = gs.files_flow.transfers.phase(&peer) {
            if now.duration_since(t) >= CREDENTIALS_WAIT {
                start_link(gs, &peer, None);
            }
        }
        if gs.files_flow.transfers.pair(&peer).link.is_some() {
            join_link(gs, &peer);
            continue;
        }
        // The dial goes with the oldest transfer accepted here: its credential is the
        // connection's one allocation, and every other transfer with them joins it.
        let wants: Option<String> = records_with(gs, &peer)
            .filter(|r| !r.outgoing && r.here && r.stage == Stage::Accepted && !r.paused)
            .map(|r| (r.at, r.offer.id.clone()))
            .min()
            .map(|(_, id)| id);
        let Some(wants) = wants else { continue };
        if !peer_online(gs, &peer) || !connected(gs) {
            continue;
        }
        let due = match gs.files_flow.transfers.phase(&peer) {
            Phase::Idle => true,
            Phase::Dropped(t) => now.duration_since(t) >= REDIAL_AFTER,
            Phase::NotSetUp(t) => now.duration_since(t) >= RETRY_NOT_SET_UP,
            Phase::Asking(_) | Phase::Connecting | Phase::Open => false,
        };
        if due {
            ask_credentials(gs, &peer, &wants, true);
        }
    }
}

/// Transfers with `peer` that are live here but not on the connection that stands join it.
fn join_link(gs: &mut GuiState, peer: &str) {
    let on: HashSet<String> = gs.files_flow.transfers.pair(peer).on_link.clone();
    let name = crate::engine::dm::dm_display_name(gs, peer);
    let missing: Vec<OfferRecord> = records_with(gs, peer)
        .filter(|r| r.here && !r.stage.ended() && !on.contains(&r.offer.id) && (r.outgoing || r.stage == Stage::Accepted))
        .cloned()
        .collect();
    for r in missing {
        if let Some(t) = transfer_of(gs, &r, &name) {
            let pair = gs.files_flow.transfers.pair(peer);
            if let Some(link) = &pair.link {
                link.send(Cmd::Add(t));
                pair.on_link.insert(r.offer.id.clone());
            }
        }
    }
}

/// Is `peer` in this server's list of people online?
fn peer_online(gs: &GuiState, peer: &str) -> bool {
    gs.chat_users.iter().any(|u| u.public_key == peer)
}

fn pump_preparing(gs: &mut GuiState) {
    let mut i = 0;
    while i < gs.files_flow.transfers.preparing.len() {
        let got = gs.files_flow.transfers.preparing[i].rx.try_recv();
        match got {
            Ok(result) => {
                let prep = gs.files_flow.transfers.preparing.remove(i);
                match result {
                    Ok(ready) => {
                        send_offer(gs, &prep, ready);
                    }
                    Err(why) if !prep.stop.load(Ordering::Relaxed) => gs.pending_notices.push(format!("{} was not offered: {why}", prep.name)),
                    Err(_) => {}
                }
            }
            Err(mpsc::TryRecvError::Empty) => i += 1,
            Err(mpsc::TryRecvError::Disconnected) => {
                gs.files_flow.transfers.preparing.remove(i);
            }
        }
    }
}

/// Apply the news of the connection with `peer`.
fn pump_link(gs: &mut GuiState, peer: &str) {
    let events = match gs.files_flow.transfers.pairs.get(peer).and_then(|p| p.link.as_ref()) {
        Some(link) => link.poll(),
        None => return,
    };
    for ev in events {
        match ev {
            LinkEvent::Signal(frame) => {
                send_frame(gs, &frame);
            }
            LinkEvent::RelayReady => gs.files_flow.transfers.pair(peer).phase = Phase::Connecting,
            LinkEvent::RelayUnavailable => gs.files_flow.transfers.pair(peer).phase = Phase::NotSetUp(Instant::now()),
            LinkEvent::Open => gs.files_flow.transfers.pair(peer).phase = Phase::Open,
            LinkEvent::TransferOpen(id) => {
                gs.files_flow.transfers.entry(&id).open = true;
                // RULE 9 OF THE SECOND REVIEW, a dial is an answer: the receiver opens a
                // transfer's channel only once it accepted, so a sender whose record still waits
                // for the answer note (lost, or slower than the dial) takes this as the answer.
                let accepted = record(gs, &id).filter(|r| r.outgoing && r.peer == peer).and_then(|r| fo::advance(&r.stage, &Happened::Accepted));
                if let Some(next) = accepted {
                    with_record(gs, &id, |r| r.stage = next);
                }
            }
            LinkEvent::TransferClosed(id) => {
                // Rule 8: the other side closed its channel. Not open, and a pause they held is
                // over (they say it again if it still holds); the connection opens it again, or
                // ends the transfer, and says which.
                let live = gs.files_flow.transfers.entry(&id);
                live.open = false;
                live.paused_them = false;
            }
            LinkEvent::Wanted(id) => {
                let Some(r) = record(gs, &id).filter(|r| r.peer == peer && r.here).cloned() else { continue };
                let name = crate::engine::dm::dm_display_name(gs, peer);
                if !r.stage.ended() {
                    // The other side opened a channel for a transfer of ours with them: it joins.
                    if let Some(t) = transfer_of(gs, &r, &name) {
                        let pair = gs.files_flow.transfers.pair(peer);
                        if let Some(link) = &pair.link {
                            link.send(Cmd::Add(t));
                            pair.on_link.insert(id);
                        }
                    }
                } else if r.stage != Stage::Done {
                    // RULE 8, AN END IS ALWAYS SAID (the review of 2026-10-12): they opened a
                    // channel for a transfer that ended here, so they never heard it ended. The
                    // connection says `cancel` on that channel (and on any they open for it
                    // again), and the cancel note goes again, once in this run. Before this the
                    // channel was never answered: their side had asked for `hashes`, and its row
                    // read "Receiving" at 0 while another transfer ran beside it.
                    if let Some(link) = gs.files_flow.transfers.pairs.get(peer).and_then(|p| p.link.as_ref()) {
                        link.send(Cmd::AddEnded(ended_transfer(&r, &name)));
                    }
                    say_cancel_again(gs, &r);
                }
            }
            LinkEvent::Progress(id, n) => gs.files_flow.transfers.entry(&id).progress = n,
            LinkEvent::PausedByThem(id, b) => gs.files_flow.transfers.entry(&id).paused_them = b,
            LinkEvent::Saved(id, path) => {
                with_record(gs, &id, |r| r.path = path.to_string_lossy().into_owned());
            }
            LinkEvent::Ended(id, end) => on_end(gs, &id, end),
            LinkEvent::Closed => {
                let pair = gs.files_flow.transfers.pair(peer);
                pair.link = None;
                let was: Vec<String> = pair.on_link.drain().collect();
                if !matches!(pair.phase, Phase::NotSetUp(_)) {
                    pair.phase = Phase::Dropped(Instant::now());
                }
                for id in was {
                    let live = gs.files_flow.transfers.entry(&id);
                    live.open = false;
                    live.paused_them = false;
                }
            }
        }
    }
}

/// How a transfer's channel ended, into its record. A failure on this side (its words, including
/// "Ann's side stopped it" when they closed the channel and stayed on the connection) is told to
/// the other side with the cancel note too, the receiver's carrying its pass (rule 8 of the second
/// review: an end is always said; the channel's `cancel` went from the connection's protocol when
/// the channel was open; the note is kept to send later when the chat is offline). Its row then
/// says they cancelled. `done` ends a sender still waiting for the answer note as Sent (rule 9,
/// file_offer.rs `advance`).
fn on_end(gs: &mut GuiState, id: &str, end: End) {
    let Some(r) = record(gs, id).cloned() else { return };
    let what = match &end {
        End::Done => Happened::Done,
        End::Failed(words) => Happened::Failed(words.clone()),
        End::Cancelled { by_them } => Happened::Cancelled { by_me: !by_them },
    };
    let Some(next) = fo::advance(&r.stage, &what) else { return };
    if matches!(end, End::Failed(_)) && !crate::engine::block::is_blocked(gs, &r.peer) {
        say_note(gs, id, &OfferCancel { id: id.to_string() }.text());
    }
    with_record(gs, id, |r| r.stage = next);
    if let End::Done = end {
        gs.files_flow.transfers.entry(id).progress = r.offer.pieces;
    }
    ended(gs, id);
}

// ── What each row says ──────────────────────────────────────────────────────────────────────

/// The words a transfer's row shows under its name (spec part 6), `name` being the other person's,
/// `phase` the connection with them and `peer_online` whether they are listed online.
pub fn status_words(r: &OfferRecord, live: Option<&Live>, phase: Phase, name: &str, peer_online: bool) -> String {
    if !r.outgoing && !r.here && matches!(r.stage, Stage::Accepted | Stage::Declined) {
        return ANSWERED_ELSEWHERE.to_string(); // a self-copy of this person's own answer
    }
    match &r.stage {
        Stage::Done if r.outgoing => "Sent".to_string(),
        Stage::Done if r.here => format!("Saved to {}", r.path),
        Stage::Done => "Saved on your other device".to_string(),
        Stage::Declined if r.outgoing => format!("{name} declined"),
        Stage::Declined => "You declined".to_string(),
        Stage::Cancelled { by_me: true } => "Cancelled".to_string(),
        Stage::Cancelled { by_me: false } => format!("{name} cancelled"),
        Stage::Failed(words) => words.clone(),
        Stage::Expired if r.outgoing => format!("{name} did not answer in time"),
        Stage::Expired => "This offer ran out".to_string(),
        Stage::Offered if r.outgoing && !r.here => "Offered from your other device".to_string(),
        Stage::Offered if r.outgoing => format!("Waiting for {name} to accept"),
        Stage::Offered => String::new(), // the question shows instead
        Stage::Accepted if !r.here => "Accepted; it goes from your other device".to_string(),
        Stage::Accepted => {
            if r.paused {
                return "Paused".to_string();
            }
            if live.is_some_and(|l| l.paused_them) {
                return format!("{name} paused it");
            }
            if live.is_some_and(|l| l.open) {
                return if r.outgoing { "Sending through the server's call relay".to_string() } else { "Receiving through the server's call relay".to_string() };
            }
            match phase {
                Phase::NotSetUp(_) => NOT_SET_UP.to_string(),
                Phase::Asking(_) | Phase::Connecting | Phase::Open => "Connecting through the server's call relay".to_string(),
                Phase::Idle | Phase::Dropped(_) if !peer_online => format!("Waiting for {name} to come online"),
                Phase::Idle | Phase::Dropped(_) if r.outgoing => format!("Waiting for {name}'s app to connect"),
                Phase::Idle | Phase::Dropped(_) => "Connecting through the server's call relay".to_string(),
            }
        }
    }
}

/// The line under a row that is not over yet: whose device has to stay open (spec part 6, "the
/// sender must be online for it to go, and the row says so").
pub fn online_line(r: &OfferRecord, name: &str) -> Option<String> {
    if r.stage.ended() || !r.here {
        return None;
    }
    Some(if r.outgoing {
        "It goes from this device: keep the app open until it is sent.".to_string()
    } else {
        format!("It comes straight from {name}'s device, so their app has to be open.")
    })
}

#[cfg(test)]
#[path = "file_transfer_tests.rs"]
mod tests;

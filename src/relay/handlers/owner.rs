//! The server's owner: the claim and the owner's changes (2026-11-20, docs/design/hide-and-keep.md,
//! "How the owner role is set"; the operator approved the claim code that day). The storage half is
//! storage/owner.rs and storage/owner_handover.rs; a recipient's check and acceptance of a change is
//! owner_accept.rs.
//!
//! WHY THE OWNER MATTERS. Hide and keep seals every held copy to the owner's key card, so the owner
//! is the one person who can ever open one. The role is therefore never an admin's to grant.
//!
//! THE CLAIM. While no key holds `owner`, the relay mints ONE code ([`mint_claim_code`]: 64 bits
//! from the operating system's generator, as 16 hex digits), prints it to its own log and writes
//! it to [`CLAIM_CODE_FILE`] beside its database. It is in no message to any client and no answer
//! of any route (the data-file routes refuse that file even to admins, storage/data_files.rs), so
//! only someone with access to the server machine can read it. Pasted into "Claim ownership" in
//! either app, or typed as `/claim <code>` (both apps keep every line that starts with /claim out
//! of every message, 2026-11-20), it makes its bearer the owner ([`claim`]), who counts as an
//! admin everywhere.
//!
//! WHAT AN ATTACKER CAN AND CANNOT DO WITH THE CLAIM (the owner review's finding 4, 2026-11-20).
//! Every attempt is compared in constant time, ALWAYS, and the right code ALWAYS claims: no count of
//! wrong guesses, by anyone, can lock the real owner out (the first version checked a server-wide
//! count before the code, so one key's thirty wrong guesses closed the claim for everyone, and kept
//! doing so window after window). Wrong guesses are counted per key (kept across reconnects, for
//! [`MISS_WINDOW_MS`] after a key's latest, at most [`MISS_KEYS_MAX`] keys: the second review, R7),
//! per connection and on the whole server, logged with the key, and each costs its connection a
//! pause before the answer ([`WRONG_CODE_PAUSE_MS`], longer past [`MISSES_PER_KEY`]); the counts
//! only decide the words and the pause, never whether a right code works. What an attacker cannot
//! do is find the code: 64 random bits at even a thousand guesses a second take on average about 290
//! million years. What they can do is fill the relay's log with wrong-code lines and slow their own
//! connections.
//!
//! The claimer must have a key card here that checks out, since held copies will be sealed to it;
//! without one the claim is refused with words saying how to get one, and the code still works. A
//! right code burns the code and deletes the file, and the claim is logged (the key and the time, in
//! the relay's log and the owner log). A fresh server's first claimer becomes its owner: there is
//! no separate first-admin claim any more.
//!
//! THE OWNER'S CHANGES: a handover while the owner is alive, and naming, changing or removing the
//! successor. The owner's app picks a member and goes ahead only once the four groups of the listed
//! key's fingerprint at positions it drew match what the person read out from their own screen
//! (every key, every time; the second review's W1, the fourth review's finding 1, the fifth
//! review's H1: the owner's screen shows only the positions), the owner types a confirmation and
//! signs an `owner_handover` (or `owner_successor`, or `owner_successor_remove`
//! naming the successor), sent as `owner_change_begin` and checked by storage/owner_handover.rs
//! (this server's DID, five minutes fresh, never used before). A change RE-WRAPS every held copy,
//! it never re-encrypts one (format v4; the owner branch's second review, 2026-11-20, replaced the
//! re-seal flow, in which the owner's device opened every part, the relay was handed every restore
//! key, and each recipient opened every copy to check it). In TWO PHASES:
//!
//! 1. RE-WRAP. The relay answers `owner_change_ready`: every held item and whom to seal it to
//!    ([`HANDOVER_WINDOW_MS`] to finish). The owner's device fetches each part's HEADER alone (`GET
//!    /api/held/{id}/{part}/header?grant=`, every request signed by the owner's own key:
//!    handlers/held.rs `signed_caller`), opens its own slot, wraps the same part key for the new
//!    list (each card checked on the device), and sends the new header (`PUT .../header`). The relay
//!    takes a header alone, for the right number of people, carrying the very key commitment of the
//!    header it replaces (storage/owner_handover.rs `take_new_header`): no body byte is ever taken
//!    and no device can change what a copy holds. `owner_change_finish` (no keys: part 0's body never
//!    changes) has the relay build each new part beside the original, the new header then the
//!    original's body copied byte for byte, and change NOTHING yet.
//! 2. ACCEPT. Each recipient of the new seal (the new owner; for a naming, the owner and the
//!    successor; for a removal, the owner alone) is sent `owner_change_check`, carrying the owner's
//!    signed action and the change's ROUND (a fresh id per begin). Their own device fetches each new
//!    HEADER (`GET .../rewrapped`, signed by them), opens ONLY its own slot, checks the key against
//!    the commitment, and signs an `owner_change_accept` naming the change and the digest of every
//!    new header's hash and the round (owner_accept.rs). No body is served to anyone in a change,
//!    so a successor-to-be or a handover's target reads nothing before holding the role. The relay
//!    works out the same digest from the headers it staged. Only when every recipient has accepted
//!    does it swap the copies and the role (or the successor) in one step and delete the originals.
//!    A decline, a digest that does not match, or no acceptance within [`ACCEPT_WINDOW_MS`] rolls
//!    everything back: the role and every original stay as they were.
//!
//! ROLLBACK AND CARRY-OUT EXCLUDE EACH OTHER (the second review, R4). The pending change leaves its
//! slot ([`ChangeSlot`]) under the lock before it is carried out, and the slot then names the round
//! being carried out: a cancel, a decline, the expiry tick and a new begin all refuse while it does,
//! so the words "rolled back" are never said of a change that commits, and nothing clears its new
//! parts while their folders are renamed.
//!
//! After a handover the old owner is an admin and no successor is named.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::files::{Folders, OwnedFolders};
use crate::relay::storage::owner::handover::{read_header, ChangeKind, HeaderRefused, OwnerChange};

/// Where the claim code is written while no key holds the owner role, beside the database. Whoever
/// reads it can claim the server, so it is for the host's eyes alone.
pub const CLAIM_CODE_FILE: &str = "data/owner-claim-code.txt";
/// Wrong codes one connection may try before its answers say so.
pub const MISSES_PER_CONNECTION: u32 = 5;
/// Wrong codes one key may try, across every connection and reconnect, before each further wrong
/// code waits [`WRONG_CODE_PAUSE_LONG_MS`].
pub const MISSES_PER_KEY: u32 = 10;
/// Wrong codes on the whole server in [`MISS_WINDOW_MS`] past which the relay's log raises its voice.
pub const MISSES_OVERALL: u32 = 30;
pub const MISS_WINDOW_MS: i64 = 10 * 60 * 1000;
/// The most keys whose wrong codes are remembered at once; past it the one whose latest wrong code is
/// oldest is forgotten first (the second review, R7: the map was never pruned).
pub const MISS_KEYS_MAX: usize = 4096;
/// The pause before a wrong code's answer, and past [`MISSES_PER_KEY`]. A right code never waits.
pub const WRONG_CODE_PAUSE_MS: u64 = 250;
pub const WRONG_CODE_PAUSE_LONG_MS: u64 = 2_000;
/// How long the owner's device has to re-wrap every part after starting a change.
pub const HANDOVER_WINDOW_MS: i64 = 30 * 60 * 1000;
/// How long the recipients have to check and accept once every new header arrived; then the
/// change rolls back. They must be signed in for it, so the owner arranges that first.
pub const ACCEPT_WINDOW_MS: i64 = 60 * 60 * 1000;

/// What each person connecting to a server with no owner is told (the code itself never travels).
pub const UNCLAIMED_NOTICE: &str = "This server has no owner yet. If it is YOUR server, read the claim code from the server's log (or data/owner-claim-code.txt on the server machine) and paste it into Claim ownership: in the desktop app, Server Settings; on the website, the Claim ownership line in the chat or the command palette. (Typing /claim followed by the code works too; the app keeps that line out of every message.)";

/// What a key holding the owner role is told when it redeems a link code (the second review, R5).
pub const OWNER_LINK_REFUSED: &str = "This device holds the server's owner role, so it cannot be linked under another name with a link code: the owner's key keeps its own name. Nothing was changed, and the code still works for another device.";

/// The wrong claim codes counted (the module's header): the server-wide window and each key's
/// count with the time of its latest, which a reconnect does not reset.
#[derive(Debug, Default)]
pub struct ClaimMisses {
    pub window_start: i64,
    pub overall: u32,
    pub per_key: std::collections::HashMap<String, (u32, i64)>,
}

impl ClaimMisses {
    /// Count a wrong code from `key` at `now`: entries older than [`MISS_WINDOW_MS`] forgotten, and
    /// at most [`MISS_KEYS_MAX`] keys kept. The key's count now.
    pub fn count(&mut self, key: &str, now: i64) -> u32 {
        self.per_key.retain(|_, (_, at)| now - *at <= MISS_WINDOW_MS);
        if !self.per_key.contains_key(key) && self.per_key.len() >= MISS_KEYS_MAX {
            if let Some(oldest) = self.per_key.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| k.clone()) {
                self.per_key.remove(&oldest);
            }
        }
        let e = self.per_key.entry(key.to_string()).or_insert((0, now));
        e.0 += 1;
        e.1 = now;
        e.0
    }
}

/// The owner change in flight, and the round being carried out right now (the module's header,
/// "Rollback and carry-out exclude each other"). Every reader and writer holds its lock.
#[derive(Debug, Default)]
pub struct ChangeSlot {
    /// The change the owner started that has not been carried out or rolled back.
    pub pending: Option<PendingHandover>,
    /// The round of the change being carried out (it left `pending` under the lock): while it is
    /// Some, nothing rolls back, cancels or begins.
    pub carrying: Option<String>,
    /// Hides sealing their copy right now, outside any change (the third review's S3): while it is
    /// above 0, no change begins, so a copy is never sealed to recipients a change is replacing. A
    /// hide that arrives while a change is pending or carried out waits instead ([`seal_waiting`]).
    pub sealing: u32,
    /// Hide-and-keep tasks in flight, each from its decision to its recorded decision ([`HidingNow`]):
    /// graceful shutdown waits for every one ([`wait_for_hides`], the owner branch's seventh
    /// review, L2), so a stop mid-hide no longer loses its signed record, decision, broadcast and
    /// linked servers' delete.
    pub hiding: u32,
}

/// One hide-and-keep task ([`ChangeSlot::hiding`]), counted from [`HidingNow::start`] until the task
/// drops it, however it ends. Owned (it holds the state), so it moves into the task.
pub struct HidingNow(Arc<RelayState>);

impl HidingNow {
    pub fn start(state: &Arc<RelayState>) -> Self {
        slot(state).hiding += 1;
        HidingNow(state.clone())
    }
}

impl Drop for HidingNow {
    fn drop(&mut self) {
        let mut s = slot(&self.0);
        s.hiding = s.hiding.saturating_sub(1);
    }
}

/// Wait until no hide-and-keep task is in flight, or `limit` has passed (graceful shutdown, relay
/// mod.rs `shutdown_signal`). True when none is left; a hide still running at the limit is logged,
/// and the next start completes it in words (handlers/held.rs `complete_unfinished_hides`).
pub async fn wait_for_hides(state: &RelayState, limit: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    let mut said = false;
    loop {
        let n = slot(state).hiding;
        if n == 0 {
            return true;
        }
        if !said {
            tracing::info!("Shutdown: waiting for {n} hide-and-keep task(s) to finish.");
            said = true;
        }
        if start.elapsed() >= limit {
            tracing::error!("Shutdown: {n} hide-and-keep task(s) still running after {} s; the next start completes them.", limit.as_secs());
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// One hide sealing its copy ([`ChangeSlot::sealing`]), counted from [`SealingNow::start`] until it
/// is dropped, however the hide ends.
pub struct SealingNow<'a>(&'a RelayState);

impl<'a> SealingNow<'a> {
    /// Count a hide as sealing now, unless an owner change is pending or being carried out (then
    /// None: its copy must wait, storage/held.rs `WAITING_PREFIX`).
    pub fn start(state: &'a RelayState) -> Option<Self> {
        let mut s = slot(state);
        if s.pending.is_some() || s.carrying.is_some() {
            return None;
        }
        s.sealing += 1;
        Some(SealingNow(state))
    }
}

impl Drop for SealingNow<'_> {
    fn drop(&mut self) {
        let mut s = slot(self.0);
        s.sealing = s.sealing.saturating_sub(1);
    }
}

/// The round being carried out ([`ChangeSlot::carrying`]), named from the moment the last
/// acceptance takes the change out of its slot until this is dropped, however the carry-out ends:
/// done, failed, or stopped by a panic (the fifth review's L1). Before, `carrying` was cleared by
/// one line after the carry-out, so a panic inside it left the round named until the relay
/// restarted: every later hide waited for a change that would never end, and every begin was
/// refused, silently. A panic is logged here; the carry-out itself puts the held folder right first
/// (owner_accept.rs, `Storage::owner_handover_recover_in`, as the next start would).
pub struct CarryingNow<'a>(&'a RelayState);

impl<'a> CarryingNow<'a> {
    /// Guard the round the caller has just named in `carrying`, under the same lock that took the
    /// change out of `pending` (so nothing ever sees neither); nothing rolls back, cancels or begins
    /// until this drops.
    pub fn adopt(state: &'a RelayState) -> Self {
        CarryingNow(state)
    }
}

impl Drop for CarryingNow<'_> {
    fn drop(&mut self) {
        let was = slot(self.0).carrying.take();
        if std::thread::panicking() {
            tracing::error!(
                "Owner change: carrying out round {} stopped with a panic. It is no longer named as being carried out, so hides seal and changes can begin again; the held folder was put right as the next start would.",
                was.as_deref().unwrap_or("?")
            );
        }
    }
}

/// Each minute (relay/mod.rs), on a blocking thread: an owner change past its window rolled back,
/// a hide waiting past a change's whole life sealed (the change rolled back if it must be), and any
/// hide still waiting with no change in flight sealed, as a safety net.
pub fn minute(state: &RelayState, folders: Folders, now: i64) {
    #[cfg(test)]
    if PANIC_NEXT_MINUTE.swap(false, std::sync::atomic::Ordering::SeqCst) {
        panic!("a forced panic in the owner's minute (test)");
    }
    expire_pending(state, folders);
    cap_waiting_at(state, folders, now);
    seal_waiting(state, folders);
}

/// A test's switch: the next [`minute`] panics, once.
#[cfg(test)]
pub(crate) static PANIC_NEXT_MINUTE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One pass of the owner's minute ([`minute`]) on a blocking thread (sealing a waiting copy reads
/// and writes whole files: never on the async runtime, the fifth review's L4), and a panic in it is
/// logged and ends only that pass (the fifth review's L1: the loop was one task, so one panic ended
/// it for good and with it every bound on how long a hide waits). Whether the pass completed.
pub async fn minute_pass(state: Arc<RelayState>, folders: OwnedFolders) -> bool {
    match tokio::task::spawn_blocking(move || minute(&state, folders.get(), now_ms())).await {
        Ok(()) => true,
        Err(e) => {
            tracing::error!("Owner change: this minute's checks stopped with a panic ({e}); they run again next minute.");
            false
        }
    }
}

/// Seal every hide whose copy waited for an owner change (the third review's S3: storage/held.rs
/// `held_seal_waiting_in`), now, to whoever the recipients are now: right after a change is carried
/// out or rolled back, at startup, each minute as a safety net, and after a hide that waited when the
/// change had meanwhile ended. Nothing while a change is pending or being carried out; counted as
/// sealing meanwhile, so no change begins until it is done. A hide never rolls a change back.
pub fn seal_waiting(state: &RelayState, folders: Folders) {
    let Some(_sealing) = SealingNow::start(state) else { return };
    let n = state.db.held_seal_waiting_in(folders);
    if n > 0 {
        tracing::info!("Hide and keep: {n} hidden post(s) that waited for an owner change are now sealed to the owner's key card.");
    }
}

/// An owner change (a handover, or the successor named or removed) the owner started and that has
/// not been carried out or rolled back.
#[derive(Debug, Clone)]
pub struct PendingHandover {
    pub change: OwnerChange,
    /// This begin's own id (random, hex), bound into every acceptance: an acceptance of one round
    /// never carries out another, even of the same signed action begun again.
    pub round: String,
    /// The signed action as it arrived (JSON), recorded in the owner log once carried out and sent
    /// to each recipient, whose device checks it.
    pub object_json: String,
    pub started_at: i64,
    /// The held items the change covers (their ids when it began).
    pub ids: Vec<i64>,
    /// When every new header arrived and the new parts were built (phase 2 began); None while the
    /// owner's device re-wraps.
    pub checked_at: Option<i64>,
    /// The finish is checking every new header and building the new parts right now (set under the
    /// lock by [`finish_in`], the third review's S1): a second finish, and any header read or sent
    /// for this change, is refused until it ends, so the parts built are made from the very headers
    /// that were there when it began.
    pub building: bool,
    /// The recipients who accepted, each with their signed acceptance.
    pub accepted: Vec<crate::relay::storage::owner::handover::Acceptance>,
}

impl PendingHandover {
    /// Whether the owner's device is still sending new headers (phase 1).
    pub fn uploading(&self) -> bool {
        self.checked_at.is_none()
    }

    /// Whether the change is still within its window at `now`.
    pub fn live(&self, now: i64) -> bool {
        match self.checked_at {
            None => now - self.started_at <= HANDOVER_WINDOW_MS,
            Some(at) => now - at <= ACCEPT_WINDOW_MS,
        }
    }

    /// The recipients who have not accepted yet.
    pub fn waiting_on(&self) -> Vec<String> {
        self.change.recipients.iter().map(|(k, _)| k.clone()).filter(|k| !self.accepted.iter().any(|a| &a.key == k)).collect()
    }
}

/// The change slot, locked.
pub(crate) fn slot(state: &RelayState) -> std::sync::MutexGuard<'_, ChangeSlot> {
    state.pending_handover.lock().unwrap_or_else(|p| p.into_inner())
}

fn now_ms() -> i64 {
    crate::relay::storage::now_millis() as i64
}

/// Write the claim code file for `code`.
pub fn write_claim_code_file(code: &str) -> std::io::Result<()> {
    write_claim_code_file_at(std::path::Path::new(CLAIM_CODE_FILE), code)
}

/// Write the claim code file for `code` at `file`: readable and writable by the relay's own user
/// alone on Unix, 0600 (the third review's S5: it was written 0644, so any account on the server
/// machine could read the code and claim the server). An older file is set to 0600 too. On Windows
/// (a relay run by hand on a desktop) it takes the data folder's permissions, which Windows gives
/// the signed-in user, the administrators and SYSTEM, not other users.
pub fn write_claim_code_file_at(file: &std::path::Path, code: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    let mut f = open.open(file)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(
        format!(
            "HumanityOS server claim code: {code}\nThis server has no owner yet. Paste the code into Claim ownership (in the desktop app, Server Settings; on the website, the Claim ownership line in the chat or the command palette) to become its owner. Typing this in the chat works too, and the app keeps the line out of every message:\n  /claim {code}\nYour key card must have reached the server first (it does when you sign in). The code burns on use and this file is deleted.\n"
        )
        .as_bytes(),
    )
}

/// At startup: when no key holds the owner role, mint the claim code, print it to the log and
/// write [`CLAIM_CODE_FILE`]; otherwise remove any stale file. The code, if one was minted.
pub fn mint_claim_code(state: &mut RelayState) -> Option<String> {
    mint_claim_code_at(state, std::path::Path::new(CLAIM_CODE_FILE))
}

/// [`mint_claim_code`] with the file at `file`.
pub fn mint_claim_code_at(state: &mut RelayState, file: &std::path::Path) -> Option<String> {
    *state.claim_code_file.get_mut().unwrap_or_else(|p| p.into_inner()) = file.to_path_buf();
    if state.db.has_owner() {
        let _ = std::fs::remove_file(file);
        *state.claim_code.get_mut() = None;
        return None;
    }
    use rand::Rng;
    // 64 bits from the CSPRNG as 16 hex digits: unguessable (the module's header), short enough to
    // type from the log.
    let code = format!("{:016X}", rand::rng().random::<u64>());
    if let Err(e) = write_claim_code_file_at(file, &code) {
        tracing::warn!("Could not write {}: {e} (the log line below still works)", file.display());
    }
    tracing::info!("==========================================================");
    tracing::info!("SERVER OWNER SETUP: this server has no owner yet.");
    tracing::info!("Claim code:   {code}");
    tracing::info!("Paste it into Claim ownership (Server Settings in the desktop app; the chat's Claim ownership line on the website).");
    tracing::info!("(also saved to {}; it burns on first use)", file.display());
    tracing::info!("==========================================================");
    *state.claim_code.get_mut() = Some(code.clone());
    Some(code)
}

/// What a wrong code costs: count it for `me` (and this connection's `misses`), and the words and
/// the pause before they are said.
fn count_wrong(state: &RelayState, me: &str, misses: &mut u32, now: i64) -> (String, u64) {
    *misses += 1;
    let mut m = state.owner_claim_misses.lock().unwrap_or_else(|p| p.into_inner());
    if now - m.window_start > MISS_WINDOW_MS {
        m.window_start = now;
        m.overall = 0;
    }
    m.overall += 1;
    let mine = m.count(me, now);
    let who = crate::relay::handlers::federation::key_in_log(me);
    if m.overall >= MISSES_OVERALL {
        tracing::warn!("Owner claim: {} wrong codes on this server in the last ten minutes; the latest from {who} ({mine} from that key). Guessing cannot find the code; the right code still works.", m.overall);
    } else {
        tracing::warn!("Owner claim: a wrong code from {who} ({misses} on this connection, {mine} from that key, {} on the server in this window)", m.overall);
    }
    let words = if *misses >= MISSES_PER_CONNECTION || mine >= MISSES_PER_KEY {
        "Wrong claim code, again. Copy it exactly as the server's log (or data/owner-claim-code.txt) shows it: 16 letters and digits. Each wrong code is logged, and a right one always works."
    } else {
        "Wrong claim code. Copy it exactly as the server's log (or data/owner-claim-code.txt) shows it."
    };
    (words.to_string(), if mine >= MISSES_PER_KEY { WRONG_CODE_PAUSE_LONG_MS } else { WRONG_CODE_PAUSE_MS })
}

/// The claim (the module's header): `attempt` from `me`, with `misses` the wrong codes this
/// connection has tried. Whether `me` is now the owner, and the words to tell them.
pub async fn claim(state: &Arc<RelayState>, me: &str, attempt: &str, misses: &mut u32) -> (bool, String) {
    let attempt = attempt.trim().to_ascii_uppercase();
    if me.starts_with("bot_") {
        return (false, "Bots cannot claim a server.".into());
    }
    if attempt.is_empty() {
        return (false, "Paste the claim code from the server's log (or data/owner-claim-code.txt on the server machine) into Claim ownership.".into());
    }
    let now = now_ms();
    let mut slot = state.claim_code.write().await;
    let Some(code) = slot.clone() else {
        return (false, "This server already has an owner. The claim code exists only while a server has none.".into());
    };
    // Always compared, whatever was tried before: the right code always claims.
    if !crate::relay::api::constant_time_eq(attempt.as_bytes(), code.as_bytes()) {
        drop(slot);
        let (words, pause) = count_wrong(state, me, misses, now);
        if !cfg!(test) {
        tokio::time::sleep(std::time::Duration::from_millis(pause)).await;
    }
        return (false, words);
    }
    if state.db.checked_card_at(me).is_none() {
        return (false, "The code is right, but your key card has not reached this server, so held copies could not be sealed to you. Sign in again with the desktop app or the website (each sends your key card when you sign in), then claim again. The code still works.".into());
    }
    match state.db.claim_owner(me, now) {
        Ok(true) => {
            *slot = None;
            drop(slot);
            remove_claim_file(state);
            tracing::info!("Owner claim: {me} is now this server's owner (claimed at {now} ms).");
            let name = state.db.name_for_key(me).ok().flatten().unwrap_or_else(|| me.chars().take(8).collect());
            let _ = state.broadcast_tx.send(RelayMessage::System { message: format!("{name} claimed this server's ownership with its claim code.") });
            let _ = state.broadcast_tx.send(RelayMessage::OwnerState { has_owner: true });
            crate::relay::handlers::broadcast::broadcast_peer_list(state).await;
            (true, "You are now this server's owner, and an admin. The claim code is burned. Held copies will be sealed to your key card. Open Server Settings to manage the server.".into())
        }
        Ok(false) => {
            *slot = None;
            remove_claim_file(state);
            (false, "This server already has an owner.".into())
        }
        Err(e) => {
            tracing::error!("Owner claim: could not save the role: {e}");
            (false, "The claim could not be saved. The server's log says why; the code still works.".into())
        }
    }
}

fn remove_claim_file(state: &RelayState) {
    let file = state.claim_code_file.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let _ = std::fs::remove_file(file);
}

pub(crate) fn reply(state: &RelayState, me: &str, kind: &str, ok: bool, message: String) {
    let _ = state.broadcast_tx.send(RelayMessage::OwnerReply { to: me.to_string(), kind: kind.to_string(), ok, message });
}

/// The frames relay.rs hands here: `owner_claim {code}`, `owner_change_begin {action}`,
/// `owner_change_finish {grant, keys}`, `owner_change_cancel`, and a recipient's
/// `owner_change_accept {action, keys}` and `owner_change_decline {grant, why}` (owner_accept.rs).
/// The frames relay.rs hands here: `owner_claim {code}`, `owner_change_begin {action}`,
/// `owner_change_finish {grant}`, `owner_change_cancel`, and a recipient's `owner_change_accept
/// {action}` and `owner_change_decline {grant, why}` (owner_accept.rs).
pub async fn handle(state: &Arc<RelayState>, me: &str, kind: &str, raw: &Value, misses: &mut u32) {
    match kind {
        "owner_claim" => {
            let (ok, words) = claim(state, me, raw.get("code").and_then(Value::as_str).unwrap_or(""), misses).await;
            // A `/claim` typed in the chat (both apps send it here, never as a message): the answer
            // goes into the chat as well.
            if raw.get("typed").and_then(Value::as_bool) == Some(true) {
                let _ = state.broadcast_tx.send(RelayMessage::Private { to: me.to_string(), message: words.clone() });
            }
            reply(state, me, "claim", ok, words);
        }
        // A begin, a cancel and a decline may roll a change back, and every rollback seals the hides
        // that waited for it (`seal_waiting`, whole files read and written): each on a blocking
        // thread, never on the async runtime (the fifth review's L4), as the finish and the
        // acceptance already were.
        "owner_change_begin" | "owner_change_cancel" | "owner_change_decline" => {
            let (st, who, raw, kind) = (state.clone(), me.to_string(), raw.clone(), kind.to_string());
            let ran = tokio::task::spawn_blocking(move || match kind.as_str() {
                "owner_change_begin" => begin(&st, &who, &raw),
                "owner_change_cancel" => cancel_in(&st, &who, Folders::live()),
                _ => accept::decline(&st, &who, &raw, Folders::live()),
            })
            .await;
            if ran.is_err() {
                reply(state, me, "handover", false, "The server stopped while doing it. Nothing was changed.".into());
            }
        }
        "owner_change_finish" => {
            let folders = OwnedFolders::of(Folders::live());
            let (st, who, raw) = (state.clone(), me.to_string(), raw.clone());
            match tokio::task::spawn_blocking(move || finish_in(&st, &who, &raw, folders.get(), now_ms())).await {
                Ok(Ok(())) => {}
                Ok(Err((kind, why))) => reply(state, me, kind, false, why),
                Err(_) => reply(state, me, "handover", false, "The server stopped while checking it. Nothing was changed.".into()),
            }
        }
        "owner_change_accept" => {
            let folders = OwnedFolders::of(Folders::live());
            let (st, who, raw) = (state.clone(), me.to_string(), raw.clone());
            match tokio::task::spawn_blocking(move || accept::accept_in(&st, &who, &raw, folders.get(), now_ms())).await {
                Ok(Ok(Some(change))) => after_change(state, &change).await,
                Ok(Ok(None)) => {}
                Ok(Err(why)) => reply(state, me, "accept", false, why),
                Err(_) => reply(state, me, "accept", false, "The server stopped while checking it. Nothing was changed.".into()),
            }
        }
        _ => {}
    }
}

/// The `owner_reply` kind for a change.
pub(crate) fn reply_kind(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Handover => "handover",
        ChangeKind::Successor | ChangeKind::SuccessorRemove => "successor",
    }
}

/// `owner_change_cancel` from `me`: their change rolled back, unless it is being carried out.
pub fn cancel_in(state: &RelayState, me: &str, folders: Folders) {
    let carrying = slot(state).carrying.is_some();
    if carrying {
        return reply(state, me, "handover", false, "Too late to cancel: every check passed and the change is being carried out right now.".into());
    }
    roll_back_if(state, folders, |p| p.change.owner == me, "Cancelled. Nothing was changed.", Some("The owner cancelled the change. Nothing was changed."));
}

/// `owner_change_begin` from `me`: check the signed owner action and answer with every held item.
pub fn begin(state: &RelayState, me: &str, raw: &Value) {
    begin_in(state, me, raw, Folders::live(), now_ms());
}

/// [`begin`] with the uploads in `folders` at `now`.
pub fn begin_in(state: &RelayState, me: &str, raw: &Value, folders: Folders, now: i64) {
    let (object, json) = match crate::relay::handlers::held::action_of(raw) {
        Ok(x) => x,
        Err(why) => return reply(state, me, "handover", false, why),
    };
    let c = match state.db.check_owner_change(&object, me, now) {
        Ok(c) => c,
        Err(why) => return reply(state, me, "handover", false, format!("Nothing was done: {why}.")),
    };
    let kind = reply_kind(c.kind);
    // A change the owner started before and did not finish is rolled back first (its new parts
    // removed under the lock), and like every other end of a change, any hide that waited for it is
    // sealed now, to the recipients as they still are (the fourth review's finding 5b: the replace
    // path took the old change and began the new one without sealing, so starting a change again
    // kept the waiting copies unsealed for as long as the owner kept restarting).
    if roll_back_if(state, folders, |_| true, "", Some("The owner started the change again, so the earlier one was rolled back. Nothing was changed.")) {
        tracing::info!("Owner change: the change pending was rolled back by a new begin from {me}; nothing changed.");
    }
    let recipients: Vec<Value> = c.recipients.iter().map(|(k, at)| json!({ "key": k, "card_at": at })).collect();
    let (grant, action, target) = (c.object_id.clone(), c.kind.action().to_string(), c.target.clone());
    let items = {
        let mut s = slot(state);
        if s.carrying.is_some() {
            drop(s);
            return reply(state, me, kind, false, "Nothing was done: a change is being carried out right now. Try again in a moment.".into());
        }
        if s.sealing > 0 {
            drop(s);
            return reply(state, me, kind, false, "Nothing was done: a reported post is being sealed to the owner's key card right now. Try again in a moment.".into());
        }
        if s.pending.is_some() {
            drop(s);
            return reply(state, me, kind, false, "Nothing was done: another change began at the same moment. Try again in a moment.".into());
        }
        // The held items read under the lock, after the waiting copies were sealed, so the new
        // change covers every one of them.
        let rows = state.db.held_items().unwrap_or_default();
        if rows.iter().any(|r| !r.opens_for(me)) {
            drop(s);
            return reply(state, me, kind, false, "Nothing was done: a held copy is sealed to an earlier owner's key card, so your device cannot open it to seal it again. Destroy it or wait for it to expire, then try again.".into());
        }
        let items: Vec<Value> = rows.iter().map(|r| json!({ "id": r.id, "package": r.package, "parts": r.parts })).collect();
        let ids = rows.iter().map(|r| r.id).collect();
        let round = hex::encode(rand::random::<[u8; 16]>());
        s.pending = Some(PendingHandover { change: c, round, object_json: json, started_at: now, ids, checked_at: None, building: false, accepted: Vec::new() });
        items
    };
    let _ = state.broadcast_tx.send(RelayMessage::OwnerChangeReady { to: me.to_string(), grant, action, target, recipients, items });
}

/// The longest a hide may wait for its sealed copy: a change's whole life (its re-wrap window, then
/// its acceptance window) and a margin (the fourth review's finding 5b).
pub const WAITING_CAP_MS: i64 = HANDOVER_WINDOW_MS + ACCEPT_WINDOW_MS + 10 * 60 * 1000;

/// Each minute (relay/mod.rs), after [`expire_pending`]: a hide that has waited longer than
/// [`WAITING_CAP_MS`] is sealed at once to the current recipients, and the change it waits for is
/// rolled back to let it (said to the owner in words). A change never outlives its own windows, so
/// this only acts when something kept one past them (the clock moved, a stuck carry-out); the wait
/// has a bound whatever happens. Never while a change is being carried out.
pub fn cap_waiting_at(state: &RelayState, folders: Folders, now: i64) {
    let Some(oldest) = state.db.held_oldest_waiting_in(folders) else { return };
    if now - oldest <= WAITING_CAP_MS {
        return;
    }
    roll_back_if(
        state,
        folders,
        |_| true,
        "A reported post waited longer than an owner change may last for its sealed copy, so the change was rolled back and the post sealed to the owner's key card as it is now. Nothing was changed; start the change again.",
        Some("The owner's change was rolled back because a reported post had waited too long for it. Nothing was changed."),
    );
    seal_waiting(state, folders);
}

/// The started change under `grant`, if it is still within its window ([`PendingHandover::live`]).
/// Naming it is not enough to act on it: every route also checks who signed the request.
pub fn pending_for(state: &RelayState, grant: &str, now: i64) -> Option<PendingHandover> {
    slot(state).pending.as_ref().filter(|p| !grant.is_empty() && p.change.object_id == grant && p.live(now)).cloned()
}

/// Roll the pending change back when `which` says so (cancelled, declined, a bad acceptance, or past
/// its window), under the slot's lock: its new parts removed, the role and every original
/// untouched. Never while a change is being carried out. `words` (when not empty) go to the owner,
/// and `recipients_too` to each recipient who was asked to check. Whether it rolled one back.
pub fn roll_back_if(state: &RelayState, folders: Folders, which: impl FnOnce(&PendingHandover) -> bool, words: &str, recipients_too: Option<&str>) -> bool {
    let p = {
        let mut s = slot(state);
        if s.carrying.is_some() || !s.pending.as_ref().is_some_and(which) {
            return false;
        }
        let Some(p) = s.pending.take() else { return false };
        state.db.owner_handover_clear_staging_in(folders);
        p
    };
    if !words.is_empty() {
        reply(state, &p.change.owner, reply_kind(p.change.kind), false, words.to_string());
    }
    if let (Some(told), Some(_)) = (recipients_too, p.checked_at) {
        for (key, _) in p.change.recipients.iter().filter(|(k, _)| *k != p.change.owner) {
            reply(state, key, "accept", false, told.to_string());
        }
    }
    tracing::info!("Owner change: {} by {} rolled back; nothing changed.", p.change.kind.action(), p.change.owner);
    // A hide that waited for this change is sealed now, to the recipients as they still are (S3).
    seal_waiting(state, folders);
    true
}

/// [`roll_back_if`] whatever the pending change is.
pub fn roll_back(state: &RelayState, folders: Folders, words: &str, recipients_too: Option<&str>) -> bool {
    roll_back_if(state, folders, |_| true, words, recipients_too)
}

/// Each minute (relay/mod.rs): roll back a change past its window, and say so.
pub fn expire_pending(state: &RelayState, folders: Folders) {
    expire_pending_at(state, folders, now_ms());
}

/// [`expire_pending`] at `now`: the window is judged under the lock, so a change carried out or
/// begun meanwhile is never the one rolled back.
pub fn expire_pending_at(state: &RelayState, folders: Folders, now: i64) {
    roll_back_if(
        state,
        folders,
        |p| !p.live(now),
        "The change ran out of time before everyone it needs had checked the copies on their own device, so it was rolled back. Nothing was changed. Ask them to be signed in, then start again.",
        Some("The owner's change ran out of time before your device checked the copies, so it was rolled back. Nothing was changed."),
    );
}

#[derive(serde::Deserialize)]
pub struct HeaderQuery {
    #[serde(default)]
    pub grant: String,
}

fn refuse(code: StatusCode, words: impl Into<String>) -> Response {
    (code, Json(json!({ "error": words.into() }))).into_response()
}

/// The held item `id` the owner's change under `grant` is re-wrapping, for `caller` (the key the
/// request's signature proved): only the owner whose change it is, still the owner, while it
/// re-wraps, for an item it covers and that opens for them.
fn rewrapping(state: &RelayState, id: i64, grant: &str, caller: &str, now: i64) -> Option<(PendingHandover, crate::relay::storage::held::HeldRow)> {
    let p = pending_for(state, grant, now).filter(|p| p.uploading() && !p.building && p.change.owner == caller && p.ids.contains(&id))?;
    if state.db.owner_keys().unwrap_or_default() != [p.change.owner.clone()] {
        return None;
    }
    let row = state.db.held_item(id).ok().flatten().filter(|r| r.opens_for(&p.change.owner))?;
    Some((p, row))
}

/// The CURRENT header of part `part` of held item `id`, for the owner re-wrapping it under `grant`
/// ([`rewrapping`]): a header alone, never a body. None for anything else.
pub fn current_header_in(state: &RelayState, id: i64, part: i64, grant: &str, caller: &str, folders: Folders, now: i64) -> Option<Vec<u8>> {
    let (_, row) = rewrapping(state, id, grant, caller, now)?;
    let part = usize::try_from(part).ok().filter(|p| (*p as i64) < row.parts)?;
    read_header(&crate::relay::storage::held::held_dir_beside(folders).join(&row.package).join(crate::relay::storage::held::part_file(part))).ok()
}

/// `GET /api/held/{id}/{part}/header?grant=`, signed by the owner (held.rs `signed_caller`): the
/// part's current header for the owner's device to re-wrap; 404 for anything else.
pub async fn api_current_header(State(state): State<Arc<RelayState>>, Path((id, part)): Path<(i64, i64)>, Query(q): Query<HeaderQuery>, headers: HeaderMap) -> Response {
    use crate::relay::handlers::held::{caller_purpose, signed_caller};
    let Some(caller) = signed_caller(&state, &headers, &caller_purpose("header", &q.grant, id, part, 0)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match current_header_in(&state, id, part, &q.grant, &caller, Folders::live(), now_ms()) {
        Some(h) => ([(axum::http::header::CONTENT_TYPE, "application/octet-stream")], h).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The new header `body` of part `part` of held item `id`, sent by `caller` re-wrapping it under
/// `grant` ([`rewrapping`]; storage/owner_handover.rs `owner_take_new_header`): a header alone,
/// for as many people as the change names, with the commitment of the header it replaces.
///
/// Taken while holding the change slot's lock, re-checked there (the third review's S1): the change
/// is the same round, still re-wrapping, and its finish is not building the new parts. So a header
/// sent on another socket can never land while the finish builds, nor after it (a modified owner
/// app racing a second finish against header sends could otherwise have the carried-out headers
/// differ from what a recipient checked). Lock order: the slot, then the held-folder lock, as
/// everywhere.
#[allow(clippy::too_many_arguments)]
pub fn new_header_in(state: &RelayState, id: i64, part: i64, grant: &str, caller: &str, body: &[u8], folders: Folders, now: i64) -> Response {
    let Some((p, row)) = rewrapping(state, id, grant, caller, now) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(part) = usize::try_from(part).ok().filter(|p| (*p as i64) < row.parts) else {
        return refuse(StatusCode::NOT_FOUND, "This held item has no such part.");
    };
    #[cfg(test)]
    BEFORE_HEADER_LOCK.with(|h| if let Some(f) = h.borrow_mut().as_mut() { f() });
    let held = slot(state);
    if !held.pending.as_ref().is_some_and(|q| q.round == p.round && q.uploading() && !q.building) {
        return refuse(StatusCode::CONFLICT, "Every new header of this change was already sent and is being checked, so this one was not taken. Nothing was changed.");
    }
    let taken = state.db.owner_take_new_header(&row, folders, part, p.change.recipients.len(), body);
    drop(held);
    match taken {
        Ok(()) => Json(json!({ "done": true })).into_response(),
        Err(HeaderRefused::NotAHeader) => refuse(StatusCode::PAYLOAD_TOO_LARGE, "A change takes a part's new header alone, for the people it names. Body bytes are never taken."),
        Err(HeaderRefused::Mismatch) => refuse(StatusCode::BAD_REQUEST, "This is not a header for the people the change names, so it was not taken."),
        Err(HeaderRefused::Commitment) => refuse(StatusCode::BAD_REQUEST, "This header is not for the key this part is sealed under, so it would not open the part. It was not taken."),
        Err(HeaderRefused::Disk(e)) => {
            tracing::error!("Owner change: a new header of #{id} could not be written: {e}");
            refuse(StatusCode::INTERNAL_SERVER_ERROR, "The server could not write this header.")
        }
    }
}

#[cfg(test)]
thread_local! {
    /// A test's step between [`new_header_in`]'s outer check and its lock (owner_tests.rs, the fourth
    /// review's finding 7: a finish starting in that gap is what the re-check under the lock is for).
    pub(crate) static BEFORE_HEADER_LOCK: std::cell::RefCell<Option<Box<dyn FnMut()>>> = const { std::cell::RefCell::new(None) };
}

/// `PUT /api/held/{id}/{part}/header?grant=`, signed by the owner (held.rs `signed_caller`): the
/// part's new header, re-wrapped by the owner's device for the new recipients.
pub async fn api_new_header(State(state): State<Arc<RelayState>>, Path((id, part)): Path<(i64, i64)>, Query(q): Query<HeaderQuery>, headers: HeaderMap, body: Bytes) -> Response {
    use crate::relay::handlers::held::{caller_purpose, signed_caller};
    let Some(caller) = signed_caller(&state, &headers, &caller_purpose("rewrap", &q.grant, id, part, 0)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let owned = OwnedFolders::of(Folders::live());
    tokio::task::spawn_blocking(move || new_header_in(&state, id, part, &q.grant, &caller, &body, owned.get(), now_ms()))
        .await
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// The held items the pending change `p` covers, as they are now: the ones destroyed, expired or
/// restored since it began are left out; a post held since it began fails it (its copy is sealed to
/// the old recipients and was never re-wrapped).
pub(crate) fn covered_rows(state: &RelayState, p: &PendingHandover) -> Result<Vec<crate::relay::storage::held::HeldRow>, String> {
    let rows = state.db.held_items().map_err(|e| format!("the server could not read its held items ({e})"))?;
    if rows.iter().any(|r| !p.ids.contains(&r.id)) {
        return Err("a post was held while it ran, so its copy was not re-wrapped; start again".into());
    }
    Ok(rows)
}

/// `owner_change_finish {grant}` from `me`: check every new header again and build each new part
/// (storage/owner_handover.rs `owner_build_new_parts`), then ask each recipient to check and accept
/// (phase 2). Nothing changes here. Its reply kind and why not, when it did not get that far.
///
/// The slot is marked BUILDING under its lock before anything is checked (the third review's S1:
/// the round-3 finish checked `uploading()` once and built unlocked, so a second finish or a header
/// sent meanwhile could change what was built): a second finish is refused while one builds, every
/// header route refuses meanwhile ([`rewrapping`], [`new_header_in`]), and the mark is cleared
/// however the finish ends.
pub fn finish_in(state: &RelayState, me: &str, raw: &Value, folders: Folders, now: i64) -> Result<(), (&'static str, String)> {
    let grant = raw.get("grant").and_then(Value::as_str).unwrap_or("");
    let p = {
        let mut s = slot(state);
        match s.pending.as_mut().filter(|q| !grant.is_empty() && q.change.object_id == grant && q.live(now) && q.change.owner == me && q.uploading()) {
            Some(q) if q.building => {
                let kind = reply_kind(q.change.kind);
                return Err((kind, "Every new header is being checked right now (a finish was already sent), so this one was not taken. Nothing was changed.".into()));
            }
            Some(q) => {
                q.building = true;
                q.clone()
            }
            None => return Err(("handover", "Nothing of yours is waiting (it may have run past its 30 minutes). Start it again. Nothing was changed.".into())),
        }
    };
    let c = &p.change;
    let kind = reply_kind(c.kind);
    let built = (|| -> Result<Vec<crate::relay::storage::held::HeldRow>, String> {
        for (key, card_at) in &c.recipients {
            if state.db.checked_card_at(key) != Some(*card_at) {
                return Err("A key card changed while it ran, so the copies sealed to the old one would not open. Start again. Nothing was changed.".into());
            }
        }
        if c.kind != ChangeKind::SuccessorRemove && state.db.is_banned(&c.target).unwrap_or(true) {
            return Err("The person named was banned while it ran. Nothing was changed.".into());
        }
        let rows = covered_rows(state, &p).map_err(|why| format!("Nothing was changed: {why}."))?;
        state.db.owner_build_new_parts(&rows, c.recipients.len(), folders).map_err(|why| format!("Nothing was changed: {why}."))?;
        Ok(rows)
    })();
    let checked = {
        let mut s = slot(state);
        let Some(q) = s.pending.as_mut().filter(|q| q.round == p.round && q.uploading()) else {
            return Err((kind, built.err().unwrap_or_else(|| "The change was cancelled while it was checked. Nothing was changed.".into())));
        };
        q.building = false;
        if let Err(why) = &built {
            return Err((kind, why.clone()));
        }
        q.checked_at = Some(now);
        q.clone()
    };
    let rows = built.unwrap_or_default();
    tracing::info!("Owner change: {} by {} naming {}: every new header arrived ({} held item(s)); waiting for the recipients to check them.", c.kind.action(), c.owner, c.target, rows.len());
    accept::ask_recipients(state, &checked, &rows);
    Ok(())
}

/// Tell everyone what changed, and the people it touches what it means for them.
pub(crate) async fn after_change(state: &Arc<RelayState>, c: &OwnerChange) {
    let name = |k: &str| state.db.name_for_key(k).ok().flatten().unwrap_or_else(|| k.chars().take(8).collect());
    let (owner_name, target_name) = (name(&c.owner), name(&c.target));
    let tell = |to: &str, message: String| {
        let _ = state.broadcast_tx.send(RelayMessage::Private { to: to.to_string(), message });
    };
    match c.kind {
        ChangeKind::Handover => {
            reply(state, &c.owner, "handover", true, format!("{target_name} checked every held copy on their device and accepted: they are now this server's owner, and every held copy is sealed to their key card. You are an admin."));
            tell(&c.target, format!("You are now this server's owner: {owner_name} handed you the role, and every held copy is sealed to your key card. Only your device can open one. You can name a successor in Server Settings."));
            let _ = state.broadcast_tx.send(RelayMessage::System { message: format!("{owner_name} handed this server's ownership to {target_name}.") });
            crate::relay::handlers::broadcast::broadcast_peer_list(state).await;
        }
        ChangeKind::Successor => {
            reply(state, &c.owner, "successor", true, format!("{target_name} is your successor: every held copy is now sealed to your key card and theirs, each checked on both devices. The server gives a held copy to the owner alone, so they can open one only once they hold the owner role."));
            tell(&c.target, format!("You are now this server's successor ({owner_name} named you). Held copies are also sealed to your key card, so if you ever hold the owner role you can open them. Until then you have no extra powers: the server gives held copies to the owner alone."));
        }
        ChangeKind::SuccessorRemove => {
            reply(state, &c.owner, "successor", true, format!("{target_name} is no longer your successor: every held copy is sealed to your key card alone again, checked on your device alone."));
            tell(&c.target, format!("{owner_name} removed you as this server's successor."));
        }
    }
    crate::relay::handlers::held::handle_list(state, &c.owner);
    // A successor who is not an admin sees no held items list (and is not told "Only admins can see
    // held items." for something they did not ask for).
    if crate::relay::handlers::msg_handlers::is_admin_role(&state.db.get_role(&c.target).unwrap_or_default()) {
        crate::relay::handlers::held::handle_list(state, &c.target);
    }
}

/// Why `actor` may not `action` the people in `keys`, when one of them is the server's owner: no
/// admin can remove, demote, ban, mute, kick, warn, verify, unverify, set a donor mark on, release
/// the name of, or game-ban the owner, and only the owner hands the role over. None when none of
/// them is the owner, or for an action that does no harm.
pub fn refuse_if_owner(state: &RelayState, keys: &[String], action: &str) -> Option<String> {
    let harms = matches!(
        action.trim_start_matches('/'),
        "kick" | "ban" | "mute" | "warn" | "mod" | "unmod" | "verify" | "unverify" | "donor" | "name-release" | "game_ban" | "erase" | "remove" | "revoke_role" | "grant_role"
    );
    (harms && keys.iter().any(|k| state.db.is_owner(k))).then(|| {
        format!("Nobody can {} this server's owner. Only the owner can hand the role to someone else.", owner_verb(action.trim_start_matches('/')))
    })
}

/// How a harmful action reads in the refusal ("Nobody can <verb> this server's owner").
fn owner_verb(action: &str) -> String {
    match action {
        "donor" => "change the donor mark of".into(),
        "name-release" => "release the name of".into(),
        "unverify" => "take the verified mark from".into(),
        other => other.replace('_', " "),
    }
}

// A recipient's check and acceptance of an owner change (phase 2, the owner review's finding 5).
#[path = "owner_accept.rs"]
pub(crate) mod accept;

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;

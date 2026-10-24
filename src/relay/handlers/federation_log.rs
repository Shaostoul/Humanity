//! Federation's numbered log: what a server sends its links, how a link catches up, and what a
//! linked server's edits, deletes and bans do here (2026-10-22, docs/design/federation-activation.md
//! Phases 2 and 3; the stored half is storage/federation_log.rs).
//!
//! THE PROBLEM IT SOLVES. Until now a federated chat line was sent once, live, to whichever links
//! were up, and a receiver refused it five minutes later as a possible replay. So a linked server
//! that was down, restarting, or (from Phase 2 on) paused for sending too fast simply never had
//! what was said meanwhile, and nothing but a chat line crossed at all: an edit, a delete or a ban
//! on one server left the other showing the old words, the deleted words, and the banned person.
//!
//! THE LOG. Everything a server sends its links goes into its outbox first and gets the next
//! number ([`publish`]): a chat line, an edit, a delete, an account's erase, a ban or unban, a
//! profile copy, a signed object. A link is sent `fed_event {log, seq, event}`, or `fed_skip {seq}`
//! for a number not meant for it (a frame passed on from that very link) or scrubbed (the words of
//! a deleted message). The receiver applies numbers strictly in order and remembers the last one
//! (its cursor, stored): a number at or below it is a repeat and is ignored; one past the next is
//! a gap, so it asks the sender for everything after its cursor (`fed_resume`), and the sender
//! answers in batches of [`CHUNK`] ending with `fed_batch_end`, pulled one at a time. A link that
//! comes up asks the same way. So nothing is lost and nothing applies twice, whether the link was
//! down, the receiver was restarted, or it paused the sender (federation_flow.rs). A receiver
//! behind by more than the outbox keeps is told `fed_gap` (and counts what it missed), and is sent
//! the sender's current ban list and profile copies so that state still agrees.
//!
//! WHAT "HONOURED" MEANS (decided 2026-10-22, written in docs/design/federation-activation.md):
//! - A message belongs to the server it was first posted on (its ORIGIN). The origin's edit or
//!   delete (by the author or a moderator there) is applied to every linked server's copy, and an
//!   account erased there has its messages removed from every copy. A linked server's own
//!   moderators can still remove a copy from their own server; that stays local, since it is not
//!   their message to remove from anyone else's.
//! - A ban on one server is honoured in the ROOMS IT SHARES: every linked server (tier 2 or above)
//!   refuses that key's posts in its federated rooms, and the banning server itself refuses that
//!   key's words arriving from any link. Each server's own rooms, and whether the person may sign
//!   in there at all, stay that server's call. Unbanning lifts it everywhere; removing a link, or
//!   lowering it below tier 2, lifts that server's bans here.
//! - Kicks and mutes are local. They are brief by nature, and sharing them would let one server's
//!   moment of moderation silence someone everywhere.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use crate::relay::handlers::federation::{key_in_log, sign_with_server_key};
use crate::relay::handlers::federation_flow::{Admit, FlowLimits, Kind, PeerFlow};
use crate::relay::relay::{RelayMessage, RelayState};

/// How many numbered frames one batch of a catch-up carries.
pub const CHUNK: usize = 100;
/// A link this many frames behind in its own queue (it is not reading) is sent nothing live until
/// it has caught up; it then sees a gap and asks for what it missed.
pub const QUEUE_MAX: usize = 2_000;
/// An unanswered request to catch up is made again after this long.
pub const ASK_AGAIN_AFTER: Duration = Duration::from_secs(5);
/// The pause between the batches of a long catch-up, so the pulls stay inside the sender's
/// allowance for them (2 a second).
pub const BETWEEN_BATCHES: Duration = Duration::from_millis(500);
/// How many bans one `fed_bans` frame carries (about 80 KB, under the socket's 128 KB message).
const BANS_PER_FRAME: usize = 20;

/// Whether this server sends a link its frames live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sending {
    /// Up, but it has not said where it is in this server's log yet.
    Waiting,
    /// Catching up, batch by batch; live frames would arrive out of order.
    Replaying,
    /// Caught up: every new frame goes as it is made.
    Streaming,
}

#[derive(Debug)]
struct LinkOut {
    sending: Sending,
    /// Frames queued for the link's writer and not yet written.
    queued: Arc<AtomicUsize>,
}

/// One linked server as this server receives it: its flow, and this server's place in its log.
#[derive(Debug, Default)]
struct PeerIn {
    flow: PeerFlow,
    /// Its outbox's id and the last number applied (`federation_cursors`).
    log: String,
    last: i64,
    loaded: bool,
    /// When this server last asked it to send again, while that is unanswered.
    asked_at: Option<Instant>,
    /// It sent frames that are not numbered (profile copies, a ban list) while paused: ask for
    /// them again when asking to catch up.
    catch_up_wanted: bool,
    /// A ban list arriving in parts, until its last part.
    bans_incoming: Option<Vec<(String, String, i64)>>,
}

/// Federation's state on this server (`RelayState::fed`).
#[derive(Debug, Default)]
pub struct FedState {
    /// The flow allowances in force (federation_flow.rs; tests set small ones).
    pub limits: RwLock<FlowLimits>,
    ins: Mutex<HashMap<String, PeerIn>>,
    /// Each link as this server sends to it. Held while appending to the outbox and sending, so
    /// every link sees numbers in order.
    outs: Mutex<HashMap<String, LinkOut>>,
    log_id: OnceLock<String>,
    appended: AtomicUsize,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// This server's outbox id.
fn log_id(state: &RelayState) -> String {
    state.fed.log_id.get_or_init(|| state.db.fed_log_id().unwrap_or_default()).clone()
}

fn limits(state: &RelayState) -> FlowLimits {
    state.fed.limits.read().map(|l| l.clone()).unwrap_or_default()
}

/// The frame a link is sent for outbox row `seq` holding `event` (already JSON).
fn event_frame(log: &str, seq: i64, event: &str) -> String {
    format!("{{\"type\":\"fed_event\",\"log\":{},\"seq\":{seq},\"event\":{event}}}", json!(log))
}

fn skip_frame(log: &str, seq: i64) -> String {
    json!({ "type": "fed_skip", "log": log, "seq": seq }).to_string()
}

// ── Sending ──────────────────────────────────────────────────────────────────────────────────

/// Add `event` to the outbox and send it to every link that is caught up. `kind` names it in the
/// outbox ("chat", "edit", "delete", "erase", "mod", "profile", "object"); `author` and `stamp`
/// say whose message it carries (so its words can be scrubbed); `not_to` is the link it came from,
/// which is sent a skip instead. Returns its number (None when the outbox could not be written).
pub async fn publish(state: &Arc<RelayState>, kind: &str, author: &str, stamp: u64, not_to: &str, event: &Value) -> Option<i64> {
    let text = event.to_string();
    let log = log_id(state);
    let conns = state.federation_connections.read().await;
    let seq = {
        let outs = state.fed.outs.lock().ok()?;
        let seq = match state.db.fed_outbox_append(kind, author, stamp, not_to, &text) {
            Ok(seq) => seq,
            Err(e) => {
                tracing::error!("Federation: could not add a {kind} frame to the outbox: {e}");
                return None;
            }
        };
        for conn in conns.values() {
            let out = outs.get(&conn.server_id);
            // A link with no entry is one registered without link_up (only tests do that): live.
            if out.is_some_and(|o| o.sending != Sending::Streaming || o.queued.load(Ordering::Relaxed) > QUEUE_MAX) {
                continue;
            }
            let frame = if !not_to.is_empty() && conn.server_id == not_to { skip_frame(&log, seq) } else { event_frame(&log, seq, &text) };
            if conn.tx.send(frame).is_ok() {
                if let Some(o) = out {
                    o.queued.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        seq
    };
    drop(conns);
    // Prune now and then, keeping no more days than the server keeps its own messages.
    if state.fed.appended.fetch_add(1, Ordering::Relaxed) % 500 == 0 {
        let days = state.db.get_server_settings().map(|s| s.message_retention_days).unwrap_or(0);
        let days = if days > 0 { days as u64 } else { crate::relay::storage::federation_log::OUTBOX_DAYS_KEPT };
        let _ = state.db.fed_outbox_prune(days);
    }
    Some(seq)
}

/// A link came up: it waits for nothing live until it says where it is in this server's log, and
/// this server asks it where it left off in ITS log. Returns the link's queue counter, which its
/// writer counts down.
pub async fn link_up(state: &Arc<RelayState>, peer: &str, tx: &UnboundedSender<String>) -> Arc<AtomicUsize> {
    let queued = Arc::new(AtomicUsize::new(0));
    if let Ok(mut outs) = state.fed.outs.lock() {
        outs.insert(peer.to_string(), LinkOut { sending: Sending::Waiting, queued: queued.clone() });
    }
    ask(state, peer, tx).await;
    queued
}

/// A link went down.
pub fn link_down(state: &RelayState, peer: &str) {
    if let Ok(mut outs) = state.fed.outs.lock() {
        outs.remove(peer);
    }
    if let Ok(mut ins) = state.fed.ins.lock() {
        if let Some(pin) = ins.get_mut(peer) {
            pin.asked_at = None;
            pin.bans_incoming = None;
        }
    }
}

/// The writer wrote one frame.
pub fn written(queued: &AtomicUsize) {
    let _ = queued.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |q| Some(q.saturating_sub(1)));
}

/// `peer` asked for everything after `from` in this server's log (`their_log` is the log id it
/// knows): one batch, then where the batch ended. A peer that knows another log (this server's
/// database was replaced) or none starts from nothing; one behind what the outbox still keeps is
/// told the gap and sent the current ban list and profile copies.
async fn serve_resume(state: &Arc<RelayState>, peer: &str, tx: &UnboundedSender<String>, their_log: &str, from: i64, catch_up: bool) {
    let log = log_id(state);
    let gap = {
        let Ok(mut outs) = state.fed.outs.lock() else { return };
        let mut from = if their_log == log { from.max(0) } else { 0 };
        let first_sync = from == 0;
        let (oldest, newest) = state.db.fed_outbox_bounds().unwrap_or((0, 0));
        let gone_upto = if oldest > 0 { oldest - 1 } else { newest };
        let mut frames = Vec::new();
        let gap = from < gone_upto;
        if gap {
            frames.push(json!({ "type": "fed_gap", "log": log, "upto": gone_upto }).to_string());
            from = gone_upto;
        }
        let rows = state.db.fed_outbox_after(from, CHUNK).unwrap_or_default();
        let mut upto = from;
        for row in rows {
            // Numbers are never reused, but a prune by age can leave a hole; say so, or the peer
            // would see a gap and ask for the same numbers again forever.
            if row.seq > upto + 1 {
                frames.push(json!({ "type": "fed_gap", "log": log, "upto": row.seq - 1 }).to_string());
            }
            frames.push(if row.frame.is_empty() || row.not_to == peer { skip_frame(&log, row.seq) } else { event_frame(&log, row.seq, &row.frame) });
            upto = row.seq;
        }
        let more = upto < newest;
        frames.push(json!({ "type": "fed_batch_end", "log": log, "upto": upto, "more": more }).to_string());
        // The whole ban list, inside the same lock, so no ban or unban published meanwhile can
        // land between the list and the numbers before it.
        if first_sync || gap || catch_up {
            frames.extend(ban_list_frames(state));
        }
        let out = outs.entry(peer.to_string()).or_insert_with(|| LinkOut { sending: Sending::Waiting, queued: Arc::new(AtomicUsize::new(0)) });
        out.sending = if more { Sending::Replaying } else { Sending::Streaming };
        for f in frames {
            if tx.send(f).is_ok() {
                out.queued.fetch_add(1, Ordering::Relaxed);
            }
        }
        gap
    };
    if gap || catch_up {
        let n = crate::relay::handlers::federation::send_own_copies_again(state, peer).await;
        tracing::info!("Federation: {} asked to catch up; sent the ban list and {n} profile copies", key_in_log(peer));
    }
}

/// This server's whole ban list, in parts, for a link that could not replay every ban: the last
/// part tells the receiver to keep exactly these.
fn ban_list_frames(state: &RelayState) -> Vec<String> {
    let mut frames = Vec::new();
    let banned: Vec<(String, String, i64)> = state
        .db
        .list_banned()
        .unwrap_or_default()
        .into_iter()
        .filter(|b| !b.public_key.is_empty() && !b.public_key.starts_with("bot_"))
        .map(|b| (b.public_key, b.name, b.banned_at))
        .collect();
    // An empty list is still sent (one part, the last), so the receiver forgets bans lifted here.
    let none: &[(String, String, i64)] = &[];
    let parts: Vec<&[(String, String, i64)]> = if banned.is_empty() { vec![none] } else { banned.chunks(BANS_PER_FRAME).collect() };
    let count = parts.len();
    for (i, part) in parts.into_iter().enumerate() {
        let last = i + 1 == count;
        let ts = now_ms();
        let keys: Vec<&str> = part.iter().map(|(k, _, _)| k.as_str()).collect();
        let Some(sig) = sign_with_server_key(&state.db, &format!("fed_bans\n{i}\n{last}\n{}\n{ts}", keys.join(","))) else { return Vec::new() };
        let bans: Vec<Value> = part.iter().map(|(k, n, at)| json!({ "key": k, "name": n, "at": at })).collect();
        frames.push(json!({ "type": "fed_bans", "part": i, "last": last, "bans": bans, "timestamp": ts, "signature": sig }).to_string());
    }
    frames
}

/// Ask `peer` to send everything after this server's place in its log.
async fn ask(state: &Arc<RelayState>, peer: &str, tx: &UnboundedSender<String>) {
    let frame = {
        let Ok(mut ins) = state.fed.ins.lock() else { return };
        let pin = place(state, &mut ins, peer);
        pin.asked_at = Some(Instant::now());
        pin.flow.asked = CHUNK as u32 + 3;
        let catch_up = std::mem::take(&mut pin.catch_up_wanted);
        json!({ "type": "fed_resume", "log": pin.log, "from": pin.last, "catch_up": catch_up }).to_string()
    };
    let _ = tx.send(frame);
}

/// `peer`'s place, loaded from the database the first time.
fn place<'a>(state: &RelayState, ins: &'a mut HashMap<String, PeerIn>, peer: &str) -> &'a mut PeerIn {
    let pin = ins.entry(peer.to_string()).or_default();
    if !pin.loaded {
        if let Ok(Some((log, last))) = state.db.fed_cursor(peer) {
            pin.log = log;
            pin.last = last;
        }
        pin.loaded = true;
    }
    pin
}

// ── Receiving ────────────────────────────────────────────────────────────────────────────────

/// The kind a frame counts as, and whether it answers this server's request to catch up.
fn kind_of(v: &Value) -> (Kind, bool) {
    let ty = v["type"].as_str().unwrap_or("");
    let inner = |t: &str| match t {
        "federated_chat" => Kind::Chat,
        "fed_edit" => Kind::Edit,
        "fed_delete" | "fed_erase" => Kind::Delete,
        "fed_mod" => Kind::Moderation,
        "profile_gossip" => Kind::Profile,
        "signed_object_gossip" => Kind::Object,
        _ => Kind::Control,
    };
    match ty {
        "fed_event" => (inner(v["event"]["type"].as_str().unwrap_or("")), true),
        "fed_skip" | "fed_gap" | "fed_batch_end" => (Kind::Control, true),
        "fed_resume" => (Kind::Resume, false),
        "fed_bans" => (Kind::Moderation, false),
        other => (inner(other), false),
    }
}

/// How long a read pump may wait for `peer`'s next frame: `most`, or less when a pause ends sooner
/// or an unanswered request is due again (so neither waits for the link to fall silent).
pub fn read_wait(state: &RelayState, peer: &str, most: Duration) -> Duration {
    let Ok(ins) = state.fed.ins.lock() else { return most };
    let Some(pin) = ins.get(peer) else { return most };
    let now = Instant::now();
    let pause_ends = pin.flow.paused_until().map(|u| u.saturating_duration_since(now));
    let ask_due = pin.asked_at.map(|at| (at + ASK_AGAIN_AFTER).saturating_duration_since(now));
    [Some(most), pause_ends, ask_due].into_iter().flatten().min().unwrap_or(most) + Duration::from_millis(10)
}

/// Called when a read pump's wait ends with nothing read: a pause that ended asks for the catch-up,
/// and a request left unanswered is made again.
pub async fn tick(state: &Arc<RelayState>, peer: &str, tx: &UnboundedSender<String>) {
    let now = Instant::now();
    let again = {
        let Ok(mut ins) = state.fed.ins.lock() else { return };
        let Some(pin) = ins.get_mut(peer) else { return };
        pin.flow.take_resume_due(now) || pin.asked_at.is_some_and(|at| now.saturating_duration_since(at) >= ASK_AGAIN_AFTER)
    };
    if again {
        ask(state, peer, tx).await;
    }
}

/// Everything a linked server sends goes through here: its flow (slowed, or set aside while it is
/// paused), then its place in that server's log, then what the frame does.
pub async fn ingest(state: &Arc<RelayState>, peer: &str, tx: &UnboundedSender<String>, text: &str) {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return };
    let ty = v["type"].as_str().unwrap_or("").to_string();
    let (kind, answers) = kind_of(&v);
    let limits = limits(state);
    let now = Instant::now();
    let (admit, resume) = {
        let Ok(mut ins) = state.fed.ins.lock() else { return };
        let pin = place(state, &mut ins, peer);
        let asked = answers && pin.flow.asked > 0;
        if asked {
            pin.flow.asked -= 1;
        }
        let pauses_before = pin.flow.pauses;
        let admit = pin.flow.admit(kind, asked, &limits, now);
        if admit == Admit::Paused && !answers && matches!(kind, Kind::Profile | Kind::Moderation) {
            pin.catch_up_wanted = true;
        }
        if pin.flow.pauses > pauses_before {
            tracing::warn!(
                "Federation: {} sent faster than its allowance for too long and is paused (pause {} in a row); what it sends meanwhile is asked for again after",
                key_in_log(peer),
                pin.flow.pauses
            );
        }
        // A request to catch up left unanswered (set aside by a peer that paused this server, or
        // lost with a link) is made again, whether or not frames keep arriving.
        let ask_again = pin.asked_at.is_some_and(|at| now.saturating_duration_since(at) >= ASK_AGAIN_AFTER);
        let pause_over = pin.flow.take_resume_due(now);
        if pause_over {
            tracing::info!("Federation: the pause on {} is over; asking it for what was set aside", key_in_log(peer));
        }
        (admit, pause_over || ask_again)
    };
    if resume {
        ask(state, peer, tx).await;
    }
    match admit {
        Admit::Paused => return,
        Admit::Slow(wait) => tokio::time::sleep(wait).await,
        Admit::Go => {}
    }
    match ty.as_str() {
        "fed_resume" => {
            let their_log = v["log"].as_str().unwrap_or("");
            let from = v["from"].as_i64().unwrap_or(0);
            serve_resume(state, peer, tx, their_log, from, v["catch_up"].as_bool().unwrap_or(false)).await;
        }
        "fed_event" | "fed_skip" | "fed_gap" => numbered(state, peer, tx, &v, kind).await,
        "fed_batch_end" => {
            let upto = v["upto"].as_i64().unwrap_or(0);
            let more = v["more"].as_bool().unwrap_or(false);
            let behind = {
                let Ok(mut ins) = state.fed.ins.lock() else { return };
                let pin = place(state, &mut ins, peer);
                pin.asked_at = None;
                pin.flow.asked = 0;
                pin.last < upto
            };
            if more || behind {
                tokio::time::sleep(BETWEEN_BATCHES).await;
                ask(state, peer, tx).await;
            }
        }
        "fed_bans" => bans_part(state, peer, &v),
        "profile_gossip" | "federation_welcome" => {
            if let Ok(msg) = serde_json::from_value::<RelayMessage>(v) {
                crate::relay::handlers::federation::handle_peer_message(state, peer, msg).await;
            }
        }
        other => tracing::debug!("Federation: {} sent a frame this version does not take un-numbered: {other}", key_in_log(peer)),
    }
}

/// A numbered frame: applied when it is the next number, ignored when it is a repeat, and a gap
/// asks for the catch-up.
async fn numbered(state: &Arc<RelayState>, peer: &str, tx: &UnboundedSender<String>, v: &Value, kind: Kind) {
    let log = v["log"].as_str().unwrap_or("").to_string();
    let ty = v["type"].as_str().unwrap_or("");
    enum Next {
        Apply(i64),
        Repeat,
        Gap,
    }
    let next = {
        let Ok(mut ins) = state.fed.ins.lock() else { return };
        let pin = place(state, &mut ins, peer);
        if pin.log != log {
            // Its outbox is new to this server (first contact, or its database was replaced).
            pin.log = log.clone();
            pin.last = 0;
        }
        if ty == "fed_gap" {
            let upto = v["upto"].as_i64().unwrap_or(0);
            if upto > pin.last {
                pin.flow.missed += (upto - pin.last) as u64;
                tracing::warn!("Federation: {} no longer had {} frames this server missed (older than its outbox keeps)", key_in_log(peer), upto - pin.last);
                pin.last = upto;
                let _ = state.db.set_fed_cursor(peer, &log, upto);
            }
            pin.flow.not_applied(kind);
            return;
        }
        let seq = v["seq"].as_i64().unwrap_or(0);
        if seq <= pin.last {
            pin.flow.not_applied(kind);
            Next::Repeat
        } else if seq > pin.last + 1 {
            pin.flow.not_applied(kind);
            let unanswered = pin.asked_at.is_some_and(|at| at.elapsed() < ASK_AGAIN_AFTER);
            if unanswered {
                Next::Repeat
            } else {
                Next::Gap
            }
        } else {
            // Taken now, before applying, so a second link from the same server cannot apply it too.
            pin.last = seq;
            Next::Apply(seq)
        }
    };
    match next {
        Next::Repeat => {}
        Next::Gap => ask(state, peer, tx).await,
        Next::Apply(seq) => {
            if ty == "fed_event" {
                apply(state, peer, &v["event"]).await;
            }
            let _ = state.db.set_fed_cursor(peer, &log, seq);
        }
    }
}

/// The key `peer` is pinned under when it is linked at tier 2 or above (what its signatures are
/// checked against), or None.
fn linked_key(state: &RelayState, peer: &str) -> Option<String> {
    state
        .db
        .list_federated_servers()
        .ok()?
        .into_iter()
        .find(|s| s.trust_tier >= 2 && s.public_key.as_deref() == Some(peer))
        .and_then(|s| s.public_key)
}

/// Whether `sig` is `peer`'s signature over `words` at `ts` (its pinned key, linked at tier 2).
fn signed_by(state: &RelayState, peer: &str, words: &str, ts: u64, sig: &str) -> bool {
    linked_key(state, peer).is_some_and(|key| crate::relay::handlers::broadcast::verify_dilithium_signature(&key, words, ts, sig))
}

/// What one numbered event from `peer` does here.
async fn apply(state: &Arc<RelayState>, peer: &str, event: &Value) {
    let s = |k: &str| event[k].as_str().unwrap_or("").to_string();
    let ts = event["timestamp"].as_u64().unwrap_or(0);
    let sig = s("signature");
    match event["type"].as_str().unwrap_or("") {
        "fed_edit" => {
            let (author, channel, new_content) = (s("author"), s("channel"), s("new_content"));
            let stamp = event["stamp"].as_u64().unwrap_or(0);
            if !signed_by(state, peer, &format!("fed_edit\n{author}\n{channel}\n{stamp}\n{new_content}"), ts, &sig) {
                tracing::warn!("Federation: refused an edit from {}: not signed by it", key_in_log(peer));
                return;
            }
            if let Ok(Some(ch)) = state.db.fed_message_edit(peer, &author, stamp, &new_content) {
                let _ = state.broadcast_tx.send(RelayMessage::Edit { from: peer.to_string(), timestamp: stamp, new_content, channel: ch });
            }
        }
        "fed_delete" => {
            let (author, channel) = (s("author"), s("channel"));
            let stamp = event["stamp"].as_u64().unwrap_or(0);
            if !signed_by(state, peer, &format!("fed_delete\n{author}\n{channel}\n{stamp}"), ts, &sig) {
                tracing::warn!("Federation: refused a delete from {}: not signed by it", key_in_log(peer));
                return;
            }
            if state.db.fed_message_delete(peer, &author, stamp).unwrap_or(false) {
                let _ = state.broadcast_tx.send(RelayMessage::Delete { from: peer.to_string(), timestamp: stamp });
            }
        }
        "fed_erase" => {
            let author = s("author");
            if !signed_by(state, peer, &format!("fed_erase\n{author}"), ts, &sig) {
                tracing::warn!("Federation: refused an erase from {}: not signed by it", key_in_log(peer));
                return;
            }
            for stamp in state.db.fed_messages_erase(peer, &author).unwrap_or_default() {
                let _ = state.broadcast_tx.send(RelayMessage::Delete { from: peer.to_string(), timestamp: stamp });
            }
        }
        "fed_mod" => {
            let (action, target, name) = (s("action"), s("target"), s("name"));
            if !signed_by(state, peer, &format!("fed_mod\n{action}\n{target}\n{name}"), ts, &sig) || target.is_empty() {
                tracing::warn!("Federation: refused a moderation notice from {}: not signed by it", key_in_log(peer));
                return;
            }
            match action.as_str() {
                "ban" => {
                    let _ = state.db.fed_ban_put(peer, &target, &name, ts as i64);
                    tracing::info!("Federation: {} banned {}, so that key cannot post in the rooms shared with it", key_in_log(peer), key_in_log(&target));
                }
                "unban" => {
                    let _ = state.db.fed_ban_remove(peer, &target);
                }
                _ => {}
            }
        }
        "federated_chat" | "profile_gossip" | "signed_object_gossip" => {
            if let Ok(msg) = serde_json::from_value::<RelayMessage>(event.clone()) {
                crate::relay::handlers::federation::handle_peer_message(state, peer, msg).await;
            }
        }
        other => tracing::debug!("Federation: {} sent an event this version does not know: {other}", key_in_log(peer)),
    }
}

/// One part of a linked server's whole ban list; the last part makes it the list kept here.
fn bans_part(state: &RelayState, peer: &str, v: &Value) {
    let part = v["part"].as_u64().unwrap_or(0);
    let last = v["last"].as_bool().unwrap_or(false);
    let ts = v["timestamp"].as_u64().unwrap_or(0);
    let bans: Vec<(String, String, i64)> = v["bans"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|b| (b["key"].as_str().unwrap_or("").to_string(), b["name"].as_str().unwrap_or("").to_string(), b["at"].as_i64().unwrap_or(0)))
                .filter(|(k, _, _)| !k.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let keys: Vec<&str> = bans.iter().map(|(k, _, _)| k.as_str()).collect();
    if !signed_by(state, peer, &format!("fed_bans\n{part}\n{last}\n{}", keys.join(",")), ts, v["signature"].as_str().unwrap_or("")) {
        tracing::warn!("Federation: refused a ban list from {}: not signed by it", key_in_log(peer));
        return;
    }
    let Ok(mut ins) = state.fed.ins.lock() else { return };
    let pin = place(state, &mut ins, peer);
    if part == 0 {
        pin.bans_incoming = Some(Vec::new());
    }
    let Some(held) = pin.bans_incoming.as_mut() else { return };
    held.extend(bans);
    if last {
        let list = pin.bans_incoming.take().unwrap_or_default();
        let keep: HashSet<&str> = list.iter().map(|(k, _, _)| k.as_str()).collect();
        for old in state.db.fed_bans_of(peer).unwrap_or_default() {
            if !keep.contains(old.public_key.as_str()) {
                let _ = state.db.fed_ban_remove(peer, &old.public_key);
            }
        }
        for (k, n, at) in &list {
            let _ = state.db.fed_ban_put(peer, k, n, *at);
        }
    }
}

// ── This server's own changes, sent to its links ─────────────────────────────────────────────

/// Whether `channel` is shared with linked servers.
fn shared(state: &RelayState, channel: &str) -> bool {
    let ch = if channel.is_empty() { "general" } else { channel };
    state.db.is_channel_federated(ch).unwrap_or(false)
}

/// Sign `words` at `ts` with this server's key and send `event` (with them) to the links.
async fn send_signed(state: &Arc<RelayState>, kind: &str, author: &str, stamp: u64, words: &str, mut event: Value) {
    let ts = now_ms();
    let Some(sig) = sign_with_server_key(&state.db, &format!("{words}\n{ts}")) else { return };
    event["timestamp"] = json!(ts);
    event["signature"] = json!(sig);
    publish(state, kind, author, stamp, "", &event).await;
}

/// Edit this server's own message (`author`'s, stamped `stamp`), and when it is in a shared room,
/// edit it on the linked servers too, and in the outbox, so a link that catches up later is sent
/// the words as they are now. The answer of `Storage::edit_message`.
pub async fn edit_own_message(state: &Arc<RelayState>, author: &str, stamp: u64, new_content: &str) -> Result<bool, rusqlite::Error> {
    let channel = state.db.message_channel(author, stamp)?;
    let edited = state.db.edit_message(author, stamp, new_content)?;
    if let Some(ch) = channel.filter(|ch| edited && shared(state, ch)) {
        resign_chat(state, author, stamp, new_content);
        let event = json!({ "type": "fed_edit", "author": author, "channel": ch, "stamp": stamp, "new_content": new_content });
        send_signed(state, "edit", author, stamp, &format!("fed_edit\n{author}\n{ch}\n{stamp}\n{new_content}"), event).await;
    }
    Ok(edited)
}

/// The outbox's chat frame for an edited message, with the new words and signed again.
fn resign_chat(state: &RelayState, author: &str, stamp: u64, new_content: &str) {
    let Ok(Some((seq, frame))) = state.db.fed_outbox_chat(author, stamp) else { return };
    let Ok(mut v) = serde_json::from_str::<Value>(&frame) else { return };
    let (name, ch) = (v["from_name"].as_str().unwrap_or("").to_string(), v["channel"].as_str().unwrap_or("").to_string());
    let Some(sig) = sign_with_server_key(&state.db, &crate::relay::handlers::federation::chat_words(author, &name, &ch, new_content, stamp)) else { return };
    v["content"] = json!(new_content);
    v["signature"] = json!(sig);
    let _ = state.db.fed_outbox_set_frame(seq, &v.to_string());
}

/// Delete this server's own message, and when it was in a shared room, delete it on the linked
/// servers too and scrub its words from the outbox. The answer of `Storage::delete_message`.
pub async fn delete_own_message(state: &Arc<RelayState>, author: &str, stamp: u64) -> Result<bool, rusqlite::Error> {
    let channel = state.db.message_channel(author, stamp)?;
    let deleted = state.db.delete_message(author, stamp)?;
    if deleted {
        after_delete(state, author, stamp, channel).await;
    }
    Ok(deleted)
}

/// `Storage::delete_message_by_id`, and the same for the linked servers.
pub async fn delete_message_by_id(state: &Arc<RelayState>, id: i64, requester: &str, is_admin: bool) -> Result<Option<(String, String)>, rusqlite::Error> {
    let found = state.db.message_by_id(id)?;
    let deleted = state.db.delete_message_by_id(id, requester, is_admin)?;
    if let (Some(_), Some((author, ch, stamp))) = (&deleted, found) {
        after_delete(state, &author, stamp, Some(ch)).await;
    }
    Ok(deleted)
}

async fn after_delete(state: &Arc<RelayState>, author: &str, stamp: u64, channel: Option<String>) {
    let _ = state.db.fed_outbox_scrub(author, Some(stamp));
    if let Some(ch) = channel.filter(|ch| shared(state, ch)) {
        let event = json!({ "type": "fed_delete", "author": author, "channel": ch, "stamp": stamp });
        send_signed(state, "delete", author, stamp, &format!("fed_delete\n{author}\n{ch}\n{stamp}"), event).await;
    }
}

/// Whether `key` posted in a shared room here (asked before an erase removes the rows).
pub fn posted_in_shared_rooms(state: &RelayState, key: &str) -> bool {
    state.db.posted_in_federated_rooms(key).unwrap_or(false)
}

/// `key`'s account here was erased: scrub its words from the outbox, and when it had posted in a
/// shared room, remove its messages from the linked servers' copies.
pub async fn after_erase(state: &Arc<RelayState>, key: &str, posted_shared: bool) {
    let _ = state.db.fed_outbox_scrub(key, None);
    if posted_shared {
        send_signed(state, "erase", "", 0, &format!("fed_erase\n{key}"), json!({ "type": "fed_erase", "author": key })).await;
    }
}

/// A ban here (`banned`) or its lifting: the linked servers keep it for the rooms they share.
pub async fn after_ban(state: &Arc<RelayState>, key: &str, name: &str, banned: bool) {
    if key.is_empty() || key.starts_with("bot_") {
        return;
    }
    let action = if banned { "ban" } else { "unban" };
    let event = json!({ "type": "fed_mod", "action": action, "target": key, "name": name });
    send_signed(state, "mod", "", 0, &format!("fed_mod\n{action}\n{key}\n{name}"), event).await;
}

/// Whether `key` may not post in `channel` because the room is shared with a linked server that
/// banned it; if so the person is told why, in words that say what is still open to them.
pub fn refused_in_shared_room(state: &RelayState, key: &str, channel: &str) -> bool {
    if key.starts_with("bot_") || !shared(state, channel) {
        return false;
    }
    let Ok(Some(server)) = state.db.fed_banned_by(key) else { return false };
    let _ = state.broadcast_tx.send(RelayMessage::Private {
        to: key.to_string(),
        message: format!(
            "This room is shared with {server}, which has banned you, so you cannot post in it. This server's own rooms are still open to you."
        ),
    });
    true
}

/// What the admin page shows for one linked server: how it is sending (flow), where this server
/// is in its log, how this server is sending to it, and the bans it told this server about.
pub fn peer_view(state: &RelayState, peer: Option<&str>) -> Value {
    let Some(peer) = peer else { return Value::Null };
    let now = Instant::now();
    let limits = limits(state);
    let (flow, cursor) = match state.fed.ins.lock() {
        Ok(mut ins) => {
            let pin = place(state, &mut ins, peer);
            (pin.flow.snapshot(&limits, now), json!({ "log": pin.log, "seq": pin.last }))
        }
        Err(_) => (Value::Null, Value::Null),
    };
    let sending = state.fed.outs.lock().ok().and_then(|outs| {
        outs.get(peer).map(|o| {
            let mode = match o.sending {
                Sending::Waiting => "waiting",
                Sending::Replaying => "catching_up",
                Sending::Streaming => "live",
            };
            json!({ "mode": mode, "queued": o.queued.load(Ordering::Relaxed) })
        })
    });
    let bans: Vec<Value> = state
        .db
        .fed_bans_of(peer)
        .unwrap_or_default()
        .into_iter()
        .take(100)
        .map(|b| json!({ "public_key": b.public_key, "name": b.name, "banned_at": b.banned_at }))
        .collect();
    json!({ "flow": flow, "cursor": cursor, "sending": sending, "bans": bans })
}

/// Pause `peer` for `how_long` as if it had flooded (tests).
pub fn pause_peer(state: &RelayState, peer: &str, how_long: Duration) {
    if let Ok(mut ins) = state.fed.ins.lock() {
        place(state, &mut ins, peer).flow.pause_for(Instant::now(), how_long);
    }
}

/// Tests only: put this server's place in `peer`'s log back to `to` (as a crash before the place
/// was stored would) and ask the peer for everything after it, over the live link.
#[cfg(test)]
pub(crate) async fn rewind_for_test(state: &Arc<RelayState>, peer: &str, to: i64) {
    let log = {
        let mut ins = state.fed.ins.lock().unwrap();
        let pin = place(state, &mut ins, peer);
        pin.last = to;
        pin.asked_at = None;
        pin.log.clone()
    };
    state.db.set_fed_cursor(peer, &log, to).unwrap();
    let tx = state.federation_connections.read().await.get(peer).map(|c| c.tx.clone()).expect("a live link");
    ask(state, peer, &tx).await;
}

#[cfg(test)]
#[path = "federation_log_tests.rs"]
mod tests;

//! Device-to-device big files, the connection (docs/design/media-showcase.md, "Device-to-device
//! big files: the shapes, exact", part 4): ONE WebRTC connection per pair of people at a time,
//! on its own thread, carrying one data channel per transfer, each labelled `file:<id>` (ordered,
//! reliable), going ONLY through the server's call forwarder (relay-only ICE), so neither person
//! learns the other's network address.
//!
//! THE SHAPE BOTH APPS KEEP (agreed with the web half, 2026-10-11). Signalling carries no transfer
//! id. The receiver of a transfer dials the sender: its `dc_offer` carries the file pass of the
//! transfer whose forwarder credentials it dialled with (`file_pass`, which the relay checks with
//! the sender as issuer and the receiver as grantee). The sender, dialled, does not know which of
//! its offers to that person the receiver chose, so it asks for credentials for each of its live
//! offers to them (at most four) and answers with one relayed address per credential: the
//! forwarder joins only the pair in the same scope, and ICE finds it. Each transfer then has its
//! own channel, opened by its receiver (whichever side dialled), and is told apart by the label.
//! If both people dial at once, the larger key (string compare of the Dilithium hex) wins and the
//! other drops its attempt (engine/file_transfer.rs).
//!
//! WHY ITS OWN THREAD AND SOCKETS, not the app's WebRTC manager (net/webrtc.rs): that manager
//! holds one call or voice room's allocation at a time, and a transfer's allocations are scoped to
//! the transfer. Each allocation has its own UDP socket, since a forwarder keeps one allocation per
//! client address. The TURN client is webrtc.rs's `mod turn`, as it is. The loop keeps str0m's one
//! rule (every mutation followed by a full drain of `poll_output`), as webrtc.rs explains.
//!
//! RELAY ONLY. The `Rtc` gets only relayed addresses as local candidates; a datagram that arrives at
//! a socket from anything but its forwarder is never read, and a transmit from anything but one of
//! the relayed addresses is dropped. The spec's direct connection between friends who both turned
//! direct connections on is not built: the app has no such setting yet
//! (`webrtc::DIRECT_CONNECTIONS` is false), so every transfer goes through the forwarder.
//!
//! ONE ENGINE PER CHANNEL (rule 2 of the first review, media-showcase.md "Device-to-device big
//! files: rules settled by the first review"). Each transfer is its own leg with its own
//! `Receiving` or `Sending`, on the dialling side and the answering side alike; one leg ending
//! (done, cancelled, failed) leaves the others as they are, and the connection closes when its
//! last channel ends (a sending leg the other side never opened a channel for, an offer not yet
//! accepted, does not keep it up). A finished transfer's channel is left open, not closed: the
//! website as first built took a channel's close for the whole connection's.
//!
//! EARLY CANDIDATES ARE KEPT (rule 3). The answering side makes its allocations only once its
//! credentials are in, and a dialler's candidates often come as `dc_ice` right after its offer (a
//! browser trickles them). The engine keeps them for that person while it waits and hands them
//! over in [`Setup::early_ice`]; the connection holds those, and any that come before its
//! allocations exist, until it has them, then installs a forwarder permission toward each and gives
//! it to ICE, as it does for the offer's own `a=candidate` lines. Without that permission the
//! forwarder drops what the other side sends, and the connection never opens.
//!
//! What is said on each channel is net/file_transfer.rs; this file only moves it.

use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use str0m::change::{SdpAnswer, SdpOffer, SdpPendingOffer};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event as RtcEvent, IceConnectionState, Input, Output, Rtc};

use super::file_offer::FilePass;
use super::file_transfer::{End, Event, Out, Receiving, Sending, Step, HIGH_WATER};
use super::webrtc::turn::{TurnClient, TurnInbound};

/// How long the forwarder may take to give this connection its relayed addresses.
pub const RELAY_WAIT: Duration = Duration::from_secs(25);
/// How long, once the forwarder answered, the first channel may take to open.
pub const OPEN_WAIT: Duration = Duration::from_secs(45);
/// After every transfer on the connection ended, how long the last messages get to leave.
const LINGER: Duration = Duration::from_secs(2);
/// The longest the loop blocks on a socket when nothing is going on.
const IDLE_WAIT: Duration = Duration::from_millis(20);
/// Messages written per loop turn at most, so commands and datagrams are never starved.
const WRITES_PER_TURN: usize = 64;
/// The most allocations an answer carries (the sender's live offers to one person).
pub const MAX_RELAYS: usize = 4;

/// The data channel's label for transfer `id`.
pub fn label(id: &str) -> String {
    format!("file:{id}")
}

/// The transfer a channel's label names, if it is one.
pub fn label_id(label: &str) -> Option<&str> {
    label.strip_prefix("file:").filter(|id| super::file_offer::is_id(id))
}

/// The forwarder's address and the credentials the server gave for one transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relay {
    pub server: String,
    pub username: String,
    pub credential: String,
}

/// Which side of one transfer this device is.
pub enum Side {
    /// The receiver: it opens the transfer's channel. The part file at `part` (made and sized by
    /// the engine) is filled in place; once whole and checked it is named `name` in `dir`.
    Receive { proto: Receiving, part: PathBuf, dir: PathBuf, name: String, sender: String },
    /// The sender: it serves the channel the receiver opens, reading the file at `path`.
    Send { proto: Sending, path: PathBuf },
}

/// One transfer on the connection.
pub struct Transfer {
    pub id: String,
    pub side: Side,
}

/// Everything a connection needs to start.
pub struct Setup {
    pub me: String,
    pub peer: String,
    /// Dialling: the pass of the transfer whose credentials it dials with (its `dc_offer` carries
    /// it). None: this side answers `their_offer`.
    pub dial_pass: Option<FilePass>,
    /// Answering: the other side's WebRTC offer (the `dc_offer`'s data, its JSON text).
    pub their_offer: Option<String>,
    /// One set of credentials when dialling; one per live offer (at most four) when answering.
    pub relays: Vec<Relay>,
    pub transfers: Vec<Transfer>,
    /// The address the sockets bind: all interfaces in the app, 127.0.0.1 in the tests.
    pub bind: String,
    /// Answering: the other side's `dc_ice` that arrived while this side waited for its
    /// credentials (each signal's data, its JSON text, in order). They are applied, with the
    /// offer's own candidates, once the allocations exist (rule 3 of the first review).
    pub early_ice: Vec<String>,
}

/// The most early candidates kept for one connection (a browser on relay-only ICE gives one or two
/// per forwarder credential, and an answer carries at most [`MAX_RELAYS`]).
pub const MAX_EARLY_ICE: usize = 32;

/// From the engine to the connection.
pub enum Cmd {
    /// A `webrtc_signal` from the other person: `dc_answer` or `dc_ice` (`data`, its JSON text).
    Signal { signal_type: String, data: String },
    /// Another transfer between the two (accepted, or offered, while the connection is up).
    Add(Transfer),
    Pause(String),
    Resume(String),
    Cancel(String),
}

/// From the connection to the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    /// A ready `webrtc_signal` frame to send on the chat socket.
    Signal(String),
    /// The forwarder gave this connection its relayed address(es).
    RelayReady,
    /// The forwarder could not be reached or refused every credential: the server is not set up.
    RelayUnavailable,
    /// The first channel opened.
    Open,
    /// This transfer's channel opened.
    TransferOpen(String),
    /// The other side opened a channel for a transfer this connection was not given.
    Wanted(String),
    /// Pieces in (receiver) or sent (sender).
    Progress(String, u64),
    PausedByThem(String, bool),
    /// The receiver's file is whole, checked and named: here.
    Saved(String, PathBuf),
    /// The transfer ended on this connection.
    Ended(String, End),
    /// The connection is gone (dropped, timed out, or closed after the last transfer ended).
    Closed,
}

/// The engine's handle on one pair's connection. Dropping it stops the thread.
pub struct FileLink {
    tx: Sender<Cmd>,
    rx: Receiver<LinkEvent>,
}

impl FileLink {
    /// Start the connection on its own thread.
    pub fn start(setup: Setup) -> FileLink {
        let (tx_cmd, rx_cmd) = mpsc::channel();
        let (tx_ev, rx_ev) = mpsc::channel();
        std::thread::spawn(move || {
            let mut allocs = Vec::new();
            for relay in setup.relays.iter().take(MAX_RELAYS) {
                match UdpSocket::bind(&setup.bind) {
                    Ok(udp) => allocs.push(Alloc::new(udp, relay.clone())),
                    Err(e) => log::warn!("file link: could not open a socket: {e}"),
                }
            }
            if allocs.is_empty() {
                let _ = tx_ev.send(LinkEvent::RelayUnavailable);
                let _ = tx_ev.send(LinkEvent::Closed);
                return;
            }
            Link::new(setup, allocs, rx_cmd, tx_ev).run()
        });
        FileLink { tx: tx_cmd, rx: rx_ev }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }

    /// Everything the connection reported since the last look.
    pub fn poll(&self) -> Vec<LinkEvent> {
        self.rx.try_iter().collect()
    }
}

/// The addresses of the `a=candidate:` lines of an SDP, for their forwarder permissions.
pub fn sdp_candidate_addrs(sdp: &str) -> Vec<SocketAddr> {
    sdp.lines()
        .filter_map(|l| l.trim().strip_prefix("a="))
        .filter(|l| l.starts_with("candidate:"))
        .filter_map(|l| Candidate::from_sdp_string(l).ok())
        .map(|c| c.addr())
        .collect()
}

/// The inner text of a signal's `data` (a JSON string by contract; an object is written out).
pub fn signal_text(data: &Value) -> String {
    match data {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// One allocation on the forwarder, on its own socket.
struct Alloc {
    udp: UdpSocket,
    local: SocketAddr,
    relay: Relay,
    turn: Option<TurnClient>,
    resolving: Option<Receiver<Option<SocketAddr>>>,
    relayed: Option<SocketAddr>,
    failed: bool,
}

impl Alloc {
    fn new(udp: UdpSocket, relay: Relay) -> Alloc {
        let local = udp.local_addr().unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
        let mut a = Alloc { udp, local, relay, turn: None, resolving: None, relayed: None, failed: false };
        // An address needs no lookup; a name is looked up on a helper thread.
        let server = a.relay.server.clone();
        if let Ok(addr) = server.parse::<SocketAddr>() {
            a.turn = Some(TurnClient::new(addr, a.relay.username.clone(), a.relay.credential.clone()));
        } else {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(server.to_socket_addrs().ok().and_then(|mut it| it.find(|a| a.is_ipv4())));
            });
            a.resolving = Some(rx);
        }
        a
    }

    /// Settled: allocated, or given up.
    fn settled(&self) -> bool {
        self.relayed.is_some() || self.failed
    }

    /// Drive the lookup and the TURN client. Returns the relayed address the first time it comes.
    fn drive(&mut self) -> Option<SocketAddr> {
        if let Some(rx) = &self.resolving {
            match rx.try_recv() {
                Ok(Some(addr)) => {
                    self.resolving = None;
                    self.turn = Some(TurnClient::new(addr, self.relay.username.clone(), self.relay.credential.clone()));
                }
                Ok(None) | Err(TryRecvError::Disconnected) => {
                    self.resolving = None;
                    self.failed = true;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        let turn = self.turn.as_mut()?;
        let newly = turn.drive(&self.udp);
        if let Some(f) = turn.failure() {
            if !self.failed {
                log::info!("file link: the forwarder: {f:?}");
            }
            self.failed = true;
            self.relayed = None;
            return None;
        }
        if newly.is_some() {
            self.relayed = newly;
        }
        newly
    }
}

/// The open file one transfer reads or writes.
enum Disk {
    None,
    File(std::fs::File),
}

/// One transfer on the connection.
struct Leg {
    side: Side,
    disk: Disk,
    cid: Option<ChannelId>,
    outbox: VecDeque<Out>,
    /// The receiver's whole-file check, on a helper thread.
    finishing: Option<Receiver<Result<PathBuf, String>>>,
    /// The sender's piece hashes, worked out again from the file on a helper thread.
    hashing: Option<Receiver<std::io::Result<Vec<[u8; 32]>>>>,
    ended: bool,
    /// A channel was bound to it at some point.
    had_channel: bool,
}

impl Leg {
    /// Whether this leg is one of the connection's channels: a receiving leg opens one; a sending
    /// leg is one once the receiver opened it (an offer to the other person that they have not
    /// accepted, or not opened on this connection, is not).
    fn is_channel(&self) -> bool {
        matches!(self.side, Side::Receive { .. }) || self.had_channel
    }
}

/// RULE 2 OF THE FIRST REVIEW: the connection closes when its last channel ends. True when there
/// is at least one channel and every one of them has ended; a sending leg nobody opened does not
/// keep the connection up.
fn last_channel_ended<'a>(legs: impl Iterator<Item = &'a Leg> + Clone) -> bool {
    let mut channels = legs.filter(|l| l.is_channel()).peekable();
    channels.peek().is_some() && channels.all(|l| l.ended)
}

struct Link {
    me: String,
    peer: String,
    dial_pass: Option<FilePass>,
    their_offer: Option<String>,
    allocs: Vec<Alloc>,
    /// The allocation data last came through: the one the loop waits on.
    active: usize,
    rx: Receiver<Cmd>,
    tx: Sender<LinkEvent>,
    started: Instant,
    ready_at: Option<Instant>,
    rtc: Option<Rtc>,
    pending: Option<SdpPendingOffer>,
    legs: HashMap<String, Leg>,
    by_cid: HashMap<ChannelId, String>,
    /// Channels the other side opened for transfers not (yet) given to this connection.
    unbound: HashMap<String, ChannelId>,
    /// Receivers' channels to open once the connection can take them.
    to_open: Vec<String>,
    any_open: bool,
    early_ice: Vec<String>,
    deadline: Option<Instant>,
    all_ended_at: Option<Instant>,
    dropped: bool,
}

impl Link {
    fn new(setup: Setup, allocs: Vec<Alloc>, rx: Receiver<Cmd>, tx: Sender<LinkEvent>) -> Link {
        let mut link = Link {
            me: setup.me,
            peer: setup.peer,
            dial_pass: setup.dial_pass,
            their_offer: setup.their_offer,
            allocs,
            active: 0,
            rx,
            tx,
            started: Instant::now(),
            ready_at: None,
            rtc: None,
            pending: None,
            legs: HashMap::new(),
            by_cid: HashMap::new(),
            unbound: HashMap::new(),
            to_open: Vec::new(),
            any_open: false,
            early_ice: Vec::new(),
            deadline: None,
            all_ended_at: None,
            dropped: false,
        };
        for t in setup.transfers {
            link.add(t);
        }
        // Candidates the engine kept while it waited for the credentials: held until the
        // allocations exist (the Rtc does not yet), then applied with the offer's own.
        for data in setup.early_ice {
            link.command(Cmd::Signal { signal_type: "dc_ice".to_string(), data });
        }
        link
    }

    fn emit(&self, ev: LinkEvent) {
        let _ = self.tx.send(ev);
    }

    fn run(mut self) {
        str0m::crypto::from_feature_flags().install_process_default();
        let mut buf = vec![0u8; 2048];
        loop {
            // 1. Commands, each one that touches the Rtc followed by a full drain.
            loop {
                match self.rx.try_recv() {
                    Ok(cmd) => {
                        self.command(cmd);
                        self.drain();
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return self.close(),
                }
            }
            // 2. The forwarder (TURN datagrams only; no Rtc is touched until it is ready).
            if !self.drive_relays() {
                return self.close();
            }
            // 3. Everything waiting on the sockets, each datagram followed by a drain.
            let mut busy = false;
            for i in 0..self.allocs.len() {
                let _ = self.allocs[i].udp.set_nonblocking(true);
                for _ in 0..512 {
                    match self.allocs[i].udp.recv_from(&mut buf) {
                        Ok((n, src)) => {
                            let data = buf[..n].to_vec();
                            self.datagram(i, src, &data);
                            busy = true;
                        }
                        Err(_) => break,
                    }
                }
            }
            // 4. The transfers: each protocol's turn, then what may be written.
            self.work();
            busy |= self.write_some();
            // 5. The Rtc's own clock.
            let now = Instant::now();
            if self.deadline.is_some_and(|d| now >= d) {
                if let Some(rtc) = self.rtc.as_mut() {
                    if rtc.handle_input(Input::Timeout(now)).is_err() {
                        rtc.disconnect();
                    }
                }
                self.drain();
            }
            // 6. Dropped, the last channel ended (rule 2: an offer nobody opened a channel for
            //    does not keep the connection up), or never opened.
            if self.dropped {
                return self.close();
            }
            if last_channel_ended(self.legs.values()) {
                let t = *self.all_ended_at.get_or_insert_with(Instant::now);
                let flushed = self.legs.values().all(|l| l.outbox.is_empty()) && self.buffered_total() == 0;
                if t.elapsed() >= LINGER || flushed {
                    return self.close();
                }
            } else {
                self.all_ended_at = None;
            }
            if !self.any_open && self.ready_at.is_some_and(|t| t.elapsed() >= OPEN_WAIT) {
                log::info!("file link: no channel opened in time");
                return self.close();
            }
            // 7. Nothing moved: wait on the socket data last came through, a little (or until the
            //    Rtc's next deadline).
            if !busy {
                let wait = self
                    .deadline
                    .map(|d| d.saturating_duration_since(Instant::now()))
                    .unwrap_or(IDLE_WAIT)
                    .clamp(Duration::from_millis(1), IDLE_WAIT);
                let i = self.active.min(self.allocs.len() - 1);
                let _ = self.allocs[i].udp.set_nonblocking(false);
                let _ = self.allocs[i].udp.set_read_timeout(Some(wait));
                if let Ok((n, src)) = self.allocs[i].udp.recv_from(&mut buf) {
                    let data = buf[..n].to_vec();
                    self.datagram(i, src, &data);
                }
            }
        }
    }

    /// Take a transfer onto the connection: its file opened, and (the receiver) its channel opened
    /// as soon as the connection can take it; a channel the other side already opened for it is
    /// bound at once.
    fn add(&mut self, t: Transfer) {
        if self.legs.contains_key(&t.id) {
            return;
        }
        let opened = match &t.side {
            Side::Receive { part, .. } => std::fs::OpenOptions::new().read(true).write(true).open(part),
            Side::Send { path, .. } => std::fs::File::open(path),
        };
        let disk = match opened {
            Ok(f) => Disk::File(f),
            Err(e) => {
                let words = match &t.side {
                    Side::Receive { name, .. } => format!("{name} could not be written on this device ({e})."),
                    Side::Send { proto, .. } => format!("{} could not be read on this device any more ({e}).", proto.offer.name),
                };
                self.emit(LinkEvent::Ended(t.id.clone(), End::Failed(words)));
                return;
            }
        };
        let receive = matches!(t.side, Side::Receive { .. });
        self.legs.insert(t.id.clone(), Leg { side: t.side, disk, cid: None, outbox: VecDeque::new(), finishing: None, hashing: None, ended: false, had_channel: false });
        if let Some(cid) = self.unbound.remove(&t.id) {
            self.bind(&t.id, cid);
        } else if receive {
            self.to_open.push(t.id.clone());
            self.open_channels();
        }
    }

    /// Open the receivers' channels waiting to be opened, when the connection can take them: once
    /// it exists and is not waiting for the answer to its offer (str0m opens a channel on a live
    /// connection without a new offer).
    fn open_channels(&mut self) {
        if self.pending.is_some() || self.to_open.is_empty() {
            return;
        }
        let Some(rtc) = self.rtc.as_mut() else { return };
        let ids = std::mem::take(&mut self.to_open);
        let mut api = rtc.sdp_api();
        for id in &ids {
            let _ = api.add_channel(label(id));
        }
        if api.apply().is_some() {
            log::info!("file link: a channel asked for a new offer; not expected on a live connection");
        }
        self.drain();
    }

    /// A channel is open for transfer `id`: its protocol begins.
    fn bind(&mut self, id: &str, cid: ChannelId) {
        let Some(leg) = self.legs.get_mut(id) else { return };
        if leg.cid.is_some() {
            return; // both sides opened one; the first serves
        }
        leg.cid = Some(cid);
        leg.had_channel = true;
        self.by_cid.insert(cid, id.to_string());
        let step = match &mut leg.side {
            Side::Receive { proto, .. } => proto.open(Instant::now()),
            Side::Send { proto, .. } => Step { out: proto.open(), events: Vec::new() },
        };
        self.emit(LinkEvent::TransferOpen(id.to_string()));
        self.apply(id, step);
    }

    /// Drive every allocation; once they are settled, make the connection. False when none can be
    /// had (the engine is told the server is not set up).
    fn drive_relays(&mut self) -> bool {
        for a in &mut self.allocs {
            a.drive();
        }
        if self.ready_at.is_none() {
            let settled = self.allocs.iter().all(Alloc::settled);
            let relayed: Vec<SocketAddr> = self.allocs.iter().filter_map(|a| a.relayed).collect();
            if (settled && relayed.is_empty()) || self.started.elapsed() >= RELAY_WAIT && relayed.is_empty() {
                self.emit(LinkEvent::RelayUnavailable);
                return false;
            }
            // The dialler has one allocation; the answerer waits until every credential settled
            // (the matching one is among them) or the wait ran out with at least one.
            if settled || self.started.elapsed() >= RELAY_WAIT {
                self.ready_at = Some(Instant::now());
                self.emit(LinkEvent::RelayReady);
                self.connect(&relayed);
                self.drain();
                for line in std::mem::take(&mut self.early_ice) {
                    self.add_candidate(&line);
                    self.drain();
                }
            }
        } else if self.allocs.iter().all(|a| a.failed) {
            return false; // every allocation was lost
        }
        true
    }

    /// The relayed addresses are ours: the dialler makes its WebRTC offer with its receivers'
    /// channels in it; the answerer answers, then opens its own receivers' channels. Only relayed
    /// addresses are local candidates, so the SDP names the server's addresses and nothing of
    /// this device's.
    fn connect(&mut self, relayed: &[SocketAddr]) {
        let mut rtc = Rtc::builder().build(Instant::now());
        for (i, &r) in relayed.iter().enumerate() {
            let base = self.allocs.iter().find(|a| a.relayed == Some(r)).map(|a| a.local).unwrap_or_else(|| self.allocs[i.min(self.allocs.len() - 1)].local);
            match Candidate::relayed(r, base, Protocol::Udp) {
                Ok(c) => {
                    rtc.add_local_candidate(c);
                }
                Err(e) => log::warn!("file link: bad relayed candidate {r}: {e}"),
            }
        }
        match self.their_offer.take() {
            None => {
                let ids = std::mem::take(&mut self.to_open);
                let mut api = rtc.sdp_api();
                for id in &ids {
                    let _ = api.add_channel(label(id));
                }
                let Some((offer, pending)) = api.apply() else {
                    self.dropped = true;
                    return;
                };
                self.pending = Some(pending);
                self.rtc = Some(rtc);
                if let Ok(v) = serde_json::to_value(&offer) {
                    self.emit_signal("dc_offer", v);
                }
            }
            Some(offer) => {
                let offered = serde_json::from_str::<Value>(&offer).ok().and_then(|v| v.get("sdp").and_then(Value::as_str).map(sdp_candidate_addrs)).unwrap_or_default();
                let parsed: SdpOffer = match serde_json::from_str(&offer) {
                    Ok(o) => o,
                    Err(e) => {
                        log::info!("file link: the other side's offer did not read: {e}");
                        self.dropped = true;
                        return;
                    }
                };
                self.permit(&offered);
                let answer = match rtc.sdp_api().accept_offer(parsed) {
                    Ok(a) => a,
                    Err(e) => {
                        log::info!("file link: the other side's offer was not taken: {e}");
                        self.dropped = true;
                        return;
                    }
                };
                self.rtc = Some(rtc);
                if let Ok(v) = serde_json::to_value(&answer) {
                    self.emit_signal("dc_answer", v);
                }
                self.drain();
                self.open_channels();
            }
        }
    }

    /// Install forwarder permissions toward the other side's candidates, on every allocation (the
    /// forwarder grants them only where the scopes match).
    fn permit(&mut self, addrs: &[SocketAddr]) {
        for a in &mut self.allocs {
            if let Some(turn) = a.turn.as_mut() {
                for &addr in addrs {
                    turn.ensure_peer(&a.udp, addr);
                }
            }
        }
    }

    /// A `webrtc_signal` frame for the other side; the dialler's offer carries its file pass.
    fn emit_signal(&self, signal_type: &str, payload: Value) {
        let mut frame = json!({
            "type": "webrtc_signal",
            "to": self.peer,
            "from": self.me,
            "signal_type": signal_type,
            "data": payload.to_string(),
        });
        if let (Some(pass), "dc_offer") = (&self.dial_pass, signal_type) {
            frame["file_pass"] = serde_json::to_value(pass).unwrap_or(Value::Null);
        }
        self.emit(LinkEvent::Signal(frame.to_string()));
    }

    fn add_candidate(&mut self, line: &str) {
        let Ok(c) = Candidate::from_sdp_string(line) else { return };
        let addr = c.addr();
        self.permit(&[addr]);
        if let Some(rtc) = self.rtc.as_mut() {
            rtc.add_remote_candidate(c);
        }
    }

    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Signal { signal_type, data } => match signal_type.as_str() {
                "dc_answer" => {
                    let Ok(v) = serde_json::from_str::<Value>(&data) else { return };
                    let answered = v.get("sdp").and_then(Value::as_str).map(sdp_candidate_addrs).unwrap_or_default();
                    self.permit(&answered);
                    if self.rtc.is_none() || self.pending.is_none() {
                        return; // not dialling, or answered already
                    }
                    let (Some(rtc), Some(pending)) = (self.rtc.as_mut(), self.pending.take()) else { return };
                    match serde_json::from_value::<SdpAnswer>(v) {
                        Ok(answer) => {
                            if let Err(e) = rtc.sdp_api().accept_answer(pending, answer) {
                                log::info!("file link: the answer was not taken: {e}");
                                rtc.disconnect();
                            }
                        }
                        Err(e) => log::info!("file link: the answer did not read: {e}"),
                    }
                    self.drain();
                    self.open_channels();
                }
                "dc_ice" => {
                    let line = match serde_json::from_str::<Value>(&data) {
                        Ok(Value::Object(m)) => m.get("candidate").and_then(Value::as_str).unwrap_or("").to_string(),
                        Ok(Value::String(s)) => s,
                        _ => return,
                    };
                    if line.is_empty() {
                        return; // the end-of-candidates marker
                    }
                    if self.rtc.is_none() {
                        if self.early_ice.len() < MAX_EARLY_ICE {
                            self.early_ice.push(line);
                        }
                        return;
                    }
                    self.add_candidate(&line);
                }
                _ => {}
            },
            Cmd::Add(t) => self.add(t),
            Cmd::Pause(id) => {
                if let Some(leg) = self.legs.get_mut(&id) {
                    let out = match &mut leg.side {
                        Side::Receive { proto, .. } => proto.pause(),
                        Side::Send { proto, .. } => proto.pause(),
                    };
                    leg.outbox.extend(out);
                }
            }
            Cmd::Resume(id) => {
                if let Some(leg) = self.legs.get_mut(&id) {
                    let out = match &mut leg.side {
                        Side::Receive { proto, .. } => proto.resume(Instant::now()),
                        Side::Send { proto, .. } => proto.resume(),
                    };
                    leg.outbox.extend(out);
                }
            }
            Cmd::Cancel(id) => {
                let step = match self.legs.get_mut(&id).map(|l| &mut l.side) {
                    Some(Side::Receive { proto, .. }) => proto.cancel(),
                    Some(Side::Send { proto, .. }) => proto.cancel(),
                    None => return,
                };
                self.apply(&id, step);
            }
        }
    }

    /// A datagram on allocation `i`'s socket: only its forwarder's are read. Relayed data goes to the
    /// Rtc as if it came from the other side's relayed address to this allocation's.
    fn datagram(&mut self, i: usize, src: SocketAddr, data: &[u8]) {
        let a = &mut self.allocs[i];
        let Some(turn) = a.turn.as_mut() else { return };
        if src != turn.server_addr() {
            return; // relay only: nothing straight from anyone else
        }
        let relayed = turn.relayed_addr();
        match turn.handle_from_server(&a.udp, data) {
            TurnInbound::Control => {}
            TurnInbound::Data { peer, inner_offset, inner_len } => {
                let (Some(rtc), Some(dest)) = (self.rtc.as_mut(), relayed) else { return };
                let Ok(contents) = data[inner_offset..inner_offset + inner_len].try_into() else { return };
                let input = Input::Receive(Instant::now(), Receive { proto: Protocol::Udp, source: peer, destination: dest, contents });
                if rtc.handle_input(input).is_err() {
                    rtc.disconnect();
                }
                self.active = i;
                self.drain();
            }
        }
    }

    /// Drain the Rtc to its next timeout: datagrams out through the allocation whose relayed
    /// address they come from (anything else is dropped, never sent straight), events after.
    fn drain(&mut self) {
        let mut events = Vec::new();
        {
            let Link { rtc, allocs, deadline, dropped, .. } = self;
            let Some(rtc) = rtc.as_mut() else { return };
            loop {
                if !rtc.is_alive() {
                    break;
                }
                match rtc.poll_output() {
                    Ok(Output::Timeout(t)) => {
                        *deadline = Some(t);
                        break;
                    }
                    Ok(Output::Transmit(t)) => {
                        if let Some(a) = allocs.iter_mut().find(|a| a.relayed == Some(t.source)) {
                            if let Some(turn) = a.turn.as_mut() {
                                turn.send_relayed(&a.udp, t.destination, &t.contents);
                            }
                        }
                    }
                    Ok(Output::Event(e)) => events.push(e),
                    Err(e) => {
                        log::info!("file link: poll_output: {e}");
                        rtc.disconnect();
                        break;
                    }
                }
            }
            if !rtc.is_alive() {
                *dropped = true;
            }
        }
        for e in events {
            self.rtc_event(e);
        }
    }

    fn rtc_event(&mut self, e: RtcEvent) {
        match e {
            RtcEvent::IceConnectionStateChange(IceConnectionState::Disconnected) => self.dropped = true,
            RtcEvent::ChannelOpen(cid, lbl) => {
                let Some(id) = label_id(&lbl).map(str::to_string) else { return };
                if !self.any_open {
                    self.any_open = true;
                    self.emit(LinkEvent::Open);
                }
                if self.legs.contains_key(&id) {
                    self.bind(&id, cid);
                } else {
                    self.unbound.insert(id.clone(), cid);
                    self.emit(LinkEvent::Wanted(id));
                }
            }
            RtcEvent::ChannelData(d) => {
                let Some(id) = self.by_cid.get(&d.id).cloned() else { return };
                let Some(leg) = self.legs.get_mut(&id) else { return };
                let now = Instant::now();
                let step = match (&mut leg.side, &mut leg.disk) {
                    (Side::Receive { proto, .. }, Disk::File(f)) if d.binary => proto.on_binary(&d.data, f, now),
                    (Side::Receive { proto, .. }, _) if !d.binary => proto.on_text(&String::from_utf8_lossy(&d.data), now),
                    (Side::Send { proto, .. }, _) if !d.binary => proto.on_text(&String::from_utf8_lossy(&d.data)),
                    _ => Step::default(),
                };
                self.apply(&id, step);
            }
            RtcEvent::ChannelClose(cid) => {
                // One transfer's channel closed: that transfer is off this connection (it is dialled
                // again later if it did not end); the others go on.
                if let Some(id) = self.by_cid.remove(&cid) {
                    if let Some(leg) = self.legs.get_mut(&id) {
                        leg.cid = None;
                        match &mut leg.side {
                            Side::Receive { proto, .. } => proto.closed(),
                            Side::Send { proto, .. } => proto.closed(),
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Take one transfer's protocol step: its messages to its outbox, its events to the engine.
    fn apply(&mut self, id: &str, step: Step) {
        if let Some(leg) = self.legs.get_mut(id) {
            leg.outbox.extend(step.out);
        }
        for ev in step.events {
            match ev {
                Event::Progress(n) => self.emit(LinkEvent::Progress(id.to_string(), n)),
                Event::PausedByThem(b) => self.emit(LinkEvent::PausedByThem(id.to_string(), b)),
                Event::Complete => self.start_finish(id),
                Event::Ended(end) => {
                    if let Some(leg) = self.legs.get_mut(id) {
                        leg.ended = true;
                        leg.disk = Disk::None;
                    }
                    self.emit(LinkEvent::Ended(id.to_string(), end));
                }
            }
        }
    }

    /// Every piece of `id` is in: close its part file and check the whole of it on a helper thread.
    fn start_finish(&mut self, id: &str) {
        let Some(leg) = self.legs.get_mut(id) else { return };
        let Side::Receive { proto, part, dir, name, sender } = &leg.side else { return };
        if let Disk::File(f) = &leg.disk {
            let _ = f.sync_data();
        }
        let (part, dir, name, sender, whole) = (part.clone(), dir.clone(), name.clone(), sender.clone(), proto.offer.blake3.clone());
        leg.disk = Disk::None; // closed before it is named
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(super::file_transfer::finish_part(&part, &dir, &name, &whole, &sender));
        });
        leg.finishing = Some(rx);
    }

    /// Each transfer's own turn: the receiver's checks (a part file from before a restart, a stall,
    /// the whole-file check's answer), the sender's list of hashes when it had to be worked out
    /// again.
    fn work(&mut self) {
        let now = Instant::now();
        let ids: Vec<String> = self.legs.keys().cloned().collect();
        for id in ids {
            let mut steps = Vec::new();
            let mut saved = None;
            let Some(leg) = self.legs.get_mut(&id) else { continue };
            if leg.ended {
                continue;
            }
            match (&mut leg.side, &mut leg.disk) {
                (Side::Receive { proto, .. }, disk) => {
                    if let (Disk::File(f), true) = (disk, leg.cid.is_some()) {
                        steps.push(proto.tick(f, now));
                    }
                    if let Some(rx) = &leg.finishing {
                        match rx.try_recv() {
                            Ok(result) => {
                                leg.finishing = None;
                                saved = result.as_ref().ok().cloned();
                                steps.push(proto.finished(result.map(|_| ())));
                            }
                            Err(TryRecvError::Disconnected) => {
                                leg.finishing = None;
                                steps.push(proto.finished(Err("The file could not be checked.".to_string())));
                            }
                            Err(TryRecvError::Empty) => {}
                        }
                    }
                }
                (Side::Send { proto, path }, _) => {
                    if !proto.has_hashes() && leg.hashing.is_none() {
                        let (tx, rx) = mpsc::channel();
                        let path = path.clone();
                        std::thread::spawn(move || {
                            let (done, stop) = (AtomicU64::new(0), AtomicBool::new(false));
                            let _ = tx.send(super::file_offer::hash_file(&path, &done, &stop).map(|h| h.pieces));
                        });
                        leg.hashing = Some(rx);
                    }
                    if let Some(rx) = &leg.hashing {
                        match rx.try_recv() {
                            Ok(Ok(list)) => {
                                leg.hashing = None;
                                steps.push(proto.set_hashes(Arc::new(list)));
                            }
                            Ok(Err(e)) => {
                                leg.hashing = None;
                                let words = format!("{} could not be read on this device any more ({e}).", proto.offer.name);
                                let mut s = proto.cancel();
                                s.events = vec![Event::Ended(End::Failed(words))];
                                steps.push(s);
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
            if let Some(path) = saved {
                self.emit(LinkEvent::Saved(id.clone(), path));
            }
            for s in steps {
                self.apply(&id, s);
            }
        }
    }

    /// The connection's buffered amount, every channel together (str0m caps it at 128 KiB).
    fn buffered_total(&mut self) -> usize {
        let cids: Vec<ChannelId> = self.legs.values().filter_map(|l| l.cid).collect();
        let Some(rtc) = self.rtc.as_mut() else { return 0 };
        cids.into_iter().map(|cid| rtc.channel(cid).map(|mut c| c.buffered_amount()).unwrap_or(0)).sum()
    }

    /// Write what may go now, one message per drain, taking the transfers in turn: each one's outbox
    /// first, then (a sender) its next piece's messages while the buffer is under its mark. True
    /// when anything was written.
    fn write_some(&mut self) -> bool {
        let mut ids: Vec<String> = self.legs.iter().filter(|(_, l)| l.cid.is_some()).map(|(k, _)| k.clone()).collect();
        ids.sort();
        let mut wrote = false;
        let mut writes = 0;
        let mut stuck = std::collections::HashSet::new();
        while writes < WRITES_PER_TURN && stuck.len() < ids.len() {
            let mut progressed = false;
            for id in &ids {
                if writes >= WRITES_PER_TURN || stuck.contains(id) {
                    continue;
                }
                match self.write_one(id) {
                    Some(true) => {
                        wrote = true;
                        progressed = true;
                        writes += 1;
                    }
                    Some(false) | None => {
                        stuck.insert(id.clone());
                    }
                }
            }
            if !progressed {
                break;
            }
        }
        wrote
    }

    /// Write one message of transfer `id`. Some(true) written; Some(false) the buffer is full; None
    /// nothing to write.
    fn write_one(&mut self, id: &str) -> Option<bool> {
        let buffered = self.buffered_total();
        let leg = self.legs.get_mut(id)?;
        let cid = leg.cid?;
        if leg.outbox.is_empty() {
            if buffered >= HIGH_WATER {
                return Some(false);
            }
            let next = match (&mut leg.side, &mut leg.disk) {
                (Side::Send { proto, .. }, Disk::File(f)) => proto.next(f, buffered),
                _ => Ok(None),
            };
            match next {
                Ok(Some((m, ev))) => {
                    leg.outbox.push_back(m);
                    if let Some(Event::Progress(n)) = ev {
                        self.emit(LinkEvent::Progress(id.to_string(), n));
                    }
                }
                Ok(None) => return None,
                Err(words) => {
                    let mut s = match &mut leg.side {
                        Side::Send { proto, .. } => proto.cancel(),
                        Side::Receive { proto, .. } => proto.cancel(),
                    };
                    s.events = vec![Event::Ended(End::Failed(words))];
                    self.apply(id, s);
                }
            }
        }
        let leg = self.legs.get_mut(id)?;
        let m = leg.outbox.front()?;
        let (binary, bytes): (bool, &[u8]) = match m {
            Out::Text(t) => (false, t.as_bytes()),
            Out::Binary(b) => (true, b.as_slice()),
        };
        let rtc = self.rtc.as_mut()?;
        let accepted = match rtc.channel(cid) {
            Some(mut ch) => ch.write(binary, bytes),
            None => return None,
        };
        match accepted {
            Ok(true) => {
                if let Some(leg) = self.legs.get_mut(id) {
                    leg.outbox.pop_front();
                }
                self.drain();
                Some(true)
            }
            Ok(false) => Some(false), // the send buffer is full: the next turn tries again
            Err(e) => {
                log::info!("file link: write: {e}");
                self.dropped = true;
                None
            }
        }
    }

    /// Let the forwarder's allocations go and say the connection is gone.
    fn close(mut self) {
        for a in self.allocs.drain(..) {
            if let Some(turn) = a.turn {
                turn.release(&a.udp);
            }
        }
        self.emit(LinkEvent::Closed);
    }
}

#[cfg(test)]
#[path = "file_link_tests.rs"]
mod tests;

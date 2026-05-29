//! Native WebRTC DataChannel P2P transport (increment 1).
//!
//! This is the native sibling of the web client's `chat-p2p.js`. It opens an
//! ordered WebRTC DataChannel to another peer, using the relay ONLY for ICE
//! signaling (offer / answer / candidates). Once the channel is open, frames
//! travel peer-to-peer and never touch the relay.
//!
//! # Why str0m (sans-IO)
//!
//! The desktop app has NO async runtime on its hot path — the GUI is egui's
//! immediate-mode loop and the WebSocket client (`ws_client.rs`) is a blocking
//! `tungstenite` socket on a plain `std::thread`. A normal WebRTC crate wants
//! tokio. `str0m` is *sans-IO*: the `Rtc` value never touches the network or a
//! clock itself. WE drive it from one `std::thread` that owns a blocking
//! `UdpSocket`, feeding it `Input` (incoming datagrams + timeouts) and pumping
//! its `Output` (datagrams to send + events + the next wake deadline). This is
//! the exact same blocking-thread + `mpsc` model as `ws_client.rs`.
//!
//! # The str0m run-loop contract (the #1 footgun)
//!
//! str0m has ONE strict rule: **every mutation of an `Rtc` must be followed by
//! a complete drain of `poll_output()` until it returns `Output::Timeout`,
//! before the next mutation of that same `Rtc`.** A "mutation" is anything
//! taking `&mut Rtc`: `handle_input`, `Channel::write`, `sdp_api().apply()`,
//! `add_remote_candidate`, etc. The canonical shape is:
//!
//! ```text
//! loop {
//!   // 1. drain poll_output -> handle Transmit/Event, record the next Timeout
//!   // 2. wait for ONE of: that timeout firing, a UDP packet, or app input
//!   // 3. feed exactly ONE Input (Receive on packet, Timeout otherwise)
//!   // 4. goto 1
//! }
//! ```
//!
//! We MUST honor the `Instant` str0m hands back as the read timeout — using a
//! fixed sleep instead starves SCTP/ICE retransmits and the channel silently
//! stalls. Our loop additionally polls two in-process queues each iteration
//! (inbound signaling from the GUI, outbound app text to send) and clamps its
//! UDP read timeout so those queues stay responsive even when str0m's next
//! deadline is far away.
//!
//! # Offerer rule (glare avoidance) — mirrors the web
//!
//! Two peers must not both send an offer (that "glare" deadlocks negotiation).
//! Mirroring `web/chat/chat-groups-p2p.js::ensureGroupMesh` and
//! `chat-p2p.js`, only the side with the lexicographically **larger** pubkey
//! hex offers (`my_key > peer_key`); the smaller side waits for the offer and
//! answers. `offer_to()` enforces this so a mis-call can't cause glare.
//!
//! # Signaling contract (must match the relay + web — interop depends on it)
//!
//! Outbound JSON we emit (the GUI relays these to `ws_client.send`):
//! ```json
//! {"type":"webrtc_signal","to":"<peer hex>","from":"<my hex>",
//!  "signal_type":"dc_offer"|"dc_answer"|"dc_ice","data":<JSON STRING>}
//! ```
//! `data` is a JSON **string** (`serde_json::to_string` of the SDP/candidate),
//! exactly like the web client's `JSON.stringify(offer)`. The relay overwrites
//! `from` with the authenticated key, so its value isn't trusted, but we still
//! include it. Inbound `webrtc_signal` (already routed to us by the GUI) has
//! `from` (sender hex), `signal_type`, and `data` (a JSON string we parse back).
//!
//! str0m's `SdpOffer`/`SdpAnswer` serialize as `{type, sdp}` — byte-identical
//! to a browser `RTCSessionDescription`, so the SDP interop is automatic. ICE
//! candidates: the browser sends an `RTCIceCandidate` object whose `.candidate`
//! field is the `candidate:...` SDP line; we parse that line with
//! `Candidate::from_sdp_string`, and emit our own host candidate the same way.
//!
//! # Increment scope
//!
//! - inc-1 (this file): one ordered DataChannel per peer, text round-trip, host
//!   ICE candidates only (same-LAN / same-host testing).
//! - inc-2 (later): group mesh (open channels to every roster member).
//! - inc-3 (later): STUN/TURN for cross-NAT — see the `// TODO inc-3` markers.

#![cfg(feature = "native")]

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use str0m::change::{SdpAnswer, SdpOffer, SdpPendingOffer};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

/// How long the UDP read may block per loop iteration, at most. We clamp
/// str0m's requested timeout to this so the in-process signaling / send queues
/// stay responsive (otherwise, if str0m's next deadline were seconds away, a
/// freshly-enqueued offer or outbound frame would sit unprocessed that long).
const MAX_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The label for our single data channel. Matches nothing load-bearing on the
/// web side (the browser names its channel `'dm'`); str0m uses the label only
/// for the DCEP handshake. We keep a stable, descriptive name.
const CHANNEL_LABEL: &str = "hum-data";

/// An event surfaced from the WebRTC thread up to the GUI. The GUI drains these
/// via `WebrtcHandle::poll_events()` each frame and turns them into debug lines
/// / chat messages.
#[derive(Debug, Clone)]
pub enum WebrtcEvent {
    /// The DataChannel to `peer` is now open and writable.
    ChannelOpen { peer: String },
    /// A text frame arrived from `peer` over its DataChannel.
    Frame { peer: String, text: String },
    /// The connection / channel to `peer` closed (ICE disconnect or SCTP close).
    Closed { peer: String },
}

/// A command sent from the GUI down into the WebRTC thread. Internal — the
/// public `WebrtcHandle` methods construct these.
enum Command {
    /// An inbound `webrtc_signal` the GUI received and forwarded to us.
    Signal {
        from: String,
        signal_type: String,
        /// The signal payload. By contract this is a JSON *string* (the web
        /// `JSON.stringify`'d the SDP/candidate); we parse it back inside.
        data: Value,
    },
    /// Application request: start a connection to `peer` (offerer side).
    OfferTo { peer: String },
    /// Application request: send `text` to `peer` over its open channel.
    SendText { peer: String, text: String },
}

/// Handle the GUI holds to talk to the WebRTC thread. All methods are
/// non-blocking (they just push onto channels); the thread does the work.
///
/// # Outbound-signaling design choice
///
/// We picked the simplest of the two options in the spec: the manager pushes
/// **ready-to-send `webrtc_signal` JSON strings** into an internal queue, and
/// the GUI drains them with [`poll_outbound`](Self::poll_outbound) and relays
/// each to `ws_client.send`. This keeps the WebRTC thread free of any WS-client
/// dependency (it never needs a clone of the WS sender, and stays testable in
/// isolation), and matches how `poll_events` already works.
pub struct WebrtcHandle {
    /// Commands down to the thread.
    tx_cmd: Sender<Command>,
    /// Events up from the thread (channel open, frames, closed).
    rx_event: Receiver<WebrtcEvent>,
    /// Ready-to-send `webrtc_signal` JSON the GUI must relay to the WS client.
    rx_outbound: Receiver<String>,
    /// Our own Dilithium pubkey hex (used for the offerer-rule comparison so the
    /// GUI can pre-check, and for diagnostics).
    my_pubkey_hex: String,
}

impl WebrtcHandle {
    /// Feed an inbound `webrtc_signal` (from the relay, via the GUI) into the
    /// manager. `from` is the sender's pubkey hex, `signal_type` is one of
    /// `dc_offer` / `dc_answer` / `dc_ice`, and `data` is the JSON value the
    /// relay forwarded (a JSON string per the contract).
    pub fn submit_signal(&self, from: String, signal_type: String, data: Value) {
        let _ = self.tx_cmd.send(Command::Signal { from, signal_type, data });
    }

    /// Begin opening a DataChannel to `peer` (by Dilithium pubkey hex).
    ///
    /// Honors the offerer rule: this only actually sends an offer if our key is
    /// lexicographically larger than `peer`. If our key is smaller, the call is
    /// a no-op on the wire — we wait for the peer's offer and answer it. (The
    /// thread enforces this too, so a mistaken caller can't cause glare.)
    pub fn offer_to(&self, peer: String) {
        let _ = self.tx_cmd.send(Command::OfferTo { peer });
    }

    /// Send a UTF-8 text frame to `peer` over its DataChannel. If the channel
    /// isn't open yet the frame is dropped (inc-1 has no send queue; the dev
    /// trigger only sends *after* it sees `ChannelOpen`). A send-queue is a
    /// later-increment nicety, matching the web's `p2pSendQueue`.
    pub fn send_text(&self, peer: String, text: String) {
        let _ = self.tx_cmd.send(Command::SendText { peer, text });
    }

    /// Non-blocking drain of all events the thread has produced since last call.
    pub fn poll_events(&self) -> Vec<WebrtcEvent> {
        let mut out = Vec::new();
        loop {
            match self.rx_event.try_recv() {
                Ok(ev) => out.push(ev),
                Err(_) => break, // Empty or Disconnected — either way, stop.
            }
        }
        out
    }

    /// Non-blocking drain of all outbound `webrtc_signal` JSON strings the
    /// thread wants sent. The GUI relays each to `ws_client.send(&s)`.
    pub fn poll_outbound(&self) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            match self.rx_outbound.try_recv() {
                Ok(s) => out.push(s),
                Err(_) => break,
            }
        }
        out
    }

    /// Our own pubkey hex (handy for the GUI to apply the offerer rule too).
    pub fn my_pubkey_hex(&self) -> &str {
        &self.my_pubkey_hex
    }
}

/// Owns all WebRTC state and runs the event loop on its own thread.
///
/// Constructed only via [`WebrtcManager::start`], which spawns the thread and
/// returns a [`WebrtcHandle`]; the manager value itself lives on the thread.
pub struct WebrtcManager {
    /// Our Dilithium pubkey hex — the identity peers know us by, and the value
    /// we compare against a peer's hex for the offerer rule.
    my_pubkey_hex: String,
    /// The one shared UDP socket. Its local address is our host ICE candidate.
    /// All peers' WebRTC traffic is multiplexed over this single socket and
    /// demuxed by source address / `Rtc::accepts`.
    udp: UdpSocket,
    /// The local socket address we advertise as a host candidate.
    local_addr: SocketAddr,
    /// One `Rtc` per peer, keyed by the peer's Dilithium pubkey hex.
    peers: HashMap<String, PeerConn>,
    /// Inbound commands from the GUI.
    rx_cmd: Receiver<Command>,
    /// Events up to the GUI.
    tx_event: Sender<WebrtcEvent>,
    /// Ready-to-send `webrtc_signal` JSON up to the GUI (it relays to the WS).
    tx_outbound: Sender<String>,
}

/// Per-peer connection state.
struct PeerConn {
    /// The sans-IO WebRTC engine for this peer.
    rtc: Rtc,
    /// If we're the offerer, the pending offer we must match the answer against.
    /// `accept_answer` consumes it. `None` once answered, or if we're the answerer.
    pending: Option<SdpPendingOffer>,
    /// The channel id, learned from `Event::ChannelOpen`. `None` until open.
    /// (The id returned by `add_channel` is NOT yet writable — `rtc.channel()`
    /// returns `None` for it until the open event fires; see str0m docs.)
    channel: Option<ChannelId>,
    /// The remote UDP address, learned from the first datagram str0m accepts.
    /// Used as a fast-path route hint for inbound packets.
    remote_addr: Option<SocketAddr>,
    /// Whether we've already surfaced `ChannelOpen` to the GUI (dedupe).
    announced_open: bool,
}

impl WebrtcManager {
    /// Spawn the WebRTC thread and return a handle the GUI uses to drive it.
    ///
    /// `my_pubkey_hex` is our Dilithium3 identity hex (the same value sent in
    /// the WS `identify` and compared for the offerer rule).
    pub fn start(my_pubkey_hex: String) -> WebrtcHandle {
        let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
        let (tx_event, rx_event) = mpsc::channel::<WebrtcEvent>();
        let (tx_outbound, rx_outbound) = mpsc::channel::<String>();

        let handle = WebrtcHandle {
            tx_cmd,
            rx_event,
            rx_outbound,
            my_pubkey_hex: my_pubkey_hex.clone(),
        };

        thread::spawn(move || {
            // Bind the shared UDP socket on an ephemeral port, all interfaces.
            // This socket's address is our host ICE candidate. 0.0.0.0:0 lets
            // the OS pick the port; we read it back for the candidate.
            let udp = match UdpSocket::bind("0.0.0.0:0") {
                Ok(s) => s,
                Err(e) => {
                    log::error!("WebRTC: failed to bind UDP socket: {e}");
                    crate::debug::push_debug(format!("WebRTC UDP bind FAILED: {e}"));
                    return;
                }
            };
            let local_addr = match udp.local_addr() {
                Ok(a) => a,
                Err(e) => {
                    log::error!("WebRTC: failed to read local addr: {e}");
                    crate::debug::push_debug(format!("WebRTC local_addr FAILED: {e}"));
                    return;
                }
            };

            log::info!("WebRTC: bound UDP {local_addr}, identity {}", &my_pubkey_hex);
            crate::debug::push_debug(format!("WebRTC bound UDP {local_addr}"));

            let mgr = WebrtcManager {
                my_pubkey_hex,
                udp,
                local_addr,
                peers: HashMap::new(),
                rx_cmd,
                tx_event,
                tx_outbound,
            };
            mgr.run();
        });

        handle
    }

    /// The thread main loop. See the module-level docs for the str0m contract.
    fn run(mut self) {
        // str0m needs a process-wide crypto provider installed once before any
        // DTLS handshake. `install_process_default` is backed by a OnceLock, so
        // calling it here (and idempotently on any future thread) is safe —
        // only the first install wins. We use the pure-Rust backend selected in
        // Cargo.toml (`rust-crypto`); `from_feature_flags` resolves to it.
        str0m::crypto::from_feature_flags().install_process_default();

        // Reusable receive buffer. WebRTC datagrams are well under 2 KB.
        let mut buf = vec![0u8; 2000];

        loop {
            // ── A. Apply AT MOST ONE GUI command per iteration.
            //
            //    This is deliberate and load-bearing for str0m's single-mutation
            //    invariant: "every mutation of an Rtc must be followed by a full
            //    poll_output drain before the next mutation of THAT SAME Rtc."
            //    A command can mutate an Rtc (add_remote_candidate, accept_answer,
            //    Channel::write, or an applied offer on a new Rtc). If we drained
            //    the whole command queue here, two commands targeting the same
            //    peer (e.g. two dc_ice in a row) would be two back-to-back
            //    mutations with NO drain between them — exactly what the invariant
            //    forbids. By taking one command, then letting step B drain every
            //    peer to Timeout, the drain always sits between consecutive
            //    command-mutations. Iterations are sub-MAX_POLL_INTERVAL, so even
            //    a burst of commands clears in a few ms.
            // `handled_cmd` tells step C to use a minimal read timeout so we
            // loop back quickly and drain the rest of a command burst (an offer
            // is usually followed immediately by several dc_ice candidates).
            let mut handled_cmd = false;
            match self.rx_cmd.try_recv() {
                Ok(cmd) => {
                    self.handle_command(cmd);
                    handled_cmd = true;
                }
                Err(TryRecvError::Empty) => { /* nothing queued — fall through */ }
                Err(TryRecvError::Disconnected) => {
                    // GUI dropped the handle — shut the thread down.
                    log::info!("WebRTC: command channel closed, stopping thread");
                    return;
                }
            }

            // ── B. Drain poll_output for EVERY peer until each returns Timeout.
            //    Collect the soonest deadline across all peers; that's how long
            //    we're allowed to block on the UDP read. Dead peers (ICE failed
            //    / SCTP closed) are reaped here.
            let now = Instant::now();
            let mut soonest = now + MAX_POLL_INTERVAL;
            let mut dead: Vec<String> = Vec::new();

            // We iterate over a snapshot of keys so we can mutate self.peers
            // (e.g. set remote_addr, announce open) and emit events freely.
            let keys: Vec<String> = self.peers.keys().cloned().collect();
            for key in &keys {
                match self.poll_peer(key) {
                    PollResult::Timeout(t) => soonest = soonest.min(t),
                    PollResult::Dead => dead.push(key.clone()),
                }
            }
            for key in dead {
                self.peers.remove(&key);
                let _ = self.tx_event.send(WebrtcEvent::Closed { peer: key.clone() });
                crate::debug::push_debug(format!("WebRTC peer closed: {}", short(&key)));
            }

            // ── C. Wait for ONE of: the soonest timeout, or an incoming packet.
            //    Clamp to MAX_POLL_INTERVAL so the command queue stays snappy,
            //    and to >= 1ms because set_read_timeout(0) is illegal. If we
            //    just handled a command, use a minimal wait so the next
            //    iteration promptly drains the rest of the burst.
            let now = Instant::now();
            let wait = if handled_cmd {
                Duration::from_millis(1)
            } else if soonest > now {
                (soonest - now).min(MAX_POLL_INTERVAL).max(Duration::from_millis(1))
            } else {
                Duration::from_millis(1)
            };
            if let Err(e) = self.udp.set_read_timeout(Some(wait)) {
                log::warn!("WebRTC: set_read_timeout failed: {e}");
            }

            // ── D. Feed exactly ONE Input: a Receive on a packet, else a
            //    Timeout to every peer to advance their clocks.
            buf.resize(2000, 0);
            match self.udp.recv_from(&mut buf) {
                Ok((n, source)) => {
                    // Route the datagram to the peer whose Rtc accepts it.
                    // str0m's `accepts` inspects the parsed datagram (STUN
                    // ufrag, DTLS/SRTP association) to decide ownership; it's
                    // the canonical demux. We build the borrowed Input inside
                    // this scope so the &buf borrow ends before the next loop.
                    let slice = &buf[..n];
                    let contents = match slice.try_into() {
                        Ok(c) => c,
                        Err(_) => {
                            // Unparseable datagram (not STUN/DTLS/RTP) — ignore.
                            continue;
                        }
                    };
                    let input = Input::Receive(
                        Instant::now(),
                        Receive {
                            proto: Protocol::Udp,
                            source,
                            destination: self.local_addr,
                            contents,
                        },
                    );

                    // Find the owning peer. We look first at remote_addr (fast
                    // path once learned), then fall back to `accepts`.
                    let owner: Option<String> = {
                        let mut found = None;
                        // Fast path: a peer whose learned remote_addr matches.
                        for (k, p) in self.peers.iter() {
                            if p.remote_addr == Some(source) {
                                found = Some(k.clone());
                                break;
                            }
                        }
                        if found.is_none() {
                            // Slow path: ask each Rtc if it accepts this input.
                            for (k, p) in self.peers.iter() {
                                if p.rtc.accepts(&input) {
                                    found = Some(k.clone());
                                    break;
                                }
                            }
                        }
                        found
                    };

                    if let Some(key) = owner {
                        if let Some(p) = self.peers.get_mut(&key) {
                            // Learn / refresh the remote address for the fast path.
                            p.remote_addr = Some(source);
                            if let Err(e) = p.rtc.handle_input(input) {
                                log::warn!("WebRTC: handle_input(Receive) error for {}: {e}", short(&key));
                                p.rtc.disconnect();
                            }
                        }
                    } else {
                        // Common during connection setup: a STUN binding may
                        // arrive before we've created the answering Rtc, or
                        // from an unrelated source. Drop quietly.
                        log::trace!("WebRTC: no peer accepts datagram from {source}");
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    // Read timed out — advance every peer's clock. WouldBlock is
                    // the unix timeout error, TimedOut is the windows one.
                    let now = Instant::now();
                    for p in self.peers.values_mut() {
                        if let Err(e) = p.rtc.handle_input(Input::Timeout(now)) {
                            log::warn!("WebRTC: handle_input(Timeout) error: {e}");
                            p.rtc.disconnect();
                        }
                    }
                }
                Err(e) => {
                    // A real socket error. Log and advance clocks so we don't
                    // wedge; if the socket is truly broken we'll notice via the
                    // command channel closing eventually.
                    log::warn!("WebRTC: recv_from error: {e}");
                    let now = Instant::now();
                    for p in self.peers.values_mut() {
                        let _ = p.rtc.handle_input(Input::Timeout(now));
                    }
                }
            }
            // ── E. goto top (back to step A/B drain).
        }
    }

    /// Drain one peer's `poll_output` to `Timeout`, handling Transmit (send on
    /// the socket) and Event (map to `WebrtcEvent`). Returns the next deadline,
    /// or `Dead` if the peer's Rtc is no longer alive.
    ///
    /// This is the per-peer half of the str0m drain. Mutations issued from
    /// inside this drain (e.g. `Channel::write` while handling an event) are
    /// allowed by the contract — the surrounding loop keeps polling afterward.
    fn poll_peer(&mut self, key: &str) -> PollResult {
        loop {
            // Re-borrow each iteration; we may have emitted events / sent on the
            // socket in between. Bail if the peer vanished or its Rtc died.
            let alive = self.peers.get(key).map(|p| p.rtc.is_alive()).unwrap_or(false);
            if !alive {
                return PollResult::Dead;
            }

            // poll_output borrows the Rtc mutably; scope it tightly so we can
            // then mutate self.peers (announce open, etc.) without overlap.
            let output = {
                let p = match self.peers.get_mut(key) {
                    Some(p) => p,
                    None => return PollResult::Dead,
                };
                match p.rtc.poll_output() {
                    Ok(o) => o,
                    Err(e) => {
                        log::warn!("WebRTC: poll_output error for {}: {e}", short(key));
                        p.rtc.disconnect();
                        return PollResult::Dead;
                    }
                }
            };

            match output {
                Output::Timeout(t) => return PollResult::Timeout(t),
                Output::Transmit(t) => {
                    // Send the datagram. str0m tells us the destination (ICE may
                    // change it over the session). A failed send isn't fatal —
                    // log and keep draining.
                    if let Err(e) = self.udp.send_to(&t.contents, t.destination) {
                        log::trace!("WebRTC: udp send_to {} failed: {e}", t.destination);
                    }
                }
                Output::Event(ev) => self.handle_event(key, ev),
            }
        }
    }

    /// Map a str0m `Event` to our `WebrtcEvent` and/or internal state change.
    fn handle_event(&mut self, key: &str, ev: Event) {
        match ev {
            Event::IceConnectionStateChange(state) => {
                use str0m::IceConnectionState as S;
                log::debug!("WebRTC: ICE state for {} -> {:?}", short(key), state);
                if state == S::Disconnected {
                    // Treat as terminal for inc-1 (no ICE restart). Disconnect
                    // the Rtc so the next poll reaps it and emits Closed.
                    if let Some(p) = self.peers.get_mut(key) {
                        p.rtc.disconnect();
                    }
                }
            }
            Event::ChannelOpen(cid, label) => {
                // The channel is now writable. Record its id and surface the
                // open event to the GUI exactly once.
                if let Some(p) = self.peers.get_mut(key) {
                    p.channel = Some(cid);
                    if !p.announced_open {
                        p.announced_open = true;
                        let _ = self.tx_event.send(WebrtcEvent::ChannelOpen { peer: key.to_string() });
                    }
                }
                log::info!("WebRTC: channel open with {} (label '{label}')", short(key));
                crate::debug::push_debug(format!("WebRTC channel OPEN with {}", short(key)));
            }
            Event::ChannelData(data) => {
                // Inbound frame. We only deal in text for inc-1. `data` has
                // `{id, binary, data: Vec<u8>}`. If a peer sent binary we still
                // try to interpret as UTF-8 (lossless for our text frames).
                let text = String::from_utf8_lossy(&data.data).into_owned();
                let _ = self.tx_event.send(WebrtcEvent::Frame { peer: key.to_string(), text });
            }
            Event::ChannelClose(_cid) => {
                // SCTP-level channel close. Disconnect so the peer is reaped and
                // a single Closed event is emitted by the reaper.
                if let Some(p) = self.peers.get_mut(key) {
                    p.rtc.disconnect();
                }
                log::info!("WebRTC: channel closed by {}", short(key));
            }
            _ => {
                // Media / stats events — not used by the data-only transport.
            }
        }
    }

    /// Apply a single GUI command (offer / answer-inbound-signal / send).
    fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::OfferTo { peer } => self.cmd_offer_to(peer),
            Command::SendText { peer, text } => self.cmd_send_text(peer, text),
            Command::Signal { from, signal_type, data } => {
                self.cmd_signal(from, signal_type, data)
            }
        }
    }

    /// Begin an outgoing connection to `peer` (offerer side), honoring the
    /// glare-avoidance offerer rule.
    fn cmd_offer_to(&mut self, peer: String) {
        if peer == self.my_pubkey_hex {
            return; // never connect to ourselves
        }
        // Offerer rule: only the LARGER pubkey hex offers. If we're the smaller
        // side, do nothing — we'll answer the peer's offer when it arrives.
        if !(self.my_pubkey_hex > peer) {
            log::debug!("WebRTC: offer_to({}) skipped — we are the answerer (smaller key)", short(&peer));
            return;
        }
        if let Some(p) = self.peers.get(&peer) {
            // Already have a connection in progress / open — don't re-offer.
            if p.rtc.is_alive() {
                return;
            }
        }

        // Build a fresh Rtc, add our host candidate, create the data channel,
        // and produce the offer. Per the single-mutation invariant, the loop's
        // step-B drain right after this handle_command batch will pump the
        // resulting Transmits — we don't need to drain inline here.
        let mut rtc = Rtc::builder().build(Instant::now());

        // Our host ICE candidate is the shared UDP socket's address.
        // TODO inc-3: also add STUN/TURN-derived candidates for cross-NAT.
        match Candidate::host(self.local_addr, "udp") {
            Ok(cand) => {
                rtc.add_local_candidate(cand);
            }
            Err(e) => {
                log::error!("WebRTC: bad host candidate {}: {e}", self.local_addr);
                return;
            }
        }

        // Create the ordered data channel. `add_channel` returns a ChannelId,
        // but that id is NOT writable yet — we must wait for Event::ChannelOpen
        // (str0m opens it after SCTP/DTLS come up). We store the real id then.
        let mut api = rtc.sdp_api();
        let _cid = api.add_channel(CHANNEL_LABEL.to_string());
        let (offer, pending) = match api.apply() {
            Some(pair) => pair,
            None => {
                // apply() returns None only if there were no changes — but we
                // just added a channel, so this shouldn't happen. Guard anyway.
                log::error!("WebRTC: sdp_api().apply() produced no offer");
                return;
            }
        };

        // Serialize the offer to a JSON string (`{type,sdp}`) and wrap it in the
        // webrtc_signal envelope. `data` is the JSON *string*, matching the web.
        let offer_json = match serde_json::to_string(&offer) {
            Ok(s) => s,
            Err(e) => {
                log::error!("WebRTC: failed to serialize offer: {e}");
                return;
            }
        };
        self.emit_signal(&peer, "dc_offer", offer_json);

        self.peers.insert(
            peer.clone(),
            PeerConn {
                rtc,
                pending: Some(pending),
                channel: None,
                remote_addr: None,
                announced_open: false,
            },
        );
        log::info!("WebRTC: sent dc_offer to {}", short(&peer));
        crate::debug::push_debug(format!("WebRTC offer -> {}", short(&peer)));
    }

    /// Send `text` to `peer` over its open DataChannel. Drops silently if the
    /// channel isn't open yet (inc-1 has no outbound queue).
    fn cmd_send_text(&mut self, peer: String, text: String) {
        let p = match self.peers.get_mut(&peer) {
            Some(p) => p,
            None => {
                log::debug!("WebRTC: send_text to unknown peer {}", short(&peer));
                return;
            }
        };
        let cid = match p.channel {
            Some(c) => c,
            None => {
                log::debug!("WebRTC: send_text dropped — channel to {} not open yet", short(&peer));
                return;
            }
        };
        // `rtc.channel(cid)` yields a writable handle once open. `write(false,
        // bytes)` sends as text. This is a mutation; the step-B drain right
        // after the command batch flushes the resulting Transmits.
        match p.rtc.channel(cid) {
            Some(mut ch) => match ch.write(false, text.as_bytes()) {
                Ok(true) => {
                    log::debug!("WebRTC: sent {} bytes to {}", text.len(), short(&peer));
                }
                Ok(false) => {
                    // Buffer full / not ready — inc-1 just drops. A real send
                    // queue + ChannelBufferedAmountLow handling comes later.
                    log::debug!("WebRTC: channel to {} not ready, frame dropped", short(&peer));
                }
                Err(e) => log::warn!("WebRTC: channel write to {} failed: {e}", short(&peer)),
            },
            None => log::debug!("WebRTC: channel handle for {} unavailable", short(&peer)),
        }
    }

    /// Handle an inbound `webrtc_signal` of one of: dc_offer / dc_answer / dc_ice.
    fn cmd_signal(&mut self, from: String, signal_type: String, data: Value) {
        // By contract `data` is a JSON STRING. Pull the inner string out; if the
        // relay ever forwarded a raw object instead, fall back to re-serializing.
        let inner: String = match &data {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        match signal_type.as_str() {
            "dc_offer" => self.on_offer(from, &inner),
            "dc_answer" => self.on_answer(from, &inner),
            "dc_ice" => self.on_ice(from, &inner),
            other => log::debug!("WebRTC: ignoring unknown signal_type '{other}' from {}", short(&from)),
        }
    }

    /// We received an offer — we are the answerer. Build an Rtc, accept the
    /// offer, and send back our answer.
    fn on_offer(&mut self, from: String, sdp_json: &str) {
        if from == self.my_pubkey_hex {
            return;
        }
        let offer: SdpOffer = match serde_json::from_str(sdp_json) {
            Ok(o) => o,
            Err(e) => {
                log::warn!("WebRTC: bad dc_offer from {}: {e}", short(&from));
                return;
            }
        };

        let mut rtc = Rtc::builder().build(Instant::now());
        match Candidate::host(self.local_addr, "udp") {
            Ok(cand) => {
                rtc.add_local_candidate(cand);
            }
            Err(e) => {
                log::error!("WebRTC: bad host candidate {}: {e}", self.local_addr);
                return;
            }
        }

        // accept_offer consumes the sdp_api and yields the answer to send back.
        let answer = match rtc.sdp_api().accept_offer(offer) {
            Ok(a) => a,
            Err(e) => {
                log::warn!("WebRTC: accept_offer from {} failed: {e}", short(&from));
                return;
            }
        };
        let answer_json = match serde_json::to_string(&answer) {
            Ok(s) => s,
            Err(e) => {
                log::error!("WebRTC: failed to serialize answer: {e}");
                return;
            }
        };

        // Insert the peer BEFORE emitting the signal (order doesn't strictly
        // matter, but this keeps state consistent if emit ever did work).
        self.peers.insert(
            from.clone(),
            PeerConn {
                rtc,
                pending: None, // answerer has no pending offer
                channel: None,
                remote_addr: None,
                announced_open: false,
            },
        );
        self.emit_signal(&from, "dc_answer", answer_json);
        log::info!("WebRTC: accepted offer from {}, sent answer", short(&from));
        crate::debug::push_debug(format!("WebRTC answer -> {}", short(&from)));
    }

    /// We received an answer to an offer we sent earlier (we're the offerer).
    fn on_answer(&mut self, from: String, sdp_json: &str) {
        let answer: SdpAnswer = match serde_json::from_str(sdp_json) {
            Ok(a) => a,
            Err(e) => {
                log::warn!("WebRTC: bad dc_answer from {}: {e}", short(&from));
                return;
            }
        };
        let p = match self.peers.get_mut(&from) {
            Some(p) => p,
            None => {
                log::debug!("WebRTC: dc_answer from {} but no pending connection", short(&from));
                return;
            }
        };
        let pending = match p.pending.take() {
            Some(pending) => pending,
            None => {
                log::debug!("WebRTC: dc_answer from {} but we had no pending offer", short(&from));
                return;
            }
        };
        // accept_answer finalizes the offerer side. The step-B drain afterward
        // begins the ICE/DTLS handshake transmits.
        if let Err(e) = p.rtc.sdp_api().accept_answer(pending, answer) {
            log::warn!("WebRTC: accept_answer from {} failed: {e}", short(&from));
            p.rtc.disconnect();
            return;
        }
        log::info!("WebRTC: applied answer from {}", short(&from));
    }

    /// We received a remote ICE candidate. Parse the SDP `candidate:` line and
    /// add it to the matching peer's Rtc.
    fn on_ice(&mut self, from: String, candidate_json: &str) {
        // The browser sends an RTCIceCandidate object: {candidate, sdpMid,
        // sdpMLineIndex, ...}. We need the `candidate` field (the SDP line).
        // str0m peers send the same shape (we serialize Candidate -> ? ) — to
        // be robust, accept either a bare SDP string or that object.
        let sdp_line: String = match serde_json::from_str::<Value>(candidate_json) {
            Ok(Value::Object(map)) => match map.get("candidate").and_then(|c| c.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => {
                    // An empty candidate is the browser's end-of-candidates
                    // marker — nothing to add.
                    log::trace!("WebRTC: empty/end-of ICE candidate from {}", short(&from));
                    return;
                }
            },
            Ok(Value::String(s)) => s,
            _ => candidate_json.to_string(),
        };

        let cand = match Candidate::from_sdp_string(&sdp_line) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("WebRTC: unparseable ICE candidate from {}: {e}", short(&from));
                return;
            }
        };
        match self.peers.get_mut(&from) {
            Some(p) => {
                p.rtc.add_remote_candidate(cand);
                log::trace!("WebRTC: added remote ICE candidate from {}", short(&from));
            }
            None => {
                // Candidate arrived before the offer/answer created the peer.
                // For inc-1 (host candidates, same LAN) we drop it; the host
                // candidate exchange in the SDP usually suffices. A pre-peer
                // candidate buffer is a later-increment robustness nicety.
                log::debug!("WebRTC: dc_ice from {} before peer exists, dropped", short(&from));
            }
        }
    }

    /// Build a `webrtc_signal` envelope and push it onto the outbound queue for
    /// the GUI to relay to the WS client. `payload` is the JSON STRING for the
    /// `data` field (per the relay/web contract).
    fn emit_signal(&self, to: &str, signal_type: &str, payload: String) {
        let msg = serde_json::json!({
            "type": "webrtc_signal",
            "to": to,
            // The relay overwrites `from` with the authenticated key, so this
            // value isn't trusted — but we include it to match the web client.
            "from": self.my_pubkey_hex,
            "signal_type": signal_type,
            // `data` is a JSON string (the stringified SDP/candidate).
            "data": payload,
        });
        let _ = self.tx_outbound.send(msg.to_string());
    }
}

/// Result of draining one peer's poll_output.
enum PollResult {
    /// The next deadline str0m wants us to wake this peer at.
    Timeout(Instant),
    /// The peer's Rtc is no longer alive and should be reaped.
    Dead,
}

/// Short, log-friendly form of a long pubkey hex (first 12 chars + ellipsis).
fn short(key: &str) -> String {
    if key.len() > 12 {
        format!("{}…", &key[..12])
    } else {
        key.to_string()
    }
}

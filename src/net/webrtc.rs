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
//! - inc-3a (THIS increment): STUN server-reflexive (srflx) candidate gathering
//!   so two peers behind *different* NATs can connect (the host candidate alone
//!   only works same-LAN). We hand-roll a tiny RFC 5389 STUN Binding client over
//!   the manager's existing shared `UdpSocket`, learn our public `ip:port`
//!   (server-reflexive address) from the Binding Response's XOR-MAPPED-ADDRESS,
//!   add it as a local candidate to every peer's `Rtc`, and *trickle* it to the
//!   far side as a `dc_ice` signal. See the `// STUN srflx` / `// inc-3a`
//!   markers and the `mod stun` block at the bottom of the file.
//! - inc-3b (later): TURN relay (RFC 5766) for the symmetric-NAT fallback —
//!   still the only remaining `// TODO inc-3b` path. NOT in this file yet.
//!
//! # Why str0m does NOT trickle our srflx for us (the inc-3a footgun)
//!
//! str0m is sans-IO and has *no* built-in candidate discovery: its own docs say
//! "This library has no built-in discovery of local network addresses on the
//! host or NATed addresses via a STUN server ... The user of the library is
//! expected to add new local candidates as they are discovered." Its
//! `IceAgentEvent` enum has variants for ICE restart / connection-state /
//! discovered-remote / nominated-send — but **none for "here is a new local
//! candidate to send."** So calling `rtc.add_local_candidate(srflx)` only makes
//! str0m USE the candidate internally for pair formation; it will never hand it
//! back to be trickled. Therefore WE serialize the candidate ourselves with
//! `Candidate::to_sdp_string()` and emit the `dc_ice` signal. (The host
//! candidate avoids this only because it's added *before* `sdp_api().apply()`,
//! so it rides inside the SDP offer/answer. The srflx is discovered *after* a
//! network round-trip to the STUN server, so it is always trickled-after — which
//! is exactly the WebRTC "trickle ICE" model str0m says it's permanently in.)

#![cfg(feature = "native")]

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use str0m::change::{SdpAnswer, SdpOffer, SdpPendingOffer};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

/// STUN servers we query to learn our server-reflexive (public) address, for
/// inc-3a cross-NAT connectivity. These mirror the web client's ICE config
/// (`web/chat/chat-voice-rooms.js`'s `rtcConfig.iceServers`), so a native peer
/// and a browser peer derive their srflx from the same public reflectors.
///
/// Stored as bare `host:port` (NOT `stun:` URLs) because we resolve them with
/// `ToSocketAddrs` directly — there's no URL parsing here, just DNS + UDP.
/// (TURN — the `turn:`/`turns:` entries in the web config — is inc-3b and is
/// deliberately NOT listed here; this increment is STUN-only.)
const STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
];

/// How often to (re)send STUN Binding Requests until we have learned a srflx
/// address. A request or its response can be lost, so we retry on a slow cadence
/// rather than fire-once. Once `srflx` is known we stop (a fixed public mapping
/// is fine for our short-lived data channels; we don't keepalive-refresh the
/// mapping in inc-3a — that's a TURN/long-session concern).
const STUN_RETRY_INTERVAL: Duration = Duration::from_secs(2);

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

    // ── inc-3a STUN / server-reflexive state ──────────────────────────────
    /// Outstanding STUN Binding queries: transaction-id → the server address we
    /// sent it to. We match an inbound Binding *Response* against this map (by
    /// txid AND source) before routing the datagram to any peer, so a STUN
    /// reply is never mistaken for WebRTC traffic. Cleared once we have a srflx.
    stun_pending: HashMap<[u8; 12], SocketAddr>,
    /// Resolved STUN server addresses (DNS results for `STUN_SERVERS`). Resolved
    /// lazily on first gather so a DNS hiccup at startup doesn't kill the thread;
    /// re-resolved each retry if empty.
    stun_servers: Vec<SocketAddr>,
    /// Our learned server-reflexive (public) address, once a Binding Response
    /// arrives. `None` until then. Cached so peers created later also get it.
    srflx: Option<SocketAddr>,
    /// When we last sent a batch of STUN Binding Requests, for the retry cadence.
    /// `None` means "never sent" → send immediately on first loop turn.
    last_stun_send: Option<Instant>,
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
                // inc-3a: STUN starts empty; the run-loop kicks off gathering.
                stun_pending: HashMap::new(),
                stun_servers: Vec::new(),
                srflx: None,
                last_stun_send: None,
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

            // ── A2. inc-3a STUN gather: until we know our server-reflexive
            //    (public) address, (re)send Binding Requests to the STUN servers
            //    on a slow cadence. This does NOT mutate any Rtc — it only sends
            //    plain STUN datagrams on the shared socket — so it's free of the
            //    str0m single-mutation invariant and can sit anywhere in the loop.
            self.maybe_send_stun();

            // ── B. Drain poll_output for EVERY peer until each returns Timeout.
            //    Collect the soonest deadline across all peers; that's how long
            //    we're allowed to block on the UDP read. Dead peers (ICE failed
            //    / SCTP closed) are reaped here.
            let now = Instant::now();
            let mut soonest = now + MAX_POLL_INTERVAL;
            // If we're still STUN-gathering, make sure we wake in time to retry.
            if self.srflx.is_none() {
                if let Some(last) = self.last_stun_send {
                    soonest = soonest.min(last + STUN_RETRY_INTERVAL);
                }
            }
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
                    let slice = &buf[..n];

                    // ── D0. inc-3a: STUN-response demux, BEFORE the per-peer
                    //    demux. If this datagram came from a STUN server we
                    //    queried AND parses as a Binding Response (type 0x0101)
                    //    carrying a transaction id we sent, it's OUR srflx
                    //    answer — consume it here and DO NOT route it to a peer.
                    //    A real WebRTC datagram (peer's ICE binding / DTLS /
                    //    SRTP) does not come from a STUN-server address and/or
                    //    won't carry one of our pending txids, so it falls
                    //    through to the existing demux below. (str0m's own ICE
                    //    connectivity-check STUN — between the two peers — is
                    //    sourced from the *peer*, not the STUN server, so it is
                    //    never swallowed here.)
                    if self.try_handle_stun_response(source, slice) {
                        // It was our STUN reply (or junk from a STUN server);
                        // handled. Skip the peer demux for this datagram.
                        continue;
                    }

                    // Route the datagram to the peer whose Rtc accepts it.
                    // str0m's `accepts` inspects the parsed datagram (STUN
                    // ufrag, DTLS/SRTP association) to decide ownership; it's
                    // the canonical demux. We build the borrowed Input inside
                    // this scope so the &buf borrow ends before the next loop.
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

        // Our host ICE candidate is the shared UDP socket's address. This still
        // rides inside the SDP offer (added before apply()) and is all that's
        // needed on a LAN.
        // inc-3b TODO: also add a TURN-relayed candidate for symmetric NAT.
        match Candidate::host(self.local_addr, "udp") {
            Ok(cand) => {
                rtc.add_local_candidate(cand);
            }
            Err(e) => {
                log::error!("WebRTC: bad host candidate {}: {e}", self.local_addr);
                return;
            }
        }

        // inc-3a: if we ALREADY know our server-reflexive address (a previous
        // peer's gathering learned it), add it before apply() so it rides in
        // THIS offer's SDP too — no extra trickle round-trip needed for it.
        // (If srflx is learned later, apply_srflx_to_all_peers trickles it.)
        if let Some(srflx) = self.srflx {
            match Candidate::server_reflexive(srflx, self.local_addr, Protocol::Udp) {
                Ok(cand) => {
                    rtc.add_local_candidate(cand);
                }
                Err(e) => log::warn!("WebRTC: bad srflx candidate {srflx} for offer: {e}"),
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

        // inc-3a: as on the offerer side, if our srflx is already known, add it
        // before accept_offer() so it rides in the SDP answer. Otherwise it's
        // trickled later by apply_srflx_to_all_peers once STUN resolves.
        if let Some(srflx) = self.srflx {
            match Candidate::server_reflexive(srflx, self.local_addr, Protocol::Udp) {
                Ok(cand) => {
                    rtc.add_local_candidate(cand);
                }
                Err(e) => log::warn!("WebRTC: bad srflx candidate {srflx} for answer: {e}"),
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
        // Native peers now send the SAME object shape too (see
        // `emit_ice_candidate`, used by the inc-3a srflx trickle) — but we still
        // accept BOTH a bare SDP string and that object here, for robustness and
        // backward-compat with any plain-line sender.
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

    // ════════════════════════════════════════════════════════════════════
    //  inc-3a — STUN server-reflexive gathering
    // ════════════════════════════════════════════════════════════════════

    /// If we don't yet know our server-reflexive (public) address, (re)send a
    /// STUN Binding Request to each configured STUN server, at most once per
    /// `STUN_RETRY_INTERVAL`. No-op once `srflx` is known.
    ///
    /// This only sends opaque UDP datagrams on the shared socket — it touches no
    /// `Rtc` — so it is exempt from str0m's single-mutation drain invariant.
    fn maybe_send_stun(&mut self) {
        if self.srflx.is_some() {
            return; // already learned our public address — nothing to do.
        }
        // Rate-limit: only send if we've never sent, or the retry interval has
        // elapsed since the last batch.
        let now = Instant::now();
        if let Some(last) = self.last_stun_send {
            if now.duration_since(last) < STUN_RETRY_INTERVAL {
                return;
            }
        }

        // Resolve STUN server hostnames to addresses if we haven't yet (or a
        // prior resolution produced nothing). DNS can transiently fail; we just
        // retry next interval rather than treating it as fatal.
        if self.stun_servers.is_empty() {
            for host in STUN_SERVERS {
                match host.to_socket_addrs() {
                    Ok(addrs) => {
                        // Prefer IPv4 so the srflx base (our local host addr,
                        // which is IPv4 from `0.0.0.0:0`) matches the family —
                        // `Candidate::server_reflexive` rejects a mismatch.
                        for a in addrs {
                            if a.is_ipv4() {
                                self.stun_servers.push(a);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("WebRTC: STUN DNS resolve '{host}' failed: {e}");
                    }
                }
            }
            if self.stun_servers.is_empty() {
                // Couldn't resolve any server this turn; arm the timer so we
                // retry on the next interval instead of hot-looping on DNS.
                self.last_stun_send = Some(now);
                return;
            }
        }

        // Send a Binding Request to each server with a fresh random txid, and
        // remember the txid → server mapping so we can match the response.
        let servers = self.stun_servers.clone();
        for server in servers {
            let txid = stun::random_transaction_id();
            let req = stun::build_binding_request(&txid);
            match self.udp.send_to(&req, server) {
                Ok(_) => {
                    self.stun_pending.insert(txid, server);
                    log::trace!("WebRTC: sent STUN Binding Request to {server}");
                }
                Err(e) => log::debug!("WebRTC: STUN send to {server} failed: {e}"),
            }
        }
        self.last_stun_send = Some(now);
        crate::debug::push_debug("WebRTC: STUN gathering (sent Binding Requests)");
    }

    /// Try to interpret an inbound datagram as a STUN Binding Response to one of
    /// our pending requests. Returns `true` if the datagram was consumed as a
    /// STUN reply (and therefore must NOT be routed to a peer), `false` if it's
    /// not ours and should fall through to the per-peer WebRTC demux.
    ///
    /// We only treat it as STUN if BOTH (a) the source is a STUN server we
    /// queried — checked via the pending map's values — and (b) it parses as a
    /// Binding Response whose transaction id is in our pending map. That double
    /// guard means a peer's ICE connectivity-check STUN (sourced from the peer,
    /// not the server) is never swallowed here.
    fn try_handle_stun_response(&mut self, source: SocketAddr, datagram: &[u8]) -> bool {
        // Cheap pre-filter: was this source one of the servers we queried? If
        // not, it can't be our STUN reply — fall through immediately.
        let from_known_server = self.stun_pending.values().any(|&s| s == source)
            || self.stun_servers.iter().any(|&s| s == source);
        if !from_known_server {
            return false;
        }

        // Parse as a STUN Binding Response. Returns the txid + the
        // XOR-MAPPED-ADDRESS (our srflx) on success.
        let (txid, mapped) = match stun::parse_binding_response(datagram) {
            Some(parsed) => parsed,
            None => {
                // From a STUN server but not a parseable Binding Response with a
                // mapped address — swallow it (it's STUN-server traffic, not a
                // peer datagram), but learn nothing.
                return true;
            }
        };

        // Only accept it if we actually sent this transaction id.
        if !self.stun_pending.contains_key(&txid) {
            log::trace!("WebRTC: STUN response from {source} with unknown txid — ignoring");
            return true; // still STUN-server traffic, don't route to a peer.
        }
        // Consume the pending entry; we have our answer.
        self.stun_pending.remove(&txid);

        log::info!("WebRTC: learned server-reflexive address {mapped} (via STUN {source})");
        crate::debug::push_debug(format!("WebRTC srflx = {mapped}"));

        // First response wins. (Multiple STUN servers may reply; behind a
        // well-behaved NAT they should agree. We don't try to detect symmetric
        // NAT here — that's the TURN/inc-3b fallback's job.)
        if self.srflx.is_none() {
            self.srflx = Some(mapped);
            // We're done gathering — drop any other outstanding requests so a
            // late duplicate reply is just ignored.
            self.stun_pending.clear();
            // Add the srflx to every live peer and trickle it to each.
            self.apply_srflx_to_all_peers(mapped);
        }
        true
    }

    /// Add the srflx candidate to every currently-live peer's `Rtc` and trickle
    /// it to each as a `dc_ice` signal. Used when srflx is learned after peers
    /// already exist.
    fn apply_srflx_to_all_peers(&mut self, srflx: SocketAddr) {
        // Snapshot keys so we can mutate self.peers / emit signals in the loop.
        let keys: Vec<String> = self.peers.keys().cloned().collect();
        for key in keys {
            self.add_and_trickle_srflx(&key, srflx);
        }
    }

    /// Add the srflx candidate to ONE peer's `Rtc` (if alive) and trickle the
    /// candidate line to that peer. Safe to call more than once for the same
    /// peer — str0m's `add_local_candidate` dedupes redundant candidates, and a
    /// duplicate trickle is harmless (the far side's `addIceCandidate` /
    /// `add_remote_candidate` also dedupes).
    fn add_and_trickle_srflx(&mut self, peer_key: &str, srflx: SocketAddr) {
        // Build the server-reflexive candidate: `addr` = our public srflx
        // address from STUN, `base` = our local host socket address (the real
        // socket the srflx is a NAT translation of). Both must be the same IP
        // family — we resolved an IPv4 STUN server above so this holds.
        let cand = match Candidate::server_reflexive(srflx, self.local_addr, Protocol::Udp) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("WebRTC: bad srflx candidate {srflx} (base {}): {e}", self.local_addr);
                return;
            }
        };

        // Serialize to the SDP `candidate:...` line BEFORE moving it into
        // add_local_candidate. str0m never emits added local candidates back to
        // us (see the module docs), so we must trickle it ourselves.
        let sdp_line = cand.to_sdp_string();

        match self.peers.get_mut(peer_key) {
            Some(p) if p.rtc.is_alive() => {
                // add_local_candidate is a mutation, but the surrounding run
                // loop always drains poll_output to Timeout afterward (step B),
                // satisfying str0m's single-mutation invariant.
                p.rtc.add_local_candidate(cand);
            }
            _ => {
                // Peer gone or dead — don't trickle a candidate nobody's using.
                return;
            }
        }

        // Trickle the srflx to the far side as a dc_ice signal. We emit the
        // browser-compatible OBJECT shape `{candidate, sdpMid, sdpMLineIndex}`
        // (see emit_ice_candidate) so a browser peer's `addIceCandidate` accepts
        // it; a native peer's `on_ice` accepts both the object and a bare line.
        self.emit_ice_candidate(peer_key, &sdp_line);
        log::debug!("WebRTC: trickled srflx candidate to {}", short(peer_key));
    }

    /// Emit a `dc_ice` signal carrying ONE local ICE candidate, in the
    /// browser-compatible RTCIceCandidate object shape.
    ///
    /// # Why the object shape (not a bare SDP line)
    ///
    /// The browser side (`web/chat/chat-p2p.js::handleDCIce`) does
    /// `pc.addIceCandidate(new RTCIceCandidate(JSON.parse(signal.data)))`. The
    /// `RTCIceCandidate` constructor REQUIRES an object with a `candidate`
    /// field, and the candidate must be tied to an m-line — passing a bare
    /// string throws `TypeError`. So `data` must stringify to
    /// `{"candidate":"candidate:...","sdpMid":"0","sdpMLineIndex":0}`.
    ///
    /// The **load-bearing** field is `sdpMLineIndex: 0`, NOT `sdpMid`. We have
    /// exactly ONE m-line (the single data channel), so index 0 always resolves
    /// to it on the receiving browser. str0m assigns the data-channel m-line a
    /// random mid via `new_mid()` (not literally "0"), so a future reader must
    /// NOT try to "fix" `sdpMid` to chase str0m's mid — the browser matches a
    /// remote candidate by `sdpMLineIndex` when `sdpMid` doesn't match, and with
    /// a single m-line index 0 is unambiguous. `sdpMid:"0"` is just a benign
    /// placeholder. Native↔native is unaffected either way: our own `on_ice`
    /// reads only the `.candidate` line and ignores both mid fields.
    fn emit_ice_candidate(&self, to: &str, sdp_line: &str) {
        // The candidate object the far side will JSON.parse. `data` itself is a
        // JSON STRING per the signaling envelope contract (matching the web
        // client's `JSON.stringify(candidate)`).
        let cand_obj = serde_json::json!({
            "candidate": sdp_line,
            "sdpMid": "0",
            "sdpMLineIndex": 0,
        });
        let data = cand_obj.to_string();
        self.emit_signal(to, "dc_ice", data);
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

// ════════════════════════════════════════════════════════════════════════
//  inc-3a — Minimal STUN (RFC 5389) Binding client
// ════════════════════════════════════════════════════════════════════════
//
// We hand-roll JUST the Binding request/response we need to learn our
// server-reflexive (public) address — no external STUN crate, no auth, no
// other message types. This is deliberately tiny (~a request builder + a
// response parser for ONE attribute). The protocol surface:
//
//   STUN message header (20 bytes, RFC 5389 §6):
//     0                   1                   2                   3
//     0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |0 0|     STUN Message Type      |         Message Length        |
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |                         Magic Cookie  (0x2112A442)            |
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |                     Transaction ID (96 bits / 12 bytes)       |
//    |                                                               |
//    |                                                               |
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//
//   Binding Request message type = 0x0001; Binding Success Response = 0x0101.
//   Magic cookie = 0x2112A442 (fixed). Our request carries NO attributes, so
//   Message Length = 0 — a STUN server replies with our mapped address anyway.
//
//   XOR-MAPPED-ADDRESS attribute (type 0x0020, RFC 5389 §15.2), IPv4 form:
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |  attr type 0x0020 (2 bytes)   |   attr length 0x0008 (2 bytes)|
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |  0x00 (reserved)  |  family   |        X-Port (XOR'd)         |
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |              X-Address (XOR'd, 4 bytes for IPv4)              |
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//   family = 0x01 (IPv4) / 0x02 (IPv6).
//   X-Port    = real port    XOR  high 16 bits of the magic cookie (0x2112).
//   X-Address = real address XOR  the magic cookie (0x2112A442), big-endian.
//   (We only parse IPv4 here; our local socket binds 0.0.0.0 → IPv4 base.)
mod stun {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    /// The fixed STUN magic cookie (RFC 5389 §6), big-endian.
    pub const MAGIC_COOKIE: u32 = 0x2112_A442;
    /// STUN message type: Binding Request.
    const TYPE_BINDING_REQUEST: u16 = 0x0001;
    /// STUN message type: Binding Success Response.
    const TYPE_BINDING_RESPONSE: u16 = 0x0101;
    /// Attribute type: XOR-MAPPED-ADDRESS.
    const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
    /// Attribute type: MAPPED-ADDRESS (legacy, non-XOR; some servers also send).
    const ATTR_MAPPED_ADDRESS: u16 = 0x0001;

    /// Generate a random 96-bit (12-byte) STUN transaction id.
    ///
    /// Uses the crate's existing `rand` 0.9 dependency (the same
    /// `rand::rng().fill_bytes(..)` idiom as `group_e2ee.rs` / `api_v2.rs`),
    /// rather than a new RNG crate. The transaction id only needs to be
    /// unguessable enough that a response is matched to its request — it's not
    /// cryptographically load-bearing.
    pub fn random_transaction_id() -> [u8; 12] {
        use rand::RngCore;
        let mut id = [0u8; 12];
        rand::rng().fill_bytes(&mut id);
        id
    }

    /// Build a 20-byte STUN Binding Request with the given transaction id and
    /// NO attributes (message length 0).
    ///
    /// Layout: type(2) | length(2) | magic(4) | txid(12).
    pub fn build_binding_request(txid: &[u8; 12]) -> [u8; 20] {
        let mut msg = [0u8; 20];
        // Message type (Binding Request), big-endian.
        msg[0..2].copy_from_slice(&TYPE_BINDING_REQUEST.to_be_bytes());
        // Message length = 0 (no attributes), big-endian.
        msg[2..4].copy_from_slice(&0u16.to_be_bytes());
        // Magic cookie, big-endian.
        msg[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
        // Transaction id (12 bytes).
        msg[8..20].copy_from_slice(txid);
        msg
    }

    /// Parse a datagram as a STUN Binding Success Response and extract the
    /// transaction id + the mapped (server-reflexive) address.
    ///
    /// Returns `Some((txid, addr))` if the datagram is a well-formed Binding
    /// Response carrying an (XOR-)MAPPED-ADDRESS, else `None`. We accept
    /// XOR-MAPPED-ADDRESS (modern, mandatory) and fall back to legacy
    /// MAPPED-ADDRESS if that's all a server sends.
    pub fn parse_binding_response(buf: &[u8]) -> Option<([u8; 12], SocketAddr)> {
        // Need at least the 20-byte header.
        if buf.len() < 20 {
            return None;
        }
        let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
        if msg_type != TYPE_BINDING_RESPONSE {
            return None;
        }
        // Validate the magic cookie — guards against random UDP junk.
        let cookie = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        if cookie != MAGIC_COOKIE {
            return None;
        }
        let msg_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        // The attributes region is `msg_len` bytes after the 20-byte header.
        if buf.len() < 20 + msg_len {
            return None;
        }
        let mut txid = [0u8; 12];
        txid.copy_from_slice(&buf[8..20]);

        // Walk the TLV attributes. Each attribute is: type(2) | length(2) |
        // value(length) | padding to a 4-byte boundary.
        let attrs = &buf[20..20 + msg_len];
        let mut off = 0usize;
        while off + 4 <= attrs.len() {
            let atype = u16::from_be_bytes([attrs[off], attrs[off + 1]]);
            let alen = u16::from_be_bytes([attrs[off + 2], attrs[off + 3]]) as usize;
            let vstart = off + 4;
            if vstart + alen > attrs.len() {
                break; // truncated/malformed attribute — stop.
            }
            let value = &attrs[vstart..vstart + alen];

            match atype {
                ATTR_XOR_MAPPED_ADDRESS => {
                    if let Some(addr) = parse_xor_mapped_address(value) {
                        return Some((txid, addr));
                    }
                }
                ATTR_MAPPED_ADDRESS => {
                    if let Some(addr) = parse_mapped_address(value) {
                        return Some((txid, addr));
                    }
                }
                _ => { /* ignore other attributes (SOFTWARE, etc.) */ }
            }

            // Advance past value + 4-byte-boundary padding.
            let padded = (alen + 3) & !3;
            off = vstart + padded;
        }
        None
    }

    /// Decode an XOR-MAPPED-ADDRESS attribute value (IPv4 only).
    ///
    /// Value layout: reserved(1) | family(1) | x_port(2) | x_address(4).
    /// x_port    = port XOR (high 16 bits of magic cookie) = port XOR 0x2112.
    /// x_address = address XOR magic cookie (big-endian).
    fn parse_xor_mapped_address(value: &[u8]) -> Option<SocketAddr> {
        // 1 reserved + 1 family + 2 port + 4 addr = 8 bytes for IPv4.
        if value.len() < 8 {
            return None;
        }
        let family = value[1];
        if family != 0x01 {
            // 0x02 = IPv6; we only handle IPv4 srflx in inc-3a (our socket base
            // is IPv4). An IPv6 mapped address is ignored.
            return None;
        }
        // X-Port XOR with the top 16 bits of the magic cookie.
        let x_port = u16::from_be_bytes([value[2], value[3]]);
        let port = x_port ^ ((MAGIC_COOKIE >> 16) as u16);
        // X-Address XOR with the full 32-bit magic cookie (big-endian).
        let x_addr = u32::from_be_bytes([value[4], value[5], value[6], value[7]]);
        let addr = x_addr ^ MAGIC_COOKIE;
        let ip = Ipv4Addr::from(addr);
        Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
    }

    /// Decode a legacy (non-XOR) MAPPED-ADDRESS attribute value (IPv4 only).
    /// Value layout: reserved(1) | family(1) | port(2) | address(4), no XOR.
    fn parse_mapped_address(value: &[u8]) -> Option<SocketAddr> {
        if value.len() < 8 {
            return None;
        }
        if value[1] != 0x01 {
            return None; // IPv4 only
        }
        let port = u16::from_be_bytes([value[2], value[3]]);
        let addr = u32::from_be_bytes([value[4], value[5], value[6], value[7]]);
        let ip = Ipv4Addr::from(addr);
        Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::net::SocketAddr;

        /// RFC 5769 §2.1 "Sample Request" transaction id.
        const RFC5769_TXID: [u8; 12] = [
            0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
        ];

        /// Known-answer test: our Binding Request builder must emit the exact
        /// header bytes of the RFC 5769 §2.1 sample request (type, length=0,
        /// magic cookie, transaction id). The RFC's full sample additionally
        /// carries SOFTWARE/USERNAME/etc. attributes; we send an attribute-less
        /// request (length 0), so we assert the 20-byte header is byte-correct.
        #[test]
        fn binding_request_header_is_rfc5769_correct() {
            let req = build_binding_request(&RFC5769_TXID);
            assert_eq!(req.len(), 20, "header is exactly 20 bytes");
            // Type = Binding Request = 0x0001.
            assert_eq!(&req[0..2], &[0x00, 0x01], "message type = Binding Request");
            // Length = 0 (no attributes).
            assert_eq!(&req[2..4], &[0x00, 0x00], "message length = 0");
            // Magic cookie = 0x2112A442.
            assert_eq!(&req[4..8], &[0x21, 0x12, 0xa4, 0x42], "magic cookie");
            // Transaction id matches the RFC sample.
            assert_eq!(&req[8..20], &RFC5769_TXID, "transaction id");
        }

        /// Known-answer test: parse the RFC 5769 §2.2 "Sample IPv4 Response"
        /// XOR-MAPPED-ADDRESS attribute and confirm it decodes to 192.0.2.1:32853.
        ///
        /// We construct a minimal valid Binding Response: the 20-byte header
        /// (type 0x0101, msg-len = 12 = one 8-byte XOR-MAPPED-ADDRESS value +
        /// its 4-byte TLV header, magic cookie, the RFC txid) followed by the
        /// exact attribute bytes from RFC 5769 §2.2:
        ///   00 20 00 08 00 01 a1 47 e1 12 a6 43
        /// where 0x0020 = XOR-MAPPED-ADDRESS, 0x0008 = value length, 0x00 =
        /// reserved, 0x01 = IPv4, 0xa147 = X-Port, 0xe112a643 = X-Address.
        ///   X-Port    0xa147 ^ 0x2112      = 0x8055 = 32853
        ///   X-Address 0xe112a643 ^ 0x2112a442 = 0xc0000201 = 192.0.2.1
        #[test]
        fn parse_rfc5769_xor_mapped_address() {
            let mut msg = Vec::new();
            // Header.
            msg.extend_from_slice(&TYPE_BINDING_RESPONSE.to_be_bytes()); // 0x0101
            msg.extend_from_slice(&12u16.to_be_bytes()); // msg length = 12
            msg.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
            msg.extend_from_slice(&RFC5769_TXID);
            // XOR-MAPPED-ADDRESS attribute (RFC 5769 §2.2 exact bytes).
            msg.extend_from_slice(&[
                0x00, 0x20, // attr type = XOR-MAPPED-ADDRESS
                0x00, 0x08, // attr length = 8
                0x00, // reserved
                0x01, // family = IPv4
                0xa1, 0x47, // X-Port
                0xe1, 0x12, 0xa6, 0x43, // X-Address
            ]);

            let (txid, addr) = parse_binding_response(&msg)
                .expect("RFC 5769 §2.2 response must parse");
            assert_eq!(txid, RFC5769_TXID, "transaction id round-trips");
            let expected: SocketAddr = "192.0.2.1:32853".parse().unwrap();
            assert_eq!(addr, expected, "XOR-MAPPED-ADDRESS decodes to 192.0.2.1:32853");
        }

        /// A full build→parse round-trip with an arbitrary address proves the
        /// XOR encode/decode is self-consistent (encode here, decode via the
        /// production parser).
        #[test]
        fn build_then_parse_roundtrip() {
            let txid = random_transaction_id();
            // Craft a response carrying a XOR-MAPPED-ADDRESS for 203.0.113.7:54321.
            let real_ip: u32 = u32::from(std::net::Ipv4Addr::new(203, 0, 113, 7));
            let real_port: u16 = 54321;
            let x_addr = real_ip ^ MAGIC_COOKIE;
            let x_port = real_port ^ ((MAGIC_COOKIE >> 16) as u16);

            let mut msg = Vec::new();
            msg.extend_from_slice(&TYPE_BINDING_RESPONSE.to_be_bytes());
            msg.extend_from_slice(&12u16.to_be_bytes());
            msg.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
            msg.extend_from_slice(&txid);
            msg.extend_from_slice(&[0x00, 0x20, 0x00, 0x08, 0x00, 0x01]);
            msg.extend_from_slice(&x_port.to_be_bytes());
            msg.extend_from_slice(&x_addr.to_be_bytes());

            let (got_txid, addr) = parse_binding_response(&msg).expect("must parse");
            assert_eq!(got_txid, txid);
            assert_eq!(addr, "203.0.113.7:54321".parse::<SocketAddr>().unwrap());
        }

        /// Non-STUN junk and wrong message types must be rejected (return None),
        /// so we never mistake a peer datagram for a STUN reply.
        #[test]
        fn rejects_non_stun_and_wrong_type() {
            // Too short.
            assert!(parse_binding_response(&[0u8; 4]).is_none());
            // Right length, wrong magic cookie.
            let mut bad = [0u8; 20];
            bad[0..2].copy_from_slice(&TYPE_BINDING_RESPONSE.to_be_bytes());
            bad[4..8].copy_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
            assert!(parse_binding_response(&bad).is_none(), "bad magic cookie rejected");
            // A Binding *Request* (0x0001), not a response — must be ignored.
            let req = build_binding_request(&RFC5769_TXID);
            assert!(parse_binding_response(&req).is_none(), "request is not a response");
        }
    }
}

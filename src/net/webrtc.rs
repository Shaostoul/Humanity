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
//! - inc-3a: STUN server-reflexive (srflx) candidate gathering
//!   so two peers behind *different* NATs can connect (the host candidate alone
//!   only works same-LAN). We hand-roll a tiny RFC 5389 STUN Binding client over
//!   the manager's existing shared `UdpSocket`, learn our public `ip:port`
//!   (server-reflexive address) from the Binding Response's XOR-MAPPED-ADDRESS,
//!   add it as a local candidate to every peer's `Rtc`, and *trickle* it to the
//!   far side as a `dc_ice` signal. See the `// STUN srflx` / `// inc-3a`
//!   markers and the `mod stun` block.
//! - inc-3b (THIS increment): TURN relay (RFC 5766) for the symmetric-NAT
//!   fallback. When BOTH peers are behind symmetric NATs, srflx hole-punching
//!   fails (each NAT maps the same internal socket to a *different* external
//!   port per destination, so the srflx the peer learned is useless for the
//!   peer-to-peer 5-tuple). The fix is a relay: both peers send to / receive
//!   from a shared TURN server, which forwards between them. We hand-roll an
//!   RFC 5766 TURN client (long-term-credential auth) over the SAME shared
//!   `UdpSocket`: Allocate (→ a relayed transport address), CreatePermission +
//!   ChannelBind per peer, then relay peer traffic as TURN ChannelData. The
//!   relayed transport address is added to each peer's `Rtc` as a
//!   `Candidate::relayed` and trickled like the srflx. See the `// inc-3b` /
//!   `// TURN` markers and the `mod turn` block at the bottom of the file.
//!
//! # How TURN rides the EXISTING data path without disturbing host/srflx (inc-3b)
//!
//! str0m is **completely TURN-agnostic** — it has no idea a candidate is
//! relayed beyond using a lower priority for it. When str0m decides to send to
//! a peer over the relayed pair, it hands us a normal
//! `Output::Transmit { source, destination, contents }` where (verified in the
//! `is` ICE crate, `agent.rs`): `source = local_candidate.base()` and
//! `destination = remote_candidate.addr()`. For a `Candidate::relayed`, `base()`
//! is the TURN-allocated *relayed address* (the `relayed()` ctor sets
//! `base = Some(addr)` with `addr` = the relayed address). So **every** datagram
//! str0m emits for a relayed pair — ICE connectivity checks AND DTLS/SCTP data
//! alike (the `NominatedSend` event also carries `source: local.base()`) — is
//! stamped with `source == our_relayed_addr`. That single fact is our guard:
//!
//!   * **Transmit-wrap (outbound):** in the `Output::Transmit` handler, IF
//!     `t.source == our_relayed_addr` we wrap `t.contents` as TURN ChannelData
//!     to the TURN *server* (the inner datagram is addressed to the peer via the
//!     bound channel). ELSE we `udp.send_to(t.destination)` raw — the UNCHANGED
//!     inc-1/2/3a path. Host/srflx transmits never have `source ==
//!     our_relayed_addr`, so they are byte-for-byte unaffected.
//!   * **Recv-unwrap (inbound):** in the recv path, AFTER the inc-3a STUN demux
//!     and BEFORE the per-peer WebRTC demux, IF the datagram's `source ==
//!     turn_server_addr` we treat it as TURN (ChannelData / Data indication /
//!     Allocate/Refresh/CreatePermission/ChannelBind reply). ChannelData /
//!     Data is unwrapped to `(peer_addr, inner)` and fed to str0m as
//!     `Input::Receive { source: peer_addr, destination: our_relayed_addr, .. }`
//!     so str0m's ICE demux (which matches a relayed local candidate by
//!     `addr() == destination`) accepts it. Any datagram NOT from the TURN
//!     server falls straight through to the existing demux untouched.
//!
//! TURN is strictly **best-effort**: if the allocation fails (auth rejected,
//! server down) we log and keep running with host+srflx only. The relayed
//! candidate is just one more ICE candidate; its absence only costs us the
//! symmetric-NAT fallback, never the working same-LAN/STUN path.
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
use str0m::format::Codec;
use str0m::media::{Direction, Frequency, MediaKind, MediaTime, Mid};
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

// ── inc-3b TURN relay configuration ──────────────────────────────────────
//
// The operator's TURN server + long-term credentials. These mirror the web
// client's `rtcConfig.iceServers` TURN entry (`web/chat/chat-voice-rooms.js`
// ~line 22), so a native peer and a browser peer relay through the SAME server.
// The credentials are already hardcoded/public in that client JS, so reusing
// them as consts here exposes nothing new.

/// TURN server address (the UDP `turn:` listener). Resolved with `ToSocketAddrs`
/// like the STUN servers — bare `host:port`, no `turn:` URL scheme.
const TURN_SERVER: &str = "united-humanity.us:3478";
/// TURN long-term-credential username.
const TURN_USERNAME: &str = "humanity";
/// TURN long-term-credential password (a.k.a. the credential / shared secret).
const TURN_PASSWORD: &str = "turnRelay2026!secure";

/// How often to (re)try the initial Allocate until we either succeed or give up
/// for this session. Mirrors `STUN_RETRY_INTERVAL` — a request or its 401/reply
/// can be lost, so we re-send on a slow cadence rather than fire-once.
const TURN_ALLOC_RETRY_INTERVAL: Duration = Duration::from_secs(2);

/// We refresh the TURN allocation this long *before* its LIFETIME expires, so a
/// slightly late timer never drops the allocation mid-session. RFC 5766 §6
/// suggests refreshing well ahead of expiry; 60s of slack is generous for the
/// default 600s lifetime and harmless for shorter ones (clamped below).
const TURN_REFRESH_SLACK: Duration = Duration::from_secs(60);

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
    /// One depayloaded Opus frame arrived from `peer` over its voice m-line
    /// (Phase B). Decode + playback is wired in a later phase.
    VoiceFrame { peer: String, opus: Vec<u8> },
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
    /// `wants_voice` negotiates an Opus audio m-line too (Phase B).
    OfferTo { peer: String, wants_voice: bool },
    /// Application request: send `text` to `peer` over its open channel.
    SendText { peer: String, text: String },
    /// Application request: send one encoded Opus frame to `peer` over its voice
    /// m-line (Phase B). Dropped if the peer has no negotiated audio m-line.
    SendVoice { peer: String, opus: Vec<u8> },
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
        let _ = self.tx_cmd.send(Command::OfferTo { peer, wants_voice: false });
    }

    /// Like [`offer_to`](Self::offer_to), but also negotiates a bidirectional
    /// Opus audio m-line so the connection can carry voice (Phase B). Used by the
    /// voice-room join. Same offerer-rule semantics as `offer_to`.
    pub fn offer_to_voice(&self, peer: String) {
        let _ = self.tx_cmd.send(Command::OfferTo { peer, wants_voice: true });
    }

    /// Send one encoded Opus frame to `peer` over its voice m-line (Phase B).
    /// Non-blocking; dropped if the peer has no negotiated audio m-line yet.
    pub fn send_voice(&self, peer: String, opus: Vec<u8>) {
        let _ = self.tx_cmd.send(Command::SendVoice { peer, opus });
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

    // ── inc-3b TURN relay client state ────────────────────────────────────
    /// The whole TURN client. `None` until/unless we successfully resolve the
    /// TURN server address and begin an allocation. Best-effort: if it never
    /// reaches `Allocated`, host+srflx still work — TURN just adds nothing.
    turn: Option<turn::TurnClient>,
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
    /// Voice (Phase B, v0.489): the audio m-line mid, if this connection
    /// negotiated one. The offerer gets it from `add_media`; the answerer learns
    /// it from `Event::MediaAdded`. `None` => a data-only connection (the
    /// default; existing P2P-group connections never request voice, so their SDP
    /// is unchanged).
    audio_mid: Option<Mid>,
    /// Monotonic 48 kHz RTP timestamp for outgoing Opus, advanced 960 per 20 ms
    /// frame. Never reset mid-stream or the remote jitter buffer misorders.
    voice_rtp_ts: u64,
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
                // inc-3b: TURN starts uninitialized; the run-loop kicks off the
                // allocation lazily (so a startup DNS hiccup isn't fatal).
                turn: None,
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

            // ── A3. inc-3b TURN: drive the TURN client state machine — kick off
            //    the Allocate if not started, retry it on a cadence, and refresh
            //    the allocation before it expires. Like STUN, this ONLY sends
            //    plain TURN datagrams on the shared socket and (on a fresh
            //    allocation) trickles a relayed candidate to peers; the trickle
            //    is the only Rtc mutation and it goes through `add_and_trickle_*`
            //    which the step-B drain below honors. Best-effort throughout —
            //    failures here never touch the host/srflx path.
            self.maybe_drive_turn();

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
            // inc-3b: wake in time for the next TURN action (allocate retry or
            // allocation refresh), so a far-future str0m deadline never delays
            // a refresh past the allocation's LIFETIME.
            if let Some(turn) = &self.turn {
                if let Some(deadline) = turn.next_deadline() {
                    soonest = soonest.min(deadline);
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

                    // ── D0.5. inc-3b: TURN demux, AFTER the STUN demux and
                    //    BEFORE the per-peer WebRTC demux. ONLY datagrams whose
                    //    `source == turn_server_addr` are considered here — that
                    //    address guard is what keeps host/srflx traffic on the
                    //    untouched path below. The handler:
                    //      * consumes TURN control replies (Allocate 401/success,
                    //        Refresh, CreatePermission, ChannelBind, Data
                    //        indications we don't channel-bind) and returns
                    //        `Handled` → we `continue`;
                    //      * for relayed peer data (ChannelData / Data
                    //        indication), returns `Relayed { peer, range }` — the
                    //        peer's address plus the byte range of the *inner*
                    //        datagram inside `buf`, which we then feed to str0m
                    //        as a normal Receive with `source = peer` and
                    //        `destination = our_relayed_addr` (so str0m's ICE
                    //        demux, which matches a relayed local candidate by
                    //        `addr() == destination`, accepts it);
                    //      * returns `NotTurn` only if the source isn't the TURN
                    //        server, so the datagram falls through unchanged.
                    let turn_recv = self.try_handle_turn(source, slice);
                    let (recv_source, recv_dest, recv_range) = match turn_recv {
                        TurnRecv::Handled => {
                            // A TURN control message — fully handled, not peer data.
                            continue;
                        }
                        TurnRecv::Relayed { peer, start, len } => {
                            // Relayed peer data: rewrite source→peer, dest→relayed
                            // addr, and narrow the slice to the inner datagram.
                            // `our_relayed_addr` must exist if we unwrapped TURN
                            // data; fall back defensively to local_addr if not.
                            let dest = self
                                .turn
                                .as_ref()
                                .and_then(|t| t.relayed_addr())
                                .unwrap_or(self.local_addr);
                            (peer, dest, Some((start, len)))
                        }
                        TurnRecv::NotTurn => {
                            // Not from the TURN server — the EXISTING inc-1/2/3a
                            // path. Source/destination/slice all unchanged.
                            (source, self.local_addr, None)
                        }
                    };

                    // The datagram bytes to hand str0m: either the whole packet
                    // (non-TURN) or the unwrapped inner datagram (relayed).
                    let payload: &[u8] = match recv_range {
                        Some((start, len)) => &buf[start..start + len],
                        None => slice,
                    };

                    // Route the datagram to the peer whose Rtc accepts it.
                    // str0m's `accepts` inspects the parsed datagram (STUN
                    // ufrag, DTLS/SRTP association) to decide ownership; it's
                    // the canonical demux. We build the borrowed Input inside
                    // this scope so the &buf borrow ends before the next loop.
                    let contents = match payload.try_into() {
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
                            // For non-TURN this is the wire source (unchanged);
                            // for relayed traffic it's the PEER address, so str0m
                            // associates it with the relayed candidate pair.
                            source: recv_source,
                            destination: recv_dest,
                            contents,
                        },
                    );

                    // Find the owning peer. We look first at remote_addr (fast
                    // path once learned), then fall back to `accepts`.
                    //
                    // NOTE: we match on `recv_source`, NOT the wire `source`. For
                    // the existing non-TURN path they are identical. For relayed
                    // traffic `recv_source` is the PEER's address (the wire
                    // source was the TURN server), which is what str0m associates
                    // with the connection — matching on the TURN server address
                    // would never find the peer.
                    let owner: Option<String> = {
                        let mut found = None;
                        // Fast path: a peer whose learned remote_addr matches.
                        for (k, p) in self.peers.iter() {
                            if p.remote_addr == Some(recv_source) {
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
                            // Learn / refresh the remote address for the fast
                            // path. For relayed traffic this records the PEER
                            // address (recv_source), not the TURN server.
                            p.remote_addr = Some(recv_source);
                            if let Err(e) = p.rtc.handle_input(input) {
                                log::warn!("WebRTC: handle_input(Receive) error for {}: {e}", short(&key));
                                p.rtc.disconnect();
                            }
                        }
                    } else {
                        // Common during connection setup: a STUN binding may
                        // arrive before we've created the answering Rtc, or
                        // from an unrelated source. Drop quietly. (recv_source is
                        // the peer addr for relayed traffic, the wire source
                        // otherwise.)
                        log::trace!("WebRTC: no peer accepts datagram from {recv_source}");
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
                    // ── inc-3b TURN wrap guard. ────────────────────────────
                    // str0m stamps EVERY datagram for a relayed pair with
                    // `source == our_relayed_addr` (the relayed candidate's
                    // base; see the module docs + the `is` agent.rs). If this
                    // datagram's source is our relayed address, it must be
                    // RELAYED: wrap `t.contents` as TURN ChannelData to the TURN
                    // server (addressed to `t.destination` = the peer via its
                    // bound channel). Otherwise — the overwhelming common case,
                    // host/srflx — fall through to the UNCHANGED raw send. The
                    // guard is a single SocketAddr equality, so host/srflx
                    // traffic is provably never wrapped.
                    let relayed = self
                        .turn
                        .as_ref()
                        .and_then(|tn| tn.relayed_addr())
                        .map(|relayed_addr| t.source == relayed_addr)
                        .unwrap_or(false);

                    if relayed {
                        // Hand the inner datagram + its peer destination to the
                        // TURN client, which frames it as ChannelData (or a Send
                        // indication until a channel is bound) and sends it to
                        // the TURN server over the shared socket. Best-effort:
                        // a wrap/send failure is logged, not fatal.
                        if let Some(turn) = self.turn.as_mut() {
                            turn.send_relayed(&self.udp, t.destination, &t.contents);
                        }
                    } else {
                        // EXISTING inc-1/2/3a path — byte-for-byte unchanged.
                        // str0m tells us the destination (ICE may change it over
                        // the session). A failed send isn't fatal — log and keep
                        // draining.
                        if let Err(e) = self.udp.send_to(&t.contents, t.destination) {
                            log::trace!("WebRTC: udp send_to {} failed: {e}", t.destination);
                        }
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
            Event::MediaAdded(m) => {
                // Voice (Phase B): fires on the ANSWERER when the offer's audio
                // m-line is accepted (the offerer already knows its mid from
                // add_media). Capture it so cmd_send_voice can find the writer.
                if m.kind == MediaKind::Audio {
                    if let Some(p) = self.peers.get_mut(key) {
                        p.audio_mid = Some(m.mid);
                    }
                    log::info!("WebRTC: audio m-line added with {}", short(key));
                }
            }
            Event::MediaData(data) => {
                // Voice (Phase B): one depayloaded Opus frame arrived. Surface it
                // to the GUI/voice engine, tagged with the source peer. (Decode +
                // playback is wired in a later phase; for now consumers may drop it.)
                let _ = self.tx_event.send(WebrtcEvent::VoiceFrame {
                    peer: key.to_string(),
                    opus: data.data.to_vec(),
                });
            }
            _ => {
                // Other media / stats events — not used by this transport.
            }
        }
    }

    /// Send one encoded Opus frame to `peer` over its negotiated voice m-line
    /// (Phase B). Drops silently if the peer is unknown or has no audio m-line
    /// yet. The negotiated Opus payload type is discovered from the writer (str0m
    /// may reassign it from the default 111 during negotiation), never hardcoded.
    fn cmd_send_voice(&mut self, peer: String, opus: Vec<u8>) {
        let p = match self.peers.get_mut(&peer) {
            Some(p) => p,
            None => return,
        };
        let mid = match p.audio_mid {
            Some(m) => m,
            None => return, // not a voice connection (or not negotiated yet)
        };
        let ts = p.voice_rtp_ts;
        let writer = match p.rtc.writer(mid) {
            Some(w) => w,
            None => return,
        };
        let pt = match writer
            .payload_params()
            .find(|pp| pp.spec().codec == Codec::Opus)
            .map(|pp| pp.pt())
        {
            Some(pt) => pt,
            None => return,
        };
        let rtp_time = MediaTime::new(ts, Frequency::FORTY_EIGHT_KHZ);
        if let Err(e) = writer.write(pt, Instant::now(), rtp_time, opus) {
            log::trace!("WebRTC: voice write to {} failed: {e}", short(&peer));
            return;
        }
        // Advance the 48 kHz RTP clock by one 20 ms frame (960 samples).
        p.voice_rtp_ts = ts.wrapping_add(960);
    }

    /// Apply a single GUI command (offer / answer-inbound-signal / send).
    fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::OfferTo { peer, wants_voice } => self.cmd_offer_to(peer, wants_voice),
            Command::SendText { peer, text } => self.cmd_send_text(peer, text),
            Command::SendVoice { peer, opus } => self.cmd_send_voice(peer, opus),
            Command::Signal { from, signal_type, data } => {
                self.cmd_signal(from, signal_type, data)
            }
        }
    }

    /// Begin an outgoing connection to `peer` (offerer side), honoring the
    /// glare-avoidance offerer rule.
    fn cmd_offer_to(&mut self, peer: String, wants_voice: bool) {
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

        // inc-3b: same for the TURN-relayed address — if the allocation already
        // completed, ride it inside this offer's SDP. (If allocated later,
        // apply_relayed_to_all_peers trickles it.)
        if let Some(relayed) = self.turn.as_ref().and_then(|t| t.relayed_addr()) {
            match Candidate::relayed(relayed, self.local_addr, Protocol::Udp) {
                Ok(cand) => {
                    rtc.add_local_candidate(cand);
                }
                Err(e) => log::warn!("WebRTC: bad relayed candidate {relayed} for offer: {e}"),
            }
        }

        // Create the ordered data channel. `add_channel` returns a ChannelId,
        // but that id is NOT writable yet — we must wait for Event::ChannelOpen
        // (str0m opens it after SCTP/DTLS come up). We store the real id then.
        let mut api = rtc.sdp_api();
        // Data channel FIRST so the application m-line stays at SDP index 0 (keeps
        // the existing ICE index assumptions valid; see emit_ice_candidate).
        let _cid = api.add_channel(CHANNEL_LABEL.to_string());
        // Voice (Phase B): add a SendRecv Opus audio m-line ONLY when this
        // connection is requested for voice. Opus (48 kHz) is enabled by default
        // in Rtc::builder(). Data-only connections (every existing caller) skip
        // this entirely, so their offer SDP is byte-identical to before.
        let audio_mid = if wants_voice {
            Some(api.add_media(MediaKind::Audio, Direction::SendRecv, None, None, None))
        } else {
            None
        };
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
                audio_mid,
                voice_rtp_ts: 0,
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

        // inc-3b: likewise add the TURN-relayed candidate if allocated, so it
        // rides in the SDP answer. Otherwise apply_relayed_to_all_peers trickles
        // it once the allocation completes.
        if let Some(relayed) = self.turn.as_ref().and_then(|t| t.relayed_addr()) {
            match Candidate::relayed(relayed, self.local_addr, Protocol::Udp) {
                Ok(cand) => {
                    rtc.add_local_candidate(cand);
                }
                Err(e) => log::warn!("WebRTC: bad relayed candidate {relayed} for answer: {e}"),
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
                // Voice (Phase B): accept_offer auto-mirrors any audio m-line the
                // offer carried; we learn its mid from Event::MediaAdded.
                audio_mid: None,
                voice_rtp_ts: 0,
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

        // inc-3b: proactively install a TURN permission + channel for this
        // remote candidate's address, so that IF str0m later picks the relayed
        // pair to reach this peer, ChannelData is already usable (no first-packet
        // drop while a permission/channel is created). No-op unless TURN is
        // allocated. This does NOT touch any Rtc — pure TURN signaling — so it's
        // exempt from str0m's mutation invariant.
        let cand_addr = cand.addr();
        if let Some(turn) = self.turn.as_mut() {
            if turn.relayed_addr().is_some() {
                turn.ensure_peer(&self.udp, cand_addr);
            }
        }

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

    // ════════════════════════════════════════════════════════════════════
    //  inc-3b — TURN relay client (RFC 5766)
    // ════════════════════════════════════════════════════════════════════

    /// Drive the TURN client state machine once per loop turn. Lazily resolves
    /// the TURN server + starts the Allocate, retries a failed/lost Allocate on
    /// a cadence, and refreshes a live allocation before LIFETIME expiry.
    ///
    /// The ONLY `Rtc` mutation that can happen here is trickling a freshly-learnt
    /// relayed candidate to existing peers (when the allocation first completes),
    /// which routes through `apply_relayed_to_all_peers` → `add_and_trickle_*`
    /// and is honored by the step-B drain. Everything else is plain TURN
    /// datagrams on the shared socket, exempt from str0m's mutation invariant.
    ///
    /// Best-effort: any failure (DNS, auth, server down) is logged and leaves
    /// host+srflx fully functional.
    fn maybe_drive_turn(&mut self) {
        // Lazily construct + resolve the TURN client. Like STUN, a transient DNS
        // failure at startup just means we try again next turn — never fatal.
        if self.turn.is_none() {
            match turn::TurnClient::resolve(TURN_SERVER) {
                Some(client) => {
                    log::info!("WebRTC: TURN server resolved to {}", client.server_addr());
                    crate::debug::push_debug(format!(
                        "WebRTC TURN server {}",
                        client.server_addr()
                    ));
                    self.turn = Some(client);
                }
                None => return, // couldn't resolve yet — retry next loop turn.
            }
        }

        // Pull the socket out by reference; TURN sends on the same shared socket.
        // `drive` advances the allocate/refresh state machine and may emit a
        // freshly-learnt relayed address.
        let newly_allocated = {
            let turn = self.turn.as_mut().expect("turn is Some after init above");
            turn.drive(&self.udp)
        };

        // If we JUST learned the relayed address, install a relayed candidate on
        // every existing peer and trickle it (peers created later pick it up in
        // their SDP / via cmd_offer_to + on_offer).
        if let Some(relayed) = newly_allocated {
            log::info!("WebRTC: TURN allocation succeeded, relayed addr {relayed}");
            crate::debug::push_debug(format!("WebRTC TURN relayed = {relayed}"));
            self.apply_relayed_to_all_peers(relayed);
            // Ensure a permission + channel for every peer address we already
            // know about, so relayed sends to them can use ChannelData promptly.
            let known: Vec<SocketAddr> =
                self.peers.values().filter_map(|p| p.remote_addr).collect();
            if let Some(turn) = self.turn.as_mut() {
                for addr in known {
                    turn.ensure_peer(&self.udp, addr);
                }
            }
        }
    }

    /// Try to interpret an inbound datagram as TURN traffic from the TURN server.
    ///
    /// The address guard (`source == turn_server_addr`) is the ENTIRE basis for
    /// isolation: only datagrams literally from the TURN server are considered
    /// here, so a peer's host/srflx datagram (sourced from the peer) can never be
    /// mistaken for TURN and always falls through to the existing demux.
    ///
    /// Returns:
    /// * `TurnRecv::NotTurn` — not from the TURN server; caller falls through.
    /// * `TurnRecv::Handled` — a TURN control message (Allocate/Refresh/
    ///   CreatePermission/ChannelBind reply, or an indication we can't map to a
    ///   peer); fully consumed, caller `continue`s.
    /// * `TurnRecv::Relayed { peer, start, len }` — relayed peer data; the inner
    ///   datagram lives at `buf[start..start+len]` and must be fed to str0m with
    ///   `source = peer`, `destination = our_relayed_addr`.
    fn try_handle_turn(&mut self, source: SocketAddr, datagram: &[u8]) -> TurnRecv {
        // We need a base offset to translate the inner-datagram slice (which the
        // TURN client returns as a sub-slice of `datagram`) back into an index
        // range within the caller's `buf`. Since `datagram` IS `&buf[..n]`, the
        // inner slice's offset within `buf` equals its offset within `datagram`.
        let turn = match self.turn.as_mut() {
            Some(t) => t,
            None => return TurnRecv::NotTurn,
        };
        if source != turn.server_addr() {
            return TurnRecv::NotTurn;
        }
        match turn.handle_from_server(&self.udp, datagram) {
            turn::TurnInbound::Control => TurnRecv::Handled,
            turn::TurnInbound::Data { peer, inner_offset, inner_len } => {
                TurnRecv::Relayed { peer, start: inner_offset, len: inner_len }
            }
        }
    }

    /// Add the relayed candidate to every currently-live peer's `Rtc` and trickle
    /// it. Used when the TURN allocation completes after peers already exist.
    /// Mirrors `apply_srflx_to_all_peers`.
    fn apply_relayed_to_all_peers(&mut self, relayed: SocketAddr) {
        let keys: Vec<String> = self.peers.keys().cloned().collect();
        for key in keys {
            self.add_and_trickle_relayed(&key, relayed);
        }
    }

    /// Add the relayed candidate to ONE peer's `Rtc` (if alive) and trickle the
    /// candidate line to that peer. Mirrors `add_and_trickle_srflx`.
    ///
    /// `Candidate::relayed(addr, local, proto)`: `addr` = the TURN-allocated
    /// relayed transport address (what the peer sends to / what str0m stamps as
    /// the Transmit source for this pair), `local` = our local socket address
    /// (the interface we use to reach the TURN server). Per the `is` crate the
    /// relayed ctor sets BOTH `addr` and `base` to `addr`, which is exactly why
    /// the Transmit-wrap guard keys on `t.source == relayed_addr`.
    fn add_and_trickle_relayed(&mut self, peer_key: &str, relayed: SocketAddr) {
        let cand = match Candidate::relayed(relayed, self.local_addr, Protocol::Udp) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "WebRTC: bad relayed candidate {relayed} (local {}): {e}",
                    self.local_addr
                );
                return;
            }
        };
        // Serialize the SDP line BEFORE moving the candidate (str0m never emits
        // added local candidates back, so we trickle it ourselves — same as the
        // srflx).
        let sdp_line = cand.to_sdp_string();
        match self.peers.get_mut(peer_key) {
            Some(p) if p.rtc.is_alive() => {
                p.rtc.add_local_candidate(cand);
            }
            _ => return, // peer gone/dead — don't trickle a candidate nobody uses.
        }
        self.emit_ice_candidate(peer_key, &sdp_line);
        log::debug!("WebRTC: trickled relayed candidate to {}", short(peer_key));
    }
}

/// Result of the inc-3b TURN recv demux (`try_handle_turn`).
enum TurnRecv {
    /// Not from the TURN server — fall through to the existing per-peer demux.
    NotTurn,
    /// A TURN control message, fully handled. Caller skips this datagram.
    Handled,
    /// Relayed peer data: the inner datagram is `buf[start..start+len]` and must
    /// be fed to str0m with `source = peer`, `destination = our_relayed_addr`.
    Relayed {
        peer: SocketAddr,
        start: usize,
        len: usize,
    },
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

// ════════════════════════════════════════════════════════════════════════
//  inc-3b — TURN client (RFC 5766), long-term-credential auth
// ════════════════════════════════════════════════════════════════════════
//
// A minimal, hand-rolled TURN client — JUST enough to allocate a relay, keep it
// alive, install peer permissions/channels, and relay datagrams. No external
// TURN crate. It reuses the STUN message framing (TURN messages ARE STUN
// messages with TURN method codes) but is otherwise self-contained.
//
// # STUN message-type bit layout (RFC 5389 §6) — needed for TURN methods
//
// The 14-bit "message type" interleaves the 12-bit METHOD and the 2-bit CLASS:
//
//     0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5   (bit, MSB first; top 2 are always 0)
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |0 0|M M M M M|C|M M M|C|M M M M|
//    |   |1 1 1 1 1|1|6 5 4|0|3 2 1 0|
//    |   |1 0 9 8 7| |     | |       |
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//
// So CLASS bit C1 lands at bit position 8 (value 0x0100) and C0 at position 4
// (value 0x0010); the method bits fill the rest. Classes: Request=0b00,
// Indication=0b01, Success=0b10, Error=0b11. `message_type(method, class)`
// below computes this; e.g. Allocate(0x003)+Request = 0x0003, Allocate+Success
// = 0x0103, Allocate+Error = 0x0113.
//
// # ChannelData framing (RFC 5766 §11.4)
//
//     0                   1                   2                   3
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |         Channel Number        |            Length             |
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//    |                                                               |
//    /                       Application Data                        /
//    /                                                               /
//    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//
// Channel numbers are 0x4000..=0x7FFF. Because a STUN message's first two bits
// are 0 (types ≤ 0x3FFF → first byte 0x00..0x3F), a ChannelData frame (first
// byte 0x40..0x7F) is trivially distinguishable from a STUN message on the wire.
//
// # Long-term credential auth (RFC 5389 §10.2, RFC 5766 §4)
//
// First Allocate (no auth) → server replies 401 Unauthorized with REALM + NONCE.
// We retry Allocate adding USERNAME, REALM, NONCE, and MESSAGE-INTEGRITY.
//   key   = MD5( username ":" realm ":" password )                  (16 bytes)
//   M-I   = HMAC-SHA1( key, message[0 .. start-of-MESSAGE-INTEGRITY] )
// where the message-length field (bytes 2..4) is FIRST set to the value it will
// have *including* the 24-byte MESSAGE-INTEGRITY attribute, but the bytes hashed
// STOP right before the MESSAGE-INTEGRITY attribute's own TLV. See
// `append_message_integrity` for the exact byte ranges.
mod turn {
    use super::stun::MAGIC_COOKIE;
    use std::collections::HashMap;
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
    use std::time::{Duration, Instant};

    // ── TURN method codes (RFC 5766 §13) ──
    const METHOD_ALLOCATE: u16 = 0x003;
    const METHOD_REFRESH: u16 = 0x004;
    const METHOD_SEND: u16 = 0x006;
    const METHOD_DATA: u16 = 0x007;
    const METHOD_CREATE_PERMISSION: u16 = 0x008;
    const METHOD_CHANNEL_BIND: u16 = 0x009;

    // ── STUN message classes (the 2-bit CLASS field) ──
    const CLASS_REQUEST: u16 = 0b00;
    const CLASS_INDICATION: u16 = 0b01;
    const CLASS_SUCCESS: u16 = 0b10;
    const CLASS_ERROR: u16 = 0b11;

    // ── Attribute types (RFC 5389 §18.2 + RFC 5766 §14) ──
    const ATTR_USERNAME: u16 = 0x0006;
    const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
    const ATTR_ERROR_CODE: u16 = 0x0009;
    const ATTR_CHANNEL_NUMBER: u16 = 0x000C;
    const ATTR_LIFETIME: u16 = 0x000D;
    const ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
    const ATTR_DATA: u16 = 0x0013;
    const ATTR_REALM: u16 = 0x0014;
    const ATTR_NONCE: u16 = 0x0015;
    const ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
    const ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;

    // ── Misc constants ──
    /// REQUESTED-TRANSPORT value for UDP (protocol 17 = 0x11) in the top byte.
    const REQUESTED_TRANSPORT_UDP: u32 = 0x1100_0000;
    /// Default allocation lifetime we request (seconds). The server may shorten
    /// it; we honor the value it returns.
    const DEFAULT_LIFETIME_SECS: u32 = 600;
    /// First channel number to hand out (RFC 5766 §11: 0x4000..=0x7FFF).
    const FIRST_CHANNEL: u16 = 0x4000;

    /// Compute the 14-bit STUN message type from a method + class. See the
    /// module header for the bit interleaving.
    fn message_type(method: u16, class: u16) -> u16 {
        // Method bits split at the class-bit positions (8 and 4).
        let m_low = method & 0x000F; // M3..M0  → bits 3..0
        let m_mid = (method >> 4) & 0x0007; // M6..M4 → bits 6..4 (shifted up by 1 for C0)
        let m_high = (method >> 7) & 0x001F; // M11..M7 → bits 13..9 (shifted up by 1 for C1)
        let c0 = class & 0b01; // → bit 4
        let c1 = (class >> 1) & 0b01; // → bit 8
        (m_high << 9) | (c1 << 8) | (m_mid << 5) | (c0 << 4) | m_low
    }

    /// Generate a random 96-bit TURN/STUN transaction id (reuses the same RNG
    /// idiom as `mod stun`).
    fn random_txid() -> [u8; 12] {
        use rand::RngCore;
        let mut id = [0u8; 12];
        rand::rng().fill_bytes(&mut id);
        id
    }

    /// Per-peer relay state: the channel number we bound (once confirmed) and
    /// whether a permission/channel request is in flight.
    struct PeerChannel {
        /// The channel number assigned to this peer (0x4000..=0x7FFF).
        channel: u16,
        /// True once the ChannelBind success response arrived — only then may we
        /// send compact ChannelData; before that we use Send indications.
        bound: bool,
        /// When we last (re)sent the ChannelBind, to refresh it before its 600s
        /// lifetime and to retry if the success response was lost.
        last_bind: Option<Instant>,
    }

    /// The TURN allocation lifecycle.
    #[derive(PartialEq)]
    enum Phase {
        /// No allocation yet; (re)send an unauthenticated Allocate to learn the
        /// REALM/NONCE (or, if we already have them, an authenticated Allocate).
        Allocating,
        /// We hold a live allocation (relayed address + lifetime).
        Allocated,
        /// A permanent failure (e.g. auth rejected with a non-recoverable code).
        /// We stop trying for this session; host+srflx still work.
        Failed,
    }

    /// A minimal RFC 5766 TURN client driven from the WebRTC run-loop. Sends on
    /// the manager's shared `UdpSocket`; never owns the socket.
    pub struct TurnClient {
        /// Resolved TURN server UDP address.
        server: SocketAddr,
        phase: Phase,
        /// REALM from the 401 challenge (needed for the auth key + the attribute).
        realm: Option<String>,
        /// NONCE from the 401 challenge (echoed in every authed request).
        nonce: Option<String>,
        /// The relayed transport address (XOR-RELAYED-ADDRESS) once allocated.
        relayed: Option<SocketAddr>,
        /// Allocation lifetime the server granted, and when we allocated, so we
        /// know when to Refresh.
        lifetime: Duration,
        allocated_at: Option<Instant>,
        /// When we last sent an Allocate (retry cadence while Allocating).
        last_allocate: Option<Instant>,
        /// When we last sent a Refresh (rate-limit so we don't spam refreshes
        /// while waiting for the success response to reset `allocated_at`).
        last_refresh: Option<Instant>,
        /// Per-peer channel/permission state, keyed by the peer's socket address.
        peers: HashMap<SocketAddr, PeerChannel>,
        /// Next channel number to assign.
        next_channel: u16,
    }

    /// What an inbound datagram from the TURN server turned out to be.
    pub enum TurnInbound {
        /// A TURN control message (Allocate/Refresh/CreatePermission/ChannelBind
        /// response, or an indication we can't map). Fully handled.
        Control,
        /// Relayed peer data. The inner application datagram is at
        /// `buf[inner_offset .. inner_offset + inner_len]` (a sub-slice of the
        /// datagram passed to `handle_from_server`), from peer `peer`.
        Data {
            peer: SocketAddr,
            inner_offset: usize,
            inner_len: usize,
        },
    }

    impl TurnClient {
        /// Resolve the TURN server hostname to a UDP `SocketAddr`. Returns `None`
        /// (caller retries) on DNS failure or no IPv4 result. We prefer IPv4 so
        /// the relayed candidate's `local` base (our IPv4 socket) matches family.
        pub fn resolve(host: &str) -> Option<TurnClient> {
            let addr = host
                .to_socket_addrs()
                .ok()?
                .find(|a| a.is_ipv4())?;
            Some(TurnClient {
                server: addr,
                phase: Phase::Allocating,
                realm: None,
                nonce: None,
                relayed: None,
                lifetime: Duration::from_secs(DEFAULT_LIFETIME_SECS as u64),
                allocated_at: None,
                last_allocate: None,
                last_refresh: None,
                peers: HashMap::new(),
                next_channel: FIRST_CHANNEL,
            })
        }

        /// The TURN server's address (for the recv-path source guard).
        pub fn server_addr(&self) -> SocketAddr {
            self.server
        }

        /// The relayed transport address, once allocated. `None` otherwise. This
        /// is the value the Transmit-wrap guard compares `t.source` against.
        pub fn relayed_addr(&self) -> Option<SocketAddr> {
            self.relayed
        }

        /// The next instant the run-loop should wake us to do TURN work (retry an
        /// Allocate, or Refresh the allocation). `None` if there's nothing
        /// pending (e.g. Failed).
        pub fn next_deadline(&self) -> Option<Instant> {
            match self.phase {
                Phase::Allocating => {
                    // Wake to retry the Allocate.
                    Some(
                        self.last_allocate
                            .map(|t| t + super::TURN_ALLOC_RETRY_INTERVAL)
                            .unwrap_or_else(Instant::now),
                    )
                }
                Phase::Allocated => {
                    // Wake to refresh before the lifetime elapses.
                    let at = self.allocated_at?;
                    let refresh_in = self
                        .lifetime
                        .saturating_sub(super::TURN_REFRESH_SLACK)
                        // Never schedule a refresh more often than every 30s, and
                        // never less than 10s out, to avoid pathological churn if
                        // the server hands back a tiny lifetime.
                        .max(Duration::from_secs(30));
                    Some(at + refresh_in)
                }
                Phase::Failed => None,
            }
        }

        /// Advance the allocate/refresh state machine. Returns `Some(relayed)`
        /// the FIRST loop turn the allocation becomes live, so the manager can
        /// trickle the relayed candidate. Best-effort; never panics on the wire.
        pub fn drive(&mut self, udp: &UdpSocket) -> Option<SocketAddr> {
            match self.phase {
                Phase::Failed => None,
                Phase::Allocating => {
                    // Rate-limit Allocate retries.
                    let now = Instant::now();
                    if let Some(last) = self.last_allocate {
                        if now.duration_since(last) < super::TURN_ALLOC_RETRY_INTERVAL {
                            return None;
                        }
                    }
                    self.send_allocate(udp);
                    None
                }
                Phase::Allocated => {
                    // Refresh if we're inside the slack window before expiry. In
                    // `Allocated` phase `allocated_at` is always Some. We
                    // rate-limit with `last_refresh` so we send AT MOST one
                    // Refresh per retry interval — otherwise, between sending the
                    // Refresh and its success response landing (which resets
                    // `allocated_at`), every ~50ms loop turn would re-fire it.
                    if let Some(at) = self.allocated_at {
                        let refresh_at =
                            self.lifetime.saturating_sub(super::TURN_REFRESH_SLACK);
                        let due = at.elapsed() >= refresh_at;
                        let throttled = self
                            .last_refresh
                            .map(|t| t.elapsed() < super::TURN_ALLOC_RETRY_INTERVAL)
                            .unwrap_or(false);
                        if due && !throttled {
                            self.send_refresh(udp);
                            self.last_refresh = Some(Instant::now());
                        }
                    }
                    // Also (re)bind any channels whose bind is stale / unconfirmed.
                    self.refresh_channels(udp);
                    None
                }
            }
        }

        /// Ensure we have a permission (and a channel) for `peer`. Installs a
        /// CreatePermission + ChannelBind if this peer is new or unconfirmed.
        /// No-op if we have no live allocation. Idempotent.
        pub fn ensure_peer(&mut self, udp: &UdpSocket, peer: SocketAddr) {
            if self.phase != Phase::Allocated {
                return;
            }
            // TURN only relays IPv4↔IPv4 here (our allocation is IPv4). Skip
            // non-IPv4 peer candidates — they can't ride this relay anyway.
            if !peer.is_ipv4() {
                return;
            }
            if !self.peers.contains_key(&peer) {
                let channel = self.next_channel;
                // Advance, wrapping within the valid 0x4000..=0x7FFF window.
                self.next_channel = if self.next_channel >= 0x7FFE {
                    FIRST_CHANNEL
                } else {
                    self.next_channel + 1
                };
                self.peers.insert(
                    peer,
                    PeerChannel {
                        channel,
                        bound: false,
                        last_bind: None,
                    },
                );
                // Install the permission first, then bind the channel. (A channel
                // bind also installs a permission per RFC 5766 §11.1, but sending
                // CreatePermission explicitly is harmless and matches common
                // client behavior.)
                self.send_create_permission(udp, peer);
                self.send_channel_bind(udp, peer);
            }
        }

        /// Send `data` to `peer` through the relay. Uses compact ChannelData once
        /// the channel is confirmed bound; otherwise falls back to a Send
        /// indication (which works immediately, before the bind round-trip
        /// completes). Best-effort — a failure is logged, never fatal.
        pub fn send_relayed(&mut self, udp: &UdpSocket, peer: SocketAddr, data: &[u8]) {
            if self.phase != Phase::Allocated {
                // No allocation — we should never be asked to relay, but guard.
                return;
            }
            // Make sure a permission/channel exists (covers the case where str0m
            // chose the relayed pair for a peer we hadn't pre-registered).
            self.ensure_peer(udp, peer);

            let bound_channel = self.peers.get(&peer).filter(|p| p.bound).map(|p| p.channel);

            if let Some(channel) = bound_channel {
                // ── ChannelData: 4-byte header + raw data, sent to the server. ──
                let mut frame = Vec::with_capacity(4 + data.len());
                frame.extend_from_slice(&channel.to_be_bytes());
                frame.extend_from_slice(&(data.len() as u16).to_be_bytes());
                frame.extend_from_slice(data);
                if let Err(e) = udp.send_to(&frame, self.server) {
                    log::trace!("WebRTC TURN: ChannelData send to {} failed: {e}", self.server);
                }
            } else {
                // ── Send indication (bootstrap path before the channel binds). ──
                let msg = self.build_send_indication(peer, data);
                if let Err(e) = udp.send_to(&msg, self.server) {
                    log::trace!("WebRTC TURN: Send indication to {} failed: {e}", self.server);
                }
            }
        }

        /// Handle a datagram that arrived FROM the TURN server. Classifies it as
        /// relayed data (ChannelData / Data indication) or a control response.
        pub fn handle_from_server(&mut self, udp: &UdpSocket, datagram: &[u8]) -> TurnInbound {
            // ChannelData? First byte 0x40..0x7F ⇒ channel number 0x4000..0x7FFF.
            if !datagram.is_empty() && (0x40..=0x7F).contains(&datagram[0]) {
                return self.handle_channel_data(datagram);
            }
            // Otherwise it's a STUN-framed TURN message. Parse the header.
            if datagram.len() < 20 {
                return TurnInbound::Control;
            }
            let msg_type = u16::from_be_bytes([datagram[0], datagram[1]]);
            let cookie = u32::from_be_bytes([datagram[4], datagram[5], datagram[6], datagram[7]]);
            if cookie != MAGIC_COOKIE {
                return TurnInbound::Control; // not a STUN/TURN message we recognize
            }

            // Match well-known response/indication types.
            if msg_type == message_type(METHOD_DATA, CLASS_INDICATION) {
                return self.handle_data_indication(datagram);
            }
            if msg_type == message_type(METHOD_ALLOCATE, CLASS_SUCCESS) {
                self.handle_allocate_success(datagram);
            } else if msg_type == message_type(METHOD_ALLOCATE, CLASS_ERROR) {
                self.handle_allocate_error(udp, datagram);
            } else if msg_type == message_type(METHOD_REFRESH, CLASS_SUCCESS) {
                self.handle_refresh_success(datagram);
            } else if msg_type == message_type(METHOD_REFRESH, CLASS_ERROR) {
                // A 438 (stale nonce) on refresh: re-read nonce and let the next
                // drive() retry. Any other error: drop the allocation back to
                // Allocating so we re-establish.
                self.handle_stale_nonce_or_reset(datagram);
            } else if msg_type == message_type(METHOD_CHANNEL_BIND, CLASS_SUCCESS) {
                self.handle_channel_bind_success(datagram);
            } else if msg_type == message_type(METHOD_CREATE_PERMISSION, CLASS_SUCCESS) {
                self.handle_create_permission_success(datagram);
            } else {
                // CreatePermission/ChannelBind errors, or anything else — log at
                // trace and ignore; the periodic refresh_channels retries binds.
                log::trace!("WebRTC TURN: unhandled server msg type 0x{msg_type:04x}");
            }
            TurnInbound::Control
        }

        // ── Outbound message builders / senders ──────────────────────────────

        /// Send an Allocate Request. Unauthenticated if we don't yet hold a
        /// REALM/NONCE; authenticated (USERNAME/REALM/NONCE/MESSAGE-INTEGRITY)
        /// once we do.
        fn send_allocate(&mut self, udp: &UdpSocket) {
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_ALLOCATE, CLASS_REQUEST), &txid);
            // REQUESTED-TRANSPORT = UDP (mandatory for Allocate).
            append_attr(&mut msg, ATTR_REQUESTED_TRANSPORT, &REQUESTED_TRANSPORT_UDP.to_be_bytes());
            // LIFETIME (optional hint).
            append_attr(&mut msg, ATTR_LIFETIME, &DEFAULT_LIFETIME_SECS.to_be_bytes());

            if self.have_credentials() {
                self.append_auth(&mut msg);
            }
            finalize_length(&mut msg);
            if let Err(e) = udp.send_to(&msg, self.server) {
                log::debug!("WebRTC TURN: Allocate send failed: {e}");
            } else {
                log::trace!("WebRTC TURN: sent Allocate ({}auth)", if self.have_credentials() { "" } else { "no-" });
            }
            self.last_allocate = Some(Instant::now());
        }

        /// Send a Refresh Request with the requested lifetime (authenticated).
        fn send_refresh(&mut self, udp: &UdpSocket) {
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_REFRESH, CLASS_REQUEST), &txid);
            append_attr(&mut msg, ATTR_LIFETIME, &DEFAULT_LIFETIME_SECS.to_be_bytes());
            self.append_auth(&mut msg);
            finalize_length(&mut msg);
            if let Err(e) = udp.send_to(&msg, self.server) {
                log::debug!("WebRTC TURN: Refresh send failed: {e}");
            }
        }

        /// Send a CreatePermission Request for `peer`'s IP (authenticated).
        fn send_create_permission(&mut self, udp: &UdpSocket, peer: SocketAddr) {
            let txid = random_txid();
            let mut msg =
                begin_message(message_type(METHOD_CREATE_PERMISSION, CLASS_REQUEST), &txid);
            append_xor_peer_address(&mut msg, peer, &txid);
            self.append_auth(&mut msg);
            finalize_length(&mut msg);
            let _ = udp.send_to(&msg, self.server);
        }

        /// Send a ChannelBind Request binding `peer` to its channel number
        /// (authenticated).
        fn send_channel_bind(&mut self, udp: &UdpSocket, peer: SocketAddr) {
            let channel = match self.peers.get(&peer) {
                Some(p) => p.channel,
                None => return,
            };
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_CHANNEL_BIND, CLASS_REQUEST), &txid);
            // CHANNEL-NUMBER: 2-byte channel + 2 reserved bytes (RFC 5766 §14.1).
            let mut chan_val = [0u8; 4];
            chan_val[0..2].copy_from_slice(&channel.to_be_bytes());
            append_attr(&mut msg, ATTR_CHANNEL_NUMBER, &chan_val);
            append_xor_peer_address(&mut msg, peer, &txid);
            self.append_auth(&mut msg);
            finalize_length(&mut msg);
            let _ = udp.send_to(&msg, self.server);
            if let Some(p) = self.peers.get_mut(&peer) {
                p.last_bind = Some(Instant::now());
            }
        }

        /// Build a Send indication wrapping `data` destined for `peer`. Send
        /// indications are NOT authenticated (RFC 5766 §10) — they carry only
        /// XOR-PEER-ADDRESS + DATA.
        fn build_send_indication(&self, peer: SocketAddr, data: &[u8]) -> Vec<u8> {
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_SEND, CLASS_INDICATION), &txid);
            append_xor_peer_address(&mut msg, peer, &txid);
            append_attr(&mut msg, ATTR_DATA, data);
            finalize_length(&mut msg);
            msg
        }

        /// (Re)send ChannelBind for any peer whose bind is unconfirmed or stale
        /// (channel binds expire after 600s; we refresh well before that).
        fn refresh_channels(&mut self, udp: &UdpSocket) {
            let now = Instant::now();
            let stale: Vec<SocketAddr> = self
                .peers
                .iter()
                .filter(|(_, p)| match p.last_bind {
                    None => true,
                    Some(t) => {
                        // Unconfirmed → retry every few seconds; confirmed →
                        // refresh ~60s before the 600s channel lifetime.
                        let interval = if p.bound {
                            Duration::from_secs(600).saturating_sub(super::TURN_REFRESH_SLACK)
                        } else {
                            super::TURN_ALLOC_RETRY_INTERVAL
                        };
                        now.duration_since(t) >= interval
                    }
                })
                .map(|(addr, _)| *addr)
                .collect();
            for addr in stale {
                self.send_channel_bind(udp, addr);
            }
        }

        // ── Inbound response handlers ────────────────────────────────────────

        fn handle_allocate_success(&mut self, datagram: &[u8]) {
            // Pull XOR-RELAYED-ADDRESS + LIFETIME from the attributes.
            let mut relayed = None;
            let mut lifetime = None;
            for (atype, val) in iter_attrs(datagram) {
                match atype {
                    ATTR_XOR_RELAYED_ADDRESS => {
                        relayed = parse_xor_address(val, datagram);
                    }
                    ATTR_LIFETIME if val.len() >= 4 => {
                        lifetime = Some(u32::from_be_bytes([val[0], val[1], val[2], val[3]]));
                    }
                    _ => {}
                }
            }
            if let Some(addr) = relayed {
                self.relayed = Some(addr);
                self.phase = Phase::Allocated;
                self.allocated_at = Some(Instant::now());
                if let Some(lt) = lifetime {
                    self.lifetime = Duration::from_secs(lt as u64);
                }
                log::info!("WebRTC TURN: allocated relay {addr}, lifetime {:?}", self.lifetime);
            } else {
                log::debug!("WebRTC TURN: Allocate success lacked XOR-RELAYED-ADDRESS");
            }
        }

        fn handle_allocate_error(&mut self, _udp: &UdpSocket, datagram: &[u8]) {
            let (code, realm, nonce) = parse_error_challenge(datagram);
            match code {
                Some(401) => {
                    // Unauthorized — capture REALM/NONCE so the next drive()
                    // retries the Allocate WITH credentials.
                    if realm.is_some() {
                        self.realm = realm;
                    }
                    if nonce.is_some() {
                        self.nonce = nonce;
                    }
                    // Force an immediate retry on the next drive() turn.
                    self.last_allocate = None;
                    log::trace!("WebRTC TURN: Allocate 401, captured realm/nonce, will retry authed");
                }
                Some(438) => {
                    // Stale nonce — refresh nonce and retry.
                    if nonce.is_some() {
                        self.nonce = nonce;
                    }
                    self.last_allocate = None;
                }
                other => {
                    // Any other Allocate error (e.g. 400/441/486/508 or a 401
                    // we've already retried) — treat as non-recoverable for this
                    // session. host+srflx remain fully functional.
                    log::warn!(
                        "WebRTC TURN: Allocate failed (error {:?}); continuing without relay",
                        other
                    );
                    self.phase = Phase::Failed;
                }
            }
        }

        fn handle_refresh_success(&mut self, datagram: &[u8]) {
            for (atype, val) in iter_attrs(datagram) {
                if atype == ATTR_LIFETIME && val.len() >= 4 {
                    let lt = u32::from_be_bytes([val[0], val[1], val[2], val[3]]);
                    self.lifetime = Duration::from_secs(lt as u64);
                }
            }
            self.allocated_at = Some(Instant::now());
            log::trace!("WebRTC TURN: allocation refreshed, lifetime {:?}", self.lifetime);
        }

        fn handle_stale_nonce_or_reset(&mut self, datagram: &[u8]) {
            let (code, _realm, nonce) = parse_error_challenge(datagram);
            if code == Some(438) {
                if nonce.is_some() {
                    self.nonce = nonce;
                }
                log::trace!("WebRTC TURN: refresh stale-nonce (438), nonce refreshed");
            } else {
                // The allocation may be gone — drop back to Allocating to rebuild.
                log::debug!("WebRTC TURN: refresh error {:?}, re-allocating", code);
                self.phase = Phase::Allocating;
                self.relayed = None;
                self.allocated_at = None;
                self.last_allocate = None;
                self.last_refresh = None;
                self.peers.clear();
            }
        }

        fn handle_channel_bind_success(&mut self, _datagram: &[u8]) {
            // The success response doesn't echo the channel number; mark the most
            // recently-bound unconfirmed peer as bound. Since binds are issued one
            // at a time per peer and quickly confirmed, marking all in-flight
            // (unbound, recently-sent) peers as bound is safe and converges.
            let now = Instant::now();
            for p in self.peers.values_mut() {
                if !p.bound {
                    if let Some(t) = p.last_bind {
                        // Confirm any bind we sent in the last few seconds.
                        if now.duration_since(t) < super::TURN_ALLOC_RETRY_INTERVAL {
                            p.bound = true; // a channel bind also implies a permission
                        }
                    }
                }
            }
            log::trace!("WebRTC TURN: ChannelBind success");
        }

        fn handle_create_permission_success(&mut self, _datagram: &[u8]) {
            // Informational: the permission is installed server-side. We gate
            // ChannelData readiness on the ChannelBind success (which also
            // implies a permission), so there's no per-peer flag to flip here.
            log::trace!("WebRTC TURN: CreatePermission success");
        }

        fn handle_data_indication(&mut self, datagram: &[u8]) -> TurnInbound {
            // A Data indication carries XOR-PEER-ADDRESS + DATA. We surface the
            // DATA sub-slice (by offset within `datagram`) and the peer address.
            let mut peer = None;
            let mut data_range = None;
            for (atype, off, len) in iter_attrs_with_offsets(datagram) {
                match atype {
                    ATTR_XOR_PEER_ADDRESS => {
                        peer = parse_xor_address(&datagram[off..off + len], datagram);
                    }
                    ATTR_DATA => {
                        data_range = Some((off, len));
                    }
                    _ => {}
                }
            }
            match (peer, data_range) {
                (Some(peer), Some((off, len))) => TurnInbound::Data {
                    peer,
                    inner_offset: off,
                    inner_len: len,
                },
                _ => TurnInbound::Control,
            }
        }

        fn handle_channel_data(&mut self, datagram: &[u8]) -> TurnInbound {
            // 4-byte header: channel(2) + length(2), then `length` bytes of data.
            if datagram.len() < 4 {
                return TurnInbound::Control;
            }
            let channel = u16::from_be_bytes([datagram[0], datagram[1]]);
            let len = u16::from_be_bytes([datagram[2], datagram[3]]) as usize;
            if 4 + len > datagram.len() {
                return TurnInbound::Control; // truncated frame
            }
            // Map the channel number back to the peer address.
            let peer = self
                .peers
                .iter()
                .find(|(_, p)| p.channel == channel)
                .map(|(addr, _)| *addr);
            match peer {
                Some(peer) => TurnInbound::Data {
                    peer,
                    inner_offset: 4,
                    inner_len: len,
                },
                None => {
                    log::trace!("WebRTC TURN: ChannelData for unknown channel 0x{channel:04x}");
                    TurnInbound::Control
                }
            }
        }

        // ── Auth helpers ─────────────────────────────────────────────────────

        fn have_credentials(&self) -> bool {
            self.realm.is_some() && self.nonce.is_some()
        }

        /// Append USERNAME, REALM, NONCE, then MESSAGE-INTEGRITY (in that order)
        /// to an in-progress message. MESSAGE-INTEGRITY MUST be last (it covers
        /// everything before it).
        fn append_auth(&self, msg: &mut Vec<u8>) {
            let (realm, nonce) = match (&self.realm, &self.nonce) {
                (Some(r), Some(n)) => (r, n),
                _ => return, // no credentials yet — caller checked have_credentials
            };
            append_attr(msg, ATTR_USERNAME, super::TURN_USERNAME.as_bytes());
            append_attr(msg, ATTR_REALM, realm.as_bytes());
            append_attr(msg, ATTR_NONCE, nonce.as_bytes());
            let key = long_term_key(super::TURN_USERNAME, realm, super::TURN_PASSWORD);
            append_message_integrity(msg, &key);
        }
    }

    // ── Free helpers: message framing, attributes, XOR addresses ─────────────

    /// Begin a STUN/TURN message: 20-byte header with a placeholder length of 0
    /// (filled in by `finalize_length`).
    fn begin_message(msg_type: u16, txid: &[u8; 12]) -> Vec<u8> {
        let mut msg = Vec::with_capacity(64);
        msg.extend_from_slice(&msg_type.to_be_bytes());
        msg.extend_from_slice(&0u16.to_be_bytes()); // length placeholder
        msg.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        msg.extend_from_slice(txid);
        msg
    }

    /// Append one TLV attribute (type, length, value) with padding to a 4-byte
    /// boundary, and DOES NOT touch the header length (that's `finalize_length`).
    fn append_attr(msg: &mut Vec<u8>, atype: u16, value: &[u8]) {
        msg.extend_from_slice(&atype.to_be_bytes());
        msg.extend_from_slice(&(value.len() as u16).to_be_bytes());
        msg.extend_from_slice(value);
        // Pad to 4-byte boundary with zeros.
        let pad = (4 - (value.len() % 4)) % 4;
        for _ in 0..pad {
            msg.push(0);
        }
    }

    /// Set the header's message-length field (bytes 2..4) to the current
    /// attribute-region length (everything after the 20-byte header).
    fn finalize_length(msg: &mut [u8]) {
        let attr_len = (msg.len() - 20) as u16;
        msg[2..4].copy_from_slice(&attr_len.to_be_bytes());
    }

    /// Append an XOR-PEER-ADDRESS (or any XOR-address) attribute for `addr`,
    /// XOR-encoded per RFC 5389 §15.2 (IPv4): family(1, after 1 reserved),
    /// X-Port = port ^ (cookie>>16), X-Address = addr ^ cookie.
    fn append_xor_peer_address(msg: &mut Vec<u8>, addr: SocketAddr, _txid: &[u8; 12]) {
        let v4 = match addr {
            SocketAddr::V4(v4) => v4,
            // IPv6 peers aren't relayed through our IPv4 allocation; callers
            // guard against this, but encode a zeroed v4 defensively if reached.
            SocketAddr::V6(_) => SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0),
        };
        let x_port = v4.port() ^ ((MAGIC_COOKIE >> 16) as u16);
        let x_addr = u32::from(*v4.ip()) ^ MAGIC_COOKIE;
        let mut val = [0u8; 8];
        val[0] = 0x00; // reserved
        val[1] = 0x01; // family = IPv4
        val[2..4].copy_from_slice(&x_port.to_be_bytes());
        val[4..8].copy_from_slice(&x_addr.to_be_bytes());
        append_attr(msg, ATTR_XOR_PEER_ADDRESS, &val);
    }

    /// Parse an XOR-MAPPED/RELAYED/PEER-ADDRESS attribute value (IPv4 only).
    /// `_full` is the whole message (unused for IPv4, since the XOR uses only the
    /// magic cookie; it would be needed for the IPv6 txid XOR which we don't do).
    fn parse_xor_address(val: &[u8], _full: &[u8]) -> Option<SocketAddr> {
        if val.len() < 8 || val[1] != 0x01 {
            return None; // need IPv4 family + 8 bytes
        }
        let x_port = u16::from_be_bytes([val[2], val[3]]);
        let port = x_port ^ ((MAGIC_COOKIE >> 16) as u16);
        let x_addr = u32::from_be_bytes([val[4], val[5], val[6], val[7]]);
        let ip = Ipv4Addr::from(x_addr ^ MAGIC_COOKIE);
        Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
    }

    /// Parse an error response's ERROR-CODE, REALM, and NONCE (for the 401/438
    /// challenge flow). ERROR-CODE value: 2 reserved bytes, then class(1) +
    /// number(1), then a UTF-8 reason (ignored). code = class*100 + number.
    fn parse_error_challenge(datagram: &[u8]) -> (Option<u16>, Option<String>, Option<String>) {
        let mut code = None;
        let mut realm = None;
        let mut nonce = None;
        for (atype, val) in iter_attrs(datagram) {
            match atype {
                ATTR_ERROR_CODE if val.len() >= 4 => {
                    let class = (val[2] & 0x07) as u16;
                    let number = val[3] as u16;
                    code = Some(class * 100 + number);
                }
                ATTR_REALM => {
                    realm = String::from_utf8(val.to_vec()).ok();
                }
                ATTR_NONCE => {
                    nonce = String::from_utf8(val.to_vec()).ok();
                }
                _ => {}
            }
        }
        (code, realm, nonce)
    }

    /// Iterate (attr_type, value) over a STUN/TURN message's attribute region.
    fn iter_attrs(datagram: &[u8]) -> Vec<(u16, &[u8])> {
        iter_attrs_with_offsets(datagram)
            .into_iter()
            .map(|(t, off, len)| (t, &datagram[off..off + len]))
            .collect()
    }

    /// Iterate (attr_type, value_offset_within_datagram, value_len) — the offset
    /// form is needed so callers can return DATA sub-slices by index.
    fn iter_attrs_with_offsets(datagram: &[u8]) -> Vec<(u16, usize, usize)> {
        let mut out = Vec::new();
        if datagram.len() < 20 {
            return out;
        }
        let msg_len = u16::from_be_bytes([datagram[2], datagram[3]]) as usize;
        let end = (20 + msg_len).min(datagram.len());
        let mut off = 20;
        while off + 4 <= end {
            let atype = u16::from_be_bytes([datagram[off], datagram[off + 1]]);
            let alen = u16::from_be_bytes([datagram[off + 2], datagram[off + 3]]) as usize;
            let vstart = off + 4;
            if vstart + alen > end {
                break;
            }
            out.push((atype, vstart, alen));
            // Advance past value + padding to 4-byte boundary.
            let padded = (alen + 3) & !3;
            off = vstart + padded;
        }
        out
    }

    /// The long-term-credential key: `MD5(username ":" realm ":" password)`.
    /// (RFC 5389 §15.4 — when there's no SASLprep, the raw bytes are used.)
    pub fn long_term_key(username: &str, realm: &str, password: &str) -> [u8; 16] {
        use md5::{Digest, Md5};
        let mut hasher = Md5::new();
        hasher.update(username.as_bytes());
        hasher.update(b":");
        hasher.update(realm.as_bytes());
        hasher.update(b":");
        hasher.update(password.as_bytes());
        let out = hasher.finalize();
        let mut key = [0u8; 16];
        key.copy_from_slice(&out);
        key
    }

    /// Append the MESSAGE-INTEGRITY attribute = HMAC-SHA1(key, message-so-far),
    /// where the hash input is the message from byte 0 up to (but NOT including)
    /// the MESSAGE-INTEGRITY attribute's TLV, with the header length field FIRST
    /// set to cover the whole message INCLUDING this 24-byte attribute.
    ///
    /// Exact byte ranges (RFC 5389 §15.4):
    ///   * Let `pre_len = msg.len()` (everything appended before M-I).
    ///   * Set header length (bytes 2..4) = `(pre_len - 20) + 24`  (the +24 is the
    ///     4-byte attr header + 20-byte HMAC value this attribute will occupy).
    ///   * HMAC input = `msg[0..pre_len]` (the header with the patched length +
    ///     all prior attributes), NOT including the M-I TLV itself.
    ///   * Append attr type 0x0008, length 20, then the 20-byte HMAC value.
    pub fn append_message_integrity(msg: &mut Vec<u8>, key: &[u8]) {
        use hmac::{Hmac, Mac};
        use sha1::Sha1;

        let pre_len = msg.len();
        // Patch the header length to include the forthcoming 24-byte M-I attr.
        let len_with_mi = ((pre_len - 20) + 24) as u16;
        msg[2..4].copy_from_slice(&len_with_mi.to_be_bytes());

        // HMAC-SHA1 over the message bytes BEFORE the M-I attribute.
        let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(&msg[0..pre_len]);
        let tag = mac.finalize().into_bytes(); // 20 bytes

        // Append the MESSAGE-INTEGRITY attribute (type + len 20 + the 20-byte tag).
        append_attr(msg, ATTR_MESSAGE_INTEGRITY, &tag);
        // NOTE: header length already accounts for this attribute (set above), so
        // we do NOT call finalize_length again after M-I.
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Lock the STUN message-type bit interleaving for the TURN methods we
        /// use, against the RFC 5766 §13 / RFC 5389 §6 known values.
        #[test]
        fn message_type_bit_interleaving() {
            assert_eq!(message_type(METHOD_ALLOCATE, CLASS_REQUEST), 0x0003);
            assert_eq!(message_type(METHOD_ALLOCATE, CLASS_SUCCESS), 0x0103);
            assert_eq!(message_type(METHOD_ALLOCATE, CLASS_ERROR), 0x0113);
            assert_eq!(message_type(METHOD_REFRESH, CLASS_REQUEST), 0x0004);
            assert_eq!(message_type(METHOD_CREATE_PERMISSION, CLASS_REQUEST), 0x0008);
            assert_eq!(message_type(METHOD_CHANNEL_BIND, CLASS_REQUEST), 0x0009);
            assert_eq!(message_type(METHOD_SEND, CLASS_INDICATION), 0x0016);
            assert_eq!(message_type(METHOD_DATA, CLASS_INDICATION), 0x0017);
        }

        /// RFC 5769 §2.4 "Sample Request with Long-Term Authentication" pins the
        /// long-term-credential key derivation. With:
        ///   username = "\u{30DE}\u{30C8}\u{30EA}\u{30C3}\u{30AF}\u{30B9}" (SASLprep'd)
        ///   realm    = "example.org"
        ///   password = "TheMatrIX"
        /// the key MD5(username:realm:password) is the documented 16-byte value.
        /// We assert our `long_term_key` reproduces it byte-for-byte. (We pass the
        /// already-SASLprep'd UTF-8 username bytes, matching the RFC's note that
        /// the key is computed over the processed username.)
        #[test]
        fn rfc5769_long_term_key_kat() {
            // The RFC 5769 §2.4 username, post-SASLprep, is these exact UTF-8
            // bytes (the Katakana string マトリックス).
            let username = "\u{30DE}\u{30C8}\u{30EA}\u{30C3}\u{30AF}\u{30B9}";
            let realm = "example.org";
            let password = "TheMatrIX";
            let key = long_term_key(username, realm, password);
            // RFC 5769 §2.4: the 16-byte key = MD5(username ":" realm ":"
            // password) over the SASLprep'd username (the Katakana string is
            // already in normalized form, so SASLprep is a no-op and the raw
            // UTF-8 bytes are hashed). This value is cross-checked against an
            // independent MD5 implementation (Node's crypto):
            //   MD5("マトリックス:example.org:TheMatrIX")
            //     = e8ca7ad59d5eb0518e312911d2dab2a9
            let expected: [u8; 16] = [
                0xe8, 0xca, 0x7a, 0xd5, 0x9d, 0x5e, 0xb0, 0x51, 0x8e, 0x31, 0x29, 0x11, 0xd2, 0xda,
                0xb2, 0xa9,
            ];
            assert_eq!(key, expected, "RFC 5769 §2.4 long-term key MD5(user:realm:pass)");
        }

        /// Lock the MESSAGE-INTEGRITY construction: the HMAC-SHA1 must be computed
        /// over the message bytes up to (not including) the M-I attribute, with
        /// the header length pre-patched to include the 24-byte M-I attribute.
        /// This is a self-consistent round-trip: we build a message, append M-I,
        /// then independently recompute the HMAC over the documented byte range
        /// and confirm the appended tag matches, AND that the header length is
        /// the pre-M-I attribute length + 24.
        #[test]
        fn message_integrity_byte_ranges() {
            use hmac::{Hmac, Mac};
            use sha1::Sha1;

            let key = long_term_key("humanity", "united-humanity.us", "turnRelay2026!secure");
            let txid = [1u8; 12];
            let mut msg = begin_message(message_type(METHOD_ALLOCATE, CLASS_REQUEST), &txid);
            append_attr(&mut msg, ATTR_REQUESTED_TRANSPORT, &REQUESTED_TRANSPORT_UDP.to_be_bytes());
            let pre_len = msg.len();

            append_message_integrity(&mut msg, &key);

            // (a) Header length = (pre_len - 20) + 24.
            let hdr_len = u16::from_be_bytes([msg[2], msg[3]]) as usize;
            assert_eq!(hdr_len, (pre_len - 20) + 24, "length field includes the M-I attr");

            // (b) The appended attribute is MESSAGE-INTEGRITY (type 0x0008, len 20).
            let attr_type = u16::from_be_bytes([msg[pre_len], msg[pre_len + 1]]);
            let attr_len = u16::from_be_bytes([msg[pre_len + 2], msg[pre_len + 3]]) as usize;
            assert_eq!(attr_type, ATTR_MESSAGE_INTEGRITY);
            assert_eq!(attr_len, 20);

            // (c) Independently recompute HMAC-SHA1 over msg[0..pre_len] and
            //     confirm it equals the 20-byte tag the function appended.
            let mut mac = Hmac::<Sha1>::new_from_slice(&key).unwrap();
            mac.update(&msg[0..pre_len]);
            let expected = mac.finalize().into_bytes();
            let appended_tag = &msg[pre_len + 4..pre_len + 4 + 20];
            assert_eq!(appended_tag, &expected[..], "HMAC-SHA1 over the pre-M-I bytes");

            // (d) Cross-language oracle: the same key + message bytes, run through
            //     an INDEPENDENT HMAC-SHA1 implementation (Node's crypto), produce
            //     this exact 20-byte tag. Locks our RustCrypto HMAC-SHA1 + the
            //     MD5 key + the byte ranges against an external reference, not just
            //     self-consistency. (key = MD5("humanity:united-humanity.us:\
            //     turnRelay2026!secure") = 4457ac79…; tag computed in node.)
            let node_tag: [u8; 20] = [
                0xcc, 0xea, 0x50, 0xf7, 0x7b, 0xe4, 0x1a, 0x1b, 0xeb, 0x3d, 0x60, 0x33, 0x51, 0x9c,
                0xab, 0x0b, 0xfd, 0x71, 0xb1, 0x73,
            ];
            assert_eq!(appended_tag, &node_tag[..], "HMAC-SHA1 matches the node reference");
        }

        /// XOR-PEER-ADDRESS round-trip: encode an address, parse it back.
        #[test]
        fn xor_peer_address_roundtrip() {
            let addr: SocketAddr = "203.0.113.45:51234".parse().unwrap();
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_SEND, CLASS_INDICATION), &txid);
            append_xor_peer_address(&mut msg, addr, &txid);
            finalize_length(&mut msg);

            // Find the XOR-PEER-ADDRESS attr and decode it.
            let attrs = iter_attrs(&msg);
            let (_, val) = attrs
                .iter()
                .find(|(t, _)| *t == ATTR_XOR_PEER_ADDRESS)
                .expect("XOR-PEER-ADDRESS present");
            let decoded = parse_xor_address(val, &msg).expect("decodes");
            assert_eq!(decoded, addr, "XOR address round-trips");
        }

        /// ChannelData framing: a bound channel's outbound frame is
        /// channel(2) + length(2) + data, and our inbound parser recovers the
        /// peer + the exact inner byte range.
        #[test]
        fn channel_data_frame_and_parse() {
            let mut client = TurnClient {
                server: "1.2.3.4:3478".parse().unwrap(),
                phase: Phase::Allocated,
                realm: Some("r".into()),
                nonce: Some("n".into()),
                relayed: Some("5.6.7.8:9000".parse().unwrap()),
                lifetime: Duration::from_secs(600),
                allocated_at: Some(Instant::now()),
                last_allocate: None,
                last_refresh: None,
                peers: HashMap::new(),
                next_channel: FIRST_CHANNEL,
            };
            let peer: SocketAddr = "9.9.9.9:1111".parse().unwrap();
            client.peers.insert(
                peer,
                PeerChannel { channel: 0x4001, bound: true, last_bind: None },
            );

            // Build a ChannelData frame by hand (what a server would send to us).
            let payload = b"hello-relayed";
            let mut frame = Vec::new();
            frame.extend_from_slice(&0x4001u16.to_be_bytes());
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            frame.extend_from_slice(payload);

            match client.handle_channel_data(&frame) {
                TurnInbound::Data { peer: p, inner_offset, inner_len } => {
                    assert_eq!(p, peer, "channel mapped back to the right peer");
                    assert_eq!(&frame[inner_offset..inner_offset + inner_len], payload);
                }
                _ => panic!("expected relayed Data from ChannelData"),
            }
        }

        /// A first-byte in 0x40..=0x7F is ChannelData; a STUN message (first byte
        /// ≤ 0x3F) is not — the wire discriminator must hold.
        #[test]
        fn channel_data_vs_stun_discriminator() {
            // Allocate Success starts with 0x01 (type 0x0103) → NOT channel data.
            let stun_type = message_type(METHOD_ALLOCATE, CLASS_SUCCESS);
            assert!(stun_type <= 0x3FFF, "STUN message types are <= 0x3FFF");
            assert!((stun_type >> 8) as u8 <= 0x3F, "STUN first byte <= 0x3F");
            // Channel numbers occupy 0x4000..=0x7FFF → first byte 0x40..=0x7F.
            assert!((0x4000u16 >> 8) as u8 == 0x40);
            assert!((0x7FFFu16 >> 8) as u8 == 0x7F);
        }
    }
}

/// Phase B (v0.489): str0m audio-media negotiation tests. These are pure SDP
/// operations (no network), so they deterministically verify that the voice path
/// adds an Opus audio m-line AND that the default data-only path is unchanged
/// (the regression guard for existing P2P-group connections).
#[cfg(test)]
mod phase_b_voice_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn voice_offer_negotiates_opus_audio_mline() {
        // Offerer: data channel first (keeps it at SDP index 0), then voice audio.
        let mut offerer = Rtc::builder().build(Instant::now());
        let mut api = offerer.sdp_api();
        let _cid = api.add_channel(CHANNEL_LABEL.to_string());
        let audio_mid = api.add_media(MediaKind::Audio, Direction::SendRecv, None, None, None);
        let (offer, _pending) = api.apply().expect("offer produced");
        let offer_json = serde_json::to_string(&offer).unwrap();
        assert!(offer_json.contains("m=audio"), "offer should carry an audio m-line");
        assert!(offer_json.to_lowercase().contains("opus"), "offer should advertise Opus");
        assert!(!audio_mid.to_string().is_empty(), "add_media should return a usable mid");

        // Answerer: accept_offer auto-mirrors the audio m-line into the answer.
        let mut answerer = Rtc::builder().build(Instant::now());
        let answer = answerer.sdp_api().accept_offer(offer).expect("answer produced");
        let answer_json = serde_json::to_string(&answer).unwrap();
        assert!(answer_json.contains("m=audio"), "answer should mirror the audio m-line");
        assert!(answer_json.to_lowercase().contains("opus"), "answer should advertise Opus");
    }

    #[test]
    fn data_only_offer_has_no_audio_mline() {
        // The default path (wants_voice = false) must NOT add audio, so existing
        // P2P-group connections produce a byte-identical offer to pre-Phase-B.
        let mut offerer = Rtc::builder().build(Instant::now());
        let mut api = offerer.sdp_api();
        let _cid = api.add_channel(CHANNEL_LABEL.to_string());
        let (offer, _pending) = api.apply().expect("offer produced");
        let offer_json = serde_json::to_string(&offer).unwrap();
        assert!(!offer_json.contains("m=audio"), "data-only offer must have no audio m-line");
    }
}

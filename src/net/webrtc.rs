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
//!   markers and the `mod stun` block. Since 2026-10-09 the STUN servers are
//!   asked only while a direct connection is being made (`stun_request_due`),
//!   not on every chat connection, and (step E) the only STUN server asked is
//!   the one the connected server lists for itself (`own_stun_hosts`), never a
//!   third party's.
//! - Who we answer (2026-10-09): a `dc_offer` is answered only when the app has
//!   a reason to connect to its sender (`OfferReason`, `direct_offer_reason`);
//!   answering hands over this device's network address.
//! - inc-3b (THIS increment): TURN relay (RFC 5766) for the symmetric-NAT
//!   fallback. When BOTH peers are behind symmetric NATs, srflx hole-punching
//!   fails (each NAT maps the same internal socket to a *different* external
//!   port per destination, so the srflx the peer learned is useless for the
//!   peer-to-peer 5-tuple). The fix is a relay: both peers send to / receive
//!   from a shared TURN server, which forwards between them. We hand-roll an
//!   RFC 5766 TURN client (long-term-credential auth) over the SAME shared
//!   `UdpSocket`: Allocate (→ a relayed transport address), CreatePermission +
//!   ChannelBind per peer, then relay peer traffic as TURN ChannelData. See the
//!   `// inc-3b` / `// TURN` markers and the `mod turn` block at the bottom of
//!   the file. Step E (2026-10-09) changed what it is for: the server's own
//!   forwarder, one allocation per call or voice room, and the relayed address
//!   is a voice connection's ONLY candidate, riding in its SDP (see "Step E"
//!   below). It is no longer fetched for everyone at startup, nor added to
//!   direct data connections.
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
//! # Step E (2026-10-09): calls and voice rooms go through the server only
//!
//! docs/design/blocking-and-safe-mode.md section 10f. A voice connection (a
//! voice room, or an accepted 1:1 call) is **relay only**: its `Rtc` gets ONE
//! local candidate, the relayed address the server's forwarder allocated for
//! that room or call, and no host or server-reflexive candidate, so the people
//! at the other end see the server's address and never this device's. The
//! credentials come from `call_credentials` on the chat socket
//! (src/net/call_relay.rs, src/engine/call_relay.rs) and are handed in with
//! `WebrtcHandle::use_relay`; one allocation per room or call (`RelayScope`).
//! Offers and answers for a scope wait (`Deferred`, at most `RELAY_WAIT`)
//! until its allocation is live. When it cannot be had (no credentials, the
//! forwarder never answers), they are dropped and `RelayUnavailable` tells the
//! call UI; nothing falls back to a direct connection. A datagram a relay-only
//! peer's `Rtc` emits from any source but the relayed address is dropped, never
//! sent raw (`route_transmit`).
//!
//! No direct data channel is opened or answered in this mode
//! (`DIRECT_CONNECTIONS`, `answer_while_hiding_address`): not the P2P group
//! mesh, not a contact-card channel, not the Dev tools "P2P test". Group and
//! card messages reach people through the server already. An offer from our
//! own other devices is the one kind still let through, and `on_offer` keeps
//! refusing it as before (it cannot tell devices apart yet), so own-device
//! sync is unchanged. The direct machinery (host and srflx candidates, our own
//! server's STUN) stays for the later opt-in step.
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

use crate::net::call_relay::{CallCredentials, CallScope};

// The STUN servers asked for this device's server-reflexive (public) address
// are no longer compiled in (step E, 10f: "Every Google STUN entry is
// removed"). A direct connection, which is now only the Dev tools "P2P test",
// asks the STUN responder of the server this session is connected to, as that
// server lists itself at `/api/turn-credentials` (`own_stun_hosts`), and only
// while such a connection is being made (`stun_request_due`). A STUN request
// shows whoever answers it this device's address; that server already sees it
// through the chat socket. Calls and voice rooms need no STUN at all: they are
// relay only.

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

// ── TURN (the server's call forwarder) ──────────────────────────────────
//
// Neither the forwarder's address nor any credential is compiled in. Both come
// from the server this session is connected to, per voice room or call, in its
// `call_credentials` reply (step E, 10f; src/net/call_relay.rs). Before step E
// they came from the unauthenticated `/api/turn-credentials`, for everyone,
// whether in a call or not; that list now carries only the server's own STUN.

/// How long offers and answers for a call or voice room wait for its connection
/// through the server: the app's own wait for credentials
/// (`call_relay::CREDENTIALS_WAIT`) plus the forwarder's Allocate give-up, with
/// room to spare. Past it they are dropped and the call UI is told.
const RELAY_WAIT: Duration = Duration::from_secs(25);

/// The most ICE candidates kept for a voice peer whose offer has not been
/// answered yet (its offer is waiting for the relay). A browser trickles its
/// relayed candidate right after its offer, so they must not be dropped.
const EARLY_ICE_MAX: usize = 32;

/// How often, at most, the server's own STUN entry is fetched for a direct
/// connection (`request_stun_servers`).
const STUN_FETCH_INTERVAL: Duration = Duration::from_secs(30);

/// Whether this app opens direct connections, which show this device's network
/// address to the person at the other end. Off in step E (10f): calls and voice
/// rooms go through the server's forwarder (that part has no switch at all),
/// and no direct data channel is offered or answered: not the P2P group mesh,
/// not a contact-card channel, not the Dev tools "P2P test" (which says so in
/// the debug console instead), since group and card messages already reach
/// people through the server (group messages are also posted to the relay and
/// polled every 4 s; a card DM falls back to the mailbox). A direct, lower-delay
/// connection both people opt into is a later step; this becomes its setting
/// then, and the machinery it switches back on is kept and tested.
pub const DIRECT_CONNECTIONS: bool = false;

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

/// Reserved pseudo-room id for 1:1 voice CALLS (v0.703). A call reuses the
/// whole voice-room audio path (str0m Opus m-line, the VoiceConnected /
/// VoiceFrame events, the mic pump), but its signaling must ride the web
/// caller's plain `webrtc_signal` envelope instead of `voice_room_signal` —
/// `emit_voice_signal` branches on this id. Never a real room id (rooms come
/// from the relay's voice_channel_list; this name is ours alone).
pub const CALL_ROOM_ID: &str = "__call__";

// ── Who this device opens a direct connection to (2026-10-09) ────────────
//
// Answering a `dc_offer` hands the sender this device's network address: our
// answer carries our host candidate, and the srflx/relayed candidates trickle
// after it. The relay forwards `dc_offer` from anyone online, and this app used
// to answer every one, so anyone could learn anyone's address without a call
// (docs/design/blocking-and-safe-mode.md, defect 3.7.2). Now an offer is
// answered only when the app already has a reason to connect to that person;
// anyone else is ignored silently: no answer, no error back, nothing to probe.

/// Why this device agreed to answer someone's direct-connection offer. Carried
/// down to the WebRTC thread with the offer (`WebrtcHandle::submit_offer`), so
/// an offer that arrives without one can never be answered by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferReason {
    /// Another device signed in with the same identity.
    OwnDevice,
    /// A friend: we still follow them and hold the friendship certificate
    /// they gave us (`holds_friendship`).
    Friend,
    /// Someone in one of the P2P groups we are in: the group mesh connects
    /// members directly (`ensure_group_mesh`, src/gui/pages/chat/p2p_groups.rs).
    GroupMember,
    /// The other person in our current 1:1 call, or someone in the voice room
    /// we are in.
    CallOrRoom,
    /// The person this device itself asked to connect to (the Dev tools
    /// "P2P test"). The offerer rule makes the side with the larger key send
    /// the offer, so the person we asked is often the one who offers.
    AskedByUs,
}

/// What the app knows about the sender of a direct-connection offer, gathered
/// by the caller from its own state (`dc_offer_reason` in
/// src/engine/frame_ws_poll.rs). Plain data, so the decision below is a pure
/// function a test can drive without a running app.
#[derive(Debug, Default)]
pub struct OfferFacts<'a> {
    /// This device's identity key.
    pub my_key: &'a str,
    /// The sender is a friend (`holds_friendship`, worked out by the caller,
    /// which holds the DM store).
    pub sender_is_friend: bool,
    /// Every member of every P2P group we are in.
    pub group_members: Vec<&'a str>,
    /// The other person in our current 1:1 call, if any.
    pub call_peer: Option<&'a str>,
    /// Everyone in the voice room we are in (empty when not in one).
    pub voice_room_peers: Vec<&'a str>,
    /// The person the Dev tools "P2P test" asked to connect to, if any.
    pub asked_peer: Option<&'a str>,
}

/// Decide whether to answer a direct-connection offer from `from`, and why.
/// `None` means ignore it silently.
pub fn direct_offer_reason(from: &str, facts: &OfferFacts) -> Option<OfferReason> {
    if from.is_empty() {
        return None;
    }
    if from == facts.my_key {
        return Some(OfferReason::OwnDevice);
    }
    if facts.sender_is_friend {
        return Some(OfferReason::Friend);
    }
    if facts.group_members.iter().any(|k| *k == from) {
        return Some(OfferReason::GroupMember);
    }
    if facts.call_peer == Some(from) || facts.voice_room_peers.iter().any(|k| *k == from) {
        return Some(OfferReason::CallOrRoom);
    }
    if facts.asked_peer == Some(from) {
        return Some(OfferReason::AskedByUs);
    }
    None
}

/// Whether `from` counts as a friend for a direct connection. Both halves are
/// needed: we still follow them (unfollowing ends it on our side, even though
/// their pass may stay in our store, section 3.2 of the design), and we hold
/// the pass they gave us on this server (`server`, its did:hum) and it really is
/// theirs, naming us. The store only keeps a pass that verified on arrival
/// (`ingest_control`); it is checked again here because one Dilithium check per
/// offer is cheap and the store is a file on disk.
pub fn holds_friendship(i_follow: bool, their_cert: Option<&str>, server: &str, from: &str, my_key: &str) -> bool {
    i_follow
        && their_cert
            .is_some_and(|c| crate::relay::core::pq_crypto::verify_friend_cert(server, from, my_key, c).is_ok())
}

/// Whether the loop should send STUN Binding Requests now. Asking a STUN server
/// shows it this device's network address, so we ask only while a connection
/// is being made (`connections_being_made` > 0: we offered, or answered an
/// offer we had a reason to answer), only until we have learned the address,
/// and at most once per `STUN_RETRY_INTERVAL`.
fn stun_request_due(
    srflx_known: bool,
    connections_being_made: usize,
    last_send: Option<Instant>,
    now: Instant,
) -> bool {
    if srflx_known || connections_being_made == 0 {
        return false;
    }
    match last_send {
        None => true,
        Some(last) => now.duration_since(last) >= STUN_RETRY_INTERVAL,
    }
}

/// Whether a `dc_offer` the app has a reason to answer may be answered while
/// this device keeps its address to itself (step E, 10f). Answering opens a
/// direct connection, which shows the sender this device's address. In this
/// mode only our own devices are answered, as on the web (decided with the web
/// half of step E, 2026-10-09): friends, group mates and call or room partners
/// are not, since calls and rooms go through the server's forwarder and group
/// and card messages through the server itself. Offers from our own devices
/// then stay refused in `on_offer` (it cannot yet tell them apart), as before,
/// so own-device sync is unchanged.
pub fn answer_while_hiding_address(reason: OfferReason) -> bool {
    DIRECT_CONNECTIONS || reason == OfferReason::OwnDevice
}

/// Where one datagram str0m wants sent goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Wrapped for the server's forwarder (the source is our relayed address).
    Turn,
    /// Straight to its destination (a direct connection's host or srflx pair).
    Direct,
    /// Nowhere: a relay-only peer's datagram from anything but our relayed
    /// address, which would otherwise leave this device's address behind.
    Drop,
}

/// Route one transmit. `relayed` is the live allocation's relayed address, if
/// any; `relay_only` says the peer is a voice peer (step E: never direct).
fn route_transmit(source: SocketAddr, relayed: Option<SocketAddr>, relay_only: bool) -> Route {
    if relayed == Some(source) {
        Route::Turn
    } else if relay_only {
        Route::Drop
    } else {
        Route::Direct
    }
}

/// The STUN entries of the server's `/api/turn-credentials` list that name the
/// server itself (`relay_host`), as `host:port`. Any other host is skipped, so a
/// server still listing a third party's STUN (one older than step E) never
/// makes this app ask it. `turn:` entries are skipped too: the forwarder is for
/// calls, with credentials from `call_credentials`.
pub(crate) fn own_stun_hosts(body: &str, relay_host: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return Vec::new() };
    let Some(servers) = v.get("iceServers").and_then(Value::as_array) else { return Vec::new() };
    let mut out = Vec::new();
    for s in servers {
        let urls: Vec<&str> = match s.get("urls") {
            Some(Value::String(u)) => vec![u.as_str()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        for url in urls {
            let Some(rest) = url.strip_prefix("stun:") else { continue };
            let addr = rest.split('?').next().unwrap_or(rest);
            let Some(hp) = crate::net::call_relay::with_port(addr, 3478) else { continue };
            let host = hp.rsplit_once(':').map(|(h, _)| h).unwrap_or(&hp);
            if host.trim_matches(|c| c == '[' || c == ']').eq_ignore_ascii_case(relay_host) && !out.contains(&hp) {
                out.push(hp);
            }
        }
    }
    out
}

/// The host name of an `http(s)://host[:port][/...]` base.
fn host_of(base: &str) -> &str {
    let rest = base.split_once("://").map(|(_, r)| r).unwrap_or(base);
    let authority = rest.split('/').next().unwrap_or(rest);
    if let Some(end) = authority.strip_prefix('[').and_then(|a| a.find(']')) {
        return &authority[1..end + 1];
    }
    authority.split(':').next().unwrap_or(authority)
}

/// The addresses of the `a=candidate:` lines in an SDP, so a relay-only peer's
/// forwarder permissions can be installed as soon as its offer or answer
/// arrives, before str0m first sends toward it.
fn sdp_candidate_addrs(sdp: &str) -> Vec<SocketAddr> {
    sdp.lines()
        .filter_map(|l| l.trim().strip_prefix("a="))
        .filter(|l| l.starts_with("candidate:"))
        .filter_map(|l| Candidate::from_sdp_string(l).ok())
        .map(|c| c.addr())
        .collect()
}

/// An event surfaced from the WebRTC thread up to the GUI. The GUI drains these
/// via `WebrtcHandle::poll_events()` each frame and turns them into debug lines
/// / chat messages.
#[derive(Debug, Clone)]
pub enum WebrtcEvent {
    /// The DataChannel to `peer` is now open and writable.
    ChannelOpen { peer: String },
    /// A text frame arrived from `peer` over its DataChannel.
    Frame { peer: String, text: String },
    /// A voice peer's WebRTC transport reached Connected (Phase C). The UI can
    /// show it and the audio pump can begin sending Opus to this peer.
    VoiceConnected { peer: String },
    /// One depayloaded Opus frame arrived from `peer` over its voice m-line
    /// (Phase B). Decode + playback is wired in a later phase.
    VoiceFrame { peer: String, opus: Vec<u8> },
    /// The connection / channel to `peer` closed (ICE disconnect or SCTP close).
    Closed { peer: String },
    /// The server's forwarder gave this device a relayed address for this call
    /// or voice room (step E, 10f): its offers and answers now go out, carrying
    /// only that address.
    RelayReady { scope: CallScope },
    /// This call or voice room cannot go through the server: the credentials
    /// never came, or the forwarder never answered or refused them (`lost`
    /// false), or it stopped answering after it had (`lost` true). Nothing
    /// falls back to a direct connection.
    RelayUnavailable { scope: CallScope, lost: bool },
}

/// A command sent from the GUI down into the WebRTC thread (or, for the ones
/// marked internal, from the thread's own helpers back to itself). The public
/// `WebrtcHandle` methods construct the rest.
enum Command {
    /// Credentials for one call or voice room arrived (step E): reach the
    /// forwarder with them.
    UseRelay(CallCredentials),
    /// Internal: the forwarder's name for `scope` was looked up (`None`: it
    /// could not be).
    RelayResolved { scope: CallScope, server: Option<SocketAddr>, username: String, password: String },
    /// The app gave up waiting for `scope`'s credentials: drop what waits for it.
    RelayUnavailable(CallScope),
    /// The call ended or the room was left: let `scope`'s allocation go.
    EndRelay(CallScope),
    /// Internal: the server's own STUN responder(s), for a direct connection.
    StunServers(Vec<SocketAddr>),
    /// An inbound `webrtc_signal` the GUI received and forwarded to us.
    Signal {
        from: String,
        signal_type: String,
        /// The signal payload. By contract this is a JSON *string* (the web
        /// `JSON.stringify`'d the SDP/candidate); we parse it back inside.
        data: Value,
        /// For a `dc_offer`: why the app agreed to answer it. `None` means it
        /// did not, and the thread drops the offer (see `OfferReason`).
        offer_reason: Option<OfferReason>,
    },
    /// Application request: start a connection to `peer` (offerer side).
    /// `wants_voice` negotiates an Opus audio m-line too (Phase B); `voice_room`
    /// (Phase C) tags it as a voice connection so signaling uses voice_room_signal.
    OfferTo { peer: String, wants_voice: bool, voice_room: Option<String> },
    /// Application request: send `text` to `peer` over its open channel.
    SendText { peer: String, text: String },
    /// Application request: send one encoded Opus frame to `peer` over its voice
    /// m-line (Phase B). Dropped if the peer has no negotiated audio m-line.
    SendVoice { peer: String, opus: Vec<u8> },
    /// Inbound voice-room signaling (Phase C): an offer/answer/ice/new_participant
    /// the relay forwarded to us, with `data` as a JSON object (the browser shape).
    VoiceSignal { from: String, room_id: String, signal_type: String, data: Value },
    /// Drop the connection to `peer` immediately (1:1 call hangup, v0.703).
    /// Without this a hung-up call's Rtc stays alive until ICE times out and
    /// its is_alive() guard would refuse the peer's NEXT call offer.
    ClosePeer { peer: String },
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
    /// manager. `from` is the sender's pubkey hex, `signal_type` is
    /// `dc_answer` or `dc_ice`, and `data` is the JSON value the relay
    /// forwarded (a JSON string per the contract). A `dc_offer` passed here is
    /// dropped: answering one hands the sender this device's address, so it
    /// goes through [`submit_offer`](Self::submit_offer) with a reason.
    pub fn submit_signal(&self, from: String, signal_type: String, data: Value) {
        let _ = self.tx_cmd.send(Command::Signal { from, signal_type, data, offer_reason: None });
    }

    /// Feed an inbound `dc_offer` the app has decided to answer, with the
    /// reason it may ([`direct_offer_reason`]). The caller decides; the thread
    /// only answers.
    pub fn submit_offer(&self, from: String, data: Value, reason: OfferReason) {
        let _ = self.tx_cmd.send(Command::Signal {
            from,
            signal_type: "dc_offer".to_string(),
            data,
            offer_reason: Some(reason),
        });
    }

    /// Begin opening a DataChannel to `peer` (by Dilithium pubkey hex).
    ///
    /// Honors the offerer rule: this only actually sends an offer if our key is
    /// lexicographically larger than `peer`. If our key is smaller, the call is
    /// a no-op on the wire — we wait for the peer's offer and answer it. (The
    /// thread enforces this too, so a mistaken caller can't cause glare.)
    pub fn offer_to(&self, peer: String) {
        let _ = self.tx_cmd.send(Command::OfferTo { peer, wants_voice: false, voice_room: None });
    }

    /// Offer a VOICE connection to `peer` in voice room `room_id` (Phase C): adds
    /// the Opus audio m-line and signals over voice_room_signal. The caller
    /// (roster-driven, "newcomer offers") decides we should offer, so the key
    /// glare rule is bypassed. Used when we join a room and dial each incumbent.
    pub fn offer_to_voice(&self, peer: String, room_id: String) {
        let _ = self.tx_cmd.send(Command::OfferTo { peer, wants_voice: true, voice_room: Some(room_id) });
    }

    /// Feed an inbound voice-room signal (offer/answer/ice/new_participant) into
    /// the manager (Phase C). `data` is the browser-shaped JSON object.
    pub fn submit_voice_signal(&self, from: String, room_id: String, signal_type: String, data: Value) {
        let _ = self.tx_cmd.send(Command::VoiceSignal { from, room_id, signal_type, data });
    }

    /// Drop the connection to `peer` now (1:1 call hangup/reject, v0.703).
    pub fn close_peer(&self, peer: String) {
        let _ = self.tx_cmd.send(Command::ClosePeer { peer });
    }

    /// Hand over the server's `call_credentials` for one call or voice room
    /// (step E): the thread allocates on its forwarder, then makes and answers
    /// that scope's voice connections through it only.
    pub fn use_relay(&self, creds: CallCredentials) {
        let _ = self.tx_cmd.send(Command::UseRelay(creds));
    }

    /// No credentials came for `scope`: the thread drops the offers and answers
    /// it was holding for it, and answers none that arrive later.
    pub fn relay_unavailable(&self, scope: CallScope) {
        let _ = self.tx_cmd.send(Command::RelayUnavailable(scope));
    }

    /// The call ended or the room was left: the thread lets `scope`'s
    /// allocation go and closes its voice connections.
    pub fn end_relay(&self, scope: CallScope) {
        let _ = self.tx_cmd.send(Command::EndRelay(scope));
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
    /// The connected server's own STUN responder(s), fetched and looked up on a
    /// helper thread the first time a direct connection needs them
    /// (`request_stun_servers`). Empty until then, and while the server lists none.
    stun_servers: Vec<SocketAddr>,
    /// When the server's STUN entry was last asked for.
    stun_fetch_last: Option<Instant>,
    /// Our learned server-reflexive (public) address, once a Binding Response
    /// arrives. `None` until then. Cached so peers created later also get it.
    srflx: Option<SocketAddr>,
    /// When we last sent a batch of STUN Binding Requests, for the retry cadence.
    /// `None` means "never sent" → send immediately on first loop turn.
    last_stun_send: Option<Instant>,

    // ── Step E: the call or voice room going through the server ───────────
    /// The one call or voice room whose connection goes through the server's
    /// forwarder now, and how far it got. One at a time: the desktop app cannot
    /// be in a call and a voice room at once (call_relay.rs `CallRelayUi`).
    relay: Option<RelayScope>,
    /// Whether a TURN client ever sent from `udp`. A server keeps an allocation
    /// per (client address, server address) pair until its lifetime runs out,
    /// so the next scope's client gets a fresh socket (`rebind_socket`) rather
    /// than meeting 437 (Allocation Mismatch).
    socket_used_for_turn: bool,
    /// Offers and answers waiting for their scope's connection through the
    /// server, replayed once it is ready (`flush_deferred`).
    deferred: Vec<Deferred>,
    /// Voice ICE candidates that arrived before their peer's connection exists
    /// (its offer is among `deferred`), by peer: (when, room id, candidate).
    early_ice: HashMap<String, Vec<(Instant, String, Value)>>,
    /// Commands the thread sends itself: a helper thread's result, or a
    /// deferred command replayed. Separate from `rx_cmd`, whose closing is what
    /// stops the thread, so holding this sender never keeps it alive.
    tx_internal: Sender<Command>,
    rx_internal: Receiver<Command>,
    /// HTTP(S) base of the relay we are CONNECTED to, for its own STUN entry
    /// (`/api/turn-credentials`). Derived from the active server URL at start(),
    /// so a self-hosted node's clients ask THAT node, never a hardcoded host.
    /// `HUMANITY_RELAY_BASE` still overrides for debugging.
    relay_base: String,
}

/// The call or voice room whose connection goes through the server (step E).
struct RelayScope {
    scope: CallScope,
    phase: RelayPhase,
    /// When this scope was first wanted, for `RELAY_WAIT`.
    since: Instant,
}

/// How far a [`RelayScope`] got.
enum RelayPhase {
    /// An offer or answer for it came before its credentials did.
    Waiting,
    /// Its credentials came; the forwarder's name is being looked up.
    Resolving,
    /// Talking to the forwarder: allocating, or allocated (`relayed_addr`).
    Turn(turn::TurnClient),
    /// Nothing will connect for it; the call UI was told.
    Unavailable,
}

/// A command held until its scope's connection through the server is ready.
struct Deferred {
    scope: CallScope,
    until: Instant,
    cmd: Command,
}

/// Whether a voice connection for a scope can be made now.
enum Gate {
    /// Yes, with this relayed address as its one candidate.
    Ready(SocketAddr),
    /// Not yet: hold it.
    Wait,
    /// No: it cannot go through the server, and nothing goes direct.
    Refuse,
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
    /// Voice (Phase C): the voice-room id this connection belongs to. `Some`
    /// means signaling for this peer rides the `voice_room_signal` envelope (data
    /// as a JSON object, with room_id) instead of the P2P `webrtc_signal` path,
    /// and the glare rule is "newcomer offers" (no key tiebreak) to match the web.
    voice_room_id: Option<String>,
    /// Whether we've surfaced VoiceConnected for this peer yet (dedupe).
    voice_announced: bool,
}

impl WebrtcManager {
    /// Spawn the WebRTC thread and return a handle the GUI uses to drive it.
    ///
    /// `my_pubkey_hex` is our Dilithium3 identity hex (the same value sent in
    /// the WS `identify` and compared for the offerer rule). `relay_base` is
    /// the HTTP(S) base of the server this session is connected to; its own
    /// STUN entry is fetched from IT (white-label: a self-hosted node's clients
    /// never call a hardcoded production host).
    pub fn start(my_pubkey_hex: String, relay_base: String) -> WebrtcHandle {
        Self::start_on(my_pubkey_hex, relay_base, "0.0.0.0:0")
    }

    /// [`start`](Self::start) with the UDP socket bound to `bind`: tests bind
    /// 127.0.0.1 only, so nothing they run listens beyond this machine.
    pub(crate) fn start_on(my_pubkey_hex: String, relay_base: String, bind: &'static str) -> WebrtcHandle {
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
            let udp = match UdpSocket::bind(bind) {
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

            // Identity truncated: the full Dilithium hex is ~5 KB and this
            // line fires on every reconnect cycle; during the 2026-08-13
            // outage it was most of a 318 KB run.log by itself.
            log::info!(
                "WebRTC: bound UDP {local_addr}, identity {}..",
                &my_pubkey_hex[..my_pubkey_hex.len().min(16)]
            );
            crate::debug::push_debug(format!("WebRTC bound UDP {local_addr}"));

            let mgr = WebrtcManager::new(my_pubkey_hex, udp, local_addr, rx_cmd, tx_event, tx_outbound, relay_base);
            mgr.run();
        });

        handle
    }

    /// The manager's state at start: no peers, no STUN asked, no call going
    /// through the server.
    fn new(
        my_pubkey_hex: String,
        udp: UdpSocket,
        local_addr: SocketAddr,
        rx_cmd: Receiver<Command>,
        tx_event: Sender<WebrtcEvent>,
        tx_outbound: Sender<String>,
        relay_base: String,
    ) -> WebrtcManager {
        let (tx_internal, rx_internal) = mpsc::channel::<Command>();
        WebrtcManager {
            my_pubkey_hex,
            udp,
            local_addr,
            peers: HashMap::new(),
            rx_cmd,
            tx_event,
            tx_outbound,
            stun_pending: HashMap::new(),
            stun_servers: Vec::new(),
            stun_fetch_last: None,
            srflx: None,
            last_stun_send: None,
            relay: None,
            socket_used_for_turn: false,
            deferred: Vec::new(),
            early_ice: HashMap::new(),
            tx_internal,
            rx_internal,
            relay_base,
        }
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
            // The thread's own queue (a helper's result, a deferred command
            // replayed) goes first; it obeys the same one-per-iteration rule.
            let mut handled_cmd = false;
            let next = match self.rx_internal.try_recv() {
                Ok(cmd) => Ok(cmd),
                Err(_) => self.rx_cmd.try_recv(),
            };
            match next {
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

            // ── A2. inc-3a STUN gather: only while a connection is being made
            //    (never with none, see `stun_request_due`; this runs right after
            //    step A, so the turn that creates the first peer also sends the
            //    first request), and until we know our server-reflexive
            //    (public) address, (re)send Binding Requests to the STUN servers
            //    on a slow cadence. This does NOT mutate any Rtc — it only sends
            //    plain STUN datagrams on the shared socket — so it's free of the
            //    str0m single-mutation invariant and can sit anywhere in the loop.
            self.maybe_send_stun();

            // ── A3. Step E: drive the forwarder client of the call or voice room
            //    in hand (Allocate, retry or give up, Refresh, channel binds),
            //    say when it is ready or cannot be had, and drop what waited too
            //    long. This ONLY sends TURN datagrams on the shared socket and
            //    queues replayed commands; it mutates no Rtc, so it is free of
            //    str0m's single-mutation invariant.
            self.maybe_drive_relay();

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
            if let Some(turn) = self.relay_client() {
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
                            let dest = self.relayed_addr().unwrap_or(self.local_addr);
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
                    //
                    // Step E: relayed data goes only to relay-only (voice)
                    // peers, and a datagram that came straight to our socket
                    // only to direct ones. A voice peer never takes traffic
                    // around the forwarder.
                    let relayed_in = recv_range.is_some();
                    let owner: Option<String> = {
                        let mut found = None;
                        // Fast path: a peer whose learned remote_addr matches.
                        for (k, p) in self.peers.iter().filter(|(_, p)| p.relay_only() == relayed_in) {
                            if p.remote_addr == Some(recv_source) {
                                found = Some(k.clone());
                                break;
                            }
                        }
                        if found.is_none() {
                            // Slow path: ask each Rtc if it accepts this input.
                            for (k, p) in self.peers.iter().filter(|(_, p)| p.relay_only() == relayed_in) {
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
                    // command channel closing eventually. On Windows a UDP send
                    // to a closed port comes back as ConnectionReset on the next
                    // read: expected while a server's forwarder port is closed.
                    if e.kind() == ErrorKind::ConnectionReset {
                        log::debug!("WebRTC: recv_from: {e} (a closed port answered)");
                    } else {
                        log::warn!("WebRTC: recv_from error: {e}");
                    }
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
                    // traffic is provably never wrapped. Step E adds the other
                    // half: a relay-only (voice) peer's datagram from any other
                    // source is DROPPED, never sent raw (`route_transmit`).
                    let relay_only = self.peers.get(key).is_some_and(|p| p.relay_only());
                    match route_transmit(t.source, self.relayed_addr(), relay_only) {
                        Route::Turn => {
                            // Hand the inner datagram + its peer destination to
                            // the TURN client, which frames it as ChannelData (or
                            // a Send indication until a channel is bound) and
                            // sends it to the TURN server over the shared socket.
                            // Best-effort: a wrap/send failure is logged, not fatal.
                            if let Some(RelayScope { phase: RelayPhase::Turn(turn), .. }) = self.relay.as_mut() {
                                turn.send_relayed(&self.udp, t.destination, &t.contents);
                            }
                        }
                        Route::Direct => {
                            // EXISTING inc-1/2/3a path, byte-for-byte unchanged.
                            // str0m tells us the destination (ICE may change it
                            // over the session). A failed send isn't fatal: log
                            // and keep draining.
                            if let Err(e) = self.udp.send_to(&t.contents, t.destination) {
                                log::trace!("WebRTC: udp send_to {} failed: {e}", t.destination);
                            }
                        }
                        Route::Drop => {
                            log::trace!("WebRTC: dropped a voice datagram from {} (relay only)", t.source);
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
                } else if matches!(state, S::Connected | S::Completed) {
                    // Voice (Phase C): surface a one-time VoiceConnected when a
                    // voice peer's transport comes up, so the UI can show it and
                    // the audio pump can start sending to this peer.
                    if let Some(p) = self.peers.get_mut(key) {
                        if p.voice_room_id.is_some() && !p.voice_announced {
                            p.voice_announced = true;
                            let _ = self.tx_event.send(WebrtcEvent::VoiceConnected { peer: key.to_string() });
                            log::info!("WebRTC: voice CONNECTED with {}", short(key));
                            crate::debug::push_debug(format!("Voice connected with {}", short(key)));
                        }
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
            Command::OfferTo { peer, wants_voice, voice_room } => self.cmd_offer_to(peer, wants_voice, voice_room),
            Command::SendText { peer, text } => self.cmd_send_text(peer, text),
            Command::SendVoice { peer, opus } => self.cmd_send_voice(peer, opus),
            Command::VoiceSignal { from, room_id, signal_type, data } => {
                self.cmd_voice_signal(from, room_id, signal_type, data)
            }
            Command::ClosePeer { peer } => {
                // Dropping the PeerConn drops its Rtc; we never touch it again,
                // so str0m's drain invariant is not violated.
                if self.peers.remove(&peer).is_some() {
                    log::info!("WebRTC: closed peer {} (call hangup)", short(&peer));
                    let _ = self.tx_event.send(WebrtcEvent::Closed { peer });
                }
            }
            Command::Signal { from, signal_type, data, offer_reason } => {
                self.cmd_signal(from, signal_type, data, offer_reason)
            }
            Command::UseRelay(creds) => self.cmd_use_relay(creds),
            Command::RelayResolved { scope, server, username, password } => {
                self.cmd_relay_resolved(scope, server, username, password)
            }
            Command::RelayUnavailable(scope) => self.cmd_relay_unavailable(scope),
            Command::EndRelay(scope) => self.cmd_end_relay(scope),
            Command::StunServers(servers) => {
                if !servers.is_empty() {
                    log::info!("WebRTC: the server's own STUN at {servers:?}");
                    self.stun_servers = servers;
                }
            }
        }
    }

    /// Begin an outgoing connection to `peer` (offerer side), honoring the
    /// glare-avoidance offerer rule. A voice connection (`voice_room` set) is
    /// relay only (step E): it waits for its call or room's allocation and
    /// carries the relayed address as its one candidate.
    fn cmd_offer_to(&mut self, peer: String, wants_voice: bool, voice_room: Option<String>) {
        if peer == self.my_pubkey_hex {
            return; // never connect to ourselves
        }
        // Offerer rule. For DATA (P2P groups): only the LARGER pubkey hex offers
        // (glare avoidance by key). For VOICE: the rule is "newcomer offers"
        // (matching the web client) and the caller (lib.rs, roster-driven) has
        // already decided we should offer, so we do NOT apply the key rule.
        if !wants_voice && !(self.my_pubkey_hex > peer) {
            log::debug!("WebRTC: offer_to({}) skipped — we are the answerer (smaller key)", short(&peer));
            return;
        }
        if let Some(p) = self.peers.get(&peer) {
            // Already have a connection in progress / open — don't re-offer.
            if p.rtc.is_alive() {
                return;
            }
        }

        // Step E: a voice connection goes through the server only. Wait for its
        // allocation, or give up on it; never offer it direct.
        let relayed = match voice_room.as_deref() {
            Some(room) => {
                let scope = CallScope::for_voice_peer(room, &peer);
                match self.relay_gate(&scope) {
                    Gate::Ready(addr) => Some(addr),
                    Gate::Wait => {
                        log::debug!("WebRTC: voice offer to {} waits for the {}", short(&peer), scope.label());
                        self.defer(scope, Command::OfferTo { peer, wants_voice, voice_room });
                        return;
                    }
                    Gate::Refuse => {
                        log::info!("WebRTC: no voice offer to {}: the {} cannot go through the server", short(&peer), scope.label());
                        return;
                    }
                }
            }
            None if !DIRECT_CONNECTIONS => {
                log::info!("WebRTC: no direct connection to {}: calls hide this device's address", short(&peer));
                crate::debug::push_debug(format!(
                    "P2P test to {}: direct connections are off while calls go through the server (step E)",
                    short(&peer)
                ));
                return;
            }
            None => None,
        };

        // Build a fresh Rtc, add our candidates, create the data channel,
        // and produce the offer. Per the single-mutation invariant, the loop's
        // step-B drain right after this handle_command batch will pump the
        // resulting Transmits — we don't need to drain inline here.
        let mut rtc = Rtc::builder().build(Instant::now());

        if let Some(relayed) = relayed {
            // Relay only: the relayed address is the ONE candidate, so the
            // offer's SDP names the server's address and nothing of ours.
            if !self.add_relayed_candidate(&mut rtc, relayed) {
                return;
            }
        } else {
            // A direct (data) connection, which both people asked for. Our host
            // ICE candidate is the shared UDP socket's address. This still rides
            // inside the SDP offer (added before apply()) and is all that's
            // needed on a LAN.
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
            // THIS offer's SDP too, with no extra trickle round-trip for it.
            // (If srflx is learned later, apply_srflx_to_all_peers trickles it.)
            if let Some(srflx) = self.srflx {
                match Candidate::server_reflexive(srflx, self.local_addr, Protocol::Udp) {
                    Ok(cand) => {
                        rtc.add_local_candidate(cand);
                    }
                    Err(e) => log::warn!("WebRTC: bad srflx candidate {srflx} for offer: {e}"),
                }
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

        // Emit the offer. VOICE rides the voice_room_signal envelope with `data`
        // as a JSON OBJECT ({type,sdp}, an RTCSessionDescription the browser reads
        // directly); DATA rides webrtc_signal with `data` as a JSON STRING. str0m
        // serializes SdpOffer to {type,sdp} either way, matching the browser.
        if let Some(ref room_id) = voice_room {
            match serde_json::to_value(&offer) {
                Ok(v) => {
                    self.emit_voice_signal(&peer, room_id, "offer", v);
                    log::info!("WebRTC: voice offer -> {} (room {})", short(&peer), room_id);
                    crate::debug::push_debug(format!("Voice offer -> {} room {}", short(&peer), room_id));
                }
                Err(e) => {
                    log::error!("WebRTC: failed to serialize voice offer: {e}");
                    return;
                }
            }
        } else {
            let offer_json = match serde_json::to_string(&offer) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("WebRTC: failed to serialize offer: {e}");
                    return;
                }
            };
            self.emit_signal(&peer, "dc_offer", offer_json);
        }

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
                voice_room_id: voice_room,
                voice_announced: false,
            },
        );
        if relayed.is_some() {
            self.replay_early_ice(&peer);
        }
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
    ///
    /// A `dc_offer` is answered only when it carries an `offer_reason` (the app
    /// decided, `direct_offer_reason`) that may open a direct connection while
    /// this device hides its address (`answer_while_hiding_address`, step E);
    /// any other is dropped without a word back. `dc_answer` and `dc_ice` need
    /// no gate: they only act on a peer this device created itself, by offering
    /// or by answering an offer it took.
    fn cmd_signal(&mut self, from: String, signal_type: String, data: Value, offer_reason: Option<OfferReason>) {
        // By contract `data` is a JSON STRING. Pull the inner string out; if the
        // relay ever forwarded a raw object instead, fall back to re-serializing.
        let inner: String = match &data {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        match signal_type.as_str() {
            "dc_offer" => match offer_reason {
                Some(reason) if answer_while_hiding_address(reason) => self.on_offer(from, &inner, reason),
                Some(reason) => log::debug!(
                    "WebRTC: dc_offer from {} not answered ({reason:?}: no direct connections while calls hide this device's address)",
                    short(&from)
                ),
                None => log::debug!("WebRTC: dc_offer from {} not answered (no reason to connect)", short(&from)),
            },
            "dc_answer" => self.on_answer(from, &inner),
            "dc_ice" => self.on_ice(from, &inner),
            other => log::debug!("WebRTC: ignoring unknown signal_type '{other}' from {}", short(&from)),
        }
    }

    /// We received an offer the app agreed to answer (`reason`), so we are the
    /// answerer. Build an Rtc, accept the offer, and send back our answer.
    fn on_offer(&mut self, from: String, sdp_json: &str, reason: OfferReason) {
        // Our own key is an allowed sender (`OfferReason::OwnDevice`), but this
        // manager names peers by identity key, and another device of ours has
        // the same key as this one, so the two cannot be told apart yet. Until
        // devices get ids of their own, an offer from our own key stays refused.
        if from == self.my_pubkey_hex {
            return;
        }
        log::debug!("WebRTC: answering dc_offer from {} ({reason:?})", short(&from));
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

        // (A data connection carries no relayed candidate since step E: the
        // server's forwarder is for calls and voice rooms, per room or call.)

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
                // Data-path answerer (P2P groups), not voice.
                voice_room_id: None,
                voice_announced: false,
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

        // (A data connection is direct: its candidates need no forwarder
        // permission. Voice candidates take `on_voice_ice`.)
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

    // ── Voice signaling (Phase C) ───────────────────────────────────────────
    // Parallel to the data-channel signaling above, but over the
    // `voice_room_signal` envelope (data as a JSON OBJECT + a room_id), matching
    // the web client so native and web interoperate in the same voice room.

    /// Give a voice connection's `rtc` its ONE local candidate (step E): the
    /// relayed address the server's forwarder allocated for its call or room,
    /// before the offer or answer is produced, so it rides in the SDP. No host
    /// and no server-reflexive candidate: those would show the people at the
    /// other end this device's address. `Candidate::relayed` writes its related
    /// address as 0.0.0.0:0, so the SDP line carries nothing of ours either.
    /// False if str0m refused the candidate (then no connection is made).
    fn add_relayed_candidate(&self, rtc: &mut Rtc, relayed: SocketAddr) -> bool {
        match Candidate::relayed(relayed, self.local_addr, Protocol::Udp) {
            Ok(cand) => {
                rtc.add_local_candidate(cand);
                true
            }
            Err(e) => {
                log::warn!("WebRTC: bad relayed candidate {relayed}: {e}");
                false
            }
        }
    }

    /// Dispatch an inbound voice-room signal.
    fn cmd_voice_signal(&mut self, from: String, room_id: String, signal_type: String, data: Value) {
        if from == self.my_pubkey_hex {
            return;
        }
        match signal_type.as_str() {
            "new_participant" => {
                // The relay tells incumbents a newcomer joined. Per the web rule,
                // the incumbent does NOTHING and waits for the newcomer's offer.
                // (lib.rs handles the reverse: when WE are the newcomer, the roster
                // drives us to offer each incumbent.)
                log::debug!("WebRTC: voice new_participant {} room {} (waiting for offer)", short(&from), room_id);
            }
            "offer" => self.on_voice_offer(from, room_id, data),
            "answer" => self.on_voice_answer(from, data),
            "ice" => self.on_voice_ice(from, room_id, data),
            other => log::debug!("WebRTC: unknown voice signal '{other}' from {}", short(&from)),
        }
    }

    /// We received a voice offer — answer it. `data` is an RTCSessionDescription
    /// object ({type,sdp}). accept_offer auto-mirrors the offer's Opus m-line, so
    /// the answer is sendrecv and we learn the mid via Event::MediaAdded.
    fn on_voice_offer(&mut self, from: String, room_id: String, data: Value) {
        if from == self.my_pubkey_hex {
            return;
        }
        // KNOWN EDGE (v0.703): a live P2P DataChannel connection to this peer
        // also trips this guard and would silently refuse their 1:1 call offer.
        // Rare today (native DC is a manual dev tool); the right fix is
        // renegotiating a voice m-line onto the existing Rtc. Tracked in
        // PRIORITIES with the call follow-ups.
        // If we already have a live connection to this peer, keep it (avoids a
        // glare overwrite). The web guards the same way (one PC per peer).
        if self.peers.get(&from).map(|p| p.rtc.is_alive()).unwrap_or(false) {
            log::debug!("WebRTC: voice offer from {} but a connection already exists", short(&from));
            return;
        }
        // Step E: answered only through the server. Hold it until the call or
        // room's allocation is live, or drop it when that cannot be had.
        let scope = CallScope::for_voice_peer(&room_id, &from);
        let relayed = match self.relay_gate(&scope) {
            Gate::Ready(addr) => addr,
            Gate::Wait => {
                log::debug!("WebRTC: voice offer from {} waits for the {}", short(&from), scope.label());
                let cmd = Command::VoiceSignal { from, room_id, signal_type: "offer".into(), data };
                self.defer(scope, cmd);
                return;
            }
            Gate::Refuse => {
                log::info!("WebRTC: voice offer from {} not answered: the {} cannot go through the server", short(&from), scope.label());
                return;
            }
        };
        // The offer's own candidates (a native offerer puts its relayed address
        // in the SDP): their forwarder permissions go in now, before str0m
        // first sends toward them.
        let offered = data.get("sdp").and_then(Value::as_str).map(sdp_candidate_addrs).unwrap_or_default();
        let offer: SdpOffer = match serde_json::from_value(data) {
            Ok(o) => o,
            Err(e) => {
                log::warn!("WebRTC: bad voice offer from {}: {e}", short(&from));
                return;
            }
        };
        let mut rtc = Rtc::builder().build(Instant::now());
        if !self.add_relayed_candidate(&mut rtc, relayed) {
            return;
        }
        self.permit(&offered);
        let answer = match rtc.sdp_api().accept_offer(offer) {
            Ok(a) => a,
            Err(e) => {
                log::warn!("WebRTC: voice accept_offer from {} failed: {e}", short(&from));
                return;
            }
        };
        let answer_val = match serde_json::to_value(&answer) {
            Ok(v) => v,
            Err(e) => {
                log::error!("WebRTC: serialize voice answer: {e}");
                return;
            }
        };
        self.peers.insert(
            from.clone(),
            PeerConn {
                rtc,
                pending: None,
                channel: None,
                remote_addr: None,
                announced_open: false,
                audio_mid: None,
                voice_rtp_ts: 0,
                voice_room_id: Some(room_id.clone()),
                voice_announced: false,
            },
        );
        self.emit_voice_signal(&from, &room_id, "answer", answer_val);
        self.replay_early_ice(&from);
        log::info!("WebRTC: voice answer -> {} (room {})", short(&from), room_id);
        crate::debug::push_debug(format!("Voice answer -> {} room {}", short(&from), room_id));
    }

    /// We received the answer to a voice offer we sent.
    fn on_voice_answer(&mut self, from: String, data: Value) {
        // The answer's candidates get their forwarder permissions now (step E).
        let answered = data.get("sdp").and_then(Value::as_str).map(sdp_candidate_addrs).unwrap_or_default();
        self.permit(&answered);
        let answer: SdpAnswer = match serde_json::from_value(data) {
            Ok(a) => a,
            Err(e) => {
                log::warn!("WebRTC: bad voice answer from {}: {e}", short(&from));
                return;
            }
        };
        let p = match self.peers.get_mut(&from) {
            Some(p) => p,
            None => {
                log::debug!("WebRTC: voice answer from {} but no connection", short(&from));
                return;
            }
        };
        let pending = match p.pending.take() {
            Some(pending) => pending,
            None => {
                log::debug!("WebRTC: voice answer from {} but no pending offer", short(&from));
                return;
            }
        };
        if let Err(e) = p.rtc.sdp_api().accept_answer(pending, answer) {
            log::warn!("WebRTC: voice accept_answer from {} failed: {e}", short(&from));
            p.rtc.disconnect();
            return;
        }
        log::info!("WebRTC: applied voice answer from {}", short(&from));
    }

    /// We received a remote ICE candidate for a voice peer. `data` is the browser
    /// RTCIceCandidate object ({candidate, sdpMid, sdpMLineIndex, ...}). One that
    /// arrives before the peer's connection exists (its offer is waiting for the
    /// relay, step E) is kept and replayed once it does: a browser trickles its
    /// relayed candidate right after its offer.
    fn on_voice_ice(&mut self, from: String, room_id: String, data: Value) {
        let sdp_line = match data.get("candidate").and_then(|c| c.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                // Empty candidate == end-of-candidates marker; nothing to add.
                return;
            }
        };
        let cand = match Candidate::from_sdp_string(&sdp_line) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("WebRTC: unparseable voice ICE from {}: {e}", short(&from));
                return;
            }
        };
        if !self.peers.contains_key(&from) {
            let kept = self.early_ice.entry(from.clone()).or_default();
            if kept.len() < EARLY_ICE_MAX {
                kept.push((Instant::now(), room_id, data));
            }
            log::debug!("WebRTC: voice ICE from {} before its connection exists, kept", short(&from));
            return;
        }
        // Its forwarder permission and channel first (no Rtc touched), so the
        // relayed pair can carry data from the first check.
        self.permit(&[cand.addr()]);
        if let Some(p) = self.peers.get_mut(&from) {
            p.rtc.add_remote_candidate(cand);
            log::trace!("WebRTC: added voice ICE from {}", short(&from));
        }
    }

    /// Build a `voice_room_signal` envelope and queue it for the GUI to relay.
    /// `data` is a JSON OBJECT (the browser RTCSessionDescription / candidate
    /// shape), NOT a string — this is the key difference from `emit_signal`.
    ///
    /// 1:1 CALL exception (v0.703): when `room_id` is [`CALL_ROOM_ID`], this is
    /// a direct call, not a room, and the web caller expects the plain
    /// `webrtc_signal` envelope (`signal_type` "offer"/"answer"/"ice", object
    /// `data`, NO room_id) — see web/chat/chat-voice-calls.js. Same audio
    /// machinery, different signaling dress.
    fn emit_voice_signal(&self, to: &str, room_id: &str, signal_type: &str, data: Value) {
        let msg = if room_id == CALL_ROOM_ID {
            serde_json::json!({
                "type": "webrtc_signal",
                "from": self.my_pubkey_hex,
                "to": to,
                "signal_type": signal_type,
                "data": data,
            })
        } else {
            serde_json::json!({
                "type": "voice_room_signal",
                "from": self.my_pubkey_hex,
                "to": to,
                "room_id": room_id,
                "signal_type": signal_type,
                "data": data,
            })
        };
        let _ = self.tx_outbound.send(msg.to_string());
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

    /// If we don't yet know our server-reflexive (public) address AND a
    /// connection is being made, (re)send a STUN Binding Request to each
    /// configured STUN server, at most once per `STUN_RETRY_INTERVAL`. No-op
    /// once `srflx` is known, and no-op while there is no connection: the
    /// manager starts on every chat connection, and until 2026-10-09 this asked
    /// Google for the address of every desktop app that connected, voice or
    /// not. Now the first request goes out in the same loop turn that creates
    /// the first connection, and the address it learns is trickled to the peer
    /// (`apply_srflx_to_all_peers`), the way a candidate learned late always was.
    ///
    /// Since step E only a DIRECT connection counts (the Dev tools "P2P test"):
    /// voice is relay only and needs no STUN. The servers asked are the
    /// connected server's own (`request_stun_servers`), never a third party's.
    ///
    /// This only sends opaque UDP datagrams on the shared socket — it touches no
    /// `Rtc` — so it is exempt from str0m's single-mutation drain invariant.
    fn maybe_send_stun(&mut self) {
        let now = Instant::now();
        // Reaped every loop turn, so this counts connections still being made
        // or in use. The server's STUN entry is asked for only after this, so
        // nothing about STUN is touched before a direct connection exists.
        let connections = self.peers.values().filter(|p| p.rtc.is_alive() && !p.relay_only()).count();
        if !stun_request_due(self.srflx.is_some(), connections, self.last_stun_send, now) {
            return;
        }

        // The server's own STUN responder, once looked up (a helper thread
        // fetches and resolves it; until it answers we try again on the retry
        // interval instead of hot-looping).
        if self.stun_servers.is_empty() {
            self.request_stun_servers(now);
            self.last_stun_send = Some(now);
            return;
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
        // Voice peers trickle over voice_room_signal with `data` as an OBJECT;
        // data-channel peers over webrtc_signal with `data` as a STRING.
        if let Some(room_id) = self.peers.get(to).and_then(|p| p.voice_room_id.clone()) {
            self.emit_voice_signal(to, &room_id, "ice", cand_obj);
        } else {
            self.emit_signal(to, "dc_ice", cand_obj.to_string());
        }
    }

    // ════════════════════════════════════════════════════════════════════
    //  Step E: calls and voice rooms through the server's forwarder
    // ════════════════════════════════════════════════════════════════════

    /// Fetch the connected server's own STUN entry and look it up, on a helper
    /// thread (an HTTP request and a name lookup must not stall the loop). The
    /// answer comes back as `Command::StunServers`. At most once per
    /// `STUN_FETCH_INTERVAL`; `HUMANITY_RELAY_BASE` overrides the server for
    /// debugging, as before.
    fn request_stun_servers(&mut self, now: Instant) {
        if self.stun_fetch_last.is_some_and(|t| now.duration_since(t) < STUN_FETCH_INTERVAL) {
            return;
        }
        self.stun_fetch_last = Some(now);
        let base = std::env::var("HUMANITY_RELAY_BASE").unwrap_or_else(|_| self.relay_base.clone());
        if base.is_empty() {
            return;
        }
        let tx = self.tx_internal.clone();
        thread::spawn(move || {
            let _ = tx.send(Command::StunServers(fetch_own_stun_servers(&base)));
        });
    }

    /// The forwarder client of the call or room in hand, if one is talking to it.
    fn relay_client(&self) -> Option<&turn::TurnClient> {
        match &self.relay {
            Some(RelayScope { phase: RelayPhase::Turn(c), .. }) => Some(c),
            _ => None,
        }
    }

    /// Our relayed address on the forwarder, while the allocation is live.
    fn relayed_addr(&self) -> Option<SocketAddr> {
        self.relay_client().and_then(|c| c.relayed_addr())
    }

    /// Whether a voice connection for `scope` can be made now. With no call or
    /// room in hand, `scope` becomes the one awaited (its credentials are on the
    /// way: the app asks as soon as it is in a call or room).
    fn relay_gate(&mut self, scope: &CallScope) -> Gate {
        match &self.relay {
            Some(r) if &r.scope == scope => match &r.phase {
                RelayPhase::Turn(c) => match (c.relayed_addr(), c.failure()) {
                    (Some(addr), _) => Gate::Ready(addr),
                    (None, Some(_)) => Gate::Refuse,
                    (None, None) => Gate::Wait,
                },
                RelayPhase::Unavailable => Gate::Refuse,
                RelayPhase::Waiting | RelayPhase::Resolving => Gate::Wait,
            },
            // Another call or room is in hand. This one's credentials may still
            // come and replace it; until then it waits, at most RELAY_WAIT.
            Some(_) => Gate::Wait,
            None => {
                self.relay = Some(RelayScope { scope: scope.clone(), phase: RelayPhase::Waiting, since: Instant::now() });
                Gate::Wait
            }
        }
    }

    /// Hold `cmd` until `scope`'s connection through the server is ready, at
    /// most `RELAY_WAIT`.
    fn defer(&mut self, scope: CallScope, cmd: Command) {
        self.deferred.push(Deferred { scope, until: Instant::now() + RELAY_WAIT, cmd });
    }

    /// Replay what waited for `scope`, now that it is ready. Each goes back
    /// through the thread's own queue, so the loop still takes one command per
    /// turn and str0m's drain sits between them.
    fn flush_deferred(&mut self, scope: &CallScope) {
        for d in std::mem::take(&mut self.deferred) {
            if &d.scope == scope {
                let _ = self.tx_internal.send(d.cmd);
            } else {
                self.deferred.push(d);
            }
        }
    }

    /// Replay the voice ICE candidates kept for `peer`, now that its connection
    /// exists (through the thread's own queue, one per loop turn).
    fn replay_early_ice(&mut self, peer: &str) {
        if let Some(kept) = self.early_ice.remove(peer) {
            for (_, room_id, data) in kept {
                let _ = self.tx_internal.send(Command::VoiceSignal {
                    from: peer.to_string(),
                    room_id,
                    signal_type: "ice".into(),
                    data,
                });
            }
        }
    }

    /// Install forwarder permissions and channels toward `addrs` (a voice peer's
    /// candidates). The forwarder refuses any address that is not an allocation
    /// of the same call or room, and the client then sends nothing toward it.
    /// No-op without a live allocation. Touches no Rtc.
    fn permit(&mut self, addrs: &[SocketAddr]) {
        if let Some(RelayScope { phase: RelayPhase::Turn(c), .. }) = self.relay.as_mut() {
            for &a in addrs {
                c.ensure_peer(&self.udp, a);
            }
        }
    }

    /// Credentials for one call or voice room arrived: look the forwarder's name
    /// up and reach it. Another call or room in hand is let go first.
    fn cmd_use_relay(&mut self, creds: CallCredentials) {
        let mut since = Instant::now();
        let mut release_other = false;
        if let Some(r) = &self.relay {
            if r.scope == creds.scope {
                match r.phase {
                    // Already reaching the forwarder with these.
                    RelayPhase::Resolving | RelayPhase::Turn(_) => return,
                    // Waiting for exactly these: keep its clock.
                    RelayPhase::Waiting => since = r.since,
                    RelayPhase::Unavailable => {}
                }
            } else {
                release_other = true;
            }
        }
        if release_other {
            self.release_relay();
        }
        log::info!("WebRTC: credentials for the {} (forwarder {})", creds.scope.label(), creds.turn_server);
        self.relay = Some(RelayScope { scope: creds.scope.clone(), phase: RelayPhase::Resolving, since });
        let CallCredentials { scope, turn_server, username, credential, .. } = creds;
        let tx = self.tx_internal.clone();
        // An address needs no lookup. A name is looked up on a helper thread, so
        // a slow name server never stalls the loop and the voice it carries.
        if let Ok(addr) = turn_server.parse::<SocketAddr>() {
            let _ = tx.send(Command::RelayResolved { scope, server: Some(addr), username, password: credential });
            return;
        }
        thread::spawn(move || {
            let server = turn_server.to_socket_addrs().ok().and_then(|mut a| a.find(|a| a.is_ipv4()));
            let _ = tx.send(Command::RelayResolved { scope, server, username, password: credential });
        });
    }

    /// The forwarder's name was looked up: start allocating on it (on a fresh
    /// socket if an earlier call's client used this one).
    fn cmd_relay_resolved(&mut self, scope: CallScope, server: Option<SocketAddr>, username: String, password: String) {
        let current = matches!(&self.relay, Some(r) if r.scope == scope && matches!(r.phase, RelayPhase::Resolving));
        if !current {
            return; // the call or room changed while the name was looked up
        }
        let Some(server) = server else {
            log::info!("WebRTC: the forwarder for the {} could not be found", scope.label());
            self.relay_failed(false);
            return;
        };
        if self.socket_used_for_turn {
            self.rebind_socket();
        }
        self.socket_used_for_turn = true;
        if let Some(r) = self.relay.as_mut() {
            r.phase = RelayPhase::Turn(turn::TurnClient::new(server, username, password));
        }
        crate::debug::push_debug(format!("WebRTC: reaching the call forwarder at {server}"));
    }

    /// The app gave up waiting for `scope`'s credentials (it already told the
    /// person): drop what waits for it, and answer nothing for it later.
    fn cmd_relay_unavailable(&mut self, scope: CallScope) {
        self.deferred.retain(|d| d.scope != scope);
        match &mut self.relay {
            Some(r) if r.scope == scope => {
                if !matches!(r.phase, RelayPhase::Turn(_)) {
                    r.phase = RelayPhase::Unavailable;
                }
            }
            Some(_) => {}
            None => {
                self.relay = Some(RelayScope { scope, phase: RelayPhase::Unavailable, since: Instant::now() });
            }
        }
    }

    /// The call ended or the room was left: let its allocation go and close its
    /// voice connections.
    fn cmd_end_relay(&mut self, scope: CallScope) {
        self.deferred.retain(|d| d.scope != scope);
        if self.relay.as_ref().is_some_and(|r| r.scope == scope) {
            self.release_relay();
        } else {
            self.close_scope_peers(&scope);
        }
    }

    /// Let the call or room in hand go: its allocation (a Refresh with lifetime
    /// 0), what waits for it, and its voice connections.
    fn release_relay(&mut self) {
        let Some(r) = self.relay.take() else { return };
        if let RelayPhase::Turn(client) = r.phase {
            client.release(&self.udp);
        }
        self.deferred.retain(|d| d.scope != r.scope);
        self.close_scope_peers(&r.scope);
        log::info!("WebRTC: let the {} go", r.scope.label());
    }

    /// Close every voice connection that belongs to `scope`.
    fn close_scope_peers(&mut self, scope: &CallScope) {
        let gone: Vec<String> = self
            .peers
            .iter()
            .filter(|(k, p)| p.voice_room_id.as_deref().is_some_and(|room| &CallScope::for_voice_peer(room, k) == scope))
            .map(|(k, _)| k.clone())
            .collect();
        for k in gone {
            self.peers.remove(&k);
            self.early_ice.remove(&k);
            let _ = self.tx_event.send(WebrtcEvent::Closed { peer: k });
        }
    }

    /// The call or room in hand cannot go through the server: drop what waits
    /// for it, close its connections when a live allocation was lost, and tell
    /// the call UI once. Nothing falls back to a direct connection.
    fn relay_failed(&mut self, lost: bool) {
        let Some(r) = self.relay.as_mut() else { return };
        if matches!(r.phase, RelayPhase::Unavailable) {
            return;
        }
        r.phase = RelayPhase::Unavailable;
        let scope = r.scope.clone();
        self.deferred.retain(|d| d.scope != scope);
        if lost {
            self.close_scope_peers(&scope);
        }
        log::info!("WebRTC: the {} cannot go through the server (lost: {lost})", scope.label());
        crate::debug::push_debug(format!("WebRTC: {} cannot go through the server", scope.label()));
        let _ = self.tx_event.send(WebrtcEvent::RelayUnavailable { scope, lost });
    }

    /// A fresh UDP socket for the next call's allocation. The server keys an
    /// allocation by (client address, server address) and keeps the last one
    /// until its lifetime ends if our release was lost; a new allocation from
    /// the same address would meet 437. Connections made on the old socket end
    /// with it (by now only a Dev tools test connection could be one).
    fn rebind_socket(&mut self) {
        let bind = SocketAddr::new(self.local_addr.ip(), 0);
        let udp = match UdpSocket::bind(bind) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("WebRTC: could not open a fresh socket for the next call: {e}");
                return;
            }
        };
        let Ok(local) = udp.local_addr() else { return };
        log::info!("WebRTC: fresh socket {local} for the next call");
        self.udp = udp;
        self.local_addr = local;
        let keys: Vec<String> = self.peers.keys().cloned().collect();
        for k in keys {
            self.peers.remove(&k);
            let _ = self.tx_event.send(WebrtcEvent::Closed { peer: k });
        }
        self.srflx = None;
        self.stun_pending.clear();
        self.last_stun_send = None;
    }

    /// Once per loop turn: drop what waited too long, drive the forwarder
    /// client of the call or room in hand, and say when it became ready (then
    /// replay what waited for it) or cannot be had.
    fn maybe_drive_relay(&mut self) {
        let now = Instant::now();
        let before = self.deferred.len();
        self.deferred.retain(|d| now < d.until);
        if self.deferred.len() < before {
            log::info!("WebRTC: dropped {} call offer(s) that waited too long for the server", before - self.deferred.len());
        }
        self.early_ice.retain(|_, kept| {
            kept.retain(|(t, ..)| now.duration_since(*t) < RELAY_WAIT);
            !kept.is_empty()
        });

        let Some(r) = self.relay.as_mut() else { return };
        let outcome = match &mut r.phase {
            RelayPhase::Waiting | RelayPhase::Resolving => {
                (now.duration_since(r.since) >= RELAY_WAIT).then_some(Err(false))
            }
            RelayPhase::Turn(client) => {
                let newly = client.drive(&self.udp);
                match client.failure() {
                    Some(turn::TurnFailure::Lost(_)) => Some(Err(true)),
                    Some(f) => {
                        log::info!("WebRTC: the call forwarder: {f:?}");
                        Some(Err(false))
                    }
                    None => newly.map(Ok),
                }
            }
            RelayPhase::Unavailable => None,
        };
        let scope = r.scope.clone();
        match outcome {
            Some(Ok(relayed)) => {
                log::info!("WebRTC: the {} goes through the server at {relayed}", scope.label());
                crate::debug::push_debug(format!("WebRTC: {} ready through the server", scope.label()));
                let _ = self.tx_event.send(WebrtcEvent::RelayReady { scope: scope.clone() });
                self.flush_deferred(&scope);
            }
            Some(Err(lost)) => self.relay_failed(lost),
            None => {}
        }
    }

    /// Try to interpret an inbound datagram as TURN traffic from the forwarder.
    ///
    /// The address guard (`source == turn_server_addr`) is the ENTIRE basis for
    /// isolation: only datagrams literally from the forwarder are considered
    /// here, so a peer's host/srflx datagram (sourced from the peer) can never be
    /// mistaken for TURN and always falls through to the existing demux.
    ///
    /// Returns:
    /// * `TurnRecv::NotTurn`: not from the forwarder; caller falls through.
    /// * `TurnRecv::Handled`: a TURN control message (an answer to one of our
    ///   requests, or one we could not match); fully consumed, caller `continue`s.
    /// * `TurnRecv::Relayed { peer, start, len }`: relayed peer data; the inner
    ///   datagram lives at `buf[start..start+len]` and must be fed to str0m with
    ///   `source = peer`, `destination = our_relayed_addr`.
    fn try_handle_turn(&mut self, source: SocketAddr, datagram: &[u8]) -> TurnRecv {
        // `datagram` IS `&buf[..n]`, so an inner slice's offset within it is its
        // offset within the caller's `buf`.
        let Some(RelayScope { phase: RelayPhase::Turn(turn), .. }) = self.relay.as_mut() else {
            return TurnRecv::NotTurn;
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
}

/// Fetch the server's `/api/turn-credentials` from `base` and look up its own
/// STUN entries (`own_stun_hosts`), IPv4 only (our socket's family). Empty on
/// any failure: a direct connection then simply has no server-reflexive
/// candidate, as on a LAN.
fn fetch_own_stun_servers(base: &str) -> Vec<SocketAddr> {
    let url = format!("{}/api/turn-credentials", base.trim_end_matches('/'));
    // ureq has no `json` feature here, so read the body and parse it ourselves.
    let Ok(resp) = ureq::get(&url).timeout(Duration::from_secs(4)).call() else { return Vec::new() };
    let Ok(body) = resp.into_string() else { return Vec::new() };
    own_stun_hosts(&body, host_of(base))
        .iter()
        .filter_map(|hp| hp.to_socket_addrs().ok()?.find(|a| a.is_ipv4()))
        .collect()
}

impl PeerConn {
    /// A voice connection (a call or a voice room): relay only since step E,
    /// never direct.
    fn relay_only(&self) -> bool {
        self.voice_room_id.is_some()
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
//  inc-3b and step E: TURN client (RFC 5766), long-term-credential auth
// ════════════════════════════════════════════════════════════════════════
//
// A minimal, hand-rolled TURN client: JUST enough to allocate a relayed address
// on the server's forwarder, keep it alive, install permissions and channels
// for peers, and carry datagrams. No external TURN crate. It reuses the STUN
// message framing (TURN messages ARE STUN messages with TURN method codes) but
// is otherwise self-contained.
//
// # What str0m provides and what this builds (step E, 10f)
//
// str0m (and its ICE crate `is`) only KNOWS about a relayed candidate: it pairs
// it, gives it a lower priority, and stamps every datagram for that pair with
// the relayed address as its source (see the module docs at the top of this
// file). It has no TURN client at all: no Allocate, no credentials, no
// permissions, no channels, no framing. Everything that talks to the server is
// here: Allocate with long-term credentials (the 401 challenge, then the signed
// request, resent unchanged until answered), Refresh before the lifetime runs
// out (always with the credentials the allocation was made with: the relay
// accepts them past their `ttl` for the allocation's life, so none are asked
// for again mid-call), CreatePermission and ChannelBind for each peer's relayed address (renewed
// before the server's 300 s permission lifetime), Send indications until a
// channel is bound and ChannelData after, and Data indications and ChannelData
// read back into (peer, datagram) for str0m. Responses are matched to their
// request by transaction id, and a response carrying MESSAGE-INTEGRITY that does
// not check out under our key is dropped as if never received.
//
// # STUN message-type bit layout (RFC 5389 §6), needed for TURN methods
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
// We retry Allocate adding USERNAME, NONCE, REALM, and MESSAGE-INTEGRITY.
//   key   = MD5( username ":" realm ":" password )                  (16 bytes)
//   M-I   = HMAC-SHA1( key, message[0 .. start-of-MESSAGE-INTEGRITY] )
// where the message-length field (bytes 2..4) is FIRST set to the value it will
// have *including* the 24-byte MESSAGE-INTEGRITY attribute, but the bytes hashed
// STOP right before the MESSAGE-INTEGRITY attribute's own TLV. See
// `append_message_integrity` for the exact byte ranges. A 401 to a request that
// already carried credentials means they are wrong, and the client gives up.
pub(crate) mod turn {
    use super::stun::MAGIC_COOKIE;
    use std::collections::HashMap;
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
    use std::time::{Duration, Instant};

    // ── TURN method codes (RFC 5766 §13) ──
    pub(crate) const METHOD_ALLOCATE: u16 = 0x003;
    pub(crate) const METHOD_REFRESH: u16 = 0x004;
    pub(crate) const METHOD_SEND: u16 = 0x006;
    pub(crate) const METHOD_DATA: u16 = 0x007;
    pub(crate) const METHOD_CREATE_PERMISSION: u16 = 0x008;
    pub(crate) const METHOD_CHANNEL_BIND: u16 = 0x009;

    // ── STUN message classes (the 2-bit CLASS field) ──
    pub(crate) const CLASS_REQUEST: u16 = 0b00;
    pub(crate) const CLASS_INDICATION: u16 = 0b01;
    pub(crate) const CLASS_SUCCESS: u16 = 0b10;
    pub(crate) const CLASS_ERROR: u16 = 0b11;

    // ── Attribute types (RFC 5389 §18.2 + RFC 5766 §14) ──
    pub(crate) const ATTR_USERNAME: u16 = 0x0006;
    pub(crate) const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
    pub(crate) const ATTR_ERROR_CODE: u16 = 0x0009;
    pub(crate) const ATTR_CHANNEL_NUMBER: u16 = 0x000C;
    pub(crate) const ATTR_LIFETIME: u16 = 0x000D;
    pub(crate) const ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
    pub(crate) const ATTR_DATA: u16 = 0x0013;
    pub(crate) const ATTR_REALM: u16 = 0x0014;
    pub(crate) const ATTR_NONCE: u16 = 0x0015;
    pub(crate) const ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
    pub(crate) const ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;

    // ── Misc constants ──
    /// REQUESTED-TRANSPORT value for UDP (protocol 17 = 0x11) in the top byte.
    const REQUESTED_TRANSPORT_UDP: u32 = 0x1100_0000;
    /// Default allocation lifetime we request (seconds). The server may shorten
    /// it; we honor the value it returns.
    const DEFAULT_LIFETIME_SECS: u32 = 600;
    /// First channel number to hand out (RFC 5766 §11: 0x4000..=0x7FFF).
    const FIRST_CHANNEL: u16 = 0x4000;
    /// How long an Allocate may go unanswered before the client gives up: the
    /// forwarder's port is closed, or nothing listens there yet (10f: "until the
    /// operator opens the port, a relay-only connection cannot form"). Five
    /// sends at the two-second retry.
    pub const ALLOCATE_GIVE_UP: Duration = Duration::from_secs(10);
    /// A permission lasts 300 s on the server and a channel binding 600 s
    /// (RFC 5766 §8, §11). A ChannelBind refreshes both, so a bound channel is
    /// bound again every 240 s, well inside the shorter of the two.
    const BIND_REFRESH: Duration = Duration::from_secs(240);
    /// A request with no answer for this long is forgotten (its answer, if it
    /// ever comes, is then ignored like any stranger's).
    const TXN_FORGET: Duration = Duration::from_secs(30);

    /// Compute the 14-bit STUN message type from a method + class. See the
    /// module header for the bit interleaving.
    pub(crate) fn message_type(method: u16, class: u16) -> u16 {
        // Method bits split at the class-bit positions (8 and 4).
        let m_low = method & 0x000F; // M3..M0  → bits 3..0
        let m_mid = (method >> 4) & 0x0007; // M6..M4 → bits 6..4 (shifted up by 1 for C0)
        let m_high = (method >> 7) & 0x001F; // M11..M7 → bits 13..9 (shifted up by 1 for C1)
        let c0 = class & 0b01; // → bit 4
        let c1 = (class >> 1) & 0b01; // → bit 8
        (m_high << 9) | (c1 << 8) | (m_mid << 5) | (c0 << 4) | m_low
    }

    /// The method and class of a 14-bit message type: the inverse of `message_type`.
    pub(crate) fn method_and_class(msg_type: u16) -> (u16, u16) {
        let method = (msg_type & 0x000F) | ((msg_type >> 1) & 0x0070) | ((msg_type >> 2) & 0x0F80);
        let class = ((msg_type >> 4) & 0b01) | ((msg_type >> 7) & 0b10);
        (method, class)
    }

    /// Generate a random 96-bit TURN/STUN transaction id (reuses the same RNG
    /// idiom as `mod stun`).
    fn random_txid() -> [u8; 12] {
        use rand::RngCore;
        let mut id = [0u8; 12];
        rand::rng().fill_bytes(&mut id);
        id
    }

    /// Per-peer relay state: the channel number we chose, and how the server
    /// took our permission and channel for it.
    struct PeerChannel {
        /// The channel number assigned to this peer (0x4000..=0x7FFF).
        channel: u16,
        /// True once the ChannelBind success response arrived; only then may we
        /// send compact ChannelData. Before that we use Send indications.
        bound: bool,
        /// When we last (re)sent the ChannelBind, to refresh it before the
        /// permission's lifetime and to retry if the answer was lost.
        last_bind: Option<Instant>,
        /// The server refused a permission or channel for this address (403:
        /// on the closed forwarder, an address that is not another allocation
        /// of the same room). Never asked again, and nothing is sent to it.
        refused: bool,
    }

    /// Why a client stopped. The manager turns this into the call UI's line.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TurnFailure {
        /// The forwarder never answered an Allocate (closed port, nothing listening).
        NoAnswer,
        /// The forwarder refused the Allocate with this error code (wrong or
        /// expired credentials, credentials for another room, no capacity).
        Refused(u16),
        /// A live allocation was lost: a Refresh was refused with this code, or
        /// went unanswered past the allocation's lifetime (code 0).
        Lost(u16),
    }

    /// The TURN allocation lifecycle.
    #[derive(Debug, PartialEq)]
    enum Phase {
        /// No allocation yet; (re)send an unauthenticated Allocate to learn the
        /// REALM/NONCE (or, if we already have them, an authenticated Allocate).
        Allocating,
        /// We hold a live allocation (relayed address + lifetime).
        Allocated,
        /// Stopped for good (see `TurnFailure`). Nothing more is sent.
        Failed(TurnFailure),
    }

    /// What an outstanding request was, so its answer is read for what it is.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Txn {
        Allocate,
        Refresh,
        Permission(SocketAddr),
        Bind(SocketAddr),
        /// A Refresh with LIFETIME 0: letting the allocation go.
        Release,
    }

    impl Txn {
        fn method(self) -> u16 {
            match self {
                Txn::Allocate => METHOD_ALLOCATE,
                Txn::Refresh | Txn::Release => METHOD_REFRESH,
                Txn::Permission(_) => METHOD_CREATE_PERMISSION,
                Txn::Bind(_) => METHOD_CHANNEL_BIND,
            }
        }
    }

    /// A minimal RFC 5766 TURN client driven from the WebRTC run-loop. Sends on
    /// the manager's shared `UdpSocket`; never owns the socket.
    pub struct TurnClient {
        /// The forwarder's UDP address.
        server: SocketAddr,
        /// The short-lived username and credential the server issued for one
        /// voice room or call (`call_credentials`, net/call_relay.rs).
        username: String,
        password: String,
        phase: Phase,
        /// REALM from the 401 challenge (needed for the auth key + the attribute).
        realm: Option<String>,
        /// NONCE from the 401 challenge (echoed in every authed request).
        nonce: Option<String>,
        /// The Allocate in flight: its transaction id and bytes, resent
        /// unchanged until answered (RFC 5389 §7.2.1). A lost success is then
        /// answered again from the server's cache instead of meeting 437
        /// (Allocation Mismatch), which a fresh transaction would.
        allocate_msg: Option<([u8; 12], Vec<u8>)>,
        /// Whether that Allocate carries credentials: a 401 to one that does
        /// means they are wrong.
        allocate_authed: bool,
        /// The relayed transport address (XOR-RELAYED-ADDRESS) once allocated.
        relayed: Option<SocketAddr>,
        /// Allocation lifetime the server granted, and when it was last granted,
        /// so we know when to Refresh.
        lifetime: Duration,
        allocated_at: Option<Instant>,
        /// When the first Allocate went out, for `give_up`.
        started: Option<Instant>,
        /// How long the Allocate may go unanswered ([`ALLOCATE_GIVE_UP`]; tests shorten it).
        give_up: Duration,
        /// When we last sent an Allocate (retry cadence while Allocating).
        last_allocate: Option<Instant>,
        /// When we last sent a Refresh (rate-limit so we don't spam refreshes
        /// while waiting for the success response to reset `allocated_at`).
        last_refresh: Option<Instant>,
        /// Per-peer channel/permission state, keyed by the peer's relayed address.
        peers: HashMap<SocketAddr, PeerChannel>,
        /// Next channel number to assign.
        next_channel: u16,
        /// Requests awaiting an answer, by transaction id.
        pending: HashMap<[u8; 12], (Txn, Instant)>,
        /// Whether `drive` has reported the allocation yet (it does so once).
        announced: bool,
    }

    /// What an inbound datagram from the TURN server turned out to be.
    pub enum TurnInbound {
        /// A TURN control message (an answer to one of our requests, or one we
        /// could not match). Fully handled.
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
        /// A client for the forwarder at `server` (already looked up: the
        /// manager does the name lookup on a helper thread), with the
        /// credentials the server issued. Nothing is sent until `drive`.
        pub fn new(server: SocketAddr, username: String, password: String) -> TurnClient {
            TurnClient {
                server,
                username,
                password,
                phase: Phase::Allocating,
                realm: None,
                nonce: None,
                allocate_msg: None,
                allocate_authed: false,
                relayed: None,
                lifetime: Duration::from_secs(DEFAULT_LIFETIME_SECS as u64),
                allocated_at: None,
                started: None,
                give_up: ALLOCATE_GIVE_UP,
                last_allocate: None,
                last_refresh: None,
                peers: HashMap::new(),
                next_channel: FIRST_CHANNEL,
                pending: HashMap::new(),
                announced: false,
            }
        }

        /// The TURN server's address (for the recv-path source guard).
        pub fn server_addr(&self) -> SocketAddr {
            self.server
        }

        /// The relayed transport address, while the allocation is live. `None`
        /// otherwise. This is the value the Transmit-wrap guard compares
        /// `t.source` against, and the only candidate a relay-only peer gets.
        pub fn relayed_addr(&self) -> Option<SocketAddr> {
            match self.phase {
                Phase::Allocated => self.relayed,
                _ => None,
            }
        }

        /// Why the client stopped, if it did.
        pub fn failure(&self) -> Option<TurnFailure> {
            match self.phase {
                Phase::Failed(f) => Some(f),
                _ => None,
            }
        }

        /// The next instant the run-loop should wake us to do TURN work (retry an
        /// Allocate, give up on it, or Refresh the allocation). `None` once failed.
        pub fn next_deadline(&self) -> Option<Instant> {
            match self.phase {
                Phase::Allocating => {
                    let retry = self
                        .last_allocate
                        .map(|t| t + super::TURN_ALLOC_RETRY_INTERVAL)
                        .unwrap_or_else(Instant::now);
                    Some(match self.started {
                        Some(s) => retry.min(s + self.give_up),
                        None => retry,
                    })
                }
                Phase::Allocated => {
                    // Wake to refresh before the lifetime elapses.
                    let at = self.allocated_at?;
                    Some(at + self.refresh_after())
                }
                Phase::Failed(_) => None,
            }
        }

        /// How long after a grant the allocation is refreshed: the slack before
        /// the lifetime runs out, never sooner than 30 s (a tiny lifetime from
        /// the server must not make us refresh in a tight loop).
        fn refresh_after(&self) -> Duration {
            self.lifetime
                .saturating_sub(super::TURN_REFRESH_SLACK)
                .max(Duration::from_secs(30))
        }

        /// Advance the allocate/refresh state machine. Returns `Some(relayed)`
        /// on the FIRST call after the allocation becomes live, so the manager
        /// can say the scope is ready. Best-effort; never panics on the wire.
        pub fn drive(&mut self, udp: &UdpSocket) -> Option<SocketAddr> {
            let now = Instant::now();
            self.pending.retain(|_, (_, at)| now.duration_since(*at) < TXN_FORGET);
            match self.phase {
                Phase::Failed(_) => None,
                Phase::Allocating => {
                    let started = *self.started.get_or_insert(now);
                    if now.duration_since(started) >= self.give_up {
                        log::info!("WebRTC TURN: no answer from {} to Allocate; giving up", self.server);
                        self.phase = Phase::Failed(TurnFailure::NoAnswer);
                        return None;
                    }
                    let due = self
                        .last_allocate
                        .map_or(true, |t| now.duration_since(t) >= super::TURN_ALLOC_RETRY_INTERVAL);
                    if due {
                        self.send_allocate(udp, now);
                    }
                    None
                }
                Phase::Allocated => {
                    let first = !self.announced;
                    self.announced = true;
                    if let Some(at) = self.allocated_at {
                        if now.duration_since(at) >= self.lifetime {
                            // No Refresh answer came in time: the server has let
                            // the allocation go by now.
                            log::info!("WebRTC TURN: allocation on {} ran out unrefreshed", self.server);
                            self.phase = Phase::Failed(TurnFailure::Lost(0));
                            return None;
                        }
                        // Refresh inside the slack window before expiry, at most
                        // once per retry interval while the answer is awaited.
                        let due = now.duration_since(at) >= self.refresh_after();
                        let throttled = self
                            .last_refresh
                            .is_some_and(|t| now.duration_since(t) < super::TURN_ALLOC_RETRY_INTERVAL);
                        if due && !throttled {
                            self.send_refresh(udp, DEFAULT_LIFETIME_SECS, Txn::Refresh, now);
                            self.last_refresh = Some(now);
                        }
                    }
                    // Also (re)bind any channels whose bind is stale / unconfirmed.
                    self.refresh_channels(udp, now);
                    if first {
                        self.relayed
                    } else {
                        None
                    }
                }
            }
        }

        /// Ensure we have a permission (and a channel) for `peer`. Installs a
        /// CreatePermission + ChannelBind if this peer is new. No-op if we have
        /// no live allocation. Idempotent.
        pub fn ensure_peer(&mut self, udp: &UdpSocket, peer: SocketAddr) {
            if self.phase != Phase::Allocated {
                return;
            }
            // TURN only relays IPv4↔IPv4 here (our allocation is IPv4). Skip
            // non-IPv4 peer candidates; they can't ride this relay anyway.
            if !peer.is_ipv4() || self.peers.contains_key(&peer) {
                return;
            }
            let channel = self.next_channel;
            // Advance, wrapping within the valid 0x4000..=0x7FFF window.
            self.next_channel = if self.next_channel >= 0x7FFE {
                FIRST_CHANNEL
            } else {
                self.next_channel + 1
            };
            self.peers.insert(
                peer,
                PeerChannel { channel, bound: false, last_bind: None, refused: false },
            );
            // The permission first, then the channel. (A ChannelBind also installs
            // a permission per RFC 5766 §11.2, but asking for it explicitly is
            // harmless and matches common client behavior.)
            let now = Instant::now();
            self.send_create_permission(udp, peer, now);
            self.send_channel_bind(udp, peer, now);
        }

        /// Send `data` to `peer` through the relay. Uses compact ChannelData once
        /// the channel is confirmed bound; otherwise a Send indication (which
        /// works as soon as the permission is in). Nothing is sent toward an
        /// address the server refused, or without a live allocation.
        pub fn send_relayed(&mut self, udp: &UdpSocket, peer: SocketAddr, data: &[u8]) {
            if self.phase != Phase::Allocated {
                return;
            }
            // Make sure a permission/channel exists (covers the case where str0m
            // chose a peer address we hadn't pre-registered).
            self.ensure_peer(udp, peer);
            let Some(pc) = self.peers.get(&peer) else { return };
            if pc.refused {
                log::trace!("WebRTC TURN: not sending toward refused address {peer}");
                return;
            }
            if pc.bound {
                self.send_channel_data(udp, pc.channel, data);
            } else {
                self.send_indication(udp, peer, data);
            }
        }

        /// Let the allocation go (a Refresh with LIFETIME 0, RFC 5766 §7), so it
        /// does not sit on the server for the rest of its lifetime after the call
        /// or room ends. Best-effort: if it is lost, the lifetime ends it.
        pub fn release(mut self, udp: &UdpSocket) {
            if self.phase == Phase::Allocated {
                self.send_refresh(udp, 0, Txn::Release, Instant::now());
            }
        }

        /// Handle a datagram that arrived FROM the TURN server. Classifies it as
        /// relayed data (ChannelData / Data indication) or a control response.
        pub fn handle_from_server(&mut self, _udp: &UdpSocket, datagram: &[u8]) -> TurnInbound {
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
            let (method, class) = method_and_class(msg_type);
            if method == METHOD_DATA && class == CLASS_INDICATION {
                return self.handle_data_indication(datagram);
            }
            if class != CLASS_SUCCESS && class != CLASS_ERROR {
                return TurnInbound::Control;
            }

            // A response: it must answer a request of ours, of the same method.
            let mut txid = [0u8; 12];
            txid.copy_from_slice(&datagram[8..20]);
            let Some(&(txn, _)) = self.pending.get(&txid) else {
                log::trace!("WebRTC TURN: response 0x{msg_type:04x} matches no request of ours");
                return TurnInbound::Control;
            };
            if txn.method() != method {
                return TurnInbound::Control;
            }
            // A response carrying MESSAGE-INTEGRITY must check out under our key;
            // one that does not is dropped as if never received (RFC 5389
            // §10.2.3). The 401 challenge carries none (we had no key yet).
            if let Some(key) = self.key() {
                if check_message_integrity(datagram, &key) == Some(false) {
                    log::debug!("WebRTC TURN: response with a wrong MESSAGE-INTEGRITY dropped");
                    return TurnInbound::Control;
                }
            }
            self.pending.remove(&txid);

            let success = class == CLASS_SUCCESS;
            match txn {
                Txn::Allocate if success => self.handle_allocate_success(datagram),
                Txn::Allocate => self.handle_allocate_error(datagram),
                Txn::Refresh if success => self.handle_refresh_success(datagram),
                Txn::Refresh => self.handle_refresh_error(datagram),
                Txn::Permission(_) if success => {
                    log::trace!("WebRTC TURN: CreatePermission success");
                }
                Txn::Bind(peer) if success => {
                    if let Some(pc) = self.peers.get_mut(&peer) {
                        pc.bound = true;
                    }
                    log::trace!("WebRTC TURN: ChannelBind success for {peer}");
                }
                Txn::Permission(peer) | Txn::Bind(peer) => self.handle_peer_error(peer, datagram),
                Txn::Release => {}
            }
            TurnInbound::Control
        }

        // ── Outbound message builders / senders ──────────────────────────────

        /// Send an Allocate Request. Unauthenticated if we don't yet hold a
        /// REALM/NONCE; authenticated (USERNAME/NONCE/REALM/MESSAGE-INTEGRITY)
        /// once we do. A retransmission resends the same bytes.
        fn send_allocate(&mut self, udp: &UdpSocket, now: Instant) {
            let authed = self.have_credentials();
            if self.allocate_msg.is_none() || self.allocate_authed != authed {
                let txid = random_txid();
                let mut msg = begin_message(message_type(METHOD_ALLOCATE, CLASS_REQUEST), &txid);
                // REQUESTED-TRANSPORT = UDP (mandatory for Allocate).
                append_attr(&mut msg, ATTR_REQUESTED_TRANSPORT, &REQUESTED_TRANSPORT_UDP.to_be_bytes());
                // LIFETIME (optional hint).
                append_attr(&mut msg, ATTR_LIFETIME, &DEFAULT_LIFETIME_SECS.to_be_bytes());
                if authed {
                    self.append_auth(&mut msg);
                } else {
                    finalize_length(&mut msg);
                }
                self.allocate_msg = Some((txid, msg));
                self.allocate_authed = authed;
            }
            let Some((txid, msg)) = &self.allocate_msg else { return };
            self.pending.insert(*txid, (Txn::Allocate, now));
            if let Err(e) = udp.send_to(msg, self.server) {
                log::debug!("WebRTC TURN: Allocate send failed: {e}");
            } else {
                log::trace!("WebRTC TURN: sent Allocate ({}auth)", if authed { "" } else { "no-" });
            }
            self.last_allocate = Some(now);
        }

        /// Send a Refresh Request with `lifetime` seconds (0 lets the allocation
        /// go), authenticated.
        fn send_refresh(&mut self, udp: &UdpSocket, lifetime: u32, txn: Txn, now: Instant) {
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_REFRESH, CLASS_REQUEST), &txid);
            append_attr(&mut msg, ATTR_LIFETIME, &lifetime.to_be_bytes());
            self.append_auth(&mut msg);
            self.pending.insert(txid, (txn, now));
            if let Err(e) = udp.send_to(&msg, self.server) {
                log::debug!("WebRTC TURN: Refresh send failed: {e}");
            }
        }

        /// Send a CreatePermission Request for `peer` (authenticated).
        fn send_create_permission(&mut self, udp: &UdpSocket, peer: SocketAddr, now: Instant) {
            let txid = random_txid();
            let mut msg =
                begin_message(message_type(METHOD_CREATE_PERMISSION, CLASS_REQUEST), &txid);
            append_xor_peer_address(&mut msg, peer);
            self.append_auth(&mut msg);
            self.pending.insert(txid, (Txn::Permission(peer), now));
            let _ = udp.send_to(&msg, self.server);
        }

        /// Send a ChannelBind Request binding `peer` to its channel number
        /// (authenticated).
        fn send_channel_bind(&mut self, udp: &UdpSocket, peer: SocketAddr, now: Instant) {
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
            append_xor_peer_address(&mut msg, peer);
            self.append_auth(&mut msg);
            self.pending.insert(txid, (Txn::Bind(peer), now));
            let _ = udp.send_to(&msg, self.server);
            if let Some(p) = self.peers.get_mut(&peer) {
                p.last_bind = Some(now);
            }
        }

        /// Send `data` to `peer` as a Send indication. Indications are NOT
        /// authenticated (RFC 5766 §10): XOR-PEER-ADDRESS + DATA only.
        fn send_indication(&self, udp: &UdpSocket, peer: SocketAddr, data: &[u8]) {
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_SEND, CLASS_INDICATION), &txid);
            append_xor_peer_address(&mut msg, peer);
            append_attr(&mut msg, ATTR_DATA, data);
            finalize_length(&mut msg);
            if let Err(e) = udp.send_to(&msg, self.server) {
                log::trace!("WebRTC TURN: Send indication to {} failed: {e}", self.server);
            }
        }

        /// Send `data` on a bound channel: a 4-byte header and the raw data. Over
        /// UDP no padding is needed (RFC 5766 §11.5).
        fn send_channel_data(&self, udp: &UdpSocket, channel: u16, data: &[u8]) {
            let mut frame = Vec::with_capacity(4 + data.len());
            frame.extend_from_slice(&channel.to_be_bytes());
            frame.extend_from_slice(&(data.len() as u16).to_be_bytes());
            frame.extend_from_slice(data);
            if let Err(e) = udp.send_to(&frame, self.server) {
                log::trace!("WebRTC TURN: ChannelData send to {} failed: {e}", self.server);
            }
        }

        /// (Re)send ChannelBind for any peer whose bind is unconfirmed or due a
        /// refresh. Refused peers are left alone.
        fn refresh_channels(&mut self, udp: &UdpSocket, now: Instant) {
            let stale: Vec<SocketAddr> = self
                .peers
                .iter()
                .filter(|(_, p)| !p.refused)
                .filter(|(_, p)| match p.last_bind {
                    None => true,
                    Some(t) => {
                        let interval = if p.bound { BIND_REFRESH } else { super::TURN_ALLOC_RETRY_INTERVAL };
                        now.duration_since(t) >= interval
                    }
                })
                .map(|(addr, _)| *addr)
                .collect();
            for addr in stale {
                self.send_channel_bind(udp, addr, now);
            }
        }

        // ── Inbound response handlers ────────────────────────────────────────

        fn handle_allocate_success(&mut self, datagram: &[u8]) {
            // Pull XOR-RELAYED-ADDRESS + LIFETIME from the attributes.
            let mut relayed = None;
            let mut lifetime = None;
            for (atype, val) in iter_attrs(datagram) {
                match atype {
                    ATTR_XOR_RELAYED_ADDRESS => relayed = parse_xor_address(val),
                    ATTR_LIFETIME if val.len() >= 4 => {
                        lifetime = Some(u32::from_be_bytes([val[0], val[1], val[2], val[3]]));
                    }
                    _ => {}
                }
            }
            let Some(addr) = relayed else {
                log::debug!("WebRTC TURN: Allocate success lacked XOR-RELAYED-ADDRESS");
                self.phase = Phase::Failed(TurnFailure::Refused(0));
                return;
            };
            self.relayed = Some(addr);
            self.phase = Phase::Allocated;
            self.allocated_at = Some(Instant::now());
            self.allocate_msg = None;
            if let Some(lt) = lifetime {
                self.lifetime = Duration::from_secs(lt as u64);
            }
            log::info!("WebRTC TURN: allocated relay {addr}, lifetime {:?}", self.lifetime);
        }

        fn handle_allocate_error(&mut self, datagram: &[u8]) {
            let (code, realm, nonce) = parse_error_challenge(datagram);
            match code {
                // The challenge: take REALM and NONCE and ask again, signed, at
                // once. A 401 to a request that was already signed means the
                // credentials are wrong; asking again would only repeat it.
                Some(401) if !self.allocate_authed && realm.is_some() && nonce.is_some() => {
                    self.realm = realm;
                    self.nonce = nonce;
                    self.allocate_msg = None;
                    self.last_allocate = None;
                    log::trace!("WebRTC TURN: Allocate 401, captured realm/nonce, will retry authed");
                }
                // Stale nonce: take the new one and ask again.
                Some(438) if nonce.is_some() => {
                    self.nonce = nonce;
                    self.allocate_msg = None;
                    self.last_allocate = None;
                }
                other => {
                    log::warn!("WebRTC TURN: Allocate refused (error {other:?})");
                    self.phase = Phase::Failed(TurnFailure::Refused(other.unwrap_or(0)));
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

        fn handle_refresh_error(&mut self, datagram: &[u8]) {
            let (code, _realm, nonce) = parse_error_challenge(datagram);
            if code == Some(438) && nonce.is_some() {
                // Stale nonce: take the new one; the next drive refreshes again.
                self.nonce = nonce;
                self.last_refresh = None;
                log::trace!("WebRTC TURN: refresh stale-nonce (438), nonce refreshed");
            } else {
                // The allocation is gone, or the server will not keep it. A new
                // one would have a new relayed address that no peer knows, so
                // the call cannot carry on over it.
                log::info!("WebRTC TURN: refresh refused (error {code:?}); allocation lost");
                self.phase = Phase::Failed(TurnFailure::Lost(code.unwrap_or(0)));
            }
        }

        /// A CreatePermission or ChannelBind was refused. 403 is final (on the
        /// closed forwarder: not another allocation of the same room); a stale
        /// nonce is taken and the bind retried by `refresh_channels`.
        fn handle_peer_error(&mut self, peer: SocketAddr, datagram: &[u8]) {
            let (code, _realm, nonce) = parse_error_challenge(datagram);
            match code {
                Some(438) if nonce.is_some() => self.nonce = nonce,
                Some(403) => {
                    if let Some(pc) = self.peers.get_mut(&peer) {
                        pc.refused = true;
                        pc.bound = false;
                    }
                    log::debug!("WebRTC TURN: the server refused a permission toward {peer}");
                }
                other => log::trace!("WebRTC TURN: permission or channel error {other:?} for {peer}"),
            }
        }

        fn handle_data_indication(&mut self, datagram: &[u8]) -> TurnInbound {
            // A Data indication carries XOR-PEER-ADDRESS + DATA. We surface the
            // DATA sub-slice (by offset within `datagram`) and the peer address.
            let mut peer = None;
            let mut data_range = None;
            for (atype, off, len) in iter_attrs_with_offsets(datagram) {
                match atype {
                    ATTR_XOR_PEER_ADDRESS => peer = parse_xor_address(&datagram[off..off + len]),
                    ATTR_DATA => data_range = Some((off, len)),
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
                .find(|(_, p)| p.channel == channel && !p.refused)
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

        /// The long-term key, once the server has told us its realm.
        fn key(&self) -> Option<[u8; 16]> {
            Some(long_term_key(&self.username, self.realm.as_deref()?, &self.password))
        }

        /// Append USERNAME, NONCE, REALM, then MESSAGE-INTEGRITY (in the order of
        /// RFC 5769 §2.4's sample) to an in-progress message, and set its length.
        /// MESSAGE-INTEGRITY MUST be last (it covers everything before it).
        fn append_auth(&self, msg: &mut Vec<u8>) {
            let (realm, nonce) = match (&self.realm, &self.nonce) {
                (Some(r), Some(n)) => (r, n),
                _ => {
                    finalize_length(msg);
                    return; // no credentials yet: an unsigned request
                }
            };
            append_attr(msg, ATTR_USERNAME, self.username.as_bytes());
            append_attr(msg, ATTR_NONCE, nonce.as_bytes());
            append_attr(msg, ATTR_REALM, realm.as_bytes());
            let key = long_term_key(&self.username, realm, &self.password);
            append_message_integrity(msg, &key);
        }

        /// A client already holding `relayed` (as if the server had granted it),
        /// for tests of what the manager builds on top of it.
        #[cfg(test)]
        pub(crate) fn allocated_for_test(server: SocketAddr, relayed: SocketAddr) -> TurnClient {
            let mut c = TurnClient::new(server, "1:test".into(), "test".into());
            c.phase = Phase::Allocated;
            c.relayed = Some(relayed);
            c.allocated_at = Some(Instant::now());
            c.announced = true;
            c
        }
    }

    // ── Free helpers: message framing, attributes, XOR addresses ─────────────

    /// Begin a STUN/TURN message: 20-byte header with a placeholder length of 0
    /// (filled in by `finalize_length`).
    pub(crate) fn begin_message(msg_type: u16, txid: &[u8; 12]) -> Vec<u8> {
        let mut msg = Vec::with_capacity(64);
        msg.extend_from_slice(&msg_type.to_be_bytes());
        msg.extend_from_slice(&0u16.to_be_bytes()); // length placeholder
        msg.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        msg.extend_from_slice(txid);
        msg
    }

    /// Append one TLV attribute (type, length, value) with padding to a 4-byte
    /// boundary, and DOES NOT touch the header length (that's `finalize_length`).
    pub(crate) fn append_attr(msg: &mut Vec<u8>, atype: u16, value: &[u8]) {
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
    pub(crate) fn finalize_length(msg: &mut [u8]) {
        let attr_len = (msg.len() - 20) as u16;
        msg[2..4].copy_from_slice(&attr_len.to_be_bytes());
    }

    /// Append an XOR-PEER-ADDRESS attribute for `addr`.
    pub(crate) fn append_xor_peer_address(msg: &mut Vec<u8>, addr: SocketAddr) {
        append_xor_address(msg, ATTR_XOR_PEER_ADDRESS, addr);
    }

    /// Append an XOR-encoded address attribute of type `atype` (XOR-PEER-,
    /// XOR-RELAYED- or XOR-MAPPED-ADDRESS) per RFC 5389 §15.2 (IPv4):
    /// family(1, after 1 reserved), X-Port = port ^ (cookie>>16),
    /// X-Address = addr ^ cookie.
    pub(crate) fn append_xor_address(msg: &mut Vec<u8>, atype: u16, addr: SocketAddr) {
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
        append_attr(msg, atype, &val);
    }

    /// Parse an XOR-MAPPED/RELAYED/PEER-ADDRESS attribute value (IPv4 only; the
    /// IPv6 form would also XOR with the transaction id, which we never need).
    pub(crate) fn parse_xor_address(val: &[u8]) -> Option<SocketAddr> {
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
    pub(crate) fn parse_error_challenge(datagram: &[u8]) -> (Option<u16>, Option<String>, Option<String>) {
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
                ATTR_REALM => realm = String::from_utf8(val.to_vec()).ok(),
                ATTR_NONCE => nonce = String::from_utf8(val.to_vec()).ok(),
                _ => {}
            }
        }
        (code, realm, nonce)
    }

    /// Iterate (attr_type, value) over a STUN/TURN message's attribute region.
    pub(crate) fn iter_attrs(datagram: &[u8]) -> Vec<(u16, &[u8])> {
        iter_attrs_with_offsets(datagram)
            .into_iter()
            .map(|(t, off, len)| (t, &datagram[off..off + len]))
            .collect()
    }

    /// Iterate (attr_type, value_offset_within_datagram, value_len). The offset
    /// form is needed so callers can return DATA sub-slices by index.
    pub(crate) fn iter_attrs_with_offsets(datagram: &[u8]) -> Vec<(u16, usize, usize)> {
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
    /// (RFC 5389 §15.4: when there's no SASLprep, the raw bytes are used.)
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

    /// Check a received message's MESSAGE-INTEGRITY under `key`: `Some(true)`
    /// when it matches, `Some(false)` when it does not, `None` when the message
    /// carries none. The same byte ranges as `append_message_integrity`, read
    /// back: everything before the attribute, with the length field as it would
    /// be if the message ended right after it (a FINGERPRINT may follow).
    pub fn check_message_integrity(msg: &[u8], key: &[u8]) -> Option<bool> {
        use hmac::{Hmac, Mac};
        use sha1::Sha1;

        let (off, len) = iter_attrs_with_offsets(msg)
            .into_iter()
            .find(|(t, _, _)| *t == ATTR_MESSAGE_INTEGRITY)
            .map(|(_, off, len)| (off, len))?;
        if len != 20 {
            return Some(false);
        }
        let attr_start = off - 4;
        let mut head = msg[..attr_start].to_vec();
        let len_with_mi = ((attr_start - 20) + 24) as u16;
        head[2..4].copy_from_slice(&len_with_mi.to_be_bytes());
        let mut mac = Hmac::<Sha1>::new_from_slice(key).ok()?;
        mac.update(&head);
        Some(mac.verify_slice(&msg[off..off + 20]).is_ok())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Lock the STUN message-type bit interleaving for the TURN methods we
        /// use, against the RFC 5766 §13 / RFC 5389 §6 known values, and its
        /// inverse.
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
            for method in [METHOD_ALLOCATE, METHOD_REFRESH, METHOD_SEND, METHOD_DATA, METHOD_CREATE_PERMISSION, METHOD_CHANNEL_BIND, 0x0FFF] {
                for class in [CLASS_REQUEST, CLASS_INDICATION, CLASS_SUCCESS, CLASS_ERROR] {
                    assert_eq!(method_and_class(message_type(method, class)), (method, class));
                }
            }
        }

        /// RFC 5769 §2.4 "Sample Request with Long-Term Authentication", byte for
        /// byte: our `append_auth` (USERNAME, NONCE, REALM, MESSAGE-INTEGRITY over
        /// the MD5 long-term key) must produce exactly the RFC's message, and the
        /// receiving check must accept it and refuse it with one bit changed.
        ///   username = "\u{30DE}\u{30C8}\u{30EA}\u{30C3}\u{30AF}\u{30B9}" (SASLprep'd)
        ///   password = "TheMatrIX", realm = "example.org",
        ///   nonce    = "f//499k954d6OL34oL9FSTvy64sA"
        /// The sample is a Binding request (method 0x001) with no other
        /// attributes, so it exercises exactly the signing path every TURN
        /// request uses. Its MESSAGE-INTEGRITY was also recomputed with Node's
        /// crypto over these bytes (2026-10-09) before it was pinned here.
        ///
        /// Seen red 2026-10-09 with `append_auth` back in its old order
        /// (USERNAME, REALM, NONCE): "RFC 5769 §2.4 sample request".
        #[test]
        fn rfc5769_long_term_sample_request_byte_for_byte() {
            const RFC5769_2_4: [u8; 116] = [
                0x00, 0x01, 0x00, 0x60, 0x21, 0x12, 0xa4, 0x42, // type, length, cookie
                0x78, 0xad, 0x34, 0x33, 0xc6, 0xad, 0x72, 0xc0, 0x29, 0xda, 0x41, 0x2e, // txid
                0x00, 0x06, 0x00, 0x12, // USERNAME
                0xe3, 0x83, 0x9e, 0xe3, 0x83, 0x88, 0xe3, 0x83, 0xaa, 0xe3, 0x83, 0x83, 0xe3, 0x82,
                0xaf, 0xe3, 0x82, 0xb9, 0x00, 0x00,
                0x00, 0x15, 0x00, 0x1c, // NONCE
                0x66, 0x2f, 0x2f, 0x34, 0x39, 0x39, 0x6b, 0x39, 0x35, 0x34, 0x64, 0x36, 0x4f, 0x4c,
                0x33, 0x34, 0x6f, 0x4c, 0x39, 0x46, 0x53, 0x54, 0x76, 0x79, 0x36, 0x34, 0x73, 0x41,
                0x00, 0x14, 0x00, 0x0b, // REALM
                0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x6f, 0x72, 0x67, 0x00,
                0x00, 0x08, 0x00, 0x14, // MESSAGE-INTEGRITY
                0xf6, 0x70, 0x24, 0x65, 0x6d, 0xd6, 0x4a, 0x3e, 0x02, 0xb8, 0xe0, 0x71, 0x2e, 0x85,
                0xc9, 0xa2, 0x8c, 0xa8, 0x96, 0x66,
            ];
            let username = "\u{30DE}\u{30C8}\u{30EA}\u{30C3}\u{30AF}\u{30B9}";
            let mut client = TurnClient::new("192.0.2.1:3478".parse().unwrap(), username.into(), "TheMatrIX".into());
            client.realm = Some("example.org".into());
            client.nonce = Some("f//499k954d6OL34oL9FSTvy64sA".into());

            let mut txid = [0u8; 12];
            txid.copy_from_slice(&RFC5769_2_4[8..20]);
            let mut msg = begin_message(0x0001, &txid);
            client.append_auth(&mut msg);
            assert_eq!(&msg[..], &RFC5769_2_4[..], "RFC 5769 §2.4 sample request");

            let key = long_term_key(username, "example.org", "TheMatrIX");
            assert_eq!(check_message_integrity(&RFC5769_2_4, &key), Some(true));
            let mut flipped = RFC5769_2_4;
            flipped[30] ^= 0x01; // one bit of the username
            assert_eq!(check_message_integrity(&flipped, &key), Some(false));
            assert_eq!(check_message_integrity(&RFC5769_2_4[..96], &key), None, "no M-I attribute");
        }

        /// RFC 5769 §2.2 "Sample IPv4 Response": its MESSAGE-INTEGRITY (under the
        /// short-term password "VOkJxbRl1RmTxUk/WvJxBt", which is the key itself)
        /// must check out even with the FINGERPRINT that follows it, and its
        /// XOR-MAPPED-ADDRESS must read 192.0.2.1:32853. This is the path every
        /// response from the forwarder takes through `check_message_integrity`.
        ///
        /// Seen red 2026-10-09 with `check_message_integrity` hashing the length
        /// field as received (covering the FINGERPRINT): Some(false), not Some(true).
        #[test]
        fn rfc5769_ipv4_sample_response_checks_out() {
            const RFC5769_2_2: [u8; 80] = [
                0x01, 0x01, 0x00, 0x3c, 0x21, 0x12, 0xa4, 0x42, // type, length, cookie
                0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae, // txid
                0x80, 0x22, 0x00, 0x0b, // SOFTWARE
                0x74, 0x65, 0x73, 0x74, 0x20, 0x76, 0x65, 0x63, 0x74, 0x6f, 0x72, 0x20,
                0x00, 0x20, 0x00, 0x08, // XOR-MAPPED-ADDRESS
                0x00, 0x01, 0xa1, 0x47, 0xe1, 0x12, 0xa6, 0x43,
                0x00, 0x08, 0x00, 0x14, // MESSAGE-INTEGRITY
                0x2b, 0x91, 0xf5, 0x99, 0xfd, 0x9e, 0x90, 0xc3, 0x8c, 0x74, 0x89, 0xf9, 0x2a, 0xf9,
                0xba, 0x53, 0xf0, 0x6b, 0xe7, 0xd7,
                0x80, 0x28, 0x00, 0x04, // FINGERPRINT
                0xc0, 0x7d, 0x4c, 0x96,
            ];
            let key = b"VOkJxbRl1RmTxUk/WvJxBt";
            assert_eq!(check_message_integrity(&RFC5769_2_2, key), Some(true));
            assert_eq!(check_message_integrity(&RFC5769_2_2, b"wrong"), Some(false));
            let mapped = iter_attrs(&RFC5769_2_2)
                .into_iter()
                .find(|(t, _)| *t == 0x0020)
                .and_then(|(_, v)| parse_xor_address(v));
            assert_eq!(mapped, Some("192.0.2.1:32853".parse().unwrap()));
        }

        /// Lock the MESSAGE-INTEGRITY construction against an independent HMAC
        /// (Node's crypto, computed when this test was written): the HMAC-SHA1
        /// over the message bytes up to (not including) the M-I attribute, with
        /// the header length pre-patched to include the 24-byte M-I attribute.
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

            // (c) Independently recompute HMAC-SHA1 over msg[0..pre_len].
            let mut mac = Hmac::<Sha1>::new_from_slice(&key).unwrap();
            mac.update(&msg[0..pre_len]);
            let expected = mac.finalize().into_bytes();
            let appended_tag = &msg[pre_len + 4..pre_len + 4 + 20];
            assert_eq!(appended_tag, &expected[..], "HMAC-SHA1 over the pre-M-I bytes");

            // (d) Cross-language oracle (Node's crypto, same key and bytes).
            let node_tag: [u8; 20] = [
                0xcc, 0xea, 0x50, 0xf7, 0x7b, 0xe4, 0x1a, 0x1b, 0xeb, 0x3d, 0x60, 0x33, 0x51, 0x9c,
                0xab, 0x0b, 0xfd, 0x71, 0xb1, 0x73,
            ];
            assert_eq!(appended_tag, &node_tag[..], "HMAC-SHA1 matches the node reference");
            assert_eq!(check_message_integrity(&msg, &key), Some(true), "and reads back");
        }

        /// XOR-PEER-ADDRESS round-trip: encode an address, parse it back.
        #[test]
        fn xor_peer_address_roundtrip() {
            let addr: SocketAddr = "203.0.113.45:51234".parse().unwrap();
            let txid = random_txid();
            let mut msg = begin_message(message_type(METHOD_SEND, CLASS_INDICATION), &txid);
            append_xor_peer_address(&mut msg, addr);
            finalize_length(&mut msg);
            let attrs = iter_attrs(&msg);
            let (_, val) = attrs
                .iter()
                .find(|(t, _)| *t == ATTR_XOR_PEER_ADDRESS)
                .expect("XOR-PEER-ADDRESS present");
            assert_eq!(parse_xor_address(val), Some(addr), "XOR address round-trips");
        }

        /// ChannelData framing: our inbound parser recovers the peer + the exact
        /// inner byte range of a frame on a channel we assigned.
        #[test]
        fn channel_data_frame_and_parse() {
            let mut client = TurnClient::allocated_for_test("1.2.3.4:3478".parse().unwrap(), "5.6.7.8:9000".parse().unwrap());
            let peer: SocketAddr = "9.9.9.9:1111".parse().unwrap();
            client.peers.insert(peer, PeerChannel { channel: 0x4001, bound: true, last_bind: None, refused: false });

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
        /// ≤ 0x3F) is not; the wire discriminator must hold.
        #[test]
        fn channel_data_vs_stun_discriminator() {
            let stun_type = message_type(METHOD_ALLOCATE, CLASS_SUCCESS);
            assert!(stun_type <= 0x3FFF, "STUN message types are <= 0x3FFF");
            assert!((stun_type >> 8) as u8 <= 0x3F, "STUN first byte <= 0x3F");
            assert!((0x4000u16 >> 8) as u8 == 0x40);
            assert!((0x7FFFu16 >> 8) as u8 == 0x7F);
        }

        // ── The exchange against an in-test forwarder on 127.0.0.1 ───────────

        use crate::net::webrtc::test_forwarder::{Forwarder, Trap};
        use std::time::Duration;

        fn socket() -> UdpSocket {
            let s = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
            s.set_read_timeout(Some(Duration::from_millis(5))).unwrap();
            s
        }

        /// Drive each client and read whatever the forwarder sent it, until
        /// `done` holds or two seconds pass. Relayed data is collected per client.
        fn pump(
            clients: &mut [(&mut TurnClient, &UdpSocket, &mut Vec<(SocketAddr, Vec<u8>)>)],
            done: impl Fn(&[(&mut TurnClient, &UdpSocket, &mut Vec<(SocketAddr, Vec<u8>)>)]) -> bool,
        ) -> bool {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut buf = [0u8; 2048];
            while Instant::now() < deadline {
                for (client, udp, got) in clients.iter_mut() {
                    client.drive(udp);
                    while let Ok((n, src)) = udp.recv_from(&mut buf) {
                        if src != client.server_addr() {
                            continue;
                        }
                        if let TurnInbound::Data { peer, inner_offset, inner_len } = client.handle_from_server(udp, &buf[..n]) {
                            got.push((peer, buf[inner_offset..inner_offset + inner_len].to_vec()));
                        }
                    }
                }
                if done(clients) {
                    return true;
                }
            }
            false
        }

        /// The whole client against a small forwarder that speaks the same closed
        /// subset as the relay's (10f), on 127.0.0.1: two clients of one room
        /// allocate with long-term credentials, install permissions and channels
        /// for each other's relayed address, and carry data both ways, first in a
        /// Send indication and then as ChannelData. A permission toward an
        /// address that is not an allocation of the same room is refused, nothing
        /// is ever sent toward it, and no datagram reaches it. A client of another
        /// room is refused too.
        ///
        /// Seen red 2026-10-09 with `send_relayed` sending even to a refused
        /// address: "nothing goes toward a refused address" (the forwarder logged
        /// a Send toward the outside address).
        #[test]
        fn allocate_permission_and_send_through_a_forwarder_on_loopback() {
            let traps: Vec<Trap> = (0..4).map(|_| Trap::bind()).collect();
            let outside = Trap::bind();
            let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
            fwd.add_account("1:a", "pa", "r1");
            fwd.add_account("1:b", "pb", "r1");
            fwd.add_account("1:c", "pc", "r2");
            let (ua, ub, uc) = (socket(), socket(), socket());
            let mut a = TurnClient::new(fwd.addr, "1:a".into(), "pa".into());
            let mut b = TurnClient::new(fwd.addr, "1:b".into(), "pb".into());
            let mut c = TurnClient::new(fwd.addr, "1:c".into(), "pc".into());
            let (mut ga, mut gb, mut gc) = (Vec::new(), Vec::new(), Vec::new());

            {
                let mut all = [(&mut a, &ua, &mut ga), (&mut b, &ub, &mut gb), (&mut c, &uc, &mut gc)];
                assert!(pump(&mut all, |cs| cs.iter().all(|(c, ..)| c.relayed_addr().is_some())), "all three allocated");
            }
            let (ra, rb, rc) = (a.relayed_addr().unwrap(), b.relayed_addr().unwrap(), c.relayed_addr().unwrap());
            for r in [ra, rb, rc] {
                assert!(traps.iter().any(|t| t.addr == r), "a relayed address the forwarder gave out");
            }

            // Each of the room's two installs a permission and a channel for the other.
            a.ensure_peer(&ua, rb);
            b.ensure_peer(&ub, ra);
            // ... and one toward an address that is not an allocation, one toward another room.
            a.ensure_peer(&ua, outside.addr);
            c.ensure_peer(&uc, ra);
            {
                let mut ab = [(&mut a, &ua, &mut ga), (&mut b, &ub, &mut gb), (&mut c, &uc, &mut gc)];
                assert!(
                    pump(&mut ab, |cs| {
                        cs[0].0.peers.get(&rb).is_some_and(|p| p.bound)
                            && cs[1].0.peers.get(&ra).is_some_and(|p| p.bound)
                            && cs[0].0.peers.get(&outside.addr).is_some_and(|p| p.refused)
                            && cs[2].0.peers.get(&ra).is_some_and(|p| p.refused)
                    }),
                    "permissions within the room granted, the others refused"
                );
            }

            // A Send indication from a to b, then ChannelData both ways.
            a.send_indication(&ua, rb, b"by indication");
            a.send_relayed(&ua, rb, b"by channel");
            b.send_relayed(&ub, ra, b"and back");
            // Nothing goes toward a refused address.
            a.send_relayed(&ua, outside.addr, b"never");
            c.send_relayed(&uc, ra, b"never either");
            {
                let mut ab = [(&mut a, &ua, &mut ga), (&mut b, &ub, &mut gb)];
                assert!(pump(&mut ab, |cs| cs[1].2.len() >= 2 && !cs[0].2.is_empty()), "the data came through");
            }
            assert!(gb.contains(&(ra, b"by indication".to_vec())), "{gb:?}");
            assert!(gb.contains(&(ra, b"by channel".to_vec())), "{gb:?}");
            assert_eq!(ga, vec![(rb, b"and back".to_vec())]);
            assert!(gc.is_empty());
            let refused = fwd.refused();
            assert!(
                refused.iter().all(|r| r.starts_with("CreatePermission") || r.starts_with("ChannelBind")),
                "nothing goes toward a refused address: {refused:?}"
            );
            assert!(!refused.is_empty(), "the forwarder did refuse");
            for t in traps.iter().chain([&outside]) {
                assert!(t.received().is_empty(), "no datagram reached {} directly", t.addr);
            }
        }

        /// Wrong credentials and a silent server both end in a failure the
        /// manager can say, rather than a retry loop. Seen red 2026-10-09 with the
        /// `!self.allocate_authed` guard taken off the 401 arm: the client asked
        /// again and again and the first assertion timed out.
        #[test]
        fn wrong_credentials_and_a_silent_server_fail() {
            let fwd = Forwarder::start(vec![Trap::bind().addr]);
            fwd.add_account("1:a", "right", "r1");
            let ua = socket();
            let mut wrong = TurnClient::new(fwd.addr, "1:a".into(), "wrong".into());
            let mut got = Vec::new();
            {
                let mut one = [(&mut wrong, &ua, &mut got)];
                assert!(pump(&mut one, |cs| cs[0].0.failure().is_some()), "a 401 to signed credentials ends it");
            }
            assert_eq!(wrong.failure(), Some(TurnFailure::Refused(401)));

            let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
            let ub = socket();
            let mut lonely = TurnClient::new(silent.local_addr().unwrap(), "1:a".into(), "right".into());
            lonely.give_up = Duration::from_millis(300);
            {
                let mut one = [(&mut lonely, &ub, &mut got)];
                assert!(pump(&mut one, |cs| cs[0].0.failure().is_some()), "no answer gives up");
            }
            assert_eq!(lonely.failure(), Some(TurnFailure::NoAnswer));
            assert!(lonely.next_deadline().is_none());
        }

        /// The Allocate is resent with the SAME transaction id while unanswered
        /// (RFC 5389 §7.2.1), so a lost success is answered again from the
        /// server's cache rather than refused with 437.
        #[test]
        fn an_unanswered_allocate_is_resent_unchanged() {
            let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
            silent.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
            let udp = socket();
            let mut client = TurnClient::new(silent.local_addr().unwrap(), "1:a".into(), "p".into());
            client.drive(&udp);
            client.last_allocate = Some(Instant::now() - super::super::TURN_ALLOC_RETRY_INTERVAL);
            client.drive(&udp);
            let mut buf = [0u8; 512];
            let (n1, _) = silent.recv_from(&mut buf).expect("first Allocate");
            let first = buf[..n1].to_vec();
            let (n2, _) = silent.recv_from(&mut buf).expect("second Allocate");
            assert_eq!(&buf[..n2], &first[..], "the retransmission is the same request");
        }
    }
}

/// A small closed TURN forwarder for tests, on 127.0.0.1 only (step E, 10f): the subset of
/// RFC 5766 the relay's own forwarder speaks, written apart from the client so each checks the
/// other. It checks MESSAGE-INTEGRITY with its own HMAC code, gives out relayed addresses from a
/// pool of [`Trap`] sockets the test owns (so a datagram anyone sends straight to a relayed
/// address, around the forwarder, is caught), passes data only between two allocations of the
/// same room that each hold a permission for the other, never sends anywhere else, and records
/// every refusal.
#[cfg(test)]
pub(crate) mod test_forwarder {
    use super::turn::*;
    use std::collections::{HashMap, HashSet};
    use std::net::{SocketAddr, UdpSocket};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::Duration;

    /// The realm the relay's forwarder uses (10f).
    pub const REALM: &str = "humanityos";
    const NONCE: &str = "test-nonce";

    /// A socket standing at a relayed address, so anything sent straight to it is seen.
    pub struct Trap {
        pub addr: SocketAddr,
        sock: UdpSocket,
    }

    impl Trap {
        pub fn bind() -> Trap {
            let sock = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
            sock.set_nonblocking(true).unwrap();
            Trap { addr: sock.local_addr().unwrap(), sock }
        }

        /// Who sent each datagram that arrived here.
        pub fn received(&self) -> Vec<SocketAddr> {
            let mut out = Vec::new();
            let mut buf = [0u8; 2048];
            while let Ok((_, from)) = self.sock.recv_from(&mut buf) {
                out.push(from);
            }
            out
        }
    }

    struct Account {
        password: String,
        room: String,
    }

    struct Alloc {
        room: String,
        relayed: SocketAddr,
        key: [u8; 16],
        perms: HashSet<SocketAddr>,
        channels: HashMap<u16, SocketAddr>,
    }

    #[derive(Default)]
    struct State {
        accounts: HashMap<String, Account>,
        allocs: HashMap<SocketAddr, Alloc>,
        pool: Vec<SocketAddr>,
        refused: Vec<String>,
        delivered: usize,
    }

    pub struct Forwarder {
        pub addr: SocketAddr,
        state: Arc<Mutex<State>>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl Forwarder {
        /// Start one on 127.0.0.1, giving out relayed addresses from `pool`.
        pub fn start(pool: Vec<SocketAddr>) -> Forwarder {
            let sock = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
            sock.set_read_timeout(Some(Duration::from_millis(10))).unwrap();
            let addr = sock.local_addr().unwrap();
            let mut pool = pool;
            pool.reverse(); // given out in the order the test listed them
            let state = Arc::new(Mutex::new(State { pool, ..Default::default() }));
            let stop = Arc::new(AtomicBool::new(false));
            let (st, halt) = (state.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while !halt.load(Ordering::Relaxed) {
                    if let Ok((n, src)) = sock.recv_from(&mut buf) {
                        let mut s = st.lock().unwrap();
                        handle(&sock, &mut s, src, &buf[..n]);
                    }
                }
            });
            Forwarder { addr, state, stop, thread: Some(thread) }
        }

        /// Issue credentials for `room`.
        pub fn add_account(&self, username: &str, password: &str, room: &str) {
            self.state.lock().unwrap().accounts.insert(
                username.to_string(),
                Account { password: password.to_string(), room: room.to_string() },
            );
        }

        /// Every refusal so far, in words.
        pub fn refused(&self) -> Vec<String> {
            self.state.lock().unwrap().refused.clone()
        }

        /// How many datagrams were passed from one allocation to another.
        pub fn delivered(&self) -> usize {
            self.state.lock().unwrap().delivered
        }
    }

    impl Drop for Forwarder {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    fn handle(sock: &UdpSocket, st: &mut State, src: SocketAddr, msg: &[u8]) {
        if msg.is_empty() {
            return;
        }
        if (0x40..=0x7F).contains(&msg[0]) {
            // ChannelData on one of the sender's bound channels.
            if msg.len() < 4 {
                return;
            }
            let ch = u16::from_be_bytes([msg[0], msg[1]]);
            let len = u16::from_be_bytes([msg[2], msg[3]]) as usize;
            if 4 + len > msg.len() {
                return;
            }
            let Some(a) = st.allocs.get(&src) else { return };
            let Some(&peer) = a.channels.get(&ch) else {
                st.refused.push(format!("ChannelData on unbound channel {ch:#06x}"));
                return;
            };
            let (from, room) = (a.relayed, a.room.clone());
            deliver(sock, st, &room, from, peer, &msg[4..4 + len]);
            return;
        }
        if msg.len() < 20 {
            return;
        }
        let (method, class) = method_and_class(u16::from_be_bytes([msg[0], msg[1]]));
        let mut txid = [0u8; 12];
        txid.copy_from_slice(&msg[8..20]);
        let attrs = iter_attrs(msg);
        let attr = |t: u16| attrs.iter().find(|(a, _)| *a == t).map(|(_, v)| *v);

        if class == CLASS_INDICATION && method == METHOD_SEND {
            let Some(a) = st.allocs.get(&src) else { return };
            let (Some(peer), Some(data)) = (attr(ATTR_XOR_PEER_ADDRESS).and_then(parse_xor_address), attr(ATTR_DATA)) else {
                return;
            };
            if !a.perms.contains(&peer) {
                st.refused.push(format!("Send toward {peer} without a permission"));
                return;
            }
            let (from, room) = (a.relayed, a.room.clone());
            deliver(sock, st, &room, from, peer, data);
            return;
        }
        if class != CLASS_REQUEST {
            return;
        }
        match method {
            METHOD_ALLOCATE => {
                let Some(user) = attr(ATTR_USERNAME).and_then(|u| std::str::from_utf8(u).ok()).map(str::to_string) else {
                    return reply_error(sock, src, method, &txid, 401, None, true);
                };
                let Some(acct) = st.accounts.get(&user) else {
                    st.refused.push("Allocate by an unknown user".into());
                    return reply_error(sock, src, method, &txid, 401, None, true);
                };
                let key = long_term_key(&user, REALM, &acct.password);
                if !integrity_ok(msg, &key) {
                    st.refused.push("Allocate with wrong credentials".into());
                    return reply_error(sock, src, method, &txid, 401, None, true);
                }
                if st.allocs.contains_key(&src) {
                    return reply_error(sock, src, method, &txid, 437, Some(&key), false);
                }
                let Some(relayed) = st.pool.pop() else {
                    return reply_error(sock, src, method, &txid, 508, Some(&key), false);
                };
                let room = acct.room.clone();
                st.allocs.insert(src, Alloc { room, relayed, key, perms: HashSet::new(), channels: HashMap::new() });
                let mut out = begin_message(message_type(METHOD_ALLOCATE, CLASS_SUCCESS), &txid);
                append_xor_address(&mut out, ATTR_XOR_RELAYED_ADDRESS, relayed);
                append_xor_address(&mut out, 0x0020, src); // XOR-MAPPED-ADDRESS
                append_attr(&mut out, ATTR_LIFETIME, &600u32.to_be_bytes());
                append_message_integrity(&mut out, &key);
                let _ = sock.send_to(&out, src);
            }
            METHOD_REFRESH | METHOD_CREATE_PERMISSION | METHOD_CHANNEL_BIND => {
                let Some(a) = st.allocs.get(&src) else {
                    return reply_error(sock, src, method, &txid, 437, None, false);
                };
                let key = a.key;
                if !integrity_ok(msg, &key) {
                    st.refused.push(format!("method {method:#05x} with wrong credentials"));
                    return reply_error(sock, src, method, &txid, 401, None, true);
                }
                if method == METHOD_REFRESH {
                    let lifetime = attr(ATTR_LIFETIME)
                        .filter(|v| v.len() >= 4)
                        .map(|v| u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
                        .unwrap_or(600);
                    if lifetime == 0 {
                        st.allocs.remove(&src);
                    }
                    let mut out = begin_message(message_type(METHOD_REFRESH, CLASS_SUCCESS), &txid);
                    append_attr(&mut out, ATTR_LIFETIME, &lifetime.to_be_bytes());
                    append_message_integrity(&mut out, &key);
                    let _ = sock.send_to(&out, src);
                    return;
                }
                let Some(peer) = attr(ATTR_XOR_PEER_ADDRESS).and_then(parse_xor_address) else {
                    return reply_error(sock, src, method, &txid, 400, Some(&key), false);
                };
                let (room, me) = (a.room.clone(), a.relayed);
                let same_room = st.allocs.values().any(|b| b.relayed == peer && b.room == room && b.relayed != me);
                let name = if method == METHOD_CHANNEL_BIND { "ChannelBind" } else { "CreatePermission" };
                if !same_room {
                    st.refused.push(format!("{name} toward {peer}"));
                    return reply_error(sock, src, method, &txid, 403, Some(&key), false);
                }
                let a = st.allocs.get_mut(&src).expect("looked up above");
                a.perms.insert(peer);
                if method == METHOD_CHANNEL_BIND {
                    if let Some(v) = attr(ATTR_CHANNEL_NUMBER).filter(|v| v.len() >= 2) {
                        a.channels.insert(u16::from_be_bytes([v[0], v[1]]), peer);
                    }
                }
                let mut out = begin_message(message_type(method, CLASS_SUCCESS), &txid);
                append_message_integrity(&mut out, &key);
                let _ = sock.send_to(&out, src);
            }
            _ => {}
        }
    }

    /// Pass `data` from the allocation at `from` to the one at `to`, inside the forwarder: only
    /// to another allocation of `room` that holds a permission for `from`. Nothing is ever sent
    /// to an address that is not a client of the forwarder.
    fn deliver(sock: &UdpSocket, st: &mut State, room: &str, from: SocketAddr, to: SocketAddr, data: &[u8]) {
        let target = st.allocs.iter().find(|(_, b)| b.relayed == to && b.room == room).map(|(client, b)| {
            let channel = b.channels.iter().find(|(_, p)| **p == from).map(|(c, _)| *c);
            (*client, b.perms.contains(&from), channel)
        });
        let Some((client, permitted, channel)) = target else {
            st.refused.push(format!("data toward {to}, not an allocation of the room"));
            return;
        };
        if !permitted {
            st.refused.push(format!("data from {from} without the receiver's permission"));
            return;
        }
        let out = match channel {
            Some(c) => {
                let mut f = Vec::with_capacity(4 + data.len());
                f.extend_from_slice(&c.to_be_bytes());
                f.extend_from_slice(&(data.len() as u16).to_be_bytes());
                f.extend_from_slice(data);
                f
            }
            None => {
                let mut m = begin_message(message_type(METHOD_DATA, CLASS_INDICATION), &[7u8; 12]);
                append_xor_peer_address(&mut m, from);
                append_attr(&mut m, ATTR_DATA, data);
                finalize_length(&mut m);
                m
            }
        };
        let _ = sock.send_to(&out, client);
        st.delivered += 1;
    }

    fn reply_error(sock: &UdpSocket, to: SocketAddr, method: u16, txid: &[u8; 12], code: u16, key: Option<&[u8; 16]>, challenge: bool) {
        let mut out = begin_message(message_type(method, CLASS_ERROR), txid);
        let mut val = vec![0, 0, (code / 100) as u8, (code % 100) as u8];
        val.extend_from_slice(b"refused");
        append_attr(&mut out, ATTR_ERROR_CODE, &val);
        if challenge {
            append_attr(&mut out, ATTR_REALM, REALM.as_bytes());
            append_attr(&mut out, ATTR_NONCE, NONCE.as_bytes());
        }
        match key {
            Some(k) => append_message_integrity(&mut out, k),
            None => finalize_length(&mut out),
        }
        let _ = sock.send_to(&out, to);
    }

    /// The forwarder's own MESSAGE-INTEGRITY check, written apart from the client's
    /// `check_message_integrity` so one cannot hide a mistake in the other.
    fn integrity_ok(msg: &[u8], key: &[u8]) -> bool {
        use hmac::{Hmac, Mac};
        use sha1::Sha1;
        let end = (20 + u16::from_be_bytes([msg[2], msg[3]]) as usize).min(msg.len());
        let mut off = 20;
        while off + 4 <= end {
            let t = u16::from_be_bytes([msg[off], msg[off + 1]]);
            let l = u16::from_be_bytes([msg[off + 2], msg[off + 3]]) as usize;
            if t == 0x0008 {
                if l != 20 || off + 24 > msg.len() {
                    return false;
                }
                let mut head = msg[..off].to_vec();
                head[2..4].copy_from_slice(&((off - 20 + 24) as u16).to_be_bytes());
                let mut mac = Hmac::<Sha1>::new_from_slice(key).unwrap();
                mac.update(&head);
                return mac.verify_slice(&msg[off + 4..off + 24]).is_ok();
            }
            off += 4 + ((l + 3) & !3);
        }
        false
    }
}

/// Who this device answers, and when it asks a STUN server for its address
/// (2026-10-09, docs/design/blocking-and-safe-mode.md defect 3.7.2 and section
/// 7.1 item 1). Everything here stays on this machine: the sockets are bound to
/// 127.0.0.1 and the one "STUN server" is a socket of the test's own.
#[cfg(test)]
mod who_we_answer_tests {
    use super::*;

    const ME: &str = "bb";
    const STRANGER: &str = "aa";

    /// A manager as `start` builds one, without the thread, so a test can call
    /// its methods directly and read what it queued.
    fn manager() -> (WebrtcManager, Receiver<String>) {
        let (mgr, outbound, _events) = test_manager(ME);
        (mgr, outbound)
    }

    /// A real data-channel offer, as another app would send it (`data` is the
    /// JSON string the signaling contract carries).
    fn an_offer() -> Value {
        let mut rtc = Rtc::builder().build(Instant::now());
        let mut api = rtc.sdp_api();
        let _ = api.add_channel(CHANNEL_LABEL.to_string());
        let (offer, _pending) = api.apply().expect("offer produced");
        Value::String(serde_json::to_string(&offer).unwrap())
    }

    /// Seen red 2026-10-09 with `cmd_signal` answering an offer that came with
    /// no reason (as every offer was answered before): "an offer with no
    /// reason to connect must get no answer".
    ///
    /// Step E (same day): while calls hide this device's address, no reason but
    /// our own device lets an offer through (and `on_offer` still refuses that
    /// one), matching the web. Seen red with `answer_while_hiding_address`
    /// answering every reason, as before step E: "a GroupMember offer is not
    /// answered in this mode". The answering itself is kept for the later
    /// opt-in to direct connections, and checked through `on_offer`.
    #[test]
    fn a_strangers_offer_gets_no_answer_and_in_this_mode_nobody_elses_does() {
        let (mut mgr, outbound) = manager();
        // An offer handed in with no reason: the old path answered it.
        mgr.cmd_signal(STRANGER.into(), "dc_offer".into(), an_offer(), None);
        assert!(outbound.try_recv().is_err(), "an offer with no reason to connect must get no answer");
        assert!(mgr.peers.is_empty(), "and no connection may be started for it");

        for reason in [OfferReason::GroupMember, OfferReason::Friend, OfferReason::CallOrRoom, OfferReason::AskedByUs] {
            mgr.cmd_signal(STRANGER.into(), "dc_offer".into(), an_offer(), Some(reason));
            assert!(outbound.try_recv().is_err(), "a {reason:?} offer is not answered in this mode");
        }
        assert!(mgr.peers.is_empty(), "no direct connection was started");
        assert!(answer_while_hiding_address(OfferReason::OwnDevice), "own-device sync is left as it was");
        mgr.cmd_signal(ME.into(), "dc_offer".into(), an_offer(), Some(OfferReason::OwnDevice));
        assert!(outbound.try_recv().is_err(), "and on_offer still refuses our own key");

        // The answering machinery itself, for the opt-in step.
        mgr.on_offer(STRANGER.into(), an_offer().as_str().unwrap(), OfferReason::GroupMember);
        let answer = outbound.try_recv().expect("on_offer answers");
        assert!(answer.contains("\"dc_answer\""), "{answer}");
        assert!(mgr.peers.contains_key(STRANGER));
    }

    /// Seen red 2026-10-09 with `direct_offer_reason` ending in an answer for
    /// everyone left over: "a stranger is ignored", left: Some(Friend).
    #[test]
    fn only_people_we_have_a_reason_to_connect_to_are_answered() {
        let group = ["g1", "g2"];
        let room = ["r1"];
        let facts = OfferFacts {
            my_key: ME,
            sender_is_friend: false,
            group_members: group.to_vec(),
            call_peer: Some("c1"),
            voice_room_peers: room.to_vec(),
            asked_peer: Some("t1"),
        };
        assert_eq!(direct_offer_reason(STRANGER, &facts), None, "a stranger is ignored");
        assert_eq!(direct_offer_reason("", &facts), None, "no sender, no answer");
        assert_eq!(direct_offer_reason(ME, &facts), Some(OfferReason::OwnDevice));
        assert_eq!(direct_offer_reason("g2", &facts), Some(OfferReason::GroupMember));
        assert_eq!(direct_offer_reason("c1", &facts), Some(OfferReason::CallOrRoom));
        assert_eq!(direct_offer_reason("r1", &facts), Some(OfferReason::CallOrRoom));
        assert_eq!(direct_offer_reason("t1", &facts), Some(OfferReason::AskedByUs));
        let friend = OfferFacts { my_key: ME, sender_is_friend: true, ..Default::default() };
        assert_eq!(direct_offer_reason(STRANGER, &friend), Some(OfferReason::Friend));
        // Being in a call or a room lets in THAT person, nobody else.
        let in_call = OfferFacts { my_key: ME, call_peer: Some("c1"), ..Default::default() };
        assert_eq!(direct_offer_reason("c2", &in_call), None);
    }

    /// Seen red 2026-10-09 with `holds_friendship` reduced to "holds a
    /// certificate": "unfollowed: their certificate alone is not enough".
    /// Passes v2 (same day): a pass names its server, so one given on another
    /// server is not a friendship here.
    #[test]
    fn friendship_needs_our_follow_and_their_certificate_naming_us() {
        use crate::relay::core::pq_crypto::{build_friend_cert, derive_dilithium_seed, DilithiumKeypair, FRIEND_PASS_DEFAULT_MAY};
        let key_of = |seed: &[u8; 32]| hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(seed)).public_key());
        let (them_seed, me_seed, other_seed) = ([0x31u8; 32], [0x32u8; 32], [0x33u8; 32]);
        let (them, me, other) = (key_of(&them_seed), key_of(&me_seed), key_of(&other_seed));
        let (here, elsewhere) = ("did:hum:here", "did:hum:elsewhere");
        let serial = "00112233445566778899aabbccddeeff";
        let pass = |seed: &[u8; 32], from: &str, to: &str, server: &str| {
            build_friend_cert(seed, server, from, to, serial, &FRIEND_PASS_DEFAULT_MAY).unwrap()
        };
        let to_me = pass(&them_seed, &them, &me, here);
        assert!(holds_friendship(true, Some(&to_me), here, &them, &me), "we follow them and hold their certificate");
        assert!(!holds_friendship(false, Some(&to_me), here, &them, &me), "unfollowed: their certificate alone is not enough");
        assert!(!holds_friendship(true, None, here, &them, &me), "no certificate yet");
        assert!(!holds_friendship(true, Some(&to_me), elsewhere, &them, &me), "a pass given on another server");
        let to_other = pass(&them_seed, &them, &other, here);
        assert!(!holds_friendship(true, Some(&to_other), here, &them, &me), "a certificate naming someone else");
        let from_other = pass(&other_seed, &other, &me, here);
        assert!(!holds_friendship(true, Some(&from_other), here, &them, &me), "someone else's certificate passed off as theirs");
    }

    /// Seen red 2026-10-09 with the connection count dropped from
    /// `stun_request_due` (the old rule, "until the address is known"):
    /// "connected to a chat server, no call: no STUN".
    #[test]
    fn stun_is_asked_only_while_a_connection_is_being_made() {
        let now = Instant::now();
        assert!(!stun_request_due(false, 0, None, now), "connected to a chat server, no call: no STUN");
        assert!(stun_request_due(false, 1, None, now), "the first connection asks at once");
        let just_now = now - Duration::from_millis(500);
        assert!(!stun_request_due(false, 1, Some(just_now), now), "not again within the retry interval");
        let a_while_ago = now - STUN_RETRY_INTERVAL;
        assert!(stun_request_due(false, 1, Some(a_while_ago), now), "retried when no answer came");
        assert!(!stun_request_due(true, 3, None, now), "the address is known: never again");
    }

    /// The same rule through the manager: with a connection-less manager the
    /// "STUN server" (a socket of this test's) hears nothing; once an offer is
    /// answered, it receives a Binding Request. Seen red 2026-10-09 with the
    /// same change to `stun_request_due`: "no connection, no request".
    ///
    /// Step E: a voice connection (relay only) never asks STUN, so it does not
    /// count; seen red with the `!p.relay_only()` filter taken out of
    /// `maybe_send_stun`: "a call through the server asks no STUN". The direct
    /// connection is made through `on_offer`, the machinery the later opt-in
    /// step switches back on (this step answers no direct offer).
    #[test]
    fn the_manager_sends_no_stun_until_it_answers_an_offer() {
        let reflector = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
        reflector.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        let (mut mgr, _outbound) = manager();
        // Filled in so no hostname is looked up and nothing leaves the machine.
        mgr.stun_servers = vec![reflector.local_addr().unwrap()];
        let mut buf = [0u8; 128];

        mgr.maybe_send_stun();
        assert!(mgr.stun_pending.is_empty() && mgr.last_stun_send.is_none(), "no connection, no request");
        assert!(reflector.recv_from(&mut buf).is_err(), "nothing reached the STUN server");

        relay_only_tests::ready_in_room(&mut mgr, "r1");
        mgr.cmd_offer_to("cc".into(), true, Some("r1".into()));
        assert!(mgr.peers.contains_key("cc"), "a voice connection exists");
        mgr.maybe_send_stun();
        assert!(mgr.stun_pending.is_empty() && mgr.last_stun_send.is_none(), "a call through the server asks no STUN");

        mgr.on_offer(STRANGER.into(), an_offer().as_str().unwrap(), OfferReason::AskedByUs);
        mgr.maybe_send_stun();
        assert_eq!(mgr.stun_pending.len(), 1, "one request per server once a connection exists");
        let (n, _) = reflector.recv_from(&mut buf).expect("the STUN server got a request");
        assert!(n >= 20 && buf[0] == 0x00 && buf[1] == 0x01, "a STUN Binding Request");
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

/// Phase B/C voice transport verified IN-PROCESS (the #3 dev-infra build). str0m
/// is sans-IO, so two instances can be wired together in one test: each one's
/// Output::Transmit is fed straight into the other's handle_input(Receive), with
/// a shared fake clock and no sockets. This drives a full ICE + DTLS handshake
/// and asserts that an Opus frame written into one peer arrives at the other as
/// Event::MediaData. It makes the voice media path CI-verifiable without a live
/// native<->web call.
#[cfg(test)]
mod inproc_webrtc_tests {
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    use str0m::format::Codec;
    use str0m::media::{Direction, Frequency, MediaKind, MediaTime, Mid};
    use str0m::net::{Protocol, Receive, Transmit};
    use str0m::{Candidate, Event, Input, Output, Rtc};

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    /// Drain one Rtc's poll_output to its next Timeout, collecting outbound
    /// datagrams + events. Returns the deadline str0m wants to be polled at next.
    fn drain(rtc: &mut Rtc, transmits: &mut Vec<Transmit>, events: &mut Vec<Event>) -> Instant {
        loop {
            match rtc.poll_output().expect("poll_output") {
                Output::Timeout(t) => return t,
                Output::Transmit(t) => transmits.push(t),
                Output::Event(e) => events.push(e),
            }
        }
    }

    /// Feed a batch of datagrams into a peer as if they arrived over UDP.
    fn feed(dst: &mut Rtc, now: Instant, transmits: Vec<Transmit>) {
        for t in transmits {
            let bytes: &[u8] = &t.contents;
            let contents: Result<str0m::net::DatagramRecv, _> = bytes.try_into();
            if let Ok(contents) = contents {
                let _ = dst.handle_input(Input::Receive(
                    now,
                    Receive {
                        proto: Protocol::Udp,
                        source: t.source,
                        destination: t.destination,
                        contents,
                    },
                ));
            }
        }
    }

    fn ice_connected(events: &[Event]) -> bool {
        use str0m::IceConnectionState as S;
        events.iter().any(|e| matches!(e, Event::IceConnectionStateChange(S::Connected | S::Completed)))
    }

    #[test]
    fn two_str0m_opus_roundtrip() {
        let a_addr = addr("1.1.1.1:1000");
        let b_addr = addr("2.2.2.2:2000");
        let mut now = Instant::now();

        let mut a = Rtc::builder().build(now);
        let mut b = Rtc::builder().build(now);
        a.add_local_candidate(Candidate::host(a_addr, "udp").unwrap());
        b.add_local_candidate(Candidate::host(b_addr, "udp").unwrap());

        // A offers a SendRecv Opus audio m-line; B accepts; A applies the answer.
        let mut api = a.sdp_api();
        let a_mid: Mid = api.add_media(MediaKind::Audio, Direction::SendRecv, None, None, None);
        let (offer, pending) = api.apply().expect("offer");
        let answer = b.sdp_api().accept_offer(offer).expect("accept_offer");
        a.sdp_api().accept_answer(pending, answer).expect("accept_answer");

        let payload: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
        let mut b_mid: Option<Mid> = None;
        let mut a_conn = false;
        let mut b_conn = false;
        let mut wrote = false;
        let mut rtp_ts: u64 = 0;
        let mut got_media: Option<Vec<u8>> = None;

        for _ in 0..5000 {
            let mut a_tx = Vec::new();
            let mut b_tx = Vec::new();
            let mut a_ev = Vec::new();
            let mut b_ev = Vec::new();
            let a_to = drain(&mut a, &mut a_tx, &mut a_ev);
            let b_to = drain(&mut b, &mut b_tx, &mut b_ev);
            feed(&mut b, now, a_tx);
            feed(&mut a, now, b_tx);

            a_conn |= ice_connected(&a_ev);
            b_conn |= ice_connected(&b_ev);
            for e in &b_ev {
                if let Event::MediaAdded(m) = e {
                    if m.kind == MediaKind::Audio {
                        b_mid = Some(m.mid);
                    }
                }
                if let Event::MediaData(d) = e {
                    got_media = Some(d.data.to_vec());
                }
            }

            // Once both ends are connected and B has its audio mid, write one
            // Opus frame from A. (We only need a single direction for the proof.)
            if a_conn && b_conn && b_mid.is_some() && !wrote {
                if let Some(writer) = a.writer(a_mid) {
                    // Compute the negotiated Opus PT first so the payload_params
                    // borrow ends before write() consumes the writer.
                    let pt = writer
                        .payload_params()
                        .find(|p| p.spec().codec == Codec::Opus)
                        .map(|p| p.pt());
                    if let Some(pt) = pt {
                        writer
                            .write(pt, now, MediaTime::new(rtp_ts, Frequency::FORTY_EIGHT_KHZ), payload.clone())
                            .expect("write opus");
                        rtp_ts += 960;
                        wrote = true;
                    }
                }
            }

            if got_media.is_some() {
                break;
            }

            // Advance the shared clock to the earliest deadline, always moving
            // forward at least 1 ms so DTLS/ICE timers actually fire.
            now = a_to.min(b_to).max(now + Duration::from_millis(1));
            let _ = a.handle_input(Input::Timeout(now));
            let _ = b.handle_input(Input::Timeout(now));
        }

        assert!(a_conn && b_conn, "ICE never connected (a={a_conn} b={b_conn})");
        assert!(wrote, "never reached a state where we could write Opus");
        let got = got_media.expect("B never received the Opus frame as MediaData");
        assert_eq!(got, payload, "received Opus payload did not match what was sent");
    }
}

/// A manager as `start` builds one, without the thread, on 127.0.0.1, so a
/// test can call its methods directly and read what it queued and emitted.
#[cfg(test)]
fn test_manager(my_key: &str) -> (WebrtcManager, Receiver<String>, Receiver<WebrtcEvent>) {
    let udp = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
    let local_addr = udp.local_addr().unwrap();
    let (_tx_cmd, rx_cmd) = mpsc::channel();
    let (tx_event, rx_event) = mpsc::channel();
    let (tx_outbound, rx_outbound) = mpsc::channel();
    let mgr = WebrtcManager::new(my_key.to_string(), udp, local_addr, rx_cmd, tx_event, tx_outbound, String::new());
    (mgr, rx_outbound, rx_event)
}

/// Step E (docs/design/blocking-and-safe-mode.md 10f): a call or voice room
/// connects relay only, waits for its allocation, and never goes direct.
#[cfg(test)]
mod relay_only_tests {
    use super::*;

    /// The relayed address a test allocation hands out (TEST-NET-3).
    const RELAYED: &str = "203.0.113.9:50000";

    /// Put `mgr` in voice room `room` with a live allocation at [`RELAYED`].
    pub(super) fn ready_in_room(mgr: &mut WebrtcManager, room: &str) {
        let client = turn::TurnClient::allocated_for_test("127.0.0.1:9".parse().unwrap(), RELAYED.parse().unwrap());
        mgr.relay = Some(RelayScope {
            scope: CallScope::Room(room.into()),
            phase: RelayPhase::Turn(client),
            since: Instant::now(),
        });
    }

    /// The candidate lines of the SDP in a queued voice signal.
    fn candidates(signal: &str) -> Vec<String> {
        let v: Value = serde_json::from_str(signal).unwrap();
        let sdp = v["data"]["sdp"].as_str().expect("an SDP").to_string();
        sdp.lines().filter(|l| l.starts_with("a=candidate:")).map(str::to_string).collect()
    }

    fn a_voice_offer() -> Value {
        let mut rtc = Rtc::builder().build(Instant::now());
        let mut api = rtc.sdp_api();
        let _ = api.add_channel(CHANNEL_LABEL.to_string());
        let _ = api.add_media(MediaKind::Audio, Direction::SendRecv, None, None, None);
        let (offer, _pending) = api.apply().expect("offer produced");
        serde_json::to_value(&offer).unwrap()
    }

    /// The offer and the answer of a voice connection each carry ONE candidate:
    /// the relayed address, with a related address of 0.0.0.0, and nothing of
    /// this device's (its socket is on 127.0.0.1). Seen red 2026-10-09 with
    /// `add_relayed_candidate` also adding the host candidate, as voice did
    /// before step E: "only the relayed address" (two candidates, one of them
    /// 127.0.0.1).
    #[test]
    fn a_voice_offer_and_answer_carry_only_the_relayed_address() {
        let (mut mgr, outbound, _events) = test_manager("bb");
        ready_in_room(&mut mgr, "r1");
        mgr.cmd_offer_to("aa".into(), true, Some("r1".into()));
        let offer = outbound.try_recv().expect("a voice offer");
        let lines = candidates(&offer);
        assert_eq!(lines.len(), 1, "only the relayed address: {lines:?}");
        assert!(lines[0].contains("203.0.113.9 50000 typ relay raddr 0.0.0.0 rport 0"), "{lines:?}");
        assert!(!offer.contains("127.0.0.1"), "nothing of this device's address");

        mgr.cmd_voice_signal("cc".into(), "r1".into(), "offer".into(), a_voice_offer());
        let answer = outbound.try_recv().expect("a voice answer");
        assert!(answer.contains("\"answer\""), "{answer}");
        let lines = candidates(&answer);
        assert_eq!(lines.len(), 1, "only the relayed address: {lines:?}");
        assert!(lines[0].contains("typ relay"));
        assert!(!answer.contains("127.0.0.1"));
        assert!(mgr.peers.get("cc").is_some_and(|p| p.relay_only()));
    }

    /// Without its allocation a voice offer waits, and once the call or room
    /// cannot go through the server, nothing is offered or answered and no
    /// direct connection is made. A data channel is not offered either in this
    /// mode. Seen red 2026-10-09 with `relay_gate` answering `Ready` from the
    /// host address when no allocation existed (the old fall-back to direct):
    /// "a voice offer waits for its allocation".
    #[test]
    fn without_the_server_nothing_is_offered_or_answered() {
        let (mut mgr, outbound, _events) = test_manager("bb");
        let room = CallScope::Room("r1".into());
        mgr.cmd_offer_to("aa".into(), true, Some("r1".into()));
        assert!(outbound.try_recv().is_err(), "a voice offer waits for its allocation");
        assert_eq!(mgr.deferred.len(), 1);
        assert!(matches!(mgr.relay, Some(RelayScope { phase: RelayPhase::Waiting, .. })));
        mgr.cmd_voice_signal("cc".into(), "r1".into(), "offer".into(), a_voice_offer());
        assert!(outbound.try_recv().is_err(), "an answer waits too");
        assert_eq!(mgr.deferred.len(), 2);

        // The app gave up on the credentials: what waited is dropped, and later
        // offers for the room are refused outright.
        mgr.cmd_relay_unavailable(room.clone());
        assert!(mgr.deferred.is_empty());
        mgr.cmd_offer_to("dd".into(), true, Some("r1".into()));
        mgr.cmd_voice_signal("ee".into(), "r1".into(), "offer".into(), a_voice_offer());
        assert!(outbound.try_recv().is_err() && mgr.deferred.is_empty() && mgr.peers.is_empty(), "nothing goes direct");

        // A 1:1 call is its own scope (the other person's key).
        mgr.cmd_offer_to("ff".into(), true, Some(CALL_ROOM_ID.into()));
        assert_eq!(mgr.deferred.last().map(|d| d.scope.clone()), Some(CallScope::Call("ff".into())));

        // No direct data channel in this mode, even for the larger key.
        mgr.cmd_offer_to("aa".into(), false, None);
        assert!(outbound.try_recv().is_err() && !mgr.peers.contains_key("aa"), "no direct data channel");
        assert!(!DIRECT_CONNECTIONS);
    }

    /// A voice peer's datagram goes to the forwarder or nowhere. Seen red
    /// 2026-10-09 with `route_transmit` sending everything that is not from the
    /// relayed address direct, as before step E: left Direct, right Drop.
    #[test]
    fn a_voice_datagram_goes_through_the_forwarder_or_nowhere() {
        let relayed: SocketAddr = RELAYED.parse().unwrap();
        let host: SocketAddr = "192.168.1.20:5000".parse().unwrap();
        assert_eq!(route_transmit(relayed, Some(relayed), true), Route::Turn);
        assert_eq!(route_transmit(host, Some(relayed), true), Route::Drop);
        assert_eq!(route_transmit(host, None, true), Route::Drop, "no allocation, still never direct");
        assert_eq!(route_transmit(host, Some(relayed), false), Route::Direct, "a direct data channel");
    }

    /// Only the connected server's own STUN entry is taken, whatever else its
    /// list carries. Seen red 2026-10-09 with the host check taken out of
    /// `own_stun_hosts`: "a third party's STUN is never asked".
    #[test]
    fn only_the_servers_own_stun_is_asked() {
        let body = r#"{"iceServers":[
            {"urls":"stun:stun.example.com:3478"},
            {"urls":["stun:united-humanity.us:3478","stun:United-Humanity.us"]},
            {"urls":"turn:united-humanity.us:3478?transport=udp","username":"u","credential":"c"}
        ]}"#;
        let hosts = own_stun_hosts(body, host_of("https://united-humanity.us"));
        assert_eq!(hosts, vec!["united-humanity.us:3478".to_string(), "United-Humanity.us:3478".to_string()], "a third party's STUN is never asked");
        assert_eq!(host_of("http://127.0.0.1:3210/ws"), "127.0.0.1");
        assert_eq!(host_of("https://[::1]:3210"), "::1");
        assert!(own_stun_hosts("not json", "x").is_empty());
    }

    /// The candidates of an SDP are read for their forwarder permissions.
    #[test]
    fn sdp_candidates_are_read() {
        let sdp = "v=0\r\na=candidate:1 1 udp 16777215 203.0.113.9 50000 typ relay raddr 0.0.0.0 rport 0\r\na=ice-ufrag:x\r\n";
        assert_eq!(sdp_candidate_addrs(sdp), vec![RELAYED.parse::<SocketAddr>().unwrap()]);
    }
}

/// Step E end to end on this machine, without the game: two desktop-app WebRTC
/// managers, each on its own thread and 127.0.0.1 socket, join one voice room
/// through the test forwarder (`test_forwarder`, the closed subset the relay
/// speaks), and an Opus frame crosses from one to the other. The forwarder
/// hands out relayed addresses that are `Trap` sockets, so a datagram either
/// app sent straight to the other's relayed address, around the forwarder,
/// would be caught.
#[cfg(test)]
mod relay_only_rig_tests {
    use super::test_forwarder::{Forwarder, Trap};
    use super::*;

    /// Seen red 2026-10-09 with `route_transmit` never choosing `Route::Turn` (as
    /// if the wrap guard were gone, so the relayed pair's checks went straight
    /// to the other relayed address): the connection never formed, "both
    /// connected (a): [RelayReady { scope: Room(\"r1\") }, Closed { peer: \"bb\" }]".
    #[test]
    fn two_apps_in_one_room_talk_only_through_the_forwarder() {
        let traps: Vec<Trap> = (0..2).map(|_| Trap::bind()).collect();
        let fwd = Forwarder::start(traps.iter().map(|t| t.addr).collect());
        fwd.add_account("1:a", "pa", "r1");
        fwd.add_account("1:b", "pb", "r1");
        let room = CallScope::Room("r1".into());
        let creds = |user: &str, pass: &str| CallCredentials {
            scope: room.clone(),
            turn_server: fwd.addr.to_string(),
            username: user.into(),
            credential: pass.into(),
            ttl_secs: 3600,
        };
        let a = WebrtcManager::start_on("aa".into(), String::new(), "127.0.0.1:0");
        let b = WebrtcManager::start_on("bb".into(), String::new(), "127.0.0.1:0");

        // A dials before its credentials arrive (the roster can come first):
        // the offer must wait for the allocation, not go direct.
        a.offer_to_voice("bb".into(), "r1".into());
        a.use_relay(creds("1:a", "pa"));
        b.use_relay(creds("1:b", "pb"));

        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let (mut ea, mut eb) = (Vec::new(), Vec::new());
        let mut sdps = Vec::new();
        let mut got = None;
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline && got.is_none() {
            for (from, to, key) in [(&a, &b, "aa"), (&b, &a, "bb")] {
                for json in from.poll_outbound() {
                    let v: Value = serde_json::from_str(&json).unwrap();
                    assert_eq!(v["type"], "voice_room_signal", "only voice signals: {json}");
                    if let Some(sdp) = v["data"]["sdp"].as_str() {
                        sdps.push(sdp.to_string());
                    }
                    to.submit_voice_signal(
                        key.into(),
                        v["room_id"].as_str().unwrap().into(),
                        v["signal_type"].as_str().unwrap().into(),
                        v["data"].clone(),
                    );
                }
            }
            ea.extend(a.poll_events());
            eb.extend(b.poll_events());
            if ea.iter().any(|e| matches!(e, WebrtcEvent::VoiceConnected { .. })) {
                a.send_voice("bb".into(), payload.clone());
            }
            got = eb.iter().find_map(|e| match e {
                WebrtcEvent::VoiceFrame { peer, opus } => Some((peer.clone(), opus.clone())),
                _ => None,
            });
            thread::sleep(Duration::from_millis(10));
        }

        for (name, events) in [("a", &ea), ("b", &eb)] {
            assert!(
                events.iter().any(|e| matches!(e, WebrtcEvent::RelayReady { scope } if *scope == room)),
                "{name} reached the forwarder: {events:?}"
            );
            assert!(events.iter().any(|e| matches!(e, WebrtcEvent::VoiceConnected { .. })), "both connected ({name}): {events:?}");
        }
        assert_eq!(got, Some(("aa".to_string(), payload)), "the frame crossed");
        assert!(sdps.len() >= 2, "an offer and an answer: {sdps:?}");
        for sdp in &sdps {
            let lines: Vec<&str> = sdp.lines().filter(|l| l.starts_with("a=candidate:")).collect();
            assert_eq!(lines.len(), 1, "one candidate each: {lines:?}");
            let port = traps.iter().map(|t| t.addr.port()).find(|p| lines[0].contains(&format!(" {p} typ relay")));
            assert!(port.is_some(), "the candidate is a relayed address the forwarder gave out: {}", lines[0]);
        }
        assert!(fwd.delivered() > 0);
        // The answerer starts its ICE checks as soon as it answers, before the
        // offerer has read the answer and installed a permission for it, so a
        // first check or two is dropped at the receiver's end (TURN's ordinary
        // start-up race; ICE sends them again). Anything else refused would be
        // a client asking for, or sending toward, an address it should not.
        let refused: Vec<String> =
            fwd.refused().into_iter().filter(|r| !r.ends_with("without the receiver's permission")).collect();
        assert!(refused.is_empty(), "the forwarder refused nothing else: {refused:?}");
        for t in &traps {
            assert!(t.received().is_empty(), "nothing went straight to {}", t.addr);
        }
    }
}

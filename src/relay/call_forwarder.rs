//! Our own STUN responder and the room-scoped call forwarder, on one UDP port
//! (docs/design/blocking-and-safe-mode.md 7.3, 7.4 and 10f).
//!
//! # Why this exists
//!
//! A call or voice room made of direct connections shows every person's network address to
//! everyone else in it, and asking Google's STUN servers for your own address shows it to Google.
//! Here the relay answers STUN itself, and calls are carried through the relay, so the people in
//! a call see only the server's address.
//!
//! # Why this is not the 2026-08-07 incident again
//!
//! Then, coturn with a static password from the public repo let anyone relay traffic to any
//! address on the internet, and the server was used to flood third parties
//! (docs/INCIDENT-PLAYBOOK.md). This forwarder cannot send anything to an address of a client's
//! choosing:
//!
//! - **Relayed addresses are virtual.** Each allocation is told a relayed address made of the
//!   public host's IP and a unique port that is never bound to a socket.
//! - **A permission, a channel or a Send is accepted only toward another live allocation of the
//!   same room** (403 or silence otherwise). The data goes to that allocation's own client, the
//!   address that made the allocation, inside this process.
//! - **Every packet this module emits goes to one of two places**: the source of the request it
//!   answers (rate limited per source address and in total, and never larger than the request
//!   plus a few dozen bytes), or the client address of a live allocation (an address that proved
//!   it receives there, by echoing a nonce sent to it, and that holds a credential for the room).
//! - **Credentials** come only over the signed-in socket, for one room, from a secret that dies
//!   with the process (call_credentials.rs).
//!
//! # What it speaks
//!
//! STUN Binding (RFC 5389): a request is answered with XOR-MAPPED-ADDRESS (and FINGERPRINT when
//! the request had one), rate limited per source address.
//!
//! The TURN subset WebRTC uses (RFC 5766): Allocate over UDP with long-term credentials in realm
//! `humanityos` and a nonce, Refresh, CreatePermission, ChannelBind, Send and Data indications,
//! ChannelData. Nothing else is answered.
//!
//! A credential must be unexpired to make an allocation; the allocation is then refreshed, and
//! its permissions and channels made, with that same credential past its expiry (WebRTC keeps
//! using the credential it allocated with, so a call longer than the credential's hour must not
//! drop), up to [`MAX_ALLOCATION_AGE`] (12 hours) from when it was made.
//!
//! Two deliberate differences from RFC 5766, both stricter: a permission is for the exact relayed
//! address (IP and port), not the IP, because every relayed address shares the one public IP; and
//! data is at most [`MAX_DATA`] bytes a packet and [`RELAY_BYTES_PER_SEC`] a second per
//! allocation.
//!
//! # Shape
//!
//! [`Forwarder`] is the whole protocol with no socket: `handle` takes one datagram and returns the
//! datagrams to send, so the tests can drive it exactly. [`start`] binds the UDP socket from the
//! relay's settings and runs [`serve`], the loop that feeds it.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::relay::call_credentials::{parse_username, CallKeys};
use crate::relay::relay::RelayState;
use crate::relay::stun_wire::{self as wire, Class, Message, MessageBuilder};

/// The realm every credential is checked in.
pub const REALM: &str = "humanityos";

/// The port the forwarder listens on when `TURN_PORT` is not set (the standard STUN/TURN port).
pub const DEFAULT_PORT: u16 = 3478;

/// Allocation lifetime when the client asks for none or for less, and the most it may ask for
/// (RFC 5766 section 6.2 defaults).
pub const DEFAULT_LIFETIME_SECS: u32 = 600;
pub const MAX_LIFETIME_SECS: u32 = 3600;
/// How long a permission and a channel binding last without being refreshed (RFC 5766 8, 11).
pub const PERMISSION_LIFETIME: Duration = Duration::from_secs(300);
pub const CHANNEL_LIFETIME: Duration = Duration::from_secs(600);
/// The longest an allocation can live, however often it is refreshed. A credential only has to
/// be unexpired to MAKE an allocation (an hour); after that the allocation is refreshed with the
/// same credential, which is how WebRTC works, so a call longer than an hour does not drop. Twelve
/// hours is longer than any call or voice-room evening, and short enough that a client left
/// running, or one that kept an allocation after leaving the room, cannot hold it (and the
/// room's consent it stands for) indefinitely: past it the client asks for credentials again,
/// which checks again that it is in the room.
pub const MAX_ALLOCATION_AGE: Duration = Duration::from_secs(12 * 60 * 60);

/// Allocations at once, in total. Past it, Allocate is refused with 508.
pub const MAX_ALLOCATIONS: usize = 2000;
/// Allocations one credential may hold at once. A browser makes one per connection, and a voice
/// room is a full mesh (one connection to each other person), so this is about the largest room.
pub const MAX_PER_CREDENTIAL: usize = 16;
/// Allocations one source IP may hold at once (a household behind one router shares an IP).
pub const MAX_PER_SOURCE_IP: usize = 32;
/// Permissions or channels one allocation may hold at once.
pub const MAX_PEERS_PER_ALLOCATION: usize = 64;

/// Replies to a source that holds no allocation (STUN answers, challenges, errors), per source
/// IP: a sustained rate and a burst. Past it, silence.
pub const STUN_PER_SEC: f64 = 20.0;
pub const STUN_BURST: f64 = 20.0;
/// The same, for all sources together: a flood of requests from forged addresses still cannot
/// make the server send more than this.
pub const UNANSWERED_TOTAL_PER_SEC: f64 = 1000.0;
pub const UNANSWERED_TOTAL_BURST: f64 = 2000.0;
/// Source IPs whose reply budget is remembered at once.
const LIMITER_TRACK_MAX: usize = 10_000;

/// The most data one Send or ChannelData may carry. WebRTC keeps its packets under about 1,200
/// bytes so they are never fragmented; this is generous and keeps every relayed datagram small.
pub const MAX_DATA: usize = 8192;
/// Bytes a second one allocation may send through the forwarder, and its burst: room for a
/// screen share at high quality, not for using the server as a pipe.
pub const RELAY_BYTES_PER_SEC: f64 = 1_250_000.0;
pub const RELAY_BYTES_BURST: f64 = 2_500_000.0;

/// The virtual ports relayed addresses are given (the dynamic range TURN servers use).
const VIRTUAL_PORTS: std::ops::RangeInclusive<u16> = 49152..=65535;

/// One datagram to send.
#[derive(Debug, Clone, PartialEq)]
pub struct Out {
    pub to: SocketAddr,
    pub bytes: Vec<u8>,
}

/// A token bucket.
#[derive(Debug, Clone)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

impl Bucket {
    fn full(burst: f64, now: Instant) -> Bucket {
        Bucket { tokens: burst, last: now }
    }

    fn take(&mut self, n: f64, rate: f64, burst: f64, now: Instant) -> bool {
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + dt * rate).min(burst);
        self.last = now;
        if self.tokens >= n {
            self.tokens -= n;
            true
        } else {
            false
        }
    }
}

/// The reply budget for sources that hold no allocation: per source IP and in total.
struct SourceLimiter {
    per_ip: HashMap<IpAddr, Bucket>,
    total: Bucket,
}

impl SourceLimiter {
    fn new(now: Instant) -> SourceLimiter {
        SourceLimiter { per_ip: HashMap::new(), total: Bucket::full(UNANSWERED_TOTAL_BURST, now) }
    }

    fn allow(&mut self, ip: IpAddr, now: Instant) -> bool {
        if self.per_ip.len() >= LIMITER_TRACK_MAX && !self.per_ip.contains_key(&ip) {
            // A bucket idle long enough to have refilled is the same as a new one: forget those.
            let refill = Duration::from_secs_f64(STUN_BURST / STUN_PER_SEC);
            self.per_ip.retain(|_, b| now.saturating_duration_since(b.last) < refill);
            if self.per_ip.len() >= LIMITER_TRACK_MAX {
                return false;
            }
        }
        let b = self.per_ip.entry(ip).or_insert_with(|| Bucket::full(STUN_BURST, now));
        b.take(1.0, STUN_PER_SEC, STUN_BURST, now)
            && self.total.take(1.0, UNANSWERED_TOTAL_PER_SEC, UNANSWERED_TOTAL_BURST, now)
    }
}

/// One allocation: a client address holding a credential for one room.
struct Allocation {
    username: String,
    key: [u8; 16],
    scope: String,
    port: u16,
    txid: [u8; 12],
    lifetime: u32,
    /// When it was made: it never lives past this plus [`MAX_ALLOCATION_AGE`].
    created: Instant,
    expires: Instant,
    /// Relayed addresses this allocation may exchange data with, to when.
    permissions: HashMap<SocketAddr, Instant>,
    /// Channel number to (relayed address, until when).
    channels: HashMap<u16, (SocketAddr, Instant)>,
    bytes: Bucket,
}

impl Allocation {
    /// A live permission for `peer`, or a live channel bound to it (a channel implies a
    /// permission, RFC 5766 section 11; a client that refreshes only its channels, as the
    /// desktop app's does, must not lose the peer when the five-minute permission lapses).
    fn permits(&self, peer: SocketAddr, now: Instant) -> bool {
        self.permissions.get(&peer).is_some_and(|t| *t > now) || self.channel_to(peer, now).is_some()
    }

    fn channel_to(&self, peer: SocketAddr, now: Instant) -> Option<u16> {
        self.channels.iter().find(|(_, (p, t))| *p == peer && *t > now).map(|(c, _)| *c)
    }
}

/// What checking a request's credentials came to.
enum Auth {
    Ok { username: String, key: [u8; 16] },
    /// Refused: send this error (from the reply budget).
    Refuse(Vec<u8>),
}

/// The forwarder: every allocation, and the protocol, with no socket.
pub struct Forwarder {
    keys: CallKeys,
    /// The IP every relayed address carries (the public host's). `None` when it could not be
    /// worked out: then Allocate is refused with 508 and STUN still works.
    relayed_ip: Option<Ipv4Addr>,
    /// The forwarder's own port, never handed out as a virtual one.
    own_port: u16,
    allocations: HashMap<SocketAddr, Allocation>,
    by_port: HashMap<u16, SocketAddr>,
    limiter: SourceLimiter,
    last_sweep: Instant,
}

/// The reason phrase sent with each error code.
fn reason(code: u16) -> &'static str {
    match code {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        437 => "Allocation Mismatch",
        438 => "Stale Nonce",
        440 => "Address Family not Supported",
        441 => "Wrong Credentials",
        442 => "Unsupported Transport Protocol",
        486 => "Allocation Quota Reached",
        _ => "Insufficient Capacity",
    }
}

fn random_txid() -> [u8; 12] {
    use rand::RngCore;
    let mut t = [0u8; 12];
    rand::rng().fill_bytes(&mut t);
    t
}

impl Forwarder {
    pub fn new(keys: CallKeys, relayed_ip: Option<Ipv4Addr>, own_port: u16, now: Instant) -> Forwarder {
        Forwarder {
            keys,
            relayed_ip,
            own_port,
            allocations: HashMap::new(),
            by_port: HashMap::new(),
            limiter: SourceLimiter::new(now),
            last_sweep: now,
        }
    }

    /// How many allocations are live (for the tests and the log).
    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }

    /// The relayed address an allocation's port stands for.
    fn relayed(&self, port: u16) -> Option<SocketAddr> {
        self.relayed_ip.map(|ip| SocketAddr::new(IpAddr::V4(ip), port))
    }

    /// One datagram from `src`: the datagrams to send because of it. `now` drives every
    /// lifetime and budget; `unix_now` checks credential expiry and nonces.
    pub fn handle(&mut self, src: SocketAddr, pkt: &[u8], now: Instant, unix_now: u64) -> Vec<Out> {
        let src = wire::canonical(src);
        if now.saturating_duration_since(self.last_sweep) >= Duration::from_secs(1) {
            self.sweep(now);
        }
        let mut out = Vec::new();
        if pkt.first().is_some_and(|b| (0x40..=0x7F).contains(b)) {
            self.on_channel_data(src, pkt, now, &mut out);
            return out;
        }
        let Some(msg) = Message::parse(pkt) else { return out };
        if msg.fingerprint_ok() == Some(false) {
            return out;
        }
        match (msg.method, msg.class) {
            (wire::METHOD_BINDING, Class::Request) => self.on_binding(src, &msg, now, &mut out),
            (wire::METHOD_ALLOCATE, Class::Request) => self.on_allocate(src, &msg, now, unix_now, &mut out),
            (wire::METHOD_REFRESH, Class::Request)
            | (wire::METHOD_CREATE_PERMISSION, Class::Request)
            | (wire::METHOD_CHANNEL_BIND, Class::Request) => self.on_allocation_request(src, &msg, now, unix_now, &mut out),
            (wire::METHOD_SEND, Class::Indication) => self.on_send(src, &msg, now, &mut out),
            // Everything else (Binding indications are keep-alives) gets nothing.
            _ => {}
        }
        out
    }

    /// Forget expired allocations, permissions and channels.
    pub fn sweep(&mut self, now: Instant) {
        self.last_sweep = now;
        let by_port = &mut self.by_port;
        self.allocations.retain(|_, a| {
            let live = a.expires > now;
            if !live {
                by_port.remove(&a.port);
            }
            live
        });
        for a in self.allocations.values_mut() {
            a.permissions.retain(|_, t| *t > now);
            a.channels.retain(|_, (_, t)| *t > now);
        }
    }

    /// Send `bytes` to `to` from the reply budget for strangers.
    fn budgeted(&mut self, to: SocketAddr, bytes: Vec<u8>, now: Instant, out: &mut Vec<Out>) {
        if self.limiter.allow(to.ip(), now) {
            out.push(Out { to, bytes });
        }
    }

    /// A response to `msg` of `class` (with FINGERPRINT when the request had one).
    fn response(&self, msg: &Message, class: Class, fill: impl FnOnce(&mut MessageBuilder), key: Option<&[u8; 16]>) -> Vec<u8> {
        let mut b = MessageBuilder::new(msg.method, class, msg.txid);
        fill(&mut b);
        if let Some(k) = key {
            b.integrity(k);
        }
        if msg.has(wire::ATTR_FINGERPRINT) {
            b.fingerprint();
        }
        b.build()
    }

    /// An error response. `key` adds MESSAGE-INTEGRITY (for an error after the request's
    /// credentials checked out).
    fn error(&self, msg: &Message, code: u16, key: Option<&[u8; 16]>) -> Vec<u8> {
        self.response(msg, Class::Error, |b| { b.error_code(code, reason(code)); }, key)
    }

    /// A 401 or 438 with the realm and a fresh nonce for `src`.
    fn challenge(&self, msg: &Message, code: u16, src: SocketAddr, unix_now: u64) -> Vec<u8> {
        let nonce = self.keys.nonce_for(src, unix_now);
        self.response(
            msg,
            Class::Error,
            |b| {
                b.error_code(code, reason(code)).attr(wire::ATTR_REALM, REALM.as_bytes()).attr(wire::ATTR_NONCE, nonce.as_bytes());
            },
            None,
        )
    }

    fn on_binding(&mut self, src: SocketAddr, msg: &Message, now: Instant, out: &mut Vec<Out>) {
        let reply = self.response(msg, Class::Success, |b| { b.xor_address(wire::ATTR_XOR_MAPPED_ADDRESS, src); }, None);
        self.budgeted(src, reply, now, out);
    }

    /// Check a request's long-term credentials (RFC 5389 section 10.2.2). `existing` is the
    /// allocation's username and key for a request about one: the username must be the same and
    /// the integrity is checked with the key it was made with, so an allocation keeps working past
    /// its credential's expiry while it is refreshed (a long call), but a new one cannot be made
    /// with an expired credential.
    fn authenticate(&self, msg: &Message, src: SocketAddr, unix_now: u64, existing: Option<(&str, &[u8; 16])>) -> Auth {
        if !msg.has(wire::ATTR_MESSAGE_INTEGRITY) {
            return Auth::Refuse(self.challenge(msg, 401, src, unix_now));
        }
        let text = |k| msg.get(k).and_then(|v| std::str::from_utf8(v).ok());
        let (Some(username), Some(realm), Some(nonce)) = (text(wire::ATTR_USERNAME), text(wire::ATTR_REALM), msg.get(wire::ATTR_NONCE)) else {
            return Auth::Refuse(self.error(msg, 400, None));
        };
        if realm != REALM {
            return Auth::Refuse(self.challenge(msg, 401, src, unix_now));
        }
        if !self.keys.nonce_ok(nonce, src, unix_now) {
            return Auth::Refuse(self.challenge(msg, 438, src, unix_now));
        }
        let key = match existing {
            Some((name, key)) => {
                if username != name {
                    return Auth::Refuse(self.error(msg, 441, None));
                }
                *key
            }
            None => {
                let Some((expiry, _)) = parse_username(username) else {
                    return Auth::Refuse(self.challenge(msg, 401, src, unix_now));
                };
                if expiry <= unix_now {
                    return Auth::Refuse(self.challenge(msg, 401, src, unix_now));
                }
                wire::long_term_key(username, REALM, &self.keys.credential_for(username))
            }
        };
        if !msg.integrity_ok(&key) {
            return Auth::Refuse(self.challenge(msg, 401, src, unix_now));
        }
        Auth::Ok { username: username.to_string(), key }
    }

    /// The lifetime to grant for a request that asked for `asked` (or nothing).
    fn lifetime(asked: Option<u32>) -> u32 {
        asked.unwrap_or(DEFAULT_LIFETIME_SECS).clamp(DEFAULT_LIFETIME_SECS, MAX_LIFETIME_SECS)
    }

    fn requested_lifetime(msg: &Message) -> Option<u32> {
        msg.get(wire::ATTR_LIFETIME).filter(|v| v.len() == 4).map(|v| u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }

    /// An Allocate success for `a`: the relayed address, the lifetime, the client's own address.
    fn allocate_success(&self, msg: &Message, src: SocketAddr, port: u16, lifetime: u32, key: &[u8; 16]) -> Vec<u8> {
        let relayed = self.relayed(port).expect("an allocation exists only with a relayed IP");
        self.response(
            msg,
            Class::Success,
            |b| {
                b.xor_address(wire::ATTR_XOR_RELAYED_ADDRESS, relayed)
                    .u32_attr(wire::ATTR_LIFETIME, lifetime)
                    .xor_address(wire::ATTR_XOR_MAPPED_ADDRESS, src);
            },
            Some(key),
        )
    }

    /// A free virtual port: random first, then the first free one.
    fn free_port(&self) -> Option<u16> {
        use rand::Rng;
        let free = |p: u16| p != self.own_port && !self.by_port.contains_key(&p);
        let mut rng = rand::rng();
        (0..64).map(|_| rng.random_range(VIRTUAL_PORTS)).find(|p| free(*p)).or_else(|| VIRTUAL_PORTS.into_iter().find(|p| free(*p)))
    }

    fn on_allocate(&mut self, src: SocketAddr, msg: &Message, now: Instant, unix_now: u64, out: &mut Vec<Out>) {
        // Checked before the credentials, so an empty request gets a short answer, not a
        // challenge (no reply is ever much larger than what asked for it).
        let Some(transport) = msg.get(wire::ATTR_REQUESTED_TRANSPORT).filter(|v| v.len() == 4) else {
            let reply = self.error(msg, 400, None);
            return self.budgeted(src, reply, now, out);
        };
        // An allocation here that ran out (and was not swept yet) is gone: forget it and its port
        // now, so a new one for this address never inherits the old port's entry.
        if self.allocations.get(&src).is_some_and(|a| a.expires <= now) {
            if let Some(old) = self.allocations.remove(&src) {
                self.by_port.remove(&old.port);
            }
        }
        let existing = self.allocations.get(&src).map(|a| (a.username.clone(), a.key, a.txid, a.port, a.lifetime));
        let (username, key) = match self.authenticate(msg, src, unix_now, existing.as_ref().map(|e| (e.0.as_str(), &e.1))) {
            Auth::Ok { username, key } => (username, key),
            Auth::Refuse(reply) => return self.budgeted(src, reply, now, out),
        };
        if let Some((_, _, txid, port, lifetime)) = existing {
            // A retransmission of the request that made it is answered again; any other
            // Allocate from an address that already has one is a mismatch.
            let reply = if txid == msg.txid {
                self.allocate_success(msg, src, port, lifetime, &key)
            } else {
                self.error(msg, 437, Some(&key))
            };
            return out.push(Out { to: src, bytes: reply });
        }
        let refuse = |this: &Self, code| Out { to: src, bytes: this.error(msg, code, Some(&key)) };
        if transport[0] != wire::PROTOCOL_UDP {
            return out.push(refuse(self, 442));
        }
        if msg.get(wire::ATTR_REQUESTED_ADDRESS_FAMILY).is_some_and(|v| v.first() != Some(&0x01)) {
            return out.push(refuse(self, 440));
        }
        let scope = parse_username(&username).map(|(_, s)| s.to_string()).unwrap_or_default();
        let held_by = |f: &dyn Fn(&SocketAddr, &Allocation) -> bool| self.allocations.iter().filter(|(c, a)| f(c, a)).count();
        if self.relayed_ip.is_none() || self.allocations.len() >= MAX_ALLOCATIONS {
            return out.push(refuse(self, 508));
        }
        if held_by(&|_, a| a.username == username) >= MAX_PER_CREDENTIAL || held_by(&|c, _| c.ip() == src.ip()) >= MAX_PER_SOURCE_IP {
            return out.push(refuse(self, 486));
        }
        let Some(port) = self.free_port() else { return out.push(refuse(self, 508)) };
        let lifetime = Self::lifetime(Self::requested_lifetime(msg));
        let reply = self.allocate_success(msg, src, port, lifetime, &key);
        self.by_port.insert(port, src);
        self.allocations.insert(
            src,
            Allocation {
                username,
                key,
                scope,
                port,
                txid: msg.txid,
                lifetime,
                created: now,
                expires: now + Duration::from_secs(lifetime as u64),
                permissions: HashMap::new(),
                channels: HashMap::new(),
                bytes: Bucket::full(RELAY_BYTES_BURST, now),
            },
        );
        out.push(Out { to: src, bytes: reply });
    }

    /// The client address of the allocation whose relayed address is `peer`, when it is live, of
    /// room `scope`, and not the asker's own (`me`). Nothing else may be a peer.
    fn same_room_peer(&self, me: SocketAddr, scope: &str, peer: SocketAddr, now: Instant) -> Option<SocketAddr> {
        let peer = wire::canonical(peer);
        if peer.ip() != IpAddr::V4(self.relayed_ip?) {
            return None;
        }
        let client = *self.by_port.get(&peer.port())?;
        let a = self.allocations.get(&client)?;
        (client != me && a.port == peer.port() && a.expires > now && a.scope == scope).then_some(client)
    }

    /// Refresh, CreatePermission and ChannelBind: about the allocation `src` holds.
    fn on_allocation_request(&mut self, src: SocketAddr, msg: &Message, now: Instant, unix_now: u64, out: &mut Vec<Out>) {
        let Some((username, key, scope)) = self.allocations.get(&src).filter(|a| a.expires > now).map(|a| (a.username.clone(), a.key, a.scope.clone()))
        else {
            let reply = self.error(msg, 437, None);
            return self.budgeted(src, reply, now, out);
        };
        match self.authenticate(msg, src, unix_now, Some((&username, &key))) {
            Auth::Ok { .. } => {}
            Auth::Refuse(reply) => return self.budgeted(src, reply, now, out),
        }
        let reply = match msg.method {
            wire::METHOD_REFRESH => self.refresh(src, msg, now, &key),
            wire::METHOD_CREATE_PERMISSION => self.create_permission(src, &scope, msg, now, &key),
            _ => self.channel_bind(src, &scope, msg, now, &key),
        };
        out.push(Out { to: src, bytes: reply });
    }

    /// Refresh: a new lifetime, never past [`MAX_ALLOCATION_AGE`] from when the allocation was
    /// made (an allocation at its age limit is ended and answered 437, as one already gone is);
    /// lifetime 0 ends it at once.
    fn refresh(&mut self, src: SocketAddr, msg: &Message, now: Instant, key: &[u8; 16]) -> Vec<u8> {
        let asked = Self::requested_lifetime(msg);
        let end = self.allocations.get(&src).map(|a| a.created + MAX_ALLOCATION_AGE);
        let left = end.map_or(Duration::ZERO, |e| e.saturating_duration_since(now));
        let granted = if asked == Some(0) || left.as_secs() == 0 {
            if let Some(a) = self.allocations.remove(&src) {
                self.by_port.remove(&a.port);
            }
            if asked != Some(0) {
                return self.error(msg, 437, Some(key));
            }
            0
        } else {
            let lifetime = (Self::lifetime(asked) as u64).min(left.as_secs()) as u32;
            if let Some(a) = self.allocations.get_mut(&src) {
                a.lifetime = lifetime;
                a.expires = now + Duration::from_secs(lifetime as u64);
            }
            lifetime
        };
        self.response(msg, Class::Success, |b| { b.u32_attr(wire::ATTR_LIFETIME, granted); }, Some(key))
    }

    fn create_permission(&mut self, src: SocketAddr, scope: &str, msg: &Message, now: Instant, key: &[u8; 16]) -> Vec<u8> {
        let peers: Option<Vec<SocketAddr>> = msg
            .get_all(wire::ATTR_XOR_PEER_ADDRESS)
            .iter()
            .map(|v| wire::decode_xor_address(v, &msg.txid).map(wire::canonical))
            .collect();
        let peers = match peers {
            Some(p) if !p.is_empty() => p,
            _ => return self.error(msg, 400, Some(key)),
        };
        // All or nothing (RFC 5766 section 9.2): one address outside the room refuses the lot.
        if peers.iter().any(|p| self.same_room_peer(src, scope, *p, now).is_none()) {
            return self.error(msg, 403, Some(key));
        }
        let a = self.allocations.get_mut(&src).expect("checked by the caller");
        let new = peers.iter().filter(|p| !a.permissions.contains_key(p)).count();
        if a.permissions.len() + new > MAX_PEERS_PER_ALLOCATION {
            return self.error(msg, 508, Some(key));
        }
        for p in peers {
            a.permissions.insert(p, now + PERMISSION_LIFETIME);
        }
        self.response(msg, Class::Success, |_| {}, Some(key))
    }

    fn channel_bind(&mut self, src: SocketAddr, scope: &str, msg: &Message, now: Instant, key: &[u8; 16]) -> Vec<u8> {
        let channel = msg.get(wire::ATTR_CHANNEL_NUMBER).filter(|v| v.len() == 4).map(|v| u16::from_be_bytes([v[0], v[1]]));
        let peer = msg.get(wire::ATTR_XOR_PEER_ADDRESS).and_then(|v| wire::decode_xor_address(v, &msg.txid)).map(wire::canonical);
        let (Some(channel), Some(peer)) = (channel, peer) else { return self.error(msg, 400, Some(key)) };
        if !(0x4000..=0x7FFF).contains(&channel) {
            return self.error(msg, 400, Some(key));
        }
        if self.same_room_peer(src, scope, peer, now).is_none() {
            return self.error(msg, 403, Some(key));
        }
        let a = self.allocations.get_mut(&src).expect("checked by the caller");
        // A channel is bound to one peer and a peer to one channel, for as long as it lasts.
        let other_peer = a.channels.get(&channel).is_some_and(|(p, t)| *p != peer && *t > now);
        let other_channel = a.channels.iter().any(|(c, (p, t))| *p == peer && *c != channel && *t > now);
        if other_peer || other_channel {
            return self.error(msg, 400, Some(key));
        }
        if !a.channels.contains_key(&channel) && a.channels.len() >= MAX_PEERS_PER_ALLOCATION
            || !a.permissions.contains_key(&peer) && a.permissions.len() >= MAX_PEERS_PER_ALLOCATION
        {
            return self.error(msg, 508, Some(key));
        }
        a.channels.insert(channel, (peer, now + CHANNEL_LIFETIME));
        a.permissions.insert(peer, now + PERMISSION_LIFETIME);
        self.response(msg, Class::Success, |_| {}, Some(key))
    }

    fn on_send(&mut self, src: SocketAddr, msg: &Message, now: Instant, out: &mut Vec<Out>) {
        let peer = msg.get(wire::ATTR_XOR_PEER_ADDRESS).and_then(|v| wire::decode_xor_address(v, &msg.txid));
        if let (Some(peer), Some(data)) = (peer, msg.get(wire::ATTR_DATA)) {
            self.deliver(src, wire::canonical(peer), data, now, out);
        }
    }

    fn on_channel_data(&mut self, src: SocketAddr, pkt: &[u8], now: Instant, out: &mut Vec<Out>) {
        let Some((channel, data)) = wire::parse_channel_data(pkt) else { return };
        let Some(a) = self.allocations.get(&src) else { return };
        if let Some((peer, until)) = a.channels.get(&channel).copied() {
            if until > now {
                self.deliver(src, peer, data, now, out);
            }
        }
    }

    /// Pass `data` from the allocation `from` holds to the allocation whose relayed address is
    /// `peer`, if that is a live allocation of the same room that both sides have a permission
    /// for, within the sender's byte budget. It goes to that allocation's client, as ChannelData
    /// when the receiver bound a channel to the sender, else as a Data indication. This is the
    /// only place relayed data is sent, and it sends only to an allocation's client.
    fn deliver(&mut self, from: SocketAddr, peer: SocketAddr, data: &[u8], now: Instant, out: &mut Vec<Out>) {
        if data.len() > MAX_DATA {
            return;
        }
        let Some(sender) = self.allocations.get(&from).filter(|a| a.expires > now) else { return };
        let (scope, from_port) = (sender.scope.clone(), sender.port);
        if !sender.permits(peer, now) {
            return;
        }
        let Some(to_client) = self.same_room_peer(from, &scope, peer, now) else { return };
        let Some(from_relayed) = self.relayed(from_port) else { return };
        let receiver = &self.allocations[&to_client];
        if !receiver.permits(from_relayed, now) {
            return;
        }
        let channel = receiver.channel_to(from_relayed, now);
        let sender = self.allocations.get_mut(&from).expect("looked up above");
        if !sender.bytes.take(data.len() as f64, RELAY_BYTES_PER_SEC, RELAY_BYTES_BURST, now) {
            return;
        }
        let bytes = match channel {
            Some(c) => wire::channel_data(c, data),
            None => MessageBuilder::new(wire::METHOD_DATA, Class::Indication, random_txid())
                .xor_address(wire::ATTR_XOR_PEER_ADDRESS, from_relayed)
                .attr(wire::ATTR_DATA, data)
                .build(),
        };
        out.push(Out { to: to_client, bytes });
    }
}

// ── The socket ──────────────────────────────────────────────────────────────

/// Where the forwarder listens and what it tells clients, from the relay's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// `TURN_BIND`: the address the UDP socket binds. Unset: the relay's own `BIND_ADDRESS`, so a
    /// development relay on 127.0.0.1 never listens anywhere else (no firewall prompt), and a
    /// server listens on every interface.
    pub bind: IpAddr,
    /// `TURN_PORT` (default 3478). 0 asks the system for a free port (tests).
    pub port: u16,
    /// `TURN_PUBLIC_HOST`: the host clients are told to use. Unset: `TURN_SERVER_HOST` (the
    /// setting `/api/turn-credentials` already used), else the bind address when it is one
    /// address, else united-humanity.us.
    pub public_host: String,
}

impl Settings {
    /// Read the settings through `var` (the environment, or a test's map). A `TURN_BIND` that
    /// is not an IP address, or a `TURN_PORT` that is not a port, is an error, never a quiet
    /// fallback to every interface.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>, relay_bind: IpAddr) -> Result<Settings, String> {
        let set = |k: &str| var(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let bind = match set("TURN_BIND") {
            None => relay_bind,
            Some(v) => crate::relay::parse_bind_address(Some(&v))
                .map_err(|_| format!("TURN_BIND={v:?} is not an IP address (127.0.0.1 for this computer only, 0.0.0.0 for every interface)"))?,
        };
        let port = match set("TURN_PORT") {
            None => DEFAULT_PORT,
            Some(v) => v.parse::<u16>().map_err(|_| format!("TURN_PORT={v:?} is not a port number"))?,
        };
        let public_host = set("TURN_PUBLIC_HOST").or_else(|| set("TURN_SERVER_HOST")).unwrap_or_else(|| {
            if bind.is_unspecified() { "united-humanity.us".to_string() } else { bind.to_string() }
        });
        Ok(Settings { bind, port, public_host })
    }
}

/// The IPv4 address relayed addresses carry: `public_host` itself when it is one, else what it
/// resolves to, else the bind address when that is one address. `None` leaves Allocate refused.
pub async fn relayed_ip_for(public_host: &str, bind: IpAddr) -> Option<Ipv4Addr> {
    if let Ok(ip) = public_host.parse::<Ipv4Addr>() {
        return Some(ip);
    }
    if public_host.parse::<std::net::Ipv6Addr>().is_err() {
        if let Ok(addrs) = tokio::net::lookup_host((public_host, 0)).await {
            if let Some(ip) = addrs.filter_map(|a| match a.ip() { IpAddr::V4(v4) => Some(v4), _ => None }).next() {
                return Some(ip);
            }
        }
    }
    match bind {
        IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// Start the forwarder for this relay: read its settings, bind its UDP socket, tell the
/// credential side where it is, and run it. Called by `run_relay` only when the owner offers
/// voice. A setting that is wrong, or a port another program holds, leaves the relay running
/// without it (calls then say this server is not set up for them) and says why in the log.
pub async fn start(state: &Arc<RelayState>, relay_bind: IpAddr) {
    let settings = match Settings::from_vars(|k| std::env::var(k).ok(), relay_bind) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Call forwarder NOT started: {e}");
            return;
        }
    };
    let socket = match tokio::net::UdpSocket::bind(SocketAddr::new(settings.bind, settings.port)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Call forwarder NOT started: UDP {}:{} could not be opened: {e}", settings.bind, settings.port);
            return;
        }
    };
    let port = socket.local_addr().map(|a| a.port()).unwrap_or(settings.port);
    let relayed_ip = relayed_ip_for(&settings.public_host, settings.bind).await;
    if relayed_ip.is_none() {
        tracing::warn!("Call forwarder: {} has no IPv4 address, so calls cannot be carried (STUN still answers)", settings.public_host);
    }
    state.calls.set_listening(&settings.public_host, port);
    tracing::info!("Call forwarder and STUN listening on UDP {}:{port}, clients told {}:{port}", settings.bind, settings.public_host);
    let forwarder = Forwarder::new(state.calls.keys().clone(), relayed_ip, port, Instant::now());
    tokio::spawn(serve(socket, forwarder));
}

/// The loop: every datagram through the forwarder, every answer back out the same socket, and a
/// sweep of expired allocations every few seconds.
pub async fn serve(socket: tokio::net::UdpSocket, mut forwarder: Forwarder) {
    let mut buf = vec![0u8; 65_536];
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            got = socket.recv_from(&mut buf) => match got {
                Ok((n, src)) => {
                    let unix_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    for o in forwarder.handle(src, &buf[..n], Instant::now(), unix_now) {
                        let _ = socket.send_to(&o.bytes, o.to).await;
                    }
                }
                // Windows reports an earlier send's "port unreachable" on the next receive; that
                // and any other error is about one datagram, not the socket.
                Err(e) => {
                    tracing::debug!("call forwarder: receive error {e}");
                    if !matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionRefused) {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            },
            _ = tick.tick() => forwarder.sweep(Instant::now()),
        }
    }
}

#[cfg(test)]
#[path = "call_forwarder_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "call_forwarder_net_tests.rs"]
mod net_tests;

// Child of call_forwarder.rs (#[path], so `use super::*` reaches everything it has): the STUN
// responder and the room-scoped forwarder driven datagram by datagram, with no socket, so every
// lifetime and budget runs on a clock the test sets (docs/design/blocking-and-safe-mode.md 10f,
// its Proof list). The same things over real loopback sockets are in call_forwarder_net_tests.rs.

use super::*;
use crate::relay::stun_wire::*;

/// The Unix time these tests start at.
pub(super) const UNIX: u64 = 1_760_000_000;
/// The relayed IP: a documentation address (RFC 5737), so no virtual address can be mistaken for
/// a real socket of the test's.
pub(super) const RELAYED_IP: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 1);

/// A client of the forwarder: where it sends from, and the credential it was given.
pub(super) struct Client {
    pub(super) addr: SocketAddr,
    pub(super) username: String,
    pub(super) key: [u8; 16],
}

/// A client at `addr` with a credential `keys` issued for `scope`.
pub(super) fn client(keys: &CallKeys, addr: SocketAddr, scope: &str) -> Client {
    let (username, credential) = keys.issue(scope, UNIX);
    let key = long_term_key(&username, REALM, &credential);
    Client { addr, username, key }
}

pub(super) fn txid() -> [u8; 12] {
    super::random_txid()
}

/// A request from `c` of `method` with its credentials and a nonce `keys` gave its address.
pub(super) fn authed(keys: &CallKeys, c: &Client, method: u16, unix: u64, fill: impl FnOnce(&mut MessageBuilder)) -> Vec<u8> {
    let mut b = MessageBuilder::new(method, Class::Request, txid());
    fill(&mut b);
    let nonce = keys.nonce_for(c.addr, unix);
    b.attr(ATTR_USERNAME, c.username.as_bytes())
        .attr(ATTR_REALM, REALM.as_bytes())
        .attr(ATTR_NONCE, nonce.as_bytes())
        .integrity(&c.key)
        .fingerprint();
    b.build()
}

pub(super) fn allocate_request(keys: &CallKeys, c: &Client, unix: u64) -> Vec<u8> {
    authed(keys, c, METHOD_ALLOCATE, unix, |b| {
        b.u32_attr(ATTR_REQUESTED_TRANSPORT, (PROTOCOL_UDP as u32) << 24);
    })
}

pub(super) fn permission_request(keys: &CallKeys, c: &Client, peer: SocketAddr, unix: u64) -> Vec<u8> {
    authed(keys, c, METHOD_CREATE_PERMISSION, unix, |b| {
        b.xor_address(ATTR_XOR_PEER_ADDRESS, peer);
    })
}

pub(super) fn bind_request(keys: &CallKeys, c: &Client, channel: u16, peer: SocketAddr, unix: u64) -> Vec<u8> {
    authed(keys, c, METHOD_CHANNEL_BIND, unix, |b| {
        b.attr(ATTR_CHANNEL_NUMBER, &[(channel >> 8) as u8, channel as u8, 0, 0]).xor_address(ATTR_XOR_PEER_ADDRESS, peer);
    })
}

/// A Send indication (never authenticated, RFC 5766 section 10) of `data` toward `peer`.
pub(super) fn send_indication(peer: SocketAddr, data: &[u8]) -> Vec<u8> {
    MessageBuilder::new(METHOD_SEND, Class::Indication, txid()).xor_address(ATTR_XOR_PEER_ADDRESS, peer).attr(ATTR_DATA, data).build()
}

pub(super) fn binding_request() -> Vec<u8> {
    MessageBuilder::new(METHOD_BINDING, Class::Request, txid()).fingerprint().build()
}

/// The error code of a response, or None for a success.
pub(super) fn code(bytes: &[u8]) -> Option<u16> {
    let m = Message::parse(bytes).expect("a STUN response");
    match m.class {
        Class::Error => m.get(ATTR_ERROR_CODE).and_then(decode_error_code),
        _ => None,
    }
}

/// The one reply to `c`, after checking nothing else was sent.
fn only_reply(outs: Vec<Out>, to: SocketAddr) -> Vec<u8> {
    assert_eq!(outs.len(), 1, "exactly one datagram, to {to}: {outs:?}");
    assert_eq!(outs[0].to, to);
    outs.into_iter().next().unwrap().bytes
}

fn forwarder(keys: &CallKeys, t: Instant) -> Forwarder {
    Forwarder::new(keys.clone(), Some(RELAYED_IP), DEFAULT_PORT, t)
}

/// Allocate for `c`; the relayed address it was given (checking the success is signed with its
/// key, as browsers check).
fn allocate(fw: &mut Forwarder, keys: &CallKeys, c: &Client, t: Instant) -> SocketAddr {
    let reply = only_reply(fw.handle(c.addr, &allocate_request(keys, c, UNIX), t, UNIX), c.addr);
    let m = Message::parse(&reply).unwrap();
    assert_eq!(code(&reply), None, "Allocate must succeed");
    assert!(m.integrity_ok(&c.key), "the success carries MESSAGE-INTEGRITY under the client's key");
    assert_eq!(m.fingerprint_ok(), Some(true));
    assert_eq!(decode_xor_address(m.get(ATTR_XOR_MAPPED_ADDRESS).unwrap(), &m.txid), Some(c.addr));
    let relayed = decode_xor_address(m.get(ATTR_XOR_RELAYED_ADDRESS).unwrap(), &m.txid).unwrap();
    assert_eq!(relayed.ip(), IpAddr::V4(RELAYED_IP), "relayed addresses are the public IP with a virtual port");
    relayed
}

fn addr(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

/// What `to` was sent, as (channel or None for a Data indication, from, data).
fn relayed_to(outs: &[Out], to: SocketAddr) -> Vec<(Option<u16>, Option<SocketAddr>, Vec<u8>)> {
    outs.iter()
        .filter(|o| o.to == to)
        .map(|o| match parse_channel_data(&o.bytes) {
            Some((ch, data)) => (Some(ch), None, data.to_vec()),
            None => {
                let m = Message::parse(&o.bytes).expect("a Data indication");
                assert_eq!((m.method, m.class), (METHOD_DATA, Class::Indication));
                let from = decode_xor_address(m.get(ATTR_XOR_PEER_ADDRESS).unwrap(), &m.txid);
                (None, from, m.get(ATTR_DATA).unwrap().to_vec())
            }
        })
        .collect()
}

/// A Binding request is answered with the source's own address, and the answer is never much
/// larger than the request. The RFC 5769 sample request (it has a FINGERPRINT) is answered with
/// a FINGERPRINT that checks.
///
/// Seen red 2026-10-09 with `on_binding` answering with the source port off by one: "XOR-MAPPED-
/// ADDRESS is the source: left: Some(203.0.113.9:40001), right: Some(203.0.113.9:40000)".
#[test]
fn a_binding_request_is_answered_with_the_source_address() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let src = addr("203.0.113.9:40000");
    for req in [binding_request(), crate::relay::stun_wire::tests::rfc5769_sample_request()] {
        let reply = only_reply(fw.handle(src, &req, t, UNIX), src);
        let m = Message::parse(&reply).unwrap();
        let q = Message::parse(&req).unwrap();
        assert_eq!((m.method, m.class, m.txid), (METHOD_BINDING, Class::Success, q.txid));
        assert_eq!(decode_xor_address(m.get(ATTR_XOR_MAPPED_ADDRESS).unwrap(), &m.txid), Some(src), "XOR-MAPPED-ADDRESS is the source");
        assert_eq!(m.fingerprint_ok(), Some(true));
        assert!(reply.len() <= req.len() + 64, "a reply ({}) is never much larger than the request ({})", reply.len(), req.len());
    }
}

/// The per-source budget: a burst of 20 answers, then 20 a second; another source is answered
/// meanwhile; and all sources together are held to the total budget.
///
/// Seen red 2026-10-09 with `budgeted` pushing every reply: "100 requests at once from one source
/// get the burst of 20: left: 100, right: 20".
#[test]
fn the_stun_responder_is_rate_limited_per_source_and_in_total() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let src = addr("198.51.100.1:5000");
    let answered = |fw: &mut Forwarder, src, n, at| (0..n).map(|_| fw.handle(src, &binding_request(), at, UNIX).len()).sum::<usize>();
    assert_eq!(answered(&mut fw, src, 100, t), 20, "100 requests at once from one source get the burst of 20");
    assert_eq!(answered(&mut fw, addr("198.51.100.2:5000"), 1, t), 1, "another source is answered");
    assert_eq!(answered(&mut fw, src, 100, t + Duration::from_millis(500)), 10, "half a second refills 10");

    // A flood from forged addresses: each is new, so each would get its own burst, but the total
    // budget stops it (2,000 at once, then 1,000 a second).
    let mut fw = forwarder(&keys, t);
    let flood: usize = (0..5000u32)
        .map(|i| fw.handle(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + i)), 9), &binding_request(), t, UNIX).len())
        .sum();
    assert_eq!(flood, UNANSWERED_TOTAL_BURST as usize, "the total budget caps a forged-source flood");
}

/// Allocate without credentials gets a 401 with realm `humanityos` and a nonce; a browser then
/// retries with them and is given an allocation. Bad credentials of every kind are refused:
/// a wrong password, an expired credential, a username with another room's tag (the HMAC covers
/// it), another realm, a credential minted by another run of the relay, a nonce given to another
/// address, a stale nonce.
///
/// Seen red 2026-10-09 with `authenticate` returning Ok without checking the integrity: "a wrong
/// password: left: None, right: Some(401)".
#[test]
fn allocate_is_refused_without_valid_credentials() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let room = keys.room_scope("lounge");
    let c = client(&keys, addr("203.0.113.5:6000"), &room);

    // The challenge, as a browser meets it.
    let mut b = MessageBuilder::new(METHOD_ALLOCATE, Class::Request, txid());
    b.u32_attr(ATTR_REQUESTED_TRANSPORT, 17 << 24);
    let bare = b.build();
    let reply = only_reply(fw.handle(c.addr, &bare, t, UNIX), c.addr);
    assert_eq!(code(&reply), Some(401));
    let m = Message::parse(&reply).unwrap();
    assert_eq!(m.get(ATTR_REALM), Some(REALM.as_bytes()));
    let nonce = m.get(ATTR_NONCE).expect("a nonce").to_vec();
    assert!(reply.len() <= bare.len() + 64, "the challenge is never much larger than the request");

    // The retry with that nonce and the credential is given an allocation.
    let mut b = MessageBuilder::new(METHOD_ALLOCATE, Class::Request, txid());
    b.u32_attr(ATTR_REQUESTED_TRANSPORT, 17 << 24)
        .attr(ATTR_USERNAME, c.username.as_bytes())
        .attr(ATTR_REALM, REALM.as_bytes())
        .attr(ATTR_NONCE, &nonce)
        .integrity(&c.key);
    let ok = only_reply(fw.handle(c.addr, &b.build(), t, UNIX), c.addr);
    assert_eq!(code(&ok), None, "the retry with the challenge's nonce succeeds");
    assert_eq!(fw.allocation_count(), 1);

    let try_allocate = |fw: &mut Forwarder, c: &Client, unix: u64, req: Vec<u8>| code(&only_reply(fw.handle(c.addr, &req, t, unix), c.addr));
    let fresh = |port: u16| addr(&format!("203.0.113.6:{port}"));

    let mut wrong = client(&keys, fresh(1), &room);
    wrong.key = long_term_key(&wrong.username, REALM, "not the credential");
    assert_eq!(try_allocate(&mut fw, &wrong, UNIX, allocate_request(&keys, &wrong, UNIX)), Some(401), "a wrong password");

    let late = client(&keys, fresh(2), &room);
    let after = UNIX + crate::relay::call_credentials::CREDENTIAL_TTL_SECS;
    assert_eq!(try_allocate(&mut fw, &late, after, allocate_request(&keys, &late, after)), Some(401), "an expired credential");

    // Another room's tag in the username, keeping this room's credential: the HMAC no longer
    // matches, so the room cannot be swapped.
    let mut swapped = client(&keys, fresh(3), &room);
    swapped.username = swapped.username.replace(&room, &keys.room_scope("another room"));
    assert_eq!(try_allocate(&mut fw, &swapped, UNIX, allocate_request(&keys, &swapped, UNIX)), Some(401), "another room's tag");

    let elsewhere = client(&CallKeys::generate(), fresh(4), &room);
    assert_eq!(try_allocate(&mut fw, &elsewhere, UNIX, allocate_request(&keys, &elsewhere, UNIX)), Some(401), "a credential of another run");

    let realm = client(&keys, fresh(5), &room);
    let mut b = MessageBuilder::new(METHOD_ALLOCATE, Class::Request, txid());
    b.u32_attr(ATTR_REQUESTED_TRANSPORT, 17 << 24)
        .attr(ATTR_USERNAME, realm.username.as_bytes())
        .attr(ATTR_REALM, b"example.org")
        .attr(ATTR_NONCE, keys.nonce_for(realm.addr, UNIX).as_bytes())
        .integrity(&realm.key);
    assert_eq!(try_allocate(&mut fw, &realm, UNIX, b.build()), Some(401), "another realm");

    // A nonce made for another address, and one past its lifetime: 438, with a fresh one.
    let moved = client(&keys, fresh(6), &room);
    let other = Client { addr: fresh(7), username: moved.username.clone(), key: moved.key };
    let req = allocate_request(&keys, &other, UNIX);
    assert_eq!(try_allocate(&mut fw, &moved, UNIX, req), Some(438), "a nonce given to another address");
    let stale = UNIX + crate::relay::call_credentials::NONCE_LIFETIME_SECS;
    assert_eq!(try_allocate(&mut fw, &moved, stale, allocate_request(&keys, &moved, UNIX)), Some(438), "a stale nonce");

    assert_eq!(fw.allocation_count(), 1, "none of those made an allocation");
}

/// Two allocations of one room exchange data both ways: by Send indications (delivered as Data
/// indications from the sender's relayed address), then over channels (ChannelData both ways).
///
/// Seen red 2026-10-09 with `deliver` addressing the datagram to the peer's relayed address
/// instead of its client: "B is sent A's data as a Data indication" found nothing sent to B.
#[test]
fn two_allocations_of_one_room_exchange_data_both_ways() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let room = keys.room_scope("lounge");
    let a = client(&keys, addr("203.0.113.10:1000"), &room);
    let b = client(&keys, addr("198.51.100.20:2000"), &room);
    let ra = allocate(&mut fw, &keys, &a, t);
    let rb = allocate(&mut fw, &keys, &b, t);
    assert_ne!(ra, rb);

    for (c, peer) in [(&a, rb), (&b, ra)] {
        let reply = only_reply(fw.handle(c.addr, &permission_request(&keys, c, peer, UNIX), t, UNIX), c.addr);
        assert_eq!(code(&reply), None, "a permission toward an allocation of the same room");
        assert!(Message::parse(&reply).unwrap().integrity_ok(&c.key));
    }

    let outs = fw.handle(a.addr, &send_indication(rb, b"hello b"), t, UNIX);
    assert_eq!(relayed_to(&outs, b.addr), vec![(None, Some(ra), b"hello b".to_vec())], "B is sent A's data as a Data indication");
    assert_eq!(outs.len(), 1, "and nothing else is sent");
    let outs = fw.handle(b.addr, &send_indication(ra, b"hello a"), t, UNIX);
    assert_eq!(relayed_to(&outs, a.addr), vec![(None, Some(rb), b"hello a".to_vec())]);

    // Channels: each binds one to the other; then ChannelData flows both ways, each side
    // receiving on the channel number it chose.
    assert_eq!(code(&only_reply(fw.handle(a.addr, &bind_request(&keys, &a, 0x4001, rb, UNIX), t, UNIX), a.addr)), None);
    assert_eq!(code(&only_reply(fw.handle(b.addr, &bind_request(&keys, &b, 0x4abc, ra, UNIX), t, UNIX), b.addr)), None);
    let outs = fw.handle(a.addr, &channel_data(0x4001, b"over a channel"), t, UNIX);
    assert_eq!(relayed_to(&outs, b.addr), vec![(Some(0x4abc), None, b"over a channel".to_vec())]);
    let outs = fw.handle(b.addr, &channel_data(0x4abc, b"and back"), t, UNIX);
    assert_eq!(relayed_to(&outs, a.addr), vec![(Some(0x4001), None, b"and back".to_vec())]);
    // A Send indication still works after the channel is bound, and arrives on the channel.
    let outs = fw.handle(a.addr, &send_indication(rb, b"send again"), t, UNIX);
    assert_eq!(relayed_to(&outs, b.addr), vec![(Some(0x4abc), None, b"send again".to_vec())]);
    // Past the permissions' five minutes, within the channels' ten: the channels still carry it
    // (a channel implies a permission; the desktop app refreshes only its channels).
    let later = t + PERMISSION_LIFETIME + Duration::from_secs(60);
    let outs = fw.handle(a.addr, &channel_data(0x4001, b"six minutes in"), later, UNIX);
    assert_eq!(relayed_to(&outs, b.addr), vec![(Some(0x4abc), None, b"six minutes in".to_vec())], "a channel outlives the permission");
}

/// Toward anything that is not a live allocation of the same room, a permission and a channel
/// are refused with 403 and a Send goes nowhere: another room's allocation, that allocation's
/// real client address, an outside address, the relayed IP at a port no allocation has, the
/// forwarder's own port, the asker's own relayed address. Nothing is sent at all, to anyone,
/// except the 403 back to the asker.
///
/// Seen red 2026-10-09 with the scope comparison dropped from `same_room_peer`: "a permission
/// toward another room's allocation: left: None, right: Some(403)"; and with `deliver` first
/// sending the data to whatever address it was named (an open relay): "a Send toward another
/// room's allocation sends nothing".
#[test]
fn nothing_outside_the_room_can_be_reached() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let a = client(&keys, addr("203.0.113.10:1000"), &keys.room_scope("lounge"));
    let c = client(&keys, addr("203.0.113.30:3000"), &keys.room_scope("another room"));
    let ra = allocate(&mut fw, &keys, &a, t);
    let rc = allocate(&mut fw, &keys, &c, t);
    // C opens its side to A, so only A's own refusal stands between them.
    let _ = fw.handle(c.addr, &permission_request(&keys, &c, ra, UNIX), t, UNIX);

    let unallocated = (49152u16..).find(|p| *p != ra.port() && *p != rc.port()).unwrap();
    for (what, target) in [
        ("another room's allocation", rc),
        ("another room's client address", c.addr),
        ("an outside address", addr("8.8.8.8:53")),
        ("the relayed IP at a port no allocation has", SocketAddr::new(IpAddr::V4(RELAYED_IP), unallocated)),
        ("the forwarder's own port", SocketAddr::new(IpAddr::V4(RELAYED_IP), DEFAULT_PORT)),
        ("the asker's own relayed address", ra),
    ] {
        let reply = only_reply(fw.handle(a.addr, &permission_request(&keys, &a, target, UNIX), t, UNIX), a.addr);
        assert_eq!(code(&reply), Some(403), "a permission toward {what}");
        let reply = only_reply(fw.handle(a.addr, &bind_request(&keys, &a, 0x4100, target, UNIX), t, UNIX), a.addr);
        assert_eq!(code(&reply), Some(403), "a channel toward {what}");
        assert!(fw.handle(a.addr, &send_indication(target, b"x"), t, UNIX).is_empty(), "a Send toward {what} sends nothing");
    }
    // The same Send from an address that holds no allocation at all is silent too.
    assert!(fw.handle(addr("192.0.2.200:1"), &send_indication(c.addr, b"x"), t, UNIX).is_empty());
    // And ChannelData on a channel nobody bound.
    assert!(fw.handle(a.addr, &channel_data(0x4100, b"x"), t, UNIX).is_empty());
}

/// A request about an allocation from an address that holds none is 437; a retransmitted
/// Allocate is answered with the same relayed address; another Allocate from an address that has
/// one is 437; UDP only.
#[test]
fn allocate_retransmissions_and_mismatches() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let room = keys.room_scope("lounge");
    let a = client(&keys, addr("203.0.113.10:1000"), &room);
    let req = allocate_request(&keys, &a, UNIX);
    let first = only_reply(fw.handle(a.addr, &req, t, UNIX), a.addr);
    let again = only_reply(fw.handle(a.addr, &req, t, UNIX), a.addr);
    assert_eq!(first, again, "a retransmission is answered the same");
    assert_eq!(code(&only_reply(fw.handle(a.addr, &allocate_request(&keys, &a, UNIX), t, UNIX), a.addr)), Some(437));

    let nobody = client(&keys, addr("203.0.113.11:1000"), &room);
    let req = authed(&keys, &nobody, METHOD_REFRESH, UNIX, |_| {});
    assert_eq!(code(&only_reply(fw.handle(nobody.addr, &req, t, UNIX), nobody.addr)), Some(437));

    let tcp = authed(&keys, &nobody, METHOD_ALLOCATE, UNIX, |b| {
        b.u32_attr(ATTR_REQUESTED_TRANSPORT, 6 << 24);
    });
    assert_eq!(code(&only_reply(fw.handle(nobody.addr, &tcp, t, UNIX), nobody.addr)), Some(442));
}

/// Allocations are capped per credential, per source IP and in total, so the table cannot grow
/// without bound.
///
/// Seen red 2026-10-09 with the per-credential check removed: "the 17th allocation on one
/// credential: left: None, right: Some(486)".
#[test]
fn allocations_are_limited_per_credential_per_source_and_in_total() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let room = keys.room_scope("lounge");
    let one = client(&keys, addr("203.0.113.1:1"), &room);
    for i in 0..MAX_PER_CREDENTIAL as u16 {
        let c = Client { addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 1, 0, i as u8)), 1000), username: one.username.clone(), key: one.key };
        allocate(&mut fw, &keys, &c, t);
    }
    let c = Client { addr: addr("10.1.1.0:1000"), username: one.username.clone(), key: one.key };
    assert_eq!(code(&only_reply(fw.handle(c.addr, &allocate_request(&keys, &c, UNIX), t, UNIX), c.addr)), Some(486), "the 17th allocation on one credential");

    let mut fw = forwarder(&keys, t);
    for port in 0..MAX_PER_SOURCE_IP as u16 {
        allocate(&mut fw, &keys, &client(&keys, SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 2, 0, 1)), 1000 + port), &room), t);
    }
    let c = client(&keys, addr("10.2.0.1:999"), &room);
    assert_eq!(code(&only_reply(fw.handle(c.addr, &allocate_request(&keys, &c, UNIX), t, UNIX), c.addr)), Some(486), "one address past its share");

    let mut fw = forwarder(&keys, t);
    for i in 0..MAX_ALLOCATIONS as u32 {
        allocate(&mut fw, &keys, &client(&keys, SocketAddr::new(IpAddr::V4(Ipv4Addr::from(0x0B00_0000 + i)), 1000), &room), t);
    }
    let c = client(&keys, addr("12.0.0.1:1000"), &room);
    assert_eq!(code(&only_reply(fw.handle(c.addr, &allocate_request(&keys, &c, UNIX), t, UNIX), c.addr)), Some(508), "past the total");
    assert_eq!(fw.allocation_count(), MAX_ALLOCATIONS);
}

/// Lifetimes: an allocation not refreshed is gone after its lifetime (and its data stops); a
/// Refresh extends it, and keeps working past its credential's expiry (a call longer than an
/// hour); a Refresh with lifetime 0 ends it at once; a permission lapses after five minutes.
///
/// Seen red 2026-10-09 with `sweep` keeping expired allocations: "unrefreshed, it is gone: left:
/// 2, right: 1".
#[test]
fn allocations_permissions_and_channels_expire() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let room = keys.room_scope("lounge");
    let a = client(&keys, addr("203.0.113.10:1000"), &room);
    let b = client(&keys, addr("203.0.113.20:2000"), &room);
    let ra = allocate(&mut fw, &keys, &a, t);
    let rb = allocate(&mut fw, &keys, &b, t);
    let _ = fw.handle(a.addr, &permission_request(&keys, &a, rb, UNIX), t, UNIX);
    let _ = fw.handle(b.addr, &permission_request(&keys, &b, ra, UNIX), t, UNIX);

    // A permission lapses after five minutes without being refreshed.
    let t1 = t + PERMISSION_LIFETIME + Duration::from_secs(1);
    assert!(fw.handle(a.addr, &send_indication(rb, b"late"), t1, UNIX).is_empty(), "a lapsed permission passes nothing");

    // B refreshes past its credential's expiry (an hour and more into a call); A does not.
    let mut tb = t;
    let mut unix = UNIX;
    for _ in 0..8 {
        tb += Duration::from_secs(500);
        unix += 500;
        let req = authed(&keys, &b, METHOD_REFRESH, unix, |m| { m.u32_attr(ATTR_LIFETIME, 600); });
        assert_eq!(code(&only_reply(fw.handle(b.addr, &req, tb, unix), b.addr)), None, "a Refresh {}s in", unix - UNIX);
    }
    fw.sweep(tb);
    assert_eq!(fw.allocation_count(), 1, "unrefreshed, it is gone");
    let req = authed(&keys, &a, METHOD_REFRESH, unix, |_| {});
    assert_eq!(code(&only_reply(fw.handle(a.addr, &req, tb, unix), a.addr)), Some(437), "A's allocation no longer exists");

    let req = authed(&keys, &b, METHOD_REFRESH, unix, |m| { m.u32_attr(ATTR_LIFETIME, 0); });
    assert_eq!(code(&only_reply(fw.handle(b.addr, &req, tb, unix), b.addr)), None);
    assert_eq!(fw.allocation_count(), 0, "lifetime 0 ends it");
}

/// A call longer than its credential's hour does not drop: an allocation made with a credential
/// is refreshed, and makes permissions and channels, with that same credential after it expired,
/// and data still flows; but a NEW Allocate with the expired credential is refused (401). And
/// nothing lives past twelve hours: the Refresh that would cross the limit is granted only what
/// is left, and the next finds the allocation gone (437).
///
/// Seen red 2026-10-09 with `authenticate` checking the credential's expiry for an existing
/// allocation as well as a new one: "a Refresh past the credential's expiry (3660s in): left:
/// Some(401), right: None".
#[test]
fn an_allocation_outlives_its_credential_but_not_twelve_hours() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let room = keys.room_scope("lounge");
    let a = client(&keys, addr("203.0.113.10:1000"), &room);
    let b = client(&keys, addr("203.0.113.20:2000"), &room);
    let ra = allocate(&mut fw, &keys, &a, t);
    let rb = allocate(&mut fw, &keys, &b, t);
    let refresh = |fw: &mut Forwarder, c: &Client, step: u64| {
        let (at, unix) = (t + Duration::from_secs(step), UNIX + step);
        let req = authed(&keys, c, METHOD_REFRESH, unix, |m| { m.u32_attr(ATTR_LIFETIME, 600); });
        let reply = only_reply(fw.handle(c.addr, &req, at, unix), c.addr);
        let m = Message::parse(&reply).unwrap();
        (code(&reply), m.get(ATTR_LIFETIME).map(|v| u32::from_be_bytes([v[0], v[1], v[2], v[3]])))
    };

    // Both refresh every 500 s until an hour and a minute in: past the credential's expiry.
    let past = crate::relay::call_credentials::CREDENTIAL_TTL_SECS + 60;
    let mut step = 0;
    while step < past {
        step = (step + 500).min(past);
        assert_eq!(refresh(&mut fw, &a, step).0, None, "a Refresh past the credential's expiry ({step}s in)");
        assert_eq!(refresh(&mut fw, &b, step).0, None);
    }
    let (at, unix) = (t + Duration::from_secs(past), UNIX + past);
    for (c, peer) in [(&a, rb), (&b, ra)] {
        assert_eq!(code(&only_reply(fw.handle(c.addr, &permission_request(&keys, c, peer, unix), at, unix), c.addr)), None, "a permission");
    }
    assert_eq!(code(&only_reply(fw.handle(a.addr, &bind_request(&keys, &a, 0x4000, rb, unix), at, unix), a.addr)), None, "a channel");
    assert_eq!(fw.handle(a.addr, &channel_data(0x4000, b"still talking"), at, unix).len(), 1, "data still flows");

    let again = Client { addr: addr("203.0.113.10:1001"), username: a.username.clone(), key: a.key };
    let reply = only_reply(fw.handle(again.addr, &allocate_request(&keys, &again, unix), at, unix), again.addr);
    assert_eq!(code(&reply), Some(401), "a NEW Allocate with the expired credential");

    // A keeps refreshing to the twelve-hour limit.
    let limit = MAX_ALLOCATION_AGE.as_secs();
    let mut last = None;
    loop {
        step += 500;
        let (c, lifetime) = refresh(&mut fw, &a, step);
        if step >= limit {
            assert_eq!(c, Some(437), "past twelve hours the allocation is gone");
            break;
        }
        assert_eq!(c, None, "a Refresh {step}s in");
        last = Some((step, lifetime));
    }
    let (last_step, lifetime) = last.unwrap();
    assert!(limit - last_step < 600, "the last Refresh came within a lifetime of the limit");
    assert_eq!(lifetime, Some((limit - last_step) as u32), "the last Refresh is granted only what is left");
    assert_eq!(fw.allocation_count(), 0);
}

/// Data is at most MAX_DATA bytes a packet and RELAY_BYTES_PER_SEC a second per allocation.
#[test]
fn relayed_data_is_limited_in_size_and_rate() {
    let keys = CallKeys::generate();
    let t = Instant::now();
    let mut fw = forwarder(&keys, t);
    let room = keys.room_scope("lounge");
    let a = client(&keys, addr("203.0.113.10:1000"), &room);
    let b = client(&keys, addr("203.0.113.20:2000"), &room);
    let ra = allocate(&mut fw, &keys, &a, t);
    let rb = allocate(&mut fw, &keys, &b, t);
    let _ = fw.handle(a.addr, &permission_request(&keys, &a, rb, UNIX), t, UNIX);
    let _ = fw.handle(b.addr, &permission_request(&keys, &b, ra, UNIX), t, UNIX);
    assert!(fw.handle(a.addr, &send_indication(rb, &vec![0u8; MAX_DATA + 1]), t, UNIX).is_empty(), "too big a packet");
    let packet = vec![0u8; MAX_DATA];
    let passed = (0..400).filter(|_| !fw.handle(a.addr, &send_indication(rb, &packet), t, UNIX).is_empty()).count();
    assert_eq!(passed, (RELAY_BYTES_BURST as usize) / MAX_DATA, "the burst, then nothing until it refills");
}

/// The settings: TURN_BIND follows the relay's own BIND_ADDRESS unless set; a bad value is an
/// error, never every interface; TURN_PUBLIC_HOST falls back to TURN_SERVER_HOST, then the bind
/// address when it is one address.
///
/// Seen red 2026-10-09 with the TURN_BIND default fixed at 0.0.0.0: "a development relay on
/// 127.0.0.1 keeps the forwarder on 127.0.0.1: left: 0.0.0.0, right: 127.0.0.1".
#[test]
fn settings_follow_the_relay_and_refuse_bad_values() {
    let vars = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let every = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

    let s = Settings::from_vars(vars(&[]), loopback).unwrap();
    assert_eq!(s.bind, loopback, "a development relay on 127.0.0.1 keeps the forwarder on 127.0.0.1");
    assert_eq!((s.port, s.public_host.as_str()), (DEFAULT_PORT, "127.0.0.1"));

    let s = Settings::from_vars(vars(&[]), every).unwrap();
    assert_eq!((s.bind, s.public_host.as_str()), (every, "united-humanity.us"));

    let s = Settings::from_vars(vars(&[("TURN_SERVER_HOST", "relay.example")]), every).unwrap();
    assert_eq!(s.public_host, "relay.example");
    let s = Settings::from_vars(vars(&[("TURN_PUBLIC_HOST", "calls.example"), ("TURN_SERVER_HOST", "relay.example"), ("TURN_PORT", "0"), ("TURN_BIND", "127.0.0.1")]), every).unwrap();
    assert_eq!((s.bind, s.port, s.public_host.as_str()), (loopback, 0, "calls.example"));

    assert!(Settings::from_vars(vars(&[("TURN_BIND", "lan")]), loopback).is_err(), "a typo is an error, not every interface");
    assert!(Settings::from_vars(vars(&[("TURN_PORT", "34780")]), loopback).is_ok());
    assert!(Settings::from_vars(vars(&[("TURN_PORT", "99999")]), loopback).is_err());
}

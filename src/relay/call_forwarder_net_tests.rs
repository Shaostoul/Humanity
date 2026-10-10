// Child of call_forwarder.rs (#[path]): the forwarder on real UDP sockets, all on 127.0.0.1 (never
// 0.0.0.0: a test that listens on every interface raises a Windows Firewall prompt, CLAUDE.md).
// The serving loop is `serve`, exactly what `start` runs. These are the network halves of the
// Proof list in docs/design/blocking-and-safe-mode.md 10f: a Binding answered, the rate limit,
// two allocations of one room exchanging data, and THE check that nothing ever leaves toward an
// address that is not an allocation of the same room, made by listening at that address.

use super::tests::{allocate_request, authed, bind_request, binding_request, code, permission_request, send_indication, Client};
use super::*;
use crate::relay::stun_wire::*;
use tokio::net::UdpSocket;

/// How long to wait for a datagram that should come, and to listen for one that must not.
const WAIT: Duration = Duration::from_millis(1500);
const QUIET: Duration = Duration::from_millis(600);

/// A forwarder serving on 127.0.0.1 with `relayed_ip`, and its address. The forwarder's clock is
/// the real one, so a test's credentials are issued for now.
async fn serving(keys: &CallKeys, relayed_ip: Ipv4Addr) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind the forwarder on loopback");
    let at = socket.local_addr().unwrap();
    assert!(at.ip().is_loopback());
    tokio::spawn(serve(socket, Forwarder::new(keys.clone(), Some(relayed_ip), at.port(), Instant::now())));
    at
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

/// A loopback socket for a test client.
async fn socket() -> UdpSocket {
    UdpSocket::bind("127.0.0.1:0").await.expect("bind a client on loopback")
}

/// A credential issued now (the forwarder checks expiry against the real clock).
fn client_now(keys: &CallKeys, sock: &UdpSocket, scope: &str) -> Client {
    let (username, credential) = keys.issue(scope, unix_now());
    let key = long_term_key(&username, REALM, &credential);
    Client { addr: sock.local_addr().unwrap(), username, key }
}

/// The next datagram on `sock`, or None within `wait`.
async fn recv(sock: &UdpSocket, wait: Duration) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; 70_000];
    match tokio::time::timeout(wait, sock.recv_from(&mut buf)).await {
        Ok(Ok((n, _))) => Some(buf[..n].to_vec()),
        _ => None,
    }
}

/// Send `bytes` to the forwarder and wait for its one answer.
async fn ask(sock: &UdpSocket, fw: SocketAddr, bytes: &[u8]) -> Vec<u8> {
    sock.send_to(bytes, fw).await.unwrap();
    recv(sock, WAIT).await.expect("the forwarder answers")
}

/// Allocate for `c` on `sock`, through the challenge as a browser does; its relayed address.
async fn allocate(sock: &UdpSocket, fw: SocketAddr, keys: &CallKeys, c: &Client) -> SocketAddr {
    let mut b = MessageBuilder::new(METHOD_ALLOCATE, Class::Request, super::random_txid());
    b.u32_attr(ATTR_REQUESTED_TRANSPORT, 17 << 24);
    let challenge = ask(sock, fw, &b.build()).await;
    assert_eq!(code(&challenge), Some(401));
    let reply = ask(sock, fw, &allocate_request(keys, c, unix_now())).await;
    assert_eq!(code(&reply), None, "Allocate succeeds");
    let m = Message::parse(&reply).unwrap();
    assert!(m.integrity_ok(&c.key));
    decode_xor_address(m.get(ATTR_XOR_RELAYED_ADDRESS).unwrap(), &m.txid).unwrap()
}

/// A Binding request over loopback is answered with the client's own address.
///
/// Seen red 2026-10-09 with `on_binding` answering with the source port off by one: "left:
/// Some(127.0.0.1:58763), right: Some(127.0.0.1:58762)".
#[tokio::test]
async fn a_stun_binding_is_answered_over_loopback() {
    let keys = CallKeys::generate();
    let fw = serving(&keys, Ipv4Addr::new(192, 0, 2, 1)).await;
    let sock = socket().await;
    let reply = ask(&sock, fw, &binding_request()).await;
    let m = Message::parse(&reply).unwrap();
    assert_eq!((m.method, m.class), (METHOD_BINDING, Class::Success));
    assert_eq!(decode_xor_address(m.get(ATTR_XOR_MAPPED_ADDRESS).unwrap(), &m.txid), Some(sock.local_addr().unwrap()));
    assert_eq!(m.fingerprint_ok(), Some(true));
}

/// Sixty Binding requests at once from one socket get about the burst of twenty, not sixty.
///
/// Seen red 2026-10-09 with `budgeted` pushing every reply (no limiter): the answer count failed
/// its bound, every request being answered.
#[tokio::test]
async fn the_stun_rate_limit_holds_over_loopback() {
    let keys = CallKeys::generate();
    let fw = serving(&keys, Ipv4Addr::new(192, 0, 2, 1)).await;
    let sock = socket().await;
    let started = Instant::now();
    for _ in 0..60 {
        sock.send_to(&binding_request(), fw).await.unwrap();
    }
    let mut answered = 0;
    while recv(&sock, QUIET).await.is_some() {
        answered += 1;
    }
    // The burst, plus whatever 20 a second refilled while the requests went out.
    let allowed = STUN_BURST as usize + (started.elapsed().as_secs_f64() * STUN_PER_SEC).ceil() as usize;
    assert!(answered >= STUN_BURST as usize && answered <= allowed.min(59), "answered {answered} of 60 (allowed {allowed})");
}

/// Two clients in one room, on two loopback sockets: data both ways by Send (arriving as Data
/// indications), then by ChannelBind and ChannelData.
///
/// Seen red 2026-10-09 with `deliver` addressing the datagram to the peer's relayed (virtual)
/// address instead of its client: "B receives A's Send" timed out.
#[tokio::test]
async fn two_allocations_of_one_room_exchange_data_over_loopback() {
    let keys = CallKeys::generate();
    let fw = serving(&keys, Ipv4Addr::new(192, 0, 2, 1)).await;
    let room = keys.room_scope("lounge");
    let (sa, sb) = (socket().await, socket().await);
    let (a, b) = (client_now(&keys, &sa, &room), client_now(&keys, &sb, &room));
    let ra = allocate(&sa, fw, &keys, &a).await;
    let rb = allocate(&sb, fw, &keys, &b).await;
    let now = unix_now();
    assert_eq!(code(&ask(&sa, fw, &permission_request(&keys, &a, rb, now)).await), None);
    assert_eq!(code(&ask(&sb, fw, &permission_request(&keys, &b, ra, now)).await), None);

    sa.send_to(&send_indication(rb, b"from a"), fw).await.unwrap();
    let got = recv(&sb, WAIT).await.expect("B receives A's Send");
    let m = Message::parse(&got).expect("a Data indication");
    assert_eq!((m.method, m.class), (METHOD_DATA, Class::Indication));
    assert_eq!(decode_xor_address(m.get(ATTR_XOR_PEER_ADDRESS).unwrap(), &m.txid), Some(ra), "from A's relayed address");
    assert_eq!(m.get(ATTR_DATA), Some(&b"from a"[..]));

    sb.send_to(&send_indication(ra, b"from b"), fw).await.unwrap();
    let got = recv(&sa, WAIT).await.expect("A receives B's Send");
    assert_eq!(Message::parse(&got).unwrap().get(ATTR_DATA), Some(&b"from b"[..]));

    assert_eq!(code(&ask(&sa, fw, &bind_request(&keys, &a, 0x4000, rb, now)).await), None);
    assert_eq!(code(&ask(&sb, fw, &bind_request(&keys, &b, 0x4001, ra, now)).await), None);
    sa.send_to(&channel_data(0x4000, b"channel a"), fw).await.unwrap();
    assert_eq!(recv(&sb, WAIT).await, Some(channel_data(0x4001, b"channel a")), "B gets it on its own channel");
    sb.send_to(&channel_data(0x4001, b"channel b"), fw).await.unwrap();
    assert_eq!(recv(&sa, WAIT).await, Some(channel_data(0x4000, b"channel b")), "and A on its");
}

/// THE INCIDENT CHECK (docs/INCIDENT-PLAYBOOK.md, 2026-08-07). Sockets listen at every address
/// a client might try to make the forwarder send to: an outside "victim" address, another room's
/// client, and (with the relayed IP set to 127.0.0.1, so a virtual address and a real one look
/// alike) a real socket at the relayed IP whose port no allocation has. A client in the room asks
/// for a permission and a channel toward each, and is refused with 403; it then sends Send
/// indications and ChannelData toward each anyway. Not one datagram arrives at any of them.
///
/// Seen red 2026-10-09 with `deliver` first sending the data to whatever address it was named
/// (an open relay, as coturn was), every permission and channel still refused: "the victim at
/// 127.0.0.1 received a datagram: left: Some([102, 108, 111, 111, 100])", the bytes of "flood".
/// And with the room check dropped from `same_room_peer`: "C cannot even open to A".
#[tokio::test]
async fn no_packet_leaves_toward_an_address_that_is_not_an_allocation_of_the_same_room() {
    // With the relayed IP on loopback, a client socket's real port could (rarely) equal a virtual
    // port the forwarder chose; then that address IS an allocation's. Start again until none do.
    let (keys, fw, sa, sb, sc, a, b, c, ra, rb, rc) = loop {
        let keys = CallKeys::generate();
        let fw = serving(&keys, Ipv4Addr::LOCALHOST).await;
        let room = keys.room_scope("lounge");
        let (sa, sb, sc) = (socket().await, socket().await, socket().await);
        let a = client_now(&keys, &sa, &room);
        let b = client_now(&keys, &sb, &room);
        let c = client_now(&keys, &sc, &keys.room_scope("another room"));
        let ra = allocate(&sa, fw, &keys, &a).await;
        let rb = allocate(&sb, fw, &keys, &b).await;
        let rc = allocate(&sc, fw, &keys, &c).await;
        let real = [a.addr.port(), b.addr.port(), c.addr.port()];
        if ![ra.port(), rb.port(), rc.port()].iter().any(|p| real.contains(p)) {
            break (keys, fw, sa, sb, sc, a, b, c, ra, rb, rc);
        }
    };
    let now = unix_now();
    // B and C both open their side to A, so only the forwarder's rule stands in the way.
    assert_eq!(code(&ask(&sb, fw, &permission_request(&keys, &b, ra, now)).await), None);
    assert_eq!(code(&ask(&sc, fw, &permission_request(&keys, &c, ra, now)).await), Some(403), "C cannot even open to A");

    // A victim: a real socket on 127.0.0.1, the relayed IP, at a port no allocation holds.
    let victim = loop {
        let v = socket().await;
        let p = v.local_addr().unwrap().port();
        if ![ra.port(), rb.port(), rc.port()].contains(&p) {
            break v;
        }
    };
    let targets = [
        ("the victim at 127.0.0.1", victim.local_addr().unwrap()),
        ("another room's client", sc.local_addr().unwrap()),
        ("another room's allocation", rc),
        ("the room's other client's real address", sb.local_addr().unwrap()),
    ];
    for (i, (what, target)) in targets.iter().enumerate() {
        assert_eq!(code(&ask(&sa, fw, &permission_request(&keys, &a, *target, now)).await), Some(403), "a permission toward {what}");
        let ch = 0x4100 + i as u16;
        assert_eq!(code(&ask(&sa, fw, &bind_request(&keys, &a, ch, *target, now)).await), Some(403), "a channel toward {what}");
        sa.send_to(&send_indication(*target, b"flood"), fw).await.unwrap();
        sa.send_to(&channel_data(ch, b"flood"), fw).await.unwrap();
    }
    // An address with no allocation at all tries the same.
    let stranger = socket().await;
    stranger.send_to(&send_indication(victim.local_addr().unwrap(), b"flood"), fw).await.unwrap();

    assert_eq!(recv(&victim, QUIET).await, None, "the victim at 127.0.0.1 received a datagram");
    assert_eq!(recv(&sc, QUIET).await, None, "another room's client received a datagram");
    assert_eq!(recv(&sb, QUIET).await, None, "B received a datagram A addressed to its real address");

    // The same client toward B's allocation, which is in the room, does get through: the check
    // above is not passing because nothing is delivered at all.
    assert_eq!(code(&ask(&sa, fw, &permission_request(&keys, &a, rb, now)).await), None);
    sa.send_to(&send_indication(rb, b"allowed"), fw).await.unwrap();
    let got = recv(&sb, WAIT).await.expect("B, in the room, receives");
    assert_eq!(Message::parse(&got).unwrap().get(ATTR_DATA), Some(&b"allowed"[..]));
}

/// Credentials from a previous run of the relay do not work: the secret is made at start and
/// never stored, so a new run refuses them (401) over the wire too.
///
/// Seen red 2026-10-09 with `CallKeys::generate` returning a fixed secret: "left: None, right:
/// Some(401)".
#[tokio::test]
async fn credentials_from_another_run_are_refused_over_loopback() {
    let old = CallKeys::generate();
    let keys = CallKeys::generate();
    let fw = serving(&keys, Ipv4Addr::new(192, 0, 2, 1)).await;
    let sock = socket().await;
    let c = client_now(&old, &sock, &old.room_scope("lounge"));
    // Use the new run's nonce (the client would have been challenged by it).
    let req = authed(&keys, &c, METHOD_ALLOCATE, unix_now(), |b| {
        b.u32_attr(ATTR_REQUESTED_TRANSPORT, 17 << 24);
    });
    assert_eq!(code(&ask(&sock, fw, &req).await), Some(401));
}

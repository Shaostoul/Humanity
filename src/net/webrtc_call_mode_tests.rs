//! Each person chooses how their calls connect (docs/design/blocking-and-safe-mode.md 10p,
//! 2026-10-22): the candidates each mode offers, the public STUN server asked only in the third,
//! and, on this machine with two apps and the test forwarder (127.0.0.1 only), a Private person and
//! a Direct person in one voice room connecting through the relay, and two Direct people connecting
//! device to device with no relay at all.

use super::relay_only_tests::ready_in_room;
use super::test_forwarder::{Forwarder, Trap};
use super::*;

/// The relayed address `ready_in_room` hands out (TEST-NET-3).
const RELAYED: &str = "203.0.113.9:50000";

/// The candidate lines of the SDP in a queued voice signal.
fn sdp_candidates(signal: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(signal).unwrap();
    let sdp = v["data"]["sdp"].as_str().expect("an SDP").to_string();
    sdp.lines().filter(|l| l.starts_with("a=candidate:")).map(str::to_string).collect()
}

/// A browser-like voice offer from someone else.
fn a_voice_offer() -> Value {
    let mut rtc = Rtc::builder().build(Instant::now());
    let mut api = rtc.sdp_api();
    let _ = api.add_channel(CHANNEL_LABEL.to_string());
    let _ = api.add_media(MediaKind::Audio, Direction::SendRecv, None, None, None);
    let (offer, _pending) = api.apply().expect("offer produced");
    serde_json::to_value(&offer).unwrap()
}

fn direct(mgr: &mut WebrtcManager) {
    mgr.cmd_set_call_connection(CallConnection::Direct, Vec::new());
}

/// What each mode offers the other side. Private: the relayed address only (step E, unchanged).
/// Direct with the allocation live: this device's host address AND the relayed one, offered and
/// answered at once, and the connection takes datagrams that come straight to the socket. Direct
/// before the allocation: offered at once with the host address, nothing held back, and the
/// relayed address trickled once it comes, naming the offer's own first mid.
///
/// Seen red 2026-10-22 with `voice_plan` ignoring the mode (every voice connection relay only):
/// "Direct offers this device's address" (one candidate, the relayed one, 203.0.113.9 50000 typ
/// relay).
#[test]
fn each_mode_offers_its_own_candidates() {
    // Private, the default.
    let (mut mgr, outbound, _events) = test_manager("bb");
    ready_in_room(&mut mgr, "r1");
    mgr.cmd_offer_to("aa".into(), true, Some("r1".into()));
    let lines = sdp_candidates(&outbound.try_recv().expect("a voice offer"));
    assert_eq!(lines.len(), 1, "Private: only the relayed address: {lines:?}");
    assert!(lines[0].contains("typ relay"));

    // Direct, the allocation live.
    let (mut mgr, outbound, _events) = test_manager("bb");
    direct(&mut mgr);
    ready_in_room(&mut mgr, "r1");
    mgr.cmd_offer_to("aa".into(), true, Some("r1".into()));
    let offer = outbound.try_recv().expect("a voice offer");
    let lines = sdp_candidates(&offer);
    assert!(lines.iter().any(|l| l.contains("127.0.0.1") && l.contains("typ host")), "Direct offers this device's address: {lines:?}");
    assert!(lines.iter().any(|l| l.contains("203.0.113.9 50000 typ relay")), "and the relayed address: {lines:?}");
    let p = mgr.peers.get("aa").expect("the connection");
    assert!(!p.relay_only() && p.takes(false) && p.takes(true), "a Direct voice connection takes both paths");
    mgr.cmd_voice_signal("cc".into(), "r1".into(), "offer".into(), a_voice_offer());
    let answer = outbound.try_recv().expect("a voice answer");
    let lines = sdp_candidates(&answer);
    assert!(lines.iter().any(|l| l.contains("typ host")) && lines.iter().any(|l| l.contains("typ relay")), "the answer too: {lines:?}");

    // Direct, before the allocation: at once, the host address only, nothing deferred.
    let (mut mgr, outbound, _events) = test_manager("bb");
    direct(&mut mgr);
    mgr.cmd_offer_to("aa".into(), true, Some("r1".into()));
    let offer = outbound.try_recv().expect("offered at once, not held for the relay");
    let lines = sdp_candidates(&offer);
    assert!(lines.iter().all(|l| l.contains("typ host")) && !lines.is_empty(), "{lines:?}");
    assert!(mgr.deferred.is_empty(), "nothing waits for the relay");
    let offer_mid = first_mid_of(serde_json::from_str::<Value>(&offer).unwrap()["data"]["sdp"].as_str().unwrap()).expect("a mid");
    // The allocation comes: the relayed address is added and trickled.
    let relayed: SocketAddr = RELAYED.parse().unwrap();
    mgr.relay_ready_for_direct(&CallScope::Room("r1".into()), relayed);
    let ice: Value = serde_json::from_str(&outbound.try_recv().expect("the relayed address trickled")).unwrap();
    assert_eq!(ice["type"], "voice_room_signal");
    assert_eq!(ice["signal_type"], "ice");
    assert!(ice["data"]["candidate"].as_str().unwrap().contains("203.0.113.9 50000 typ relay"), "{ice}");
    assert_eq!(ice["data"]["sdpMid"].as_str(), Some(offer_mid.as_str()), "the offer's own first mid, which a browser has");
    assert!(mgr.peers.get("aa").is_some_and(|p| p.has_relayed));
    mgr.relay_ready_for_direct(&CallScope::Room("r1".into()), relayed);
    assert!(outbound.try_recv().is_err(), "trickled once");

    // Direct, the relay refused: still offered and answered, device to device.
    let (mut mgr, outbound, _events) = test_manager("bb");
    direct(&mut mgr);
    mgr.cmd_relay_unavailable(CallScope::Room("r1".into()));
    mgr.cmd_offer_to("aa".into(), true, Some("r1".into()));
    assert!(outbound.try_recv().is_ok(), "an offer without the server's relay");
    mgr.cmd_voice_signal("cc".into(), "r1".into(), "offer".into(), a_voice_offer());
    assert!(outbound.try_recv().is_ok(), "an answer without the server's relay");
}

/// A server-reflexive address (this device's public address) is never given to a connection that
/// keeps the address to itself. Seen red 2026-10-22 with the `!p.hides_address` filter taken out
/// of `apply_srflx_to_all_peers` and `add_and_trickle_srflx`: "a Private connection never gets
/// this device's address" (a `typ srflx` candidate went to "aa").
#[test]
fn a_private_connection_never_gets_the_public_address() {
    let (mut mgr, outbound, _events) = test_manager("bb");
    ready_in_room(&mut mgr, "r1");
    mgr.cmd_offer_to("aa".into(), true, Some("r1".into())); // Private
    let _ = outbound.try_recv();
    direct(&mut mgr);
    mgr.cmd_offer_to("cc".into(), true, Some("r1".into())); // Direct
    let _ = outbound.try_recv();
    mgr.apply_srflx_to_all_peers("198.51.100.7:40000".parse().unwrap());
    let sent: Vec<Value> = outbound.try_iter().map(|s| serde_json::from_str(&s).unwrap()).collect();
    assert!(sent.iter().all(|v| v["to"] != "aa"), "a Private connection never gets this device's address: {sent:?}");
    assert!(sent.iter().any(|v| v["to"] == "cc" && v["data"]["candidate"].as_str().unwrap_or("").contains("typ srflx")), "the Direct one does: {sent:?}");
}

/// The public STUN server is asked only in the third mode, and no longer once the person leaves
/// it; the server's own is never replaced by it. Everything here stays on 127.0.0.1: the
/// "public service" is a socket of the test's own, named by its address so nothing is looked up.
/// Seen red 2026-10-22 with `request_stun_servers` ignoring `public_stun`: nothing was looked up,
/// "the lookup came back: Timeout".
#[test]
fn the_public_service_is_asked_only_in_the_third_mode() {
    let public = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
    public.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
    let public_hp = public.local_addr().unwrap().to_string();
    let mut buf = [0u8; 128];
    let (mut mgr, _outbound, _events) = test_manager("bb");

    // Direct: a connection that shows the address asks STUN, but no public service.
    direct(&mut mgr);
    mgr.cmd_offer_to("aa".into(), true, Some("r1".into()));
    mgr.maybe_send_stun();
    assert!(mgr.stun_servers.is_empty(), "no server named, none asked");
    assert!(public.recv_from(&mut buf).is_err(), "Direct asks no public service");

    // The third mode: looked up on a helper thread, then asked.
    mgr.cmd_set_call_connection(CallConnection::DirectPublicStun, vec![public_hp.clone()]);
    mgr.last_stun_send = None;
    mgr.maybe_send_stun();
    let looked_up = mgr.rx_internal.recv_timeout(Duration::from_secs(5)).expect("the lookup came back");
    mgr.handle_command(looked_up);
    assert_eq!(mgr.stun_servers, vec![public.local_addr().unwrap()]);
    mgr.last_stun_send = None;
    mgr.maybe_send_stun();
    let (n, _) = public.recv_from(&mut buf).expect("the public service is asked in the third mode");
    assert!(n >= 20 && buf[0] == 0x00 && buf[1] == 0x01, "a STUN Binding Request");

    // Back to Direct: the public service is forgotten and never asked again; a lookup made for the
    // old choice that comes late is dropped.
    mgr.cmd_set_call_connection(CallConnection::Direct, Vec::new());
    assert!(mgr.stun_servers.is_empty() && mgr.stun_pending.is_empty());
    mgr.handle_command(Command::StunServers { servers: vec![public.local_addr().unwrap()], for_public: vec![public_hp] });
    assert!(mgr.stun_servers.is_empty(), "a late lookup for the old choice is dropped");
    mgr.last_stun_send = None;
    mgr.maybe_send_stun();
    assert!(public.recv_from(&mut buf).is_err(), "never asked again once left");
}

/// The SDP's first mid is read, and only the first m-line's.
#[test]
fn the_first_mid_is_read() {
    let sdp = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=mid:xy7\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=mid:zz\r\n";
    assert_eq!(first_mid_of(sdp).as_deref(), Some("xy7"));
    assert_eq!(first_mid_of("v=0\r\nm=audio 9 x\r\nm=video 9 y\r\na=mid:1\r\n"), None, "a mid after the second m-line is not the first's");
}

/// Pump two managers' signals into each other until `done` holds or 20 s pass. Returns every
/// signal each sent, and every event each emitted.
#[allow(clippy::type_complexity)]
fn pump(
    a: &WebrtcHandle,
    b: &WebrtcHandle,
    mut done: impl FnMut(&[WebrtcEvent], &[WebrtcEvent]) -> bool,
    mut each_turn: impl FnMut(&[WebrtcEvent], &[WebrtcEvent]),
) -> (Vec<Value>, Vec<Value>, Vec<WebrtcEvent>, Vec<WebrtcEvent>) {
    let (mut sa, mut sb, mut ea, mut eb) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && !done(&ea, &eb) {
        for (from, to, key, log) in [(a, b, "aa", &mut sa), (b, a, "bb", &mut sb)] {
            for json in from.poll_outbound() {
                let v: Value = serde_json::from_str(&json).unwrap();
                assert_eq!(v["type"], "voice_room_signal", "only voice signals: {json}");
                to.submit_voice_signal(
                    key.into(),
                    v["room_id"].as_str().unwrap().into(),
                    v["signal_type"].as_str().unwrap().into(),
                    v["data"].clone(),
                );
                log.push(v);
            }
        }
        ea.extend(a.poll_events());
        eb.extend(b.poll_events());
        each_turn(&ea, &eb);
        thread::sleep(Duration::from_millis(10));
    }
    (sa, sb, ea, eb)
}

/// Every candidate line a manager sent: in its SDPs and trickled.
fn candidate_lines(signals: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for v in signals {
        if let Some(sdp) = v["data"]["sdp"].as_str() {
            out.extend(sdp.lines().filter_map(|l| l.trim().strip_prefix("a=")).filter(|l| l.starts_with("candidate:")).map(str::to_string));
        }
        if let Some(c) = v["data"]["candidate"].as_str().filter(|c| !c.is_empty()) {
            out.push(c.to_string());
        }
    }
    out
}

fn frame_from(events: &[WebrtcEvent], peer: &str) -> Option<Vec<u8>> {
    events.iter().find_map(|e| match e {
        WebrtcEvent::VoiceFrame { peer: p, opus } if p == peer => Some(opus.clone()),
        _ => None,
    })
}

fn connected(events: &[WebrtcEvent]) -> bool {
    events.iter().any(|e| matches!(e, WebrtcEvent::VoiceConnected { .. }))
}

/// A Private person (a) and a Direct person (b) in one voice room, through the test forwarder on
/// 127.0.0.1: they connect, and voice crosses both ways, through the relay. a's signals carry only
/// relayed addresses; b's carry its own address too. The forwarder gives out relayed addresses
/// that are `Trap` sockets: b, being Direct, may try its own address toward a's relayed one (in the
/// real forwarder a port that is never bound, so dropped), but nothing ever arrives at a trap from
/// a's side. b dials before its credentials come (its relayed address is then trickled).
///
/// Seen red 2026-10-22 with `relay_ready_for_direct` not called when b's allocation came (b's
/// relayed address never offered): no relayed pair could form, "both connected: a [RelayReady {
/// scope: Room(\"r1\") }, Closed { peer: \"bb\" }] / b [..., Closed { peer: \"aa\" }]". And with
/// `voice_plan` ignoring the mode: "b (Direct) offered its own address".
#[test]
fn a_private_and_a_direct_person_connect_through_the_relay() {
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
    a.set_call_connection(CallConnection::Private, Vec::new());
    b.set_call_connection(CallConnection::Direct, Vec::new());

    // b dials first, before any credentials: a Direct offer goes at once.
    b.offer_to_voice("aa".into(), "r1".into());
    thread::sleep(Duration::from_millis(100));
    a.use_relay(creds("1:a", "pa"));
    b.use_relay(creds("1:b", "pb"));

    let to_b = vec![0xA1, 0xA2, 0xA3, 0xA4];
    let to_a = vec![0xB1, 0xB2, 0xB3, 0xB4];
    let (sa, sb, ea, eb) = pump(
        &a,
        &b,
        |ea, eb| frame_from(eb, "aa").is_some() && frame_from(ea, "bb").is_some(),
        |ea, eb| {
            if connected(ea) {
                a.send_voice("bb".into(), to_b.clone());
            }
            if connected(eb) {
                b.send_voice("aa".into(), to_a.clone());
            }
        },
    );
    assert!(connected(&ea) && connected(&eb), "both connected: a {ea:?} / b {eb:?}");
    assert_eq!(frame_from(&eb, "aa"), Some(to_b), "voice crossed a to b");
    assert_eq!(frame_from(&ea, "bb"), Some(to_a), "voice crossed b to a");
    assert!(fwd.delivered() > 0, "through the forwarder");

    let a_lines = candidate_lines(&sa);
    assert!(!a_lines.is_empty() && a_lines.iter().all(|l| l.contains("typ relay")), "a (Private) offered only relayed addresses: {a_lines:?}");
    let b_lines = candidate_lines(&sb);
    let b_host: SocketAddr = b_lines
        .iter()
        .find(|l| l.contains("typ host"))
        .and_then(|l| Candidate::from_sdp_string(l).ok())
        .map(|c| c.addr())
        .expect("b (Direct) offered its own address");
    assert!(b_lines.iter().any(|l| l.contains("typ relay")), "and its relayed one: {b_lines:?}");

    // Nothing from a reached a relayed address around the forwarder: whatever a trap got came
    // from b's own socket.
    for t in &traps {
        for from in t.received() {
            assert_eq!(from, b_host, "only the Direct side tries its own address, never a ({})", t.addr);
        }
    }
    // a's relay may ask once toward b's own address (str0m pairs everything it was given) and is
    // refused, and the forwarder sends nothing there. Anything else refused would be a client
    // reaching for an address it should not (the start-up permission race is as in the Private
    // rig).
    let refused: Vec<String> = fwd
        .refused()
        .into_iter()
        .filter(|r| !r.ends_with("without the receiver's permission"))
        .filter(|r| !r.contains(&format!("toward {b_host}")))
        .collect();
    assert!(refused.is_empty(), "the forwarder refused nothing else: {refused:?}");
}

/// Two Direct people with no relay at all (the server cannot carry calls): they connect device to
/// device and voice crosses. Seen red 2026-10-22 with `voice_plan` holding Direct connections for
/// the relay like Private ones: nothing was offered, "both connected: a [] / b []".
#[test]
fn two_direct_people_connect_device_to_device_without_the_relay() {
    let a = WebrtcManager::start_on("aa".into(), String::new(), "127.0.0.1:0");
    let b = WebrtcManager::start_on("bb".into(), String::new(), "127.0.0.1:0");
    a.set_call_connection(CallConnection::Direct, Vec::new());
    b.set_call_connection(CallConnection::Direct, Vec::new());
    let room = CallScope::Room("r1".into());
    a.relay_unavailable(room.clone());
    b.relay_unavailable(room);
    thread::sleep(Duration::from_millis(50));
    a.offer_to_voice("bb".into(), "r1".into());
    let payload = vec![0xC1, 0xC2, 0xC3, 0xC4];
    let (sa, _sb, ea, eb) = pump(
        &a,
        &b,
        |_, eb| frame_from(eb, "aa").is_some(),
        |ea, _| {
            if connected(ea) {
                a.send_voice("bb".into(), payload.clone());
            }
        },
    );
    assert!(connected(&ea) && connected(&eb), "both connected: a {ea:?} / b {eb:?}");
    assert_eq!(frame_from(&eb, "aa"), Some(payload), "voice crossed device to device");
    assert!(candidate_lines(&sa).iter().all(|l| !l.contains("typ relay")), "no relay was involved");
}

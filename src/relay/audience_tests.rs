// Every signature a person makes for a server names that server (2026-11-21, the signed-requests
// round 2, D1, D2 and D6), through real relays on 127.0.0.1.
//
// THE FAULT (the review of round 1, findings 1 and 2): a request named no server, so a server the
// person sent one to could replay it at another server inside the five-minute window (the desktop
// signs a channel read for every channel a server lists; a server listing a channel named like
// another server's members-only room read that room as the person). And a sign-in named no server:
// a server opened a socket to another server as the person, passed that server's challenge through
// to the person's app, returned the signature, and was the person there, where slash commands run
// on the socket's identity (`/link` made a link code it redeemed with its own key).
//
// RED EVIDENCE (2026-11-21, `cargo test --features relay --no-default-features --lib --
// audience_tests`, each check taken out in turn):
// - `a_request_is_good_only_at_the_server_it_names`: with the `names.accepts` test taken out of
//   `signed_request::check_for` (the audience taken from the Host header unchecked), "B refuses a
//   request signed for A, even when told it is A" failed: left 200, right 401.
// - `a_sign_in_is_good_only_at_the_server_it_names` and `a_server_cannot_pass_another_servers_
//   challenge_through`: with the `names.accepts` filter taken out of `verify_identify` (any audience
//   the message names taken at its word, which is what the old shape without an audience amounted
//   to), "B refuses a sign-in made for A" and "B refuses the passed-through proof" failed: B bound
//   the socket and answered peer_list.
// - `a_request_from_before_a_restart_is_refused_after_it`: with the `started_at_ms` test taken out
//   of `check_for`, "the restarted relay refuses a request signed before it started" failed: 200.
//
// ROUND 3 (the second review of 2026-11-21): relays that SHARE a name (every relay on a loopback
// port, every home node at a common LAN address) took each other's signatures, a sign-in could name
// any of a relay's names whatever its socket was opened to, and the words listed every name. Red
// evidence for the round-3 tests below is in each test's own comment.

use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use crate::relay::audience::{test_identify_response, test_request_sig, OwnNames, DEV_LOOPBACK_NAMES_ENV, PUBLIC_HOSTS_ENV};
use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;

type Sock = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Relay {
    state: Arc<RelayState>,
    port: u16,
}

impl Relay {
    fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    fn aud(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

async fn relay(tag: &str) -> Relay {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = crate::relay::serve_relay(Storage::open_temp(tag), listener).await;
    crate::relay::default_channels::make_missing(&state.db, true);
    Relay { state, port }
}

fn keypair(seed: u8) -> (DilithiumKeypair, String) {
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    let key = hex::encode(kp.public_key());
    (kp, key)
}

/// A member of `r` (named long enough ago to post).
fn member(r: &Relay, key: &str, name: &str) {
    r.state.db.register_name(name, key).unwrap();
    r.state.db.with_conn(|c| c.execute("UPDATE registered_names SET registered_at = 0 WHERE public_key = ?1", [key])).unwrap();
    r.state.db.join_server(key, name).unwrap();
}

/// POST `body` to `path` on `r`, its Host header naming `host` when given: the status and words.
async fn post(r: &Relay, host: Option<&str>, path: &str, body: &Value) -> (u16, String) {
    let mut req = reqwest::Client::new().post(format!("{}{path}", r.base())).json(body);
    if let Some(h) = host {
        req = req.header(reqwest::header::HOST, h);
    }
    let res = req.send().await.expect("the relay answers");
    (res.status().as_u16(), res.text().await.unwrap_or_default())
}

/// The next frame `pick` accepts, within ten seconds.
async fn frame_where(sock: &mut Sock, what: &str, pick: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(Ok(m)) = sock.next().await {
            if let WsMsg::Text(t) = m {
                if let Ok(v) = serde_json::from_str::<Value>(&t) {
                    if pick(&v) {
                        return v;
                    }
                }
            }
        }
        panic!("the socket closed before {what}");
    })
    .await
    .unwrap_or_else(|_| panic!("no {what} within ten seconds"))
}

/// Open /ws on `r` and ask to sign in as `key`: the socket and the relay's challenge nonce.
async fn challenge(r: &Relay, key: &str, name: &str) -> (Sock, String) {
    let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", r.port)).await.unwrap();
    sock.send(WsMsg::Text(json!({ "type": "identify", "public_key": key, "display_name": name }).to_string().into())).await.unwrap();
    let c = frame_where(&mut sock, "a challenge", |f| f["type"] == "identify_challenge").await;
    (sock, c["nonce"].as_str().unwrap().to_string())
}

/// D1, requests: a request signed for A is refused by B, sent plainly (its signature does not hold
/// for B's name) or naming A as its Host (A is not one of B's names: the words name both), and the
/// same request is accepted by A.
#[test]
fn a_request_is_good_only_at_the_server_it_names() {
    on_a_big_stack(|| async {
        let (a, b) = (relay("aud_req_a").await, relay("aud_req_b").await);
        let (kp, key) = keypair(31);
        member(&a, &key, "Ann");
        member(&b, &key, "Ann");
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": key, "timestamp": ts, "sig": test_request_sig(&kp, &a.base(), "account_export", ts) });

        let (code, _) = post(&b, None, "/api/account/export", &body).await;
        assert_eq!(code, 401, "B refuses a request signed for A");
        let (code, words) = post(&b, Some(&a.aud()), "/api/account/export", &body).await;
        assert_eq!(code, 401, "B refuses a request signed for A, even when told it is A");
        // One sentence for every audience refusal since L1 (2026-11-21): it names no address.
        assert_eq!(words, crate::relay::audience::REFUSED_AS_REACHED, "the refusal names no address, never B's names");

        let (code, words) = post(&a, None, "/api/account/export", &body).await;
        assert_eq!(code, 200, "A takes it: {words}");
        assert!(b.state.replay.is_empty(), "nothing B refused landed in its replay memory");
    });
}

/// D1, sign-ins: a sign-in made for A is refused by B in words naming both, and never binds; the
/// same person signs in to B with a proof made for B.
#[test]
fn a_sign_in_is_good_only_at_the_server_it_names() {
    on_a_big_stack(|| async {
        let (a, b) = (relay("aud_id_a").await, relay("aud_id_b").await);
        let (kp, key) = keypair(32);
        let (mut sock, nonce) = challenge(&b, &key, "Bea").await;
        sock.send(WsMsg::Text(test_identify_response(&kp, &a.aud(), &nonce, &key).to_string().into())).await.unwrap();
        let told = frame_where(&mut sock, "the refusal", |f| f["type"] == "system" || f["type"] == "peer_list").await;
        assert_eq!(told["type"], "system", "B refuses a sign-in made for A: {told}");
        let words = told["message"].as_str().unwrap_or_default();
        // One sentence for every audience refusal since L1 (2026-11-21): it names no address.
        assert_eq!(words, crate::relay::audience::REFUSED_AS_REACHED, "the refusal names no address, never B's names");

        let (mut sock, nonce) = challenge(&b, &key, "Bea").await;
        sock.send(WsMsg::Text(test_identify_response(&kp, &b.aud(), &nonce, &key).to_string().into())).await.unwrap();
        frame_where(&mut sock, "signed in", |f| f["type"] == "peer_list").await;
    });
}

/// D1 and D2, the attack itself: a server opens a socket to B as the person, passes B's challenge
/// through to the person's app, and returns the proof the app makes. The app signs for the server
/// it opened (the attacker's), so B refuses it, the socket stays unbound, and `/link` on it makes
/// no link code. Slash commands need nothing more than the socket's identity, proven to THIS relay
/// by a fresh nonce bound to its name (relay.rs runs every command after that proof only).
#[test]
fn a_server_cannot_pass_another_servers_challenge_through() {
    on_a_big_stack(|| async {
        let (attacker, b) = (relay("aud_pass_a").await, relay("aud_pass_b").await);
        let (victim_kp, victim) = keypair(33);
        member(&b, &victim, "Vic");
        let (mut sock, nonce_from_b) = challenge(&b, &victim, "Vic").await;
        // The victim's app answers what it was handed, for the server it opened: the attacker's.
        let proof = test_identify_response(&victim_kp, &attacker.aud(), &nonce_from_b, &victim);
        sock.send(WsMsg::Text(proof.to_string().into())).await.unwrap();
        let told = frame_where(&mut sock, "the refusal", |f| f["type"] == "system" || f["type"] == "peer_list").await;
        assert_eq!(told["type"], "system", "B refuses the passed-through proof: {told}");
        // The attacker tries the command anyway, and the proof again with B's name written in.
        let _ = sock.send(WsMsg::Text(json!({ "type": "chat", "content": "/link", "timestamp": 1, "channel": "general" }).to_string().into())).await;
        let mut relabelled = proof.clone();
        relabelled["audience"] = json!(b.aud());
        let _ = sock.send(WsMsg::Text(relabelled.to_string().into())).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let codes: i64 = b.state.db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM link_codes", [], |r| r.get(0))).unwrap();
        assert_eq!(codes, 0, "no link code was made for the victim's name");
        assert!(!b.state.peers.read().await.contains_key(&victim), "the socket never became the victim");
    });
}

/// D6: a relay forgets its replay memory when it restarts, so a request first used just before
/// would work once more; a request signed before the relay's start is refused instead. "Restart"
/// here is a fresh relay process answering to the same address (a new state with the first one's
/// names, its requests sent with that Host).
#[test]
fn a_request_from_before_a_restart_is_refused_after_it() {
    on_a_big_stack(|| async {
        let first = relay("aud_restart_1").await;
        let (kp, key) = keypair(34);
        member(&first, &key, "Rex");
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": key, "timestamp": ts, "sig": test_request_sig(&kp, &first.base(), "account_export", ts) });
        assert_eq!(post(&first, None, "/api/account/export", &body).await.0, 200, "used once before the restart");
        assert_eq!(post(&first, None, "/api/account/export", &body).await.0, 401, "and not twice");

        tokio::time::sleep(Duration::from_millis(20)).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(RelayState::new(Storage::open_temp("aud_restart_2")));
        let _ = state.own_names.set(OwnNames::loopback(format!("127.0.0.1:{}", first.port).parse().unwrap()));
        let router = crate::relay::build_router(state.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>()).await;
        });
        let restarted = Relay { state, port };
        member(&restarted, &key, "Rex");
        assert!(restarted.state.started_at.wall_ms(crate::relay::signed_request::now_ms()) > ts, "test setup: the restart came after the signature");
        let (code, words) = post(&restarted, Some(&first.aud()), "/api/account/export", &body).await;
        assert_eq!(code, 401, "the restarted relay refuses a request signed before it started: {words}");
        assert!(words.contains("restarted"), "{words}");
        // A request signed after the restart is taken.
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": key, "timestamp": ts, "sig": test_request_sig(&kp, &first.base(), "account_export", ts) });
        let (code, words) = post(&restarted, Some(&first.aud()), "/api/account/export", &body).await;
        assert_eq!(code, 200, "a request made after the restart is taken: {words}");
    });
}

/// D3 at the door: a request whose key is spelled in upper case is refused before anything is
/// verified or remembered, so one signature cannot be spent once per spelling. Red evidence
/// (2026-11-21, the `is_canonical_key` test taken out of `check_for` and the replay memory hashing
/// the key's text as given, as round 1 did): "the upper-cased key is refused" failed, left 200
/// (the same signature read the groups again under a fresh replay slot).
#[test]
fn a_key_spelled_in_upper_case_is_refused() {
    on_a_big_stack(|| async {
        let r = relay("aud_case").await;
        let (kp, key) = keypair(35);
        member(&r, &key, "Cas");
        let ts = crate::relay::signed_request::next_test_timestamp();
        let sig = test_request_sig(&kp, &r.base(), &crate::relay::api_v2_objects::group_read_purpose("mine"), ts);
        let lower = json!({ "key": key, "timestamp": ts, "sig": sig });
        let upper = json!({ "key": key.to_ascii_uppercase(), "timestamp": ts, "sig": sig });
        assert_eq!(post(&r, None, "/api/v2/groups", &lower).await.0, 200, "the canonical key reads");
        let (code, words) = post(&r, None, "/api/v2/groups", &upper).await;
        assert_eq!(code, 400, "the upper-cased key is refused: {words}");
        assert_eq!(post(&r, None, "/api/v2/groups", &lower).await.0, 401, "and the signature stays spent");
    });
}

/// A relay on 127.0.0.1 answering to the names `names` gives for its listening address.
async fn relay_named(tag: &str, names: impl FnOnce(std::net::SocketAddr) -> OwnNames) -> Relay {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(RelayState::new(Storage::open_temp(tag)));
    let _ = state.own_names.set(names(addr));
    let router = crate::relay::build_router(state.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>()).await;
    });
    crate::relay::default_channels::make_missing(&state.db, true);
    Relay { state, port: addr.port() }
}

/// The names from these settings (the relay's environment).
fn names_from(pairs: &'static [(&'static str, &'static str)]) -> impl FnOnce(std::net::SocketAddr) -> OwnNames {
    move |addr| OwnNames::from_settings(|k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string()), addr)
}

/// POST `body` to `path` on `r` with the Host `host` and, when given, the client address `from`
/// (a client connecting straight from that address: the test-only `client_addr::TEST_RAW_PEER`
/// header, which the edge takes as the connection's own address; a written `X-Real-IP` never makes
/// a client local since H1): the status and words.
async fn post_as(r: &Relay, host: &str, from: Option<&str>, path: &str, body: &Value) -> (u16, String) {
    let mut req = reqwest::Client::new().post(format!("{}{path}", r.base())).json(body).header(reqwest::header::HOST, host);
    if let Some(ip) = from {
        req = req.header(crate::relay::client_addr::TEST_RAW_PEER, ip);
    }
    let res = req.send().await.expect("the relay answers");
    (res.status().as_u16(), res.text().await.unwrap_or_default())
}

/// [`challenge`] over a socket whose upgrade names the Host `host` and, when given, comes from `from`.
async fn challenge_as(r: &Relay, host: &str, from: Option<&str>, key: &str, name: &str) -> (Sock, String) {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("ws://127.0.0.1:{}/ws", r.port).into_client_request().unwrap();
    req.headers_mut().insert("host", host.parse().unwrap());
    if let Some(ip) = from {
        req.headers_mut().insert(crate::relay::client_addr::TEST_RAW_PEER, ip.parse().unwrap());
    }
    let (mut sock, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    sock.send(WsMsg::Text(json!({ "type": "identify", "public_key": key, "display_name": name }).to_string().into())).await.unwrap();
    let c = frame_where(&mut sock, "a challenge", |f| f["type"] == "identify_challenge").await;
    (sock, c["nonce"].as_str().unwrap().to_string())
}

/// The first answer to a sign-in proof: a refusal (`system`) or the signed-in `peer_list`.
async fn answer_to(sock: &mut Sock, proof: &Value) -> Value {
    sock.send(WsMsg::Text(proof.to_string().into())).await.unwrap();
    frame_where(sock, "an answer", |f| f["type"] == "system" || f["type"] == "peer_list").await
}

/// C1: a sign-in must be signed for the address its socket was opened to, as an HTTP request must
/// for its Host. A relay with both a public and a loopback name (the dev switch on) refuses a proof
/// signed for one of them over a socket opened to the other, in words that name only what it was
/// signed for. Red evidence (2026-11-21, the Host comparison taken out of `OwnNames::admit`, round
/// 2's rule back): "a proof for a listed name over a socket opened to another" failed, the socket
/// signed in (peer_list).
#[test]
fn a_sign_in_must_name_the_address_its_socket_was_opened_to() {
    on_a_big_stack(|| async {
        let r = relay_named("aud_c1", names_from(&[(PUBLIC_HOSTS_ENV, "relay.example.org"), (DEV_LOOPBACK_NAMES_ENV, "1")])).await;
        let (kp, key) = keypair(36);
        let (mut sock, nonce) = challenge_as(&r, "relay.example.org", None, &key, "Cyd").await;
        let told = answer_to(&mut sock, &test_identify_response(&kp, &r.base(), &nonce, &key)).await;
        assert_eq!(told["type"], "system", "a proof for a listed name over a socket opened to another: {told}");
        let words = told["message"].as_str().unwrap_or_default();
        assert_eq!(words, crate::relay::audience::REFUSED_AS_REACHED, "the one sentence (L1)");
        assert!(!words.contains("relay.example.org"), "the words never list the relay's names: {words}");

        let (mut sock, nonce) = challenge(&r, &key, "Cyd").await;
        let told = answer_to(&mut sock, &test_identify_response(&kp, "https://relay.example.org", &nonce, &key)).await;
        assert_eq!(told["type"], "system", "the other way round too: {told}");

        let (mut sock, nonce) = challenge_as(&r, "relay.example.org", None, &key, "Cyd").await;
        let told = answer_to(&mut sock, &test_identify_response(&kp, "https://relay.example.org", &nonce, &key)).await;
        assert_eq!(told["type"], "peer_list", "a proof for the socket's own address signs in: {told}");
    });
}

/// C2, the shared-name case: two relays on loopback that share a name (every home node at
/// 192.168.1.5:3210 does). The victim signs in at Mallory's node from Mallory's household network;
/// Mallory opens a socket to Alice's node from the internet naming that same address and passes
/// Alice's challenge through: refused, a LAN name from a public client. A request the victim signed
/// for that address, replayed at Alice from the internet, is refused too; from Alice's own network
/// the name works. Red evidence (2026-11-21, `peer_may_use` made to answer true for every reach):
/// "Alice refuses the passed-through LAN proof from the internet" failed, the socket signed in.
#[test]
fn relays_that_share_a_name_do_not_take_each_others_signatures() {
    on_a_big_stack(|| async {
        const LAN: &[(&str, &str)] = &[(PUBLIC_HOSTS_ENV, "192.168.1.5:3210")];
        let (mallory, alice) = (relay_named("aud_share_m", names_from(LAN)).await, relay_named("aud_share_a", names_from(LAN)).await);
        let (kp, victim) = keypair(37);
        member(&alice, &victim, "Vic");
        let opened = "http://192.168.1.5:3210";
        // The real sign-in, at Mallory's node, from her household's network.
        let (mut sock, nonce) = challenge_as(&mallory, "192.168.1.5:3210", Some("192.168.1.20"), &victim, "Vic").await;
        assert_eq!(answer_to(&mut sock, &test_identify_response(&kp, opened, &nonce, &victim)).await["type"], "peer_list");
        // The pass-through, at Alice's node, from the internet.
        let (mut sock, nonce) = challenge_as(&alice, "192.168.1.5:3210", Some("203.0.113.9"), &victim, "Vic").await;
        let told = answer_to(&mut sock, &test_identify_response(&kp, opened, &nonce, &victim)).await;
        assert_eq!(told["type"], "system", "Alice refuses the passed-through LAN proof from the internet: {told}");
        assert_eq!(told["message"].as_str().unwrap_or_default(), crate::relay::audience::REFUSED_AS_REACHED, "{told}");
        assert!(!alice.state.peers.read().await.contains_key(&victim));
        // A request signed for that address: refused from the internet, taken from Alice's network.
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": victim, "timestamp": ts, "sig": test_request_sig(&kp, opened, "account_export", ts) });
        let (code, words) = post_as(&alice, "192.168.1.5:3210", Some("203.0.113.9"), "/api/account/export", &body).await;
        assert_eq!(code, 401, "a LAN name from a public client is refused: {words}");
        let (code, words) = post_as(&alice, "192.168.1.5:3210", Some("192.168.1.20"), "/api/account/export", &body).await;
        assert_eq!(code, 200, "from Alice's own network the name works: {words}");
    });
}

/// C2: a relay listening on loopback (a public relay behind nginx is one) has no loopback name
/// without the dev switch, so a signature made for `127.0.0.1:{port}` is refused there; its public
/// name works. Red evidence (2026-11-21, round 2's loopback names from the bind address put back in
/// `from_settings`): "a loopback-named signature at a relay without the dev switch" failed, left 200.
#[test]
fn a_loopback_bound_relay_without_the_dev_switch_refuses_a_loopback_name() {
    on_a_big_stack(|| async {
        let r = relay_named("aud_noswitch", names_from(&[(PUBLIC_HOSTS_ENV, "relay.example.org")])).await;
        let (kp, key) = keypair(38);
        member(&r, &key, "Lou");
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": key, "timestamp": ts, "sig": test_request_sig(&kp, &r.base(), "account_export", ts) });
        let (code, words) = post(&r, None, "/api/account/export", &body).await;
        assert_eq!(code, 401, "a loopback-named signature at a relay without the dev switch: {words}");
        assert_eq!(words, crate::relay::audience::REFUSED_AS_REACHED, "the one sentence, naming nothing (L1)");
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": key, "timestamp": ts, "sig": test_request_sig(&kp, "https://relay.example.org", "account_export", ts) });
        let (code, words) = post_as(&r, "relay.example.org", Some("203.0.113.9"), "/api/account/export", &body).await;
        assert_eq!(code, 200, "its public name works from anywhere: {words}");
    });
}

/// C6: a request older than the relay's start, long after the start, is a slow clock, and the words
/// say so. A server clock fast at the start and set right since (round 3 let that step refuse
/// nothing) now keeps the start where the fast clock put it (L2, 2026-11-21: otherwise a clock
/// stepped back lets requests from before the restart in), so a new request is refused for as long
/// as the clock was ahead, in words that say the server's clock was set back.
/// Red evidence (2026-11-21): with the clock hint taken out (round 2's refusal, no seconds), "a slow
/// clock" failed, the refusal BeforeStart(None); with the `StartAhead` branch taken out of
/// `check_for`, "the words say the server's clock was set back" failed, left BeforeStart(None).
#[test]
fn a_slow_clock_is_told_so_and_a_server_clock_set_back_says_so() {
    on_a_big_stack(|| async {
        use crate::relay::signed_request::{check, now_ms, SignedRefusal, StartedAt};
        let (kp, key) = keypair(39);
        let aud = crate::relay::audience::TEST_AUDIENCE;
        // Started a minute ago; this device's clock is 90 seconds behind.
        let mut state = RelayState::new(Storage::open_temp("aud_clock"));
        let now = now_ms();
        state.started_at = StartedAt::at(now - 60_000, Duration::from_secs(60));
        let ts = now - 90_000;
        let refused = check(&state, &key, ts, &test_request_sig(&kp, aud, "account_export", ts), "account_export", "t").await;
        let Err(SignedRefusal::BeforeStart(Some(s))) = refused.clone() else { panic!("a slow clock: {refused:?}") };
        assert!((89..=92).contains(&s), "about 90 seconds: {s}");
        assert!(refused.unwrap_err().words().contains("Your clock is behind this server's"), "the words say the clock is behind");
        // The server's clock was an hour ahead when it started and has since been put right.
        state.started_at = StartedAt::at(now + 3_600_000, Duration::from_secs(60));
        let ts = crate::relay::signed_request::next_test_timestamp();
        let refused = check(&state, &key, ts, &test_request_sig(&kp, aud, "account_export", ts), "account_export", "t").await;
        let Err(SignedRefusal::StartAhead(m)) = refused.clone() else { panic!("the words say the server's clock was set back: {refused:?}") };
        assert!((59..=61).contains(&m), "about an hour: {m} minutes");
        assert!(refused.unwrap_err().words().contains("clock was set back"), "the words say so");
    });
}

/// POST `body` to `path` on `r` from this computer (127.0.0.1) with the Host `host` and the extra
/// headers `extra`, written as the client pleases: the status and words.
async fn post_with(r: &Relay, host: &str, extra: &[(&str, &str)], path: &str, body: &Value) -> (u16, String) {
    let mut req = reqwest::Client::new().post(format!("{}{path}", r.base())).json(body).header(reqwest::header::HOST, host);
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let res = req.send().await.expect("the relay answers");
    (res.status().as_u16(), res.text().await.unwrap_or_default())
}

/// [`challenge`] over a socket from this computer whose upgrade names the Host `host` and carries the
/// extra headers `extra`.
async fn challenge_with(r: &Relay, host: &str, extra: &[(&str, &str)], key: &str, name: &str) -> (Sock, String) {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("ws://127.0.0.1:{}/ws", r.port).into_client_request().unwrap();
    req.headers_mut().insert("host", host.parse().unwrap());
    for (k, v) in extra {
        req.headers_mut().insert(tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
    }
    let (mut sock, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    sock.send(WsMsg::Text(json!({ "type": "identify", "public_key": key, "display_name": name }).to_string().into())).await.unwrap();
    let c = frame_where(&mut sock, "a challenge", |f| f["type"] == "identify_challenge").await;
    (sock, c["nonce"].as_str().unwrap().to_string())
}

/// H1 (the review of rounds 3 and 4, 2026-11-21): whether a client is on this computer or this local
/// network is decided from the connection's OWN address, never from a header a client or a proxy
/// wrote, and a connection from this computer is this computer, never the local network. A home
/// node answering to an onion address and its LAN address (so it lists no loopback name), with Tor
/// on the same computer handing every visitor over from 127.0.0.1 and no header: a request and a
/// sign-in made for the LAN address, arriving that way, are refused (round 2's pass-through: an
/// attacker in a cafe answering at a friend's saved LAN address passes the friend's proof on to
/// the node through Tor). And a visitor reaching the relay directly who writes an `X-Real-IP` or an
/// `X-Forwarded-For` on the node's network is refused too: a header never makes a client local.
/// The onion name works through Tor.
/// Red evidence (2026-11-21, before the fix): "a LAN-named request through a Tor-style forwarder"
/// failed, left 200 (a loopback client counted as the local network); and the written X-Real-IP
/// was admitted before the fix by design: round 3's `relays_that_share_a_name_do_not_take_each_
/// others_signatures` sent `X-Real-IP: 192.168.1.20` from 127.0.0.1 and asserted 200 (now it
/// stands in for a client on the network with the test-only raw-address header instead). With the
/// header rule taken out of client_addr.rs `direct_peer` (only the loopback rule left), "a LAN-named
/// request forwarded by a proxy on the network" failed, left 200.
/// L6 of the review of round 5 (2026-11-21): the written-header asserts were sent from 127.0.0.1,
/// so the loopback rule refused them and they passed without the header rule. Each now pins its own
/// rule: the written headers come from a client ON the network (which only the header rule
/// refuses), and the loopback-named request through nginx on this computer is refused by the
/// header rule alone. Every case is checked before the test fails, so one red run shows which rule
/// each holds. Red evidence (2026-11-21, the header rule taken out of `direct_peer`): exactly the
/// five header cases failed, each admitted (the written X-Real-IP and X-Forwarded-For from the
/// network, nginx's loopback-named request, the proxy on the network, the sign-in with a written
/// X-Real-IP), the Tor-style and the taken cases holding; with the loopback rule taken out of
/// `peer_may_use` instead (a loopback client on every network), exactly the two Tor-style cases
/// failed (the request answered 200, the sign-in signed in).
#[test]
fn a_forwarded_client_or_a_written_address_is_never_on_the_local_network() {
    on_a_big_stack(|| async {
        const NODE: &[(&str, &str)] = &[(PUBLIC_HOSTS_ENV, "abcdefghijklmnop.onion,192.168.1.5:3210"), (DEV_LOOPBACK_NAMES_ENV, "1")];
        let r = relay_named("aud_h1", names_from(NODE)).await;
        let (kp, key) = keypair(40);
        member(&r, &key, "Hal");
        let lan = "http://192.168.1.5:3210";
        // A signed read of the main channel (chat_access.rs): signed_request::check, and no limit a
        // minute, so a red run's admitted requests do not change what the next asserts see.
        let read = |opened: &str| {
            let ts = crate::relay::signed_request::next_test_timestamp();
            let purpose = humanity_protocol::chat_access::read_purpose("messages", humanity_protocol::chat_access::MAIN_CHANNEL);
            json!({ "key": key, "timestamp": ts, "sig": test_request_sig(&kp, opened, &purpose, ts), "limit": 1 })
        };
        let peer = crate::relay::client_addr::TEST_RAW_PEER;
        let here = format!("127.0.0.1:{}", r.port);
        // (what, Host, headers, the address it was signed for, the status wanted)
        let cases: Vec<(&str, &str, Vec<(&str, &str)>, String, u16)> = vec![
            ("a LAN-named request through a Tor-style forwarder (127.0.0.1, no header)", "192.168.1.5:3210", vec![], lan.into(), 401),
            ("a LAN-named request from the network with a written X-Real-IP", "192.168.1.5:3210", vec![(peer, "192.168.1.20"), ("x-real-ip", "192.168.1.20")], lan.into(), 401),
            ("a LAN-named request from the network with a written X-Forwarded-For", "192.168.1.5:3210", vec![(peer, "192.168.1.20"), ("x-forwarded-for", "192.168.1.20")], lan.into(), 401),
            // nginx on this computer passing a visitor on: the loopback name is this computer's alone.
            ("a loopback-named request passed on by nginx on this computer", &here, vec![("x-real-ip", "203.0.113.9")], r.base(), 401),
            // A proxy on the node's own network that forwards a visitor is not the visitor.
            ("a LAN-named request forwarded by a proxy on the network", "192.168.1.5:3210", vec![(peer, "192.168.1.2"), ("x-forwarded-for", "203.0.113.9")], lan.into(), 401),
            ("a LAN-named request straight from the network is taken", "192.168.1.5:3210", vec![(peer, "192.168.1.20")], lan.into(), 200),
            ("a loopback-named request from this computer is taken", &here, vec![], r.base(), 200),
        ];
        let mut wrong = Vec::new();
        for (what, host, extra, opened, want) in cases {
            let (code, words) = post_with(&r, host, &extra, "/api/messages", &read(&opened)).await;
            if code != want {
                wrong.push(format!("{what}: {code}, wanted {want} ({words})"));
            }
        }
        let signs: Vec<(&str, &str, Vec<(&str, &str)>, &str, &str)> = vec![
            ("a LAN-named sign-in through a Tor-style forwarder", "192.168.1.5:3210", vec![], lan, "system"),
            ("a LAN-named sign-in from the network with a written X-Real-IP", "192.168.1.5:3210", vec![(peer, "192.168.1.20"), ("x-real-ip", "192.168.1.20")], lan, "system"),
            ("the onion name works through Tor", "abcdefghijklmnop.onion", vec![], "http://abcdefghijklmnop.onion", "peer_list"),
        ];
        for (what, host, extra, opened, want) in signs {
            let (mut sock, nonce) = challenge_with(&r, host, &extra, &key, "Hal").await;
            let told = answer_to(&mut sock, &test_identify_response(&kp, opened, &nonce, &key)).await;
            if told["type"] != want {
                wrong.push(format!("{what}: {told}"));
            }
        }
        assert!(wrong.is_empty(), "each case holds its own rule: {wrong:#?}");
    });
}

/// H1 at the word level: a loopback client is this computer, never the local network (whatever
/// forwards visitors on this computer hands them over from 127.0.0.1).
/// Red evidence (2026-11-21, before the fix): "a LAN name from this computer" failed, admitted
/// (`peer_may_use` let a loopback client use every LAN name).
#[test]
fn a_loopback_client_is_never_on_the_local_network() {
    let own = OwnNames::from_settings(|k| (k == PUBLIC_HOSTS_ENV).then(|| "192.168.1.5:3210,mypc.local:3210".to_string()), "0.0.0.0:3210".parse().unwrap());
    let at = |host: &str, peer: &str| crate::relay::audience::Arrival { host: Some(host.to_string()), peer: Some(peer.parse().unwrap()), ..Default::default() };
    assert!(own.admit("192.168.1.5:3210", &at("192.168.1.5:3210", "127.0.0.1")).is_err(), "a LAN name from this computer");
    assert!(own.admit("mypc.local:3210", &at("mypc.local:3210", "::1")).is_err(), "a .local name from this computer");
    assert!(own.admit("192.168.1.5:3210", &at("192.168.1.5:3210", "192.168.1.7")).is_ok(), "a LAN name from its network");
}

/// L4 (the review of rounds 3 and 4, 2026-11-21): a shared carrier address (100.64.0.0/10: a
/// tailnet, a carrier's NAT) is its own network, shared with nobody; and a `.local` or one-word
/// name is taken only from a client on the /24 of one of the node's own LAN addresses, not from
/// any private address.
/// Red evidence (2026-11-21, round 3's rules put back in `same_network` and `peer_may_use`: the
/// range one network, a `.local` name from any private address): "another carrier customer"
/// failed, admitted.
#[test]
fn a_local_name_is_taken_only_from_the_nodes_own_network() {
    let own = OwnNames::from_settings(
        |k| (k == PUBLIC_HOSTS_ENV).then(|| "100.101.1.2:3210,192.168.1.5:3210,mypc.local:3210,mypc:3210".to_string()),
        "0.0.0.0:3210".parse().unwrap(),
    );
    let at = |host: &str, peer: &str| crate::relay::audience::Arrival { host: Some(host.to_string()), peer: Some(peer.parse().unwrap()), ..Default::default() };
    assert!(own.admit("100.101.1.2:3210", &at("100.101.1.2:3210", "100.64.9.9")).is_err(), "another carrier customer");
    assert!(own.admit("mypc.local:3210", &at("mypc.local:3210", "10.0.0.4")).is_err(), "a .local name from another private network");
    assert!(own.admit("mypc:3210", &at("mypc:3210", "172.16.0.9")).is_err(), "a one-word name from another private network");
    assert!(own.admit("mypc.local:3210", &at("mypc.local:3210", "192.168.1.7")).is_ok(), "a .local name from the node's own network");
    assert!(own.admit("mypc:3210", &at("mypc:3210", "192.168.1.7")).is_ok());
    assert!(own.admit("192.168.1.5:3210", &at("192.168.1.5:3210", "192.168.1.7")).is_ok(), "a LAN name from its network");
    assert!(own.admit("100.101.1.2:3210", &at("100.101.1.2:3210", "100.101.1.2")).is_ok(), "a carrier address from itself");
    let no_lan = OwnNames::from_settings(|k| (k == PUBLIC_HOSTS_ENV).then(|| "mypc.local:3210".to_string()), "0.0.0.0:3210".parse().unwrap());
    assert!(no_lan.admit("mypc.local:3210", &at("mypc.local:3210", "192.168.1.7")).is_err(), "a node with no LAN address takes no .local name");
}

/// L1 (the review of rounds 3 and 4, 2026-11-21): the refusal words are no membership oracle. An
/// audience that is not the connection's own address, one whose port differs, one that is not this
/// relay's name, and one whose reach the client lacks are all refused in the SAME words, so a
/// prober cannot learn which names a relay answers to (an onion address tied to its clearnet name,
/// a home node's LAN address); the detail goes to the server's log.
/// Red evidence (2026-11-21, before the fix): "the same words for every case" failed, the four
/// cases in different sentences.
#[test]
fn every_audience_port_or_reach_refusal_has_the_same_words() {
    let own = OwnNames::from_settings(
        |k| (k == PUBLIC_HOSTS_ENV).then(|| "example.org:8443,192.168.1.5:3210,abcdefghijklmnop.onion".to_string()),
        "0.0.0.0:3210".parse().unwrap(),
    );
    let at = |host: &str| crate::relay::audience::Arrival { host: Some(host.to_string()), peer: Some("203.0.113.9".parse().unwrap()), ..Default::default() };
    let words: Vec<String> = [
        own.admit("abcdefghijklmnop.onion", &at("example.org:8443")),
        own.admit("example.org", &at("example.org")),
        own.admit("evil.example", &at("evil.example")),
        own.admit("192.168.1.5:3210", &at("192.168.1.5:3210")),
    ]
    .into_iter()
    .map(|r| r.expect_err("each is refused"))
    .collect();
    assert!(words.iter().all(|w| *w == words[0]), "the same words for every case: {words:#?}");
    for name in ["onion", "example", "192.168", "8443"] {
        assert!(!words[0].contains(name), "the words name nothing: {}", words[0]);
    }
}

/// L2 (the review of rounds 3 and 4, 2026-11-21): a request signed before a restart is not taken
/// after it once the server's clock has read right (round 7 corrected "whatever the wall clock
/// does": a clock slow at the start and never set right, or set back within the hour of being set
/// right, is a residual, signed_request.rs `StartedAt`). The start is the LATEST the wall clock has put
/// it at (each reading's time less the time since the start by a clock that never steps), kept in
/// memory for the run, so a clock stepped back after the start (or slow at the start and set right
/// since, then stepped back again) never moves the start earlier. Proved three ways: correct at the
/// start and stepped back an hour since; slow by an hour at the start and set right since; fast by
/// an hour at the start and set right since.
/// Red evidence (2026-11-21, round 3's rule, the earlier of the recorded start and now less the
/// time since): "correct at the start, stepped back an hour since" failed, left Ok(()) (a request
/// from before the restart taken).
#[test]
fn a_request_from_before_a_restart_is_refused_whatever_the_server_clock_did() {
    on_a_big_stack(|| async {
        use crate::relay::signed_request::{check, now_ms, SignedRefusal, StartedAt};
        let (kp, key) = keypair(41);
        let aud = crate::relay::audience::TEST_AUDIENCE;
        let mut state = RelayState::new(Storage::open_temp("aud_clock_steps"));
        let now = now_ms();
        // The request was signed at `ts` by a correct clock; the run began 50 seconds later.
        let cases: [(&str, u64, u64, u64); 3] = [
            // (what happened, the start the wall clock recorded, ms since the start, the request's time)
            ("correct at the start, stepped back an hour since", now - 50_000, 3_650_000, now - 60_000),
            ("slow at the start by an hour, set right since", now - 3_650_000, 50_000, now - 60_001),
            ("fast at the start by an hour, set right since", now + 3_550_000, 50_000, now - 60_002),
        ];
        for (what, recorded, since, ts) in cases {
            state.started_at = StartedAt::at(recorded, Duration::from_millis(since));
            let refused = check(&state, &key, ts, &test_request_sig(&kp, aud, "account_export", ts), "account_export", "t").await;
            assert!(matches!(refused, Err(SignedRefusal::BeforeStart(_) | SignedRefusal::StartAhead(_))), "{what}: {refused:?}");
        }
        // The start never moves earlier once the clock held it: slow at the start, set right and
        // held for an hour (L5: the hour is what makes a step count), then stepped back again. The
        // time since the start is given by hand (`reading`), as the clock that never steps would.
        use crate::relay::signed_request::CLOCK_STEP_CONFIRM_MS;
        let start = StartedAt::at(now - 3_650_000, Duration::from_millis(50_000));
        let set_right = start.reading(now, 50_000);
        assert_eq!(set_right, now - 50_000, "set right: the start is the later reading at once");
        assert_eq!(start.reading(now + CLOCK_STEP_CONFIRM_MS, 50_000 + CLOCK_STEP_CONFIRM_MS), set_right, "held for the hour");
        assert_eq!(start.reading(now + CLOCK_STEP_CONFIRM_MS - 3_600_000, 50_001 + CLOCK_STEP_CONFIRM_MS), set_right, "stepped back again: the start stays where it was");
    });
}

/// L5 (the review of round 5, 2026-11-21): one wall clock reading far ahead (a year: a bad time
/// server answer, a date typed wrong) no longer sets the start a year ahead for the rest of the run.
/// A forward step counts only once the clock has held it for an hour by the clock that never steps;
/// until then nothing from before it is taken, and a reading back at the start ends it. A step the
/// clock did hold, then undone, refuses every request until a restart, in words that say a restart
/// clears it. Only the relay's own clock moves the start: a request's time never does.
/// Red evidence (2026-11-21, round 5's rule put back, every forward reading kept): "after one
/// reading a year ahead, the clock put right, a correct request is taken" failed, left
/// Err(StartAhead(525540)), right Ok(()). Round 7 (2026-11-21), the stranger's request during the
/// step, which now reaches the start: with the rule round 7 turned down (a step kept for good once a
/// request is taken under it), "a request taken during a step does not keep the step" failed, left
/// Err(StartAhead(525540)).
#[test]
fn a_reading_a_year_ahead_locks_nothing_out_unless_the_clock_held_it() {
    on_a_big_stack(|| async {
        use crate::relay::signed_request::{check, now_ms, SignedRefusal, StartedAt, CLOCK_STEP_CONFIRM_MS};
        const YEAR: u64 = 365 * 24 * 3_600_000;
        let (kp, key) = keypair(42);
        let aud = crate::relay::audience::TEST_AUDIENCE;
        let mut state = RelayState::new(Storage::open_temp("aud_clock_year"));
        let now = now_ms();
        // A correct clock, the relay up an hour; one reading a year ahead, then the clock is right.
        state.started_at = StartedAt::at(now - 3_600_000, Duration::from_secs(3_600));
        let ahead = state.started_at.wall_ms(now + YEAR);
        assert!(ahead > now, "while the clock reads a year ahead, the step is the start (nothing older is taken)");
        let ts = crate::relay::signed_request::next_test_timestamp();
        let taken = check(&state, &key, ts, &test_request_sig(&kp, aud, "account_export", ts), "account_export", "t").await;
        assert_eq!(taken, Ok(()), "after one reading a year ahead, the clock put right, a correct request is taken");
        // DURING such a step, a stranger with a key of his own stamps a request with the server's
        // Date (a year ahead): fresh to the server, signed, for this relay, so it REACHES the start
        // and is taken (round 7: the version before it was refused as from the future and never got
        // there). Once the clock is put right, every correct request is taken again: the reading
        // the stranger's request made did not keep the year.
        let (skp, stranger) = keypair(50);
        let during = now_ms() + YEAR;
        let at = crate::relay::audience::Arrival { host: Some(aud.to_string()), ..Default::default() };
        let sig = test_request_sig(&skp, aud, "account_export", during);
        let reached = crate::relay::signed_request::check_at(&state, &at, &stranger, during, &sig, "account_export", "t", during).await;
        assert_eq!(reached, Ok(()), "the stranger's request during the step reaches the start and is taken");
        let ts = crate::relay::signed_request::next_test_timestamp();
        let taken = check(&state, &key, ts, &test_request_sig(&kp, aud, "account_export", ts), "account_export", "t").await;
        assert_eq!(taken, Ok(()), "a request taken during a step does not keep the step: correct requests are taken after it");
        // The clock HELD the year for the hour, and was then put right: every request is refused
        // until a restart, and the words say a restart clears it.
        let held = StartedAt::at(now - 3_600_000, Duration::from_secs(3_600));
        held.reading(now + YEAR, 3_600_000);
        held.reading(now + YEAR + CLOCK_STEP_CONFIRM_MS, 3_600_000 + CLOCK_STEP_CONFIRM_MS);
        state.started_at = held;
        let ts = crate::relay::signed_request::next_test_timestamp();
        let refused = check(&state, &key, ts, &test_request_sig(&kp, aud, "account_export", ts), "account_export", "t").await;
        let Err(why @ SignedRefusal::StartAhead(_)) = refused else { panic!("a step held for the hour stays: {refused:?}") };
        assert!(why.words().contains("restarting it clears this"), "the words say a restart clears it: {}", why.words());
    });
}

/// L4 (the review of round 5, 2026-11-21): a made-up signature for one of the relay's names and one
/// for a name it does not answer to are refused in the same words, the same status and the same
/// code, by a request and by a sign-in: the signature is verified whatever the audience, and a
/// failed one is the one sentence. (A prober signing with a key of its own is still taken for a
/// right guess, as anyone is for a public name: docs/reference/cryptography.md.)
/// Red evidence (2026-11-21, round 5's order and words put back: the audience asked first, a failed
/// signature "The signature does not match this request." and a failed sign-in "Identify challenge
/// verification failed."): "a made-up signature for a real name and for a guess get the same
/// answer" failed, left (401, "The signature does not match this request."), right (401, the one
/// sentence); the test stops there, so the sign-in half was not reached in that run. Round 7
/// (2026-11-21): with the old order put back and the new words kept (the signature verified only
/// for an admitted audience), every answer read the same and "a request for a name not answered to
/// still has its signature verified" failed (the test stops there); with round 5's sign-in words
/// alone put back, the sign-in half failed, left "Identify challenge verification failed.
/// Reconnect and re-identify.", right the one sentence.
#[test]
fn a_bad_signature_and_a_name_not_answered_to_get_the_same_answer() {
    on_a_big_stack(|| async {
        let r = relay_named("aud_l4", names_from(&[(PUBLIC_HOSTS_ENV, "abcdefghijklmnop.onion")])).await;
        let (kp, key) = keypair(43);
        let (other, _) = keypair(44);
        member(&r, &key, "Pro");
        // A made-up signature: the prober's key, someone else's signature.
        let made_up = |opened: &str| {
            let ts = crate::relay::signed_request::next_test_timestamp();
            json!({ "key": key, "timestamp": ts, "sig": test_request_sig(&other, opened, "account_export", ts) })
        };
        let real = post_as(&r, "abcdefghijklmnop.onion", Some("203.0.113.9"), "/api/account/export", &made_up("http://abcdefghijklmnop.onion")).await;
        let guess = post_as(&r, "zzzzzzzzzzzzzzzz.onion", Some("203.0.113.9"), "/api/account/export", &made_up("http://zzzzzzzzzzzzzzzz.onion")).await;
        assert_eq!(real, guess, "a made-up signature for a real name and for a guess get the same answer");
        assert_eq!(real.1, crate::relay::audience::REFUSED_AS_REACHED);
        use crate::relay::signed_request::SignedRefusal;
        assert_eq!(SignedRefusal::Signature.code(), SignedRefusal::Audience(String::new()).code(), "one code for both");
        assert_eq!(SignedRefusal::Signature.status(), SignedRefusal::Audience(String::new()).status());
        // The same by a sign-in: a made-up proof for a real name, and a proof for a guess.
        let (mut sock, nonce) = challenge_as(&r, "abcdefghijklmnop.onion", Some("203.0.113.9"), &key, "Pro").await;
        let mut proof = test_identify_response(&kp, "http://abcdefghijklmnop.onion", &nonce, &key);
        proof["sig_b64"] = test_identify_response(&kp, "http://abcdefghijklmnop.onion", &"0".repeat(64), &key)["sig_b64"].clone();
        let real = answer_to(&mut sock, &proof).await;
        let (mut sock, nonce) = challenge_as(&r, "zzzzzzzzzzzzzzzz.onion", Some("203.0.113.9"), &key, "Pro").await;
        let guess = answer_to(&mut sock, &test_identify_response(&kp, "http://zzzzzzzzzzzzzzzz.onion", &nonce, &key)).await;
        assert_eq!(real, guess, "a made-up proof for a real name and a proof for a guess get the same answer");
        assert_eq!(real["message"], crate::relay::audience::REFUSED_AS_REACHED);
        // The signature is verified BEFORE the audience decides, whatever the audience (so the
        // time an answer takes tells nothing either): a guess by a key of its own, refused for its
        // name, still had its signature verified, by a request and by a sign-in (round 7: with the
        // old order and the new words every answer above reads the same).
        // Seeds no other test uses (the review of round 7): was_verified reads one record for the whole
        // test program, so a key another test signs in with would pass these checks whatever the order.
        let (gkp, guesser) = keypair(231);
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": guesser, "timestamp": ts, "sig": test_request_sig(&gkp, "http://zzzzzzzzzzzzzzzz.onion", "account_export", ts) });
        assert_eq!(post_as(&r, "zzzzzzzzzzzzzzzz.onion", Some("203.0.113.9"), "/api/account/export", &body).await.0, 401);
        assert!(crate::relay::signed_request::was_verified(&guesser), "a request for a name not answered to still has its signature verified");
        let (ikp, identifier) = keypair(232);
        let (mut sock, nonce) = challenge_as(&r, "zzzzzzzzzzzzzzzz.onion", Some("203.0.113.9"), &identifier, "Gue").await;
        assert_eq!(answer_to(&mut sock, &test_identify_response(&ikp, "http://zzzzzzzzzzzzzzzz.onion", &nonce, &identifier)).await["type"], "system");
        assert!(crate::relay::signed_request::was_verified(&identifier), "a sign-in for a name not answered to still has its signature verified");
    });
}

/// L3 (the review of round 5, 2026-11-21): a Tailscale network is the owner's to declare. With
/// `ON_TAILNET=1` a friend on the tailnet (any address in 100.64.0.0/10) may use the node's
/// Tailscale address and its MagicDNS one-word name; without it neither, since a carrier gives the
/// same range to its customers. Through a real relay, a sign-in from the friend's tailnet address.
/// Red evidence (2026-11-21, `same_network` without the tailnet rule, round 5's): "a tailnet
/// friend signs in by the node's Tailscale address, with the opt-in" failed, the socket refused.
#[test]
fn a_tailnet_friend_is_taken_only_when_the_owner_says_it_is_a_tailnet() {
    on_a_big_stack(|| async {
        const TS: &str = "100.101.1.2:3210,mypc:3210";
        let on = relay_named("aud_ts_on", names_from(&[(PUBLIC_HOSTS_ENV, TS), (crate::relay::audience::TAILNET_ENV, "1")])).await;
        let off = relay_named("aud_ts_off", names_from(&[(PUBLIC_HOSTS_ENV, TS)])).await;
        let (kp, key) = keypair(45);
        for (r, want, what) in [(&on, "peer_list", "with the opt-in"), (&off, "system", "without it")] {
            for name in ["100.101.1.2:3210", "mypc:3210"] {
                let (mut sock, nonce) = challenge_as(r, name, Some("100.88.7.6"), &key, "Tai").await;
                let told = answer_to(&mut sock, &test_identify_response(&kp, &format!("http://{name}"), &nonce, &key)).await;
                assert_eq!(told["type"], want, "a tailnet friend signs in by {name}, {what}: {told}");
            }
        }
        // With the opt-in, still never from this computer or from outside the range.
        let own = |pairs: &'static [(&'static str, &'static str)]| names_from(pairs)("0.0.0.0:3210".parse().unwrap());
        let ts = own(&[(PUBLIC_HOSTS_ENV, TS), (crate::relay::audience::TAILNET_ENV, "1")]);
        let at = |host: &str, peer: &str| crate::relay::audience::Arrival { host: Some(host.to_string()), peer: Some(peer.parse().unwrap()), ..Default::default() };
        assert!(ts.admit("100.101.1.2:3210", &at("100.101.1.2:3210", "127.0.0.1")).is_err(), "from this computer");
        assert!(ts.admit("100.101.1.2:3210", &at("100.101.1.2:3210", "203.0.113.9")).is_err(), "from the internet");
        assert!(ts.admit("100.101.1.2:3210", &at("100.101.1.2:3210", "192.168.1.7")).is_err(), "from a home network");
    });
}

/// M2 (the review of round 5, 2026-11-21), the relay's half: the owner's door (OWNER_PORT, a
/// second listener on 127.0.0.1) takes its own names, for the owner's key alone; its names are
/// refused through the main port (where a forwarder's visitors arrive), the main port's names are
/// refused through it, and a connection through it carrying a forwarding header is refused.
/// Red evidence (2026-11-21): with the door rule taken out of `OwnNames::admit` (the door's names
/// put among the main names, the round-5 loopback names), "the door's name through the main port
/// is refused" failed, the socket signed in; with the key rule taken out of `admit_signer`,
/// "another key through the door is refused" failed, the socket signed in.
/// Round 7 (2026-11-21): the relay opens the door itself, the way `run_relay` does (`serve_on`);
/// red evidence: with `open_owner_door` taken out of `serve_on` (the door's listener held but never
/// served), the first sign-in through the door got no answer and the test stalled ("has been running
/// for over 60 seconds") until it was stopped; with the listener dropped as well, as `run_relay`
/// would have none, the connection would be refused outright (not run: round 7's step budget). And through the door the owner is 127.0.0.1, as is
/// every visitor a headerless forwarder on this computer hands over, so the door keeps its own
/// counts: a visitor that spends this computer's sign-in allowance on the main port (30 a minute)
/// does not lock the owner out. Red evidence: with the counts keyed by address alone (`door_key`
/// giving the address), the owner's last sign-in at the door was refused before its challenge:
/// the relay answered too many attempts and closed, and the test failed "the socket closed before a
/// challenge".
#[test]
fn the_owners_door_takes_its_own_names_for_the_owners_key_alone() {
    on_a_big_stack(|| async {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let (owner_kp, owner) = keypair(46);
        let (other_kp, other) = keypair(47);
        let main = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        // The door's port, held from the start and handed to serve_on (run_relay's path), which
        // serves the door on it only because the names' OWNER_PORT says so.
        let door = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (main_addr, door_port) = (main.local_addr().unwrap(), door.local_addr().unwrap().port());
        let settings = [
            (PUBLIC_HOSTS_ENV, "abcdefghijklmnop.onion".to_string()),
            (crate::relay::audience::OWNER_PORT_ENV, door_port.to_string()),
            (crate::relay::audience::OWNER_KEY_ENV, owner.clone()),
        ];
        let names = OwnNames::from_settings(|k| settings.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone()), main_addr);
        assert_eq!(names.door_port(), Some(door_port));
        let state = crate::relay::serve_relay_named(Storage::open_temp("aud_door"), main, Some(door), names).await;
        crate::relay::default_channels::make_missing(&state.db, true);
        member(&Relay { state: state.clone(), port: main_addr.port() }, &owner, "Own");
        let door_name = format!("127.0.0.1:{door_port}");
        // Open /ws at `port` with the Host `host` and the extra headers, and answer the challenge
        // with a proof from `kp` for the address `opened`.
        let sign_in = |port: u16, host: String, extra: Vec<(&'static str, &'static str)>, kp: DilithiumKeypair, key: String, opened: String| async move {
            let mut req = format!("ws://127.0.0.1:{port}/ws").into_client_request().unwrap();
            req.headers_mut().insert("host", host.parse().unwrap());
            for (k, v) in extra {
                req.headers_mut().insert(k, v.parse().unwrap());
            }
            let (mut sock, _) = tokio_tungstenite::connect_async(req).await.unwrap();
            // Each key its own name (the owner's is registered), so a key let in signs in plainly.
            let name = format!("K{}", humanity_core::text::prefix(&key, 6));
            sock.send(WsMsg::Text(json!({ "type": "identify", "public_key": key, "display_name": name }).to_string().into())).await.unwrap();
            let c = frame_where(&mut sock, "a challenge", |f| f["type"] == "identify_challenge").await;
            let nonce = c["nonce"].as_str().unwrap().to_string();
            answer_to(&mut sock, &test_identify_response(&kp, &opened, &nonce, &key)).await["type"].as_str().unwrap_or_default().to_string()
        };
        let opened = format!("http://{door_name}");
        let again = || DilithiumKeypair::from_seed(&derive_dilithium_seed(&[46; 32]));
        assert_eq!(sign_in(door_port, door_name.clone(), vec![], again(), owner.clone(), opened.clone()).await, "peer_list", "the owner signs in through the door");
        assert_eq!(sign_in(door_port, door_name.clone(), vec![], other_kp, other.clone(), opened.clone()).await, "system", "another key through the door is refused");
        assert_eq!(sign_in(main_addr.port(), door_name.clone(), vec![], again(), owner.clone(), opened.clone()).await, "system", "the door's name through the main port is refused");
        assert_eq!(
            sign_in(door_port, "abcdefghijklmnop.onion".into(), vec![], again(), owner.clone(), "http://abcdefghijklmnop.onion".into()).await,
            "system",
            "the main port's name through the door is refused"
        );
        assert_eq!(sign_in(door_port, door_name.clone(), vec![("x-real-ip", "203.0.113.9")], again(), owner.clone(), opened.clone()).await, "system", "the door through a proxy is refused");
        // A signed request through the door, by the owner.
        let ts = crate::relay::signed_request::next_test_timestamp();
        let body = json!({ "key": owner, "timestamp": ts, "sig": test_request_sig(&owner_kp, &opened, "account_export", ts) });
        let res = reqwest::Client::new().post(format!("http://{door_name}/api/account/export")).json(&body).send().await.unwrap();
        assert_eq!(res.status().as_u16(), 200, "the owner's signed request through the door: {}", res.text().await.unwrap_or_default());
        // A visitor through a forwarder on this computer (127.0.0.1 on the main port) has spent this
        // computer's sign-in allowance (30 a minute; set here, as thirty sign-ins would have), and
        // the main port says so.
        state.identify_rate.lock().unwrap().insert("127.0.0.1".to_string(), vec![std::time::Instant::now(); 30]);
        let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", main_addr.port())).await.unwrap();
        sock.send(WsMsg::Text(json!({ "type": "identify", "public_key": other, "display_name": "Vis" }).to_string().into())).await.unwrap();
        let told = frame_where(&mut sock, "too many", |f| f["type"] == "system").await;
        assert!(told["message"].as_str().unwrap_or_default().contains("Too many"), "the main port's allowance is spent: {told}");
        assert_eq!(
            sign_in(door_port, door_name.clone(), vec![], again(), owner.clone(), opened.clone()).await,
            "peer_list",
            "the owner still signs in through the door after the main port's allowance is spent"
        );
    });
}

/// L6 (the review of round 5, 2026-11-21): an `http://` origin in ALLOWED_ORIGINS adds no name
/// (docs/reference/cryptography.md promises the origins' names are ones a certificate binds),
/// except behind the dev switch the rigs set.
/// Red evidence (2026-11-21, round 5's filter put back, http and https alike): "an http origin
/// adds no name" failed, left ["a.example", "b.example"].
#[test]
fn only_https_origins_name_the_relay_unless_the_dev_switch_is_on() {
    let bound: std::net::SocketAddr = "0.0.0.0:3210".parse().unwrap();
    let origins = "https://a.example,http://b.example";
    let own = names_from(&[("ALLOWED_ORIGINS", "https://a.example,http://b.example")])(bound);
    assert_eq!(own.names(), ["a.example"], "an http origin adds no name");
    let dev = names_from(&[("ALLOWED_ORIGINS", "https://a.example,http://b.example"), (DEV_LOOPBACK_NAMES_ENV, "1")])(bound);
    assert!(dev.accepts("b.example"), "behind the dev switch it does ({origins})");
}

/// Run `test` on a thread with a 64 MB stack and a runtime of its own: Dilithium steps and `json!`
/// in a debug build overflow a test thread's 2 MB (as signed_request_tests.rs does).
fn on_a_big_stack<F, Fut>(test: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()>,
{
    let run = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
            rt.block_on(test());
        })
        .unwrap()
        .join();
    if let Err(e) = run {
        std::panic::resume_unwind(e);
    }
}

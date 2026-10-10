// Child of call_credentials.rs (#[path], so `use super::*` reaches everything it has): who is
// given call forwarder credentials, over the handler as the socket calls it, against a real
// relay state (docs/design/blocking-and-safe-mode.md 10f, its Proof list). The socket half (the
// reply reaching the asker's sockets alone, and voice switched off) is in features.rs.

use super::*;
use crate::relay::call_forwarder::{Forwarder, REALM};
use crate::relay::relay::VoiceRoom;
use crate::relay::storage::Storage;
use crate::relay::stun_wire::{self as wire, Class, Message, MessageBuilder};

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

fn fresh_state(tag: &str) -> Arc<RelayState> {
    let state = Arc::new(RelayState::new(Storage::open_temp(&format!("callcreds_{tag}"))));
    state.calls.set_listening("relay.example", 3478);
    state
}

/// Put `keys` in voice room `room`.
fn seat(state: &RelayState, room: &str, keys: &[&str]) {
    block(async {
        let mut rooms = state.voice_rooms.write().await;
        let r = rooms.entry(room.to_string()).or_insert_with(|| VoiceRoom { name: room.to_string(), participants: Vec::new() });
        for k in keys {
            r.participants.push((k.to_string(), k.to_string()));
        }
    });
}

/// Ask for credentials as `who`; what `who` was sent: the credentials, or the notice that came
/// with a refusal. A refusal must be the refused reply (echoing `room` and `call` as asked, with
/// no urls, username or credential, so a client stops waiting at once) plus the notice.
fn ask(state: &Arc<RelayState>, who: &str, request: serde_json::Value) -> Result<CallCredentials, String> {
    let mut rx = state.broadcast_tx.subscribe();
    block(handle(state, who, &request));
    let mut got = Vec::new();
    while let Ok(m) = rx.try_recv() {
        got.push(m);
    }
    let mut creds = None;
    let mut notice = None;
    for m in got.iter().cloned() {
        match m {
            RelayMessage::CallCredentials { to, creds: c } if to == who && creds.is_none() => creds = Some(c),
            RelayMessage::Private { to, message } if to == who && notice.is_none() => notice = Some(message),
            other => panic!("unexpected answer {other:?} among {got:?}"),
        }
    }
    let creds = creds.expect("every request gets a call_credentials reply, granted or refused");
    if !creds.refused {
        assert!(notice.is_none(), "a grant comes alone: {got:?}");
        return Ok(creds);
    }
    let as_asked = |k: &str| request.get(k).and_then(|v| v.as_str()).map(str::to_string);
    assert_eq!((creds.room.clone(), creds.call.clone()), (as_asked("room"), as_asked("call")), "the refusal echoes what was asked");
    assert!(creds.urls.is_empty() && creds.username.is_empty() && creds.credential.is_empty() && creds.ttl == 0, "{creds:?}");
    Err(notice.expect("a refusal comes with a notice saying why"))
}

fn room(id: &str) -> serde_json::Value {
    serde_json::json!({ "type": "call_credentials", "room": id })
}

fn call(key: &str) -> serde_json::Value {
    serde_json::json!({ "type": "call_credentials", "call": key })
}

/// Someone in the voice room is given credentials for it, in the shape the clients build against:
/// both URLs, a username `"{expiry}:{room tag}"` for this room, the HMAC credential, ttl 3600.
/// Someone not in it gets the notice and nothing else.
///
/// Seen red 2026-10-09 with the roster check removed from `handle`: "someone not in the room:
/// left: Ok(CallCredentials { room: Some(\"lounge\"), ...". The refusal's wire shape, seen red
/// with `urls` written even when empty: the refusal carried "urls": [].
#[test]
fn credentials_are_given_only_to_someone_in_the_voice_room() {
    let state = fresh_state("room");
    seat(&state, "lounge", &["alice_key", "bob_key"]);

    let creds = ask(&state, "alice_key", room("lounge")).expect("alice is in the lounge");
    assert_eq!(creds.room.as_deref(), Some("lounge"));
    assert_eq!(creds.call, None);
    assert_eq!(creds.urls, vec!["turn:relay.example:3478?transport=udp", "stun:relay.example:3478"]);
    assert_eq!(creds.ttl, 3600);
    let (expiry, scope) = parse_username(&creds.username).expect("the username has this relay's shape");
    assert!(expiry > unix_now() && expiry <= unix_now() + CREDENTIAL_TTL_SECS);
    assert_eq!(scope, state.calls.keys().room_scope("lounge"), "the username names this room");
    assert_eq!(creds.credential, state.calls.keys().credential_for(&creds.username));
    // The wire shape: the room or call key, then the fields, flat beside "type".
    let wire = serde_json::to_value(RelayMessage::CallCredentials { to: "alice_key".into(), creds: creds.clone() }).unwrap();
    assert_eq!(wire["type"], "call_credentials");
    assert_eq!(wire["room"], "lounge");
    assert!(wire.get("to").is_none() && wire.get("call").is_none() && wire.get("refused").is_none(), "{wire}");
    // A refusal is that reply with "refused" and nothing to connect with, so a client stops
    // waiting at once; the web client reads a reply without urls, username and credential as
    // "no credentials".
    let refusal = CallCredentials { call: Some("bob_key".into()), refused: true, ..Default::default() };
    let wire = serde_json::to_value(RelayMessage::CallCredentials { to: "alice_key".into(), creds: refusal }).unwrap();
    assert_eq!(wire, serde_json::json!({ "type": "call_credentials", "call": "bob_key", "refused": true }));

    let bob = ask(&state, "bob_key", room("lounge")).unwrap();
    assert_ne!(bob.username, creds.username, "every request gets its own username");
    assert_eq!(parse_username(&bob.username).unwrap().1, scope, "the same room, the same tag");

    assert_eq!(ask(&state, "mallory_key", room("lounge")), Err(NOT_IN_IT.to_string()), "someone not in the room");
    assert_eq!(ask(&state, "alice_key", room("another room")), Err(NOT_IN_IT.to_string()), "a room alice is not in");
}

/// Both people in an open call (a ring let through, handlers/reach.rs) are given credentials for
/// the same call; a third person naming either of them is not, nor is anyone once it ends.
///
/// Seen red 2026-10-09 with `call_is_open` answering true for any two keys: "a third person:
/// left: Ok(CallCredentials { ...".
#[test]
fn credentials_are_given_only_to_the_two_people_in_an_open_call() {
    let state = fresh_state("call");
    state.db.set_reach_settings("callee_key", &[("call", "anyone")]).unwrap();
    assert!(crate::relay::handlers::reach::call_may_pass(&state, "caller_key", "callee_key", "ring", None), "the ring is let through");

    let a = ask(&state, "caller_key", call("callee_key")).expect("the caller is in the call");
    let b = ask(&state, "callee_key", call("caller_key")).expect("the callee is in the call");
    assert_eq!(a.call.as_deref(), Some("callee_key"));
    assert_eq!(parse_username(&a.username).unwrap().1, parse_username(&b.username).unwrap().1, "one call, one tag");
    assert_ne!(parse_username(&a.username).unwrap().1, state.calls.keys().room_scope("callee_key"), "a call is not a room");

    assert_eq!(ask(&state, "third_key", call("callee_key")), Err(NOT_IN_IT.to_string()), "a third person");
    assert_eq!(ask(&state, "caller_key", call("caller_key")), Err(NOT_IN_IT.to_string()), "a call with yourself");

    crate::relay::handlers::reach::call_may_pass(&state, "callee_key", "caller_key", "hangup", None);
    assert_eq!(ask(&state, "caller_key", call("callee_key")), Err(NOT_IN_IT.to_string()), "once the call ended");
}

/// A request naming both a room and a call, or neither, is refused; with the forwarder not
/// listening there is nothing to give; past twenty a minute, no more.
#[test]
fn malformed_requests_no_forwarder_and_too_many_are_refused() {
    let state = fresh_state("refusals");
    seat(&state, "lounge", &["alice_key"]);
    let both = serde_json::json!({ "type": "call_credentials", "room": "lounge", "call": "bob_key" });
    assert_eq!(ask(&state, "alice_key", both), Err(NOT_IN_IT.to_string()));
    assert_eq!(ask(&state, "alice_key", serde_json::json!({ "type": "call_credentials" })), Err(NOT_IN_IT.to_string()));

    for _ in 0..ISSUES_PER_MINUTE {
        ask(&state, "alice_key", room("lounge")).expect("within the minute's share");
    }
    assert_eq!(ask(&state, "alice_key", room("lounge")), Err(TOO_MANY.to_string()));

    let quiet = Arc::new(RelayState::new(Storage::open_temp("callcreds_not_listening")));
    seat(&quiet, "lounge", &["alice_key"]);
    assert_eq!(ask(&quiet, "alice_key", room("lounge")), Err(NOT_SET_UP.to_string()));
}

/// Credentials stop working when the relay restarts, even on the same database: the secret is
/// made at start and kept nowhere. A forwarder of the new run refuses an Allocate made with the
/// old run's credential (401); the old run's own forwarder accepted it.
///
/// Seen red 2026-10-09 with `CallKeys::generate` returning a fixed secret: "a credential from
/// before the restart: left: None, right: Some(401)".
#[test]
fn credentials_stop_working_after_the_relay_restarts() {
    let db = Arc::new(crate::test_temp::db("callcreds_restart"));
    let before = Arc::new(RelayState::new(Storage::open_sharing(&db)));
    before.calls.set_listening("127.0.0.1", 3478);
    seat(&before, "lounge", &["alice_key"]);
    let creds = ask(&before, "alice_key", room("lounge")).unwrap();
    let old_keys = before.calls.keys().clone();
    drop(before);
    let after = RelayState::new(Storage::open_sharing(&db));

    let allocate_with = |keys: &CallKeys| {
        let t = Instant::now();
        let mut fw = Forwarder::new(keys.clone(), Some(std::net::Ipv4Addr::new(192, 0, 2, 1)), 3478, t);
        let src: SocketAddr = "203.0.113.5:6000".parse().unwrap();
        let unix = unix_now();
        let key = wire::long_term_key(&creds.username, REALM, &creds.credential);
        let mut b = MessageBuilder::new(wire::METHOD_ALLOCATE, Class::Request, [9u8; 12]);
        b.u32_attr(wire::ATTR_REQUESTED_TRANSPORT, 17 << 24)
            .attr(wire::ATTR_USERNAME, creds.username.as_bytes())
            .attr(wire::ATTR_REALM, REALM.as_bytes())
            .attr(wire::ATTR_NONCE, keys.nonce_for(src, unix).as_bytes())
            .integrity(&key);
        let outs = fw.handle(src, &b.build(), t, unix);
        let m = Message::parse(&outs[0].bytes).unwrap();
        (m.class == Class::Error).then(|| wire::decode_error_code(m.get(wire::ATTR_ERROR_CODE).unwrap()).unwrap())
    };
    assert_eq!(allocate_with(&old_keys), None, "the run that issued it accepts it");
    assert_eq!(allocate_with(after.calls.keys()), Some(401), "a credential from before the restart");
    assert_ne!(after.calls.keys().credential_for(&creds.username), creds.credential);
}

/// The nonce: good from the address it was given to, within its lifetime; not from another
/// address, not after, not altered, not from another run.
///
/// Seen red 2026-10-09 with `CallKeys::generate` returning a fixed secret: "another run's nonce".
#[test]
fn nonces_are_bound_to_the_address_and_expire() {
    let keys = CallKeys::generate();
    let at: SocketAddr = "203.0.113.5:6000".parse().unwrap();
    let n = keys.nonce_for(at, 1_000_000);
    assert!(keys.nonce_ok(n.as_bytes(), at, 1_000_000 + NONCE_LIFETIME_SECS - 1));
    assert!(!keys.nonce_ok(n.as_bytes(), at, 1_000_000 + NONCE_LIFETIME_SECS));
    assert!(!keys.nonce_ok(n.as_bytes(), "203.0.113.5:6001".parse().unwrap(), 1_000_000));
    let mut altered = n.clone().into_bytes();
    altered[23] = if altered[23] == b'0' { b'1' } else { b'0' };
    assert!(!keys.nonce_ok(&altered, at, 1_000_000));
    assert!(!CallKeys::generate().nonce_ok(n.as_bytes(), at, 1_000_000), "another run's nonce");
}

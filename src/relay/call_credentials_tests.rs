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
    assert_eq!(
        (creds.room.clone(), creds.call.clone(), creds.file.clone()),
        (as_asked("room"), as_asked("call"), as_asked("file")),
        "the refusal echoes what was asked"
    );
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

    // The two refused requests above count toward the minute's share too (2026-10-12: every
    // request counts, before anything else is checked).
    for _ in 0..ISSUES_PER_MINUTE - 2 {
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

// ── File transfers (device-to-device big files, 2026-10-11) ──────────────────

/// A real identity: its BIP39 seed and its Dilithium3 public key, hex.
fn person(seed_byte: u8) -> (Vec<u8>, String) {
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    let seed = vec![seed_byte; 32];
    let key = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key());
    (seed, key)
}

/// The file pass `issuer` (seed, key) gives `grantee` on this relay for transfer `id`, ending at
/// `expiry`, signed by hand (so a run-out or overlong one can be made too).
fn file_pass(state: &RelayState, issuer: &(Vec<u8>, String), grantee: &str, id: &str, expiry: u64) -> serde_json::Value {
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, file_pass_preimage, DilithiumKeypair};
    use base64::Engine;
    let server = state.db.server_did().unwrap();
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&issuer.0));
    let sig = base64::engine::general_purpose::STANDARD.encode(kp.sign(file_pass_preimage(&server, &issuer.1, grantee, id, expiry).as_bytes()));
    serde_json::json!({ "v": 1, "id": id, "expiry": expiry, "sig": sig })
}

fn file(id: &str, peer: &str, pass: Option<&serde_json::Value>) -> serde_json::Value {
    let mut r = serde_json::json!({ "type": "call_credentials", "file": id, "peer": peer });
    if let Some(p) = pass {
        r["file_pass"] = p.clone();
    }
    r
}

const ID: &str = "00112233445566778899aabbccddeeff";

/// FILE CREDENTIALS ONLY WITH A MATCHING PASS (the shapes, part 2). Ann sends Bo a file and gave
/// him a file pass for it. Bo (its grantee) asking with Ann as his peer, and Ann (its issuer)
/// asking with Bo as hers, are each given credentials for the same transfer: one scope, the
/// keyed hash of the id and the two keys, not a call's or a room's. Each credential carries its
/// asker's person tag and their role's forwarded day (Bo Verified, 2048; Ann Admin, 0, no
/// limit) inside the part the HMAC covers; a room's carries none. The reply echoes `file`.
/// Refused, with the refusal and the notice: another file's id, a third person, the pass with
/// someone else named as peer, yourself as your own peer, no pass, a forged pass, one run out,
/// one longer than a week, and `file` beside a `room`. With the forwarder not listening, a good
/// pass gets the not-set-up words.
///
/// Seen red 2026-10-11 with the id check left out of `file_pass_fits`: "another file's id",
/// left: None (it was granted), right: Some("No credentials for that transfer: ...").
#[test]
fn file_credentials_only_with_a_matching_pass_both_ways_round() {
    let state = fresh_state("file");
    let (ann, bo, cy) = (person(201), person(202), person(203));
    // Both hold a name here: file credentials need the other person to be on this server.
    state.db.register_name("Ann", &ann.1).unwrap();
    state.db.register_name("Bo", &bo.1).unwrap();
    state.db.set_role(&ann.1, "admin").unwrap();
    state.db.set_role(&bo.1, "verified").unwrap();
    let now = unix_now();
    let pass = file_pass(&state, &ann, &bo.1, ID, now + 86_400);

    let to_bo = ask(&state, &bo.1, file(ID, &ann.1, Some(&pass))).expect("Bo, the pass's grantee, is given credentials");
    let to_ann = ask(&state, &ann.1, file(ID, &bo.1, Some(&pass))).expect("Ann, its issuer, is too");
    assert_eq!((to_bo.file.as_deref(), to_bo.room.as_deref(), to_bo.call.as_deref()), (Some(ID), None, None), "the reply echoes the file");
    let scope = |c: &CallCredentials| parse_username(&c.username).expect("this relay's shape").1.to_string();
    assert_eq!(scope(&to_bo), state.calls.keys().file_scope(ID, &ann.1, &bo.1), "the transfer's scope");
    assert_eq!(scope(&to_bo), scope(&to_ann), "one transfer, one scope, whichever of the two asks");
    assert_ne!(scope(&to_bo), state.calls.keys().call_scope(&ann.1, &bo.1), "a transfer is not a call between the two");
    assert_eq!(to_bo.credential, state.calls.keys().credential_for(&to_bo.username));
    let bo_meter = username_meter(&to_bo.username).expect("a file credential carries a tag");
    let ann_meter = username_meter(&to_ann.username).expect("so does Ann's");
    assert_eq!(bo_meter, Meter { tag: state.calls.keys().person_tag(&bo.1), limit_mb: 2048 }, "Bo's tag and Verified's forwarded day");
    assert_eq!(ann_meter, Meter { tag: state.calls.keys().person_tag(&ann.1), limit_mb: 0 }, "Ann's tag and Admin's: no limit");
    assert_ne!(bo_meter.tag, ann_meter.tag, "one tag per person");
    let wire = serde_json::to_value(RelayMessage::CallCredentials { to: bo.1.clone(), creds: to_bo.clone() }).unwrap();
    assert_eq!(wire["file"], ID, "{wire}");

    let refused = |who: &str, req: serde_json::Value| ask(&state, who, req).err();
    let no = Some(NO_FILE_PASS.to_string());
    assert_eq!(refused(&bo.1, file("ffeeddccbbaa99887766554433221100", &ann.1, Some(&pass))), no, "another file's id");
    assert_eq!(refused(&cy.1, file(ID, &ann.1, Some(&pass))), no, "a third person naming the issuer");
    assert_eq!(refused(&cy.1, file(ID, &bo.1, Some(&pass))), no, "a third person naming the grantee");
    assert_eq!(refused(&bo.1, file(ID, &cy.1, Some(&pass))), no, "someone else named as the peer");
    let own = file_pass(&state, &bo, &bo.1, ID, now + 86_400);
    assert_eq!(refused(&bo.1, file(ID, &bo.1, Some(&own))), no, "yourself as your own peer");
    assert_eq!(refused(&bo.1, file(ID, &ann.1, None)), no, "no pass");
    let mut forged = pass.clone();
    forged["sig"] = file_pass(&state, &cy, &bo.1, ID, now + 86_400)["sig"].clone();
    assert_eq!(refused(&bo.1, file(ID, &ann.1, Some(&forged))), no, "a forged pass");
    assert_eq!(refused(&bo.1, file(ID, &ann.1, Some(&file_pass(&state, &ann, &bo.1, ID, now - 1)))), no, "a pass run out");
    assert_eq!(refused(&bo.1, file(ID, &ann.1, Some(&file_pass(&state, &ann, &bo.1, ID, now + 30 * 86_400)))), no, "a pass of thirty days");
    let with_room = serde_json::json!({ "type": "call_credentials", "file": ID, "peer": ann.1, "file_pass": pass, "room": "lounge" });
    assert_eq!(refused(&bo.1, with_room), Some(NOT_IN_IT.to_string()), "a file beside a room");

    // A room's credentials carry no tag: the forwarder never counts them.
    seat(&state, "lounge", &[&bo.1]);
    let room_creds = ask(&state, &bo.1, room("lounge")).unwrap();
    assert_eq!(username_meter(&room_creds.username), None, "a room's credential carries no tag");

    let quiet = Arc::new(RelayState::new(Storage::open_temp("callcreds_file_quiet")));
    quiet.db.register_name("Ann", &ann.1).unwrap();
    quiet.db.register_name("Bo", &bo.1).unwrap();
    let pass_there = file_pass(&quiet, &ann, &bo.1, ID, now + 86_400);
    assert_eq!(ask(&quiet, &bo.1, file(ID, &ann.1, Some(&pass_there))), Err(NOT_SET_UP.to_string()), "no forwarder, no transfer through it");
}

/// THE USERNAME'S TWO SHAPES. A call's and a room's: `{expiry}:{scope}-{nonce}`, no meter. A
/// file's: `{expiry}:{scope}-{nonce}-{tag}-{limit}`, the tag and limit read back exactly; a limit
/// of 0 is "no limit". Anything else in between, an upper-case tag, a missing limit or a sixth
/// part, is not this relay's shape at all (an Allocate with it is challenged).
///
/// Seen red 2026-10-11 with `username_parts` accepting anything after the limit: "a sixth part",
/// left `Some((4600, "7c50.."))`, right `None`.
#[test]
fn usernames_carry_a_meter_only_for_files() {
    let keys = CallKeys::generate();
    let (call, _) = keys.issue(&keys.call_scope("a", "b"), 1_000);
    assert_eq!(parse_username(&call).map(|(e, _)| e), Some(1_000 + CREDENTIAL_TTL_SECS));
    assert_eq!(username_meter(&call), None, "a call's carries none");
    let (f, _) = keys.issue_metered(&keys.file_scope(ID, "a", "b"), "a", 2048, 1_000);
    assert_eq!(parse_username(&f).map(|(_, s)| s.to_string()), Some(keys.file_scope(ID, "a", "b")));
    let m = username_meter(&f).expect("a file's carries one");
    assert_eq!(m, Meter { tag: keys.person_tag("a"), limit_mb: 2048 });
    assert_eq!(m.limit_bytes(), Some(2048 * 1024 * 1024));
    let (free, _) = keys.issue_metered(&keys.file_scope(ID, "a", "b"), "a", 0, 1_000);
    assert_eq!(username_meter(&free).and_then(|m| m.limit_bytes()), None, "0 is no limit");
    assert_ne!(keys.person_tag("a"), keys.person_tag("b"));
    assert_ne!(keys.person_tag("a"), CallKeys::generate().person_tag("a"), "another run, another tag");
    assert_eq!(parse_username(&format!("{f}-7")), None, "a sixth part");
    assert_eq!(parse_username(&f.replace(&keys.person_tag("a"), &keys.person_tag("a").to_uppercase())), None, "an upper-case tag");
    let (head, _) = f.rsplit_once('-').unwrap();
    assert_eq!(parse_username(head), None, "a tag without its limit");
}

/// FILE CREDENTIALS NEED ANOTHER PERSON WHO IS HERE, AND THE LIMIT COUNTS EVERY REQUEST (the relay
/// review of 2026-10-12, rule 5): a pass the asker signs naming a key nobody holds on this server
/// gets nothing, and refused requests count toward the 20 a minute, which is checked before any
/// pass is.
///
/// Seen red 2026-10-12 with the registered-peer check taken out of `file_pass_fits`: "a pass to a
/// key nobody holds here gets no credentials" failed.
#[test]
fn file_credentials_need_a_registered_peer_and_refusals_count_toward_the_limit() {
    let state = fresh_state("file_peer");
    let (ann, bo) = (person(211), person(212));
    state.db.register_name("Ann", &ann.1).unwrap();
    state.db.register_name("Bo", &bo.1).unwrap();
    let now = unix_now();
    let nobody = "ab".repeat(1952);
    let to_nobody = file_pass(&state, &ann, &nobody, ID, now + 86_400);
    assert_eq!(
        ask(&state, &ann.1, file(ID, &nobody, Some(&to_nobody))).err(),
        Some(NO_FILE_PASS.to_string()),
        "a pass to a key nobody holds here gets no credentials"
    );
    let pass = file_pass(&state, &ann, &bo.1, ID, now + 86_400);
    assert!(ask(&state, &bo.1, file(ID, &ann.1, Some(&pass))).is_ok(), "the pass's grantee, both registered: credentials");
    for _ in 0..25 {
        let _ = ask(&state, &bo.1, file(ID, &ann.1, None));
    }
    assert_eq!(
        ask(&state, &bo.1, file(ID, &ann.1, Some(&pass))).err(),
        Some(TOO_MANY.to_string()),
        "after 25 refused requests in a minute, even a good one waits"
    );
}

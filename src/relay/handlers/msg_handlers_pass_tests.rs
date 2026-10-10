// Child of msg_handlers.rs (#[path], so `use super::*` reaches everything it has): friendship
// passes v2 at every contact path (2026-10-09, docs/design/blocking-and-safe-mode.md 10b).
// Kept out of msg_handlers.rs under the file-size ratchet (tests/file_size_ratchet.rs).

use super::*;
use crate::relay::handlers::friend_passes::{friend_pass, handle_cert_revoke, test_pass};
use crate::relay::relay::{Peer, RelayState};

fn fresh_state() -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp("passes")))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A real Dilithium identity: (BIP39 seed, public key hex).
fn identity(seed_byte: u8) -> (Vec<u8>, String) {
    let seed = vec![seed_byte; 32];
    let dil = crate::relay::core::pq_crypto::derive_dilithium_seed(&seed);
    (seed, hex::encode(crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil).public_key()))
}

fn envelope() -> String {
    serde_json::json!({ "v": 2, "ek_ct_b64": "QUJD", "nonce_b64": "REVG", "ct_b64": "R0hJ" }).to_string()
}

fn connect(st: &Arc<RelayState>, key: &str) {
    block(async {
        st.peers.write().await.insert(
            key.to_string(),
            Peer { public_key_hex: key.to_string(), display_name: None, upload_token: None, kyber_public: None, conn_id: 1 },
        );
    });
}

/// One DM from `from` to `to`, with the DM send limiter reset (handlers/dm_rate.rs, tested there).
fn dm(st: &Arc<RelayState>, from: &str, to: &str, pass: Option<&str>) {
    block(async {
        st.dm_rate.forget(from);
        handle_dm_put(st, from, to.to_string(), envelope(), pass.map(str::to_string), reach::DmAsk::Ordinary, None).await;
    });
}

fn landed(st: &Arc<RelayState>, to: &str) -> usize {
    st.db.mailbox_fetch(to, 0, 500).unwrap().len()
}

/// `who` sends `cert_revoke {serial}`; the serials the relay answered `cert_revoked` for, to
/// `who`, and any refusal it gave.
fn revoke(st: &Arc<RelayState>, who: &str, serial: &str) -> (Vec<String>, Vec<String>) {
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_cert_revoke(st, who, &serde_json::json!({ "type": "cert_revoke", "serial": serial })));
    let (mut acks, mut told) = (Vec::new(), Vec::new());
    while let Ok(m) = rx.try_recv() {
        match m {
            RelayMessage::CertRevoked { to, serial } if to == who => acks.push(serial),
            RelayMessage::Private { to, message } if to == who => told.push(message),
            _ => {}
        }
    }
    (acks, told)
}

/// THE POINT OF V2: a pass can be taken back, and the relay honours that at once. Alice holds
/// Bob's pass, so her DMs to him flow after her day's stranger budget is spent; Bob withdraws it
/// (and is answered, so his client stops resending); the same pass now counts as none, and with
/// the budget spent nothing more lands. Carol "withdrawing" Alice's other pass from Bob changes
/// nothing: a withdrawal is recorded under (a fingerprint of) the key that sent it, so nobody
/// can take back a pass they did not give.
/// Seen red 2026-10-09 with the withdrawal check removed from `friend_pass`
/// (handlers/friend_passes.rs): "a withdrawn pass counts as none", left 2, right 1.
#[test]
fn a_withdrawn_pass_falls_back_to_the_strangers_lane() {
    let st = fresh_state();
    let (_alice_seed, alice) = identity(71);
    let (bob_seed, bob) = identity(72);
    let carol = "carol_key";
    for (name, k) in [("Alice", alice.as_str()), ("Bob", bob.as_str()), ("Carol", carol)] {
        st.db.register_name(name, k).unwrap();
    }
    let (pass, serial) = test_pass(&st, &bob_seed, &bob, &alice);
    let (other_pass, other_serial) = test_pass(&st, &bob_seed, &bob, &alice);
    // Both take strangers' mail ("who can reach me", handlers/reach.rs), so the stranger's lane
    // is the daily knock budget rather than a refusal.
    for k in ["stranger_key", bob.as_str()] {
        st.db.set_reach_settings(k, &[("message", "anyone")]).unwrap();
    }

    // Alice spends her day's knocks on a stranger.
    for _ in 0..DM_KNOCKS_PER_DAY {
        dm(&st, &alice, "stranger_key", None);
    }
    assert_eq!(landed(&st, "stranger_key") as u32, DM_KNOCKS_PER_DAY);
    dm(&st, &alice, "stranger_key", None);
    assert_eq!(landed(&st, "stranger_key") as u32, DM_KNOCKS_PER_DAY, "precondition: the budget is spent");

    dm(&st, &alice, &bob, Some(&pass));
    assert_eq!(landed(&st, &bob), 1, "a friend's DM flows past a spent budget");

    let (acks, told) = revoke(&st, &bob, &serial);
    assert_eq!(acks, vec![serial.clone()], "the issuer is answered: {told:?}");
    dm(&st, &alice, &bob, Some(&pass));
    assert_eq!(landed(&st, &bob), 1, "a withdrawn pass counts as none");
    assert!(friend_pass(&st, &bob, &alice, Some(&pass)).is_none());

    // Carol cannot withdraw Bob's other pass to Alice.
    let (acks, _) = revoke(&st, carol, &other_serial);
    assert_eq!(acks, vec![other_serial.clone()], "carol's own (meaningless) withdrawal is recorded under her key");
    dm(&st, &alice, &bob, Some(&other_pass));
    assert_eq!(landed(&st, &bob), 2, "a pass its issuer did not withdraw still works");

    // Withdrawing again is answered again (a client resending after a dropped answer).
    assert_eq!(revoke(&st, &bob, &serial).0, vec![serial]);
}

/// A pass is good only on the server it was given on, and only for the person it names, both
/// read from the relay's own facts (its did:hum, the sender's socket key), never from the pass.
/// Seen red 2026-10-09 with issuer and grantee swapped in `friend_pass`'s call to
/// `verify_friend_cert`: "Bob's pass for Alice works for Alice".
#[test]
fn a_pass_from_another_server_or_for_someone_else_counts_as_none() {
    let st = fresh_state();
    let (_alice_seed, alice) = identity(73);
    let (bob_seed, bob) = identity(74);
    let (_dan_seed, dan) = identity(75);
    let (pass, _) = test_pass(&st, &bob_seed, &bob, &alice);
    assert!(friend_pass(&st, &bob, &alice, Some(&pass)).is_some(), "Bob's pass for Alice works for Alice");
    assert!(friend_pass(&st, &bob, &dan, Some(&pass)).is_none(), "and for nobody else");
    let elsewhere = {
        let other = Storage::open_temp("passes_other");
        let server = other.server_did().unwrap();
        assert_ne!(server, st.db.server_did().unwrap(), "two servers, two identities");
        let serial = crate::relay::core::pq_crypto::new_friend_cert_serial().unwrap();
        crate::relay::core::pq_crypto::build_friend_cert(&bob_seed, &server, &bob, &alice, &serial, &["message"]).unwrap()
    };
    assert!(friend_pass(&st, &bob, &alice, Some(&elsewhere)).is_none(), "a pass given on another server");
    assert!(friend_pass(&st, &bob, &alice, None).is_none());
    assert!(friend_pass(&st, &bob, &alice, Some("")).is_none());
}

/// A trade request follows the pass the same way: a friend's note is kept whole, and once the
/// pass is withdrawn the same request is a stranger's (a knock, its note cut short).
/// Seen red 2026-10-09 with `handle_trade_request` reading the pass from the wrong field
/// ("cert"): "a friend's note is kept whole", left `[80]`, right `[200]`.
#[test]
fn a_trade_request_follows_the_pass_and_its_withdrawal() {
    let st = fresh_state();
    let (_friend_seed, friend) = identity(76);
    let (target_seed, target) = identity(77);
    for (name, k) in [("Friend", friend.as_str()), ("Target", target.as_str())] {
        st.db.register_name(name, k).unwrap();
        connect(&st, k);
    }
    let (pass, serial) = test_pass(&st, &target_seed, &target, &friend);
    // The target takes strangers' trade requests (handlers/reach.rs), so a withdrawn pass makes
    // this a stranger's request rather than a refused one.
    st.db.set_reach_settings(&target, &[("trade", "anyone")]).unwrap();
    let note = "a".repeat(200);
    let send = || {
        block(handle_trade_request(
            &st,
            &friend,
            &serde_json::json!({ "target_key": target, "message": note, "friend_cert": pass }),
        ))
    };
    let notes = || -> Vec<usize> {
        st.db.get_trades_for_user(&friend).unwrap().iter().filter(|t| t.initiator_key == friend).map(|t| t.message.clone().unwrap_or_default().len()).collect()
    };
    send();
    assert_eq!(notes(), vec![200], "a friend's note is kept whole");
    revoke(&st, &target, &serial);
    send();
    let mut got = notes();
    got.sort_unstable();
    assert_eq!(got, vec![TRADE_NOTE_MAX_CHARS_NON_FRIEND, 200], "after the withdrawal the note is a stranger's");
}

/// A ring and a direct-connection offer read the pass they carry and never forward it: the target
/// gets the ring or the offer without the pass, which only the relay needs. On the wire both
/// accept `friend_cert`, and the relay's challenge names its own did:hum, which is what a client
/// signs a pass for. Since step B ("who can reach me", handlers/reach.rs) the pass also decides:
/// with the callee taking calls from friends, a valid pass lets the ring and the offer through,
/// a forged one or none lets neither (the offer has no shared group or voice room to fall back
/// on), and nothing comes back to the caller either way.
/// Seen red 2026-10-09 with handle_voice_call forwarding the caller's `friend_cert` instead of
/// `None`: "the ring reaches the callee without the pass".
#[test]
fn rings_and_offers_read_the_pass_and_never_forward_it() {
    let st = fresh_state();
    let (_caller_seed, caller) = identity(78);
    let (callee_seed, callee) = identity(79);
    connect(&st, &caller);
    connect(&st, &callee);
    st.db.set_reach_settings(&callee, &[("call", "friends")]).unwrap();
    let (pass, serial) = test_pass(&st, &callee_seed, &callee, &caller);
    assert_eq!(
        friend_pass(&st, &callee, &caller, Some(&pass)).map(|p| p.may.wire()).as_deref(),
        Some("invite,message,trade,voice_message")
    );
    assert!(friend_pass(&st, &callee, &caller, None).is_none());

    // From the wire, as a client sends it.
    let ring: RelayMessage = serde_json::from_value(serde_json::json!({
        "type": "voice_call", "from": caller, "from_name": null, "to": callee, "action": "ring", "friend_cert": pass,
    }))
    .unwrap();
    let RelayMessage::VoiceCall { to, action, friend_cert, .. } = ring else { panic!("a voice_call") };
    assert_eq!(friend_cert.as_deref(), Some(pass.as_str()), "the ring carries the pass to the relay");
    let offer: RelayMessage = serde_json::from_value(serde_json::json!({
        "type": "webrtc_signal", "to": callee, "signal_type": "dc_offer", "data": "{}", "friend_cert": pass,
    }))
    .unwrap();
    assert!(matches!(offer, RelayMessage::WebrtcSignal { friend_cert: Some(_), .. }), "so does a dc_offer");

    for cert in [Some(pass.clone()), Some("Zm9yZ2Vk".to_string()), None] {
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_voice_call(&st, &caller, to.clone(), action.clone(), cert.clone()));
        block(handle_webrtc_signal(&st, &caller, callee.clone(), "dc_offer".into(), serde_json::json!("{}"), cert.clone()));
        let (mut rings, mut offers, mut to_caller) = (0, 0, 0);
        while let Ok(m) = rx.try_recv() {
            match m {
                RelayMessage::VoiceCall { to, friend_cert, .. } if to == callee => {
                    assert!(friend_cert.is_none(), "the ring reaches the callee without the pass");
                    rings += 1;
                }
                RelayMessage::WebrtcSignal { ref to, ref friend_cert, .. } if *to == callee => {
                    assert!(friend_cert.is_none(), "the offer reaches its target without the pass");
                    assert!(!serde_json::to_string(&m).unwrap().contains("friend_cert"), "and the wire form has no such field");
                    offers += 1;
                }
                RelayMessage::Private { ref to, .. } | RelayMessage::ReachRefused { sender: ref to, .. } if *to == caller => to_caller += 1,
                _ => {}
            }
        }
        let through = usize::from(cert.as_deref() == Some(pass.as_str()));
        assert_eq!((rings, offers, to_caller), (through, through, 0), "the pass {cert:?} decides, and the caller hears nothing back");
    }
    revoke(&st, &callee, &serial);
    assert!(friend_pass(&st, &callee, &caller, Some(&pass)).is_none(), "a withdrawn pass reads as none");

    let challenge = RelayMessage::IdentifyChallenge { nonce: "ab".into(), server_did: st.db.server_did().unwrap() };
    let wire: serde_json::Value = serde_json::to_value(&challenge).unwrap();
    assert_eq!(wire["server_did"], st.db.server_did().unwrap(), "the challenge names the server a pass is signed for");
}

/// A server that cannot keep its fingerprint secret (a damaged `erased-accounts.key`, the
/// erased-accounts module's "this run only" case) fails safe: a pass counts as withdrawn, so a
/// friend's DM falls to the stranger's lane, and a withdrawal is neither recorded nor confirmed,
/// so the issuer's client keeps resending it until a run can keep it.
/// Seen red 2026-10-09 with `CannotKeep` answered like `Recorded` in `handle_cert_revoke`: "a
/// withdrawal this run cannot keep is not confirmed: [\"8ac2d454..\"]" (the serial answered).
#[test]
fn a_server_that_cannot_keep_withdrawals_fails_safe() {
    let dir = crate::test_temp::dir("passes_damaged_secret");
    std::fs::write(dir.join(crate::relay::storage::erased_accounts::KEY_FILE), b"0123456789").unwrap();
    let st = Arc::new(RelayState::new(Storage::open(&dir.join("relay.db")).expect("opens with a damaged secret")));
    assert!(!st.db.erase_memory_kept(), "precondition: this run cannot keep its secret");
    let (_alice_seed, alice) = identity(91);
    let (bob_seed, bob) = identity(92);
    st.db.register_name("Alice", &alice).unwrap();
    st.db.register_name("Bob", &bob).unwrap();
    let (pass, serial) = test_pass(&st, &bob_seed, &bob, &alice);
    for k in ["stranger_key", bob.as_str()] {
        st.db.set_reach_settings(k, &[("message", "anyone")]).unwrap(); // the stranger's lane is the knock budget
    }
    assert!(friend_pass(&st, &bob, &alice, Some(&pass)).is_none(), "the pass counts as withdrawn");
    for _ in 0..DM_KNOCKS_PER_DAY {
        dm(&st, &alice, "stranger_key", None);
    }
    dm(&st, &alice, &bob, Some(&pass));
    assert_eq!(landed(&st, &bob), 0, "a friend's DM is in the stranger's lane, whose budget is spent");
    let (acks, told) = revoke(&st, &bob, &serial);
    assert!(acks.is_empty(), "a withdrawal this run cannot keep is not confirmed: {acks:?}");
    assert!(told.is_empty(), "and the person is not shown an error their app will retry by itself: {told:?}");
    assert_eq!(st.db.friend_cert_withdrawals_of(&bob), 0);
}

/// A withdrawal must name a pass serial; anything else is refused with a reason and records
/// nothing.
/// Seen red 2026-10-09 with the serial check removed from `handle_cert_revoke`: "\"\" is not
/// acknowledged" (the empty word was recorded and answered as a withdrawal).
#[test]
fn a_withdrawal_must_name_a_serial() {
    let st = fresh_state();
    for bad in ["", "not-a-serial", "00112233445566778899AABBCCDDEEFF", "0011"] {
        let (acks, told) = revoke(&st, "ada_key", bad);
        assert!(acks.is_empty(), "{bad:?} is not acknowledged");
        assert!(told.iter().any(|m| m.contains("not a friendship pass serial")), "{bad:?}: the sender is told: {told:?}");
    }
    assert_eq!(st.db.friend_cert_withdrawals_of("ada_key"), 0, "a word that is not a serial");
}

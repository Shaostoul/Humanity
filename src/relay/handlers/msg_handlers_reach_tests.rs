// Child of msg_handlers.rs (#[path], so `use super::*` reaches everything it has),
// moved out under the file-size ratchet on 2026-10-09 (tests/file_size_ratchet.rs).

use super::*;
use crate::relay::relay::{Peer, RelayState};

fn fresh_state() -> Arc<RelayState> {
    let db = Storage::open_temp("reach");
    Arc::new(RelayState::new(db))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A real Dilithium identity, for the friendship certificate tests.
fn identity(seed_byte: u8) -> (Vec<u8>, String) {
    let seed = vec![seed_byte; 32];
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(&seed);
    let hex_pk = hex::encode(
        crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed).public_key(),
    );
    (seed, hex_pk)
}

fn envelope() -> String {
    serde_json::json!({
        "v": 2, "ek_ct_b64": "QUJD", "nonce_b64": "REVG", "ct_b64": "R0hJ"
    })
    .to_string()
}

/// Mark `key` as connected, the way a socket that finished identify is.
fn connect(st: &Arc<RelayState>, key: &str) {
    block(async {
        st.peers.write().await.insert(
            key.to_string(),
            Peer {
                public_key_hex: key.to_string(),
                display_name: None,
                upload_token: None,
                kyber_public: None,
                conn_id: 1,
            },
        );
    });
}

/// A member who keeps the default and hides their presence.
fn hidden_member(st: &Arc<RelayState>, key: &str) {
    st.db.join_server(key, key).unwrap();
    assert!(st.db.presence_hidden(key), "precondition: new members join hidden");
}

/// A member who chose to show their presence.
fn visible_member(st: &Arc<RelayState>, key: &str) {
    st.db.join_server(key, key).unwrap();
    st.db.set_hide_presence(key, false).unwrap();
}

/// Everything addressed to `who` that a handler put on the broadcast bus, in
/// a form two runs can be compared by: a trade record becomes its status
/// alone (its id, time and the recipient's key differ by construction and
/// say nothing about presence), any other message stays as it is.
fn replies_to(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>, who: &str) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        match msg {
            RelayMessage::Private { to, message } if to == who => {
                if let Some(json) = message.strip_prefix("__trade_data__:") {
                    let v: serde_json::Value = serde_json::from_str(json).unwrap();
                    out.push(format!("trade_data status={}", v["trade"]["status"]));
                } else {
                    out.push(message);
                }
            }
            RelayMessage::VoiceCall { to, action, .. } if to == who => {
                out.push(format!("voice_call {action}"));
            }
            RelayMessage::WebrtcSignal { to, signal_type, .. } if to == who => {
                out.push(format!("webrtc_signal {signal_type}"));
            }
            _ => {}
        }
    }
    out
}

/// The defect (3.7.3): these three answered "not online" for an offline
/// target and stayed silent for an online one, whoever the target was, so
/// ringing a person who hides their presence told you whether they were
/// connected. Now a hidden target's sender gets the same reply in both
/// states, an online hidden target still receives what was sent, and a
/// visible offline target is still reported offline.
///
/// Seen red 2026-10-09: with `may_report_offline` returning true for
/// everyone (the presence check disabled), the first comparison failed for
/// voice_call: the hidden offline target's caller got "User is not
/// online." and the hidden online target's caller got nothing. Restored, it
/// passes.
#[test]
fn hidden_presence_gets_the_same_answer_online_or_not() {
    let st = fresh_state();
    let caller = "caller_key";
    let (hid_on, hid_off, vis_off) = ("hidden_online_key", "hidden_offline_key", "visible_offline_key");
    hidden_member(&st, hid_on);
    hidden_member(&st, hid_off);
    visible_member(&st, vis_off);
    connect(&st, caller);
    connect(&st, hid_on);
    // This test is about presence, so "who can reach me" (handlers/reach.rs, tested in
    // reach_tests.rs) is set out of its way: each target takes calls and trade requests from
    // anyone and shares a voice room with the caller, which lets a direct-connection offer in.
    for t in [hid_on, hid_off, vis_off] {
        st.db.set_reach_settings(t, &[("call", "anyone"), ("trade", "anyone")]).unwrap();
    }
    let lounge = crate::relay::relay::VoiceRoom {
        name: "Lounge".into(),
        participants: [caller, hid_on, hid_off, vis_off].iter().map(|k| (k.to_string(), k.to_string())).collect(),
    };
    block(async { st.voice_rooms.write().await.insert("lounge".into(), lounge) });

    // A ring and a direct-connection offer.
    type SendFn = fn(&Arc<RelayState>, &str, &str);
    let sends: [(&str, SendFn); 2] = [
        ("voice_call", |st, from, to| {
            block(handle_voice_call(st, from, to.to_string(), "ring".to_string(), None))
        }),
        ("webrtc_signal", |st, from, to| {
            block(handle_webrtc_signal(
                st,
                from,
                to.to_string(),
                "dc_offer".to_string(),
                serde_json::json!({ "sdp": "x" }),
                None,
            ))
        }),
    ];
    // Send once to `target`; what the caller saw, and what the target got.
    let probe = |send: SendFn, target: &str| -> (Vec<String>, Vec<String>) {
        let mut to_caller = st.broadcast_tx.subscribe();
        let mut to_target = st.broadcast_tx.subscribe();
        send(&st, caller, target);
        (replies_to(&mut to_caller, caller), replies_to(&mut to_target, target))
    };
    for (name, send) in sends {
        let (caller_saw_online, target_got) = probe(send, hid_on);
        let (caller_saw_offline, _) = probe(send, hid_off);
        assert_eq!(
            caller_saw_online, caller_saw_offline,
            "{name}: a hidden person's caller must get the same reply online or not"
        );
        assert_eq!(target_got.len(), 1, "{name}: a hidden online target still gets it: {target_got:?}");
        let (caller_saw_visible, _) = probe(send, vis_off);
        assert_eq!(
            caller_saw_visible,
            vec!["User is not online.".to_string()],
            "{name}: a visible offline target is still reported offline"
        );
    }

    // A trade request. The reply to the sender is the trade record they
    // created, pending, in both cases; the online target is sent it too.
    let trade: SendFn = |st, from, to| {
        block(handle_trade_request(st, from, &serde_json::json!({ "target_key": to, "message": "hi" })))
    };
    let (to_online, target_got) = probe(trade, hid_on);
    let (to_offline, _) = probe(trade, hid_off);
    assert_eq!(to_online, to_offline, "trade: same reply whether the hidden target is online or not");
    assert_eq!(to_online, vec!["trade_data status=\"pending\"".to_string()]);
    assert_eq!(
        target_got,
        vec!["trade_data status=\"pending\"".to_string()],
        "a hidden online target still receives the trade request"
    );
    assert_eq!(
        probe(trade, vis_off).0,
        vec!["Trade target is not online.".to_string()],
        "trade: a visible offline target is still reported offline"
    );
}

/// 3.7.5: a trade request was free text to anyone online with no daily
/// ceiling. Without a friendship certificate it now spends from the same
/// per-sender daily budget as a DM knock, in both directions: knocks spent
/// on DMs block trade requests, and trade requests use up knocks for DMs. A
/// valid certificate from the target lifts the limit, as it does for DMs.
///
/// Seen red 2026-10-09: with the `spend_knock` call in handle_trade_request
/// disabled, "trade requests and DM knocks spend one budget" failed with 20
/// DMs landing instead of 15 (the five trade requests had spent nothing).
/// Restored, it passes.
#[test]
fn trade_requests_share_the_knock_budget() {
    let st = fresh_state();
    let sender = "stranger_key";
    st.db.register_name("Stranger", sender).unwrap();
    let (target_seed, target) = identity(61);
    st.db.register_name("Target", &target).unwrap();
    connect(&st, sender);
    connect(&st, &target);
    // Both take strangers' requests and mail (handlers/reach.rs), so the stranger's lane is the
    // knock budget rather than a refusal.
    st.db.set_reach_settings(&target, &[("trade", "anyone")]).unwrap();
    st.db.set_reach_settings("dm_target_key", &[("message", "anyone")]).unwrap();
    let trades = |st: &Arc<RelayState>| st.db.get_trades_for_user(sender).unwrap().len();

    // Five trade requests, each a knock.
    for i in 0..5 {
        block(handle_trade_request(&st, sender, &serde_json::json!({ "target_key": target, "message": format!("t{i}") })));
    }
    assert_eq!(trades(&st), 5);

    // So only the rest of the day's knocks are left for DMs.
    let dm_target = "dm_target_key";
    for _ in 0..(DM_KNOCKS_PER_DAY + 5) {
        block(async {
            st.rate_limits.write().await.remove(sender);
            handle_dm_put(&st, sender, dm_target.to_string(), envelope(), None, false).await;
        });
    }
    assert_eq!(
        st.db.mailbox_fetch(dm_target, 0, 200).unwrap().len() as u32,
        DM_KNOCKS_PER_DAY - 5,
        "trade requests and DM knocks spend one budget"
    );

    // The budget is spent: a certless trade request is refused, with the reason.
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_trade_request(&st, sender, &serde_json::json!({ "target_key": target, "message": "again" })));
    assert_eq!(trades(&st), 5, "a stranger's trade request after the day's knocks is refused");
    let told = replies_to(&mut rx, sender);
    assert!(told.iter().any(|m| m.contains("Daily limit")), "the sender is told why: {told:?}");

    // A forged certificate counts as none.
    block(handle_trade_request(
        &st,
        sender,
        &serde_json::json!({ "target_key": target, "message": "x", "friend_cert": "Zm9yZ2VkLXNpZw==" }),
    ));
    assert_eq!(trades(&st), 5, "a forged certificate must not lift the limit");

    // The target's real certificate for this sender lifts it.
    let (cert, _) = crate::relay::handlers::friend_passes::test_pass(&st, &target_seed, &target, sender);
    block(handle_trade_request(
        &st,
        sender,
        &serde_json::json!({ "target_key": target, "message": "friend", "friend_cert": cert }),
    ));
    assert_eq!(trades(&st), 6, "a friend's trade request is not limited by the knock budget");
}

/// 3.7.5: a non-friend's trade note is cut to TRADE_NOTE_MAX_CHARS_NON_FRIEND
/// characters (counted as characters, so a multi-byte note is never cut
/// inside a character), the sender is told, and the trade itself still goes
/// through. A friend's note is kept whole.
///
/// Seen red 2026-10-09: with the cut disabled (`note` set to the whole
/// message for everyone), "a stranger's note is cut" failed with the full
/// 200-character note stored. Restored, it passes.
#[test]
fn a_strangers_trade_note_is_cut_and_a_friends_is_not() {
    let st = fresh_state();
    let stranger = "stranger_key";
    let (_friend_seed, friend) = identity(62);
    let (target_seed, target) = identity(63);
    for (name, k) in [("Stranger", stranger), ("Friend", friend.as_str()), ("Target", target.as_str())] {
        st.db.register_name(name, k).unwrap();
        connect(&st, k);
        st.db.set_reach_settings(k, &[("trade", "anyone")]).unwrap(); // strangers' requests let in (handlers/reach.rs)
    }
    let long_note: String = "é".repeat(200);

    let mut rx = st.broadcast_tx.subscribe();
    block(handle_trade_request(&st, stranger, &serde_json::json!({ "target_key": target, "message": long_note })));
    let stored = st.db.get_trades_for_user(stranger).unwrap();
    assert_eq!(stored.len(), 1, "the trade itself still goes through");
    let note = stored[0].message.clone().unwrap_or_default();
    assert_eq!(note.chars().count(), TRADE_NOTE_MAX_CHARS_NON_FRIEND, "a stranger's note is cut");
    assert_eq!(note, "é".repeat(TRADE_NOTE_MAX_CHARS_NON_FRIEND));
    let told = replies_to(&mut rx, stranger);
    assert!(told.iter().any(|m| m.contains("shortened")), "the sender is told: {told:?}");

    // A note at the limit is not touched, and nobody is told anything.
    let mut rx = st.broadcast_tx.subscribe();
    let exact = "a".repeat(TRADE_NOTE_MAX_CHARS_NON_FRIEND);
    block(handle_trade_request(&st, stranger, &serde_json::json!({ "target_key": friend, "message": exact })));
    assert!(!replies_to(&mut rx, stranger).iter().any(|m| m.contains("shortened")));

    // The friend holds the target's certificate: their note is kept whole.
    let (cert, _) = crate::relay::handlers::friend_passes::test_pass(&st, &target_seed, &target, &friend);
    block(handle_trade_request(
        &st,
        &friend,
        &serde_json::json!({ "target_key": target, "message": long_note, "friend_cert": cert }),
    ));
    let mine: Vec<_> = st
        .db
        .get_trades_for_user(&friend)
        .unwrap()
        .into_iter()
        .filter(|t| t.initiator_key == friend)
        .collect();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].message.as_deref(), Some(long_note.as_str()), "a friend's note is kept whole");
}

// Child of reach.rs (#[path], so `use super::*` reaches everything it has): "who can reach me" at
// each door, under each audience (2026-10-09, docs/design/blocking-and-safe-mode.md 10c, its
// Proof list). The handlers are driven directly against a real database, as the other handler
// tests are; the socket half (the settings after identify, and with every feature switched
// off) is in features.rs.

use super::*;
use crate::relay::core::pq_crypto::{build_friend_cert, derive_dilithium_seed, new_friend_cert_serial, DilithiumKeypair, KYBER_CIPHERTEXT_LEN};
use crate::relay::handlers::msg_handlers::{handle_dm_put, handle_trade_request, handle_voice_call, handle_webrtc_signal};
use crate::relay::relay::{Peer, VoiceRoom};
use crate::relay::storage::Storage;

fn fresh_state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("reach_{tag}"))))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A real identity: its BIP39 seed and its Dilithium3 public key, hex.
struct Person {
    seed: Vec<u8>,
    key: String,
}

fn person(seed_byte: u8) -> Person {
    let seed = vec![seed_byte; 32];
    let key = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key());
    Person { seed, key }
}

impl Person {
    fn keypair(&self) -> DilithiumKeypair {
        DilithiumKeypair::from_seed(&derive_dilithium_seed(&self.seed))
    }
}

/// Mark `key` as connected, the way a socket that finished identify is.
fn connect(st: &Arc<RelayState>, key: &str) {
    block(async {
        st.peers.write().await.insert(
            key.to_string(),
            Peer { public_key_hex: key.to_string(), display_name: None, upload_token: None, kyber_public: None, conn_id: 1 },
        );
    });
}

/// The pass `issuer` gives `grantee` on this relay, allowing `may`.
fn pass(st: &Arc<RelayState>, issuer: &Person, grantee: &str, may: &[&str]) -> String {
    let server = st.db.server_did().expect("the relay's own did:hum");
    let serial = new_friend_cert_serial().expect("randomness");
    build_friend_cert(&issuer.seed, &server, &issuer.key, grantee, &serial, may).expect("a pass")
}

/// What two new friends' passes allow (step A): everything but calls.
const DEFAULT_MAY: [&str; 4] = ["invite", "message", "trade", "voice_message"];

/// A v2 envelope of the size every signed DM has: a signed inner payload carries two Dilithium3
/// keys and a signature (about 12 KB), so the clients pad it to the 16,384-byte bucket
/// (`net::dm_pq::DM_PAD_BUCKETS`), and AES-GCM adds its 16-byte tag. The relay cannot look in.
fn envelope() -> String {
    sealed(16_384 + 16)
}

/// A v2 envelope shaped as the clients seal one: an ML-KEM-768 ciphertext, a 12-byte nonce and a
/// body of `ct_len` bytes (the padded plaintext plus the tag).
fn sealed(ct_len: usize) -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    serde_json::json!({
        "v": 2,
        "ek_ct_b64": B64.encode(vec![7u8; KYBER_CIPHERTEXT_LEN]),
        "nonce_b64": B64.encode([9u8; 12]),
        "ct_b64": B64.encode(vec![5u8; ct_len]),
    })
    .to_string()
}

fn set(st: &Arc<RelayState>, key: &str, kind: Kind, audience: Audience) {
    st.db.set_reach_settings(key, &[(kind.word(), audience.word())]).unwrap();
}

/// Everything the bus carried to `who`, in a form a test can compare: a refusal as
/// "reach_refused <kind> <to>", a notice as its text, a call message as "voice_call <action>" or
/// "webrtc_signal <type>", a trade record as "trade_data".
fn heard(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>, who: &str) -> Vec<String> {
    let mut all = Vec::new();
    while let Ok(m) = rx.try_recv() {
        all.push(m);
    }
    heard_in(&all, who)
}

/// `heard` over messages already taken off the bus.
fn heard_in(all: &[RelayMessage], who: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in all {
        match m {
            RelayMessage::ReachRefused { sender, kind, to } if sender == who => out.push(format!("reach_refused {kind} {to}")),
            RelayMessage::Private { to, message } if to == who => {
                out.push(if message.starts_with("__trade_data__:") { "trade_data".to_string() } else { message.clone() })
            }
            RelayMessage::VoiceCall { to, action, .. } if to == who => out.push(format!("voice_call {action}")),
            RelayMessage::WebrtcSignal { to, signal_type, .. } if to == who => out.push(format!("webrtc_signal {signal_type}")),
            RelayMessage::DmNew { target: Some(to), .. } if to == who => out.push("dm_new".to_string()),
            _ => {}
        }
    }
    out
}

/// Trades `sender` asked `target` for.
fn trades(st: &Arc<RelayState>, sender: &str, target: &str) -> usize {
    st.db.get_trades_for_user(target).unwrap().iter().filter(|t| t.initiator_key == sender && t.recipient_key == target).count()
}

/// `sender` tries to reach `target` by `kind`, presenting `cert`: (was it delivered, what the
/// sender heard back). A message is delivered when it lands in the mailbox, a call when the ring
/// reaches the target, a trade when the request is stored. The rate limiter is reset first: it
/// has tests of its own.
fn try_reach(st: &Arc<RelayState>, kind: Kind, sender: &str, target: &str, cert: Option<&str>) -> (bool, Vec<String>) {
    let mut rx = st.broadcast_tx.subscribe();
    let stored = match kind {
        Kind::Message => {
            let before = st.db.mailbox_fetch(target, 0, 1_000).unwrap().len();
            block(async {
                st.rate_limits.write().await.remove(sender);
                handle_dm_put(st, sender, target.to_string(), envelope(), cert.map(str::to_string), false).await;
            });
            st.db.mailbox_fetch(target, 0, 1_000).unwrap().len() > before
        }
        Kind::Call => {
            block(handle_voice_call(st, sender, target.to_string(), "ring".to_string(), cert.map(str::to_string)));
            false
        }
        Kind::Trade => {
            let before = trades(st, sender, target);
            block(handle_trade_request(st, sender, &serde_json::json!({ "target_key": target, "message": "hi", "friend_cert": cert })));
            trades(st, sender, target) > before
        }
    };
    let mut all = Vec::new();
    while let Ok(m) = rx.try_recv() {
        all.push(m);
    }
    let delivered = match kind {
        Kind::Call => heard_in(&all, target).contains(&"voice_call ring".to_string()),
        _ => stored,
    };
    (delivered, heard_in(&all, sender))
}

/// Put `members` in one P2P group that `creator` starts: the group, then each member's own
/// request to join and the creator's admit (the consenting path, storage/groups_p2p.rs).
fn group(st: &Arc<RelayState>, creator: &Person, members: &[&Person], name: &str) -> String {
    use crate::relay::core::object::ObjectBuilder;
    use ciborium::Value;
    let text = |k: &str, v: &str| (Value::Text(k.into()), Value::Text(v.into()));
    let g = ObjectBuilder::new("group_v1")
        .created_at(1000)
        .payload_cbor(&Value::Map(vec![text("name", name)]))
        .unwrap()
        .sign(&creator.keypair())
        .unwrap();
    let gid = g.object_id().unwrap().to_hex();
    assert!(st.db.put_signed_object(&g, None).unwrap());
    for m in members {
        let ask = ObjectBuilder::new("group_join_v1")
            .reference(&gid)
            .reference("no-such-invite")
            .created_at(1003)
            .payload_cbor(&Value::Map(vec![(Value::Text("secret".into()), Value::Bytes(vec![1u8; 16]))]))
            .unwrap()
            .sign(&m.keypair())
            .unwrap();
        st.db.put_signed_object(&ask, None).unwrap();
        let admit = ObjectBuilder::new("group_member_v1")
            .reference(&gid)
            .created_at(1001)
            .payload_cbor(&Value::Map(vec![
                text("action", "admit"),
                (Value::Text("subject".into()), Value::Bytes(m.keypair().public_key().to_vec())),
            ]))
            .unwrap()
            .sign(&creator.keypair())
            .unwrap();
        st.db.put_signed_object(&admit, None).unwrap();
        assert!(st.db.p2p_group_has_member(&gid, &m.keypair().public_key()).unwrap(), "precondition: in the group");
    }
    gid
}

/// THE MATRIX: each kind the relay enforces (message, call, trade), under each audience, from
/// each kind of sender: a stranger, someone sharing a P2P group with the target (no pass), a
/// friend holding the default pass (no calls), and a friend whose pass allows calls only. What a
/// refused sender hears: `reach_refused {kind, to}` for a message or a trade, nothing at all for
/// a call; a sender let in never hears a refusal.
///
/// Seen red 2026-10-09 with the `Chosen` arm of `allowed` reading `pass.is_some()` (any pass, the
/// `friends` rule): "Message under Chosen from the friend whose pass allows calls only: delivered
/// true, wanted false".
#[test]
fn every_kind_under_every_audience_from_every_kind_of_sender() {
    let st = fresh_state("matrix");
    let (target, stranger, mate, friend, caller) = (person(101), person(102), person(103), person(104), person(105));
    for p in [&target, &stranger, &mate, &friend, &caller] {
        connect(&st, &p.key);
    }
    group(&st, &target, &[&mate], "Neighbours");
    let default_pass = pass(&st, &target, &friend.key, &DEFAULT_MAY);
    let call_pass = pass(&st, &target, &caller.key, &["call"]);
    let senders: [(&str, &Person, Option<&str>); 4] = [
        ("stranger", &stranger, None),
        ("group mate", &mate, None),
        ("friend with the default pass", &friend, Some(&default_pass)),
        ("friend whose pass allows calls only", &caller, Some(&call_pass)),
    ];
    let want = |kind: Kind, audience: Audience, who: &str| -> bool {
        let friend_default = who == "friend with the default pass";
        let friend_call = who == "friend whose pass allows calls only";
        match audience {
            Audience::Nobody => false,
            Audience::Chosen => (friend_default && kind != Kind::Call) || (friend_call && kind == Kind::Call),
            Audience::Friends => friend_default || friend_call,
            Audience::Groups => friend_default || friend_call || who == "group mate",
            Audience::Anyone => true,
        }
    };
    for kind in Kind::ALL {
        for audience in Audience::ALL {
            set(&st, &target.key, kind, audience);
            for (who, sender, cert) in senders {
                let (delivered, back) = try_reach(&st, kind, &sender.key, &target.key, cert);
                let wanted = want(kind, audience, who);
                assert_eq!(delivered, wanted, "{kind:?} under {audience:?} from the {who}: delivered {delivered}, wanted {wanted}");
                let refusal = format!("reach_refused {} {}", kind.word(), target.key);
                if wanted {
                    assert!(!back.iter().any(|m| m.starts_with("reach_refused")), "{kind:?} under {audience:?}, {who} let in but told: {back:?}");
                } else if kind == Kind::Call {
                    assert!(back.is_empty(), "a refused call hears nothing at all, {audience:?}, {who}: {back:?}");
                } else {
                    assert_eq!(back, vec![refusal], "{kind:?} under {audience:?}, {who}: the refusal everyone gets");
                }
            }
        }
    }
}

/// A person who saved nothing gets the safe defaults (10c): messages and trade requests from
/// friends, calls from people they choose. So a stranger reaches them by none of the three, a
/// friend with the default pass by message and trade but not by call, and a friend whose pass
/// allows calls by call too. The settings read back with every kind filled in.
///
/// Seen red 2026-10-09 with `Kind::Call`'s default audience set to `Friends`: "the safe defaults,
/// every kind filled in", left `call: "friends"`, right `call: "chosen"`.
#[test]
fn a_person_with_no_saved_settings_gets_the_safe_defaults() {
    let st = fresh_state("defaults");
    let (target, stranger, friend, close) = (person(111), person(112), person(113), person(114));
    for p in [&target, &stranger, &friend, &close] {
        connect(&st, &p.key);
    }
    assert!(st.db.reach_settings_of(&target.key).is_empty(), "precondition: nothing saved");
    assert_eq!(
        settings_of(&st, &target.key),
        ReachSettings { message: "friends".into(), call: "chosen".into(), trade: "friends".into() },
        "the safe defaults, every kind filled in"
    );
    let default_pass = pass(&st, &target, &friend.key, &DEFAULT_MAY);
    let close_pass = pass(&st, &target, &close.key, &["call", "invite", "message", "trade", "voice_message"]);
    for kind in Kind::ALL {
        assert!(!try_reach(&st, kind, &stranger.key, &target.key, None).0, "a stranger cannot reach them by {kind:?} by default");
    }
    assert!(try_reach(&st, Kind::Message, &friend.key, &target.key, Some(&default_pass)).0, "a friend can message by default");
    assert!(try_reach(&st, Kind::Trade, &friend.key, &target.key, Some(&default_pass)).0, "a friend can ask to trade by default");
    assert!(!try_reach(&st, Kind::Call, &friend.key, &target.key, Some(&default_pass)).0, "a friend whose pass does not allow calls cannot call by default");
    assert!(try_reach(&st, Kind::Call, &close.key, &target.key, Some(&close_pass)).0, "someone they chose can call");
    // A withdrawn pass is no pass: back to the stranger's answer.
    let serial = crate::relay::core::pq_crypto::parse_friend_cert(&close_pass).unwrap().0.serial;
    st.db.withdraw_friend_cert(&target.key, &serial, 20_000).unwrap();
    let (delivered, back) = try_reach(&st, Kind::Message, &close.key, &target.key, Some(&close_pass));
    assert!(!delivered && back == vec![format!("reach_refused message {}", target.key)], "a withdrawn pass: {back:?}");
}

/// `groups`: friends, plus people who share a P2P group with the person. Someone in a different
/// group, or in no group, is a stranger; leaving the group (removed from it) ends it.
///
/// Seen red 2026-10-09 with `share_a_group` answering false always: "someone in a group with
/// them can message under groups".
#[test]
fn groups_lets_in_people_who_share_a_p2p_group_and_no_one_else() {
    let st = fresh_state("groups");
    let (target, mate, outsider, other_creator) = (person(121), person(122), person(123), person(124));
    for p in [&target, &mate, &outsider, &other_creator] {
        connect(&st, &p.key);
    }
    let gid = group(&st, &target, &[&mate], "Allotment");
    group(&st, &other_creator, &[&outsider], "Elsewhere");
    for kind in Kind::ALL {
        set(&st, &target.key, kind, Audience::Groups);
    }
    for kind in Kind::ALL {
        assert!(try_reach(&st, kind, &mate.key, &target.key, None).0, "someone in a group with them can {kind:?} under groups");
        assert!(!try_reach(&st, kind, &outsider.key, &target.key, None).0, "someone in another group cannot {kind:?}");
    }
    // The group mate is removed: from then on a stranger.
    use crate::relay::core::object::ObjectBuilder;
    use ciborium::Value;
    let removal = ObjectBuilder::new("group_member_v1")
        .reference(&gid)
        .created_at(1005)
        .payload_cbor(&Value::Map(vec![
            (Value::Text("action".into()), Value::Text("remove".into())),
            (Value::Text("subject".into()), Value::Bytes(mate.keypair().public_key().to_vec())),
        ]))
        .unwrap()
        .sign(&target.keypair())
        .unwrap();
    st.db.put_signed_object(&removal, None).unwrap();
    assert!(!st.db.p2p_group_has_member(&gid, &mate.keypair().public_key()).unwrap(), "precondition: removed");
    assert!(!try_reach(&st, Kind::Message, &mate.key, &target.key, None).0, "a removed member is a stranger again");
}

/// Admins and moderators are bound like everyone (10a, question 5): no role is let past an
/// audience. Under the defaults, staff without a pass reach no one by any kind, and `nobody`
/// refuses them even holding a pass that allows everything.
///
/// Seen red 2026-10-09 with `allowed` answering true for an admin or moderator sender (the
/// exemption this test forbids, `state.db.get_role(sender)` checked first): "a admin without a
/// pass cannot Message someone at the defaults".
#[test]
fn admins_and_moderators_are_bound() {
    for (n, role) in [(131u8, "admin"), (135u8, "mod")] {
        let st = fresh_state(&format!("staff_{role}"));
        let (target, staff) = (person(n), person(n + 1));
        connect(&st, &target.key);
        connect(&st, &staff.key);
        st.db.set_role(&staff.key, role).unwrap();
        for kind in Kind::ALL {
            assert!(!try_reach(&st, kind, &staff.key, &target.key, None).0, "a {role} without a pass cannot {kind:?} someone at the defaults");
        }
        let everything = pass(&st, &target, &staff.key, &["call", "invite", "message", "trade", "voice_message"]);
        for kind in Kind::ALL {
            set(&st, &target.key, kind, Audience::Nobody);
            assert!(!try_reach(&st, kind, &staff.key, &target.key, Some(&everything)).0, "nobody means nobody, {role} included ({kind:?})");
        }
        // A contact request is bound by nobody too.
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_dm_put(&st, &staff.key, target.key.clone(), envelope(), None, true));
        assert_eq!(heard(&mut rx, &staff.key), vec![format!("reach_refused message {}", target.key)]);
    }
}

/// A contact request (10c, as amended in review on 2026-10-09). A stranger refused as a message
/// by the defaults may send `contact_request: true` on an ordinary signed, sealed DM of ordinary
/// size. It is let through, with nothing said back and no knock spent, under every audience but
/// `nobody`, which refuses it with the refusal everyone gets; the ordinary DM size limit still
/// holds. The relay keeps no record of the request, yet the accepter's reply reaches the
/// requester, who takes messages from friends only: it presents the requester's own pass (which
/// the request carried, sealed) as its `friend_cert`. The same reply without that pass is refused.
///
/// Seen red 2026-10-09 twice: with the `nobody` check taken out of `dm_gate`'s contact-request
/// branch, "nobody means no contact requests either" (left 5, right 4); and with `admit` checking
/// the pass with issuer and grantee swapped, "a reply carrying the requester's pass is admitted
/// under friends: (false, [\"reach_refused message <the requester's key>\"])".
#[test]
fn a_contact_request_gets_through_at_ordinary_size_and_the_reply_carrying_its_pass_is_admitted() {
    let st = fresh_state("contact_request");
    let (target, stranger) = (person(141), person(142));
    connect(&st, &target.key);
    connect(&st, &stranger.key);
    let landed = |st: &Arc<RelayState>| st.db.mailbox_fetch(&target.key, 0, 100).unwrap().len();
    let request = |content: String| -> Vec<String> {
        let mut rx = st.broadcast_tx.subscribe();
        block(async {
            st.rate_limits.write().await.remove(&stranger.key);
            handle_dm_put(&st, &stranger.key, target.key.clone(), content, None, true).await;
        });
        heard(&mut rx, &stranger.key)
    };

    let (delivered, back) = try_reach(&st, Kind::Message, &stranger.key, &target.key, None);
    assert!(!delivered && back == vec![format!("reach_refused message {}", target.key)], "precondition: refused as a message");

    // The defaults (friends), then each other audience but nobody.
    assert!(request(envelope()).is_empty(), "nothing said to the sender");
    assert_eq!(landed(&st), 1, "a contact request of ordinary size is let through at the defaults");
    for audience in [Audience::Chosen, Audience::Groups, Audience::Anyone] {
        set(&st, &target.key, Kind::Message, audience);
        let before = landed(&st);
        request(envelope());
        assert_eq!(landed(&st), before + 1, "a contact request is let through under {audience:?}");
    }
    assert!(block(st.dm_knocks.read()).get(&stranger.key).is_none_or(|(_, n)| *n == 0), "a contact request spends no knock");
    let back = request(sealed(131_072));
    assert_eq!(landed(&st), 4, "the ordinary size limit still holds");
    assert!(back.iter().any(|m| m.contains("exceeds")), "{back:?}");

    set(&st, &target.key, Kind::Message, Audience::Nobody);
    let back = request(envelope());
    assert_eq!(landed(&st), 4, "nobody means no contact requests either");
    assert_eq!(back, vec![format!("reach_refused message {}", target.key)], "with the refusal everyone gets");

    // The accepter replies. The requester keeps the default (messages from friends) and the relay
    // keeps nothing about the request: the requester's pass for the accepter, which the request
    // carried, is what lets the reply in.
    let requesters_pass = pass(&st, &stranger, &target.key, &DEFAULT_MAY);
    let refused = vec![format!("reach_refused message {}", stranger.key)];
    assert_eq!(try_reach(&st, Kind::Message, &target.key, &stranger.key, None), (false, refused), "without the requester's pass the reply is refused");
    let reply = try_reach(&st, Kind::Message, &target.key, &stranger.key, Some(&requesters_pass));
    assert_eq!(reply, (true, Vec::<String>::new()), "a reply carrying the requester's pass is admitted under friends: {reply:?}");
}

/// Five contact requests a day per sender, separate from the 20 knocks: a sender who spent every
/// knock still has five requests, the sixth is refused with a reason, another sender is not
/// affected, and a send the rate limiter slows down spends none (it is paid for after the limiter).
///
/// Seen red 2026-10-09 with `pay` spending the contact request from the knock budget
/// (`spend_knock` in the `ContactRequest` arm): "a sender whose knocks are spent still has five
/// contact requests" (0 landed).
#[test]
fn contact_requests_are_five_a_day_per_sender_and_separate_from_knocks() {
    let st = fresh_state("contact_budget");
    let (sender, open, target, other) = (person(151), person(152), person(153), person(154));
    for p in [&sender, &open, &target, &other] {
        connect(&st, &p.key);
    }
    // The sender spends today's knocks on someone who takes strangers' mail.
    set(&st, &open.key, Kind::Message, Audience::Anyone);
    for _ in 0..DM_KNOCKS_PER_DAY {
        assert!(try_reach(&st, Kind::Message, &sender.key, &open.key, None).0);
    }
    assert!(!try_reach(&st, Kind::Message, &sender.key, &open.key, None).0, "precondition: the knocks are spent");

    let landed = |st: &Arc<RelayState>| st.db.mailbox_fetch(&target.key, 0, 100).unwrap().len();
    let request = |from: &str, reset: bool| -> Vec<String> {
        let mut rx = st.broadcast_tx.subscribe();
        block(async {
            if reset {
                st.rate_limits.write().await.remove(from);
            }
            handle_dm_put(&st, from, target.key.clone(), envelope(), None, true).await;
        });
        heard(&mut rx, from)
    };
    request(&sender.key, true);
    assert_eq!(landed(&st), 1, "a sender whose knocks are spent still has five contact requests");
    // Straight after it, the limiter slows the next one down: nothing is spent.
    let back = request(&sender.key, false);
    assert!(back.iter().any(|m| m.contains("Slow down")), "precondition: slowed down: {back:?}");
    for _ in 0..(CONTACT_REQUESTS_PER_DAY - 1) {
        request(&sender.key, true);
    }
    assert_eq!(landed(&st) as u32, CONTACT_REQUESTS_PER_DAY, "five a day, the slowed-down one not counted");
    let back = request(&sender.key, true);
    assert_eq!(landed(&st) as u32, CONTACT_REQUESTS_PER_DAY, "the sixth is refused");
    assert!(back.iter().any(|m| m.contains("contact requests")), "with the reason: {back:?}");
    request(&other.key, true);
    assert_eq!(landed(&st) as u32, CONTACT_REQUESTS_PER_DAY + 1, "another sender has their own five");
}

/// A refused call gets nothing back at all, whatever the callee's presence: a stranger's ring to
/// someone who shows as offline is not told "not online" (a caller who is let in is), nor is one
/// to someone online, and the stranger's own call signals do not reach them. A ring that is let
/// through opens the call: the callee's accept reaches the caller although the caller's own
/// default (calls from people they choose) would refuse the callee, and offers, answers and
/// candidates pass both ways, until a hangup closes it.
///
/// Seen red 2026-10-09 with the gate in `handle_voice_call` moved after the presence check:
/// "a refused ring to someone shown offline hears nothing: [\"User is not online.\"]".
#[test]
fn a_refused_call_hears_nothing_and_a_call_let_through_flows_both_ways() {
    let st = fresh_state("calls");
    let (callee, caller, stranger) = (person(161), person(162), person(163));
    st.db.join_server(&callee.key, "Callee").unwrap();
    st.db.set_hide_presence(&callee.key, false).unwrap(); // shown, and not connected yet
    let call_pass = pass(&st, &callee, &caller.key, &["call"]);
    connect(&st, &caller.key);
    connect(&st, &stranger.key);

    let send = |from: &str, to: &str, what: &str, cert: Option<&str>| -> (Vec<String>, Vec<String>) {
        let mut rx = st.broadcast_tx.subscribe();
        let mut rx_to = st.broadcast_tx.subscribe();
        if let Some(signal) = what.strip_prefix("signal ") {
            block(handle_webrtc_signal(&st, from, to.to_string(), signal.to_string(), serde_json::json!({ "sdp": "x" }), cert.map(str::to_string)));
        } else {
            block(handle_voice_call(&st, from, to.to_string(), what.to_string(), cert.map(str::to_string)));
        }
        (heard(&mut rx, from), heard(&mut rx_to, to))
    };

    assert_eq!(send(&stranger.key, &callee.key, "ring", None).0, Vec::<String>::new(), "a refused ring to someone shown offline hears nothing");
    assert_eq!(send(&caller.key, &callee.key, "ring", Some(&call_pass)).0, vec!["User is not online.".to_string()], "a caller who is let in is told");

    connect(&st, &callee.key);
    for what in ["ring", "signal offer", "signal ice", "hangup"] {
        let (back, reached) = send(&stranger.key, &callee.key, what, None);
        assert!(back.is_empty() && reached.is_empty(), "a stranger's {what}: heard {back:?}, reached {reached:?}");
    }

    let (_, reached) = send(&caller.key, &callee.key, "ring", Some(&call_pass));
    assert_eq!(reached, vec!["voice_call ring".to_string()]);
    for (from, to, what, arrives) in [
        (&callee.key, &caller.key, "accept", "voice_call accept"),
        (&caller.key, &callee.key, "signal offer", "webrtc_signal offer"),
        (&callee.key, &caller.key, "signal answer", "webrtc_signal answer"),
        (&caller.key, &callee.key, "signal ice", "webrtc_signal ice"),
        (&callee.key, &caller.key, "signal ice", "webrtc_signal ice"),
        (&callee.key, &caller.key, "hangup", "voice_call hangup"),
    ] {
        assert_eq!(send(from, to, what, None).1, vec![arrives.to_string()], "within the call: {what}");
    }
    // Hung up: the call is closed, and neither side's late signals reach the other.
    assert!(send(&caller.key, &callee.key, "signal ice", None).1.is_empty(), "after the hangup the caller's signals stop");
    assert!(send(&callee.key, &caller.key, "signal ice", None).1.is_empty(), "and the callee's");
}

/// A direct-connection offer (it hands over a network address) reaches the target only from
/// their own key (their other devices), a holder of a valid pass from them, someone sharing a P2P
/// group with them, or someone in the same voice room; anyone else's is dropped silently, with
/// nothing back. Whatever the target's settings say: `anyone` for messages and calls does not open
/// it to strangers. An offer's answer and candidates are not gated (they answer an offer).
///
/// Seen red 2026-10-09 with the `dc_offer` arm of `signal_may_pass` answering true (no gate, as
/// before step B): "a stranger's direct-connection offer is dropped: heard [], reached
/// [\"webrtc_signal dc_offer\"]".
#[test]
fn a_direct_connection_offer_only_from_someone_the_target_knows() {
    let st = fresh_state("dc_offer");
    let (target, stranger, friend, mate, roomie) = (person(171), person(172), person(173), person(174), person(175));
    for p in [&target, &stranger, &friend, &mate, &roomie] {
        connect(&st, &p.key);
    }
    for kind in Kind::ALL {
        set(&st, &target.key, kind, Audience::Anyone);
    }
    group(&st, &target, &[&mate], "Choir");
    let friend_pass_cert = pass(&st, &target, &friend.key, &DEFAULT_MAY);
    let room = VoiceRoom { name: "Lounge".into(), participants: vec![(target.key.clone(), "T".into()), (roomie.key.clone(), "R".into())] };
    block(async { st.voice_rooms.write().await.insert("lounge".into(), room) });

    let offer = |from: &str, signal: &str, cert: Option<&str>| -> (Vec<String>, Vec<String>) {
        let mut rx = st.broadcast_tx.subscribe();
        let mut rx_to = st.broadcast_tx.subscribe();
        block(handle_webrtc_signal(&st, from, target.key.clone(), signal.to_string(), serde_json::json!("{}"), cert.map(str::to_string)));
        (heard(&mut rx, from), heard(&mut rx_to, &target.key))
    };
    let (back, reached) = offer(&stranger.key, "dc_offer", None);
    assert!(back.is_empty() && reached.is_empty(), "a stranger's direct-connection offer is dropped: heard {back:?}, reached {reached:?}");
    assert!(offer(&stranger.key, "dc_offer", Some("Zm9yZ2Vk")).1.is_empty(), "a forged pass counts as none");
    let through = vec!["webrtc_signal dc_offer".to_string()];
    assert_eq!(offer(&friend.key, "dc_offer", Some(&friend_pass_cert)).1, through, "from a friend");
    assert_eq!(offer(&mate.key, "dc_offer", None).1, through, "from a group mate");
    assert_eq!(offer(&roomie.key, "dc_offer", None).1, through, "from someone in the same voice room");
    assert_eq!(offer(&target.key, "dc_offer", None).1, through, "from their own other device");
    block(async { st.voice_rooms.write().await.clear() });
    assert!(offer(&roomie.key, "dc_offer", None).1.is_empty(), "the room is left: a stranger again");
    assert_eq!(offer(&stranger.key, "dc_answer", None).1, vec!["webrtc_signal dc_answer".to_string()], "an answer is not gated");
}

/// `reach_set` saves any subset of the kinds and is answered with `reach_settings`, every kind
/// filled in, to the person's own sockets only. An unknown kind (`invite` is a word in a pass but
/// not a setting yet) or an unknown audience refuses the whole set with a notice and changes
/// nothing, and is still answered with the settings as they stand. A set from one person never
/// touches another's.
///
/// Seen red 2026-10-09 with `parse_settings` skipping a word it does not know instead of refusing
/// the set: "an unknown word changes nothing: message was saved as Anyone for
/// {\"call\":\"everyone\",\"message\":\"anyone\"}".
#[test]
fn reach_set_saves_a_subset_refuses_unknown_words_and_always_answers() {
    let st = fresh_state("reach_set");
    let (me, them) = (person(181), person(182));
    let set_and_hear = |raw: serde_json::Value| -> (Vec<ReachSettings>, Vec<String>) {
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_reach_set(&st, &me.key, &raw));
        let (mut answers, mut notices) = (Vec::new(), Vec::new());
        while let Ok(m) = rx.try_recv() {
            match m {
                RelayMessage::ReachSettings { to, settings } => {
                    assert_eq!(to, me.key, "the settings go to the person's own sockets only");
                    answers.push(settings);
                }
                RelayMessage::Private { to, message } if to == me.key => notices.push(message),
                _ => {}
            }
        }
        (answers, notices)
    };
    let defaults = ReachSettings { message: "friends".into(), call: "chosen".into(), trade: "friends".into() };

    let (answers, notices) = set_and_hear(serde_json::json!({ "type": "reach_set", "settings": { "call": "friends" } }));
    assert_eq!(answers, vec![ReachSettings { call: "friends".into(), ..defaults.clone() }], "{notices:?}");
    assert!(notices.is_empty());
    assert_eq!(audience(&st, &me.key, Kind::Call), Audience::Friends);

    let (answers, _) = set_and_hear(serde_json::json!({ "type": "reach_set", "settings": {} }));
    assert_eq!(answers.len(), 1, "an empty set is answered too");

    for bad in [
        serde_json::json!({ "message": "anyone", "call": "everyone" }),
        serde_json::json!({ "message": "anyone", "invite": "friends" }),
        serde_json::json!({ "message": "anyone", "trade": 3 }),
        serde_json::json!("anyone"),
    ] {
        let (answers, notices) = set_and_hear(serde_json::json!({ "type": "reach_set", "settings": bad }));
        assert_eq!(audience(&st, &me.key, Kind::Message), Audience::Friends, "an unknown word changes nothing: message was saved as {:?} for {bad}", audience(&st, &me.key, Kind::Message));
        assert!(notices.iter().any(|n| n.starts_with("Who can reach you was not changed")), "{bad}: {notices:?}");
        assert_eq!(answers, vec![ReachSettings { call: "friends".into(), ..defaults.clone() }], "{bad}: still answered");
    }
    let (_, notices) = set_and_hear(serde_json::json!({ "type": "reach_set" }));
    assert!(notices.iter().any(|n| n.contains("missing")), "{notices:?}");

    let (answers, _) = set_and_hear(serde_json::json!({ "type": "reach_set", "settings": { "message": "anyone", "call": "nobody", "trade": "groups" } }));
    assert_eq!(answers, vec![ReachSettings { message: "anyone".into(), call: "nobody".into(), trade: "groups".into() }]);
    assert_eq!(settings_of(&st, &them.key), defaults, "another person is untouched");
}

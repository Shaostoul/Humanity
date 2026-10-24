//! The "$0 federation test": two complete relays in one process, on real
//! localhost sockets, proving the whole peering loop that was dormant for
//! months. Before the 2026-08-13 repair this test CANNOT pass: the inbound
//! hello was eaten by the identify gate, the welcome was misrouted to local
//! chat clients, both signature preimages mismatched their verifiers, and a
//! URL-added peer could never be matched against a key-identified hello.
//! Six independent defects, each alone fatal to the handshake.
//!
//! Run: cargo test --features relay --no-default-features --test federation_two_relays
//! (also compiles under --features native, which includes relay).
#![cfg(feature = "relay")]

use std::sync::Arc;
use std::time::Duration;

use humanity_engine::relay::handlers::federation;
use humanity_engine::relay::relay::RelayState;
use humanity_engine::relay::storage::Storage;
use humanity_engine::relay::serve_relay;

/// Spin up one complete relay on an ephemeral localhost port.
async fn spawn_relay(dir: &std::path::Path) -> (Arc<RelayState>, u16) {
    std::fs::create_dir_all(dir).expect("test db dir");
    let db = Storage::open(&dir.join("relay.db")).expect("open test db");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let port = listener.local_addr().expect("addr").port();
    let state = serve_relay(db, listener).await;
    (state, port)
}

/// Make `a` trust `b` at tier 2 with b's key pinned, the exact row shape
/// /server-add + the server-info discovery fetch produce in production
/// (URL as the row id, peer key pinned by discovery).
fn trust(a: &Arc<RelayState>, b: &Arc<RelayState>, b_port: u16) {
    let b_url = format!("http://127.0.0.1:{b_port}");
    let (b_pk, _) = b.db.get_or_create_server_keypair().expect("b keypair");
    a.db.add_federated_server(&b_url, "peer", &b_url).expect("add row");
    a.db.set_server_trust_tier(&b_url, 2).expect("trust tier");
    a.db.update_federated_server_info(&b_url, "peer", Some(&b_pk), false)
        .expect("pin key");
}

/// Poll until `check` passes or the deadline expires; federation is
/// asynchronous end to end, so every assertion is an "eventually".
async fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..100 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out waiting for: {what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn two_relays_handshake_chat_and_gossip() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init();
    let base = std::env::temp_dir().join(format!("hos_fed_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (a, _a_port) = spawn_relay(&base.join("a")).await;
    let (b, b_port) = spawn_relay(&base.join("b")).await;
    let (a_pk, _) = a.db.get_or_create_server_keypair().expect("a keypair");

    // Mutual operator trust, keys pinned (the /server-add + discovery shape).
    trust(&a, &b, b_port);
    // B trusts A too; A's row on B is keyed by A's url, key pinned, so the
    // inbound hello (which self-identifies by KEY) must match via the pin.
    {
        let a_url = "http://127.0.0.1:1"; // placeholder url: inbound match goes via the pinned key
        b.db.add_federated_server(a_url, "peer-a", a_url).expect("add row");
        b.db.set_server_trust_tier(a_url, 2).expect("trust tier");
        b.db.update_federated_server_info(a_url, "peer-a", Some(&a_pk), false)
            .expect("pin key");
    }

    // A dials B. Inbound side (B) must verify the hello THROUGH the identify
    // gate and register the peer; outbound side (A) must receive the welcome
    // on its own socket.
    let started = federation::start_federation_connections(&a).await;
    assert_eq!(started, 1, "one outbound connection should start");

    let b_for_conns = b.clone();
    eventually("B registers A as an inbound federation peer", || {
        b_for_conns.federation_connections
            .try_read()
            .map(|c| c.contains_key(&a_pk))
            .unwrap_or(false)
    })
    .await;

    // Federated chat: a message on A's federated channel must arrive at B,
    // pass B's signature verification, and be persisted there.
    // Create the channel rows first: set_channel_federated is an UPDATE and
    // returns Ok(false) on a missing row, which a bare .expect() happily
    // swallows (the check-that-cannot-fail class, caught live in this test).
    let _ = a.db.create_channel("general", "General", None, "test", false);
    let _ = b.db.create_channel("general", "General", None, "test", false);
    assert!(a.db.set_channel_federated("general", true).expect("federate a:general"), "channel row must exist on A");
    assert!(b.db.set_channel_federated("general", true).expect("federate b:general"), "channel row must exist on B");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    // The author's key travels with the line since 2026-10-22 (a server refuses the words of a
    // key it, or a server sharing the room, banned); any stand-in key will do here.
    federation::forward_to_federation(&a, "general", "alice-key", "Alice", "hello across servers", now_ms).await;

    let b_for_chat = b.clone();
    eventually("B persists the federated chat line", || {
        b_for_chat.db.load_recent_messages(50)
            .map(|msgs| {
                msgs.iter().any(|m| matches!(m,
                    humanity_engine::relay::relay::RelayMessage::FederatedChat { content, .. }
                        if content == "hello across servers"))
            })
            .unwrap_or(false)
    })
    .await;

    // THE REVERSE DIRECTION: chat from B must land on A, which receives it
    // on its OUTBOUND socket. This is the leg the first live test failed
    // (2026-08-14): the outbound connection was registered under the URL it
    // dialed while messages identify their origin by PUBLIC KEY, so the
    // source-identity check dropped every message arriving on an outbound
    // socket. Both directions must key peers by their pinned key.
    federation::forward_to_federation(&b, "general", "bela-key", "Bela", "hello back the other way", now_ms + 5).await;
    let a_for_chat = a.clone();
    eventually("A persists the federated chat line from B (outbound-socket leg)", || {
        a_for_chat.db.load_recent_messages(50)
            .map(|msgs| {
                msgs.iter().any(|m| matches!(m,
                    humanity_engine::relay::relay::RelayMessage::FederatedChat { content, .. }
                        if content == "hello back the other way"))
            })
            .unwrap_or(false)
    })
    .await;

    // Profile gossip: a self-certifying copy, signed by its owner's app
    // (core::profile_copy, BUG-191), sent by A must be cached by B; an
    // UNSIGNED one must be refused (the old code accepted empty signatures,
    // which let anyone overwrite any profile). Links are "{}": a copy's links
    // are always a JSON object (a blank stored value is read as "{}").
    use humanity_engine::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    use humanity_engine::relay::core::profile_copy::{self, PublicCopy};
    let seed = [21u8; 32];
    let user_pk_hex = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key());
    let ts = now_ms + 1;
    let copy = PublicCopy {
        name: "Nova".to_string(),
        bio: "explorer".to_string(),
        socials: "{}".to_string(),
        ..PublicCopy::default()
    };
    let sig_hex = profile_copy::sign_copy(&seed, &user_pk_hex, &copy, ts);
    let signed = serde_json::json!({
        "type": "profile_gossip",
        "public_key": user_pk_hex, "name": "Nova", "bio": "explorer",
        "avatar_url": "", "banner_url": "", "socials": "{}", "pronouns": "",
        "location": "", "website": "", "timestamp": ts, "signature": sig_hex,
    });
    let unsigned = serde_json::json!({
        "type": "profile_gossip",
        "public_key": "deadbeef", "name": "Imposter", "bio": "",
        "avatar_url": "", "banner_url": "", "socials": "", "pronouns": "",
        "location": "", "website": "", "timestamp": ts, "signature": "",
    });
    {
        let conns = a.federation_connections.read().await;
        let peer = conns.values().next().expect("A has its outbound peer");
        peer.tx.send(signed.to_string()).expect("send signed gossip");
        peer.tx.send(unsigned.to_string()).expect("send unsigned gossip");
    }

    let b_for_profile = b.clone();
    eventually("B caches the signed profile", || {
        b_for_profile.db
            .get_signed_profile(&user_pk_hex)
            .map(|p| p.is_some())
            .unwrap_or(false)
    })
    .await;
    // The unsigned one must never appear, no matter how long we wait a beat.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        b.db.get_signed_profile("deadbeef")
            .expect("query")
            .is_none(),
        "unsigned profile gossip must be refused, not cached"
    );

    let _ = std::fs::remove_dir_all(&base);
}

// ── A person's profile, saved by their app on one server, on the servers it is linked with ──
// (BUG-191 Wave 2, docs/design/bug-191-profile-copies-plan.md 5 and 9.) Everything here goes
// through the real `/ws` socket a person's app uses, the identify handshake included, and the
// person's side is played the way both apps do it (src/gui/pages/own_profile.rs `take_own_copy`,
// web/chat/chat-profile.js `signProfileCopy`): read the own copy that asks, sign the public copy
// of exactly that with `core::profile_copy`, send `profile_sign`.

use humanity_engine::relay::core::profile_copy::{
    is_withdrawal, next_copy_time, public_copy, sign_copy, verify_copy, withdrawal,
};
use humanity_engine::relay::storage::SignedProfileRecord;
use serde_json::{json, Value};

type Sock = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

/// A person's key as their app has it: the Dilithium3 key of the seed `[seed; 32]`.
fn key_of(seed: u8) -> String {
    use humanity_engine::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32])).public_key())
}

/// `dialer` dials `dialed`, each trusting the other with its key pinned (the /server-add shape;
/// the dialed side's row has a placeholder address, since the hello it receives names the dialer
/// by KEY), and both sides have the link registered before this returns, so nothing either sends
/// next can miss it.
async fn link(dialer: &Arc<RelayState>, dialed: &Arc<RelayState>, dialed_port: u16, placeholder: &str) {
    trust(dialer, dialed, dialed_port);
    let (dialer_pk, _) = dialer.db.get_or_create_server_keypair().expect("dialer keypair");
    let (dialed_pk, _) = dialed.db.get_or_create_server_keypair().expect("dialed keypair");
    dialed.db.add_federated_server(placeholder, "peer", placeholder).expect("add row");
    dialed.db.set_server_trust_tier(placeholder, 2).expect("trust tier");
    dialed.db.update_federated_server_info(placeholder, "peer", Some(&dialer_pk), false).expect("pin key");
    assert_eq!(federation::start_federation_connections(dialer).await, 1, "one outbound connection");
    let (d1, d2) = (dialed.clone(), dialer.clone());
    eventually("the dialed server registers the link", || {
        d1.federation_connections.try_read().map(|c| c.contains_key(&dialer_pk)).unwrap_or(false)
    })
    .await;
    eventually("the dialer registers the link", || {
        d2.federation_connections.try_read().map(|c| c.contains_key(&dialed_pk)).unwrap_or(false)
    })
    .await;
}

/// Sign in to the relay on `port` as `name`, with the key of `[seed; 32]`, through `/ws` and the
/// two-phase identify handshake (as src/relay/features.rs `bind_socket` does).
async fn sign_in(port: u16, seed: u8, name: &str) -> Sock {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use futures::{SinkExt, StreamExt};
    use humanity_engine::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
    use tokio_tungstenite::tungstenite::Message as WsMsg;
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    let pubkey = hex::encode(dil.public_key());
    let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws")).await.expect("connects to /ws");
    let identify = json!({ "type": "identify", "public_key": pubkey, "display_name": name });
    sock.send(WsMsg::Text(identify.to_string().into())).await.unwrap();
    let challenge: Value = serde_json::from_str(&sock.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(challenge["type"], "identify_challenge", "expected a challenge: {challenge}");
    let nonce = challenge["nonce"].as_str().unwrap();
    let sig = B64.encode(dil.sign(format!("hum/identify/v1\n{nonce}\n{pubkey}").as_bytes()));
    sock.send(WsMsg::Text(json!({ "type": "identify_response", "sig_b64": sig }).to_string().into())).await.unwrap();
    sock
}

async fn send_json(sock: &mut Sock, frame: Value) {
    use futures::SinkExt;
    sock.send(tokio_tungstenite::tungstenite::Message::Text(frame.to_string().into())).await.unwrap();
}

/// The next frame on `sock` that `want` picks, reading past the rest (a freshly signed-in socket
/// is sent peers, channels and history first). Panics after 10 s, naming `what`.
async fn frame_where(sock: &mut Sock, what: &str, want: impl Fn(&Value) -> bool) -> Value {
    use futures::StreamExt;
    let found = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(Ok(msg)) = sock.next().await {
            if let Ok(text) = msg.into_text() {
                if let Ok(frame) = serde_json::from_str::<Value>(&text) {
                    if want(&frame) {
                        return Some(frame);
                    }
                }
            }
        }
        None
    })
    .await;
    found.ok().flatten().unwrap_or_else(|| panic!("never came: {what}"))
}

/// A save from the person's app, on a NEW socket (the relay allows one profile update per 30 s per
/// socket): it signs in, sends `profile_update` with `fields`, reads its own copy that asks
/// (`sign_now`), signs the public copy of exactly that, dated after the copy the server holds and
/// after `last_here` (the last one it signed there), and sends `profile_sign`. Returns the socket
/// and the copy's time.
async fn save_and_sign(port: u16, seed: u8, name: &str, fields: Value, last_here: u64) -> (Sock, u64) {
    let mut sock = sign_in(port, seed, name).await;
    let mut update = json!({ "type": "profile_update", "socials": "{}" });
    for (k, v) in fields.as_object().expect("fields") {
        update[k] = v.clone();
    }
    send_json(&mut sock, update).await;
    let own = frame_where(&mut sock, "the own copy of the save, asking to be signed", |f| {
        f["type"] == "profile_data" && f["sign_now"] == true
    })
    .await;
    let s = |k: &str| own[k].as_str().unwrap_or("").to_string();
    let copy = public_copy(&s("name"), &s("bio"), &s("avatar_url"), &s("banner_url"), &s("socials"), &s("pronouns"), &s("location"), &s("website"), &s("privacy"));
    let at = next_copy_time(now_ms(), own["signed_at"].as_u64().unwrap_or(0), last_here);
    let signature = sign_copy(&[seed; 32], &key_of(seed), &copy, at);
    send_json(&mut sock, json!({ "type": "profile_sign", "at": at, "signature": signature })).await;
    (sock, at)
}

/// The copy `state` holds for `key` once `check` passes, waiting up to 10 s; panics with `fail`.
async fn copy_on(state: &Arc<RelayState>, key: &str, fail: &str, check: impl Fn(&SignedProfileRecord) -> bool) -> SignedProfileRecord {
    for _ in 0..100 {
        if let Some(held) = state.db.get_signed_profile(key).expect("read the copy").filter(|h| check(h)) {
            return held;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{fail}");
}

/// `GET /api/profile/{key}` on the relay at `port`: the HTTP status.
async fn lookup(port: u16, key: &str) -> u16 {
    reqwest::get(format!("http://127.0.0.1:{port}/api/profile/{key}")).await.expect("the lookup answers").status().as_u16()
}

/// A SAVED PROFILE REACHES THE OTHER SERVER WITH ONLY WHAT ANYONE MAY SEE (BUG-191 Wave 2A), on
/// three real relays over real sockets. Ann (seed 7) signs in on A, saves with her location kept
/// for friends, and her app signs the copy her own copy asks for: B keeps her bio and pronouns with
/// her location blank, and the signature checks on B. Then, each save on a new socket:
/// - her location made public: B shows "Lisbon";
/// - kept for friends again: B's copy no longer holds it;
/// - unlisted: B holds the withdrawal, and the lookup answers 404 on B and on A;
/// - listed again with a new bio (so the last leg has a copy to carry: a withdrawal reaching a
///   server that never held a copy leaves no trace there, which a test cannot tell from nothing).
///
/// A third relay C, linked to A only after all that, gets Ann's current copy by the catch-up, and
/// it checks as hers.
///
/// Seen red 2026-10-12 with `gossip_profile` taken out of `handle_profile_sign`: "B never had
/// Ann's profile". And with `send_own_copies` not called (both calls, `run_inbound_peer` and the
/// welcome, taken out): "C never heard Ann's copy".
#[tokio::test(flavor = "multi_thread")]
async fn a_saved_profile_reaches_the_other_server_with_only_what_anyone_may_see() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    let base = std::env::temp_dir().join(format!("hos_fed_profile_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (a, a_port) = spawn_relay(&base.join("a")).await;
    let (b, b_port) = spawn_relay(&base.join("b")).await;
    link(&a, &b, b_port, "http://127.0.0.1:1").await;
    let ann = key_of(7);
    let kept = r#"{"location":"private"}"#;
    let profile = |bio: &str, privacy: &str| json!({ "bio": bio, "pronouns": "she/her", "location": "Lisbon", "privacy": privacy });

    let (mut sock, at1) = save_and_sign(a_port, 7, "Ann", profile("Keeps bees.", kept), 0).await;
    let held = copy_on(&b, &ann, "B never had Ann's profile", |h| h.timestamp == at1).await;
    assert_eq!((held.name.as_str(), held.bio.as_str(), held.pronouns.as_str()), ("Ann", "Keeps bees.", "she/her"));
    assert_eq!(held.location, "", "her location, kept for friends, reached B");
    assert!(verify_copy(&ann, &held.copy(), held.timestamp, &held.signature), "the signature checks on B");
    let _ = sock.close(None).await;

    let (mut sock, at2) = save_and_sign(a_port, 7, "Ann", profile("Keeps bees.", "{}"), at1).await;
    let held = copy_on(&b, &ann, "B never had Ann's public location", |h| h.timestamp == at2).await;
    assert_eq!(held.location, "Lisbon", "made public, B shows it");
    let _ = sock.close(None).await;

    let (mut sock, at3) = save_and_sign(a_port, 7, "Ann", profile("Keeps bees.", kept), at2).await;
    let held = copy_on(&b, &ann, "B never had the copy without her location", |h| h.timestamp == at3).await;
    assert_eq!(held.location, "", "kept for friends again: gone from B");
    let _ = sock.close(None).await;

    let unlisted = r#"{"directory":"unlisted","location":"private"}"#;
    let (mut sock, at4) = save_and_sign(a_port, 7, "Ann", profile("Keeps bees.", unlisted), at3).await;
    let held = copy_on(&b, &ann, "B never had Ann's withdrawal", |h| h.timestamp == at4).await;
    assert!(is_withdrawal(&held.copy()) && held.copy() == withdrawal(), "B holds the withdrawal: {held:?}");
    assert!(verify_copy(&ann, &held.copy(), held.timestamp, &held.signature), "signed by her key");
    assert_eq!(lookup(b_port, &ann).await, 404, "unlisted: B's lookup answers not found");
    assert_eq!(lookup(a_port, &ann).await, 404, "unlisted: A's lookup answers not found");
    let _ = sock.close(None).await;

    let (mut sock, at5) = save_and_sign(a_port, 7, "Ann", profile("Back again.", kept), at4).await;
    copy_on(&b, &ann, "B never had Ann's copy listed again", |h| h.timestamp == at5).await;
    assert_eq!(lookup(b_port, &ann).await, 200, "listed again: B answers");
    let _ = sock.close(None).await;

    // A third server, linked only now: it dials A, and A's catch-up sends it Ann's current copy.
    let (c, _c_port) = spawn_relay(&base.join("c")).await;
    link(&c, &a, a_port, "http://127.0.0.1:2").await;
    let held = copy_on(&c, &ann, "C never heard Ann's copy", |h| h.timestamp == at5).await;
    assert_eq!((held.bio.as_str(), held.location.as_str()), ("Back again.", ""), "her current copy, nothing kept for friends in it");
    assert!(verify_copy(&ann, &held.copy(), held.timestamp, &held.signature), "and it checks on C");

    let _ = std::fs::remove_dir_all(&base);
}

/// ERASING AN ACCOUNT WITHDRAWS ITS COPY ON THE LINKED SERVER (BUG-191 Wave 2B, plan section 9,
/// decision 2). Boris saves a listed profile on A and his app signs its copy, which reaches B.
/// Then he erases his account on A, and his app sends the withdrawal it signed with the erase. A
/// passes it on before it erases: B holds the withdrawal, signed by his key, and its lookup
/// answers 404; on A the account and the copy are gone.
///
/// Seen red 2026-10-12 with the withdrawal not passed on before the erase (the `withdraw_on_erase`
/// call taken out of `handle_account_delete`): "B still holds Boris's profile after his erase";
/// and the same with that call moved after the erase (`carry_out`).
#[tokio::test(flavor = "multi_thread")]
async fn erasing_an_account_withdraws_its_copy_on_the_linked_server() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    let base = std::env::temp_dir().join(format!("hos_fed_erase_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (a, a_port) = spawn_relay(&base.join("a")).await;
    let (b, b_port) = spawn_relay(&base.join("b")).await;
    link(&a, &b, b_port, "http://127.0.0.1:1").await;
    let boris = key_of(8);

    let (mut sock, at) = save_and_sign(a_port, 8, "Boris", json!({ "bio": "Fixes bikes.", "location": "Porto" }), 0).await;
    copy_on(&b, &boris, "test setup: B never had Boris's profile", |h| h.timestamp == at).await;
    assert_eq!(lookup(b_port, &boris).await, 200, "test setup: B answers");

    // What his app sends: the typed name and the withdrawal, dated after the copy A holds.
    let w_at = next_copy_time(now_ms(), at, at);
    let signature = sign_copy(&[8; 32], &boris, &withdrawal(), w_at);
    send_json(&mut sock, json!({ "type": "account_delete", "confirm_name": "Boris", "withdrawal": { "at": w_at, "signature": signature } })).await;
    let erased = frame_where(&mut sock, "the erase's answer", |f| f["type"] == "account_erased").await;
    assert_eq!(erased["partial"], false, "the erase went through: {erased}");

    let held = copy_on(&b, &boris, "B still holds Boris's profile after his erase", |h| is_withdrawal(&h.copy())).await;
    assert_eq!((held.copy(), held.timestamp), (withdrawal(), w_at));
    assert!(verify_copy(&boris, &held.copy(), held.timestamp, &held.signature), "signed by his key");
    assert_eq!(lookup(b_port, &boris).await, 404, "B's lookup answers not found");
    assert_eq!(a.db.name_for_key(&boris).expect("read"), None, "his account on A is erased");
    assert!(a.db.get_signed_profile(&boris).expect("read").is_none(), "and the copy A held with it");

    let _ = std::fs::remove_dir_all(&base);
}

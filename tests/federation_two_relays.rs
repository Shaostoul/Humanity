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
    federation::forward_to_federation(&a, "general", "Alice", "hello across servers", now_ms).await;

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
    federation::forward_to_federation(&b, "general", "Bela", "hello back the other way", now_ms + 5).await;
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

    // Profile gossip: a self-certifying (Dilithium3-signed) profile sent by
    // A must be cached by B; an UNSIGNED one must be refused (the old code
    // accepted empty signatures, which let anyone overwrite any profile).
    use humanity_engine::relay::core::pq_crypto::DilithiumKeypair;
    let user = DilithiumKeypair::generate().expect("user keypair");
    let user_pk_hex = hex::encode(user.public_key());
    let ts = now_ms + 1;
    let canonical = federation::canonical_profile_message(
        &user_pk_hex, "Nova", "explorer", "", "", "", "", "", "", ts,
    );
    let sig_hex = hex::encode(user.sign(canonical.as_bytes()));
    let signed = serde_json::json!({
        "type": "profile_gossip",
        "public_key": user_pk_hex, "name": "Nova", "bio": "explorer",
        "avatar_url": "", "banner_url": "", "socials": "", "pronouns": "",
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

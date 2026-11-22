// The owner's own way into the node this app hosts, and the forwarders a node with a public address
// may sit behind (2026-11-21, the review of the signed requests' round 5, M1 and M2).
//
// M2: round 5 took the loopback names off a node with a public address, but "Add to my servers"
// still saved http://127.0.0.1:{port}, so the owner's sign-in and every signed request his app made
// there were refused in words that gave no cause. Now every node this page starts opens its
// owner's door (127.0.0.1 at the next port, relay OWNER_PORT), which takes this app's key alone,
// and the app saves and shows that. Proved end to end: the names the page hands the relay, a real
// relay with its door, the address "Add to my servers" saves, the app's own sign-in answer and its
// own signed channel read through that address.
//
// M1: a node with a public address listed its LAN address too, so a forwarder on the network that
// adds no header (Tor on a router, a Pi or a NAS; Tor on this computer pointed at its own LAN
// address; a tunnel agent on another device; a Whonix gateway) made every visitor local, and a
// proof a friend's app made for that LAN address (an attacker in a cafe answering at it) was taken
// through the forwarder. Now the LAN address is listed beside a public one only when the owner
// ticks "Also answer on my local network", which the page says risks exactly this.

use super::*;
use crate::relay::audience::{Arrival, OwnNames, OWNER_PORT_ENV, PUBLIC_HOSTS_ENV, REFUSED_AS_REACHED};

const ONION: &str = "abcdefghijklmnop.onion";

/// M1, each forwarder kind the review named: a proof made for the node's LAN address, brought in by
/// a forwarder that adds no header from the forwarder's own address, is refused when the node has a
/// public address and the owner did not ask to answer on the local network; with the box ticked it
/// is taken (the risk the page's words name). The public name is taken through each either way, and
/// a device in the home connecting straight is taken only with the box.
/// Red evidence (2026-11-21, round 5's `node_public_hosts` put back, the LAN address listed
/// whenever the node is on the network): "Tor on this computer, HiddenServicePort pointed at its
/// own LAN address: refused without the box" failed, left Ok("192.168.1.5:3210"), right the one
/// sentence (the first kind; the test stops there).
#[test]
fn a_forwarder_on_the_network_makes_no_visitor_local_unless_the_owner_asks() {
    // (what, the node's LAN address, the forwarder's address)
    let kinds = [
        ("Tor on this computer, HiddenServicePort pointed at its own LAN address", "192.168.1.5", "192.168.1.5"),
        ("Tor on the router", "192.168.1.5", "192.168.1.1"),
        ("Tor on a Raspberry Pi", "192.168.1.5", "192.168.1.3"),
        ("a tunnel agent (frp, ngrok, playit) on a NAS", "192.168.1.5", "192.168.1.10"),
        ("a Whonix gateway in front of the workstation", "10.152.152.11", "10.152.152.10"),
    ];
    for (what, lan, forwarder) in kinds {
        let lan_name = format!("{lan}:3210");
        let at = |host: &str, peer: &str| Arrival { host: Some(host.to_string()), peer: Some(peer.parse().unwrap()), ..Default::default() };
        for lan_too in [false, true] {
            let told = node_public_hosts(3210, false, Some(lan), &[ONION.to_string()], lan_too);
            let own = OwnNames::from_settings(|k| (k == PUBLIC_HOSTS_ENV).then(|| told.clone()), "0.0.0.0:3210".parse().unwrap());
            let passed_through = own.admit(&lan_name, &at(&lan_name, forwarder));
            if lan_too {
                assert_eq!(passed_through.as_deref(), Ok(lan_name.as_str()), "{what}: taken with the box (the risk its words name)");
            } else {
                assert_eq!(passed_through, Err(REFUSED_AS_REACHED.to_string()), "{what}: refused without the box");
            }
            assert!(own.admit(ONION, &at(ONION, forwarder)).is_ok(), "{what}: the public name works through it");
            let neighbour = format!("{}.77", lan.rsplit_once('.').unwrap().0);
            assert_eq!(own.admit(&lan_name, &at(&lan_name, &neighbour)).is_ok(), lan_too, "{what}: a device at home, straight, only with the box");
        }
    }
}

/// M2, end to end: a node with an onion address, started from the page's own plan, with its owner's
/// door; the address "Add to my servers" saves is the door; through it the app's own sign-in answer
/// (net/identity.rs `identify_response`) signs in, and its own signed channel read
/// (net/chat_read.rs `fetch_history`) is answered. Round 5's saved address, the node's main port by
/// loopback, is refused in the one sentence; another key through the door is refused; the "On this
/// computer" row shows the same address ([`owner_url`]). Round 7 (2026-11-21): the owner key comes from
/// the page's settings (the app's identity) through the plan's environment, applied as `start`
/// applies it, to the relay's own `from_env`, and the relay opens its door the way `run_relay` does
/// (`serve_on`); before, the test put its own key in and started the door itself. Red evidence
/// (2026-11-21, the plan dropping the owner key it was given): "the owner's app signs in through
/// the address it saved" failed, left "system" (the one sentence).
/// Red evidence (2026-11-21, `owner_url` giving round 5's address, the node's port by loopback):
/// "the owner's app signs in through the address it saved" failed, left "system" (the one sentence),
/// right "peer_list".
#[test]
fn the_owners_app_signs_in_and_reads_through_the_address_it_saved() {
    let run = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
            rt.block_on(owners_connection());
        })
        .unwrap()
        .join();
    if let Err(e) = run {
        std::panic::resume_unwind(e);
    }
}

async fn owners_connection() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let seed = [61u8; 32];
    let key = crate::net::identity::derive_pq_identity(&seed).expect("an identity").dilithium_hex;
    // A main port and, at the next port, the owner's door (a fresh pair until both are free).
    let (main, door, port) = loop {
        let main = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = main.local_addr().unwrap().port();
        if port == u16::MAX {
            continue;
        }
        if let Ok(door) = tokio::net::TcpListener::bind(("127.0.0.1", owner_port_for(port))).await {
            break (main, door, port);
        }
    };
    let mut n = LocalNode::new();
    n.port_input = port.to_string();
    n.db_input = std::env::temp_dir().join("humanity-host-node-owner-test").join("relay.db").display().to_string();
    n.public_input = ONION.to_string();
    // The owner key from the page's settings (the identity the page notes each frame), through the
    // plan's environment, as start() hands it over, to the relay's own reading of it.
    let mut page = GuiState::default();
    page.profile_public_key = key.clone();
    let plan = plan_start_for(&n, Some(page.profile_public_key.clone())).expect("a valid form");
    apply_env(&plan.env);
    let names = crate::relay::audience::from_env(main.local_addr().unwrap());
    apply_env(&plan.env.iter().map(|(k, _)| (*k, None)).collect::<Vec<_>>());
    assert_eq!(names.door_port(), Some(owner_port_for(port)), "the plan opens the owner's door ({OWNER_PORT_ENV})");
    assert!(!names.names().iter().any(|n| n.starts_with("127.0.0.1") || n.starts_with("localhost")), "the main port answers to no loopback name: {:?}", names.names());
    let state = crate::relay::serve_relay_named(crate::relay::storage::Storage::open_temp("owner_door_e2e"), main, Some(door), names).await;
    crate::relay::default_channels::make_missing(&state.db, true);

    // What "Add to my servers" saves for the running node.
    n.running_port = port;
    n.running_name = "Test node".to_string();
    let mut gs = GuiState::default();
    add_to_my_servers(&mut gs, &n);
    let saved = gs.chat_servers.last().expect("the node is in the server list").url.clone();
    assert_eq!(saved, owner_url(port), "the On this computer row shows the address saved");

    // Sign in through it, as the app does: its socket address, its answer to the challenge.
    let sign_in = |url: String, seed: [u8; 32], key: String| async move {
        let (mut sock, _) = tokio_tungstenite::connect_async(crate::gui::pages::chat::derive_ws_url(&url)).await.expect("the node answers");
        let hello = serde_json::json!({ "type": "identify", "public_key": key, "display_name": format!("K{}", humanity_core::text::prefix(&key, 6)) });
        sock.send(Message::Text(hello.to_string().into())).await.unwrap();
        let mut answered = None;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while let Ok(Some(Ok(Message::Text(t)))) = tokio::time::timeout_at(deadline, sock.next()).await {
            let v: serde_json::Value = serde_json::from_str(&t).unwrap_or_default();
            match v["type"].as_str() {
                Some("identify_challenge") => {
                    let proof = crate::net::identity::identify_response(&seed, &url, v["nonce"].as_str().unwrap(), &key).expect("the app signs");
                    sock.send(Message::Text(proof.into())).await.unwrap();
                }
                Some(t @ ("system" | "peer_list")) => {
                    answered = Some((t.to_string(), v["message"].as_str().unwrap_or_default().to_string()));
                    break;
                }
                _ => {}
            }
        }
        answered.expect("an answer to the sign-in")
    };
    let (answer, words) = sign_in(saved.clone(), seed, key.clone()).await;
    assert_eq!(answer, "peer_list", "the owner's app signs in through the address it saved: {words}");

    // A signed channel read through it, the app's own.
    let read = |base: String, seed: [u8; 32], key: String| {
        tokio::task::spawn_blocking(move || {
            let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(10)).build();
            crate::net::chat_read::fetch_history(&agent, &base, Some(&seed), &key, humanity_protocol::chat_access::MAIN_CHANNEL, 5)
        })
    };
    let body = read(saved.clone(), seed, key.clone()).await.unwrap();
    assert!(body.is_ok(), "the owner's signed channel read through the saved address is answered: {body:?}");

    // Round 5's saved address, the main port by loopback: refused, in the one sentence.
    let main_url = format!("http://127.0.0.1:{port}");
    let refused = read(main_url.clone(), seed, key.clone()).await.unwrap();
    assert!(refused.as_ref().is_err_and(|e| e.starts_with("HTTP 401") && e.contains(REFUSED_AS_REACHED)), "round 5's address is refused: {refused:?}");
    let (answer, _) = sign_in(main_url, seed, key.clone()).await;
    assert_eq!(answer, "system", "and so is a sign-in there");

    // Another identity through the door: refused (the owner's key alone).
    let stranger = [62u8; 32];
    let stranger_key = crate::net::identity::derive_pq_identity(&stranger).expect("an identity").dilithium_hex;
    let (answer, words) = sign_in(saved, stranger, stranger_key).await;
    assert_eq!((answer.as_str(), words.as_str()), ("system", REFUSED_AS_REACHED), "another key through the door is refused");
}

/// Round 7 (2026-11-21), a node saved before the upgrade: round 5 saved a network node as
/// http://127.0.0.1:{port}, which it no longer answers to. "Add to my servers" REPLACES that entry in
/// its place with the owner's door (and the server in use follows), never adding the door beside it.
/// Red evidence (2026-11-21, round 6's add_to_my_servers, which only added): "the old entry is
/// replaced, not kept beside the door" failed, left 3 servers.
#[test]
fn add_to_my_servers_replaces_the_nodes_old_loopback_entry() {
    let mut n = LocalNode::new();
    n.running_port = 3210;
    n.running_name = "Home".to_string();
    let mut gs = GuiState::default();
    let server = |url: &str| crate::gui::ChatServer { id: format!("srv_{url}"), name: "x".into(), url: url.into(), connected: false, channels: Vec::new(), voice_channels: Vec::new() };
    gs.chat_servers = vec![server("https://united-humanity.us"), server("http://127.0.0.1:3210")];
    gs.server_url = "http://127.0.0.1:3210".to_string();
    add_to_my_servers(&mut gs, &n);
    let urls: Vec<&str> = gs.chat_servers.iter().map(|s| s.url.as_str()).collect();
    assert_eq!(urls.len(), 2, "the old entry is replaced, not kept beside the door: {urls:?}");
    assert_eq!(urls, ["https://united-humanity.us", "http://127.0.0.1:3211"], "in its place");
    assert_eq!(gs.server_url, "http://127.0.0.1:3211", "the server in use follows");
    add_to_my_servers(&mut gs, &n);
    assert_eq!(gs.chat_servers.len(), 2, "once");
}

/// Round 7 (2026-11-21): "Copy address to share" never offers a network node's loopback port (it
/// does not answer to it); with no local address known it offers none, and the page says so.
/// Red evidence (2026-11-21, round 6's choice, the loopback port when nothing else was known): "a
/// network node with no local address known offers nothing" failed, left
/// Some("http://127.0.0.1:3210").
#[test]
fn copy_address_to_share_never_offers_a_network_nodes_loopback_port() {
    let mut n = LocalNode::new();
    n.running_port = 3210;
    n.running_local_only = false;
    n.lan_ip = None;
    assert_eq!(share_url_for(&n), None, "a network node with no local address known offers nothing");
    n.lan_ip = Some("192.168.1.5".into());
    assert_eq!(share_url_for(&n).as_deref(), Some("http://192.168.1.5:3210"));
    n.running_public = format!("{ONION}, other.example");
    assert_eq!(share_url_for(&n).as_deref(), Some("http://abcdefghijklmnop.onion"), "the public address first");
    let mut local = LocalNode::new();
    local.running_port = 3210;
    local.running_local_only = true;
    assert_eq!(share_url_for(&local).as_deref(), Some("http://127.0.0.1:3210"), "this computer only: its own port");
}

// Child of gui/pages/chat.rs: my own copy of a private message leaves apart from the recipient's
// (2026-11-06, docs/BUGS.md BUG-259; net/self_mail.rs, the website's twin web/shared/self-mail.js).

use super::*;
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use std::time::{Duration, Instant};

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

fn frames(sent: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    sent.try_iter().map(|f| serde_json::from_str(&f).unwrap()).collect()
}

/// A DM TYPED AND SENT: the recipient's copy goes at once, my own copy only after the shortest
/// wait and by the longest, and it is the same signed message sealed to me, so my other devices
/// still get my sent history. This device keeps its own copy at once.
///
/// Seen red 2026-11-06 with `WsClient::send` passing every frame straight on (the old send): "only
/// the recipient's copy goes at once" failed (left: 2 puts, my copy among them).
#[test]
fn a_dm_sends_my_copy_apart_from_the_recipients() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(181);
    let (bob_seed, bob) = identity(182);
    let mut gs = GuiState::default();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    gs.server_url = format!("https://self-mail-{nanos}.example");
    gs.profile_public_key = me.clone();
    gs.user_name = "Me".into();
    gs.private_key_bytes = Some(seed.clone());
    gs.peer_kyber_keys.insert(bob.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&bob_seed).unwrap().public_base64());
    gs.chat_active_channel = format!("dm:{bob}");
    let (client, sent) = crate::net::ws_client::WsClient::recording_holding(&me);
    gs.ws_client = Some(client);

    let t0 = Instant::now();
    assert!(super::send_composed_content(&mut gs, "hello Bob"), "the DM is sent");
    let now: Vec<serde_json::Value> = frames(&sent).into_iter().filter(|f| f["type"] == "dm_put").collect();
    assert_eq!(now.len(), 1, "only the recipient's copy goes at once: {now:?}");
    assert_eq!(now[0]["to"], bob.as_str());
    assert!(
        gs.dm_store.as_ref().is_some_and(|s| s.conversation(&bob).iter().any(|m| m.text == "hello Bob")),
        "this device keeps its own copy at once"
    );

    let client = gs.ws_client.as_ref().unwrap();
    client.release_due_at(t0 + Duration::from_millis(crate::net::self_mail::SELF_MAIL_DELAY_MIN_MS) - Duration::from_secs(1));
    assert!(frames(&sent).is_empty(), "nothing of mine before the shortest wait");
    assert_eq!(client.self_mail_waiting(), 1);
    client.release_due_at(t0 + Duration::from_millis(crate::net::self_mail::SELF_MAIL_DELAY_MAX_MS) + Duration::from_secs(5));
    let later = frames(&sent);
    assert_eq!(later.len(), 1, "my copy goes by the longest wait: {later:?}");
    assert_eq!((later[0]["type"].as_str(), later[0]["to"].as_str()), (Some("dm_put"), Some(me.as_str())));

    // The same signed message, sealed to me: what my other devices open as my sent history.
    let mine = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&seed).unwrap();
    let inner_json = crate::net::dm_pq::open_v2(&mine, later[0]["content"].as_str().unwrap()).expect("it opens with my own key");
    let inner = crate::net::dm_pq::parse_verify_inner(&inner_json).expect("its signature checks");
    assert_eq!((inner.from.as_str(), inner.to.as_str(), inner.text.as_str()), (me.as_str(), bob.as_str(), "hello Bob"));
}

/// Every real connection holds mail to myself (a recording test client holds it only when asked,
/// so the features' own tests see their copies at once). The address is a closed port on this
/// computer: the connection fails at once and nothing listens.
#[test]
fn every_real_connection_holds_mail_to_myself() {
    let client = crate::net::ws_client::WsClient::connect("ws://127.0.0.1:9/ws", "Me", "me_key");
    assert!(client.holds_self_mail());
}

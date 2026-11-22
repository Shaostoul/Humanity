//! The app's and the server's own names are nobody's (the owner branch's seventh review, H1 (b),
//! 2026-11-21): the REAL relay (real router, real `/ws` identify) refuses "System" and its
//! lookalikes with words, in any case, and takes an ordinary name. A member named "System" wrote
//! lines the desktop app drew exactly like its own.
//!
//! Seen red 2026-11-21 by mutation (scratchpad ownfix10-red-rust.log) with the reserved-name block
//! taken out of relay.rs identify: "System" was signed in (the reply was `peer_list`, not `name_taken`).

use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message as WsMsg;

use crate::relay::relay::RelayState;

async fn spawn_relay(tag: &str) -> u16 {
    let state = Arc::new(RelayState::new(crate::relay::storage::Storage::open_temp(&format!("reserved_{tag}"))));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    // Its own names, the loopback ones on its port: a sign-in names the address its socket opened
    // (audience.rs, since the signed requests were merged, 2026-11-22).
    let _ = state.own_names.set(crate::relay::audience::OwnNames::loopback(listener.local_addr().unwrap()));
    let app = crate::relay::build_router(state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await;
    });
    port
}

/// Identify as `name` with the key of `seed`; the first `name_taken` or `peer_list` frame back.
async fn identify(port: u16, seed: [u8; 32], name: &str) -> Value {
    let dil = humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&humanity_protocol::pq_crypto::derive_dilithium_seed(&seed));
    let pubkey = hex::encode(dil.public_key());
    let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws")).await.expect("connects");
    sock.send(WsMsg::Text(serde_json::json!({ "type": "identify", "public_key": pubkey, "display_name": name }).to_string().into())).await.unwrap();
    let challenge: Value = serde_json::from_str(&sock.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(challenge["type"], "identify_challenge", "{challenge}");
    let nonce = challenge["nonce"].as_str().unwrap();
    let proof = crate::relay::audience::test_identify_response(&dil, &format!("127.0.0.1:{port}"), nonce, &pubkey);
    sock.send(WsMsg::Text(proof.to_string().into())).await.unwrap();
    let mut seen = Vec::new();
    let got = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(frame) = sock.next().await {
            let Ok(msg) = frame else { seen.push("an error".to_string()); break };
            if let Ok(text) = msg.into_text() {
                let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if v["type"] == "name_taken" || v["type"] == "peer_list" {
                    return Some(v);
                }
                seen.push(text.chars().take(120).collect());
            }
        }
        None
    })
    .await
    .ok()
    .flatten();
    got.unwrap_or_else(|| serde_json::json!({ "seen": seen }))
}

#[tokio::test]
async fn identify_refuses_the_apps_and_servers_names_with_words() {
    // A relay each: one connection may open only a few new accounts an hour.
    for (i, name) in ["System", "SYSTEM", "Sy5tem", "HumanityOS", "From_this_server"].iter().enumerate() {
        let port = spawn_relay(&format!("identify{i}")).await;
        let got = identify(port, [40 + i as u8; 32], name).await;
        assert_eq!(got["type"], "name_taken", "{name} was refused: {got}");
        assert_eq!(got["message"], crate::relay::handlers::live_conns::RESERVED_NAME_REFUSED, "{name}: the words say why");
    }
    let ok = identify(spawn_relay("identify_ok").await, [60; 32], "Ada").await;
    assert_eq!(ok["type"], "peer_list", "an ordinary name signs in: {ok}");
}

/// THE BOT ROUTE HOLDS A BOT'S NAME TO THE RULE A PERSON'S IS (the owner branch's eighth review): it
/// checked only the name's ASCII letters ("S\u{0443}stem", a Cyrillic u, folded to "Sstem") and then
/// registered the whole name. Every reserved name and every name an honest relay would refuse is
/// refused, with words; an ordinary bot name is taken.
/// Seen red 2026-11-22 by mutation (scratchpad ownfix11-red-relay.log): with the honest-name check
/// taken out (only the reserved one, as before), "S\u{0443}stem" was taken.
#[test]
fn the_bot_route_refuses_what_a_person_could_not_be_named() {
    for bad in ["S\u{0443}stem", "System", "Sy5tem", "S y s t e m", "Heron \u{1fab6}", "a sentence. with spaces", " Lead", "Two  spaces", ""] {
        assert!(crate::relay::api::bot_name_refusal(bad).is_some(), "{bad:?} refused");
    }
    assert_eq!(crate::relay::api::bot_name_refusal("System"), Some(crate::relay::handlers::live_conns::RESERVED_NAME_REFUSED));
    assert_eq!(crate::relay::api::bot_name_refusal("S\u{0443}stem"), Some(crate::relay::api::BOT_NAME_REFUSED));
    assert_eq!(crate::relay::api::bot_name_refusal("Heron"), None, "an ordinary bot name");
    // The bots that exist keep working (the ninth review): the release announcer, whose key the
    // front door reads its cards from, and the server's disk alerts. Seen red 2026-11-22 against
    // round 9's person-name rule: "Deploy Bot" was refused.
    for name in ["Deploy Bot", "Disk Guard"] {
        assert_eq!(crate::relay::api::bot_name_refusal(name), None, "{name:?}, an existing bot, is taken");
    }
}

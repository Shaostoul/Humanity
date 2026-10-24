// Child of key_cards.rs (#[path]): what the relay does with key cards (2026-10-22,
// docs/design/key-cards.md). It keeps a card sent with a sign-in only when it checks out as the
// signed-in person's; it hands cards out to whoever asks, exactly as signed; a card one server
// keeps reaches a linked server, which keeps it by the same rule and serves it, so an app can take
// a person's card from any server; and a card a linked server sends that does not check out is
// refused there too.

use super::*;
use crate::relay::core::key_card::{card_for, card_json, sign_card, sign_card_with, verify_card, KeyCard, Keep};
use crate::relay::core::pq_crypto::{derive_dilithium_seed, derive_kyber_seed, DilithiumKeypair, KyberKeypair};
use crate::relay::storage::{now_millis, Storage};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use std::time::Instant;

fn state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("keycards_{tag}"))))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

fn dil(seed: u8) -> DilithiumKeypair {
    DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]))
}
fn key_of(seed: u8) -> String {
    hex::encode(dil(seed).public_key())
}
fn kyber_of(seed: u8) -> Vec<u8> {
    KyberKeypair::from_seed(&derive_kyber_seed(&[seed; 32])).unwrap().public_key()
}
fn card_of(seed: u8) -> KeyCard {
    sign_card(&[seed; 32], now_millis() - 1_000).unwrap()
}

/// A server this one is linked with: a fake link whose frames land in the returned receiver.
fn link_a_server(st: &Arc<RelayState>, id: &str) -> tokio::sync::mpsc::UnboundedReceiver<String> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let link = FederatedConnection { tx, server_id: id.to_string(), server_name: id.to_string(), trust_tier: 1, connected_at: Instant::now() };
    block(async { st.federation_connections.write().await.insert(id.to_string(), link) });
    rx
}

/// The `key_card_gossip` frames a linked server has received since last asked: the catch-up's
/// sent as they are, and the live ones inside the numbered log's `fed_event` (federation_log.rs).
fn cards_got(rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> Vec<Value> {
    let mut got = Vec::new();
    while let Ok(text) = rx.try_recv() {
        let frame: Value = serde_json::from_str(&text).expect("a frame is JSON");
        let frame = if frame["type"] == "fed_event" { frame["event"].clone() } else { frame };
        if frame["type"] == "key_card_gossip" {
            got.push(frame);
        }
    }
    got
}

/// THE RELAY REJECTS A CARD SIGNED BY THE WRONG KEY. Ann signs in and her app sends a card that
/// names her key but carries Bo's signature; then one Bo signed for himself (a good card, of
/// somebody else); then her own card with somebody else's Kyber key put in; then one with no
/// signature. None of them is kept, nothing is passed on, and Ann is told each time. Her own good
/// card is kept.
///
/// Seen red 2026-10-23 with `key_card::verify_card` passing every card of the right shape: "a card
/// naming Ann's key with Bo's signature was kept" failed (left `Some(New)`, right `None`).
#[test]
fn the_relay_rejects_a_card_signed_by_the_wrong_key() {
    let st = state("wrong_key");
    let mut link = link_a_server(&st, "peer.example");
    let mut told = st.broadcast_tx.subscribe();
    let ann = key_of(7);
    let good = card_of(7);
    let by_bo = KeyCard { sig: B64.encode(dil(9).sign(crate::relay::core::key_card::card_text(&good.key, &good.kyber, good.at).as_bytes())), ..good.clone() };
    let bos_own = card_of(9);
    let swapped = KeyCard { kyber: B64.encode(kyber_of(9)), ..good.clone() };

    for (bad, said) in [
        (card_json(&by_bo), "a card naming Ann's key with Bo's signature was kept"),
        (card_json(&bos_own), "Bo's own card, sent with Ann's sign-in, was kept"),
        (card_json(&swapped), "Ann's card with another Kyber key put in was kept"),
        (json!({ "v": 1, "key": ann, "kyber": good.kyber, "at": good.at }), "an unsigned card was kept"),
    ] {
        assert_eq!(block(keep_from_sign_in(&st, &ann, bad)), None, "{said}");
        assert!(st.db.get_key_card(&ann).unwrap().is_none(), "{said}");
        assert!(st.db.get_key_card(&key_of(9)).unwrap().is_none(), "{said} (under Bo's key)");
        match told.try_recv() {
            Ok(RelayMessage::Private { to, message }) => assert_eq!((to.as_str(), message.as_str()), (ann.as_str(), REFUSED_SENTENCE)),
            other => panic!("Ann was not told: {other:?}"),
        }
    }
    assert!(cards_got(&mut link).is_empty(), "a refused card was passed on");

    assert_eq!(block(keep_from_sign_in(&st, &ann, card_json(&good))), Some(Keep::New), "her own good card");
    assert_eq!(st.db.get_key_card(&ann).unwrap(), Some(good.clone()));
    assert_eq!(cards_got(&mut link).len(), 1, "a kept card goes to the linked server");
    // The same card again (another sign-in) changes nothing and is not passed on again.
    assert_eq!(block(keep_from_sign_in(&st, &ann, card_json(&sign_card(&[7; 32], now_millis()).unwrap()))), Some(Keep::SameKey));
    assert_eq!(st.db.key_card_at(&ann), Some(good.at), "the first time stays");
    assert!(cards_got(&mut link).is_empty());
}

/// CARDS ARE HANDED OUT EXACTLY AS SIGNED, to the asker alone: a `key_card_request` gets the cards
/// held and the keys with none (at most 16 looked up), and `GET /api/key-card/{key}` the card.
#[test]
fn cards_are_handed_out_exactly_as_signed() {
    let st = state("serve");
    let mut rx = st.broadcast_tx.subscribe();
    let good = card_of(7);
    assert_eq!(block(keep_from_sign_in(&st, &good.key, card_json(&good))), Some(Keep::New));
    while rx.try_recv().is_ok() {}
    let nobody = key_of(11);
    let mut keys: Vec<String> = vec![good.key.clone(), nobody.clone(), "not a key".into(), good.key.clone()];
    keys.extend((0..20u8).map(|i| key_of(100 + i)));
    handle_request(&st, "asker", &json!({ "type": "key_card_request", "keys": keys }));
    let Ok(RelayMessage::KeyCards { to, cards, missing }) = rx.try_recv() else { panic!("no answer") };
    assert_eq!(to, "asker", "to the asker alone");
    assert_eq!(cards.0.as_slice(), [good.clone()], "the card, exactly as signed");
    assert_eq!(missing.len(), key_card::REQUEST_MAX - 1, "at most 16 looked up, the repeat once: {}", missing.len());
    assert!(missing.contains(&nobody) && missing.contains(&"not a key".to_string()));
    let wire: Value = serde_json::from_str(&serde_json::to_string(&RelayMessage::KeyCards { to, cards, missing }).unwrap()).unwrap();
    assert_eq!(wire["type"], "key_cards");
    assert_eq!(wire["cards"][0], card_json(&good), "the wire carries the card as its owner wrote it");

    use axum::response::IntoResponse;
    let resp = block(api_get_key_card(State(st.clone()), Path(good.key.clone()))).into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = block(axum::body::to_bytes(resp.into_body(), 1 << 20)).unwrap();
    let served: Value = serde_json::from_slice(&body).unwrap();
    assert!(card_for(&key_card::parse_card(&served).expect("a card"), &good.key), "what is served checks out as hers");
    assert_eq!(block(api_get_key_card(State(st.clone()), Path(nobody))).into_response().status(), StatusCode::NOT_FOUND);
    assert_eq!(block(api_get_key_card(State(st), Path("../x".into()))).into_response().status(), StatusCode::BAD_REQUEST);
}

/// A VALID CARD IS ACCEPTED FROM ANY SERVER. Ann signs in on server A; A passes her card to B,
/// a linked server she never used; B checks it, keeps it and serves it, and what B serves checks
/// out as Ann's exactly as what A serves does: a card names no server.
///
/// Seen red 2026-10-23 with the `KeyCardGossip` arm of `handle_peer_message` dropping the card
/// (`let _ = card;`): "server B keeps Ann's card from A" failed.
#[test]
fn a_valid_card_is_accepted_from_any_server() {
    let a = state("any_a");
    let b = state("any_b");
    let mut a_to_b = link_a_server(&a, "b.example");
    let ann = key_of(7);
    a.db.register_name("Ann", &ann).unwrap();
    let card = card_of(7);
    block(keep_from_sign_in(&a, &ann, card_json(&card)));
    let frames = cards_got(&mut a_to_b);
    assert_eq!(frames.len(), 1, "A passes the card on");
    let msg: RelayMessage = serde_json::from_value(frames[0].clone()).expect("a relay message");
    block(crate::relay::handlers::federation::handle_peer_message(&b, "a.example", msg));
    let at_b = b.db.get_key_card(&ann).unwrap().expect("server B keeps Ann's card from A");
    assert!(card_for(&at_b, &ann), "and what B serves checks out as Ann's");
    assert_eq!(at_b, a.db.get_key_card(&ann).unwrap().unwrap(), "the same card on both");
}

/// A LINKED SERVER'S BAD CARD IS REFUSED, and a card is passed on only for an account here: B
/// refuses a swapped card from A; a good card of someone with no account at B is kept there but
/// goes no further; one of B's own accounts goes on to B's other links, never back to A.
///
/// Seen red 2026-10-23 with `key_card::verify_card` passing every card of the right shape: "a
/// swapped card from a link was kept" failed.
#[test]
fn a_linked_servers_bad_card_is_refused_and_only_ours_go_on() {
    let b = state("link_rules");
    let mut b_to_a = link_a_server(&b, "a.example");
    let mut b_to_c = link_a_server(&b, "c.example");
    let good = card_of(7);
    let swapped = KeyCard { kyber: B64.encode(kyber_of(9)), ..good.clone() };
    assert!(!block(keep_from_link(&b, "a.example", swapped)), "a swapped card from a link was kept");
    assert!(b.db.get_key_card(&good.key).unwrap().is_none());

    assert!(block(keep_from_link(&b, "a.example", good.clone())));
    assert!(cards_got(&mut b_to_c).is_empty(), "a card of someone with no account here went on");

    let bo = card_of(9);
    b.db.register_name("Bo", &bo.key).unwrap();
    assert!(block(keep_from_link(&b, "a.example", bo.clone())));
    assert_eq!(cards_got(&mut b_to_c).len(), 1, "one of ours goes on to the other links");
    assert!(cards_got(&mut b_to_a).is_empty(), "never back where it came from");
    assert!(verify_card(&bo));
    // A different key for Bo, signed by Bo and newer, replaces it; an older one does not.
    let newer = sign_card_with(&dil(9), &kyber_of(13), bo.at + 5);
    assert!(block(keep_from_link(&b, "a.example", newer.clone())));
    assert!(!block(keep_from_link(&b, "a.example", sign_card_with(&dil(9), &kyber_of(15), bo.at + 1))));
    assert_eq!(b.db.get_key_card(&bo.key).unwrap(), Some(newer));
}

/// THE CATCH-UP: a new link is sent this server's accounts' cards with its profile copies.
#[test]
fn a_new_link_is_sent_this_servers_cards() {
    let st = state("catch_up");
    let ann = card_of(7);
    st.db.register_name("Ann", &ann.key).unwrap();
    block(keep_from_sign_in(&st, &ann.key, card_json(&ann)));
    let mut rx = link_a_server(&st, "new.example");
    block(crate::relay::handlers::federation::send_own_copies(&st, "new.example"));
    let got = cards_got(&mut rx);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0]["card"], card_json(&ann));
}

/// FEDERATION SWITCHED OFF TAKES NO CARDS IN. A linked server sends a good card in its numbered
/// log (`fed_event`, the way every card now travels, federation_log.rs) and the same card in a
/// catch-up (un-numbered): a server with federation off keeps neither, and one with federation on
/// keeps each.
///
/// Seen red 2026-10-24 with the federation check taken out of `federation_log::ingest`: "a
/// server with federation off kept a card from a linked server". (A peer's frames never pass the
/// socket's feature gate: its hello leaves the signed-in loop before it.)
#[test]
fn federation_switched_off_takes_no_cards_from_other_servers() {
    let card = card_of(7);
    let numbered = json!({ "type": "fed_event", "log": "log-a", "seq": 1, "event": { "type": "key_card_gossip", "card": card_json(&card) } }).to_string();
    let catch_up = json!({ "type": "key_card_gossip", "card": card_json(&card) }).to_string();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

    let mut off = RelayState::new(Storage::open_temp("keycards_fed_off"));
    off.features.set(crate::relay::features::Feature::Federation, false);
    let off = Arc::new(off);
    block(crate::relay::handlers::federation_log::ingest(&off, "a.example", &tx, &numbered));
    block(crate::relay::handlers::federation_log::ingest(&off, "a.example", &tx, &catch_up));
    assert_eq!(off.db.get_key_card(&card.key).unwrap(), None, "a server with federation off kept a card from a linked server");

    let on = state("fed_on");
    block(crate::relay::handlers::federation_log::ingest(&on, "a.example", &tx, &numbered));
    assert_eq!(on.db.get_key_card(&card.key).unwrap(), Some(card.clone()), "a server with federation on keeps the card from the log");
    let on2 = state("fed_on_catch_up");
    block(crate::relay::handlers::federation_log::ingest(&on2, "a.example", &tx, &catch_up));
    assert_eq!(on2.db.get_key_card(&card.key).unwrap(), Some(card.clone()), "and from a catch-up");
}

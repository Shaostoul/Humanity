// Child of engine/key_cards.rs (#[path]): key cards in the desktop app (2026-10-22,
// docs/design/key-cards.md). A key a server swapped into someone's card gives nothing to seal
// to; a good card is taken from any server; a newer card with a different key is a change the
// person must accept before anything is sealed to them again; our own card rides every sign-in;
// the open conversation asks for its card at once; and no app code reads a bare Kyber key a
// server sends.

use super::*;
use crate::relay::core::key_card::{card_json, card_text, sign_card, sign_card_with, KeyCard, Refusal};
use crate::relay::core::pq_crypto::{derive_dilithium_seed, derive_kyber_seed, DilithiumKeypair, KyberKeypair};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::json;

const AT: u64 = 1_760_000_000_000;

fn dil(b: u8) -> DilithiumKeypair {
    DilithiumKeypair::from_seed(&derive_dilithium_seed(&[b; 32]))
}
fn key_of(b: u8) -> String {
    hex::encode(dil(b).public_key())
}
fn kyber_of(b: u8) -> Vec<u8> {
    KyberKeypair::from_seed(&derive_kyber_seed(&[b; 32])).unwrap().public_key()
}

/// The app of the identity of seed `[me; 32]`, unlocked, its pins in a temp file.
fn app(me: u8) -> GuiState {
    let mut gs = GuiState::default();
    gs.private_key_bytes = Some(vec![me; 32]);
    gs.profile_public_key = key_of(me);
    on_identity(&mut gs);
    gs
}

/// A `key_cards` answer carrying `cards`.
fn answer(cards: &[serde_json::Value]) -> serde_json::Value {
    json!({ "type": "key_cards", "to": "me", "cards": cards, "missing": [] })
}

/// A SERVER-SWAPPED KEY IS REFUSED. The server answers with Ann's card but its own Kyber768 key
/// in it (and, separately, a card Bo signed naming Ann's key): neither checks out, so nothing is
/// sealed to Ann (a pass, a permit, a note: `control_puts` builds nothing), the Chat page says
/// she has no card it can check, and her real card afterwards is taken.
///
/// Seen red 2026-10-23 with `key_card::verify_card` passing every card of the right shape (so
/// `card_for` checked only the name: the old trust in what the server says): "a swapped key gave
/// Ann a key" failed.
#[test]
fn a_server_swapped_key_is_refused() {
    let mut gs = app(30);
    let ann = key_of(31);
    let real = sign_card(&[31; 32], AT).unwrap();
    let swapped = KeyCard { kyber: B64.encode(kyber_of(99)), ..real.clone() };
    let by_bo = KeyCard { sig: B64.encode(dil(32).sign(card_text(&real.key, &real.kyber, real.at).as_bytes())), ..real.clone() };
    on_key_cards_frame(&mut gs, &answer(&[card_json(&swapped), card_json(&by_bo)]));
    assert!(!gs.peer_kyber_keys.contains_key(&ann), "a swapped key gave Ann a key");
    assert!(crate::engine::dm::control_puts(&gs, &ann, "[[hum:follow]]", None).is_none(), "something was sealed to the swapped key");
    assert_eq!(refusal(&gs, &ann), Some(Refusal::NoCard));
    let said = refusal_text(&mut gs, &ann).expect("a sentence");
    assert!(said.starts_with("Nothing was sent:") && said.contains("no signed key card"), "{said}");

    on_key_cards_frame(&mut gs, &answer(&[card_json(&real)]));
    assert_eq!(gs.peer_kyber_keys.get(&ann), Some(&real.kyber), "her real card is taken");
    assert!(crate::engine::dm::control_puts(&gs, &ann, "[[hum:follow]]", None).is_some(), "and something can be sealed to her");
}

/// A VALID CARD IS ACCEPTED FROM ANY SERVER: a card names no server, so the same check passes
/// whichever link brought it (the active server's socket or a background link both hand the
/// answer to `on_key_cards_frame`), and the pin belongs to the identity, so it still seals after
/// the app moves to another server. The shared fixture's card (made by the relay's builder, read
/// by the website's test too) is taken the same way.
#[test]
fn a_valid_card_is_accepted_from_any_server() {
    let mut gs = app(33);
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/key-card-v1.json")).unwrap(),
    )
    .unwrap();
    on_key_cards_frame(&mut gs, &answer(&[fixture["card"].clone()]));
    let signer = fixture["card"]["key"].as_str().unwrap().to_string();
    assert!(gs.peer_kyber_keys.contains_key(&signer), "the fixture's card, from server A");
    gs.server_url = "https://another.example".into();
    gs.dm_store = None;
    let bo = sign_card(&[34; 32], AT).unwrap();
    on_key_cards_frame(&mut gs, &answer(&[card_json(&bo)]));
    assert!(gs.peer_kyber_keys.contains_key(&bo.key), "Bo's card, from server B");
    assert!(gs.peer_kyber_keys.contains_key(&signer), "and the pin from server A still seals on B");
    assert!(crate::engine::dm::control_puts(&gs, &signer, "[[hum:follow]]", None).is_some());
}

/// A CHANGED KEY RAISES THE CHANGE NOTICE: Ann's first card is pinned; a newer card she signed
/// with another Kyber768 key takes her key away (nothing is sealed to her), the conversation shows
/// the notice, a send says to accept first; Accept makes the new key the one sealed to. An older
/// card with another key is ignored.
///
/// Seen red 2026-10-23 with net/key_pins.rs `sealing_key` ignoring the change waiting (so the old
/// key stayed in `peer_kyber_keys`): "a changed key still sealed" failed.
#[test]
fn a_changed_key_raises_the_change_notice() {
    let mut gs = app(35);
    let ann = key_of(36);
    let first = sign_card(&[36; 32], AT).unwrap();
    assert_eq!(ingest(&mut gs, &card_json(&first)), Some(Seen::First));
    assert!(change_notice(&gs, &ann).is_none());

    let older = sign_card_with(&dil(36), &kyber_of(80), AT - 10);
    assert_eq!(ingest(&mut gs, &card_json(&older)), Some(Seen::Stale));
    assert_eq!(gs.peer_kyber_keys.get(&ann), Some(&first.kyber), "an older card changed nothing");

    let newer = sign_card_with(&dil(36), &kyber_of(81), AT + 10);
    assert_eq!(ingest(&mut gs, &card_json(&newer)), Some(Seen::Changed));
    assert!(!gs.peer_kyber_keys.contains_key(&ann), "a changed key still sealed");
    assert!(crate::engine::dm::control_puts(&gs, &ann, "[[hum:follow]]", None).is_none());
    assert_eq!(refusal(&gs, &ann), Some(Refusal::Changed));
    let notice = change_notice(&gs, &ann).expect("the notice");
    assert!(notice.contains("encryption key changed") && notice.contains("before you accept"), "{notice}");
    assert!(refusal_text(&mut gs, &ann).unwrap().contains("Accept the new key"));

    assert!(accept(&mut gs, &ann));
    assert_eq!(gs.peer_kyber_keys.get(&ann), Some(&newer.kyber), "the accepted key is sealed to");
    assert!(change_notice(&gs, &ann).is_none());
    assert!(crate::engine::dm::control_puts(&gs, &ann, "[[hum:follow]]", None).is_some());
}

/// OUR OWN CARD RIDES EVERY SIGN-IN: unlocking signs it, and the identify frame carries it, a card
/// that checks out as ours with the Kyber768 key our seed gives.
#[test]
fn our_own_card_rides_every_sign_in() {
    let gs = app(37);
    let frame = crate::net::ws_client::identify_frame(&gs.profile_public_key, "Me", &gs.own_key_card, false);
    let card = crate::relay::core::key_card::parse_card(&frame["key_card"]).expect("the identify carries our card");
    assert!(crate::relay::core::key_card::card_for(&card, &gs.profile_public_key));
    assert_eq!(crate::relay::core::key_card::kyber_bytes(&card), Some(kyber_of(37)));
}

/// THE OPEN CONVERSATION ASKS FOR ITS CARD AT ONCE, in a `key_card_request`, once a minute at
/// most; a list showing a card newer than the pin asks again even though a key is pinned.
#[test]
fn the_open_conversation_asks_for_its_card() {
    let mut gs = app(38);
    let ann = key_of(39);
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.ws_identified = true;
    gs.key_cards.last_sweep = Some(Instant::now());
    gs.chat_active_channel = format!("dm:{ann}");
    pump(&mut gs);
    let frames: Vec<serde_json::Value> = sent.try_iter().filter_map(|f| serde_json::from_str(&f).ok()).collect();
    assert_eq!(frames.len(), 1, "{frames:?}");
    assert_eq!((frames[0]["type"].as_str(), frames[0]["keys"][0].as_str()), (Some("key_card_request"), Some(ann.as_str())));
    pump(&mut gs);
    assert_eq!(sent.try_iter().count(), 0, "asked again within the minute");

    ingest(&mut gs, &card_json(&sign_card(&[39; 32], AT).unwrap()));
    note_listed(&mut gs, &ann, Some(AT));
    pump(&mut gs);
    assert_eq!(sent.try_iter().count(), 0, "the same card time asks nothing");
    note_listed(&mut gs, &ann, Some(AT + 1));
    pump(&mut gs);
    assert_eq!(sent.try_iter().count(), 1, "a newer card time asks again");
}

/// NO APP CODE READS A BARE KYBER KEY FROM THE SERVER ANY MORE: every key sealed to comes from a
/// checked key card. A source scan, so a list parser that starts reading `kyber_public` again fails
/// here before anything is sealed to a server's word.
#[test]
fn no_app_code_reads_a_bare_kyber_key() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    for dir in ["engine", "gui", "net"] {
        let mut stack = vec![root.join(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap_or_default();
                    if text.contains("get(\"kyber_public\")") {
                        hits.push(p.display().to_string());
                    }
                }
            }
        }
    }
    assert!(hits.is_empty(), "these read a server's bare Kyber key: {hits:?}");
}

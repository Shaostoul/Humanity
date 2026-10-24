// Key cards (2026-10-22, docs/design/key-cards.md): the card's words, its check, what a server
// keeps, what an app pins, the sentences a person reads, and the shared fixture both apps read.

use super::*;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, derive_kyber_seed, DilithiumKeypair, KyberKeypair};

/// The signer's seed byte: the 32-byte seed `[7; 32]`, the key every Rust fixture uses.
const SIGNER: u8 = 7;
/// Somebody else's seed byte (the server's substitute key, the wrong signer).
const OTHER: u8 = 9;
/// The time on the fixture's card.
const AT: u64 = 1_760_000_000_000;

fn seed(b: u8) -> [u8; 32] {
    [b; 32]
}
fn dil(b: u8) -> DilithiumKeypair {
    DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed(b)))
}
fn kyber(b: u8) -> Vec<u8> {
    KyberKeypair::from_seed(&derive_kyber_seed(&seed(b))).unwrap().public_key()
}

/// A GOOD CARD IS GOOD FOR ITS OWN PERSON ONLY: a card signed from a seed checks out and names
/// the keys that seed gives; it is no card for anybody else.
#[test]
fn a_card_from_a_seed_checks_out_for_that_person_only() {
    let card = sign_card(&seed(SIGNER), AT).unwrap();
    assert!(shape_ok(&card), "{card:?}");
    assert!(verify_card(&card));
    assert_eq!(card.key, hex::encode(dil(SIGNER).public_key()), "the card names the seed's identity");
    assert_eq!(kyber_bytes(&card), Some(kyber(SIGNER)), "and its Kyber768 key");
    assert!(card_for(&card, &card.key.clone()));
    assert!(!card_for(&card, &hex::encode(dil(OTHER).public_key())), "a good card of somebody else is no card for them");
    assert_eq!(parse_card(&card_json(&card)), Some(card), "the JSON reads back");
}

/// A SERVER-SWAPPED KEY IS REFUSED: a server that puts a Kyber768 key of its own into someone's
/// card cannot sign the words again, so the card no longer checks out.
///
/// Seen red 2026-10-23 with `verify_card` passing every card of the right shape (the signature
/// checked and its answer thrown away): "a card whose key the server swapped checked out" failed.
#[test]
fn a_card_whose_key_the_server_swapped_is_refused() {
    let card = sign_card(&seed(SIGNER), AT).unwrap();
    let swapped = KeyCard { kyber: B64.encode(kyber(OTHER)), ..card.clone() };
    assert!(shape_ok(&swapped), "the swap keeps the shape, so only the signature can catch it");
    assert!(!verify_card(&swapped), "a card whose key the server swapped checked out");
    assert!(!card_for(&swapped, &card.key));
    // Nor can the time be moved, so an old card cannot be dated after a new one.
    assert!(!verify_card(&KeyCard { at: AT + 1, ..card }), "a card whose time was changed checked out");
}

/// A CARD SIGNED BY THE WRONG KEY IS REFUSED: the words name Ann's key, the signature is Bo's.
///
/// Seen red 2026-10-23 with the same break as above: "a card signed by somebody else's key
/// checked out" failed.
#[test]
fn a_card_signed_by_the_wrong_key_is_refused() {
    let ann = sign_card(&seed(SIGNER), AT).unwrap();
    let by_bo = B64.encode(dil(OTHER).sign(card_text(&ann.key, &ann.kyber, ann.at).as_bytes()));
    let wrong = KeyCard { sig: by_bo, ..ann.clone() };
    assert!(shape_ok(&wrong));
    assert!(!verify_card(&wrong), "a card signed by somebody else's key checked out");
    // Bo's own card for Bo's key checks out, and is still no card for Ann.
    let bo = sign_card_with(&dil(OTHER), &kyber(SIGNER), AT);
    assert!(verify_card(&bo));
    assert!(!card_for(&bo, &ann.key), "Bo's card, carrying Ann's Kyber key, passed as Ann's");
}

/// ONE SPELLING: an upper-case key, base64 with a space or without padding, a key or signature of
/// the wrong length, version 2, time 0, an extra field or a missing one: none of them is a card.
#[test]
fn a_card_has_one_spelling() {
    let card = sign_card(&seed(SIGNER), AT).unwrap();
    let bad = [
        KeyCard { key: card.key.to_uppercase(), ..card.clone() },
        KeyCard { kyber: format!(" {}", &card.kyber[1..]), ..card.clone() },
        KeyCard { kyber: card.kyber.trim_end_matches('=').to_string(), ..card.clone() },
        KeyCard { sig: card.sig[..card.sig.len() - 4].to_string(), ..card.clone() },
        KeyCard { key: card.key[2..].to_string(), ..card.clone() },
        KeyCard { v: 2, ..card.clone() },
        KeyCard { at: 0, ..card.clone() },
    ];
    for (i, c) in bad.iter().enumerate() {
        assert!(!shape_ok(c), "bad spelling {i} passed: {c:?}");
        assert!(!verify_card(c), "bad spelling {i} checked out");
    }
    let mut extra = card_json(&card);
    extra["name"] = serde_json::json!("Ann");
    assert_eq!(parse_card(&extra), None, "an unsigned field rode along");
    let mut short = card_json(&card);
    short.as_object_mut().unwrap().remove("at");
    assert_eq!(parse_card(&short), None, "a card without its time");
    assert_eq!(parse_card(&serde_json::json!("not a card")), None);
}

/// WHAT A SERVER KEEPS: nothing held keeps it; the same key changes nothing (the first time
/// stays); a different key replaces only when newer; a card a day and a minute ahead is refused.
#[test]
fn the_keep_rule() {
    let now = AT;
    let card = |kyber: &str, at: u64| KeyCard { v: 1, key: "k".into(), kyber: kyber.into(), at, sig: String::new() };
    assert_eq!(keep_rule(None, &card("K1", now), now), Keep::New);
    assert_eq!(keep_rule(Some(("K1", now - 5)), &card("K1", now), now), Keep::SameKey);
    assert_eq!(keep_rule(Some(("K1", now - 5)), &card("K2", now), now), Keep::Replace);
    assert_eq!(keep_rule(Some(("K1", now)), &card("K2", now), now), Keep::Older, "the same time is not newer");
    assert_eq!(keep_rule(Some(("K1", now)), &card("K2", now - 1), now), Keep::Older);
    assert_eq!(keep_rule(None, &card("K1", now + MAX_AHEAD_MS + 60_000), now), Keep::TooFarAhead);
    assert_eq!(keep_rule(None, &card("K1", now + MAX_AHEAD_MS - 60_000), now), Keep::New);
    assert!(Keep::New.stored() && Keep::Replace.stored() && !Keep::SameKey.stored() && !Keep::Older.stored());
}

// ── The shared fixture ──

/// The fixture both apps read.
const FIXTURE: &str = "scripts/tests/fixtures/key-card-v1.json";

/// The pin rule's rows: (pinned key and time or none, the card's key and time, what it means).
fn compare_rows() -> Vec<(Option<(&'static str, u64)>, (&'static str, u64), Seen)> {
    vec![
        (None, ("K1", 5), Seen::First),
        (Some(("K1", 5)), ("K1", 5), Seen::Same),
        (Some(("K1", 5)), ("K1", 9), Seen::Same),
        (Some(("K1", 5)), ("K1", 2), Seen::Same),
        (Some(("K1", 5)), ("K2", 6), Seen::Changed),
        (Some(("K1", 5)), ("K2", 5), Seen::Stale),
        (Some(("K1", 5)), ("K2", 4), Seen::Stale),
    ]
}

/// The whole fixture, as this code makes it today.
fn fixture_now() -> Value {
    let card = sign_card(&seed(SIGNER), AT).unwrap();
    let swapped = KeyCard { kyber: B64.encode(kyber(OTHER)), ..card.clone() };
    let wrong_signer = KeyCard { sig: B64.encode(dil(OTHER).sign(card_text(&card.key, &card.kyber, card.at).as_bytes())), ..card.clone() };
    let other = sign_card(&seed(OTHER), AT + 1).unwrap();
    let rows: Vec<Value> = compare_rows()
        .into_iter()
        .map(|(pin, (k, at), seen)| {
            serde_json::json!({
                "pin": pin.map(|(pk, pa)| serde_json::json!({ "kyber": pk, "at": pa })),
                "card": { "kyber": k, "at": at },
                "seen": seen.word(),
            })
        })
        .collect();
    serde_json::json!({
        "about": "Key cards (2026-10-22, docs/design/key-cards.md). Made by the relay's own builder (src/relay/core/key_card.rs; ML-DSA signing there is deterministic). `card` is the card of the 32-byte seed [signer_master_byte; 32] dated its `at`, and `text` the exact words its signature covers. `swapped` is that card with the Kyber768 key of the seed [other_master_byte; 32] put in its place (what a lying server would serve): it must NOT check out. `wrong_signer` is the card's words signed by the other seed's identity: it must NOT check out. `other_card` is the other seed's own card: it checks out, but is no card for the signer. `compare` rows are the pin rule (key_card.rs `compare`): a pinned key and time (or none) against a good card's key and time. `refusals` are the sentences both apps show, for the name `name`. Written by key_card_tests.rs write_the_key_card_fixture; checked by the_shared_fixture_is_what_the_builder_makes; read by scripts/tests/key-card-web.test.js.",
        "signer_master_byte": SIGNER,
        "other_master_byte": OTHER,
        "card": card_json(&card),
        "text": card_text(&card.key, &card.kyber, card.at),
        "swapped": card_json(&swapped),
        "wrong_signer": card_json(&wrong_signer),
        "other_card": card_json(&other),
        "compare": rows,
        "refusals": {
            "name": "Ann",
            "no_card": Refusal::NoCard.text("Ann"),
            "fetching": Refusal::Fetching.text("Ann"),
            "changed": Refusal::Changed.text("Ann"),
            "notice": change_notice("Ann"),
            "accept": ACCEPT_LABEL,
        },
    })
}

/// WRITES scripts/tests/fixtures/key-card-v1.json from fixed seeds. Run it again only when the
/// shape changes on purpose: `node scripts/lib-tests.js write_the_key_card_fixture --ignored`.
#[test]
#[ignore = "writes scripts/tests/fixtures/key-card-v1.json; run: node scripts/lib-tests.js write_the_key_card_fixture --ignored"]
fn write_the_key_card_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&fixture_now()).unwrap())).expect("the fixture is written");
}

/// THE SHARED FIXTURE BOTH APPS READ IS WHAT THIS CODE MAKES: the card is the seed's, its words
/// are `card_text`, its signature is what the builder makes byte for byte and checks out; the
/// swapped and wrongly signed cards do not; the other card is good and no card for the signer;
/// the pin rows hold; and the whole file is what the writer makes today.
///
/// Seen red 2026-10-23 with `kyber` and `key` swapped in `card_text` (the builder then signs other
/// words than the fixture holds): "the card, byte for byte" failed.
#[test]
fn the_shared_fixture_is_what_the_builder_makes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let fx: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("JSON");
    let card = parse_card(&fx["card"]).expect("a card");
    assert_eq!(card, sign_card(&seed(SIGNER), AT).unwrap(), "the card, byte for byte");
    assert_eq!(fx["text"].as_str(), Some(card_text(&card.key, &card.kyber, card.at).as_str()), "the words");
    assert!(card_for(&card, &card.key));
    let swapped = parse_card(&fx["swapped"]).expect("the swapped card");
    assert!(!verify_card(&swapped), "the swapped card checks out");
    let wrong = parse_card(&fx["wrong_signer"]).expect("the wrongly signed card");
    assert!(!verify_card(&wrong), "the wrongly signed card checks out");
    let other = parse_card(&fx["other_card"]).expect("the other card");
    assert!(verify_card(&other) && !card_for(&other, &card.key));
    for row in fx["compare"].as_array().expect("rows") {
        let pin = row["pin"].as_object().map(|p| (p["kyber"].as_str().unwrap(), p["at"].as_u64().unwrap()));
        let seen = compare(pin, row["card"]["kyber"].as_str().unwrap(), row["card"]["at"].as_u64().unwrap());
        assert_eq!(Some(seen.word()), row["seen"].as_str(), "{row}");
    }
    assert_eq!(fx, fixture_now(), "the fixture is what the writer makes today");
}

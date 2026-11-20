// Child of held.rs (#[path]): whom a held copy can be sealed to. The hide itself, through a
// report's decision, is tested in handlers/held_tests.rs.

use super::*;
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

fn key_of(seed: &[u8]) -> String {
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(seed)).public_key())
}

/// No owner, two owners, an owner with no card, an owner whose card is someone else's or was
/// altered: each refused with its own words. The owner's own good card: ready, carrying the
/// Kyber768 key that card names.
///
/// Seen red 2026-11-20 with `owner_card` skipping the `card_for` check: "a card naming someone
/// else is refused" (it answered `Ready` with the other person's Kyber768 key).
#[test]
fn a_copy_is_sealed_only_to_the_one_owners_own_checked_card() {
    let db = Storage::open_temp("held_owner_card");
    assert_eq!(db.owner_card(), OwnerCard::NoOwner);
    assert_eq!(OwnerCard::NoOwner.refusal(), Some(NO_OWNER));
    let (own, other) = ([9u8; 32], [8u8; 32]);
    let (own_key, other_key) = (key_of(&own), key_of(&other));
    db.set_role(&own_key, "owner").unwrap();
    db.set_role(&other_key, "owner").unwrap();
    assert_eq!(db.owner_card(), OwnerCard::ManyOwners(2));
    db.set_role(&other_key, "admin").unwrap();
    assert_eq!(db.owner_card(), OwnerCard::NoCard);
    assert_eq!(db.owner_card().refusal(), Some(NO_CARD));

    // A good card, but the other person's, filed under the owner's key by hand.
    let theirs = key_card::sign_card(&other, 1_000).unwrap();
    db.with_conn(|c| {
        c.execute(
            "INSERT INTO key_cards (public_key, kyber_public, signed_at, signature) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![own_key, theirs.kyber, theirs.at as i64, theirs.sig],
        )
    })
    .unwrap();
    assert!(matches!(db.owner_card(), OwnerCard::BadCard), "a card naming someone else is refused");

    db.with_conn(|c| c.execute("DELETE FROM key_cards", [])).unwrap();
    let mine = key_card::sign_card(&own, 2_000).unwrap();
    assert!(db.store_key_card(&mine).unwrap().stored());
    match db.owner_card() {
        OwnerCard::Ready { owner, kyber, card_at } => {
            assert_eq!(owner, own_key);
            assert_eq!(Some(kyber), key_card::kyber_bytes(&mine));
            assert_eq!(card_at, 2_000);
        }
        other => panic!("the owner's own card is ready, not {other:?}"),
    }
}

// Child of storage/recovery.rs: what the server keeps for Recovery through trusted people, driven
// through the same door every app uses (`put_signed_object`, the chokepoint behind
// POST /api/v2/objects). Each test was seen failing once; what it saw red is at the test.

use super::*;
use crate::relay::core::object::ObjectBuilder;
use crate::relay::core::pq_crypto::{DilithiumKeypair, KyberKeypair, derive_dilithium_seed, derive_kyber_seed};
use crate::relay::core::recovery::{
    self as proto, OneTimeAsk, ServedPiece, TrustedPerson, build_setup, open_answer, open_piece, seal_answer, served_piece,
};

struct Person {
    secret: [u8; 32],
    dil: DilithiumKeypair,
    kyber: KyberKeypair,
    did: String,
}

fn person(b: u8) -> Person {
    let secret = [b; 32];
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&secret));
    let kyber = KyberKeypair::from_seed(&derive_kyber_seed(&secret)).unwrap();
    let did = did_for_pubkey(&dil.public_key());
    Person { secret, dil, kyber, did }
}

fn now() -> u64 {
    super::super::now_millis()
}

/// A setup of `holder` with these trusted people, `needed` of them, made at `at`.
fn setup_of(holder: &Person, people: &[&Person], needed: usize, at: u64) -> (String, Vec<Object>) {
    let trusted: Vec<TrustedPerson> = people
        .iter()
        .enumerate()
        .map(|(i, p)| TrustedPerson { did: p.did.clone(), name: format!("Friend {i}"), kyber_public: p.kyber.public_key() })
        .collect();
    build_setup(&holder.secret, &holder.dil, &holder.kyber.public_key(), &trusted, needed, at).unwrap()
}

fn post_all(db: &Storage, objects: &[Object]) {
    for o in objects {
        assert!(db.put_signed_object(o, None).unwrap(), "a new piece is kept");
    }
}

fn served(db: &Storage, holder: &str) -> Vec<ServedPiece> {
    let view = db.recovery_setup(holder).unwrap().expect("a setup stands");
    view.active.expect("it stands").pieces.iter().map(|p| served_piece(&p.object).unwrap()).collect()
}

/// Every byte the database keeps on disk, the database file and its write-ahead log together.
fn disk_bytes(db: &Storage) -> Vec<u8> {
    db.with_conn(|c| c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")).unwrap();
    let path: String = db.with_conn(|c| c.query_row("PRAGMA database_list", [], |r| r.get::<_, String>(2))).unwrap();
    let mut all = std::fs::read(&path).unwrap();
    if let Ok(wal) = std::fs::read(format!("{path}-wal")) {
        all.extend(wal);
    }
    all
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// THE SERVER KEEPS ONLY WHAT IT CANNOT READ: a setup posted through the object door stands and
/// is served back as objects every app can check; not one trusted person's account id, name or
/// key is anywhere on the disk, nor the secret or a piece of it in the clear; and the pieces are
/// not in the object store that is listed to anyone and passed to other servers.
///
/// Seen red 2026-10-21 with `put_signed_object` going on to store the piece in `signed_objects`
/// after keeping it (the object store lists it to anyone): "a piece is not in the listed object
/// store".
#[test]
fn the_server_keeps_only_what_it_cannot_read() {
    let db = Storage::open_temp("recovery_only_ciphertext");
    let (holder, a, b, c) = (person(0x11), person(0x12), person(0x13), person(0x14));
    let (set_id, objects) = setup_of(&holder, &[&a, &b, &c], 2, now());
    post_all(&db, &objects);
    let pieces = served(&db, &holder.did);
    assert_eq!(pieces.len(), 3);
    assert!(pieces.iter().all(|p| p.holder_did == holder.did && p.payload.set_id == set_id));
    let shares: Vec<_> = pieces.iter().zip([&a, &b, &c]).map(|(p, who)| open_piece(&who.kyber, p).unwrap().share).collect();

    let disk = disk_bytes(&db);
    for who in [&a, &b, &c] {
        assert!(!contains(&disk, who.did.as_bytes()), "a trusted person's account id is on the disk");
        assert!(!contains(&disk, &who.kyber.public_key()[..64]), "a trusted person's key is on the disk");
        assert!(!contains(&disk, &who.dil.public_key()[..64]), "a trusted person's signing key is on the disk");
    }
    assert!(!contains(&disk, b"Friend 0"), "a trusted person's name is on the disk");
    assert!(!contains(&disk, &holder.secret), "the secret is on the disk");
    for s in &shares {
        assert!(!contains(&disk, &s[1..17]), "a piece is on the disk in the clear");
    }
    let listed = db.with_conn(|c| {
        c.query_row("SELECT COUNT(*) FROM signed_objects WHERE object_type = 'recovery_share_v1'", [], |r| r.get::<_, i64>(0))
    });
    assert_eq!(listed.unwrap(), 0, "a piece is not in the listed object store");
}

/// MALFORMED, UNSIGNED, FOREIGN OR MISTIMED PIECES ARE REFUSED, and say why: a signature that
/// does not verify, a payload of the wrong shape, a piece from another server, a time a day old
/// or ahead of the clock, a piece that does not match the rest of its setup. Nothing is kept.
///
/// Seen red 2026-10-21 with the time window taken out of `put_recovery_piece`: "a piece made two
/// days ago is refused".
#[test]
fn malformed_unsigned_foreign_or_mistimed_pieces_are_refused() {
    let db = Storage::open_temp("recovery_refused");
    let (holder, a, b) = (person(0x21), person(0x22), person(0x23));
    let (_, objects) = setup_of(&holder, &[&a, &b], 2, now());

    let mut unsigned = objects[0].clone();
    unsigned.signature[10] ^= 1;
    let e = db.put_signed_object(&unsigned, None).unwrap_err().to_string();
    assert!(e.contains("signature verification failed"), "an altered signature: {e}");

    let mut bent = objects[0].clone();
    bent.payload.push(0);
    let e = db.put_signed_object(&bent, None).unwrap_err().to_string();
    assert!(e.contains("signature verification failed") || e.contains("invalid recovery_share_v1"), "{e}");

    // A well-signed object with a malformed payload: the piece cut short.
    let mut p = proto::SharePayload::parse(&objects[0].payload).unwrap();
    p.piece.truncate(100);
    let short = ObjectBuilder::new(proto::SHARE_OBJECT_TYPE).created_at(now()).payload_cbor(&p.to_cbor()).unwrap().sign(&holder.dil).unwrap();
    let e = db.put_signed_object(&short, None).unwrap_err().to_string();
    assert!(e.contains("invalid recovery_share_v1") && e.contains("piece"), "a short piece: {e}");

    let e = db.put_signed_object(&objects[0], Some("peer-server")).unwrap_err().to_string();
    assert!(e.contains("only on the server it was given to"), "from another server: {e}");

    let (_, old) = setup_of(&holder, &[&a, &b], 2, now() - 2 * 86_400_000);
    let e = db.put_signed_object(&old[0], None).unwrap_err().to_string();
    assert!(e.contains("more than a day ago"), "a piece made two days ago is refused: {e}");
    let (_, ahead) = setup_of(&holder, &[&a, &b], 2, now() + 3_600_000);
    let e = db.put_signed_object(&ahead[0], None).unwrap_err().to_string();
    assert!(e.contains("ahead of the server"), "a piece from an hour ahead: {e}");

    // Same setup id, another shape.
    let p0 = proto::SharePayload::parse(&objects[0].payload).unwrap();
    db.put_signed_object(&objects[0], None).unwrap();
    let mut other = p0.clone();
    other.slot = 1;
    other.needed = 2;
    other.total = 3;
    let mismatched = ObjectBuilder::new(proto::SHARE_OBJECT_TYPE)
        .created_at(objects[0].created_at.unwrap())
        .payload_cbor(&other.to_cbor())
        .unwrap()
        .sign(&holder.dil)
        .unwrap();
    let e = db.put_signed_object(&mismatched, None).unwrap_err().to_string();
    assert!(e.contains("does not match the rest of its setup"), "{e}");
    let view = db.recovery_setup(&holder.did).unwrap().unwrap();
    assert!(view.active.is_none(), "one piece of two is not a setup");
    assert_eq!(view.sending.unwrap().have, 1);
}

/// A NEW SETUP REPLACES THE OLD ONE ONLY WHEN IT IS WHOLE, AND THE OLD ONE NEVER COMES BACK: while
/// the new one arrives the old one stands; once the last piece arrives the old one is gone; posting
/// the old objects again is refused; taking everything back empties it and keeps it empty.
///
/// Seen red 2026-10-21 with the floor not written when a setup completes: "the old setup's pieces
/// cannot be posted again".
#[test]
fn a_new_setup_replaces_the_old_only_when_whole_and_the_old_never_returns() {
    let db = Storage::open_temp("recovery_replace");
    let (holder, a, b, c) = (person(0x31), person(0x32), person(0x33), person(0x34));
    let (old_id, old) = setup_of(&holder, &[&a, &b], 2, now() - 60_000);
    post_all(&db, &old);
    let (new_id, new) = setup_of(&holder, &[&b, &c, &a], 2, now());
    db.put_signed_object(&new[0], None).unwrap();
    db.put_signed_object(&new[1], None).unwrap();
    let view = db.recovery_setup(&holder.did).unwrap().unwrap();
    assert_eq!(view.active.as_ref().unwrap().set_id, old_id, "the old setup stands while the new one arrives");
    assert_eq!(view.sending.as_ref().unwrap().have, 2);
    db.put_signed_object(&new[2], None).unwrap();
    let view = db.recovery_setup(&holder.did).unwrap().unwrap();
    assert_eq!(view.active.as_ref().unwrap().set_id, new_id, "the new one stands");
    assert!(view.sending.is_none());
    let rows: i64 = db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM recovery_pieces", [], |r| r.get(0))).unwrap();
    assert_eq!(rows, 3, "the old setup's pieces are gone");
    let again = db.put_signed_object(&old[0], None);
    assert!(again.is_err(), "the old setup's pieces cannot be posted again");

    assert_eq!(db.recovery_withdraw(&holder.did, now()).unwrap(), 3);
    assert!(db.recovery_setup(&holder.did).unwrap().is_none(), "taken back, nothing stands");
    assert!(db.put_signed_object(&new[0], None).is_err(), "a withdrawn setup cannot be posted again");
}

/// ASKING, ANSWERING, REBUILDING: an ask is filed under the code computed from the account and the
/// one-time key; its list entry shows neither; an answer needs the slot's token; answers come back
/// sealed and open only on the asking device; the pieces rebuild the account there. Limits: an
/// account with no setup cannot be asked about, and three open asks are the most for one account.
///
/// Seen red 2026-10-21 with the token check taken out of `recovery_answer`: "an answer without
/// the slot's token is refused".
#[test]
fn asking_answering_and_rebuilding() {
    let db = Storage::open_temp("recovery_ask");
    let (holder, a, b, c) = (person(0x41), person(0x42), person(0x43), person(0x44));
    let (_, objects) = setup_of(&holder, &[&a, &b, &c], 2, now());
    let nobody = person(0x45);
    let ask = OneTimeAsk::new().unwrap();
    let close = proto::close_hash(&ask.close_secret);
    assert!(
        matches!(db.recovery_open_ask(&holder.did, &ask.public, &close, now()), Err(RecoveryRefusal::NotFound(_))),
        "no setup, no ask"
    );
    post_all(&db, &objects);
    assert!(matches!(db.recovery_open_ask(&nobody.did, &ask.public, &close, now()), Err(RecoveryRefusal::NotFound(_))));
    let view = db.recovery_open_ask(&holder.did, &ask.public, &close, now()).unwrap();
    let code = proto::ask_code(&holder.did, &ask.public);
    assert_eq!(view.code, code, "filed under the code computed from the account and the key");
    assert_eq!((view.needed, view.total), (2, 3));
    assert_eq!(db.recovery_open_ask(&holder.did, &ask.public, &close, now()).unwrap().code, code, "asking again is the same ask");
    let listed = db.recovery_open_asks(now(), 10).unwrap();
    assert_eq!(listed.len(), 1);
    let listed_json = serde_json::to_string(&listed).unwrap();
    assert!(!listed_json.contains(&code) && !listed_json.contains(&view.one_time_key_b64[..40]), "the list shows no code and no key");

    let pieces = served(&db, &holder.did);
    let opened_a = open_piece(&a.kyber, &pieces[0]).unwrap();
    let opened_c = open_piece(&c.kyber, &pieces[2]).unwrap();
    let sealed_a = seal_answer(&ask.public, &code, &holder.did, 0, &opened_a.share).unwrap();
    assert!(
        matches!(db.recovery_answer(&code, 0, &[0u8; 32], &sealed_a, now()), Err(RecoveryRefusal::NotAllowed(_))),
        "an answer without the slot's token is refused"
    );
    assert!(matches!(db.recovery_answer(&code, 0, &opened_c.token, &sealed_a, now()), Err(RecoveryRefusal::NotAllowed(_))), "another slot's token");
    assert!(matches!(db.recovery_answer(&code, 0, &opened_a.token, &sealed_a[..100], now()), Err(RecoveryRefusal::Bad(_))));
    assert!(matches!(db.recovery_answer("0000000000000000", 0, &opened_a.token, &sealed_a, now()), Err(RecoveryRefusal::NotFound(_))));
    assert_eq!(db.recovery_answer(&code, 0, &opened_a.token, &sealed_a, now()).unwrap(), 1);
    let sealed_c = seal_answer(&ask.public, &code, &holder.did, 2, &opened_c.share).unwrap();
    assert_eq!(db.recovery_answer(&code, 2, &opened_c.token, &sealed_c, now()).unwrap(), 2);
    assert_eq!(db.recovery_answer(&code, 2, &opened_c.token, &sealed_c, now()).unwrap(), 2, "a second answer for a slot replaces the first");

    use base64::{Engine, engine::general_purpose::STANDARD};
    let got = db.recovery_ask(&code, now()).unwrap().unwrap();
    let back: Vec<_> = got
        .answers
        .iter()
        .map(|r| open_answer(&ask.keys, &code, &holder.did, r.slot, &STANDARD.decode(&r.sealed_b64).unwrap()).unwrap())
        .collect();
    assert_eq!(proto::rebuild(&back, 2, &holder.did), Some(holder.secret), "the asking device rebuilds the account");
    let stranger = OneTimeAsk::new().unwrap();
    assert!(open_answer(&stranger.keys, &code, &holder.did, 0, &sealed_a).is_none(), "only the asking device opens an answer");

    // Three open asks for one account at most.
    for _ in 0..2 {
        let k = OneTimeAsk::new().unwrap();
        db.recovery_open_ask(&holder.did, &k.public, &proto::close_hash(&k.close_secret), now()).unwrap();
    }
    let k = OneTimeAsk::new().unwrap();
    assert!(matches!(db.recovery_open_ask(&holder.did, &k.public, &close, now()), Err(RecoveryRefusal::Full(_))), "a fourth ask is refused");
}

/// ASKS END: seven days after it opened an ask is no longer read, answered or listed, and the
/// expiry pass takes it and its answers off the disk; a setup never finished goes a day after it
/// began; and closing works only with the asking device's secret or for the account it names.
///
/// Seen red 2026-10-21 with `recovery_expire` not deleting the asks: "the ask and its one answer"
/// (1 row removed, not 2).
#[test]
fn asks_end_and_close() {
    let db = Storage::open_temp("recovery_expiry");
    let (holder, a, b) = (person(0x51), person(0x52), person(0x53));
    let (_, objects) = setup_of(&holder, &[&a, &b], 2, now());
    post_all(&db, &objects);
    let ask = OneTimeAsk::new().unwrap();
    let t0 = now();
    let view = db.recovery_open_ask(&holder.did, &ask.public, &proto::close_hash(&ask.close_secret), t0).unwrap();
    let opened = open_piece(&a.kyber, &served(&db, &holder.did)[0]).unwrap();
    let sealed = seal_answer(&ask.public, &view.code, &holder.did, 0, &opened.share).unwrap();
    db.recovery_answer(&view.code, 0, &opened.token, &sealed, t0).unwrap();
    let later = t0 + proto::ASK_MS + 1;
    assert!(db.recovery_ask(&view.code, later).unwrap().is_none(), "an ended ask is not read");
    assert!(db.recovery_open_asks(later, 10).unwrap().is_empty(), "nor listed");
    assert!(matches!(db.recovery_answer(&view.code, 0, &opened.token, &sealed, later), Err(RecoveryRefusal::NotFound(_))), "nor answered");
    assert_eq!(db.recovery_expire(later).unwrap(), 2, "the ask and its one answer");
    let left: i64 = db
        .with_conn(|c| c.query_row("SELECT (SELECT COUNT(*) FROM recovery_asks) + (SELECT COUNT(*) FROM recovery_answers)", [], |r| r.get(0)))
        .unwrap();
    assert_eq!(left, 0, "the expiry pass takes the ended ask off the disk");

    // An unfinished setup goes a day after it began; the standing one stays.
    let (_, half) = setup_of(&holder, &[&a, &b], 2, now() + 1_000);
    db.put_signed_object(&half[0], None).unwrap();
    db.recovery_expire(now() + proto::ASK_MS).unwrap();
    let view = db.recovery_setup(&holder.did).unwrap().unwrap();
    assert!(view.sending.is_none(), "the unfinished setup went");
    assert!(view.active.is_some(), "the standing one stays");

    // Closing.
    let k = OneTimeAsk::new().unwrap();
    let v = db.recovery_open_ask(&holder.did, &k.public, &proto::close_hash(&k.close_secret), now()).unwrap();
    assert!(matches!(db.recovery_close_with_secret(&v.code, &[1u8; 32]), Err(RecoveryRefusal::NotAllowed(_))), "a wrong secret");
    db.recovery_close_with_secret(&v.code, &k.close_secret).unwrap();
    assert!(db.recovery_ask(&v.code, now()).unwrap().is_none(), "closed by the device that asked");
    let k = OneTimeAsk::new().unwrap();
    let v = db.recovery_open_ask(&holder.did, &k.public, &proto::close_hash(&k.close_secret), now()).unwrap();
    assert_eq!(db.recovery_close_all_for(&a.did).unwrap(), 0, "someone else's account closes nothing of it");
    assert!(db.recovery_ask(&v.code, now()).unwrap().is_some());
    assert_eq!(db.recovery_close_all_for(&holder.did).unwrap(), 1, "the account it names can");
    assert!(db.recovery_ask(&v.code, now()).unwrap().is_none());
}

/// THE ERASE AND THE EXPORT COVER IT: the export lists the setup, its floor, the asks naming the
/// person and their answers; the erase removes all of it; and `erase_left_rows` sees a setup left
/// behind.
///
/// Seen red 2026-10-21 with the recovery deletes taken out of `delete_account`: "the erase leaves
/// no piece".
#[test]
fn the_erase_and_the_export_cover_it() {
    let db = Storage::open_temp("recovery_erase");
    let (holder, a, b) = (person(0x61), person(0x62), person(0x63));
    let key = hex::encode(holder.dil.public_key());
    let (_, objects) = setup_of(&holder, &[&a, &b], 2, now());
    post_all(&db, &objects);
    let ask = OneTimeAsk::new().unwrap();
    let view = db.recovery_open_ask(&holder.did, &ask.public, &proto::close_hash(&ask.close_secret), now()).unwrap();
    let opened = open_piece(&a.kyber, &served(&db, &holder.did)[0]).unwrap();
    let sealed = seal_answer(&ask.public, &view.code, &holder.did, 0, &opened.share).unwrap();
    db.recovery_answer(&view.code, 0, &opened.token, &sealed, now()).unwrap();

    let export = db.export_account(&key, "holder");
    assert_eq!(export["recovery_pieces"].as_array().map(|a| a.len()), Some(2), "the export lists the pieces");
    assert_eq!(export["recovery_floor"].as_array().map(|a| a.len()), Some(1));
    assert_eq!(export["recovery_asks"].as_array().map(|a| a.len()), Some(1));
    assert_eq!(export["recovery_answers"].as_array().map(|a| a.len()), Some(1));
    assert!(db.erase_left_rows(&key), "a setup is something the erase answers for");

    let receipt = db.delete_account(&key, "holder");
    assert!(!receipt.iter().any(|(l, _)| l.ends_with("_FAILED")), "{receipt:?}");
    let left: i64 = db
        .with_conn(|c| {
            c.query_row(
                "SELECT (SELECT COUNT(*) FROM recovery_pieces) + (SELECT COUNT(*) FROM recovery_floors)
                      + (SELECT COUNT(*) FROM recovery_asks) + (SELECT COUNT(*) FROM recovery_answers)",
                [],
                |r| r.get(0),
            )
        })
        .unwrap();
    assert_eq!(left, 0, "the erase leaves no piece, floor, ask or answer");
    assert!(!db.erase_left_rows(&key));
}

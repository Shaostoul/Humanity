// Child of net/file_offer.rs (#[path]): the words of device-to-device big files, each rule seen
// failing once against the break named in its doc comment.

use super::*;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const ID: &str = "00112233445566778899aabbccddeeff";

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

/// The spec's example offer (clip.mp4, 2,468,000,000 bytes), with fixed digests and pass.
fn spec_offer() -> Offer {
    Offer {
        id: ID.into(),
        name: "clip.mp4".into(),
        size: 2_468_000_000,
        mime: "video/mp4".into(),
        blake3: "ab".repeat(32),
        piece: 262_144,
        pieces: 9_415,
        hashes: "cd".repeat(32),
        pass: FilePass { v: 1, id: ID.into(), expiry: 1_760_000_000, sig: "c2ln".into() },
    }
}

/// THE BYTES BOTH APPS WRITE (spec, "the shapes, exact", parts 1 and 3), as the spec writes them
/// (the shared fixture is checked in the next test): the offer is the marker then its fields in
/// the spec's order with no spaces, the pass nested last
/// as `{"v","id","expiry","sig"}`; the answer and cancel notes likewise; and the pass signs
/// `hum/file-pass/v1\n{server}\n{issuer}\n{grantee}\n{id}\n{expiry}`.
/// Seen red 2026-10-11 with `mime` moved after `blake3` in `Offer` (the field order the JSON is
/// written in): "the offer note's text" failed, left `...,"blake3":"abab...","type":"video/mp4",...`.
#[test]
fn the_offer_note_and_pass_words_are_the_spec_s() {
    let want = format!(
        "[[hum:offer:v1]]{{\"id\":\"{ID}\",\"name\":\"clip.mp4\",\"size\":2468000000,\"type\":\"video/mp4\",\"blake3\":\"{}\",\"piece\":262144,\"pieces\":9415,\"hashes\":\"{}\",\"pass\":{{\"v\":1,\"id\":\"{ID}\",\"expiry\":1760000000,\"sig\":\"c2ln\"}}}}",
        "ab".repeat(32),
        "cd".repeat(32)
    );
    assert_eq!(spec_offer().text(), want, "the offer note's text");
    assert_eq!(OfferAnswer { id: ID.into(), accept: true }.text(), format!("[[hum:offer-answer:v1]]{{\"id\":\"{ID}\",\"accept\":true}}"));
    assert_eq!(OfferAnswer { id: ID.into(), accept: false }.text(), format!("[[hum:offer-answer:v1]]{{\"id\":\"{ID}\",\"accept\":false}}"));
    assert_eq!(OfferCancel { id: ID.into() }.text(), format!("[[hum:offer-cancel:v1]]{{\"id\":\"{ID}\"}}"));
    assert_eq!(
        file_pass_preimage(SERVER, "aa11", "bb22", ID, 1_760_000_000),
        format!("hum/file-pass/v1\n{SERVER}\naa11\nbb22\n{ID}\n1760000000"),
        "the pass's words"
    );
    assert_eq!(pieces_for(2_468_000_000), 9_415, "pieces = ceil(size / piece)");
    assert_eq!(size_words(2_468_000_000), "2.3 GB", "the question's size, as the spec writes it");
}

/// THE SHARED FIXTURE (scripts/tests/fixtures/file-offer-v1.json, which the relay's builder made and
/// the web's tests read): from the same seeds, this app's identities are the fixture's keys, its
/// pass is the fixture's pass byte for byte (ML-DSA signing is deterministic) and checks, the
/// rule-defined 600,000-byte file read here has the fixture's hashes, and the offer, accept,
/// decline and cancel notes this app builds are the fixture's texts exactly (the name's umlaut and
/// quotes escaped as both JSON writers escape them).
/// Seen red 2026-10-11 with `FILE_PASS_DOMAIN` spelt "hum/file-pass/v2": "the pass's words" failed
/// against the fixture's preimage.
#[test]
fn the_shared_fixture_is_what_this_app_builds() {
    let fx: serde_json::Value = serde_json::from_str(include_str!("../../scripts/tests/fixtures/file-offer-v1.json")).unwrap();
    let s = |k: &str| fx[k].as_str().unwrap().to_string();
    let n = |k: &str| fx[k].as_u64().unwrap();
    let (issuer_seed, issuer) = identity(n("issuer_master_byte") as u8);
    let (_g, grantee) = identity(n("grantee_master_byte") as u8);
    assert_eq!((issuer.as_str(), grantee.as_str()), (s("issuer").as_str(), s("grantee").as_str()), "the same keys from the same seeds");
    let (server, id, expiry, minted) = (s("server"), s("id"), n("expiry"), n("minted_at"));
    assert_eq!(file_pass_preimage(&server, &issuer, &grantee, &id, expiry), s("preimage"), "the pass's words");
    let pass = mint_file_pass(&issuer_seed, &server, &issuer, &grantee, &id, expiry, minted).unwrap();
    assert_eq!(serde_json::to_string(&pass).unwrap(), s("pass_text"), "the pass, byte for byte");
    assert_eq!(serde_json::to_value(&pass).unwrap(), fx["pass"]);
    assert_eq!(verify_file_pass(&issuer, &server, &grantee, &pass, minted), Ok(()));

    let f = &fx["file"];
    let size = f["size"].as_u64().unwrap();
    let dir = std::env::temp_dir().join(format!("hum_offer_fixture_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("f.bin");
    std::fs::write(&path, (0..size).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
    let h = hash_file(&path, &AtomicU64::new(0), &AtomicBool::new(false)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let piece_hex: Vec<String> = h.pieces.iter().map(hex::encode).collect();
    let want_pieces: Vec<String> = f["piece_hashes"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(piece_hex, want_pieces, "each piece's hash");
    assert_eq!(h.whole, f["blake3"].as_str().unwrap());
    assert_eq!(hashes_digest(&h.pieces), f["hashes"].as_str().unwrap());
    let offer = Offer {
        id: id.clone(),
        name: f["name"].as_str().unwrap().into(),
        size,
        mime: f["type"].as_str().unwrap().into(),
        blake3: h.whole.clone(),
        piece: f["piece"].as_u64().unwrap(),
        pieces: f["pieces"].as_u64().unwrap(),
        hashes: hashes_digest(&h.pieces),
        pass,
    };
    assert_eq!(offer.text(), s("offer_note"), "the offer note");
    assert_eq!(parse_note(&s("offer_note")), Some(Note::Offer(offer)), "and it reads back");
    assert_eq!(OfferAnswer { id: id.clone(), accept: true }.text(), s("accept_note"));
    assert_eq!(OfferAnswer { id: id.clone(), accept: false }.text(), s("decline_note"));
    assert_eq!(OfferCancel { id }.text(), s("cancel_note"));
}

/// A PASS IS GOOD ONLY FOR ITS TWO PEOPLE, THE WAY ROUND IT WAS GIVEN, ITS FILE AND ITS SERVER, and
/// only within its 7 days (a day of slack when checked, none when minted).
/// Seen red 2026-10-11 with `verify_file_pass` signing words without the grantee (`grantee`
/// dropped from the preimage it rebuilds): "the right pass checks" failed, BadSignature.
#[test]
fn a_pass_checks_only_for_its_two_people_its_file_and_its_server() {
    let (s_seed, sender) = identity(41);
    let (_r_seed, receiver) = identity(42);
    let now = 1_760_000_000;
    let pass = mint_file_pass(&s_seed, SERVER, &sender, &receiver, ID, now + FILE_PASS_MAX_SECS, now).unwrap();
    assert_eq!((pass.v, pass.id.as_str(), pass.expiry), (1, ID, now + FILE_PASS_MAX_SECS));
    assert_eq!(verify_file_pass(&sender, SERVER, &receiver, &pass, now), Ok(()), "the right pass checks");
    assert_eq!(verify_file_pass(&receiver, SERVER, &sender, &pass, now), Err(PassError::BadSignature), "the wrong way round");
    assert_eq!(verify_file_pass(&sender, "did:hum:elsewhere", &receiver, &pass, now), Err(PassError::BadSignature), "another server");
    let other_file = FilePass { id: "ff".repeat(16), ..pass.clone() };
    assert_eq!(verify_file_pass(&sender, SERVER, &receiver, &other_file, now), Err(PassError::BadSignature), "another file");
    assert_eq!(verify_file_pass(&sender, SERVER, &receiver, &pass, now + FILE_PASS_MAX_SECS), Err(PassError::Expired), "run out");
    assert_eq!(verify_file_pass(&sender, SERVER, &receiver, &pass, now - FILE_PASS_CLOCK_SLACK_SECS), Ok(()), "a day of slack on the checker's clock");
    assert_eq!(verify_file_pass(&sender, SERVER, &receiver, &pass, now - FILE_PASS_CLOCK_SLACK_SECS - 1), Err(PassError::TooLong));
    assert_eq!(mint_file_pass(&s_seed, SERVER, &sender, &receiver, ID, now + FILE_PASS_MAX_SECS + 1, now), Err(PassError::TooLong), "no slack when minting");
    assert_eq!(mint_file_pass(&s_seed, SERVER, &sender, &receiver, "not-an-id", now + 60, now), Err(PassError::Malformed));
}

/// An offer reads back as written; one this app does not take is not read at all (wrong piece size
/// or count, digests that are not 64 lowercase hex, a pass for another transfer, nothing to send,
/// past the size this app holds a list for). Answers and cancels read too.
/// Seen red 2026-10-11 with the piece-count check taken out of `Offer::check`: "pieces that do not
/// add up" read as an offer.
#[test]
fn an_offer_reads_back_and_one_that_does_not_add_up_is_not_read() {
    let o = spec_offer();
    assert_eq!(parse_note(&o.text()), Some(Note::Offer(o.clone())));
    assert!(is_note(&o.text()));
    let bad = |f: &dyn Fn(&mut Offer)| {
        let mut b = spec_offer();
        f(&mut b);
        parse_note(&b.text())
    };
    assert_eq!(bad(&|b| b.pieces += 1), None, "pieces that do not add up");
    assert_eq!(bad(&|b| b.piece = 65_536), None, "another piece size");
    assert_eq!(bad(&|b| b.blake3 = "AB".repeat(32)), None, "upper-case hex");
    assert_eq!(bad(&|b| b.hashes = "cd".repeat(31)), None, "a short digest");
    assert_eq!(bad(&|b| b.pass.id = "ff".repeat(16)), None, "a pass for another transfer");
    assert_eq!(bad(&|b| { b.size = 0; b.pieces = 0; }), None, "nothing to send");
    assert_eq!(bad(&|b| { b.size = MAX_OFFER_BYTES + 1; b.pieces = pieces_for(b.size); }), None, "past the size held");
    assert_eq!(bad(&|b| b.name = String::new()), None, "no name");
    assert_eq!(parse_note(&format!("[[hum:offer-answer:v1]]{{\"id\":\"{ID}\",\"accept\":true}}")), Some(Note::Answer(OfferAnswer { id: ID.into(), accept: true })));
    assert_eq!(parse_note(&format!("[[hum:offer-cancel:v1]]{{\"id\":\"{ID}\"}}")), Some(Note::Cancel(OfferCancel { id: ID.into() })));
    assert_eq!(parse_note("[[hum:offer-cancel:v1]]{\"id\":\"xyz\"}"), None, "a cancel naming no transfer");
    assert!(is_note("[[hum:offer:v1]]not json"), "a broken note is still never shown");
    assert!(!is_note("I said [[hum:offer:v1]]"));
}

/// THE RECEIVER APPLIES "FILES IN MESSAGES" BEFORE ASKING (spec part 3): someone this person does
/// not take files from gets the one-file card, and only its Accept leads on to the question; a
/// friend under "Friends", or anyone under "Anyone I talk to", is asked at once; under the
/// protected setup a non-friend's offer is left out whatever was pressed.
/// Seen red 2026-10-11 with the Files in messages check taken out of `question` (`if false && ...`):
/// "a stranger's offer is the card first" got the question.
#[test]
fn the_receiver_applies_files_in_messages_before_asking() {
    let ask = Question::Ask("Ann wants to send you clip.mp4 (2.3 GB). Accept?".into());
    let q = |from, friend, hides, let_in| question(from, friend, hides, let_in, "Ann", "clip.mp4", 2_468_000_000);
    assert_eq!(
        q(AcceptFrom::Friends, false, false, false),
        Question::NotAccepted("Ann wants to send you a file. You accept files only from friends. Accept this one?".into()),
        "a stranger's offer is the card first"
    );
    assert_eq!(q(AcceptFrom::Friends, false, false, true), ask, "after Accept this one, the question");
    assert_eq!(q(AcceptFrom::Friends, true, false, false), ask, "a friend is asked at once");
    assert_eq!(q(AcceptFrom::Anyone, false, false, false), ask, "anyone I talk to");
    assert_eq!(
        q(AcceptFrom::Nobody, true, false, false),
        Question::NotAccepted("Ann wants to send you a file. You do not accept files in messages. Accept this one?".into()),
        "nobody, a friend included"
    );
    assert_eq!(q(AcceptFrom::Anyone, false, true, true), Question::LeftOut, "the protected setup leaves it out");
}

/// NOTES MOVE A TRANSFER FORWARD ONLY. An offer is listed once (its pass checked); an answer counts
/// only while the offer waits for one and only from the receiver; a cancel from either of the two
/// ends it; nothing changes an ended transfer (a relay that delivers a note twice cannot undo a
/// cancel); a self-copy lists my offer as not on this device; a note naming other people changes
/// nothing.
/// Seen red 2026-10-11 with `advance` letting `Declined` follow `Accepted` (the `(Stage::Offered,
/// Happened::Declined)` arm widened to any stage): "a decline after acceptance changes nothing" failed.
#[test]
fn notes_move_a_transfer_forward_only() {
    let (me, ann, cy) = ("me", "ann", "cy");
    let mut recs = Vec::new();
    let offer = || Note::Offer(spec_offer());
    let yes = |accept| Note::Answer(OfferAnswer { id: ID.into(), accept });
    let off = || Note::Cancel(OfferCancel { id: ID.into() });

    assert_eq!(apply_note(&mut recs, me, ann, me, 5, offer(), |_| false), Applied::Nothing, "a pass that does not check");
    assert_eq!(apply_note(&mut recs, me, ann, me, 5, offer(), |_| true), Applied::Listed(ID.into()));
    assert_eq!(apply_note(&mut recs, me, ann, me, 6, offer(), |_| true), Applied::Nothing, "listed once");
    assert!(!recs[0].outgoing && !recs[0].here && recs[0].peer == ann && recs[0].at == 5);
    assert_eq!(apply_note(&mut recs, me, ann, me, 7, yes(true), |_| true), Applied::Nothing, "the sender cannot answer their own offer");
    assert_eq!(apply_note(&mut recs, me, cy, me, 7, off(), |_| true), Applied::Nothing, "someone else cannot cancel it");
    assert_eq!(apply_note(&mut recs, me, me, ann, 8, yes(true), |_| true), Applied::Moved(ID.into(), Stage::Accepted), "accepted on my other device");
    assert_eq!(apply_note(&mut recs, me, me, ann, 9, yes(false), |_| true), Applied::Nothing, "a decline after acceptance changes nothing");
    assert_eq!(apply_note(&mut recs, me, ann, me, 10, off(), |_| true), Applied::Moved(ID.into(), Stage::Cancelled { by_me: false }));
    assert_eq!(apply_note(&mut recs, me, me, ann, 11, yes(true), |_| true), Applied::Nothing, "nothing brings it back");

    // My own offer, from my other device: listed as mine, not on this device; Ann declines it.
    let mut mine = Vec::new();
    assert_eq!(apply_note(&mut mine, me, me, cy, 5, offer(), |_| false), Applied::Listed(ID.into()), "my own offers need no check");
    assert!(mine[0].outgoing && !mine[0].here && mine[0].peer == cy);
    assert_eq!(apply_note(&mut mine, me, cy, me, 6, yes(false), |_| true), Applied::Moved(ID.into(), Stage::Declined));
    assert_eq!(apply_note(&mut mine, me, ann, cy, 7, off(), |_| true), Applied::Nothing, "a note between other people");
    assert_eq!(advance(&Stage::Accepted, &Happened::Done), Some(Stage::Done));
}

/// RULE 9 OF THE SECOND REVIEW (media-showcase.md, "a dial is an answer"): `done` on the channel
/// ends a transfer as Done even while its record still waits for the answer note (Offered), so a
/// sender whose answer note was lost is Sent, not left to end as "did not answer in time". Nothing
/// else about an ended transfer changes, and an answer note arriving after it changes nothing.
/// Seen red 2026-10-12 with the `(Stage::Offered, Happened::Done)` arm taken out of `advance` (as
/// before the review): "done from Offered" was None.
#[test]
fn done_ends_a_transfer_still_waiting_for_its_answer() {
    assert_eq!(advance(&Stage::Offered, &Happened::Done), Some(Stage::Done), "done from Offered");
    assert_eq!(advance(&Stage::Accepted, &Happened::Done), Some(Stage::Done));
    assert_eq!(advance(&Stage::Done, &Happened::Accepted), None, "the lost answer arriving late changes nothing");
    assert_eq!(advance(&Stage::Declined, &Happened::Done), None, "an ended transfer stays ended");
    assert_eq!(advance(&Stage::Expired, &Happened::Done), None);
}

/// The hashes of a file read for its offer: the whole file's BLAKE3, each piece's, and the digest
/// of the list, the last piece short.
/// Seen red 2026-10-11 with `hash_file` hashing the whole piece buffer instead of the bytes read
/// (`&buf` for `&buf[..n]`): "the last piece is short" failed.
#[test]
fn a_file_is_read_for_its_hashes() {
    let dir = std::env::temp_dir().join(format!("hum_offer_hash_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("f.bin");
    let bytes: Vec<u8> = (0..(PIECE_SIZE as usize * 2 + 1000)).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let (done, stop) = (AtomicU64::new(0), AtomicBool::new(false));
    let h = hash_file(&path, &done, &stop).unwrap();
    assert_eq!(h.size, bytes.len() as u64);
    assert_eq!(h.whole, blake3::hash(&bytes).to_hex().to_string());
    assert_eq!(h.pieces.len(), 3);
    assert_eq!(&h.pieces[2], blake3::hash(&bytes[2 * PIECE_SIZE as usize..]).as_bytes(), "the last piece is short");
    assert_eq!(done.load(Ordering::Relaxed), bytes.len() as u64);
    assert_eq!(piece_len(bytes.len() as u64, 2), 1000);
    let _ = std::fs::remove_dir_all(&dir);
}

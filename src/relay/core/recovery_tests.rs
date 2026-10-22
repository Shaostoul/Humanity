// Child of relay/core/recovery.rs: the protocol's own tests, and the half of the cross-language
// fixture (scripts/tests/fixtures/recovery-v1.json) this side reads. Each test was seen failing
// once; what it saw red is at the test.

use super::*;
use crate::relay::core::pq_crypto::derive_kyber_seed;

fn random_secret() -> [u8; SECRET_LEN] {
    let mut s = [0u8; SECRET_LEN];
    getrandom::getrandom(&mut s).unwrap();
    s
}

/// Every way of choosing `k` items of `n`, as index lists.
fn choose(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn go(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            go(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(0, n, k, &mut Vec::new(), &mut out);
    out
}

/// A person's keys from a fixed 32-byte secret, the way both apps derive them.
fn person(secret: &[u8; 32]) -> (DilithiumKeypair, KyberKeypair, String) {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(secret));
    let kyb = KyberKeypair::from_seed(&derive_kyber_seed(secret)).unwrap();
    let did = did_for_pubkey(&dil.public_key());
    (dil, kyb, did)
}

/// SPLIT AND JOIN ROUND TRIP: every choice of `needed` pieces, and all of them, give back the
/// secret, for every shape a setup may have.
///
/// Seen red 2026-10-21 with `combine` handing a repeated piece to blahaj twice (its dedupe taken
/// out): "a repeated piece counts once" (two copies of one point cancel in Lagrange's sum, so
/// the join of [a, a, b] was not the secret; blahaj counts distinct x but sums every share).
#[test]
fn split_and_join_round_trip_for_every_shape() {
    for total in MIN_PEOPLE..=MAX_PEOPLE {
        for needed in MIN_NEEDED..=total {
            let secret = random_secret();
            let shares = split(&secret, needed, total).unwrap();
            assert_eq!(shares.len(), total);
            for (i, s) in shares.iter().enumerate() {
                assert_eq!(s[0] as usize, i + 1, "slot {i} sits at x = {}", i + 1);
            }
            for pick in choose(total, needed).into_iter().take(40) {
                let chosen: Vec<_> = pick.iter().map(|&i| shares[i]).collect();
                assert_eq!(combine(&chosen, needed).unwrap(), secret, "{needed} of {total}, pieces {pick:?}");
            }
            assert_eq!(combine(&shares, needed).unwrap(), secret, "all {total} pieces");
        }
    }
    let secret = random_secret();
    let shares = split(&secret, 2, 3).unwrap();
    assert_eq!(combine(&[shares[0], shares[0], shares[1]], 2).unwrap(), secret, "a repeated piece counts once");
    assert!(combine(&[shares[0], shares[0]], 2).is_err(), "one piece twice is still one piece");
    let mut zero = shares[0];
    zero[0] = 0;
    assert!(combine(&[zero, shares[1]], 2).is_err(), "a piece at x = 0 is refused");
}

/// FEWER THAN NEEDED REVEALS NOTHING USABLE: below the threshold the join is refused, and the
/// pieces it would have had leave EVERY value of every secret byte possible. With two pieces
/// of a 3-of-5 split, a third point at x = 3 that runs over all 256 values gives 256 different
/// values of each secret byte: the two pieces fix nothing about the secret.
///
/// Seen red 2026-10-21 with `combine` joining with as many pieces as it was given (its own check
/// out and `Sharks(kept.len())` for `Sharks(needed)`): "two pieces of a 3-of-5 are refused".
#[test]
fn fewer_than_needed_reveals_nothing_usable() {
    let secret = random_secret();
    let shares = split(&secret, 3, 5).unwrap();
    assert!(combine(&shares[..2], 3).is_err(), "two pieces of a 3-of-5 are refused");
    for byte in [0usize, 7, 31] {
        let mut values = std::collections::BTreeSet::new();
        for y in 0..=255u8 {
            let mut third = shares[2];
            third[1 + byte] = y;
            values.insert(combine(&[shares[0], shares[1], third], 3).unwrap()[byte]);
        }
        assert_eq!(values.len(), 256, "secret byte {byte}: two pieces leave every value possible");
    }
    // Joining two pieces as if two were enough gives a value that is not the secret.
    let wrong = combine(&shares[..2], 2).unwrap();
    assert_ne!(wrong, secret, "two pieces of a 3-of-5 do not give the secret");
}

/// SEALING: what is sealed to a key opens only with that key and the same associated data, and
/// any change to the bytes is caught.
///
/// Seen red 2026-10-21 with `seal` and `open` both leaving the associated data out: "other words
/// do not open it".
#[test]
fn sealed_opens_only_for_its_key_and_its_words() {
    let (_, me, _) = person(&[3u8; 32]);
    let (_, other, _) = person(&[4u8; 32]);
    let sealed = seal(PIECE_DOMAIN, &me.public_key(), b"the words", b"hello").unwrap();
    assert_eq!(sealed.len(), SEAL_OVERHEAD + 5);
    assert_eq!(open(PIECE_DOMAIN, &me, b"the words", &sealed).as_deref(), Some(&b"hello"[..]));
    assert!(open(PIECE_DOMAIN, &other, b"the words", &sealed).is_none(), "another key does not open it");
    assert!(open(PIECE_DOMAIN, &me, b"other words", &sealed).is_none(), "other words do not open it");
    assert!(open(NOTE_DOMAIN, &me, b"the words", &sealed).is_none(), "another use does not open it");
    let mut bent = sealed.clone();
    *bent.last_mut().unwrap() ^= 1;
    assert!(open(PIECE_DOMAIN, &me, b"the words", &bent).is_none(), "a changed byte is caught");
    assert!(open(PIECE_DOMAIN, &me, b"the words", &sealed[..10]).is_none());
}

/// THE CODE: 16 characters of Crockford's alphabet, the same for the same account and key and
/// different for another key; read back in any case with dashes and spaces, O as 0 and I or L as
/// 1; refused when it is not 16 characters of the alphabet.
///
/// Seen red 2026-10-21 with `normalize_code` not mapping O to 0: "a code read with O for zero".
#[test]
fn the_code_names_one_account_and_one_key() {
    let (_, kp, did) = person(&[5u8; 32]);
    let code = ask_code(&did, &kp.public_key());
    assert_eq!(code.len(), CODE_CHARS);
    assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)));
    assert_eq!(code, ask_code(&did, &kp.public_key()));
    let (_, other, other_did) = person(&[6u8; 32]);
    assert_ne!(code, ask_code(&did, &other.public_key()), "another key, another code");
    assert_ne!(code, ask_code(&other_did, &kp.public_key()), "another account, another code");
    let shown = show_code(&code);
    assert_eq!(shown.len(), 19);
    assert_eq!(normalize_code(&shown.to_lowercase()), Some(code.clone()));
    assert_eq!(normalize_code(&format!(" {} ", shown.replace('-', " "))), Some(code.clone()));
    assert_eq!(normalize_code("OOOO-IIII-LLLL-0000"), Some("0000111111110000".into()), "a code read with O for zero");
    assert_eq!(normalize_code("UUUU-0000-0000-0000"), None, "U is not in the alphabet");
    assert_eq!(normalize_code("0000-0000-0000"), None, "too short");
    assert_eq!(normalize_code("0000-0000-0000-0000-0"), None, "too long");
    assert_eq!(normalize_code("ÉÉÉÉ-0000-0000-0000"), None);
    assert_eq!(base32(&[0u8; 10]), "0000000000000000");
    assert_eq!(base32(&[0xffu8; 10]), "ZZZZZZZZZZZZZZZZ");
    assert_eq!(base32(&[0x08, 0x86, 0x42, 0x98, 0xe8]), "12345678", "the digits 1 to 8, five bits each");
}

/// A SETUP, END TO END: every object verifies and reads back from its served JSON; each trusted
/// person opens their own piece and nobody else's; the person's own notes say who keeps which;
/// a piece sent back to a new device opens there; and any `needed` of them rebuild the account.
///
/// Seen red 2026-10-21 with `open_piece` not checking the token against its public hash (a
/// piece whose token had been swapped opened): "a piece with another token does not open".
#[test]
fn a_setup_end_to_end() {
    let secret = [0x21u8; 32];
    let (holder, holder_kyber, holder_did) = person(&secret);
    let people: Vec<([u8; 32], DilithiumKeypair, KyberKeypair, String)> = (0..4u8)
        .map(|i| {
            let s = [0x40 + i; 32];
            let (d, k, did) = person(&s);
            (s, d, k, did)
        })
        .collect();
    let trusted: Vec<TrustedPerson> = people
        .iter()
        .enumerate()
        .map(|(i, (_, _, k, did))| TrustedPerson { did: did.clone(), name: format!("Friend {i}"), kyber_public: k.public_key() })
        .collect();
    let (set_id, objects) = build_setup(&secret, &holder, &holder_kyber.public_key(), &trusted, 3, 1_700_000_000_000).unwrap();
    assert!(set_id_ok(&set_id));
    assert_eq!(objects.len(), 4);
    let served: Vec<ServedPiece> = objects.iter().map(|o| served_piece(&object_json(o)).unwrap()).collect();
    let mut opened = Vec::new();
    for (slot, p) in served.iter().enumerate() {
        assert_eq!(p.holder_did, holder_did);
        assert_eq!((p.payload.slot, p.payload.needed, p.payload.total), (slot, 3, 4));
        assert_eq!(p.payload.piece.len(), PIECE_LEN);
        assert_eq!(p.payload.note.len(), NOTE_LEN);
        for (other, (_, _, k, _)) in people.iter().enumerate() {
            assert_eq!(open_piece(k, p).is_some(), other == slot, "slot {slot} opens only for its own person, not {other}");
        }
        assert!(open_piece(&holder_kyber, p).is_none(), "the person cannot open their own trusted people's pieces");
        let note = open_note(&holder_kyber, p).expect("the person's own note opens");
        assert_eq!(note, NoteToSelf { did: people[slot].3.clone(), name: format!("Friend {slot}") });
        assert!(open_note(&people[slot].2, p).is_none(), "the trusted person cannot read the note");
        opened.push(open_piece(&people[slot].2, p).unwrap());
    }
    // A piece whose token was swapped does not open.
    let mut swapped = served[0].clone();
    swapped.payload.token_hash = [9u8; 32];
    assert!(open_piece(&people[0].2, &swapped).is_none(), "a piece with another token does not open");

    // Asking: a one-time key, its code, the pieces sealed back to it.
    let ask = OneTimeAsk::new().unwrap();
    let code = ask_code(&holder_did, &ask.public);
    let answers: Vec<(usize, Vec<u8>)> = opened
        .iter()
        .enumerate()
        .map(|(slot, o)| (slot, seal_answer(&ask.public, &code, &holder_did, slot, &o.share).unwrap()))
        .collect();
    for (slot, sealed) in &answers {
        assert_eq!(sealed.len(), ANSWER_LEN);
        assert!(open_answer(&ask.keys, &code, &holder_did, (slot + 1) % 4, sealed).is_none(), "an answer is bound to its slot");
        assert!(open_answer(&ask.keys, "0000000000000000", &holder_did, *slot, sealed).is_none(), "an answer is bound to its ask");
    }
    let back: Vec<[u8; SHARE_LEN]> = answers.iter().map(|(slot, s)| open_answer(&ask.keys, &code, &holder_did, *slot, s).unwrap()).collect();
    for pick in choose(4, 3) {
        let chosen: Vec<_> = pick.iter().map(|&i| back[i]).collect();
        assert_eq!(rebuild(&chosen, 3, &holder_did), Some(secret), "pieces {pick:?}");
    }
    assert_eq!(rebuild(&back[..2], 3, &holder_did), None, "two of a 3-of-4 rebuild nothing");
    assert_eq!(rebuild(&back, 3, &people[0].3), None, "the pieces do not rebuild another account");
}

/// A WRONG PIECE AMONG THE RIGHT ONES: with one piece from another setup in the pile, the
/// rebuild still finds the account (it tries every choice of `needed`), and with too few right
/// ones it finds nothing rather than a wrong account.
///
/// Seen red 2026-10-21 with `rebuild` trying only the first `needed` pieces: "one wrong piece
/// first, three right ones after".
#[test]
fn a_wrong_piece_among_the_right_ones() {
    let secret = random_secret();
    let did = account_of(&secret);
    let shares = split(&secret, 3, 5).unwrap();
    let stray = split(&random_secret(), 3, 5).unwrap();
    let pile = vec![stray[0], shares[1], shares[2], shares[3]];
    assert_eq!(rebuild(&pile, 3, &did), Some(secret), "one wrong piece first, three right ones after");
    let thin = vec![stray[0], shares[1], shares[2]];
    assert_eq!(rebuild(&thin, 3, &did), None);
}

/// THE PAYLOAD IS READ STRICTLY: exactly the seven fields, a shape this protocol allows, the
/// slot inside it and every sealed part its exact length; anything else is refused with words.
///
/// Seen red 2026-10-21 with the piece's length check taken out of `parse`: "a short piece".
#[test]
fn the_payload_is_read_strictly() {
    let good = SharePayload {
        set_id: "0123456789abcdef0123456789abcdef".into(),
        slot: 1,
        needed: 2,
        total: 3,
        piece: vec![1; PIECE_LEN],
        note: vec![2; NOTE_LEN],
        token_hash: [3; 32],
    };
    let bytes = |v: &Value| crate::relay::core::encoding::to_canonical_bytes(v).unwrap();
    assert_eq!(SharePayload::parse(&bytes(&good.to_cbor())).unwrap(), good);
    let with = |f: &dyn Fn(&mut SharePayload)| {
        let mut p = good.clone();
        f(&mut p);
        SharePayload::parse(&bytes(&p.to_cbor()))
    };
    assert!(with(&|p| p.slot = 3).is_err(), "slot outside the setup");
    assert!(with(&|p| p.needed = 1).is_err(), "one alone is never enough");
    assert!(with(&|p| p.needed = 4).is_err(), "more needed than there are");
    assert!(with(&|p| { p.total = 11; p.slot = 0 }).is_err(), "more than ten");
    assert!(with(&|p| p.piece.pop().map(|_| ()).unwrap_or(())).is_err(), "a short piece");
    assert!(with(&|p| p.note.push(0)).is_err(), "a long note");
    assert!(with(&|p| p.set_id = "0123456789ABCDEF0123456789ABCDEF".into()).is_err(), "an uppercase setup id");
    let Value::Map(mut entries) = good.to_cbor() else { unreachable!() };
    entries.push((Value::Text("guardian_did".into()), Value::Text("did:hum:x".into())));
    assert!(SharePayload::parse(&bytes(&Value::Map(entries.clone()))).is_err(), "an extra field is refused");
    entries.retain(|(k, _)| k.as_text() != Some("token_hash") && k.as_text() != Some("guardian_did"));
    assert!(SharePayload::parse(&bytes(&Value::Map(entries))).is_err(), "a missing field is refused");
    assert!(SharePayload::parse(b"not cbor").is_err());
}

/// A SETUP REFUSES WHAT CANNOT BE RIGHT: a phrase that is not the account's, the person as their
/// own trusted person, someone chosen twice, a key that is not known, a shape out of bounds.
///
/// Seen red 2026-10-21 with the account check taken out of `build_setup`: "another account's
/// phrase is refused".
#[test]
fn a_setup_refuses_what_cannot_be_right() {
    let secret = [0x31u8; 32];
    let (holder, hk, holder_did) = person(&secret);
    let (_, ak, adid) = person(&[0x32u8; 32]);
    let (_, bk, bdid) = person(&[0x33u8; 32]);
    let a = TrustedPerson { did: adid.clone(), name: "A".into(), kyber_public: ak.public_key() };
    let b = TrustedPerson { did: bdid, name: "B".into(), kyber_public: bk.public_key() };
    let hpk = hk.public_key();
    assert!(build_setup(&secret, &holder, &hpk, &[a.clone(), b.clone()], 2, 1).is_ok());
    assert!(build_setup(&[0x30u8; 32], &holder, &hpk, &[a.clone(), b.clone()], 2, 1).is_err(), "another account's phrase is refused");
    let me = TrustedPerson { did: holder_did, name: "Me".into(), kyber_public: hpk.clone() };
    assert!(build_setup(&secret, &holder, &hpk, &[a.clone(), me], 2, 1).is_err(), "not your own trusted person");
    assert!(build_setup(&secret, &holder, &hpk, &[a.clone(), a.clone()], 2, 1).is_err(), "not twice");
    let unknown = TrustedPerson { kyber_public: vec![], ..b.clone() };
    assert!(build_setup(&secret, &holder, &hpk, &[a.clone(), unknown], 2, 1).is_err(), "a key that is not known");
    assert!(build_setup(&secret, &holder, &hpk, &[a.clone(), b.clone()], 3, 1).is_err(), "3 of 2");
    assert!(build_setup(&secret, &holder, &hpk, &[a], 1, 1).is_err(), "1 of 1");
}

/// A note too long for its space is cut at a character, never mid-character, and reads back.
#[test]
fn a_long_name_fits_its_note() {
    let note = NoteToSelf { did: "did:hum:AbCdEfGhIjKlMnOpQrStUv".into(), name: "é".repeat(300) };
    let plain = note_plain(&note);
    assert_eq!(plain.len(), NOTE_PLAIN_LEN);
    let back = parse_note(&plain).unwrap();
    assert_eq!(back.did, note.did);
    assert!(back.name.chars().all(|c| c == 'é') && !back.name.is_empty());
}

// ── The cross-language fixture ──
//
// scripts/tests/fixtures/recovery-v1.json holds what each side made, and each side reads what
// the other made. This side writes its half when HOS_WRITE_RECOVERY_FIXTURE=1
// (`node scripts/lib-tests.js recovery_fixture` or the relay test build with the same filter);
// scripts/tests/recovery-web.test.js writes the web half with the same variable.

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/recovery-v1.json")
}

fn fixture() -> serde_json::Value {
    let text = std::fs::read_to_string(fixture_path()).expect("the recovery fixture is checked in");
    serde_json::from_str(&text).expect("the recovery fixture is JSON")
}

fn hexs(v: &serde_json::Value) -> Vec<u8> {
    hex::decode(v.as_str().expect("hex text")).expect("hex")
}

fn share_of(v: &serde_json::Value) -> [u8; SHARE_LEN] {
    hexs(v).try_into().expect("a 33-byte share")
}

/// The fixed people of the fixture's setups: the holder from secret 0x21.., the trusted people
/// from 0x40.., 0x41.., 0x42.. (keys derived the way both apps derive them).
fn fixture_people() -> ([u8; 32], Vec<[u8; 32]>) {
    ([0x21u8; 32], (0..3u8).map(|i| [0x40 + i; 32]).collect())
}

/// Write this side's half of the fixture (only with HOS_WRITE_RECOVERY_FIXTURE=1).
#[test]
fn recovery_fixture_write_rust_half() {
    if std::env::var("HOS_WRITE_RECOVERY_FIXTURE").as_deref() != Ok("1") {
        return;
    }
    let mut fx: serde_json::Value = std::fs::read_to_string(fixture_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let secret = random_secret();
    let shares = split(&secret, 3, 5).unwrap();
    let (hs, ts) = fixture_people();
    let (holder, hk, holder_did) = person(&hs);
    let trusted: Vec<TrustedPerson> = ts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let (_, k, did) = person(s);
            TrustedPerson { did, name: format!("Friend {i}"), kyber_public: k.public_key() }
        })
        .collect();
    let (_, objects) = build_setup(&hs, &holder, &hk.public_key(), &trusted, 2, 1_761_000_000_000).unwrap();
    // A piece sent back: trusted person 1 answers an ask.
    let (_, k1, _) = person(&ts[1]);
    let p1 = served_piece(&object_json(&objects[1])).unwrap();
    let opened = open_piece(&k1, &p1).unwrap();
    let one_time_seed = [0x77u8; KYBER_SEED_LEN];
    let one_time = KyberKeypair::from_seed(&one_time_seed).unwrap();
    let code = ask_code(&holder_did, &one_time.public_key());
    let answer = seal_answer(&one_time.public_key(), &code, &holder_did, 1, &opened.share).unwrap();
    fx["rust"] = serde_json::json!({
        "split": {
            "secret_hex": hex::encode(secret),
            "needed": 3,
            "total": 5,
            "shares_hex": shares.iter().map(hex::encode).collect::<Vec<_>>(),
        },
        "setup": {
            "holder_secret_hex": hex::encode(hs),
            "trusted_secrets_hex": ts.iter().map(hex::encode).collect::<Vec<_>>(),
            "needed": 2,
            "objects": objects.iter().map(object_json).collect::<Vec<_>>(),
        },
        "answer": {
            "one_time_seed_hex": hex::encode(one_time_seed),
            "holder_did": holder_did,
            "code": code,
            "slot": 1,
            "sealed_hex": hex::encode(answer),
        },
        "codes": (0..3u8).map(|i| {
            let k = KyberKeypair::from_seed(&[0x90 + i; KYBER_SEED_LEN]).unwrap();
            let did = account_of(&[0x50 + i; 32]);
            serde_json::json!({
                "holder_did": did,
                "kyber_seed_hex": hex::encode([0x90 + i; KYBER_SEED_LEN]),
                "code": ask_code(&did, &k.public_key()),
            })
        }).collect::<Vec<_>>(),
    });
    std::fs::create_dir_all(fixture_path().parent().unwrap()).unwrap();
    std::fs::write(fixture_path(), serde_json::to_string_pretty(&fx).unwrap() + "\n").unwrap();
}

/// RUST AND THE WEBSITE AGREE (Rust's side): the pieces the website's split made join here to
/// its secret, every way of choosing `needed`; a setup the website built reads, verifies and
/// opens here, and its pieces rebuild the account; an answer the website sealed opens here;
/// the codes the website computed are the codes computed here; and this side's own half still
/// reads back (the fixture is not stale).
///
/// Seen red 2026-10-21 with the website's GF(256) built on 0x11b (the AES polynomial) instead of
/// blahaj's 0x11d: "the website's pieces join here" (the joined bytes were not its secret).
#[test]
fn recovery_fixture_rust_and_the_website_agree() {
    let fx = fixture();
    let web = &fx["web"];
    assert!(web.is_object(), "the fixture carries the website's half (scripts/tests/recovery-web.test.js writes it)");

    // The website's split joins here.
    let split = &web["split"];
    let secret = hexs(&split["secret_hex"]);
    let needed = split["needed"].as_u64().unwrap() as usize;
    let shares: Vec<[u8; SHARE_LEN]> = split["shares_hex"].as_array().unwrap().iter().map(share_of).collect();
    for pick in choose(shares.len(), needed) {
        let chosen: Vec<_> = pick.iter().map(|&i| shares[i]).collect();
        assert_eq!(combine(&chosen, needed).unwrap().to_vec(), secret, "the website's pieces join here: {pick:?}");
    }

    // The website's setup reads, verifies and opens here.
    let setup = &web["setup"];
    let hs: [u8; 32] = hexs(&setup["holder_secret_hex"]).try_into().unwrap();
    let (_, hk, holder_did) = person(&hs);
    let ts: Vec<[u8; 32]> = setup["trusted_secrets_hex"].as_array().unwrap().iter().map(|v| hexs(v).try_into().unwrap()).collect();
    let needed = setup["needed"].as_u64().unwrap() as usize;
    let mut back = Vec::new();
    for (slot, o) in setup["objects"].as_array().unwrap().iter().enumerate() {
        let p = served_piece(o).expect("the website's object verifies here");
        assert_eq!(p.holder_did, holder_did);
        assert_eq!(p.payload.slot, slot);
        let (_, k, did) = person(&ts[slot]);
        let opened = open_piece(&k, &p).expect("the trusted person opens the website's piece here");
        let note = open_note(&hk, &p).expect("the website's note to self opens here");
        assert_eq!(note.did, did);
        back.push(opened.share);
    }
    assert_eq!(rebuild(&back[..needed], needed, &holder_did), Some(hs), "the website's setup rebuilds the account here");

    // The website's answer opens here.
    let a = &web["answer"];
    let seed: [u8; KYBER_SEED_LEN] = hexs(&a["one_time_seed_hex"]).try_into().unwrap();
    let kp = KyberKeypair::from_seed(&seed).unwrap();
    let code = a["code"].as_str().unwrap();
    let did = a["holder_did"].as_str().unwrap();
    assert_eq!(ask_code(did, &kp.public_key()), code, "the website's code for its ask is this side's");
    let slot = a["slot"].as_u64().unwrap() as usize;
    let share = open_answer(&kp, code, did, slot, &hexs(&a["sealed_hex"])).expect("the website's answer opens here");
    assert_eq!(share[0] as usize, slot + 1);

    // The website's codes are this side's.
    for c in web["codes"].as_array().unwrap() {
        let k = KyberKeypair::from_seed(&hexs(&c["kyber_seed_hex"]).try_into().unwrap()).unwrap();
        assert_eq!(ask_code(c["holder_did"].as_str().unwrap(), &k.public_key()), c["code"].as_str().unwrap());
    }

    // This side's own half still holds.
    let rust = &fx["rust"];
    let rs = hexs(&rust["split"]["secret_hex"]);
    let shares: Vec<[u8; SHARE_LEN]> = rust["split"]["shares_hex"].as_array().unwrap().iter().map(share_of).collect();
    assert_eq!(combine(&shares[..3], 3).unwrap().to_vec(), rs);
    for o in rust["setup"]["objects"].as_array().unwrap() {
        served_piece(o).expect("this side's own fixture objects still verify");
    }
}

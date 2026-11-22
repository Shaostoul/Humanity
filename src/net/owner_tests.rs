// Child of net/owner.rs (#[path]): the owner's rules in the desktop app, with no socket and no GUI.
// Each test was seen red once on purpose, recorded at the test.

use super::*;
use std::collections::BTreeMap;
use crate::net::held::tests::{owner_keys, package, submission_object};
use humanity_protocol::held_seal::{header_len, open_stream, part_context};
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

/// A person's key (their Dilithium3 public key, hex) from a test seed.
pub(crate) fn key_of(seed: u8) -> String {
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32])).public_key())
}

/// A person's key card from a test seed, dated `at`, as JSON.
pub(crate) fn card_of(seed: u8, at: u64) -> Value {
    key_card::card_json(&key_card::sign_card(&[seed; 32], at).unwrap())
}

/// The text fields of a signed action's payload.
pub(crate) fn payload_text(o: &humanity_protocol::object::Object) -> std::collections::HashMap<String, String> {
    let v: ciborium::Value = ciborium::from_reader(&o.payload[..]).unwrap();
    v.as_map()
        .unwrap()
        .iter()
        .filter_map(|(k, v)| Some((k.as_text()?.to_string(), v.as_text().map(str::to_string).unwrap_or_else(|| format!("{v:?}")))))
        .collect()
}

/// THE OWNER FRAMES AND THE NEW HELD ITEMS FIELDS ARE READ AS THE RELAY SENDS THEM: `owner_state`
/// (whether a key holds the role), `owner_reply` (kind, ok, the words), `owner_change_ready` (the
/// grant, the change, the person, the recipients in slot order with their card times, every item),
/// and on `held_items` the successor, the owner log and each item's `sealed_to_successor`. Other
/// frames are not taken for them.
///
/// Seen red 2026-11-20 with `parse_ready` reading a recipient's card time from "at" instead of
/// "card_at": "the recipients in slot order with their card times" failed (left [.., card_at: 0]).
#[test]
fn the_owner_frames_and_the_new_held_items_fields_are_read() {
    assert_eq!(parse_state(&serde_json::json!({ "type": "owner_state", "has_owner": false })), Some(false));
    assert_eq!(parse_state(&serde_json::json!({ "type": "owner_state", "has_owner": true })), Some(true));
    assert_eq!(parse_state(&serde_json::json!({ "type": "owner_reply", "has_owner": false })), None);
    let r = parse_reply(&serde_json::json!({ "type": "owner_reply", "kind": "claim", "ok": true, "message": "You are now this server's owner" })).unwrap();
    assert_eq!((r.kind.as_str(), r.ok, r.message.as_str()), ("claim", true, "You are now this server's owner"));
    assert!(parse_reply(&serde_json::json!({ "type": "owner_state" })).is_none());

    let ready = parse_ready(&serde_json::json!({ "type": "owner_change_ready", "grant": "g1", "action": "owner_successor", "target": "kb",
        "recipients": [{ "key": "ka", "card_at": 11 }, { "key": "kb", "card_at": 22 }],
        "items": [{ "id": 3, "package": "p3", "parts": 2 }, { "id": 4, "package": "p4", "parts": 5 }] }))
    .unwrap();
    assert_eq!((ready.grant.as_str(), ready.action.as_str(), ready.target.as_str()), ("g1", "owner_successor", "kb"));
    assert_eq!(ready.recipients, vec![Recipient { key: "ka".into(), card_at: 11 }, Recipient { key: "kb".into(), card_at: 22 }], "the recipients in slot order with their card times");
    assert_eq!(ready.items, vec![ReadyItem { id: 3, package: "p3".into(), parts: 2 }, ReadyItem { id: 4, package: "p4".into(), parts: 5 }]);
    assert!(parse_ready(&serde_json::json!({ "type": "held_items" })).is_none());

    let list = crate::net::held::parse_list(&serde_json::json!({ "type": "held_items", "can_keep": true, "i_am_owner": false,
        "items": [{ "id": 1, "sealed_to_owner": true, "sealed_to_successor": true }, { "id": 2, "sealed_to_owner": true, "sealed_to_successor": false }],
        "actions": [], "counts": {},
        "successor": { "key": "kb", "name": "Bea" },
        "owner_log": [{ "kind": "owner_successor", "old_name": "Ada", "new_name": "Bea", "reason": "just in case", "at": 1_763_600_000_000i64, "object": "{}" },
                      { "kind": "claim", "old_name": "", "new_name": "Ada", "reason": "", "at": 1_763_500_000_000i64, "object": "" }] }))
    .unwrap();
    assert_eq!(list.successor, Some(Successor { key: "kb".into(), name: "Bea".into() }));
    assert_eq!(list.items.iter().map(|i| i.sealed_to_successor).collect::<Vec<_>>(), vec![true, false]);
    assert_eq!(list.owner_log.len(), 2);
    assert_eq!(list.owner_log[0], OwnerLogLine { kind: "owner_successor".into(), old_name: "Ada".into(), new_name: "Bea".into(), reason: "just in case".into(), at: 1_763_600_000_000 });
    assert_eq!((list.owner_log[1].kind.as_str(), list.owner_log[1].new_name.as_str()), ("claim", "Ada"));
    let none = crate::net::held::parse_list(&serde_json::json!({ "type": "held_items", "successor": null, "items": [] })).unwrap();
    assert_eq!((none.successor, none.owner_log.len()), (None, 0), "no successor named, no log");
}

/// EACH CHANGE IS SIGNED BY ME, NAMING THE MEMBER, FOR THIS SERVER, WITH ITS REASON, AND GOES OUT
/// IN THE FRAMES THE RELAY TAKES: `owner_change_begin` carries the signed `mod_action_v1` (the
/// action, `target_kind` identity, the member's key, the reason trimmed, no end date, this server's
/// DID); the finish carries every restore key by item number; the claim carries the code trimmed.
///
/// Seen red 2026-11-20 with `sign` naming `target_kind` "object": "signs" failed (the payload
/// builder refuses an owner action that names anything but a person).
#[test]
fn a_change_is_signed_by_me_naming_the_member_for_this_server() {
    let bea = key_of(7);
    for change in [Change::Handover, Change::Successor, Change::SuccessorRemove] {
        let s = sign(&[9u8; 32], "did:hum:owner-test", change, &bea, "  moving away  ", 1_763_600_000_000).expect("signs");
        let frame: Value = serde_json::from_str(&begin_frame(&s)).unwrap();
        assert_eq!(frame["type"], "owner_change_begin");
        let o = submission_object(&frame["action"]);
        assert!(o.verify_signature().is_ok(), "the action verifies");
        assert_eq!(hex::encode(&o.author_public_key), key_of(9), "signed by me");
        assert_eq!(o.space_id.as_deref(), Some("did:hum:owner-test"), "for this server");
        assert_eq!(o.object_id().unwrap().to_hex(), s.object_id, "its object id is the grant");
        let p = payload_text(&o);
        assert_eq!(p["action"], change.action());
        assert_eq!((p["target_kind"].as_str(), p["target"].as_str(), p["reason"].as_str()), ("identity", bea.as_str(), "moving away"));
        assert_eq!(Change::from_action(change.action()), Some(change));
    }
    assert!(sign(&[9u8; 32], "", Change::Handover, &bea, "r", 1).is_err(), "never without this server's identity");

    let finish: Value = serde_json::from_str(&finish_frame("g")).unwrap();
    assert_eq!(finish, serde_json::json!({ "type": "owner_change_finish", "grant": "g" }), "the finish carries no key (the second review, W3)");
    assert_eq!(serde_json::from_str::<Value>(&claim_frame("  ABCD1234ABCD1234\n")).unwrap(), serde_json::json!({ "type": "owner_claim", "code": "ABCD1234ABCD1234" }));
    assert_eq!(serde_json::from_str::<Value>(&cancel_frame()).unwrap(), serde_json::json!({ "type": "owner_change_cancel" }));
}

/// THE RELAY'S ANSWER MUST BE THE ANSWER TO WHAT THIS DEVICE SIGNED: the same grant (the signed
/// action's id), the same change, the same person, and exactly the people that change seals to in
/// slot order (the new owner alone; the owner then the successor; the owner alone). A relay that
/// adds its own key, swaps the order, names another person or another change is refused in words.
///
/// Seen red 2026-11-20 with the recipients comparison taken out of `check_ready`: "a recipient the
/// change does not name is refused" failed (Ok(())).
#[test]
fn the_relays_answer_must_match_what_was_signed() {
    let ready = |action: &str, target: &str, keys: &[&str]| Ready {
        grant: "g".into(),
        action: action.into(),
        target: target.into(),
        recipients: keys.iter().map(|k| Recipient { key: k.to_string(), card_at: 1 }).collect(),
        items: vec![],
    };
    assert_eq!(expected_recipients(Change::Handover, "me", "bea"), vec!["bea"]);
    assert_eq!(expected_recipients(Change::Successor, "me", "bea"), vec!["me", "bea"]);
    assert_eq!(expected_recipients(Change::SuccessorRemove, "me", "bea"), vec!["me"]);
    assert!(check_ready(&ready("owner_handover", "bea", &["bea"]), Change::Handover, "me", "bea", "g").is_ok());
    assert!(check_ready(&ready("owner_successor", "bea", &["me", "bea"]), Change::Successor, "me", "bea", "g").is_ok());
    assert!(check_ready(&ready("owner_successor_remove", "bea", &["me"]), Change::SuccessorRemove, "me", "bea", "g").is_ok());

    let mismatch = t("owner.ready_mismatch").to_string();
    let recipients = t("owner.recipients_mismatch").to_string();
    assert_eq!(check_ready(&ready("owner_handover", "bea", &["bea"]), Change::Handover, "me", "bea", "another grant"), Err(mismatch.clone()), "another grant");
    assert_eq!(check_ready(&ready("owner_successor", "bea", &["bea"]), Change::Handover, "me", "bea", "g"), Err(mismatch.clone()), "another change");
    assert_eq!(check_ready(&ready("owner_handover", "cy", &["cy"]), Change::Handover, "me", "bea", "g"), Err(mismatch), "another person");
    assert_eq!(check_ready(&ready("owner_handover", "bea", &["bea", "relay"]), Change::Handover, "me", "bea", "g"), Err(recipients.clone()), "a recipient the change does not name is refused");
    assert_eq!(check_ready(&ready("owner_successor", "bea", &["bea", "me"]), Change::Successor, "me", "bea", "g"), Err(recipients.clone()), "the slots in another order");
    assert_eq!(check_ready(&ready("owner_successor_remove", "bea", &["me", "bea"]), Change::SuccessorRemove, "me", "bea", "g"), Err(recipients), "still sealed to the old successor");
}

/// A KEY CARD THAT DOES NOT CHECK OUT ON THIS DEVICE IS REFUSED WITH WORDS, AND NOTHING IS SEALED:
/// a card signed by another key, no card at all, a card dated other than the one the relay checked,
/// a key other than the one this device pinned for the person, and my own card carrying a key that
/// is not mine. Good cards give each recipient's Kyber768 key in slot order.
///
/// Seen red 2026-11-20 with `check_card` taking a card without `card_for` (no signature check): "a
/// card signed by another key is refused" failed (Ok: the swapped card's key was used).
#[test]
fn a_card_that_does_not_check_out_is_refused_with_words() {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (me, bea) = (key_of(9), key_of(7));
    let my_kyber = owner_keys(9).public_key();
    let names = |k: &str| if k == bea { "Bea".to_string() } else { "Ada".to_string() };
    let both = [Recipient { key: me.clone(), card_at: 100 }, Recipient { key: bea.clone(), card_at: 200 }];
    let cards = |bea_card: Option<Value>| move |k: &str| -> Result<Option<Value>, String> { Ok(if k == key_of(9) { Some(card_of(9, 100)) } else { bea_card.clone() }) };
    let no_pins = |_: &str| None;

    let keys = recipient_keys(&me, &my_kyber, &both, cards(Some(card_of(7, 200))), no_pins, names).expect("good cards");
    assert_eq!(keys, vec![my_kyber.clone(), owner_keys(7).public_key()], "each recipient's key, in slot order");

    // Cy's card with Bea's key written in: not signed by Bea.
    let mut swapped = key_card::sign_card(&[8; 32], 200).unwrap();
    swapped.key = bea.clone();
    let words = recipient_keys(&me, &my_kyber, &both, cards(Some(key_card::card_json(&swapped))), no_pins, names);
    assert_eq!(words, Err(tf("owner.card_bad", &[("name", &"Bea")])), "a card signed by another key is refused");
    // A good card for Cy, sent for Bea.
    let words = recipient_keys(&me, &my_kyber, &both, cards(Some(card_of(8, 200))), no_pins, names);
    assert_eq!(words, Err(tf("owner.card_bad", &[("name", &"Bea")])), "somebody else's good card is refused");
    let words = recipient_keys(&me, &my_kyber, &both, cards(None), no_pins, names);
    assert_eq!(words, Err(tf("owner.card_none", &[("name", &"Bea")])), "no card");
    let words = recipient_keys(&me, &my_kyber, &both, cards(Some(card_of(7, 201))), no_pins, names);
    assert_eq!(words, Err(tf("owner.card_at_mismatch", &[("name", &"Bea")])), "a card other than the one the relay checked");
    let pinned_other = B64.encode(owner_keys(8).public_key());
    let words = recipient_keys(&me, &my_kyber, &both, cards(Some(card_of(7, 200))), |k: &str| (k == bea).then(|| pinned_other.clone()), names);
    assert_eq!(words, Err(tf("owner.card_changed", &[("name", &"Bea")])), "a key other than the one pinned here");
    let pinned_same = B64.encode(owner_keys(7).public_key());
    assert!(recipient_keys(&me, &my_kyber, &both, cards(Some(card_of(7, 200))), |k: &str| (k == bea).then(|| pinned_same.clone()), names).is_ok(), "the pinned key itself");

    // My own card on the server, signed by me but carrying Cy's Kyber768 key.
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[9; 32]));
    let mine_wrong = key_card::card_json(&key_card::sign_card_with(&dil, &owner_keys(8).public_key(), 100));
    let only_me = [Recipient { key: me.clone(), card_at: 100 }];
    let words = recipient_keys(&me, &my_kyber, &only_me, |_: &str| Ok(Some(mine_wrong.clone())), no_pins, names);
    assert_eq!(words, Err(t("owner.own_card_not_mine").to_string()), "my own card must carry my own key");
    let words = recipient_keys(&me, &my_kyber, &only_me, |_: &str| Err("refused".to_string()), no_pins, names);
    assert_eq!(words, Err(tf("owner.card_fetch_failed", &[("name", &"Ada"), ("why", &"refused")])));
}

/// THE DEVICE'S RE-WRAP FETCHES AND SENDS HEADERS ALONE (format v4; the second review, W3): two
/// held items, sealed to the owner, are re-wrapped for the owner and a successor (slot order); every
/// fetch asks for a header, every upload is a header alone, and each new header heading the part's
/// OLD body unchanged opens for both test seeds and for nobody else, holding exactly what it held;
/// the progress counts every item and part.
///
/// Seen red 2026-11-21 (run by mutation, scratchpad ownfix4-combo.js) with `rewrap_all` wrapping
/// for the first recipient only: "a header alone for two people" failed first (left 1177, a header
/// for one; right 2313). (The 2026-11-20 note named "the successor opens part 0 of p1", which that
/// earlier assertion now reaches first.)
#[test]
fn the_rewrap_sends_headers_alone_for_the_recipients() {
    let (owner, successor, outsider) = (owner_keys(9), owner_keys(7), owner_keys(8));
    let held: BTreeMap<i64, (String, Vec<Vec<u8>>)> = [(1i64, "p1"), (2, "p2")].into_iter().map(|(id, pkg)| (id, (pkg.to_string(), package(&owner, pkg)))).collect();
    let items: Vec<ReadyItem> = held.iter().map(|(id, (pkg, parts))| ReadyItem { id: *id, package: pkg.clone(), parts: parts.len() as i64 }).collect();
    let mut uploaded: BTreeMap<(i64, usize), Vec<u8>> = BTreeMap::new();
    let mut seen = Vec::new();
    rewrap_all(
        &owner,
        &[owner.public_key(), successor.public_key()],
        &items,
        |id, part| Ok(held[&id].1[part][..header_len(1)].to_vec()),
        |id, part, header| {
            uploaded.insert((id, part), header.to_vec());
            Ok(())
        },
        |p| seen.push(p),
    )
    .expect("the re-wrap runs");
    assert_eq!(uploaded.len(), 6, "every part of both items got a new header");
    for ((id, part), header) in &uploaded {
        let (pkg, parts) = &held[id];
        assert_eq!(header.len(), header_len(2), "a header alone for two people");
        let context = part_context(pkg, *part);
        let mut was = Vec::new();
        open_stream(&owner, &context, parts[*part].as_slice(), &mut was).unwrap();
        let rebuilt = [&header[..], &parts[*part][header_len(1)..]].concat();
        for (who, kp) in [("owner", &owner), ("successor", &successor)] {
            let mut now = Vec::new();
            open_stream(kp, &context, rebuilt.as_slice(), &mut now).unwrap_or_else(|e| panic!("the {who} opens part {part} of {pkg}: {e}"));
            assert_eq!(now, was, "the {who} reads exactly what part {part} of {pkg} held");
        }
        assert!(open_stream(&outsider, &context, rebuilt.as_slice(), &mut Vec::new()).is_err(), "nobody else opens part {part} of {pkg}");
    }
    assert_eq!(seen.first(), Some(&Progress { items_done: 0, items: 2, parts_done: 0, parts: 6 }));
    assert_eq!(seen.last(), Some(&Progress { items_done: 2, items: 2, parts_done: 6, parts: 6 }), "every item and part counted");
    assert!(seen.windows(2).all(|w| w[1].parts_done >= w[0].parts_done && w[1].items_done >= w[0].items_done));
}

/// A LINE STARTING WITH /claim NEVER REACHES A PERSON, HOWEVER IT IS SPELLED (the second review,
/// W5): a zero-width space or a byte-order mark in front, the code right after the word, after a
/// colon, in any case, the bare word: each is a claim for the server alone; a line that only starts
/// the same and is not a well-formed claim ("/claims are...", a code with words after it) is the
/// claim help, never a message; any other line is not a claim at all.
///
/// Seen red 2026-11-21 (run by mutation) with the branch's `typed_claim` (the first word must equal
/// /claim): "the code right after the word" was None (sent on as a message): left None, right
/// Some(Code("0123ABCD4567EF89")).
#[test]
fn a_claim_line_never_reaches_a_person_however_it_is_spelled() {
    let code = |c: &str| Some(TypedClaim::Code(c.to_string()));
    assert_eq!(typed_claim("/claim 0123ABCD4567EF89"), code("0123ABCD4567EF89"));
    assert_eq!(typed_claim("  /CLAIM   0123abcd4567ef89 "), code("0123abcd4567ef89"), "any case, spaces around");
    assert_eq!(typed_claim("/claim0123ABCD4567EF89"), code("0123ABCD4567EF89"), "the code right after the word");
    assert_eq!(typed_claim("/claim:0123ABCD4567EF89"), code("0123ABCD4567EF89"), "after a colon");
    assert_eq!(typed_claim("/claim: 0123ABCD4567EF89"), code("0123ABCD4567EF89"));
    assert_eq!(typed_claim("\u{200B}/claim 0123ABCD4567EF89"), code("0123ABCD4567EF89"), "a zero-width space in front");
    assert_eq!(typed_claim("\u{FEFF}/cl\u{200D}aim 0123ABCD4567EF89"), code("0123ABCD4567EF89"), "format characters inside");
    assert_eq!(typed_claim("/claim"), code(""), "the bare word");
    assert_eq!(typed_claim("/claims are open"), Some(TypedClaim::Help), "a word that starts the same is help, never sent");
    assert_eq!(typed_claim("/claim 0123ABCD4567EF89 please"), Some(TypedClaim::Help), "words after the code");
    assert_eq!(typed_claim("/clai m"), None);
    assert_eq!(typed_claim("hello /claim 0123"), None, "only the first word");
    assert_eq!(typed_claim("/cla"), None);
}

/// THE CHECK IS BELIEVED ONLY WHEN THE OWNER SIGNED IT, AND THE OWNER'S OWN CHANGE IS KNOWN FROM ITS
/// SIGNATURE (the second review, W1 and W6): a check carrying the owner's signed action checks out
/// and is the owner's own on the owner's device (with nothing remembered: a reload or a second
/// window), but not on anyone else's; a check whose action another key signed, or whose grant,
/// action or person is not the signed one, is refused; a device the change does not seal to is told
/// so.
///
/// Seen red 2026-11-20 with `is_own_change` answering by the frame's `owner` alone: "a check
/// whose action Cy signed is not the owner's own" passed as the owner's own.
#[test]
fn a_check_is_believed_only_when_the_owner_signed_it() {
    let (owner, bea, cy) = (key_of(9), key_of(7), key_of(8));
    let check_of = |seed: u8, owner_key: &str| {
        let s = sign(&[seed; 32], "did:hum:owner-test", Change::Successor, &bea, "a successor", 1_763_600_000_000).unwrap();
        Check {
            grant: s.object_id.clone(),
            round: "r1".into(),
            action: "owner_successor".into(),
            object: s.submission.clone(),
            owner: owner_key.to_string(),
            owner_name: "Ola".into(),
            target: bea.clone(),
            target_name: "Bea".into(),
            items: Vec::new(),
            until: 0,
        }
    };
    let good = check_of(9, &owner);
    assert_eq!(check_action(&good), Ok(Change::Successor));
    assert!(is_own_change(&good, &owner), "the owner's own change, known from its signature alone");
    assert!(!is_own_change(&good, &bea), "not the successor's own");
    let forged = check_of(8, &owner);
    assert!(check_action(&forged).is_err() && !is_own_change(&forged, &owner), "a check whose action Cy signed is not the owner's own");
    let mut other = good.clone();
    other.grant = "00".repeat(32);
    assert!(check_action(&other).is_err(), "another grant");
    let mut other = good.clone();
    other.target = cy.clone();
    assert!(check_action(&other).is_err(), "another person");
    let mut other = good.clone();
    other.action = "owner_handover".into();
    assert!(check_action(&other).is_err(), "another change");
    let kp = owner_keys(8);
    assert_eq!(check_all(&kp, &cy, &good, |_, _| Ok(Vec::new()), |_| {}), Err(t("owner.check_not_yours").to_string()), "a device the change does not seal to");
    assert!(check_all(&owner_keys(7), &bea, &good, |_, _| Ok(Vec::new()), |_| {}).is_ok(), "the successor's device, nothing held");
}

/// EVERY KEY IS CHOSEN ONLY WITH THE FOUR GROUPS ASKED (the second review, W1; random positions, the
/// third review's M2; four and no exemption, the fourth review's findings 1 and 5; judged, the fifth
/// review's H1): the groups of its own fingerprint at the positions asked, in any case, match; two
/// groups are short, not a mismatch; the right groups at other positions (the last four) and
/// another key's are a mismatch. There is no "trusted" key that skips it. A key ground to match
/// three groups is refused at every ask.
///
/// Seen red 2026-11-20 with `pick_allowed` answering `true`: "a stranger's key needs its groups"
/// passed with nothing typed. Seen red 2026-11-21 with `pick_allowed` ignoring `asked` and reading
/// the last two groups (the round-3 rule): "the groups asked, in any case" failed. (Round 6 turned
/// `pick_allowed` into `judge`, whose answers key_card.rs `a_wrong_set_is_not_an_empty_one` saw red.)
#[test]
fn every_key_needs_the_four_groups_asked() {
    use key_card::Typed;
    let (bea, cy) = (key_of(7), key_of(8));
    let at = [1, 2, 4, 5];
    assert_eq!(judge(&bea, Some(at), ""), Typed::Empty, "a stranger's key needs its groups");
    assert_eq!(judge(&bea, None, "anything at all, sixteen"), Typed::Empty, "nothing asked, nothing confirms");
    let fp = key_card::fingerprint(&bea);
    let groups: Vec<&str> = fp.split(' ').collect();
    assert_eq!(judge(&bea, Some(at), &format!("{} {} {} {}", groups[1], groups[2], groups[4], groups[5]).to_uppercase()), Typed::Matched, "the groups asked, in any case");
    assert_eq!(judge(&bea, Some(at), &format!("{} {}", groups[4], groups[5])), Typed::Short, "two groups are not enough");
    assert_eq!(judge(&bea, Some(at), &format!("{} {} {} {}", groups[2], groups[3], groups[4], groups[5])), Typed::Mismatched, "the last four are not the groups asked");
    assert_eq!(judge(&bea, Some(at), &key_card::groups_at(&key_card::fingerprint(&cy), at)), Typed::Mismatched, "not another key's");
    assert_eq!(key_card::fingerprint(&bea).split(' ').count(), 6, "six groups to compare");
    // A ground key: Bea's fingerprint at groups 4, 5 and 6, nothing else. Every ask holds another
    // group, so Bea's real groups (what she reads out) never confirm it.
    let ground = groups.iter().enumerate().map(|(i, g)| if i >= 3 { g.to_string() } else { "0000".into() }).collect::<Vec<_>>().join(" ");
    for _ in 0..300 {
        let ask = key_card::confirm_positions();
        assert!(!key_card::groups_match_at(&ground, ask, &key_card::groups_at(&fp, ask)), "a key ground on three groups passed {ask:?}");
    }
}

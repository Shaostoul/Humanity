//! The rules of src/net/group_report.rs and src/net/group_remove.rs (10j's client proof list, the
//! desktop app's items). The group's signed objects are REAL: real ML-DSA-65 keys sign real
//! `group_v1`, `group_msg_v1`, `group_epoch_key_v1` and `group_member_v1` objects, the messages are
//! really encrypted under the group's key, and the new key is really sealed with ML-KEM-768, so
//! the creator's check and the removal run over the same bytes the server would hold. The marker
//! is held byte for byte to what the WEB builds, through the shared fixture
//! scripts/tests/fixtures/group-report-marker.json (generated from web/shared/group-report.js and
//! read by the web's own test too). Each test was seen red once on purpose, recorded at the test.

use super::*;
use crate::net::api_v2;
use crate::net::group_e2ee::{open_epoch_key, parse_group_epoch_key_payload, random_epoch_key};
use crate::net::group_remove::{remove_member, GroupServer, RemoveError};
use crate::relay::core::encoding::{cbor_map, cbor_text, from_canonical_bytes};
use crate::relay::core::object::{Object, ObjectBuilder};
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

const FIXTURE: &str = include_str!("../../scripts/tests/fixtures/group-report-marker.json");
const SPEC: &str = include_str!("../../docs/design/blocking-and-safe-mode.md");
const FILE_TEXT: &str = "[[hum:file:v1]]eyJrZXkiOiJTRUNSRVQifQ==";
const T0: u64 = 1_760_000_000_000;

fn person(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

fn fp(key: &str) -> String {
    api_v2::author_fingerprint_hex(&hex::decode(key).unwrap())
}

/// 10j with its line wrapping undone (a sentence may wrap in the source).
fn ten_j() -> String {
    SPEC[SPEC.find("## 10j.").unwrap()..SPEC.find("## 11.").unwrap()].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A signed object as the server serves it: its submission JSON plus the `object_id` the server
/// files it under.
fn served(id: &str, json: &str) -> Value {
    let mut v: Value = serde_json::from_str(json).unwrap();
    v["object_id"] = Value::from(id);
    v
}

/// A `group_v1` signed by `seed`: its id, and the record as served.
fn group_v1(seed: &[u8], name: &str) -> (String, Value) {
    let b = ObjectBuilder::new("group_v1").created_at(T0).payload_cbor(&cbor_map(vec![("name", cbor_text(name))])).unwrap();
    let (id, json) = api_v2::sign_submission(seed, b).unwrap();
    (id.clone(), served(&id, &json))
}

/// A group message signed by `seed` in `gid`, encrypted under `key` (epoch 1): its id, its time,
/// and the object as served.
fn message(seed: &[u8], gid: &str, key: &[u8], text: &str) -> (String, u64, Value) {
    let (id, json) = api_v2::build_group_msg_submission(seed, gid, 1, key, text).unwrap();
    let v = served(&id, &json);
    let ts = v["created_at"].as_u64().unwrap();
    (id, ts, v)
}

/// The id a served object's own bytes hash to, signature and all (what `verify_submission_json`
/// recomputes), for an object whose signature does not check.
fn id_of(v: &Value) -> String {
    let b = |k: &str| B64.decode(v[k].as_str().unwrap()).unwrap();
    let obj = Object {
        protocol_version: v["protocol_version"].as_u64().unwrap(),
        object_type: v["object_type"].as_str().unwrap().to_string(),
        space_id: None,
        channel_id: None,
        author_public_key: b("author_public_key_b64"),
        created_at: v["created_at"].as_u64(),
        references: v["references"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect(),
        payload_schema_version: v["payload_schema_version"].as_u64().unwrap(),
        payload_encoding: v["payload_encoding"].as_str().unwrap().to_string(),
        payload: b("payload_b64"),
        signature: b("signature_b64"),
    };
    obj.object_id().unwrap().to_hex()
}

fn fields(v: &Value) -> Report {
    let s = |k: &str| v[k].as_str().unwrap().to_string();
    Report {
        group_id: s("group_id"),
        group_name: s("group_name"),
        target: s("target"),
        reason: s("reason"),
        note: s("note"),
        items: v["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|it| Item { id: it["id"].as_str().unwrap().into(), from: it["from"].as_str().unwrap().into(), ts: it["ts"].as_u64().unwrap(), text: it["text"].as_str().unwrap().into() })
            .collect(),
    }
}

/// THE MARKER, byte for byte the web's: for every case in the shared fixture (generated from
/// web/shared/group-report.js, read by the web's test too) the text built here is the text the web
/// built, and it reads back to the same bytes; every refused case is refused with the web's
/// sentence. The marker itself is 10j's. Seen red 2026-10-10 by writing `target` before
/// `group_name` in `build_text`: the first fixture case differed ("escapes, an upper-case key and
/// id, a trimmed note").
#[test]
fn the_text_is_byte_for_byte_what_the_web_builds_from_the_shared_fixture() {
    let doc: Value = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
    let built = doc["built"].as_array().unwrap();
    assert!(built.len() >= 4, "the fixture's cases are there");
    for case in built {
        let name = case["name"].as_str().unwrap();
        let text = build_text(&fields(&case["fields"])).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(text, case["text"].as_str().unwrap(), "{name}");
        let back = parse(&text).unwrap_or_else(|| panic!("{name}: it reads back"));
        assert_eq!(build_text(&back).unwrap(), text, "{name}: built again from what was read, the same bytes");
    }
    for case in doc["refused"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        assert_eq!(build_text(&fields(&case["fields"])).err(), case["error"].as_str(), "{name}");
    }
    let marker = ten_j();
    assert!(marker.contains(&format!("the marker `{MARKER}`")), "the marker is 10j's");
    assert!(marker.contains("`\"group_report\": true` on the `dm_put`"), "10j's flag (engine/group_report.rs sets it)");
    assert!(is_report_text(&format!("{MARKER}{{}}")) && !is_report_text("[[hum:contact-request:v1]]{}"));
}

/// The dialog's choices (10j and the web's third case), in 10j's words. Seen red 2026-10-10 with
/// the "I created it" case taken out of `choices`: "I created the group: only the admins" got all
/// three.
#[test]
fn the_dialog_offers_the_three_with_the_creator_first_or_only_the_admins_and_says_why() {
    let (me, them, creator) = ("a1".repeat(32), "b2".repeat(32), "c3".repeat(32));
    let admins_only = |line| Choices { choices: vec![Dest::Admins], chosen: Some(Dest::Admins), line: Some(line), finding: false };
    assert_eq!(
        choices(&me, &them, Some(&creator), false),
        Choices { choices: vec![Dest::Creator, Dest::Admins, Dest::Both], chosen: Some(Dest::Creator), line: None, finding: false },
        "all three, the creator by default"
    );
    assert_eq!(choices(&me, &them, Some(&me.to_uppercase()), false), admins_only(YOU_CREATED), "I created the group: only the admins");
    assert_eq!(choices(&me, &creator, Some(&creator), false), admins_only(THEY_CREATED), "they created the group: only the admins");
    assert_eq!(choices(&me, &them, None, false), admins_only(CREATOR_UNKNOWN), "the creator cannot be found out: only the admins");
    let finding = choices(&me, &them, None, true);
    assert!(finding.choices.is_empty() && finding.chosen.is_none() && finding.line == Some(FINDING_CREATOR), "nothing while finding");
    assert!(Dest::Creator.to_creator() && Dest::Both.to_creator() && !Dest::Admins.to_creator());
    assert!(Dest::Admins.to_admins() && Dest::Both.to_admins() && !Dest::Creator.to_admins());

    // The dialog's part: nothing while finding, then the default; a pick stands while offered.
    let mut g = GroupTarget { id: "ab".repeat(32), finding: true, ..Default::default() };
    assert_eq!(g.destination(&me, &them), None, "Send waits while the creator is being found");
    g.found(Some(creator.to_uppercase()));
    assert_eq!((g.creator.as_deref(), g.destination(&me, &them)), (Some(creator.as_str()), Some(Dest::Creator)));
    g.send_to = Some(Dest::Both);
    assert_eq!(g.destination(&me, &them), Some(Dest::Both), "a pick stands");
    g.found(Some(me.clone()));
    g.send_to = Some(Dest::Creator);
    assert_eq!(g.destination(&me, &them), Some(Dest::Admins), "a pick no longer offered does not");

    // The words are 10j's (its "As built" paragraph holds the web's own).
    let spec = ten_j();
    let said = |s: &str| assert!(spec.contains(s), "10j says {s:?}");
    said(&format!(
        "a choice \"{SEND_TO}\", with \"{}\" (default), \"{}\", and \"{}\"",
        Dest::Creator.label(),
        Dest::Admins.label(),
        Dest::Both.label()
    ));
    said(&format!("(\"{YOU_CREATED}\" or \"{THEY_CREATED}\")"));
    said(&format!("\"{CREATOR_UNKNOWN}\""));
    said(&format!("\"{FINDING_CREATOR}\""));
    said(&format!("the dialog says so before sending: \"{CREATOR_SEES}\""));
    said(&format!("\"{CREATOR_CHECKS}\""));
    said(&format!("\"{SENT_LINE}\""));
    said(&format!("\"{REFUSED_LINE}\""));
    said(&format!("\"{}\"; anything else \"{NOT_FOUND}\"", found_badge("<name>")));
    said(&format!("\"{TITLE}\" shows when there is a report"));
    said(&format!("\"{HELP}\""));
    said(&format!("\"{NONE}\""));
    said(&format!("the actions \"{ACTION_REMOVE}\" (the existing remove path), \"{ACTION_BLOCK}\", and \"{ACTION_DISMISS}\""));
    said(&format!("\"{}\"", arrived_line("<reporter>", "<person>", "<group>")));
    said(&format!("\"{}\"", count_title(1)));
    said(&format!("\"{}\"", remove_confirm("<person>", "<group>")));
    said(&format!("(\"{}\")", RemoveError::NoKey(String::new()).sentence("<person>", "<group>")));
    said(&format!("\"{}\" and \"{REMOVED}\"", removed_line("<person>", "<group>")));
    said(&format!("\"{NOT_IN_GROUP}\""));
    said(&format!("\"{BLOCKED}\""));
    said(&format!("(or \"{NO_ITEMS}\")"));
}

fn base() -> Report {
    Report { group_id: "ab".repeat(32), group_name: "Hikers".into(), target: "cd".repeat(32), reason: "harassment".into(), note: String::new(), items: vec![] }
}

fn an_item(n: u64, text: &str) -> Item {
    Item { id: format!("{n:064x}"), from: "cd".repeat(32), ts: T0 + n, text: text.into() }
}

/// 10j's limits, on the way out and on the way in: 20 items and 16 KB of text pass, one more of
/// either does not, and the 16 KB are bytes, not characters. Seen red 2026-10-10 twice: with the
/// size test written `> MAX_BYTES + 1`, "16 KB and one byte is refused" failed; with the item limit
/// written `> MAX_ITEMS + 1` in `parse`, "a received report with 21 is not read" failed.
#[test]
fn twenty_items_and_sixteen_kb_pass_and_one_more_does_not() {
    let twenty: Vec<Item> = (1..=20).map(|n| an_item(n, &format!("words {n}"))).collect();
    let text20 = build_text(&Report { items: twenty.clone(), ..base() }).expect("20 items pass");
    assert_eq!(parse(&text20).map(|r| r.items.len()), Some(20));
    let mut more = twenty.clone();
    more.push(an_item(21, "words 21"));
    assert_eq!(build_text(&Report { items: more, ..base() }), Err("At most 20 messages can be included. Untick some."), "21 items are refused");
    let extra = format!("\"items\":[{{\"id\":\"{}\",\"from\":\"{}\",\"ts\":{},\"text\":\"w\"}},", "9".repeat(64), "cd".repeat(32), T0);
    let text21 = text20.replacen("\"items\":[", &extra, 1);
    assert_eq!(parse(&text21), None, "a received report with 21 is not read");

    let overhead = build_text(&Report { items: vec![an_item(1, "")], ..base() }).unwrap().len();
    let fill = |n: usize| build_text(&Report { items: vec![an_item(1, &"x".repeat(n))], ..base() });
    let exact = fill(MAX_BYTES - overhead).expect("16 KB passes");
    assert_eq!(exact.len(), MAX_BYTES, "a report of exactly 16 KB");
    assert!(parse(&exact).is_some(), "and is read");
    assert!(fill(MAX_BYTES - overhead + 1).is_err(), "16 KB and one byte is refused");
    assert_eq!(parse(&exact.replacen("xx", "xxx", 1)), None, "and is not read");
    let emoji = build_text(&Report { items: vec![an_item(1, &"\u{1F600}".repeat((MAX_BYTES - overhead).div_ceil(4) + 1))], ..base() });
    assert!(emoji.is_err(), "the limit counts UTF-8 bytes");
    // What a report must name.
    assert_eq!(build_text(&Report { group_id: "nope".into(), ..base() }), Err("This group is not known here."));
    assert_eq!(build_text(&Report { reason: String::new(), ..base() }), Err("Choose a reason first."));
}

/// Never a file (step D's rule): a group message with a file is no item, a builder handed one
/// refuses it, a note with one is refused, and a received report with one anywhere is not read.
/// Seen red 2026-10-10 with `carries_file` taken out of `item`: "a group message with a file is
/// never an item" got an item.
#[test]
fn a_file_is_never_an_item_never_built_and_never_read() {
    let (id, t) = ("01".repeat(32), "cd".repeat(32));
    assert_eq!(item(&id, &t, T0, FILE_TEXT), None, "a group message with a file is never an item");
    assert_eq!(item(&id, &t, T0, &format!("see {}", FILE_TEXT.replace("v1", "v2"))), None, "nor with another version of the marker");
    assert!(item(&id, &t, T0, "plain words").is_some());
    let forced = Report { items: vec![Item { id: id.clone(), from: t.clone(), ts: T0, text: FILE_TEXT.into() }], ..base() };
    assert_eq!(build_text(&forced), Err(FILES_NOT_INCLUDED), "the builder refuses a file item put there by hand");
    assert_eq!(build_text(&Report { note: format!("look {FILE_TEXT}"), ..base() }), Err(FILES_NOT_INCLUDED), "and a file in the note");
    let clean = build_text(&Report { items: vec![an_item(1, "PLACEHOLDER")], ..base() }).unwrap();
    assert_eq!(parse(&clean.replace("PLACEHOLDER", FILE_TEXT)), None, "a received report with a file is not read");
}

/// THE CREATOR'S CHECKS against their own copy, over real signed objects: found (with who signed
/// it), not in the copy, altered text, wrong sender, wrong time, a stored forgery whose signature
/// does not check, another group's message, an id the server put on other bytes, and an epoch
/// whose key is not held. Seen red 2026-10-10 three ways: with the text comparison taken out of
/// `check_items`, the altered item was found; with the sender comparison taken out, the wrong
/// sender was found; and with an unchecked reading of the object used when its signature did not
/// check, the forgery was found.
#[test]
fn the_creators_checks_against_their_own_copy() {
    let (ben_seed, _ben) = person(71);
    let (ann_seed, ann) = person(72);
    let (cy_seed, cy) = person(73);
    let (gid, _) = group_v1(&ben_seed, "Hikers");
    let key = random_epoch_key();
    let keys: HashMap<u64, Vec<u8>> = [(1, key.clone())].into();
    let (m1, t1, o1) = message(&cy_seed, &gid, &key, "you are all idiots");
    let (_m2, _t2, o2) = message(&ann_seed, &gid, &key, "hello all");
    // A stored forgery: Cy's other message with a signature that is not over its bytes, filed
    // under the id its own bytes hash to (so only the signature can tell).
    let (_, tf, mut forged) = message(&cy_seed, &gid, &key, "I will hurt you");
    forged["signature_b64"] = o1["signature_b64"].clone();
    let forged_id = id_of(&forged);
    forged["object_id"] = Value::from(forged_id.as_str());
    assert!(api_v2::verify_submission_json(&forged.to_string()).is_none(), "the forgery's signature does not check");
    // The same words by Cy in another group.
    let (other_gid, _) = group_v1(&ben_seed, "Elsewhere");
    let (m3, t3, o3) = message(&cy_seed, &other_gid, &key, "you are all idiots");
    let copy = vec![o1.clone(), o2.clone(), forged, o3];
    let it = |id: &str, from: &str, ts: u64, text: &str| Item { id: id.into(), from: from.into(), ts, text: text.into() };
    let rep = |items: Vec<Item>| Report { group_id: gid.clone(), group_name: "Hikers".into(), target: cy.clone(), reason: "threats".into(), note: String::new(), items };
    let out = check_items(
        &rep(vec![
            it(&m1, &cy, t1, "you are all idiots"),
            it(&"ef".repeat(32), &cy, t1, "you are all idiots"),
            it(&m1, &cy, t1, "you are all IDIOTS!!"),
            it(&m1, &ann, t1, "you are all idiots"),
            it(&m1, &cy, t1 + 1, "you are all idiots"),
            it(&forged_id, &cy, tf, "I will hurt you"),
            it(&m3, &cy, t3, "you are all idiots"),
        ]),
        &copy,
        &keys,
    );
    assert_eq!(
        out.iter().map(|i| i.found).collect::<Vec<_>>(),
        [true, false, false, false, false, false, false],
        "found; not in my copy; altered text; wrong sender; wrong time; bad signature; another group's"
    );
    assert_eq!(out[0].signer, cy, "a found item names who signed it");
    assert_eq!(badge(&out[0], "Cy"), (true, "Found in your copy of the group, signed by Cy".to_string()));
    assert_eq!(badge(&out[1], "Cy"), (false, NOT_FOUND.to_string()));
    // A server that files Ann's bytes under Cy's message id: the id is the hash of the bytes.
    let mut mislabelled = o2.clone();
    mislabelled["object_id"] = Value::from(m1.as_str());
    let relabelled = check_items(&rep(vec![it(&m1, &cy, t1, "you are all idiots")]), &[mislabelled], &keys);
    assert!(!relabelled[0].found, "an id the server put on other bytes is not found");
    // A copy I hold no key for: every item shown, none found, none dropped.
    let unread = check_items(&rep(vec![it(&m1, &cy, t1, "you are all idiots")]), &copy, &HashMap::new());
    assert_eq!(unread.len(), 1);
    assert!(!unread[0].found, "words I cannot open are not found");
}

/// Which reports the creator keeps (10j): only about a group held AS ITS CREATOR, from someone
/// in it, addressed to me, and not about me; the group's name is the one my own list has. A
/// report arriving from me, or one that does not read, is never kept. Seen red 2026-10-10 with
/// the creator check taken out of `accepts`: "a group I am in but did not create" was kept.
#[test]
fn a_report_is_kept_only_about_a_group_i_created_from_someone_in_it_to_me_not_about_me() {
    let (me, ann, cy, dan) = ("a1".repeat(32), "b2".repeat(32), "d4".repeat(32), "e5".repeat(32));
    let g = "ab".repeat(32);
    let groups = vec![P2pGroupInfo { group_id: g.clone(), name: "Hikers".into(), members: vec![me.clone(), ann.clone(), cy.clone()], is_creator: true }];
    let rep = Report { group_id: g.clone(), group_name: "Totally not Hikers".into(), target: cy.clone(), reason: "spam".into(), note: String::new(), items: vec![] };
    let verdict = |from: &str, to: &str, r: &Report, gs: &[P2pGroupInfo]| accepts(&me, from, to, r, gs).map(|g| g.name.clone());
    assert_eq!(verdict(&ann, &me, &rep, &groups), Ok("Hikers".to_string()), "kept");
    assert_eq!(verdict(&ann, &me, &Report { group_id: "cd".repeat(32), ..rep.clone() }, &groups), Err(Why::NotHeld), "a group I do not hold");
    assert_eq!(verdict(&ann, &me, &rep, &[]), Err(Why::NotHeld), "no groups at all");
    let joined = vec![P2pGroupInfo { is_creator: false, ..groups[0].clone() }];
    assert_eq!(verdict(&ann, &me, &rep, &joined), Err(Why::NotCreator), "a group I am in but did not create");
    assert_eq!(verdict(&dan, &me, &rep, &groups), Err(Why::ReporterNotInGroup), "from someone not in it");
    assert_eq!(verdict(&ann, &ann, &rep, &groups), Err(Why::NotToMe), "addressed to someone else");
    assert_eq!(verdict(&ann, &me, &Report { target: me.clone(), ..rep.clone() }, &groups), Err(Why::AboutMe), "about me");

    // As it arrives: a verified DM whose text is the marker and the JSON.
    let text = build_text(&rep).unwrap();
    let dm = |from: &str, text: &str| DmInner { from: from.into(), to: me.clone(), ts: 7, text: text.into(), sig_b64: format!("sig-{from}"), cert: None };
    let pending = pending_from(&me, &dm(&ann, &text)).expect("a report from Ann waits for its check");
    assert_eq!(pending.id, record_id(&format!("sig-{ann}")), "one record per report: its signature names it");
    assert_eq!((pending.from.as_str(), pending.report.group_name.as_str()), (ann.as_str(), "Totally not Hikers"));
    assert!(pending_from(&me, &dm(&me, &text)).is_none(), "my own, from another device: nothing to keep");
    assert!(pending_from(&me, &dm(&ann, &format!("{MARKER}{{nope"))).is_none(), "one that does not read");
    assert!(pending_from(&me, &dm(&ann, "hello")).is_none(), "not a report");
    let kept = resolve(&me, &pending, &groups, &[], &HashMap::new()).expect("kept");
    assert_eq!(kept.group_name, "Hikers", "the group's name as my own list has it, not as the report claims");
    assert_eq!(resolve(&me, &pending, &joined, &[], &HashMap::new()), Err(Why::NotCreator));
    let other = KeptReport { group_id: "cd".repeat(32), ..kept.clone() };
    assert_eq!(count(&[kept.clone(), kept.clone(), other], &g), 2, "the count on the group");
}

/// The group's creator, for the reporter's dialog, is read from the group's own signed record:
/// its signature checked and its bytes hashing to the group's id, never what a server says. Seen
/// red 2026-10-10 with the `group_v1` test taken out of `creator_from_group_object`: a group
/// message's author was taken for the creator.
#[test]
fn the_groups_creator_is_read_from_its_own_signed_record() {
    let (ben_seed, ben) = person(74);
    let (cy_seed, _) = person(75);
    let (gid, record) = group_v1(&ben_seed, "Hikers");
    assert_eq!(creator_from_group_object(&record, &gid), Some(ben.clone()), "Ben created it");
    assert_eq!(creator_from_group_object(&record, &"ab".repeat(32)), None, "a record that is another group's");
    let mut tampered = record.clone();
    tampered["payload_b64"] = Value::from(B64.encode(b"\xa1\x64name\x66Ripped"));
    assert_eq!(creator_from_group_object(&tampered, &gid), None, "a record whose signature does not check");
    let (mid, _, msg) = message(&cy_seed, &gid, &random_epoch_key(), "hi");
    assert_eq!(creator_from_group_object(&msg, &mid), None, "a group message is not the group's record");
}

/// The server a removal talks to, recording every request in order.
struct Recorder {
    log: Vec<String>,
    posted: Vec<String>,
    epoch: Result<Option<Vec<u8>>, String>,
    members: Result<Vec<(String, Option<String>)>, String>,
    refuse_posts: bool,
    /// The group asked about, and who this device had on its removed list at each post.
    gid: String,
    removed_at_post: Vec<Vec<String>>,
}

impl Recorder {
    fn new(epoch: Result<Option<Vec<u8>>, String>, members: Result<Vec<(String, Option<String>)>, String>, refuse_posts: bool) -> Self {
        Self { log: vec![], posted: vec![], epoch, members, refuse_posts, gid: String::new(), removed_at_post: vec![] }
    }
}

impl GroupServer for Recorder {
    fn epoch_payload(&mut self, group_id: &str) -> Result<Option<Vec<u8>>, String> {
        self.log.push("GET epoch".into());
        self.gid = group_id.to_string();
        self.epoch.clone()
    }
    fn members(&mut self, _group_id: &str) -> Result<Vec<(String, Option<String>)>, String> {
        self.log.push("GET members".into());
        self.members.clone()
    }
    fn post(&mut self, submission_json: &str) -> Result<(), String> {
        let v: Value = serde_json::from_str(submission_json).unwrap();
        self.log.push(format!("POST {}", v["object_type"].as_str().unwrap()));
        self.removed_at_post.push(crate::net::group_remove::removed_here(&self.gid));
        if self.refuse_posts {
            return Err("HTTP 403".into());
        }
        self.posted.push(submission_json.to_string());
        Ok(())
    }
}

fn kyber(seed: &[u8]) -> String {
    crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap().public_base64()
}

/// "REMOVE THEM FROM THE GROUP" (10j, the web's order): a new group key for the next epoch,
/// sealed to everyone but the person removed (left out although the server still lists them), is
/// posted BEFORE the signed remove naming them; the others can open the new key and the person
/// removed has no copy of it. A key that cannot be made removes no one. Seen red 2026-10-10 three
/// ways: with the removal posted before the key, "the new key, then the remove" failed; with the
/// person left in the roster the key is sealed to, "sealed to everyone but Cy" failed; and with
/// an unreadable member list and a refused key both ignored (the removal posted anyway), the
/// `Err(RemoveError::NoKey(_))` assertion for the unreadable list failed.
#[test]
fn remove_posts_a_new_key_for_everyone_else_before_the_removal() {
    let (ben_seed, ben) = person(76);
    let (ann_seed, ann) = person(77);
    let (cy_seed, cy) = person(78);
    let (gid, _) = group_v1(&ben_seed, "Hikers");
    // The group's current key, epoch 1, as the server holds it.
    let everyone = |list: &[(&[u8], &str)]| -> Vec<crate::net::group_e2ee::GroupMemberKey> {
        list.iter().map(|(s, k)| crate::net::group_e2ee::GroupMemberKey { fp: fp(k), kyber_pub_b64: kyber(s) }).collect()
    };
    let all = [(ben_seed.as_slice(), ben.as_str()), (ann_seed.as_slice(), ann.as_str()), (cy_seed.as_slice(), cy.as_str())];
    let current = crate::net::group_e2ee::build_group_epoch_key_v1(&gid, 1, &random_epoch_key(), &everyone(&all)).unwrap();
    let (_, current_json) = api_v2::sign_submission(&ben_seed, current).unwrap();
    let current_payload = B64.decode(serde_json::from_str::<Value>(&current_json).unwrap()["payload_b64"].as_str().unwrap()).unwrap();
    let roster = vec![(ben.clone(), Some(kyber(&ben_seed))), (ann.clone(), Some(kyber(&ann_seed))), (cy.clone(), Some(kyber(&cy_seed)))];
    let mut server = Recorder::new(Ok(Some(current_payload.clone())), Ok(roster.clone()), false);

    let (epoch, key) = remove_member(&mut server, &ben_seed, &gid, &cy).expect("removed");
    assert_eq!(server.log, ["GET epoch", "GET members", "POST group_epoch_key_v1", "POST group_member_v1"], "the new key, then the remove");
    assert_eq!(epoch, 2, "the next epoch");
    let rekey = api_v2::verify_submission_json(&server.posted[0]).expect("the new key is signed");
    assert_eq!((rekey.author_pubkey_hex.as_str(), rekey.references.as_slice()), (ben.as_str(), [gid.clone()].as_slice()), "by me, for this group");
    let sealed = parse_group_epoch_key_payload(&rekey.payload).unwrap();
    assert_eq!(sealed.epoch, 2);
    let mut to: Vec<String> = sealed.recipients.iter().map(|r| r.fp.clone()).collect();
    to.sort();
    let mut want = vec![fp(&ann), fp(&ben)];
    want.sort();
    assert_eq!(to, want, "sealed to everyone but Cy, though the server still lists Cy");
    let ann_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&ann_seed).unwrap();
    assert_eq!(open_epoch_key(&sealed, &fp(&ann), &ann_kp).unwrap(), (2, key.clone()), "Ann can open the new key");
    let cy_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&cy_seed).unwrap();
    assert!(open_epoch_key(&sealed, &fp(&cy), &cy_kp).is_err(), "Cy has no copy of it");
    let removal = api_v2::verify_submission_json(&server.posted[1]).expect("the remove is signed");
    assert_eq!((removal.object_type.as_str(), removal.author_pubkey_hex.as_str()), ("group_member_v1", ben.as_str()));
    let ciborium::Value::Map(fields) = from_canonical_bytes(&removal.payload).unwrap() else { panic!("a map") };
    let field = |k: &str| fields.iter().find(|(f, _)| f.as_text() == Some(k)).map(|(_, v)| v.clone()).unwrap();
    assert_eq!(field("action").as_text(), Some("remove"));
    assert_eq!(field("subject").as_bytes().map(hex::encode), Some(cy.clone()), "with Cy as the subject");

    // The member list cannot be read: no key can be made, and no one is removed.
    let mut down = Recorder::new(Ok(Some(current_payload.clone())), Err("HTTP 502".into()), false);
    assert!(matches!(remove_member(&mut down, &ben_seed, &gid, &cy), Err(RemoveError::NoKey(_))));
    assert!(down.posted.is_empty() && !down.log.iter().any(|l| l.starts_with("POST")), "nothing posted when the key cannot be made");
    // The server refuses the new key: the removal is never sent.
    let mut refusing = Recorder::new(Ok(None), Ok(roster), true);
    assert!(matches!(remove_member(&mut refusing, &ben_seed, &gid, &cy), Err(RemoveError::NoKey(_))));
    assert_eq!(refusing.log.last().map(String::as_str), Some("POST group_epoch_key_v1"), "the key was tried, the remove never was");
    assert_eq!(
        RemoveError::NoKey(String::new()).sentence("Cy", "Hikers"),
        "Could not remove Cy from Hikers: a new group key could not be made."
    );
}

/// ONLY "NO KEY YET" IS EPOCH 1 (the 2026-10-10 review, item 2): when the group's current key
/// cannot be asked for (the server is down) or what it holds does not read, a removal posts
/// nothing at all, rather than guessing "no key yet" and posting epoch 1 over the real current
/// key; only the server saying it holds none starts at epoch 1.
/// Seen red 2026-10-10 with the old `.ok().flatten()...unwrap_or(0)` restored: "the current key
/// cannot be asked for: no one is removed" failed.
#[test]
fn a_removal_that_cannot_read_the_current_key_posts_nothing() {
    let (ben_seed, ben) = person(81);
    let (ann_seed, ann) = person(82);
    let (_cy_seed, cy) = person(83);
    let (gid, _) = group_v1(&ben_seed, "Walkers");
    let roster = vec![(ben.clone(), Some(kyber(&ben_seed))), (ann.clone(), Some(kyber(&ann_seed))), (cy.clone(), None)];
    for (why, epoch) in [("the current key cannot be asked for", Err("HTTP 502".to_string())), ("the key the server holds does not read", Ok(Some(b"not a key payload".to_vec())))] {
        let mut server = Recorder::new(epoch, Ok(roster.clone()), false);
        assert!(matches!(remove_member(&mut server, &ben_seed, &gid, &cy), Err(RemoveError::NoKey(_))), "{why}: no one is removed");
        assert!(!server.log.iter().any(|l| l.starts_with("POST")), "{why}: nothing is posted: {:?}", server.log);
        assert!(!crate::net::group_remove::removed_here(&gid).contains(&cy), "{why}: they stay a member later keys reach");
    }
    let mut fresh = Recorder::new(Ok(None), Ok(roster), false);
    assert_eq!(remove_member(&mut fresh, &ben_seed, &gid, &cy).map(|(e, _)| e), Ok(1), "the server holds no key: epoch 1");
}

/// THE REKEY NEVER SEALS TO SOMEONE BEING REMOVED (the review, item 2): the person is on this
/// device's removed list BEFORE the new key is posted (so the creator's rekey, which runs every
/// couple of seconds while the group is open, cannot seal them a copy in the moment the server
/// still lists them), and the rekey's roster leaves them out while it lists everyone else; a key
/// that could not be made takes them back off the list.
/// Seen red 2026-10-10 two ways: with `note_removed` called after the key was posted, "on the
/// list when the new key is posted" failed (an empty list at the first POST); and with
/// `roster_to_seal` ignoring `left_out`, "the rekey leaves out the person removed" failed.
#[test]
fn the_creators_rekey_never_seals_to_someone_this_device_removed() {
    let (ben_seed, ben) = person(84);
    let (ann_seed, ann) = person(85);
    let (cy_seed, cy) = person(86);
    let (gid, _) = group_v1(&ben_seed, "Climbers");
    let roster = vec![(ben.clone(), Some(kyber(&ben_seed))), (ann.clone(), Some(kyber(&ann_seed))), (cy.clone(), Some(kyber(&cy_seed)))];

    let mut server = Recorder::new(Ok(None), Ok(roster.clone()), false);
    remove_member(&mut server, &ben_seed, &gid, &cy).expect("removed");
    assert_eq!(server.removed_at_post.first().cloned(), Some(vec![cy.clone()]), "on the list when the new key is posted");
    assert!(crate::net::group_remove::removed_here(&gid).contains(&cy), "and after it, for this run");

    // The rekey, while the server still lists Cy: the current key covers Ben and Ann only.
    let covered: std::collections::HashSet<String> = [fp(&ben), fp(&ann)].into_iter().collect();
    let (sealable, added) = api_v2::roster_to_seal(roster.clone(), &covered, &crate::net::group_remove::removed_here(&gid));
    assert!(sealable.iter().all(|m| m.fp != fp(&cy)), "the rekey leaves out the person removed");
    assert_eq!((sealable.len(), added), (2, 0), "so nobody is missing a key and no rekey runs");
    let (_, others) = api_v2::roster_to_seal(roster.clone(), &covered, &[]);
    assert_eq!(others, 1, "without the list Cy would have been sealed a key");

    // A key that cannot be made removes no one, and leaves no one on the list.
    let (dee_seed, dee) = person(87);
    let mut refusing = Recorder::new(Ok(None), Ok(vec![(ben.clone(), Some(kyber(&ben_seed))), (dee.clone(), Some(kyber(&dee_seed)))]), true);
    assert!(remove_member(&mut refusing, &ben_seed, &gid, &dee).is_err());
    assert!(!crate::net::group_remove::removed_here(&gid).contains(&dee), "a failed removal takes them back off");
}

/// Kept and waiting reports live in the encrypted DM store and survive a restart; a report handed
/// over again is not kept twice; Dismiss takes it off this device. Seen red 2026-10-10 with
/// `group_reports` marked `#[serde(skip)]`: "kept across a restart" failed.
#[test]
fn reports_are_kept_in_the_encrypted_store_until_dismissed() {
    let (seed, me) = person(79);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let server = format!("wss://group-report-{nanos}.example");
    let mut store = super::super::dm_store::DmStore::load(&seed, &me, &server);
    let rep = Report { note: "the group reports test's note".into(), ..base() };
    let p = PendingReport { id: record_id("s1"), from: "b2".repeat(32), to: me.clone(), ts: 5, report: rep.clone() };
    assert!(store.add_pending_group_report(p.clone()));
    assert!(!store.add_pending_group_report(p.clone()), "the same report again waits once");
    store.save();
    let mut store = super::super::dm_store::DmStore::load(&seed, &me, &server);
    assert_eq!(store.pending_group_reports(), [p.clone()], "a waiting report is kept across a restart");
    let groups = vec![P2pGroupInfo { group_id: rep.group_id.clone(), name: "Hikers".into(), members: vec![me.clone(), "b2".repeat(32)], is_creator: true }];
    let kept = resolve(&me, &p, &groups, &[], &HashMap::new()).unwrap();
    assert!(store.settle_group_report(&p.id, Some(kept.clone())), "kept now");
    assert!(store.pending_group_reports().is_empty());
    assert!(!store.add_pending_group_report(p.clone()), "a kept report handed over again is not kept twice");
    store.save();
    let mut store = super::super::dm_store::DmStore::load(&seed, &me, &server);
    assert_eq!(store.group_reports(), [&kept], "kept across a restart");
    assert_eq!(store.group_report_count(&rep.group_id), 1);
    assert!(store.set_group_report_removed(&p.id) && store.group_report(&p.id).unwrap().removed);
    assert!(store.remove_group_report(&p.id), "Dismiss");
    assert!(store.group_reports().is_empty() && store.group_report_count(&rep.group_id) == 0, "gone from this device");
    store.remove_file_for_test();
}

/// A loopback server holding one group: GET of the group object, its latest key and its member
/// list answer from `group`, `epoch_payload_b64` and `members`; every POST of a signed object is
/// handed to the receiver and answered 200. Loopback only, so no firewall prompt.
fn group_server(group: Value, epoch_payload_b64: Option<String>, members: Value) -> (String, std::sync::mpsc::Receiver<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let gid = group["object_id"].as_str().unwrap().to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(16) {
            let Ok(mut s) = stream else { continue };
            let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(5)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            let (head, body) = loop {
                let Ok(got) = s.read(&mut chunk) else { break (String::new(), String::new()) };
                if got == 0 {
                    break (String::new(), String::new());
                }
                buf.extend_from_slice(&chunk[..got]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if buf.len() >= end + 4 + len {
                        break (text[..end].to_string(), text[end + 4..].to_string());
                    }
                }
            };
            let line = head.lines().next().unwrap_or("").to_string();
            let (status, answer) = if line.starts_with(&format!("GET /api/v2/objects/{gid} ")) {
                (200, group.to_string())
            } else if line.starts_with(&format!("GET /api/v2/groups/{gid}/epoch ")) {
                match &epoch_payload_b64 {
                    Some(p) => (200, serde_json::json!({ "payload_b64": p }).to_string()),
                    None => (404, "{}".to_string()),
                }
            } else if line.starts_with(&format!("GET /api/v2/groups/{gid}/members ")) {
                (200, members.to_string())
            } else if line.starts_with("POST /api/v2/objects ") {
                let _ = tx.send(body);
                (200, "{}".to_string())
            } else {
                (404, "{}".to_string())
            };
            let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
        }
    });
    (url, rx)
}

/// A SHARED-HISTORY GROUP'S FRESH KEY TAKES THE NEXT NUMBER (the 2026-10-10 server review): when the
/// creator cannot open the group's current key (epoch 3 here, sealed to others only), the rekey's
/// fallback mints a fresh key as epoch 4, never epoch 1 again (which would put a second, different
/// key under a number members already hold); and a current key that does not read at all makes no
/// new key.
/// Seen red 2026-10-10 two ways: with the fallback's epoch put back to 1, "the next number, not 1"
/// failed (left: 1, right: 4); and with a current key that does not read taken as "no key yet"
/// again, "a current key that does not read makes no new key" failed.
#[test]
fn a_shared_history_groups_fresh_key_never_reuses_a_number() {
    let (ben_seed, ben) = person(88);
    let (ann_seed, ann) = person(89);
    let (cy_seed, cy) = person(90);
    let payload = cbor_map(vec![("name", cbor_text("Readers")), ("share_history", crate::relay::core::encoding::cbor_int(1))]);
    let builder = ObjectBuilder::new("group_v1").created_at(T0).payload_cbor(&payload).unwrap();
    let (gid, json) = api_v2::sign_submission(&ben_seed, builder).unwrap();
    let group = served(&gid, &json);
    // The current key, epoch 3, sealed to Ann only: Ben, the creator, cannot open his copy.
    let only_ann = [crate::net::group_e2ee::GroupMemberKey { fp: fp(&ann), kyber_pub_b64: kyber(&ann_seed) }];
    let current = crate::net::group_e2ee::build_group_epoch_key_v1(&gid, 3, &random_epoch_key(), &only_ann).unwrap();
    let (_, current_json) = api_v2::sign_submission(&ben_seed, current).unwrap();
    let current_b64 = serde_json::from_str::<Value>(&current_json).unwrap()["payload_b64"].as_str().unwrap().to_string();
    let members = serde_json::json!({ "members": [
        { "pubkey": ben, "kyber_public": kyber(&ben_seed) },
        { "pubkey": ann, "kyber_public": kyber(&ann_seed) },
        { "pubkey": cy, "kyber_public": kyber(&cy_seed) },
    ] });

    let (url, posted) = group_server(group.clone(), Some(current_b64), members.clone());
    let (epoch, key, added) = api_v2::rekey_if_creator_needs(&url, &ben_seed, &gid).expect("the rekey runs").expect("a new key");
    assert_eq!(epoch, 4, "the next number, not 1");
    assert_eq!(added, 2, "Ben and Cy lacked the current key");
    let sent = posted.recv_timeout(std::time::Duration::from_secs(10)).expect("the new key is posted");
    let sealed = parse_group_epoch_key_payload(&api_v2::verify_submission_json(&sent).expect("signed").payload).unwrap();
    assert_eq!(sealed.epoch, 4, "posted under the next number");
    let cy_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&cy_seed).unwrap();
    assert_eq!(open_epoch_key(&sealed, &fp(&cy), &cy_kp).unwrap(), (4, key), "and Cy can open it");

    let (url, posted) = group_server(group, Some(B64.encode(b"not a key payload")), members);
    assert!(api_v2::rekey_if_creator_needs(&url, &ben_seed, &gid).is_err(), "a current key that does not read makes no new key");
    assert!(posted.recv_timeout(std::time::Duration::from_millis(300)).is_err(), "and nothing is posted");
}

/// SOMEONE REMOVED AND LET BACK IN GETS THE GROUP KEY AGAIN (2026-10-10): this device removes Cy
/// (new key without her, then the remove). While the server still lists her she is left out of
/// every rekey. Once a member list without her has come back, the removal has landed, and when
/// she rejoins with a new ticket the creator's next rekey seals the next key to her as to anyone;
/// a slower list asked for before the removal was seen to land still leaves her out.
/// Seen red 2026-10-10 two ways: with `left_out` never letting anyone go (the old behaviour, the
/// list cleared only by a restart), "she rejoined: sealed to on the next rekey" failed (no rekey
/// at all, Ok(None)); and with `left_out` dropping her the moment a list without her came back,
/// "a list asked for before the removal landed still leaves her out" failed.
#[test]
fn someone_removed_then_let_back_in_is_sealed_to_again() {
    let (ben_seed, ben) = person(91);
    let (ann_seed, ann) = person(92);
    let (cy_seed, cy) = person(93);
    let (gid, group) = group_v1(&ben_seed, "Rowers");
    let listing = |who: &[(&[u8], &str)]| {
        let members: Vec<Value> = who.iter().map(|(s, k)| serde_json::json!({ "pubkey": k, "kyber_public": kyber(s) })).collect();
        serde_json::json!({ "members": members })
    };
    let (b, a, c) = ((ben_seed.as_slice(), ben.as_str()), (ann_seed.as_slice(), ann.as_str()), (cy_seed.as_slice(), cy.as_str()));
    let roster = vec![(ben.clone(), Some(kyber(&ben_seed))), (ann.clone(), Some(kyber(&ann_seed))), (cy.clone(), Some(kyber(&cy_seed)))];

    let mut server = Recorder::new(Ok(None), Ok(roster.clone()), false);
    remove_member(&mut server, &ben_seed, &gid, &cy).expect("removed");
    let key_without_cy = serde_json::from_str::<Value>(&server.posted[0]).unwrap()["payload_b64"].as_str().unwrap().to_string();

    // The server still lists her: left out.
    let (url, posted) = group_server(group.clone(), Some(key_without_cy.clone()), listing(&[b, a, c]));
    assert_eq!(api_v2::rekey_if_creator_needs(&url, &ben_seed, &gid), Ok(None), "still listed: left out, so nobody lacks the key");
    assert!(posted.recv_timeout(std::time::Duration::from_millis(200)).is_err());
    let before_landing = std::time::Instant::now();

    // The removal has landed: a list without her.
    let (url, _) = group_server(group.clone(), Some(key_without_cy.clone()), listing(&[b, a]));
    assert_eq!(api_v2::rekey_if_creator_needs(&url, &ben_seed, &gid), Ok(None), "her removal landed: nobody lacks the key");
    assert_eq!(
        crate::net::group_remove::left_out(&gid, &roster, before_landing),
        vec![cy.clone()],
        "a list asked for before the removal landed still leaves her out"
    );

    // She rejoins with a new ticket: the next rekey seals the next key to her.
    let (url, posted) = group_server(group, Some(key_without_cy), listing(&[b, a, c]));
    let (epoch, key, added) = api_v2::rekey_if_creator_needs(&url, &ben_seed, &gid).expect("the rekey runs").expect("she rejoined: sealed to on the next rekey");
    assert_eq!((epoch, added), (2, 1), "the next key, for the one member without it");
    let sent = posted.recv_timeout(std::time::Duration::from_secs(10)).expect("the new key is posted");
    let sealed = parse_group_epoch_key_payload(&api_v2::verify_submission_json(&sent).expect("signed").payload).unwrap();
    let cy_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&cy_seed).unwrap();
    assert_eq!(open_epoch_key(&sealed, &fp(&cy), &cy_kp).unwrap(), (2, key), "and she can open it");
    assert!(!crate::net::group_remove::removed_here(&gid).contains(&cy), "she is off the list");
}

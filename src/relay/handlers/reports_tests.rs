// Child of reports.rs (#[path], so `use super::*` reaches everything it has): reports the admins
// can check, at the relay (2026-10-09, docs/design/blocking-and-safe-mode.md 10e, its Proof list).
// The handlers are driven directly against a real database with real Dilithium3 identities, as
// the other handler tests are; the socket half (every feature switched off, and a list reaching
// only whoever asked) is in features.rs; the table's own tests (a database from before it, the
// 90-day cull, the export and the erase) are in storage/reports_tests.rs.

use super::*;
use crate::relay::core::pq_crypto::{build_report_sig, derive_dilithium_seed, dm_inner_preimage, report_evidence_hash, DilithiumKeypair};
use crate::relay::relay::Peer;
use crate::relay::storage::Storage;

fn fresh_state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("reports_{tag}"))))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A real identity: its BIP39 seed and its Dilithium3 public key, hex.
struct Person {
    seed: Vec<u8>,
    key: String,
}

fn person(seed_byte: u8) -> Person {
    let seed = vec![seed_byte; 32];
    let key = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key());
    Person { seed, key }
}

impl Person {
    /// This person's signature over a DM v2 inner payload, as their client makes it.
    fn sign_dm(&self, from: &str, to: &str, ts: u64, text: &str) -> String {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&self.seed));
        B64.encode(kp.sign(dm_inner_preimage(from, to, ts, text).as_bytes()))
    }

    /// A DM evidence item: `signer` signed `text` as sent from `from` to `to`.
    fn dm(&self, from: &str, to: &str, ts: u64, text: &str) -> Value {
        serde_json::json!({ "kind": "dm", "from": from, "to": to, "ts": ts, "text": text, "sig": self.sign_dm(from, to, ts, text) })
    }
}

const TS: u64 = 1_760_000_000_000;

/// The frame a client sends, as compact JSON (serde_json writes the evidence inside the frame
/// exactly as it writes the evidence alone), signed by `reporter` over that evidence text.
fn frame(reporter: &Person, target: &str, reason: &str, context: &str, evidence: Value) -> (Value, String) {
    let sig = build_report_sig(&reporter.seed, &reporter.key, target, reason, &evidence.to_string(), TS);
    let v = serde_json::json!({
        "type": "report_v2", "target": target, "context": context, "reason": reason, "note": "",
        "evidence": evidence, "ts": TS, "sig": sig
    });
    let text = v.to_string();
    (v, text)
}

/// Everything the bus carried, taken off it.
fn drain(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Vec<RelayMessage> {
    let mut all = Vec::new();
    while let Ok(m) = rx.try_recv() {
        all.push(m);
    }
    all
}

/// What `who` was sent: a notice as its text, a receipt as "report_received <id>", a list as
/// "reports <state> <count>", a warning or other Private as its text.
fn heard(all: &[RelayMessage], who: &str) -> Vec<String> {
    all.iter()
        .filter_map(|m| match m {
            RelayMessage::Private { to, message } if to == who => Some(message.clone()),
            RelayMessage::ReportReceived { to, id } if to == who => Some(format!("report_received {id}")),
            RelayMessage::Reports { to, state, items } if to == who => Some(format!("reports {state} {}", items.len())),
            _ => None,
        })
        .collect()
}

/// `reporter` sends `(v, text)`: what everyone was sent.
fn send(st: &Arc<RelayState>, reporter: &Person, (v, text): (Value, String)) -> Vec<RelayMessage> {
    let mut rx = st.broadcast_tx.subscribe();
    block(handle(st, &reporter.key, "report_v2", &v, &text));
    drain(&mut rx)
}

/// The id of the report `who` was told was received, if one was.
fn received(all: &[RelayMessage], who: &str) -> Option<i64> {
    all.iter().find_map(|m| match m {
        RelayMessage::ReportReceived { to, id } if to == who => Some(*id),
        _ => None,
    })
}

fn kept_items(st: &Arc<RelayState>, id: i64) -> Vec<Value> {
    serde_json::from_str(&st.db.report_by_id(id).unwrap().expect("the report is kept").evidence).unwrap()
}

fn count(st: &Arc<RelayState>) -> usize {
    st.db.reports_latest(1_000).unwrap().len()
}

/// A DM handed over as evidence is `checked` only when it is what it claims: written by the
/// reported person, to the reporter, with that text. A genuine one is checked; one the reporter
/// forged (signed with their own key), one pinned on the reported person but written by someone
/// else, one the reported person sent to someone else, and that same one with its `to` changed
/// to the reporter are not; all five are kept, labelled. The reporter alone is told the report
/// was received.
///
/// Seen red 2026-10-09 with `Item::keep` checking the signature over the item's own `from` and
/// `to` (no longer requiring `to` to be the reporter): "only the genuine message is checked",
/// left `[true, false, false, true, false]` (the message he sent someone else checked), right
/// `[true, false, false, false, false]`.
#[test]
fn a_genuine_dm_is_checked_and_a_forged_or_misdirected_one_is_not() {
    let st = fresh_state("dm_checked");
    let (rae, tom, xan) = (person(1), person(2), person(3));
    let mut to_changed = tom.dm(&tom.key, &xan.key, 5, "sent to someone else, to changed");
    to_changed["to"] = Value::String(rae.key.clone());
    let evidence = Value::Array(vec![
        tom.dm(&tom.key, &rae.key, 1, "genuine"),
        rae.dm(&tom.key, &rae.key, 2, "forged by the reporter"),
        xan.dm(&tom.key, &rae.key, 3, "written by someone else, pinned on him"),
        tom.dm(&tom.key, &xan.key, 4, "sent to someone else"),
        to_changed,
    ]);
    let all = send(&st, &rae, frame(&rae, &tom.key, "harassment", "dm", evidence));
    let id = received(&all, &rae.key).unwrap_or_else(|| panic!("the report was received: {:?}", heard(&all, &rae.key)));
    assert!(heard(&all, &tom.key).is_empty(), "the reported person is told nothing");
    let flags: Vec<bool> = kept_items(&st, id).iter().map(|i| i["checked"].as_bool().unwrap()).collect();
    assert_eq!(flags, [true, false, false, false, false], "only the genuine message is checked");
    let items = kept_items(&st, id);
    assert_eq!(items[0]["text"], "genuine");
    assert_eq!(items[1]["text"], "forged by the reporter", "unproven items are kept");
}

/// A public post is looked up in `messages` by its author and time, and the text the server has
/// is kept with it (`checked`); a post the server does not have, and someone else's, are kept
/// unproven without text; a group message is kept with its text, unproven and labelled so.
///
/// Seen red 2026-10-09 with the lookup in `Item::keep` reading the post at `timestamp + 1`: "the
/// server's text is kept with the post", left `(Some(false), None)`.
#[test]
fn a_post_is_looked_up_and_a_group_message_is_kept_unproven() {
    let st = fresh_state("posts");
    let (rae, tom, xan) = (person(1), person(2), person(3));
    for (from, ts, text) in [(&tom.key, 42i64, "what the server has"), (&xan.key, 43, "someone else's post")] {
        st.db
            .with_conn(|c| {
                c.execute(
                    "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json, channel_id) VALUES ('chat', ?1, 'X', ?2, ?3, '{}', 'general')",
                    rusqlite::params![from, text, ts],
                )
            })
            .unwrap();
    }
    let evidence = serde_json::json!([
        { "kind": "post", "from": tom.key, "timestamp": 42 },
        { "kind": "post", "from": tom.key, "timestamp": 99 },
        { "kind": "post", "from": xan.key, "timestamp": 43 },
        { "kind": "group_text", "from": tom.key, "ts": 7, "text": "said in the group" },
    ]);
    let all = send(&st, &rae, frame(&rae, &tom.key, "hate", "post", evidence));
    let id = received(&all, &rae.key).expect("received");
    let items = kept_items(&st, id);
    assert_eq!((items[0]["checked"].as_bool(), items[0]["text"].as_str()), (Some(true), Some("what the server has")), "the server's text is kept with the post");
    assert_eq!(items[0]["channel"], "general");
    assert_eq!((items[1]["checked"].as_bool(), items[1].get("text")), (Some(false), None), "a post the server does not have");
    assert_eq!((items[2]["checked"].as_bool(), items[2].get("text")), (Some(false), None), "someone else's post is not looked up");
    assert_eq!((items[3]["kind"].as_str(), items[3]["checked"].as_bool(), items[3]["text"].as_str()), (Some("group_text"), Some(false), Some("said in the group")));
}

/// The reporter's signature covers the BLAKE3 hash of the evidence's JSON text exactly as it
/// arrived. A report signed over the compact text is kept with that hash and that text; the same
/// report with its evidence sent spaced out (other bytes, so another hash) is refused; and so are
/// one signed by someone else in the reporter's name and one signed over another reason.
///
/// Seen red 2026-10-09 with `check_report` hashing the evidence re-serialised by serde_json
/// instead of the text as sent: "the spaced-out evidence is other bytes: [\"report_received 1\"]"
/// (accepted).
#[test]
fn the_evidence_hash_and_the_reporter_signature() {
    let st = fresh_state("hash");
    let (rae, tom, xan) = (person(1), person(2), person(3));
    let evidence = serde_json::json!([{ "kind": "group_text", "from": tom.key, "ts": 7, "text": "hi" }]);
    let compact = evidence.to_string();

    // Spaced out: the same JSON value in other bytes, under a signature over the compact text.
    let (v, text) = frame(&rae, &tom.key, "spam", "group", evidence.clone());
    let spaced = text.replace(&compact, &serde_json::to_string_pretty(&evidence).unwrap());
    assert_ne!(spaced, text);
    let all = send(&st, &rae, (v.clone(), spaced));
    assert!(heard(&all, &rae.key).iter().any(|m| m.contains("signature did not check out")), "the spaced-out evidence is other bytes: {:?}", heard(&all, &rae.key));

    let (v2, t2) = frame(&xan, &tom.key, "spam", "group", evidence.clone()); // Xan signs as if he were Rae
    let all = send(&st, &rae, (v2, t2));
    assert!(heard(&all, &rae.key).iter().any(|m| m.contains("signature did not check out")), "signed by someone else");
    let (mut v3, _) = frame(&rae, &tom.key, "spam", "group", evidence.clone());
    v3["reason"] = Value::String("scam".into());
    let all = send(&st, &rae, (v3.clone(), v3.to_string()));
    assert!(heard(&all, &rae.key).iter().any(|m| m.contains("signature did not check out")), "signed over another reason");
    assert_eq!(count(&st), 0, "nothing refused is kept");

    let all = send(&st, &rae, (v.clone(), text));
    let id = received(&all, &rae.key).expect("the compact report is received");
    let (hash, signed): (String, String) = st
        .db
        .with_conn(|c| c.query_row("SELECT evidence_hash, evidence_signed FROM reports_v2 WHERE id = ?1", rusqlite::params![id], |r| Ok((r.get(0)?, r.get(1)?))))
        .unwrap();
    assert_eq!(signed, compact, "the evidence text is kept as it was signed");
    assert_eq!(hash, report_evidence_hash(&compact));
}

/// Every refusal says why to the reporter alone, and keeps nothing: a bad signature, an unknown
/// reason, a report about yourself, an unknown context, a note over 500 characters, more than
/// 20 pieces of evidence, more than 64 KB of it, an item of an unknown kind, no target, and a
/// target that cannot be a key (not ASCII, which a byte-wise cut of it later would panic on).
///
/// Seen red 2026-10-09 with the self-report check removed from `check_report`: "a report about
/// yourself: [\"report_received 1\"]".
#[test]
fn each_refusal_says_why_and_keeps_nothing() {
    let st = fresh_state("refusals");
    let (rae, tom) = (person(1), person(2));
    let one = || serde_json::json!([{ "kind": "group_text", "from": tom.key, "ts": 1, "text": "x" }]);
    let cases: Vec<(&str, (Value, String), &str)> = vec![
        ("a bad signature", {
            let (mut v, _) = frame(&rae, &tom.key, "spam", "dm", one());
            v["sig"] = Value::String("bm90LWEtc2ln".into());
            let t = v.to_string();
            (v, t)
        }, "signature did not check out"),
        ("an unknown reason", frame(&rae, &tom.key, "being annoying", "dm", one()), "is not one of this server's reasons"),
        ("a report about yourself", frame(&rae, &rae.key, "spam", "dm", one()), "can't report yourself"),
        ("an unknown context", frame(&rae, &tom.key, "spam", "the moon", one()), "does not say where"),
        ("a long note", {
            let (mut v, _) = frame(&rae, &tom.key, "spam", "dm", one());
            v["note"] = Value::String("n".repeat(NOTE_MAX_CHARS + 1));
            let t = v.to_string();
            (v, t)
        }, "longer than 500 characters"),
        ("21 pieces of evidence", frame(&rae, &tom.key, "spam", "dm", Value::Array(vec![serde_json::json!({ "kind": "group_text", "from": "x", "ts": 1, "text": "x" }); 21])), "more than 20 pieces"),
        ("more than 64 KB", frame(&rae, &tom.key, "spam", "dm", serde_json::json!([{ "kind": "group_text", "from": "x", "ts": 1, "text": "y".repeat(EVIDENCE_MAX_BYTES) }])), "more than 64 KB"),
        ("an unknown kind", frame(&rae, &tom.key, "spam", "dm", serde_json::json!([{ "kind": "photo", "from": "x" }])), "is not a direct message, a post or a group message"),
        ("a field missing", frame(&rae, &tom.key, "spam", "dm", serde_json::json!([{ "kind": "dm", "from": "x" }])), "missing a field"),
        ("no target", frame(&rae, "", "spam", "dm", one()), "does not say who"),
        ("a target that is no key", frame(&rae, "a\u{e9}\u{e9}\u{e9}\u{e9}", "spam", "dm", one()), "does not say who"),
    ];
    for (what, f, why) in cases {
        let all = send(&st, &rae, f);
        let told = heard(&all, &rae.key);
        assert!(told.len() == 1 && told[0].starts_with("Your report was not sent:") && told[0].contains(why), "{what}: {told:?}");
        assert!(received(&all, &rae.key).is_none(), "{what} was received");
    }
    assert_eq!(count(&st), 0, "nothing refused is kept");
}

/// A message carrying an encrypted file is never handed to the admins: its marker carries the
/// key to the file, which could be abuse imagery. A report with such a direct message among its
/// evidence (genuinely signed, so it would otherwise check), with the marker anywhere in the
/// text, with a later version of the marker (`[[hum:file:` is the rule, as the web's
/// `reportTextHasFile`), with it in a group message, or pasted into the note, is refused with
/// the one sentence that points to the official lines, and nothing of it is kept: no row, no
/// text. The same report without that message goes through. In the desktop build the marker
/// net::dm_pq seals files under starts with the prefix this rule looks for.
///
/// Seen red 2026-10-09 with the check taken out of `check_report`: "a direct message carrying a
/// file", left `["report_received 1"]` (received and kept, the file's key in the database), right
/// the one sentence. And, once the rule became any version of the marker, with `carries_file`
/// looking for `[[hum:file:v1]]` only: "a later version of the marker".
#[test]
fn a_message_carrying_a_file_is_never_included_in_a_report() {
    #[cfg(feature = "native")]
    assert!(crate::net::dm_pq::FILE_MARKER.starts_with(DM_FILE_MARKER_PREFIX), "the marker the desktop app seals files under");
    let st = fresh_state("file_marker");
    let (rae, tom) = (person(1), person(2));
    let file_text = "[[hum:file:v1]]eyJrZXkiOiJzZWNyZXQifQ==".to_string();
    let plain = tom.dm(&tom.key, &rae.key, 1, "you'll see");
    let cases: Vec<(&str, (Value, String))> = vec![
        ("a direct message carrying a file", frame(&rae, &tom.key, "child_danger", "dm", Value::Array(vec![plain.clone(), tom.dm(&tom.key, &rae.key, 2, &file_text)]))),
        ("the marker later in the text", frame(&rae, &tom.key, "child_danger", "dm", Value::Array(vec![tom.dm(&tom.key, &rae.key, 3, &format!("look {file_text}"))]))),
        ("a later version of the marker", frame(&rae, &tom.key, "child_danger", "dm", Value::Array(vec![tom.dm(&tom.key, &rae.key, 5, "[[hum:file:v2]]eyJ9")]))),
        ("a group message carrying a file", frame(&rae, &tom.key, "child_danger", "group", serde_json::json!([{ "kind": "group_text", "from": tom.key, "ts": 4, "text": file_text }]))),
        ("the marker in the note", {
            let (mut v, _) = frame(&rae, &tom.key, "child_danger", "dm", Value::Array(vec![plain.clone()]));
            v["note"] = Value::String(file_text.clone());
            let t = v.to_string();
            (v, t)
        }),
    ];
    for (what, f) in cases {
        let all = send(&st, &rae, f);
        assert_eq!(heard(&all, &rae.key), [FILE_IN_EVIDENCE], "{what}");
        assert!(received(&all, &rae.key).is_none(), "{what} was received");
        let kept = st.db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM reports_v2", [], |r| r.get::<_, i64>(0))).unwrap();
        assert_eq!(kept, 0, "{what} was kept");
    }
    let all = send(&st, &rae, frame(&rae, &tom.key, "child_danger", "dm", Value::Array(vec![plain])));
    let id = received(&all, &rae.key).expect("the same report without the file goes through");
    assert_eq!(kept_items(&st, id)[0]["checked"], true);
}

/// At most three reports an hour from one reporter, and one report of the same person a day.
/// The fourth in an hour is refused; two hours later the same person is still refused (a day has
/// not passed) while someone new is not; a day later the same person may be reported again.
///
/// Seen red 2026-10-09 with the 24-hour check removed from `check_report`: "the same person
/// again within a day: [\"report_received 4\"]".
#[test]
fn three_reports_an_hour_and_one_report_of_a_person_a_day() {
    let st = fresh_state("limits");
    let rae = person(1);
    let (a, b, c, d) = (person(11), person(12), person(13), person(14));
    let ev = || serde_json::json!([]);
    for p in [&a, &b, &c] {
        assert!(received(&send(&st, &rae, frame(&rae, &p.key, "spam", "profile", ev())), &rae.key).is_some());
    }
    let all = send(&st, &rae, frame(&rae, &d.key, "spam", "profile", ev()));
    assert!(heard(&all, &rae.key)[0].contains("3 reports in the last hour"), "the fourth in an hour: {:?}", heard(&all, &rae.key));

    let back = |hours: i64, target: Option<&str>| {
        st.db
            .with_conn(|cn| {
                cn.execute(
                    "UPDATE reports_v2 SET created_at = created_at - ?1 WHERE (?2 IS NULL OR target_key = ?2)",
                    rusqlite::params![hours * 3_600_000, target],
                )
            })
            .unwrap();
    };
    back(2, None);
    let all = send(&st, &rae, frame(&rae, &a.key, "spam", "profile", ev()));
    assert!(heard(&all, &rae.key).iter().any(|m| m.contains("last 24 hours")), "the same person again within a day: {:?}", heard(&all, &rae.key));
    assert!(received(&send(&st, &rae, frame(&rae, &d.key, "spam", "profile", ev())), &rae.key).is_some(), "someone new, two hours later");
    back(23, Some(&a.key));
    assert!(received(&send(&st, &rae, frame(&rae, &a.key, "spam", "profile", ev())), &rae.key).is_some(), "the same person a day later");
}

/// Admins and moderators may list reports, nobody else. An admin is told who reported (key and
/// name) and gets the evidence whole; a moderator gets "a member", and the direct message's `to`
/// and signature (which name the reporter as well) are left out, while what the relay found and
/// the text stay. The list goes to whoever asked, and says which list it is.
///
/// Seen red 2026-10-09 with `handle_list` passing `true` for every role: "a moderator is not told
/// who reported" (the reporter's key was in the moderator's item).
#[test]
fn admins_see_who_reported_and_moderators_see_a_member() {
    let st = fresh_state("listing");
    let (rae, tom, ada, mo, ned) = (person(1), person(2), person(3), person(4), person(5));
    st.db.register_name("Rae", &rae.key).unwrap();
    st.db.register_name("Tom", &tom.key).unwrap();
    st.db.set_role(&ada.key, "admin").unwrap();
    st.db.set_role(&mo.key, "mod").unwrap();
    let all = send(&st, &rae, frame(&rae, &tom.key, "threats", "dm", serde_json::json!([tom.dm(&tom.key, &rae.key, 1, "watch out")])));
    assert!(received(&all, &rae.key).is_some());

    let list = |who: &Person, state: &str| -> (Vec<String>, Vec<Value>) {
        let mut rx = st.broadcast_tx.subscribe();
        block(handle(&st, &who.key, "reports_list", &serde_json::json!({ "type": "reports_list", "state": state }), ""));
        let all = drain(&mut rx);
        let items = all
            .iter()
            .find_map(|m| match m {
                RelayMessage::Reports { to, items, .. } if *to == who.key => Some(items.clone()),
                _ => None,
            })
            .unwrap_or_default();
        (heard(&all, &who.key), items)
    };

    let (_, admin_items) = list(&ada, "open");
    let a = &admin_items[0];
    assert_eq!((a["reporter"].as_str(), a["reporter_name"].as_str()), (Some(rae.key.as_str()), Some("Rae")), "an admin is told who reported");
    assert_eq!((a["target"].as_str(), a["target_name"].as_str(), a["reason"].as_str(), a["reason_label"].as_str()), (Some(tom.key.as_str()), Some("Tom"), Some("threats"), Some("Threats of violence")));
    assert_eq!((a["context"].as_str(), a["state"].as_str()), (Some("dm"), Some("open")));
    assert!(a["created_at"].as_i64().is_some() && a["decision"].is_null());
    assert_eq!(a["evidence"][0]["to"], rae.key.as_str(), "an admin gets the evidence whole");
    assert!(a["evidence"][0]["sig"].is_string());

    let (_, mod_items) = list(&mo, "open");
    let m = &mod_items[0];
    assert!(m.get("reporter").is_none(), "a moderator is not told who reported: {m}");
    assert_eq!(m["reporter_name"], "a member");
    assert!(!m.to_string().contains(&rae.key), "the reporter's key is nowhere in a moderator's item");
    assert_eq!((m["evidence"][0]["checked"].as_bool(), m["evidence"][0]["text"].as_str()), (Some(true), Some("watch out")), "what was found stays");

    let (told, items) = list(&ned, "open");
    assert!(items.is_empty() && told.iter().any(|t| t.contains("Only admins and moderators")), "a member is refused: {told:?}");
    let (told, items) = list(&ada, "decided");
    assert!(items.is_empty() && told == ["reports decided 0"], "the decided list, empty, says which it is: {told:?}");
}

/// A decision is carried out through the moderation path, so its rules hold: a moderator cannot
/// ban (admins only) nor act on an admin, and a report about the moderator is for an admin; each
/// refusal leaves the report open. A moderator's mute mutes and is recorded with who decided,
/// what, the note and when; a decided report is not decided again; nobody else may decide.
///
/// Seen red 2026-10-09 with `handle_decide` calling `handle_mod_action` without asking
/// `mod_action_refusal` first: "a moderator's ban is refused and the report stays open:
/// [\"'ban' requires admin privileges.\", \"Report #1 decided: ban.\"]", the moderation path
/// refusing the ban while the report recorded it.
#[test]
fn a_decision_goes_through_the_moderation_path_and_keeps_its_rules() {
    let st = fresh_state("decide");
    let (rae, tom, ada, mo, ned) = (person(1), person(2), person(3), person(4), person(5));
    st.db.register_name("Tom", &tom.key).unwrap();
    st.db.set_role(&ada.key, "admin").unwrap();
    st.db.set_role(&mo.key, "mod").unwrap();
    let mut ids = Vec::new();
    for target in [&tom, &ada, &mo] {
        ids.push(received(&send(&st, &rae, frame(&rae, &target.key, "harassment", "profile", serde_json::json!([]))), &rae.key).expect("received"));
    }
    let (about_tom, about_admin, about_mod) = (ids[0], ids[1], ids[2]);
    let decide = |who: &Person, id: i64, decision: &str| -> Vec<String> {
        let mut rx = st.broadcast_tx.subscribe();
        let raw = serde_json::json!({ "type": "report_decide", "id": id, "decision": decision, "note": "seen it" });
        block(handle(&st, &who.key, "report_decide", &raw, ""));
        heard(&drain(&mut rx), &who.key)
    };
    let open = |id: i64| st.db.report_by_id(id).unwrap().unwrap().state == "open";

    let told = decide(&mo, about_tom, "ban");
    assert!(told.iter().any(|t| t.contains("requires admin")) && open(about_tom) && !st.db.is_banned(&tom.key).unwrap(), "a moderator's ban is refused and the report stays open: {told:?}");
    let told = decide(&mo, about_admin, "mute");
    assert!(told.iter().any(|t| t.contains("Only an admin can act on another admin")) && open(about_admin), "a moderator cannot act on an admin: {told:?}");
    let told = decide(&mo, about_mod, "dismiss");
    assert!(told.iter().any(|t| t.contains("is about you")) && open(about_mod), "a report about the moderator is for an admin: {told:?}");
    let told = decide(&ned, about_tom, "dismiss");
    assert!(told.iter().any(|t| t.contains("Only admins and moderators")) && open(about_tom), "a member cannot decide: {told:?}");
    let told = decide(&mo, about_tom, "shout");
    assert!(told.iter().any(|t| t.contains("a decision is one of")) && open(about_tom), "an unknown decision: {told:?}");

    let told = decide(&mo, about_tom, "mute");
    assert!(st.db.is_muted(&tom.key).unwrap(), "the mute was carried out: {told:?}");
    let r = st.db.report_by_id(about_tom).unwrap().unwrap();
    assert_eq!((r.state.as_str(), r.decision.as_deref(), r.decided_by.as_deref(), r.decision_note.as_str()), ("decided", Some("mute"), Some(mo.key.as_str()), "seen it"));
    assert!(r.decided_at.is_some_and(|t| t > 0));
    assert!(told.iter().any(|t| t.contains("decided: mute")), "{told:?}");
    let told = decide(&ada, about_tom, "ban");
    assert!(told.iter().any(|t| t.contains("already decided")) && !st.db.is_banned(&tom.key).unwrap(), "a decided report is not decided again: {told:?}");
    let told = decide(&ada, about_mod, "dismiss");
    assert!(!open(about_mod), "an admin decides the report about the moderator: {told:?}");
}

/// A warning reaches the reported person and names nobody; `delete_post` deletes the reported
/// post (and tells every client to drop it), and is refused for a report with no post of theirs;
/// a dismissal is recorded and sends the reported person nothing.
///
/// Seen red 2026-10-09 with the post deletion in `handle_decide` skipped: "the reported post is
/// gone" failed.
#[test]
fn warn_delete_post_and_dismiss() {
    let st = fresh_state("decide_kinds");
    let (rae, tom, ada) = (person(1), person(2), person(3));
    st.db.register_name("Rae", &rae.key).unwrap();
    st.db.register_name("Tom", &tom.key).unwrap();
    st.db.set_role(&ada.key, "admin").unwrap();
    block(async {
        st.peers.write().await.insert(
            tom.key.clone(),
            Peer { public_key_hex: tom.key.clone(), display_name: Some("Tom".into()), upload_token: None, kyber_public: None, conn_id: 1 },
        );
    });
    st.db
        .with_conn(|c| {
            c.execute(
                "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json, channel_id) VALUES ('chat', ?1, 'Tom', 'a cruel post', 42, '{}', 'general')",
                rusqlite::params![tom.key],
            )
        })
        .unwrap();
    let post = received(&send(&st, &rae, frame(&rae, &tom.key, "hate", "post", serde_json::json!([{ "kind": "post", "from": tom.key, "timestamp": 42 }]))), &rae.key).unwrap();
    let tom2 = person(6);
    let dm_only = received(&send(&st, &rae, frame(&rae, &tom2.key, "spam", "dm", serde_json::json!([tom2.dm(&tom2.key, &rae.key, 1, "buy")]))), &rae.key).unwrap();
    let tom3 = person(7);
    let to_warn = received(&send(&st, &rae, frame(&rae, &tom3.key, "harassment", "profile", serde_json::json!([]))), &rae.key).unwrap();

    let decide = |id: i64, decision: &str| -> Vec<RelayMessage> {
        let mut rx = st.broadcast_tx.subscribe();
        let raw = serde_json::json!({ "type": "report_decide", "id": id, "decision": decision, "note": "" });
        block(handle(&st, &ada.key, "report_decide", &raw, ""));
        drain(&mut rx)
    };
    let all = decide(post, "delete_post");
    assert!(st.db.post_for_report(&tom.key, 42).unwrap().is_none(), "the reported post is gone");
    assert!(all.iter().any(|m| matches!(m, RelayMessage::Delete { from, timestamp: 42 } if *from == tom.key)), "every client is told to drop it");
    assert_eq!(st.db.report_by_id(post).unwrap().unwrap().decision.as_deref(), Some("delete_post"));

    let all = decide(dm_only, "delete_post");
    assert!(heard(&all, &ada.key).iter().any(|t| t.contains("no post of theirs")), "{:?}", heard(&all, &ada.key));
    assert_eq!(st.db.report_by_id(dm_only).unwrap().unwrap().state, "open");

    let all = decide(to_warn, "warn");
    let warned = heard(&all, &tom3.key);
    assert!(warned.len() == 1 && warned[0].contains("warned you"), "the warning reaches them: {warned:?}");
    assert!(!warned[0].contains("Rae") && !warned[0].contains(&rae.key), "and names nobody");
    assert_eq!(st.db.report_by_id(to_warn).unwrap().unwrap().decision.as_deref(), Some("warn"));

    let all = decide(dm_only, "dismiss");
    assert!(heard(&all, &tom2.key).is_empty(), "a dismissal sends them nothing");
    assert_eq!(st.db.report_by_id(dm_only).unwrap().unwrap().decision.as_deref(), Some("dismiss"));
}

/// The shipped reasons file loads: the ten ids of 10e in order, each with a label and help, and
/// the two danger reasons carrying 10e's sentence word for word. A file that is missing or does
/// not load is replaced by the copy built into the exe (said in the log), and the parser refuses
/// a repeated id, an unknown field and an empty list.
///
/// Seen red 2026-10-09 with `someone_in_danger`'s help edited to drop ", not police": "the danger
/// reasons carry the sentence word for word (someone_in_danger)".
#[test]
fn the_shipped_report_reasons_load() {
    let shipped = ReportReasons::parse(include_str!("../../../data/safety/report_reasons.json")).expect("the shipped file loads");
    let ids: Vec<&str> = shipped.0.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["spam", "scam", "harassment", "threats", "hate", "unwanted_sexual", "impersonation", "child_danger", "someone_in_danger", "other"]);
    const DANGER: &str = "If anyone is in danger right now, contact your local emergency number. This server's admins are volunteers, not police.";
    for id in ["child_danger", "someone_in_danger"] {
        assert_eq!(shipped.get(id).unwrap().help, DANGER, "the danger reasons carry the sentence word for word ({id})");
    }
    assert_eq!(ReportReasons::load(), shipped, "the relay reads the file it ships");

    let dir = crate::test_temp::dir("report_reasons");
    assert_eq!(ReportReasons::load_file(&dir.join("missing.json")), shipped, "a missing file: the built-in copy");
    std::fs::write(dir.join("broken.json"), "[{\"id\":\"spam\"}]").unwrap();
    assert_eq!(ReportReasons::load_file(&dir.join("broken.json")), shipped, "a file that does not load: the built-in copy");
    let r = |id: &str| serde_json::json!({ "id": id, "label": "L", "help": "H" });
    assert!(ReportReasons::parse(&serde_json::json!([r("a"), r("a")]).to_string()).is_err(), "a repeated id");
    assert!(ReportReasons::parse(&serde_json::json!([{ "id": "a", "label": "L", "help": "H", "extra": 1 }]).to_string()).is_err(), "an unknown field");
    assert!(ReportReasons::parse("[]").is_err(), "no reasons");
    assert!(ReportReasons::parse(&serde_json::json!([r("Not A Word")]).to_string()).is_err(), "an id that is not a lowercase word");
}

/// The evidence text is read out of the frame exactly as it was sent: compact or spaced, with
/// strings holding braces, brackets, escaped quotes and backslashes, after other members, under
/// a key spelled with an escape, and the last of two (as serde_json reads it); absent, or not an
/// object, is None.
///
/// Seen red 2026-10-09 with `string_end` not skipping an escaped character: "a string holding an
/// escaped quote and a bracket", left `None` (the frame no longer read as an object).
#[test]
fn the_evidence_is_read_out_of_the_frame_as_sent() {
    assert_eq!(raw_member(r#"{"a":1,"evidence":[1,2]}"#, "evidence"), Some("[1,2]"));
    assert_eq!(raw_member("{ \"evidence\" :\n [ 1 , 2 ] ,\"b\":2}", "evidence"), Some("[ 1 , 2 ]"), "spaced");
    let tricky = r#"{"evidence":[{"text":"a \" ] } [ { \\"}],"z":"]"}"#;
    assert_eq!(raw_member(tricky, "evidence"), Some(r#"[{"text":"a \" ] } [ { \\"}]"#), "a string holding an escaped quote and a bracket");
    assert_eq!(raw_member(r#"{"x":{"evidence":[9]},"evidence":[]}"#, "evidence"), Some("[]"), "only the top level");
    assert_eq!(raw_member(r#"{"evidence":[3]}"#, "evidence"), Some("[3]"), "a key spelled with an escape");
    assert_eq!(raw_member(r#"{"evidence":[1],"evidence":[2]}"#, "evidence"), Some("[2]"), "the last of two, as serde_json reads it");
    assert_eq!(raw_member(r#"{"a":"evidence"}"#, "evidence"), None);
    assert_eq!(raw_member("[1]", "evidence"), None);
    assert_eq!(raw_member(r#"{"n":-1.5e3,"t":true,"evidence":null}"#, "evidence"), Some("null"));
}

/// The slash commands read the new table: `/reports` lists the latest reports to admins and
/// moderators (a moderator seeing "a member" for whoever reported) and refuses anyone else;
/// `/report` says how reports are made now and keeps nothing; `/reports-clear` is for admins and
/// deletes only the decided reports.
///
/// Seen red 2026-10-09 with `handle_slash` passing `true` to `item_json` for every role: "a
/// moderator's list says a member" (its lines read "from Rae").
#[test]
fn the_slash_commands_read_the_new_table() {
    let st = fresh_state("slash");
    let (rae, tom, ada, mo, ned) = (person(1), person(2), person(3), person(4), person(5));
    st.db.register_name("Rae", &rae.key).unwrap();
    st.db.register_name("Tom", &tom.key).unwrap();
    st.db.set_role(&ada.key, "admin").unwrap();
    st.db.set_role(&mo.key, "mod").unwrap();
    let open = received(&send(&st, &rae, frame(&rae, &tom.key, "scam", "profile", serde_json::json!([]))), &rae.key).unwrap();
    let other = person(8);
    let decided = received(&send(&st, &rae, frame(&rae, &other.key, "spam", "profile", serde_json::json!([]))), &rae.key).unwrap();
    st.db.report_record_decision(decided, &ada.key, "dismiss", "", 1).unwrap();
    let slash = |who: &Person, cmd: &str| -> Vec<String> {
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_slash(&st, &who.key, cmd));
        heard(&drain(&mut rx), &who.key)
    };
    let admin = slash(&ada, "/reports").join("\n");
    assert!(admin.contains(&format!("#{open} | open | Scam or fraud | about Tom | from Rae")), "{admin}");
    assert!(admin.contains(&format!("#{decided} | decided: dismiss")), "{admin}");
    let moderator = slash(&mo, "/reports").join("\n");
    assert!(moderator.contains("from a member") && !moderator.contains("Rae"), "a moderator's list says a member: {moderator}");
    assert!(slash(&ned, "/reports")[0].contains("Only admins and moderators"));
    assert!(slash(&rae, "/report")[0].contains("use Report"));
    assert!(slash(&mo, "/reports-clear")[0].contains("Only admins"));
    assert!(slash(&ada, "/reports-clear")[0].contains("Cleared 1 decided"));
    assert!(st.db.report_by_id(open).unwrap().is_some() && st.db.report_by_id(decided).unwrap().is_none(), "the open report stays");
}

/// A reported post's uploaded file and its web links never reach admins as something they can
/// open (2026-10-10, the report-duties finding). Seen red 2026-10-10 with `without_files_or_links`
/// returning the text unchanged: "the upload's address reached the admins".
#[test]
fn a_reported_posts_files_and_links_are_not_shown_to_admins() {
    let text = "look ![x](/uploads/abc123.png) and https://evil.example/path?q=1 or http://[::1]/y\nbye";
    let shown = without_files_or_links(text);
    assert!(!shown.contains("/uploads/"), "the upload's address reached the admins: {shown}");
    assert!(!shown.contains("://"), "a link reached the admins: {shown}");
    assert!(shown.contains("[a file posted here, not shown]"), "{shown}");
    assert!(shown.contains("[a link to evil.example, not shown]"), "{shown}");
    assert!(shown.contains("[a link, not shown]"), "a link with no plain host still goes: {shown}");
    assert!(shown.starts_with("look ") && shown.ends_with("\nbye"), "the words and line breaks around them stay: {shown}");
    assert_eq!(without_files_or_links("just words, no links"), "just words, no links");
}

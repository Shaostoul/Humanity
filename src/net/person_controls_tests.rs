// Child of net/person_controls.rs (#[path]): the per-person record and the files rule, held to the
// shared cases the website's twin answers too (scripts/tests/fixtures/person-controls-cases.json).

use super::*;

fn fixture() -> serde_json::Value {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/person-controls-cases.json");
    serde_json::from_str(&std::fs::read_to_string(&path).expect("the shared cases")).expect("they parse")
}

fn controls_of(v: &serde_json::Value) -> Controls {
    serde_json::from_value(v.clone()).expect("a record always reads")
}

/// EVERY SHARED FILE CASE, as the website answers it too: nothing chosen leaves Files in messages
/// to decide; Always ask and Never whatever the size; Up to N MB takes a file within N and asks
/// for a bigger one, only narrows under the protected setup, and keeps N in 1..1024.
/// Seen red 2026-11-03 with the limit compared by `>=`: "up to 10 MB, exactly 10 MB is taken"
/// answered ask_over. (The website's twin was seen red on "up to 10 MB under the protected setup:
/// within the limit my setting decides" with that fallback taken out.)
#[test]
fn every_shared_file_case_gets_the_same_answer() {
    let fx = fixture();
    let cases = fx["files"].as_array().expect("files");
    assert!(cases.len() >= 12, "the shared cases shrank to {}", cases.len());
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let c = controls_of(&case["controls"]);
        let got = file_verdict(&c, case["size"].as_u64().unwrap(), case["protected"].as_bool().unwrap());
        assert_eq!(got.id(), case["expect"].as_str().unwrap(), "{name}");
        if let Some(mb) = case.get("mb").and_then(|v| v.as_u64()) {
            assert_eq!(got, FileVerdict::AskOver(mb as u32), "{name}: the limit it is over");
        }
    }
}

/// A SAVED RECORD READS SAFELY: anything missing or unknown is its default, so a hand-edited or
/// older `config.json` never fails to load (a record that failed to read would take the whole
/// config with it). Seen red 2026-11-03 with an unknown word read as `mentions`: "words from
/// elsewhere read as the defaults: notices" got mentions.
#[test]
fn every_shared_record_reads_as_its_defaults_where_it_says_nothing_usable() {
    let fx = fixture();
    for case in fx["records"].as_array().expect("records") {
        let name = case["name"].as_str().unwrap();
        let c = controls_of(&case["raw"]);
        let want = &case["expect"];
        assert_eq!(c.notices.id(), want["notices"].as_str().unwrap(), "{name}: notices");
        assert_eq!(c.files.id(), want["files"].as_str().unwrap(), "{name}: files");
        assert_eq!(c.files_mb as u64, want["files_mb"].as_u64().unwrap(), "{name}: files_mb");
        assert_eq!(c.is_default(), want["default"].as_bool().unwrap(), "{name}: default");
    }
}

/// THE RECORD KEEPS ONLY WHAT DIFFERS, by identity: a choice for someone is read back under the
/// same identity in any letter case and not under another; going back to the defaults removes the
/// entry (and the identity once it has none); nothing is kept for myself or without a key; and
/// THE rule is handed a level only when one was chosen. Seen red 2026-11-03 with `set` keeping
/// the defaults as an entry: "back to the defaults leaves nothing kept" failed.
#[test]
fn the_record_keeps_only_what_differs_by_identity() {
    let mut p = PeopleControls::default();
    let quiet = Controls { notices: Notices::Nothing, ..Default::default() };
    p.set("ME", "Ab12", quiet);
    assert_eq!(p.get("me", "ab12"), quiet, "read back in any case");
    assert_eq!(p.get("other", "ab12"), Controls::default(), "another identity has its own");
    assert_eq!(p.level_of("me", "ab12"), Some(Notices::Nothing));
    assert_eq!(p.level_of("me", "cd34"), None, "nothing chosen: no level");
    assert_eq!(p.level_of("me", ""), None, "no sender: no level");

    p.set("me", "ab12", Controls::default());
    assert!(p.0.is_empty(), "back to the defaults leaves nothing kept: {:?}", p.0);

    p.set("me", "me", quiet);
    p.set("", "ab12", quiet);
    p.set("me", "", quiet);
    assert!(p.0.is_empty(), "never for myself, never without both keys");

    // A size kept with Up to, then another choice picked: still kept (it is there again), and
    // with Everything and my setting the entry goes.
    let up = Controls { files: FilesChoice::UpTo, files_mb: 50, ..Default::default() };
    p.set("me", "ab12", up);
    assert_eq!(p.get("me", "ab12").mb(), 50);
    p.set("me", "ab12", Controls { files: FilesChoice::Mine, ..up });
    assert!(p.0.is_empty(), "my setting again with Everything is the defaults, whatever size was kept");
}

/// The record survives `config.json` exactly (serde both ways), keys and all.
#[test]
fn the_record_round_trips_through_json() {
    let mut p = PeopleControls::default();
    p.set("me", "ab12", Controls { notices: Notices::Mentions, files: FilesChoice::UpTo, files_mb: 25 });
    p.set("me", "cd34", Controls { files: FilesChoice::Never, ..Default::default() });
    let text = serde_json::to_string(&p).unwrap();
    assert_eq!(serde_json::from_str::<PeopleControls>(&text).unwrap(), p, "{text}");
    assert!(text.contains("\"up_to\"") && text.contains("\"mentions\"") && text.contains("\"never\""), "{text}");
}

/// Each verdict that asks or refuses has its line, by key, naming the person (and the limit).
#[test]
fn each_asking_verdict_has_its_line() {
    assert_eq!(verdict_line(FileVerdict::AskAlways, "Ann").as_deref(), Some("You chose to be asked about every file from Ann. Accept this one?"));
    assert_eq!(
        verdict_line(FileVerdict::AskOver(10), "Ann").as_deref(),
        Some("This file is over the 10 MB you take from Ann without asking. Accept this one?")
    );
    assert_eq!(verdict_line(FileVerdict::Never, "Ann").as_deref(), Some("You chose not to take files from Ann."));
    assert_eq!(verdict_line(FileVerdict::Mine, "Ann"), None);
    assert_eq!(verdict_line(FileVerdict::Accept, "Ann"), None);
}

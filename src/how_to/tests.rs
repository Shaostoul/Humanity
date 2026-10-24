//! The how-to tests (increment 1). Run with `just test-lib how_to`; the ones whose names hold
//! `shipped` also run in `just validate-data`.
//!
//! What they prove, the design's list: the shipped data loads; every gate refuses its broken
//! fixture (tests/fixtures/how_to/broken.json), which is how each one is seen red; Short never hides
//! a serious or lethal note; a stale review hash fails; a run survives its steps being reordered
//! and edited, and a restart; the schema, the fixture and this model name the same fields; and the
//! Rust hash and the JavaScript hash (data/how_to/index.json) are the same bytes.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value as Json;

use super::check::{self, Code, Context};
use super::library::{self, Library};
use super::model::{HowTo, Kind};
use super::runs::{Outcome, Run, RunStore};
use super::view::{self, View};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn data() -> PathBuf {
    repo().join("data")
}

/// The app's real pages and Settings sections, so go steps resolve as they do in the app.
fn pages() -> Option<library::Pages> {
    #[cfg(feature = "native")]
    {
        Some(crate::gui::pages::how_to::app_pages())
    }
    #[cfg(not(feature = "native"))]
    {
        None
    }
}

fn shipped() -> Library {
    Library::load(&data(), pages())
}

/// The context the fixtures are checked in: the shipped data's, with every written how-to known.
fn fixture_context() -> Context {
    let lib = shipped();
    let mut cx = library::context_from(&data(), &lib.categories, &lib.planned, pages());
    for l in &lib.how_tos {
        cx.written.insert(l.how_to.id.clone(), l.file.clone());
    }
    cx
}

fn read_json(p: &Path) -> Json {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
}

/// Apply one edit of broken.json to `doc`.
fn apply(doc: &mut Json, edit: &Json) {
    let path = edit["path"].as_str().expect("an edit has a path");
    let parts: Vec<String> = path.trim_start_matches('/').split('/').map(|s| s.replace("~1", "/").replace("~0", "~")).collect();
    let (last, parents) = parts.split_last().expect("a path names something");
    let value = if let Some(n) = edit["fill"].as_u64() {
        Some(Json::String("x".repeat(n as usize)))
    } else if edit["hash"].as_bool() == Some(true) {
        Some(Json::String(super::canonical::hash(doc).expect("the edited fixture hashes")))
    } else if edit["delete"].as_bool() == Some(true) {
        None
    } else {
        Some(edit["set"].clone())
    };
    let mut at = doc;
    for p in parents {
        at = match at {
            Json::Object(o) => o.entry(p.clone()).or_insert(Json::Object(Default::default())),
            Json::Array(a) => &mut a[p.parse::<usize>().unwrap_or_else(|_| panic!("{path}: {p} is not an index"))],
            _ => panic!("{path}: {p} is not inside an object or array"),
        };
    }
    match (at, value) {
        (Json::Object(o), Some(v)) => {
            o.insert(last.clone(), v);
        }
        (Json::Object(o), None) => {
            o.remove(last);
        }
        (Json::Array(a), Some(v)) if last == "-" => a.push(v),
        (Json::Array(a), Some(v)) => a[last.parse::<usize>().unwrap()] = v,
        (Json::Array(a), None) => {
            a.remove(last.parse::<usize>().unwrap());
        }
        _ => panic!("{path}: cannot edit there"),
    }
}

struct Case {
    why: String,
    code: Option<Code>,
    file: String,
    bytes: Vec<u8>,
    doc: Option<Json>,
}

/// Every case of broken.json (with `code`) and every pass (without), edited and serialised.
fn cases() -> Vec<Case> {
    let fx = read_json(&repo().join("tests/fixtures/how_to/broken.json"));
    let base = read_json(&repo().join("tests/fixtures/how_to/base.json"));
    let base_file = fx["base_file"].as_str().unwrap().to_string();
    let mut out = Vec::new();
    for (list, has_code) in [("cases", true), ("passes", false)] {
        for c in fx[list].as_array().unwrap() {
            let why = c["why"].as_str().unwrap_or("").to_string();
            let code = if has_code {
                let id = c["code"].as_str().unwrap();
                Some(Code::from_id(id).unwrap_or_else(|| panic!("broken.json names a code the checker does not have: {id}")))
            } else {
                None
            };
            let file = c["file"].as_str().map(String::from).unwrap_or_else(|| base_file.clone());
            if let Some(raw) = c["raw"].as_str() {
                out.push(Case { why, code, file, bytes: raw.as_bytes().to_vec(), doc: None });
                continue;
            }
            let mut doc = base.clone();
            for e in c["edits"].as_array().into_iter().flatten() {
                apply(&mut doc, e);
            }
            let bytes = serde_json::to_vec_pretty(&doc).unwrap();
            out.push(Case { why, code, file, bytes, doc: Some(doc) });
        }
    }
    out
}

/// THE SHIPPED HOW-TOS LOAD, AND PASS EVERY GATE. A refused shipped file is a build failure, not a
/// file quietly missing from the page.
#[test]
fn the_shipped_how_tos_load_and_pass_every_gate() {
    let lib = shipped();
    assert!(lib.problems.is_empty(), "the tree and the plan read: {:?}", lib.problems);
    let refused: Vec<String> = lib.refused.iter().map(|r| format!("{}: {:?}", r.file, r.findings.iter().map(|f| format!("{} {}", f.code.id(), f.message)).collect::<Vec<_>>())).collect();
    assert!(refused.is_empty(), "shipped how-tos refused:\n{}", refused.join("\n"));
    for want in ["change_furnace_filter", "boil_water_safe", "test_jar_seals", "mix_ors_packet", "back_up_recovery_phrase", "restore_save_snapshot"] {
        assert!(lib.find(want).is_some(), "{want} ships");
    }
    for l in &lib.how_tos {
        assert!(l.review_current, "{}: a shipped review must match its words", l.how_to.id);
        for f in &l.findings {
            eprintln!("warning {}: {} {}", l.file, f.code.id(), f.message);
        }
    }
    assert!(lib.planned.len() >= 30, "about 30 planned how-tos so the gaps count: {}", lib.planned.len());
    assert_eq!(lib.categories.len(), 11, "the starting tree's eleven branches");
    // Every written and planned how-to sits in a branch that exists.
    for p in &lib.planned {
        assert!(lib.category(&p.category).is_some(), "planned {} sits in {}, which is not in the tree", p.id, p.category);
    }
    // The ORS how-to names the game's existing recipe, and the checker resolved it.
    assert_eq!(lib.find("mix_ors_packet").unwrap().how_to.makes.as_ref().map(|m| m.recipe.as_str()), Some("mix_ors"));
}

/// EVERY GATE IS SEEN RED. Each case of broken.json must be refused (or, for a warning, warned)
/// with its code, and every code the checker has must have a case: a gate with no red is a gate
/// that may not be able to fail.
#[test]
fn every_gate_refuses_its_broken_fixture() {
    let cx = fixture_context();
    let mut red: BTreeSet<Code> = BTreeSet::new();
    let mut failures = Vec::new();
    for c in cases().iter().filter(|c| c.code.is_some()) {
        let code = c.code.unwrap();
        let (findings, _) = check::check_file(&c.file, &c.bytes, &cx);
        if findings.iter().any(|f| f.code == code) {
            red.insert(code);
        } else {
            failures.push(format!("{} ({}): got {:?}", code.id(), c.why, findings.iter().map(|f| f.code.id()).collect::<Vec<_>>()));
        }
    }
    assert!(failures.is_empty(), "gates that did not fire on their fixture:\n{}", failures.join("\n"));
    let never: Vec<&str> = Code::ALL.iter().filter(|c| !red.contains(c)).map(|c| c.id()).collect();
    assert!(never.is_empty(), "codes with no red fixture in tests/fixtures/how_to/broken.json: {never:?}");
}

/// THE GATES DO NOT FIRE ON WHAT IS ALLOWED: the base itself and every `passes` edit leave no
/// refusal, so a gate cannot pass its red test by refusing everything.
#[test]
fn the_base_fixture_and_the_allowed_edits_pass() {
    let cx = fixture_context();
    let base = std::fs::read(repo().join("tests/fixtures/how_to/base.json")).unwrap();
    let (findings, parsed) = check::check_file("household/fixture_base.json", &base, &cx);
    let refusing: Vec<String> = check::refusals(&findings).map(|f| format!("{} {}", f.code.id(), f.message)).collect();
    assert!(refusing.is_empty(), "the base fixture is refused: {refusing:?}");
    assert!(parsed.is_some(), "and parsed by the model, every field of it");
    for c in cases().iter().filter(|c| c.code.is_none()) {
        let (findings, _) = check::check_file(&c.file, &c.bytes, &cx);
        let refusing: Vec<String> = check::refusals(&findings).map(|f| format!("{} {}", f.code.id(), f.message)).collect();
        assert!(refusing.is_empty(), "{}: refused {refusing:?}", c.why);
    }
}

/// Every key path of a document, in the schema's notation (`steps[].safety.level`).
fn paths_of(v: &Json, at: &str, out: &mut BTreeSet<String>) {
    match v {
        Json::Object(o) => {
            for (k, x) in o {
                let p = if at.is_empty() { k.clone() } else { format!("{at}.{k}") };
                out.insert(p.clone());
                // `if` is a map whose keys the author chooses: its own entry is the whole field.
                if k == "if" {
                    continue;
                }
                paths_of(x, &p, out);
            }
        }
        Json::Array(a) => {
            for x in a {
                paths_of(x, &format!("{at}[]"), out);
            }
        }
        _ => {}
    }
}

/// The field paths schemas/how_to.toml lists in its `[fields...]` tables.
fn schema_paths() -> BTreeSet<String> {
    let text = std::fs::read_to_string(repo().join("schemas/how_to.toml")).unwrap();
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("[fields.").and_then(|r| r.strip_suffix(']')))
        .map(|p| p.trim_matches('"').to_string())
        .collect()
}

/// THE SCHEMA, THE FIXTURE AND THE MODEL NAME THE SAME FIELDS. Every field the schema lists is
/// used by the base fixture or one of its edits, every field they use is listed, and the model
/// parses the base (it refuses unknown fields), so none of the three can grow a field alone.
#[test]
fn the_schema_lists_every_field_and_the_model_knows_them() {
    let listed = schema_paths();
    let mut used = BTreeSet::new();
    paths_of(&read_json(&repo().join("tests/fixtures/how_to/base.json")), "", &mut used);
    for c in cases() {
        // Only the edits that pass the shape: a misspelt field is used precisely to be refused.
        if let Some(doc) = &c.doc {
            if serde_json::from_value::<HowTo>(doc.clone()).is_ok() {
                paths_of(doc, "", &mut used);
            }
        }
    }
    let unlisted: Vec<&String> = used.difference(&listed).collect();
    let unused: Vec<&String> = listed.difference(&used).collect();
    assert!(unlisted.is_empty(), "fields the fixture uses that schemas/how_to.toml does not list: {unlisted:?}");
    assert!(unused.is_empty(), "fields schemas/how_to.toml lists that no fixture uses (add them to base.json): {unused:?}");
}

/// SHORT NEVER HIDES A SERIOUS OR LETHAL NOTE: on every shipped how-to, and on a file built to try
/// (which the checker also refuses, gate short_hides_safety, so this is the second of two locks).
#[test]
fn short_never_hides_a_serious_or_lethal_note() {
    let lib = shipped();
    let mut grave_seen = 0;
    for l in &lib.how_tos {
        let mut run = Run::start(&l.how_to, &l.hash, 0);
        // With every choice made each way, so steps behind an `if` are looked at too.
        for s in l.how_to.steps.iter().filter(|s| s.kind == Kind::Choose) {
            for o in &s.options {
                run.choices.insert(s.id.clone(), o.id.clone());
                let short: HashSet<&str> = view::shown(&l.how_to, Some(&run), View::Short).iter().map(|s| s.id.as_str()).collect();
                for s in view::visible_steps(&l.how_to, Some(&run)) {
                    if s.level().map(|lv| lv.is_grave()).unwrap_or(false) {
                        grave_seen += 1;
                        assert!(short.contains(s.id.as_str()), "{}: the {} note at step {} is missing from Short", l.how_to.id, s.level().unwrap().hazard().id(), s.id);
                        assert!(s.safety.as_ref().and_then(|x| x.short.as_deref()).is_some(), "{}: step {} has no short line", l.how_to.id, s.id);
                    }
                }
            }
        }
    }
    assert!(grave_seen > 0, "the shipped how-tos carry serious and lethal notes for this test to look at");

    let mut base: HowTo = serde_json::from_slice(&std::fs::read(repo().join("tests/fixtures/how_to/base.json")).unwrap()).unwrap();
    let lethal = base.steps.iter().position(|s| s.level().map(|l| l.is_grave()).unwrap_or(false)).unwrap();
    base.steps[lethal].short = Some(false);
    let mut run = Run::start(&base, "", 0);
    run.choices.insert("choose_kind".into(), "gas".into());
    let short: Vec<&str> = view::shown(&base, Some(&run), View::Short).iter().map(|s| s.id.as_str()).collect();
    assert!(short.contains(&base.steps[lethal].id.as_str()), "short: false cannot take a lethal step out of Short");
    let bytes = serde_json::to_vec(&base).unwrap();
    let (findings, _) = check::check_file("household/fixture_base.json", &bytes, &fixture_context());
    assert!(findings.iter().any(|f| f.code == Code::ShortHidesSafety), "and the checker refuses the file");
}

/// A STALE REVIEW HASH FAILS, on a real how-to: checked, it passes; one word changed, it is refused
/// at build time and shown as "Changed since it was checked" at run time.
#[test]
fn a_stale_review_hash_fails_on_a_broken_file() {
    let path = data().join("how_to/household/change_furnace_filter.json");
    let mut doc = read_json(&path);
    let hash = super::canonical::hash(&doc).unwrap();
    doc["review"] = serde_json::json!({ "status": "verified", "hash": hash, "checked": "2026-10-23", "by": "test", "person": "2026-10-23" });
    let cx = fixture_context();
    let file = "household/change_furnace_filter.json";
    let (ok, _) = check::check_file(file, &serde_json::to_vec(&doc).unwrap(), &cx);
    assert!(check::refusals(&ok).next().is_none(), "checked against its own words it passes: {ok:?}");
    doc["steps"][0]["text"] = Json::String("Turn off the furnace's electrical supply first.".into());
    let (stale, _) = check::check_file(file, &serde_json::to_vec(&doc).unwrap(), &cx);
    assert!(stale.iter().any(|f| f.code == Code::ReviewHashStale), "one word changed: {stale:?}");

    // At run time, in a data folder a person edited, it is still shown, marked as changed.
    let dir = crate::test_temp::dir("how_to_stale");
    let root = dir.path().join("data");
    copy_tree(&data().join("how_to"), &root.join("how_to"));
    for f in ["items.csv", "recipes.csv"] {
        std::fs::copy(data().join(f), root.join(f)).unwrap();
    }
    std::fs::write(root.join("how_to/household/change_furnace_filter.json"), serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    let lib = Library::load(&root, pages());
    let l = lib.find("change_furnace_filter").expect("a changed how-to is still shown");
    assert!(!l.review_current, "and marked: Changed since it was checked");
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let dest = to.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &dest);
        } else {
            std::fs::copy(&p, &dest).unwrap();
        }
    }
}

/// A RUN SURVIVES ITS STEPS BEING REORDERED AND EDITED, AND A RESTART. Ticks are keyed by step
/// id, so they follow their steps wherever they move; a step that went is listed as removed; and
/// the run read back from disk is the run that was written.
#[test]
fn a_run_survives_its_steps_being_reordered_and_edited() {
    let lib = shipped();
    let l = lib.find("change_furnace_filter").unwrap();
    let h = &l.how_to;
    let mut run = Run::start(h, &l.hash, 1_792_789_331_000);
    run.set_done("power_off", true, 1_792_789_340_000);
    run.record("size", "16 x 25 x 1", 1_792_789_350_000);
    run.choose("filter_kind", "disposable", 1_792_789_360_000);
    assert_eq!(view::current_step(h, &run).map(|s| s.id.as_str()), Some("take_out"), "the first step not done");
    let (done, total) = view::progress(h, &run);
    assert_eq!(done, 3);
    assert!(total < h.steps.len(), "the washable branch is hidden after choosing throw-away");

    // Saved, and read back as after a restart.
    let dir = crate::test_temp::dir("how_to_runs");
    let store = RunStore::new(dir.path().join("runs"));
    store.save(&run).unwrap();
    let back = store.list();
    assert_eq!(back, vec![run.clone()], "the run on disk is the run");

    // The how-to is edited while the run is open: steps reversed, one reworded, one removed.
    let mut edited = h.clone();
    edited.steps.reverse();
    edited.steps.iter_mut().find(|s| s.id == "size").unwrap().text = "Note the old filter's size.".into();
    edited.steps.retain(|s| s.id != "power_off");
    let run = &back[0];
    assert!(run.is_done("size") && run.is_done("filter_kind"), "the ticks follow their steps");
    assert_eq!(run.steps["size"].value.as_deref(), Some("16 x 25 x 1"), "and so do the values");
    assert_eq!(view::removed_steps(&edited, run), vec!["power_off".to_string()], "a removed step is listed");
    assert_eq!(view::current_step(&edited, run).map(|s| s.id.as_str()), Some("power_on"), "the reversed order's first undone step");
}

/// A CHECK ANSWERED NO goes to its problem (the check is answered) or back to its step (that step
/// and everything after it up to the check are done again).
#[test]
fn a_no_goes_to_its_problem_or_back_to_its_step() {
    let lib = shipped();
    let furnace = &lib.find("change_furnace_filter").unwrap().how_to;
    let mut run = Run::start(furnace, "", 0);
    run.answer(furnace, "arrow", false, 1000);
    assert!(run.is_done("arrow"), "answered");
    assert_eq!(run.steps["arrow"].went_to.as_deref(), Some("arrow_backwards"), "to its problem");

    let phrase = &lib.find("back_up_recovery_phrase").unwrap().how_to;
    let mut run = Run::start(phrase, "", 0);
    for s in ["private", "open_account", "unlock", "show", "write"] {
        run.set_done(s, true, 1000);
    }
    run.answer(phrase, "check_back", false, 2000);
    assert!(!run.is_done("write") && !run.is_done("check_back"), "back to writing, and the check asked again");
    assert!(run.is_done("show"), "the steps before it stay done");
    assert_eq!(view::current_step(phrase, &run).map(|s| s.id.as_str()), Some("write"));
    run.answer(phrase, "check_back", true, 3000);
    assert_eq!(run.steps["check_back"].answer.as_deref(), Some("yes"));

    let mut timed = Run::start(phrase, "", 10_000);
    timed.start_wait("x", 60, 10_000);
    assert_eq!(timed.wait_left_ms("x", 40_000), Some(30_000));
    assert_eq!(timed.wait_left_ms("x", 99_000), Some(0), "a timer that ended while the app was closed is due");
    timed.finish(Outcome::Worked, 100_000);
    assert!(!timed.is_open() && timed.waits.is_empty());
}

/// SEARCH FINDS A HOW-TO BY ITS SYMPTOM: people arrive with what they see, not a title.
#[test]
fn search_finds_a_how_to_by_the_sign_of_its_problem() {
    let lib = shipped();
    let seals = &lib.find("test_jar_seals").unwrap().how_to;
    assert!(view::matches(seals, "lid springs up"), "a problem's sign");
    assert!(view::matches(seals, "teaspoon"), "a tool");
    assert!(!view::matches(seals, "furnace arrow"));
}

/// THE TWO HASHES AGREE ON EVERY SHIPPED HOW-TO: the one this crate works out and the one
/// scripts/build-how-to.js writes into data/how_to/index.json from web/shared/how-to-logic.js. A
/// review made on one side must bind on the other.
#[test]
fn the_two_hashes_agree_on_every_shipped_how_to() {
    let index = read_json(&data().join("how_to/index.json"));
    let lib = shipped();
    let listed = index["how_tos"].as_array().expect("index.json lists how_tos");
    assert_eq!(listed.len(), lib.how_tos.len(), "index.json lists every shipped how-to (run node scripts/build-how-to.js)");
    for l in &lib.how_tos {
        let entry = listed.iter().find(|e| e["id"] == l.how_to.id.as_str()).unwrap_or_else(|| panic!("{} missing from index.json", l.how_to.id));
        assert_eq!(entry["hash"].as_str(), Some(l.hash.as_str()), "{}: the JavaScript hash and the Rust hash differ", l.how_to.id);
    }
}

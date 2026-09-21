//! `just snapshot <name>` must not claim to render a page it did not render.
//!
//! THE TRAP. The recipe ran `cargo test --lib snapshot_<name>` and then echoed
//! `Wrote tests/snapshots/<name>.png` unconditionally. Cargo exits 0 when a test
//! FILTER matches nothing, so a typo, or a page whose test was deleted, reported
//! success and rendered nothing. If a stale PNG of that name happened to be
//! sitting in `tests/snapshots/`, opening it showed a picture and the whole
//! thing looked like it had worked. That is the house's most expensive defect
//! class: a check whose evidence is the setup rather than the result.
//!
//! The name is now validated before cargo runs (`scripts/snapshot-name.js`).
//! These two tests guard the parts of that which can rot:
//!
//!   1. The recipe still CALLS the validator. Deleting that line restores the
//!      original trap silently.
//!   2. No PNG in `tests/snapshots/` outlives the test that wrote it, so a stale
//!      picture can never stand in for a render that never happened.
//!
//! Standalone by design (std only, no crate imports) so it runs without linking
//! the bin - see the LNK1318 note in CLAUDE.md.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every name `cargo test --lib snapshot_<name>` can actually match: the
/// macro-generated ones and the hand-written ones that really carry `#[test]`.
/// A plain helper `fn snapshot_x` is deliberately NOT counted - cargo's filter
/// matches tests, and counting a helper would make this permissive in exactly
/// the direction it exists to prevent.
fn snapshot_test_names() -> BTreeSet<String> {
    let src = fs::read_to_string(repo_root().join("src/gui/ui_snapshots.rs"))
        .expect("read src/gui/ui_snapshots.rs");
    let lines: Vec<&str> = src.lines().collect();
    let mut out = BTreeSet::new();
    for (i, raw) in lines.iter().enumerate() {
        let t = raw.trim();
        if let Some(rest) = t.strip_prefix("page_snapshot!(snapshot_") {
            if let Some(name) = rest.split(',').next() {
                out.insert(name.trim().to_string());
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("fn snapshot_") {
            let lo = i.saturating_sub(4);
            let attributed = lines[lo..i].iter().any(|l| l.trim().starts_with("#[test]"));
            if attributed {
                if let Some(name) = rest.split('(').next() {
                    out.insert(name.trim().to_string());
                }
            }
        }
    }
    assert!(
        out.len() > 40,
        "found only {} snapshot tests, which means this parser stopped matching the file \
         rather than that the tests vanished - fix the parser before trusting a pass",
        out.len()
    );
    out
}

#[test]
fn the_snapshot_recipe_still_validates_its_name_before_running_cargo() {
    let just = fs::read_to_string(repo_root().join("Justfile")).expect("read Justfile");
    let at = just.find("\nsnapshot name:").expect("the `snapshot name:` recipe");
    let body: String = just[at + 1..].lines().take(4).collect::<Vec<_>>().join("\n");
    assert!(
        body.contains("scripts/snapshot-name.js"),
        "`just snapshot` no longer validates its argument. Cargo exits 0 on a filter that \
         matches nothing and the recipe echoes \"Wrote ...\" afterwards, so without that \
         line a typo reports success and renders nothing. Recipe body was:\n{body}"
    );
    assert!(
        fs::metadata(repo_root().join("scripts/snapshot-name.js")).is_ok(),
        "the recipe calls scripts/snapshot-name.js and it is not there"
    );
}

#[test]
fn no_snapshot_png_outlives_the_test_that_wrote_it() {
    let tests = snapshot_test_names();
    let dir = repo_root().join("tests/snapshots");
    let mut orphans: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("png") {
                continue;
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if !tests.contains(stem) {
                    orphans.push(stem.to_string());
                }
            }
        }
    }
    orphans.sort();
    assert!(
        orphans.is_empty(),
        "tests/snapshots/ holds {} PNG(s) with no test that could have written them: \
         {orphans:?}\nA stale picture is worse than a missing one: it makes a render that \
         never ran look like it did. Delete the file, or restore the test.",
        orphans.len()
    );
}

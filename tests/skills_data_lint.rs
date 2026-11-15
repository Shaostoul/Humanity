//! Skills data lint (skills rung 1, 2026-11-14; docs/design/skills.md).
//!
//! The operator approved the base set of 90 skills and its migration from the twenty old rows
//! ("I'm happy with the list."). A skill id that names no row makes its XP vanish silently (the
//! SkillSystem skips an unknown id) and its gate impossible to meet, and a renamed id left behind
//! is the "differing names" confusion CLAUDE.md's names rule forbids. So this lint holds three
//! claims to the files:
//!
//!   1. every skill a data file names is a row of data/skills/skills.csv: the skill columns of
//!      every CSV under data/ (skill_required, skill), `skill_required:` and `xp_rewards` in the
//!      RON files, and the `skills` lists of the curriculum, the how-tos, the activities catalog and
//!      the practice table;
//!   2. no retired skill id remains in code (shelter_building, metalworking, engineering, melee,
//!      ranged, defense passed to anything that awards or reads skill XP, and the two unambiguous
//!      ids anywhere in a string literal of src/, crates/ or web/);
//!   3. every practice id the code names (skills::practice and its kin) is an entry of
//!      data/skills/practice.json, so a verb never teaches nothing by a typo.
//!
//! The eleven fantasy schools of data/abilities.csv (pyromancy and the rest) are not skills and
//! never were rows; skills.md section 9 question 9 (decided: fantasy abilities stop naming a skill
//! and unlock by quest or setting) is a later rung, so they are listed in FANTASY_SCHOOLS, a list
//! that only shrinks.
//!
//! Seen red 2026-11-14 on the tree before rung 1 (this lint compiled against a copy of HEAD):
//! check 1 failed on "got 20 rows", and with the new skills.csv over the old tree on 15 distinct
//! references: recipes.csv (shelter_building, metalworking, engineering), the syllabus (those
//! three and ranged), getting_started.ron, abilities.csv, the furnace-filter how-to, and four that
//! were never rows at all (docking.ron's "piloting", manufacturing.ron's "crafting",
//! creative_arts.ron's "art", food_system.ron's "engineering"); check 2 on 6 lines (the
//! construction award of "shelter_building", crafting's tests of "metalworking"); check 3 on "data/skills/practice.json exists".

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Not skills: the fantasy schools of data/abilities.csv, pending skills.md question 9's rung.
const FANTASY_SCHOOLS: &[&str] = &[
    "pyromancy", "cryomancy", "geomancy", "aeromancy", "druidism", "shadow_arts", "divinity", "arcana",
    "summoning", "enchanting", "psionics",
];

/// The retired ids (skills.md section 6). The first two mean nothing else, so no string literal
/// may hold them; the rest are ordinary words (a room is "engineering", an NPC fights "melee"),
/// so they are refused only where skill XP is awarded or read.
const RETIRED_UNAMBIGUOUS: &[&str] = &["shelter_building", "metalworking"];
const RETIRED_WORDS: &[&str] = &["engineering", "melee", "ranged", "defense"];

/// The calls that award or read skill XP, whose string arguments are skill or practice ids.
const SKILL_CALLS: &[&str] = &["award_skill_xp(", "award_practice(", ".level(", ".xp(", "practice_with("];
/// The calls whose first string argument is a practice id.
const PRACTICE_CALLS: &[&str] = &["practice(", "practice_first(", "practice_scaled(", "practice_distance(", "queue_practice("];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_default()
}

fn walk(dir: &Path, ext: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if !matches!(name, "target" | "node_modules" | ".git" | "history") {
                walk(&p, ext, out);
            }
        } else if ext.iter().any(|x| name.ends_with(x)) {
            out.push(p);
        }
    }
}

fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

fn skill_ids() -> BTreeSet<String> {
    read("data/skills/skills.csv")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .skip(1)
        .filter_map(|l| l.split(',').next())
        .map(|s| s.trim().to_string())
        .collect()
}

/// The quoted strings in `s` (no escapes needed for ids).
fn quoted(s: &str) -> Vec<&str> {
    s.split('"').skip(1).step_by(2).collect()
}

/// The string items of every `"<key>": [ ... ]` array in a JSON text.
fn json_lists<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let needle = format!("\"{key}\":");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(&needle) {
        rest = &rest[i + needle.len()..];
        let t = rest.trim_start();
        if !t.starts_with('[') {
            continue;
        }
        let end = t.find(']').unwrap_or(t.len());
        out.extend(quoted(&t[1..end]));
    }
    out
}

/// Every skill a data file names: (where, id).
fn named_skills() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut files = Vec::new();
    walk(&root().join("data"), &[".csv", ".ron", ".json"], &mut files);
    for f in files {
        let text = fs::read_to_string(&f).unwrap_or_default();
        let who = rel(&f);
        if who.ends_with(".csv") {
            let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'));
            let header: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
            let cols: Vec<usize> =
                header.iter().enumerate().filter(|(_, h)| matches!(**h, "skill_required" | "skill")).map(|(i, _)| i).collect();
            for l in lines {
                let cells: Vec<&str> = l.split(',').collect();
                for &c in &cols {
                    let v = cells.get(c).map_or("", |s| s.trim());
                    // plants.csv's skill_required is a level (a number), not an id.
                    if !v.is_empty() && !v.chars().all(|ch| ch.is_ascii_digit()) {
                        out.push((who.clone(), v.to_string()));
                    }
                }
            }
        } else if who.ends_with(".ron") {
            for line in text.lines() {
                let l = line.trim();
                if l.starts_with("//") {
                    continue;
                }
                if let Some(i) = l.find("skill_required:") {
                    if let Some(id) = quoted(&l[i..]).first() {
                        out.push((who.clone(), id.to_string()));
                    }
                }
            }
            // Quest rewards: xp_rewards: [ ("skill", n), ... ]
            let mut rest = text.as_str();
            while let Some(i) = rest.find("xp_rewards:") {
                rest = &rest[i..];
                let end = rest.find(']').unwrap_or(rest.len());
                for id in quoted(&rest[..end]) {
                    out.push((who.clone(), id.to_string()));
                }
                rest = &rest[end.min(rest.len())..];
            }
        } else if who == "data/curriculum/syllabus.json"
            || who.starts_with("data/how_to/")
            || who == "data/gameplay/activities.json"
            || who == "data/skills/practice.json"
        {
            for id in json_lists(&text, "skills") {
                out.push((who.clone(), id.to_string()));
            }
        }
    }
    out
}

#[test]
fn every_skill_a_data_file_names_is_a_skill() {
    let ids = skill_ids();
    assert!(ids.len() >= 90, "data/skills/skills.csv holds the base set (got {} rows)", ids.len());
    let mut bad: Vec<String> = named_skills()
        .into_iter()
        .filter(|(_, id)| !ids.contains(id) && !FANTASY_SCHOOLS.contains(&id.as_str()))
        .map(|(who, id)| format!("{who}: `{id}` is not a row of data/skills/skills.csv"))
        .collect();
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "{} skill references name no skill:\n  {}", bad.len(), bad.join("\n  "));
}

/// The string arguments of each call to one of `calls` in `text` (up to the closing paren of a
/// one-line call).
fn call_args<'a>(text: &'a str, calls: &[&str]) -> Vec<(usize, &'a str)> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for c in calls {
            let mut rest = line;
            while let Some(i) = rest.find(c) {
                // A whole name only: "practice(" must not match inside "award_practice(".
                let joined = c.starts_with(|ch: char| ch.is_ascii_alphanumeric())
                    && rest[..i].chars().last().map_or(false, |ch| ch.is_ascii_alphanumeric() || ch == '_');
                let args = &rest[i + c.len()..];
                let end = args.find(')').unwrap_or(args.len());
                if !joined {
                    for q in quoted(&args[..end]) {
                        out.push((n + 1, q));
                    }
                }
                rest = &rest[i + c.len()..];
            }
        }
    }
    out
}

#[test]
fn no_retired_skill_id_remains_in_code() {
    let mut files = Vec::new();
    for dir in ["src", "crates", "web"] {
        walk(&root().join(dir), &[".rs", ".js", ".html"], &mut files);
    }
    let mut bad = Vec::new();
    for f in files {
        let who = rel(&f);
        if who.ends_with("tests/skills_data_lint.rs") {
            continue;
        }
        let text = fs::read_to_string(&f).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            for id in RETIRED_UNAMBIGUOUS {
                if line.contains(&format!("\"{id}\"")) || line.contains(&format!("'{id}'")) {
                    bad.push(format!("{who}:{}: the retired skill id `{id}`", n + 1));
                }
            }
        }
        for (n, arg) in call_args(&text, SKILL_CALLS) {
            if RETIRED_WORDS.contains(&arg) || RETIRED_UNAMBIGUOUS.contains(&arg) {
                bad.push(format!("{who}:{n}: skill XP for the retired id `{arg}`"));
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "retired skill ids remain (skills.md section 6 renames them):\n  {}", bad.join("\n  "));
}

#[test]
fn every_practice_the_code_names_is_in_the_table() {
    let table = read("data/skills/practice.json");
    assert!(!table.is_empty(), "data/skills/practice.json exists");
    // The table's keys: every `"id": {` at the practices level.
    let keys: BTreeSet<String> = table
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            l.starts_with('"').then(|| quoted(l).first().map(|s| s.to_string())).flatten().filter(|_| l.contains("\"skills\""))
        })
        .collect();
    let mut files = Vec::new();
    for dir in ["src", "crates"] {
        walk(&root().join(dir), &[".rs"], &mut files);
    }
    let mut bad = Vec::new();
    let mut named = 0;
    for f in files {
        let who = rel(&f);
        let text = fs::read_to_string(&f).unwrap_or_default();
        for (n, id) in call_args(&text, PRACTICE_CALLS) {
            // Skip the functions' own definitions and tests that feed made-up ids on purpose.
            if who.ends_with("skills/mod.rs") || who.ends_with("skills/skill_tests.rs") {
                continue;
            }
            named += 1;
            // A format string ("collect.{item}", "build.{bp_id}") needs at least one entry with
            // its prefix; a plain id needs its own entry.
            let ok = match id.find('{') {
                Some(i) => keys.iter().any(|k| k.starts_with(&id[..i])),
                None => keys.contains(id),
            };
            if !ok {
                bad.push(format!("{who}:{n}: practice `{id}` is not in data/skills/practice.json"));
            }
        }
    }
    assert!(named >= 10, "the lint found the practice calls ({named})");
    assert!(bad.is_empty(), "verbs that would teach nothing:\n  {}", bad.join("\n  "));
}

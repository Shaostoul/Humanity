//! Activities catalog lint (2026-11-14).
//!
//! `data/gameplay/activities.json` lists every activity (the "minigames") that makes up the one
//! game, each with its true build status, so the Play hub, the skills page, quests and tutorials
//! can enumerate them later (Infinite-of-X: the list is data). The design is
//! `docs/design/activities.md`. A catalog of statuses is only worth anything while it is true, so
//! this lint holds every claim to something it can check:
//!
//!   1. every status is one of live / partial / designed / idea;
//!   2. every evidence path, design doc and real-life twin path exists in the repo;
//!   3. every skill id is a row of data/skills/skills.csv;
//!   4. no live entry lacks a source file (a `.rs` under src/ or crates/) or a FEATURES.md heading,
//!      and no partial entry lacks evidence;
//!   5. every FEATURES.md heading named is a real heading of docs/FEATURES.md;
//!   6. every designed entry names its design doc;
//!   7. every levels_setting is a setting of data/gameplay/levels.json;
//!   8. ids are unique snake_case, families, tiers, engagement modes and budget classes are the
//!      catalog's own lists, and every activity supports the direct mode (an activity IS the
//!      direct mode of its domain, docs/design/engagement-modes.md);
//!   9. every id appears in docs/design/activities.md, and its table row (written by
//!      scripts/activities-tables.js) shows the data's status, so the document and the data cannot
//!      drift;
//!  10. the activities the operator must decide on (SENSITIVE) keep their operator_to_decide
//!      question: the catalog records the question and never decides it.
//!
//! Seen red (2026-11-14), before the shipped catalog was trusted: with `src/engine/fruiting_tent.rs`
//! misspelt `fruting_tent.rs` and the combat entry's operator_to_decide set to null, the shipped
//! catalog test failed with
//!   "mushrooms: evidence path src/engine/fruting_tent.rs does not exist" and
//!   "combat: a sensitive activity must keep its operator_to_decide question".
//! The `broken_fixture_trips_every_check` test keeps that red permanently: it feeds a fixture with
//! one defect of each kind and insists every one is reported.
//!
//! Standalone by design: std only, imports nothing from the crate, reads the repo files at
//! runtime, so it runs without linking the native bin (the LNK1318 note in CLAUDE.md). Run it the
//! way `just lints` does:
//!
//!   CARGO_MANIFEST_DIR="$(pwd)" rustc --test --edition 2021 -A warnings \
//!       tests/activities_catalog_lint.rs -o t.exe && ./t.exe

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const STATUSES: &[&str] = &["live", "partial", "designed", "idea"];

/// Activities whose existence or shape the operator decides. Their entries must keep an
/// operator_to_decide question until he answers; removing one from this list needs his answer,
/// quoted, in the design doc. hunting_butchering left the list on 2026-11-14 with his answer
/// ("They're too valuable to leave out", docs/design/activities.md section 6).
const SENSITIVE: &[&str] = &["combat", "point_defense"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

// ── What the checks need from the rest of the repo ──────────────────────────

struct Repo<'a> {
    root: &'a Path,
    skills: BTreeSet<String>,
    levels: BTreeSet<String>,
    feature_headings: BTreeSet<String>,
    design_doc: String,
}

fn skill_ids(csv: &str) -> BTreeSet<String> {
    csv.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .skip(1) // the header row
        .filter_map(|l| l.split(',').next())
        .map(|s| s.trim().to_string())
        .collect()
}

fn level_ids(levels_json: &str) -> BTreeSet<String> {
    let v = parse_json(levels_json).expect("data/gameplay/levels.json parses");
    let mut out = BTreeSet::new();
    if let Some(Json::Arr(items)) = v.get("settings") {
        for s in items {
            if let Some(Json::Str(id)) = s.get("id") {
                out.insert(id.clone());
            }
        }
    }
    out
}

fn headings(md: &str) -> BTreeSet<String> {
    md.lines()
        .filter(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .collect()
}

// ── The checks ──────────────────────────────────────────────────────────────

fn is_snake(id: &str) -> bool {
    !id.is_empty()
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !id.starts_with('_')
        && !id.ends_with('_')
}

fn is_source_file(p: &str) -> bool {
    (p.starts_with("src/") || p.starts_with("crates/")) && p.ends_with(".rs")
}

fn str_list(v: Option<&Json>) -> Vec<String> {
    match v {
        Some(Json::Arr(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn key_set(v: Option<&Json>) -> BTreeSet<String> {
    match v {
        Some(Json::Obj(o)) => o.iter().map(|(k, _)| k.clone()).collect(),
        Some(Json::Arr(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string).or_else(|| x.get("id").and_then(|i| i.as_str()).map(str::to_string)))
            .collect(),
        _ => BTreeSet::new(),
    }
}

/// Every problem with the catalog, one line each (empty when it is true).
fn check(catalog: &str, repo: &Repo) -> Vec<String> {
    let mut bad = Vec::new();
    let v = match parse_json(catalog) {
        Ok(v) => v,
        Err(e) => return vec![format!("the catalog does not parse: {e}")],
    };
    let families = key_set(v.get("families"));
    let modes = key_set(v.get("engagement_modes"));
    let tiers = key_set(v.get("tiers"));
    let frames = key_set(v.get("frame_budgets"));
    let networks = key_set(v.get("network_classes"));
    let statuses = key_set(v.get("statuses"));
    for s in STATUSES {
        if !statuses.contains(*s) {
            bad.push(format!("the statuses table does not explain `{s}`"));
        }
    }
    let acts = match v.get("activities") {
        Some(Json::Arr(a)) if !a.is_empty() => a,
        _ => return vec!["the catalog has no activities array".into()],
    };
    let exists = |p: &str| repo.root.join(p).exists();
    let mut seen = BTreeSet::new();
    for a in acts {
        let id = a.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let who = if id.is_empty() { "(no id)".to_string() } else { id.clone() };
        if !is_snake(&id) {
            bad.push(format!("{who}: id must be snake_case"));
        }
        if !seen.insert(id.clone()) {
            bad.push(format!("{who}: duplicate id"));
        }
        if a.get("name").and_then(|x| x.as_str()).map_or(true, |n| n.trim().is_empty()) {
            bad.push(format!("{who}: no name"));
        }
        let family = a.get("family").and_then(|x| x.as_str()).unwrap_or("");
        if !families.contains(family) {
            bad.push(format!("{who}: family `{family}` is not in the families list"));
        }
        let status = a.get("status").and_then(|x| x.as_str()).unwrap_or("");
        if !STATUSES.contains(&status) {
            bad.push(format!("{who}: status `{status}` is not one of live / partial / designed / idea"));
        }
        match a.get("skill") {
            Some(Json::Null) | None => {}
            Some(Json::Str(s)) if repo.skills.contains(s) => {}
            Some(other) => bad.push(format!("{who}: skill {} is not a row of data/skills/skills.csv", other.show())),
        }
        let evidence = str_list(a.get("evidence"));
        for p in &evidence {
            if !exists(p) {
                bad.push(format!("{who}: evidence path {p} does not exist"));
            }
        }
        for key in ["design_doc", "real_life_twin_path"] {
            if let Some(Json::Str(p)) = a.get(key) {
                if !exists(p) {
                    bad.push(format!("{who}: {key} {p} does not exist"));
                }
            }
        }
        let heading = a.get("features_heading").and_then(|x| x.as_str());
        if let Some(h) = heading {
            if !repo.feature_headings.contains(h) {
                bad.push(format!("{who}: features_heading `{h}` is not a heading of docs/FEATURES.md"));
            }
        }
        match status {
            "live" => {
                if !evidence.iter().any(|p| is_source_file(p)) {
                    bad.push(format!("{who}: a live entry must name a source file (.rs under src/ or crates/)"));
                }
                if heading.is_none() {
                    bad.push(format!("{who}: a live entry must name its FEATURES.md heading"));
                }
            }
            "partial" if evidence.is_empty() => bad.push(format!("{who}: a partial entry must name the evidence that runs")),
            "designed" if a.get("design_doc").and_then(|x| x.as_str()).is_none() => {
                bad.push(format!("{who}: a designed entry must name its design doc"))
            }
            _ => {}
        }
        if let Some(Json::Str(l)) = a.get("levels_setting") {
            if !repo.levels.contains(l) {
                bad.push(format!("{who}: levels_setting `{l}` is not a setting of data/gameplay/levels.json"));
            }
        }
        match a.get("tier") {
            Some(Json::Null) | None => {}
            Some(Json::Str(t)) if tiers.contains(t) => {}
            Some(other) => bad.push(format!("{who}: tier {} is not in the tiers list", other.show())),
        }
        let em = str_list(a.get("engagement_modes"));
        if !em.iter().any(|m| m == "direct") {
            bad.push(format!("{who}: an activity is the direct mode of its domain, so engagement_modes must include direct"));
        }
        for m in &em {
            if !modes.contains(m) {
                bad.push(format!("{who}: engagement mode `{m}` is not one of the five"));
            }
        }
        let budget = a.get("budget");
        let frame = budget.and_then(|b| b.get("frame")).and_then(|x| x.as_str()).unwrap_or("");
        let network = budget.and_then(|b| b.get("network")).and_then(|x| x.as_str()).unwrap_or("");
        if !frames.contains(frame) {
            bad.push(format!("{who}: budget.frame `{frame}` is not a frame budget class"));
        }
        if !networks.contains(network) {
            bad.push(format!("{who}: budget.network `{network}` is not a network class"));
        }
        if SENSITIVE.contains(&id.as_str())
            && a.get("operator_to_decide").and_then(|x| x.as_str()).map_or(true, |q| q.trim().is_empty())
        {
            bad.push(format!("{who}: a sensitive activity must keep its operator_to_decide question"));
        }
        if !id.is_empty() && !repo.design_doc.contains(&format!("`{id}`")) {
            bad.push(format!("{who}: not named (as `{id}`) in docs/design/activities.md"));
        }
        // The document's table row (scripts/activities-tables.js writes it) shows the data's status.
        let row_start = format!("| `{id}` ");
        if let Some(row) = repo.design_doc.lines().find(|l| l.starts_with(&row_start)) {
            let shown = row.split('|').nth(2).map(str::trim).unwrap_or("");
            if !(shown == status || shown.starts_with(&format!("{status},"))) {
                bad.push(format!(
                    "{who}: the table in docs/design/activities.md says `{shown}`, the data says `{status}` (run node scripts/activities-tables.js)"
                ));
            }
        }
    }
    for s in SENSITIVE {
        if !seen.contains(*s) {
            bad.push(format!("sensitive activity `{s}` is missing from the catalog"));
        }
    }
    bad
}

fn shipped_repo(root: &Path) -> Repo<'_> {
    Repo {
        root,
        skills: skill_ids(&read("data/skills/skills.csv")),
        levels: level_ids(&read("data/gameplay/levels.json")),
        feature_headings: headings(&read("docs/FEATURES.md")),
        design_doc: read("docs/design/activities.md"),
    }
}

#[test]
fn the_shipped_catalog_is_true() {
    let r = root();
    let repo = shipped_repo(&r);
    let bad = check(&read("data/gameplay/activities.json"), &repo);
    assert!(bad.is_empty(), "data/gameplay/activities.json has {} problem(s):\n  {}", bad.len(), bad.join("\n  "));
}

#[test]
fn the_catalog_covers_every_family_and_status() {
    let v = parse_json(&read("data/gameplay/activities.json")).unwrap();
    let Some(Json::Arr(acts)) = v.get("activities") else { panic!("no activities") };
    let used_families: BTreeSet<&str> = acts.iter().filter_map(|a| a.get("family")?.as_str()).collect();
    for f in key_set(v.get("families")) {
        assert!(used_families.contains(f.as_str()), "family `{f}` has no activity");
    }
    let used_status: BTreeSet<&str> = acts.iter().filter_map(|a| a.get("status")?.as_str()).collect();
    for s in STATUSES {
        assert!(used_status.contains(s), "no activity is `{s}`: the catalog is suspiciously uniform");
    }
}

/// The permanent red: one defect of each kind, each of which must be reported.
#[test]
fn broken_fixture_trips_every_check() {
    let r = root();
    let mut repo = shipped_repo(&r);
    repo.design_doc = "`ok_one` `combat` `bad_status`\n| `ok_one` Fine | live | none | x | y |".into();
    let fixture = r#"{
      "statuses": {"live": "", "partial": "", "designed": "", "idea": ""},
      "families": [{"id": "body"}],
      "engagement_modes": ["direct", "trade", "automation", "cooperative", "background"],
      "tiers": ["T0"],
      "frame_budgets": {"none": ""},
      "network_classes": {"local": ""},
      "activities": [
        {"id": "ok_one", "family": "body", "name": "Fine", "skill": null, "status": "idea", "evidence": [],
         "design_doc": null, "tier": "T0", "engagement_modes": ["direct"], "budget": {"frame": "none", "network": "local"}},
        {"id": "bad_status", "family": "nowhere", "name": "X", "skill": "juggling", "status": "rumoured",
         "evidence": ["src/engine/fruting_tent.rs"], "design_doc": "docs/design/nope.md", "tier": "T9",
         "levels_setting": "no_such_level", "features_heading": "No Such Heading",
         "engagement_modes": ["trade", "telepathy"], "budget": {"frame": "huge", "network": "psychic"}},
        {"id": "live_no_source", "family": "body", "name": "Y", "skill": null, "status": "live",
         "evidence": ["data/plants.csv"], "tier": "T0", "engagement_modes": ["direct"],
         "budget": {"frame": "none", "network": "local"}},
        {"id": "designed_no_doc", "family": "body", "name": "Z", "skill": null, "status": "designed",
         "evidence": [], "design_doc": null, "tier": "T0", "engagement_modes": ["direct"],
         "budget": {"frame": "none", "network": "local"}},
        {"id": "ok_one", "family": "body", "name": "Dupe", "skill": null, "status": "idea", "evidence": [],
         "tier": "T0", "engagement_modes": ["direct"], "budget": {"frame": "none", "network": "local"}},
        {"id": "combat", "family": "body", "name": "Combat", "skill": "melee", "status": "idea", "evidence": [],
         "operator_to_decide": null, "tier": "T0", "engagement_modes": ["direct"], "budget": {"frame": "none", "network": "local"}}
      ]
    }"#;
    let bad = check(fixture, &repo).join("\n");
    for needle in [
        "bad_status: family `nowhere`",
        "bad_status: status `rumoured`",
        "bad_status: skill \"juggling\"",
        "bad_status: evidence path src/engine/fruting_tent.rs does not exist",
        "bad_status: design_doc docs/design/nope.md does not exist",
        "bad_status: features_heading `No Such Heading`",
        "bad_status: levels_setting `no_such_level`",
        "bad_status: tier \"T9\"",
        "bad_status: an activity is the direct mode",
        "bad_status: engagement mode `telepathy`",
        "bad_status: budget.frame `huge`",
        "bad_status: budget.network `psychic`",
        "live_no_source: a live entry must name a source file",
        "live_no_source: a live entry must name its FEATURES.md heading",
        "live_no_source: not named",
        "designed_no_doc: a designed entry must name its design doc",
        "ok_one: duplicate id",
        "combat: a sensitive activity must keep its operator_to_decide question",
        "sensitive activity `point_defense` is missing",
        "ok_one: the table in docs/design/activities.md says `live`, the data says `idea`",
    ] {
        assert!(bad.contains(needle), "the fixture's defect `{needle}` was not reported; the report was:\n{bad}");
    }
    assert!(!bad.contains("ok_one: id must be"), "a good id was refused:\n{bad}");
}

// ── A small JSON reader (std-only) ──────────────────────────────────────────

#[derive(Debug, Clone)]
enum Json {
    Obj(Vec<(String, Json)>),
    Arr(Vec<Json>),
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    fn show(&self) -> String {
        match self {
            Json::Str(s) => format!("{s:?}"),
            Json::Num(n) => n.to_string(),
            Json::Bool(b) => b.to_string(),
            Json::Null => "null".into(),
            Json::Arr(_) => "an array".into(),
            Json::Obj(_) => "an object".into(),
        }
    }
}

fn parse_json(s: &str) -> Result<Json, String> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let v = parse_value(&c, &mut i)?;
    skip_ws(&c, &mut i);
    if i != c.len() {
        return Err(format!("trailing text at {i}"));
    }
    Ok(v)
}

fn skip_ws(c: &[char], i: &mut usize) {
    while *i < c.len() && c[*i].is_whitespace() {
        *i += 1;
    }
}

fn parse_string(c: &[char], i: &mut usize) -> Result<String, String> {
    if c.get(*i) != Some(&'"') {
        return Err(format!("expected a string at {i}"));
    }
    *i += 1;
    let mut s = String::new();
    while *i < c.len() {
        let ch = c[*i];
        if ch == '"' {
            *i += 1;
            return Ok(s);
        }
        if ch == '\\' {
            let e = *c.get(*i + 1).ok_or("bad escape")?;
            *i += 2;
            match e {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'r' => s.push('\r'),
                'b' => s.push('\u{8}'),
                'f' => s.push('\u{c}'),
                'u' => {
                    let hex: String = c.get(*i..*i + 4).ok_or("short \\u escape")?.iter().collect();
                    *i += 4;
                    let cp = u32::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                    s.push(char::from_u32(cp).unwrap_or('?'));
                }
                other => s.push(other),
            }
            continue;
        }
        s.push(ch);
        *i += 1;
    }
    Err("unterminated string".into())
}

fn parse_value(c: &[char], i: &mut usize) -> Result<Json, String> {
    skip_ws(c, i);
    match c.get(*i) {
        Some('{') => {
            *i += 1;
            let mut v = Vec::new();
            skip_ws(c, i);
            if c.get(*i) == Some(&'}') {
                *i += 1;
                return Ok(Json::Obj(v));
            }
            loop {
                skip_ws(c, i);
                let k = parse_string(c, i)?;
                skip_ws(c, i);
                if c.get(*i) != Some(&':') {
                    return Err(format!("expected ':' at {i}"));
                }
                *i += 1;
                let val = parse_value(c, i)?;
                v.push((k, val));
                skip_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some('}') => {
                        *i += 1;
                        return Ok(Json::Obj(v));
                    }
                    _ => return Err(format!("expected ',' or '}}' at {i}")),
                }
            }
        }
        Some('[') => {
            *i += 1;
            let mut v = Vec::new();
            skip_ws(c, i);
            if c.get(*i) == Some(&']') {
                *i += 1;
                return Ok(Json::Arr(v));
            }
            loop {
                v.push(parse_value(c, i)?);
                skip_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some(']') => {
                        *i += 1;
                        return Ok(Json::Arr(v));
                    }
                    _ => return Err(format!("expected ',' or ']' at {i}")),
                }
            }
        }
        Some('"') => parse_string(c, i).map(Json::Str),
        Some('t') if c[*i..].starts_with(&['t', 'r', 'u', 'e']) => {
            *i += 4;
            Ok(Json::Bool(true))
        }
        Some('f') if c[*i..].starts_with(&['f', 'a', 'l', 's', 'e']) => {
            *i += 5;
            Ok(Json::Bool(false))
        }
        Some('n') if c[*i..].starts_with(&['n', 'u', 'l', 'l']) => {
            *i += 4;
            Ok(Json::Null)
        }
        Some(ch) if *ch == '-' || ch.is_ascii_digit() => {
            let start = *i;
            while *i < c.len() && (c[*i].is_ascii_digit() || matches!(c[*i], '-' | '+' | '.' | 'e' | 'E')) {
                *i += 1;
            }
            let t: String = c[start..*i].iter().collect();
            t.parse::<f64>().map(Json::Num).map_err(|e| format!("bad number {t}: {e}"))
        }
        _ => Err(format!("unexpected character at {i}")),
    }
}

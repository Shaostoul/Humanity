//! The how-to checker: what a how-to file must be before it is shown (docs/design/how-to.md;
//! the gates are listed in schemas/how_to.toml).
//!
//! THE SAME RULES TWICE, HELD TOGETHER. The JavaScript twin is `HowToLogic.check` in
//! web/shared/how-to-logic.js, run by scripts/build-how-to.js in `just lints` and `just
//! validate-data`. Both emit the same [`Code`]s, and tests/fixtures/how_to/broken.json is a list of
//! edits to a valid base how-to, one per code, each of which BOTH checkers must refuse with that
//! code (src/how_to/tests.rs here, `node scripts/build-how-to.js --check` there). So every gate is
//! seen red on every run, which is the guard against a check that cannot fail (memory
//! feedback_checks_that_cannot_fail); and the JS script compares its list of codes with this file's
//! [`Code::ALL`], so a gate added to one checker and not the other fails the lint.
//!
//! ORDER. Size, then JSON, then whole numbers, then shape (serde with deny_unknown_fields), then the
//! meaning of the fields, then the review's hash. A file that fails an early stage is not checked
//! further: its later findings would be guesses about a file we could not read.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value as Json;

use super::model::{HowTo, Kind, Level, Status};

/// A refusal is a reason the file is not shown; a warning is printed and the file is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Refuse,
    Warn,
}

/// Every gate, by the code both checkers print. The string forms are the contract with the JS
/// checker and the fixtures: renaming one is renaming it in three places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Code {
    TooBig,
    Json,
    WholeNumbers,
    Shape,
    FileName,
    Folder,
    CategoryUnknown,
    DuplicateId,
    StepIdDuplicate,
    KindFields,
    IfUnknown,
    NoTarget,
    HazardBelowStep,
    SafetyWithoutShort,
    ShortHidesSafety,
    UncitedNumber,
    CiteUnknown,
    SourceUnknown,
    InProjectMissing,
    RoleUnknown,
    ItemUnknown,
    UsesUnlisted,
    RecipeUnknown,
    SkillUnknown,
    TopicUnknown,
    ReadingUnknown,
    TagUnknown,
    NeedsUnknown,
    CallUnknown,
    PageUnknown,
    ActionUnknown,
    ImageWithoutAlt,
    ReviewStatus,
    VerifiedWithoutHash,
    LethalWithoutPerson,
    ReviewHashStale,
    // Warnings.
    Big,
    SeriousWithoutStop,
    LongQuote,
}

impl Code {
    /// Every code, refusals then warnings.
    pub const ALL: &'static [Code] = &[
        Code::TooBig,
        Code::Json,
        Code::WholeNumbers,
        Code::Shape,
        Code::FileName,
        Code::Folder,
        Code::CategoryUnknown,
        Code::DuplicateId,
        Code::StepIdDuplicate,
        Code::KindFields,
        Code::IfUnknown,
        Code::NoTarget,
        Code::HazardBelowStep,
        Code::SafetyWithoutShort,
        Code::ShortHidesSafety,
        Code::UncitedNumber,
        Code::CiteUnknown,
        Code::SourceUnknown,
        Code::InProjectMissing,
        Code::RoleUnknown,
        Code::ItemUnknown,
        Code::UsesUnlisted,
        Code::RecipeUnknown,
        Code::SkillUnknown,
        Code::TopicUnknown,
        Code::ReadingUnknown,
        Code::TagUnknown,
        Code::NeedsUnknown,
        Code::CallUnknown,
        Code::PageUnknown,
        Code::ActionUnknown,
        Code::ImageWithoutAlt,
        Code::ReviewStatus,
        Code::VerifiedWithoutHash,
        Code::LethalWithoutPerson,
        Code::ReviewHashStale,
        Code::Big,
        Code::SeriousWithoutStop,
        Code::LongQuote,
    ];

    /// The code as both checkers print it.
    pub fn id(self) -> &'static str {
        match self {
            Code::TooBig => "too_big",
            Code::Json => "json",
            Code::WholeNumbers => "whole_numbers",
            Code::Shape => "shape",
            Code::FileName => "file_name",
            Code::Folder => "folder",
            Code::CategoryUnknown => "category_unknown",
            Code::DuplicateId => "duplicate_id",
            Code::StepIdDuplicate => "step_id_duplicate",
            Code::KindFields => "kind_fields",
            Code::IfUnknown => "if_unknown",
            Code::NoTarget => "no_target",
            Code::HazardBelowStep => "hazard_below_step",
            Code::SafetyWithoutShort => "safety_without_short",
            Code::ShortHidesSafety => "short_hides_safety",
            Code::UncitedNumber => "uncited_number",
            Code::CiteUnknown => "cite_unknown",
            Code::SourceUnknown => "source_unknown",
            Code::InProjectMissing => "in_project_missing",
            Code::RoleUnknown => "role_unknown",
            Code::ItemUnknown => "item_unknown",
            Code::UsesUnlisted => "uses_unlisted",
            Code::RecipeUnknown => "recipe_unknown",
            Code::SkillUnknown => "skill_unknown",
            Code::TopicUnknown => "topic_unknown",
            Code::ReadingUnknown => "reading_unknown",
            Code::TagUnknown => "tag_unknown",
            Code::NeedsUnknown => "needs_unknown",
            Code::CallUnknown => "call_unknown",
            Code::PageUnknown => "page_unknown",
            Code::ActionUnknown => "action_unknown",
            Code::ImageWithoutAlt => "image_without_alt",
            Code::ReviewStatus => "review_status",
            Code::VerifiedWithoutHash => "verified_without_hash",
            Code::LethalWithoutPerson => "lethal_without_person",
            Code::ReviewHashStale => "review_hash_stale",
            Code::Big => "big",
            Code::SeriousWithoutStop => "serious_without_stop",
            Code::LongQuote => "long_quote",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            Code::Big | Code::SeriousWithoutStop | Code::LongQuote => Severity::Warn,
            _ => Severity::Refuse,
        }
    }

    pub fn from_id(id: &str) -> Option<Code> {
        Code::ALL.iter().copied().find(|c| c.id() == id)
    }
}

/// One thing the checker found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: Code,
    /// In words, naming the field or step.
    pub message: String,
}

impl Finding {
    fn new(code: Code, message: impl Into<String>) -> Self {
        Finding { code, message: message.into() }
    }
}

/// Past this a how-to is refused: it is turning into a Library guide.
pub const MAX_BYTES: usize = 64 * 1024;
/// Past this the checker warns.
pub const WARN_BYTES: usize = 16 * 1024;
/// A quote longer than this is warned about: quote the words that carry the meaning, short.
pub const LONG_QUOTE: usize = 300;

/// What the checker resolves references against. Every set is optional, because the app checks a
/// person's data folder with what it can read, and a set it could not read is not a reason to
/// refuse every how-to; the build and the tests fill them all.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Category id to its top-level ancestor's id.
    pub categories: HashMap<String, String>,
    /// Every written how-to id, with the file that holds it (for duplicates and calls).
    pub written: HashMap<String, String>,
    /// Every planned how-to id.
    pub planned: HashSet<String>,
    pub items: Option<HashSet<String>>,
    pub recipes: Option<HashSet<String>>,
    pub skills: Option<HashSet<String>>,
    pub topics: Option<HashSet<String>>,
    pub reading: Option<HashSet<String>>,
    pub tags: Option<HashSet<String>>,
    /// Source registry ids, each with whether it is a role.
    pub sources: Option<HashMap<String, bool>>,
    /// Jurisdiction ids of data/laws/laws.json (an `if` on `place`).
    pub places: Option<HashSet<String>>,
    /// Page id to the sections a go step may name on it.
    pub pages: Option<HashMap<String, Vec<String>>>,
    /// Admin action ids (data/admin/ops_registry.json has none until increment 6).
    pub actions: HashSet<String>,
    /// The repository root, when the checker runs in it: `in_project` paths are checked there.
    pub repo_root: Option<std::path::PathBuf>,
}

/// Check one how-to file. `file` is its path relative to data/how_to/ with forward slashes
/// ("water/boil_water_safe.json"); `bytes` its contents. Returns everything found, refusals and
/// warnings, and the parsed how-to when it got that far.
pub fn check_file(file: &str, bytes: &[u8], cx: &Context) -> (Vec<Finding>, Option<HowTo>) {
    let mut out = Vec::new();
    if bytes.len() > MAX_BYTES {
        out.push(Finding::new(
            Code::TooBig,
            format!("{} bytes, past the {MAX_BYTES} a how-to may be: the explanation belongs in a Library guide that embeds it", bytes.len()),
        ));
        return (out, None);
    }
    if bytes.len() > WARN_BYTES {
        out.push(Finding::new(Code::Big, format!("{} bytes: this is turning into a Library guide", bytes.len())));
    }
    let json: Json = match serde_json::from_slice(bytes) {
        Ok(j) => j,
        Err(e) => {
            out.push(Finding::new(Code::Json, format!("not JSON: {e}")));
            return (out, None);
        }
    };
    let mut bad_numbers = Vec::new();
    find_bad_numbers(&json, "", &mut bad_numbers);
    if !bad_numbers.is_empty() {
        out.push(Finding::new(
            Code::WholeNumbers,
            format!(
                "every number in a how-to is a whole number of zero or more, so it can be signed (write 500 ml, not 0.5 l): {}",
                bad_numbers.join(", ")
            ),
        ));
        return (out, None);
    }
    let how_to: HowTo = match serde_json::from_value(json.clone()) {
        Ok(h) => h,
        Err(e) => {
            out.push(Finding::new(Code::Shape, format!("does not match schemas/how_to.toml: {e}")));
            return (out, None);
        }
    };
    check_meaning(file, &how_to, cx, &mut out);
    check_review(&json, &how_to, &mut out);
    (out, Some(how_to))
}

/// Only the refusals among `findings`.
pub fn refusals(findings: &[Finding]) -> impl Iterator<Item = &Finding> {
    findings.iter().filter(|f| f.code.severity() == Severity::Refuse)
}

fn find_bad_numbers(v: &Json, at: &str, out: &mut Vec<String>) {
    match v {
        Json::Number(n) => {
            if n.as_u64().is_none() {
                out.push(format!("{at} = {n}"));
            }
        }
        Json::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                find_bad_numbers(x, &format!("{at}/{i}"), out);
            }
        }
        Json::Object(o) => {
            for (k, x) in o {
                find_bad_numbers(x, &format!("{at}/{k}"), out);
            }
        }
        _ => {}
    }
}

fn is_id(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// YYYY-MM-DD with a month 01-12 and a day 01-31.
pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_digit());
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let month: u32 = s[5..7].parse().unwrap_or(0);
    let day: u32 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// A source's own date: YYYY-MM-DD, YYYY-MM, YYYY, or the word `undated`.
fn is_source_date(s: &str) -> bool {
    if s == "undated" || is_date(s) {
        return true;
    }
    let b = s.as_bytes();
    let year = |r: std::ops::Range<usize>| b.len() >= r.end && b[r].iter().all(|c| c.is_ascii_digit());
    match b.len() {
        4 => year(0..4),
        7 => year(0..4) && b[4] == b'-' && year(5..7) && (1..=12).contains(&s[5..7].parse::<u32>().unwrap_or(0)),
        _ => false,
    }
}

fn has_digit(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit())
}

fn check_meaning(file: &str, h: &HowTo, cx: &Context, out: &mut Vec<Finding>) {
    let mut push = |code: Code, msg: String| out.push(Finding::new(code, msg));

    // ── Shape the serde model cannot say ──
    let mut shape = Vec::new();
    if !is_id(&h.id) {
        shape.push(format!("id `{}` is not snake_case", h.id));
    }
    if !(1..=5).contains(&h.difficulty) {
        shape.push(format!("difficulty {} is not 1 to 5", h.difficulty));
    }
    if h.realities.is_empty() {
        shape.push("realities is empty: say real, sim or both".to_string());
    }
    if h.steps.is_empty() {
        shape.push("a how-to has at least one step".to_string());
    }
    for (what, s) in [("title", &h.title), ("summary", &h.summary), ("for", &h.for_whom)] {
        if s.trim().is_empty() {
            shape.push(format!("{what} is empty"));
        }
    }
    if !is_date(&h.updated) {
        shape.push(format!("updated `{}` is not YYYY-MM-DD", h.updated));
    }
    for s in &h.steps {
        if !is_id(&s.id) {
            shape.push(format!("step id `{}` is not snake_case", s.id));
        }
        if s.text.trim().is_empty() {
            shape.push(format!("step `{}` has no text", s.id));
        }
    }
    for s in &h.sources {
        if !is_id(&s.key) {
            shape.push(format!("source key `{}` is not snake_case", s.key));
        }
        if !is_date(&s.read) {
            shape.push(format!("source `{}`: read `{}` is not YYYY-MM-DD", s.key, s.read));
        }
        if !is_source_date(&s.dated) {
            shape.push(format!("source `{}`: dated `{}` is not a date or the word undated", s.key, s.dated));
        }
    }
    for d in [&h.review.checked, &h.review.person].into_iter().flatten() {
        if !is_date(d) {
            shape.push(format!("review date `{d}` is not YYYY-MM-DD"));
        }
    }
    for n in h.tools.iter().chain(&h.materials) {
        if n.item.is_none() && n.proposed_item.is_none() {
            shape.push("a tool or material names neither item nor proposed_item".to_string());
        }
    }
    // Every field the schema types `id` is snake_case, not only the how-to's own id: a reference
    // written another way would never resolve, and the JS checker reads the same rule from
    // schemas/how_to.toml.
    for (what, v) in id_fields(h) {
        if !is_id(&v) {
            shape.push(format!("{what} `{v}` is not snake_case"));
        }
    }
    for m in shape {
        push(Code::Shape, m);
    }

    // ── Where it sits ──
    let file_stem = file.rsplit('/').next().unwrap_or(file);
    if file_stem != format!("{}.json", h.id) {
        push(Code::FileName, format!("the file is `{file_stem}` but the id is `{}`: name it {}.json", h.id, h.id));
    }
    match cx.categories.get(&h.category) {
        None => push(Code::CategoryUnknown, format!("category `{}` is not in data/how_to/categories.json", h.category)),
        Some(top) => {
            let folder = file.split('/').next().unwrap_or("");
            if file.contains('/') && folder != top {
                push(Code::Folder, format!("category `{}` is in the `{top}` branch, so the file belongs in data/how_to/{top}/, not {folder}/", h.category));
            }
        }
    }
    if let Some(other) = cx.written.get(&h.id) {
        if other != file {
            push(Code::DuplicateId, format!("`{}` is also the id of {other}", h.id));
        }
    }
    if cx.planned.contains(&h.id) {
        push(Code::DuplicateId, format!("`{}` is still in data/how_to/planned.json: take it out of the plan now it is written", h.id));
    }

    // ── Steps ──
    let mut seen = HashSet::new();
    for s in &h.steps {
        if !seen.insert(s.id.as_str()) {
            push(Code::StepIdDuplicate, format!("two steps are called `{}`", s.id));
        }
    }
    let step_ids: HashSet<&str> = h.steps.iter().map(|s| s.id.as_str()).collect();
    let problem_ids: HashSet<&str> = h.problems.iter().map(|p| p.id.as_str()).collect();
    // The choose steps seen so far, with their options, for `if`.
    let mut chosen: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for s in &h.steps {
        for m in kind_field_problems(s) {
            push(Code::KindFields, format!("step `{}` ({}): {m}", s.id, s.kind.id()));
        }
        for (key, want) in &s.when {
            if key == "place" {
                if let Some(places) = &cx.places {
                    if !places.contains(want) {
                        push(Code::IfUnknown, format!("step `{}` shows only in place `{want}`, which is not a jurisdiction in data/laws/laws.json", s.id));
                    }
                }
                continue;
            }
            match chosen.get(key.as_str()) {
                None => push(Code::IfUnknown, format!("step `{}` depends on `{key}`, which is not a choose step before it", s.id)),
                Some(opts) if !opts.contains(&want.as_str()) => {
                    push(Code::IfUnknown, format!("step `{}` depends on `{key}` = `{want}`, which is not one of its options", s.id))
                }
                _ => {}
            }
        }
        if s.kind == Kind::Choose {
            chosen.insert(s.id.as_str(), s.options.iter().map(|o| o.id.as_str()).collect());
        }
        if let Some(no) = &s.no {
            if !problem_ids.contains(no.as_str()) && !step_ids.contains(no.as_str()) {
                push(Code::NoTarget, format!("step `{}` sends No to `{no}`, which is neither a problem nor a step", s.id));
            }
        }
        if let Some(who) = &s.who {
            if !h.roles.contains(who) {
                push(Code::RoleUnknown, format!("step `{}` is done by `{who}`, which is not in roles", s.id));
            }
        }
        if let Some(img) = &s.image {
            if img.alt.as_deref().map(|a| a.trim().is_empty()).unwrap_or(true) {
                push(Code::ImageWithoutAlt, format!("step `{}`: the picture {} says nothing for someone who cannot see it (add alt)", s.id, img.file));
            }
        }
        if let Some(how_to) = &s.how_to {
            if s.kind == Kind::Call && !cx.written.contains_key(how_to) {
                push(Code::CallUnknown, format!("step `{}` calls `{how_to}`, which is not a written how-to", s.id));
            }
        }
        if s.kind == Kind::Go {
            if let (Some(pages), Some(page)) = (&cx.pages, &s.page) {
                match pages.get(page) {
                    None => push(Code::PageUnknown, format!("step `{}` opens page `{page}`, which is not a page of the app", s.id)),
                    Some(sections) => {
                        if let Some(section) = &s.section {
                            if !sections.contains(section) {
                                push(Code::PageUnknown, format!("step `{}` opens `{page}` at `{section}`, which is not one of its sections", s.id));
                            }
                        }
                    }
                }
            }
        }
        if let Some(action) = &s.action {
            if !cx.actions.contains(action) {
                push(
                    Code::ActionUnknown,
                    format!("step `{}` runs the admin action `{action}`, which the admin registry does not have (its actions get ids in increment 6)", s.id),
                );
            }
        }
        let listed: HashSet<&str> = h.tools.iter().chain(&h.materials).filter_map(|n| n.item.as_deref()).collect();
        for u in &s.uses {
            if !listed.contains(u.as_str()) {
                push(Code::UsesUnlisted, format!("step `{}` uses `{u}`, which is not in the tools or materials", s.id));
            }
        }
    }

    // ── Safety ──
    let worst = h.steps.iter().filter_map(|s| s.level()).max();
    if let Some(w) = worst {
        if w.hazard() > h.hazard {
            push(Code::HazardBelowStep, format!("the hazard is `{}` but a step's safety note is `{}`: the how-to is at least as dangerous as its worst step", h.hazard.id(), w.hazard().id()));
        }
    }
    for s in &h.steps {
        let Some(safety) = &s.safety else { continue };
        if safety.level.is_grave() {
            if safety.short.as_deref().map(|t| t.trim().is_empty()).unwrap_or(true) {
                push(Code::SafetyWithoutShort, format!("step `{}`: a {} note needs a short line, which both views show", s.id, level_id(safety.level)));
            }
            if s.short == Some(false) {
                push(Code::ShortHidesSafety, format!("step `{}` carries a {} note, so the Short view may not leave it out (short: false)", s.id, level_id(safety.level)));
            }
        }
    }
    let grave = h.hazard >= super::model::Hazard::Serious;
    if grave {
        for s in &h.steps {
            let mut words: Vec<&str> = vec![s.text.as_str()];
            words.extend(s.why.iter().map(String::as_str));
            words.extend(s.detail.iter().map(String::as_str));
            if let Some(safety) = &s.safety {
                words.extend(safety.short.as_deref());
                words.extend(safety.full.iter().map(String::as_str));
            }
            // An option's label or a measurement's label can carry the number itself ("Lower than
            // 1,000 metres").
            words.extend(s.options.iter().map(|o| o.label.as_str()));
            words.extend(s.record.as_ref().map(|r| r.label.as_str()));
            let states_a_number = words.iter().any(|w| has_digit(w)) || s.wait.is_some() || s.record.as_ref().and_then(|r| r.range.as_ref()).is_some();
            if states_a_number && s.cites.is_empty() {
                push(Code::UncitedNumber, format!("step `{}` states a number in a {} how-to and cites no source", s.id, h.hazard.id()));
            }
        }
        for p in &h.problems {
            let words = [Some(p.sign.as_str()), p.why.as_deref(), Some(p.fix.as_str())];
            if words.iter().flatten().any(|w| has_digit(w)) && p.cites.is_empty() {
                push(Code::UncitedNumber, format!("problem `{}` states a number in a {} how-to and cites no source", p.id, h.hazard.id()));
            }
        }
        for (i, c) in h.stop_and_call.iter().enumerate() {
            if (has_digit(&c.when) || has_digit(&c.then)) && c.cites.is_empty() {
                push(Code::UncitedNumber, format!("stop and call entry {} states a number in a {} how-to and cites no source", i + 1, h.hazard.id()));
            }
        }
        if h.stop_and_call.is_empty() {
            push(Code::SeriousWithoutStop, format!("a {} how-to with no 'stop and call someone if' entry", h.hazard.id()));
        }
    }

    // ── Sources ──
    let keys: HashSet<&str> = h.sources.iter().map(|s| s.key.as_str()).collect();
    let mut cited: Vec<(String, &Vec<String>)> = Vec::new();
    for s in &h.steps {
        cited.push((format!("step `{}`", s.id), &s.cites));
    }
    for p in &h.problems {
        cited.push((format!("problem `{}`", p.id), &p.cites));
    }
    for (i, c) in h.stop_and_call.iter().enumerate() {
        cited.push((format!("stop and call entry {}", i + 1), &c.cites));
    }
    for (who, list) in cited {
        for k in list {
            if !keys.contains(k.as_str()) {
                push(Code::CiteUnknown, format!("{who} cites `{k}`, which is not a key in sources"));
            }
        }
    }
    let mut seen_keys = HashSet::new();
    for s in &h.sources {
        if !seen_keys.insert(s.key.as_str()) {
            push(Code::Shape, format!("two sources share the key `{}`", s.key));
        }
        match (&s.source, &s.in_project) {
            (None, None) => push(Code::SourceUnknown, format!("source `{}` names neither a registry source nor an in_project path", s.key)),
            (Some(id), _) => {
                if let Some(reg) = &cx.sources {
                    match reg.get(id) {
                        None => push(Code::SourceUnknown, format!("source `{}` names `{id}`, which is not in data/sources/registry.json", s.key)),
                        Some(true) => push(Code::SourceUnknown, format!("source `{}` names `{id}`, which is a role, not a source anyone can read", s.key)),
                        Some(false) => {}
                    }
                }
            }
            _ => {}
        }
        if let (Some(path), Some(root)) = (&s.in_project, &cx.repo_root) {
            if path.contains("..") || !root.join(path).exists() {
                push(Code::InProjectMissing, format!("source `{}` names {path}, which is not in this repository", s.key));
            }
        }
        if s.quote.as_deref().map(|q| q.chars().count() > LONG_QUOTE).unwrap_or(false) {
            push(Code::LongQuote, format!("source `{}` quotes more than {LONG_QUOTE} characters: quote the words that carry the meaning", s.key));
        }
    }

    // ── Who to call ──
    for (i, c) in h.stop_and_call.iter().enumerate() {
        if let Some(role) = c.who.as_deref().and_then(|w| w.strip_prefix("role:")) {
            if let Some(reg) = &cx.sources {
                if reg.get(role) != Some(&true) {
                    push(Code::RoleUnknown, format!("stop and call entry {} names role `{role}`, which is not a role in data/sources/registry.json", i + 1));
                }
            }
        }
    }

    // ── References into the rest of the app's data ──
    let check_set = |set: &Option<HashSet<String>>, ids: &[String]| -> Vec<String> {
        match set {
            Some(set) => ids.iter().filter(|i| !set.contains(*i)).cloned().collect(),
            None => Vec::new(),
        }
    };
    let mut items: Vec<String> = h.tools.iter().chain(&h.materials).filter_map(|n| n.item.clone()).collect();
    if let Some(help) = &h.help {
        items.extend(help.acts_on.iter().cloned());
    }
    if let Some(m) = &h.makes {
        items.extend(m.inputs.iter().chain(&m.outputs).map(|c| c.item.clone()));
        items.extend(m.station.iter().cloned());
    }
    for i in check_set(&cx.items, &items) {
        push(Code::ItemUnknown, format!("`{i}` is not an item in data/items.csv (add the row, or use proposed_item)"));
    }
    if let Some(m) = &h.makes {
        for r in check_set(&cx.recipes, std::slice::from_ref(&m.recipe)) {
            push(Code::RecipeUnknown, format!("makes names recipe `{r}`, which is not in data/recipes.csv"));
        }
    }
    let mut skills = h.skills.clone();
    if let Some(m) = &h.makes {
        skills.extend(m.skill.iter().cloned());
    }
    for s in check_set(&cx.skills, &skills) {
        push(Code::SkillUnknown, format!("skill `{s}` is not in data/skills/skills.csv"));
    }
    for t in check_set(&cx.topics, &h.topics) {
        push(Code::TopicUnknown, format!("topic `{t}` is not in data/curriculum/syllabus.json"));
    }
    for r in check_set(&cx.reading, &h.reading) {
        push(Code::ReadingUnknown, format!("`{r}` is not a Library document (data/library/index.json)"));
    }
    for t in check_set(&cx.tags, &h.tags) {
        push(Code::TagUnknown, format!("tag `{t}` is not in data/library/tags.json"));
    }
    for n in &h.needs_first {
        if !cx.written.contains_key(n) && !cx.planned.contains(n) {
            push(Code::NeedsUnknown, format!("needs_first names `{n}`, which is neither a written nor a planned how-to"));
        }
    }
}

/// Every value of a field schemas/how_to.toml types `id` (the how-to's own id, the step ids and the
/// source keys are checked with their own messages above), with where it is.
fn id_fields(h: &HowTo) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![("category".into(), h.category.clone())];
    let mut list = |what: &str, vs: &[String]| {
        for v in vs {
            out.push((what.to_string(), v.clone()));
        }
    };
    list("tag", &h.tags);
    list("skill", &h.skills);
    list("topic", &h.topics);
    list("needs_first", &h.needs_first);
    list("role", &h.roles);
    for s in &h.steps {
        list("uses", &s.uses);
        list("cites", &s.cites);
        let ids: Vec<String> = s.options.iter().map(|o| o.id.clone()).chain(s.who.clone()).chain(s.no.clone()).chain(s.how_to.clone()).chain(s.page.clone()).chain(s.section.clone()).chain(s.action.clone()).collect();
        list(&format!("step `{}`", s.id), &ids);
    }
    for p in &h.problems {
        list("problem", std::slice::from_ref(&p.id));
        list("cites", &p.cites);
    }
    for c in &h.stop_and_call {
        list("cites", &c.cites);
    }
    let items: Vec<String> = h.tools.iter().chain(&h.materials).filter_map(|n| n.item.clone()).collect();
    list("item", &items);
    if let Some(help) = &h.help {
        list("help verb", &help.verbs);
        list("item", &help.acts_on);
    }
    if let Some(m) = &h.makes {
        let ids: Vec<String> = std::iter::once(m.recipe.clone()).chain(m.inputs.iter().chain(&m.outputs).map(|c| c.item.clone())).chain(m.station.clone()).chain(m.skill.clone()).collect();
        list("makes", &ids);
    }
    out
}

fn level_id(l: Level) -> &'static str {
    l.hazard().id()
}

/// The fields a step's kind needs and the ones no other kind may carry.
fn kind_field_problems(s: &super::model::Step) -> Vec<String> {
    let mut out = Vec::new();
    let has = |b: bool, name: &str, kinds: &[Kind], out: &mut Vec<String>| {
        if b && !kinds.contains(&s.kind) {
            out.push(format!("`{name}` belongs to a {} step", kinds.iter().map(|k| k.id()).collect::<Vec<_>>().join(" or ")));
        }
    };
    has(s.no.is_some(), "no", &[Kind::Check], &mut out);
    has(s.wait.is_some(), "wait", &[Kind::Wait], &mut out);
    has(s.record.is_some(), "record", &[Kind::Measure], &mut out);
    has(!s.options.is_empty(), "options", &[Kind::Choose], &mut out);
    has(s.how_to.is_some(), "how_to", &[Kind::Call], &mut out);
    has(s.page.is_some(), "page", &[Kind::Go], &mut out);
    has(s.section.is_some(), "section", &[Kind::Go], &mut out);
    has(s.action.is_some(), "action", &[Kind::Action], &mut out);
    has(s.command.is_some(), "command", &[Kind::Command], &mut out);
    has(s.shell.is_some(), "shell", &[Kind::Command], &mut out);
    let need = |b: bool, what: &str, out: &mut Vec<String>| {
        if !b {
            out.push(format!("a {} step needs {what}", s.kind.id()));
        }
    };
    match s.kind {
        Kind::Wait => need(s.wait.as_ref().map(|w| w.max_s.map(|m| m >= w.min_s).unwrap_or(true) && w.min_s > 0).unwrap_or(false), "`wait` with min_s above 0 and max_s, if given, not below it", &mut out),
        Kind::Measure => need(s.record.is_some(), "`record`", &mut out),
        Kind::Choose => {
            let ids: HashSet<&str> = s.options.iter().map(|o| o.id.as_str()).collect();
            need(s.options.len() >= 2 && ids.len() == s.options.len(), "two or more `options` with different ids", &mut out)
        }
        Kind::Call => need(s.how_to.is_some(), "`how_to`", &mut out),
        Kind::Go => need(s.page.is_some(), "`page`", &mut out),
        Kind::Action => need(s.action.is_some(), "`action`", &mut out),
        Kind::Command => need(s.command.as_deref().map(|c| !c.trim().is_empty()).unwrap_or(false), "`command`", &mut out),
        Kind::Do | Kind::Check => {}
    }
    out
}

fn check_review(json: &Json, h: &HowTo, out: &mut Vec<Finding>) {
    let r = &h.review;
    if r.status == Status::Absent {
        out.push(Finding::new(Code::ReviewStatus, "a written how-to cannot be `absent`: that is for planned ones (data/how_to/planned.json)"));
    }
    if r.status >= Status::Sourced && h.sources.is_empty() {
        out.push(Finding::new(Code::ReviewStatus, format!("marked `{}` with no sources", r.status.id())));
    }
    if r.status == Status::Verified {
        if r.hash.is_none() {
            out.push(Finding::new(Code::VerifiedWithoutHash, "verified with no hash: a review binds to the words it checked"));
        }
        if h.hazard == super::model::Hazard::Lethal && r.person.is_none() {
            out.push(Finding::new(
                Code::LethalWithoutPerson,
                "a lethal how-to is not verified until a person has read it and dated it (review.person)",
            ));
        }
    }
    if let Some(want) = &r.hash {
        match super::canonical::hash(json) {
            Ok(now) if &now == want => {}
            Ok(now) => out.push(Finding::new(
                Code::ReviewHashStale,
                format!("the review checked words whose hash was {want}, and the words now hash to {now}: it changed since it was checked"),
            )),
            Err(e) => out.push(Finding::new(Code::ReviewHashStale, format!("the hash cannot be worked out: {e}"))),
        }
    }
}

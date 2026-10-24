//! Every word a person reads in the app, by key, from `data/i18n/<language>.json`
//! (the ELI5 mandate, docs/PRIORITIES.md TIER 3).
//!
//! Operator, 2026-10-22: "let's only focus on English for now but, make sure we
//! prep for all the other languages." So a page asks for a KEY and this module
//! answers with the words in the person's language, falling back to English for
//! any key that language has not translated yet, and to the key itself when even
//! English lacks it (which `tests/i18n_lint.rs` turns into a failing build for any
//! key written in the source). A translation is then DATA: a new
//! `data/i18n/<code>.json` beside the exe, with no code change.
//!
//! The website reads the same files through `web/shared/i18n.js` (its English is
//! `web/shared/i18n-en.js`, generated from `data/i18n/en.json` by
//! `node scripts/gen-i18n.js`), with the same keys, the same `{{name}}`
//! placeholders and the same `[[term]]` glossary marks, so one string serves both.
//!
//! ## The calls
//!
//! ```ignore
//! use crate::gui::i18n::{t, tf, tn};
//! ui.label(t("calculator.title"));                          // "Calculator"
//! ui.label(tf("market.listing_by", &[("name", &seller)]));  // "Listed by {{name}}"
//! ui.label(tn("inventory.items", count, &[]));              // "1 item" / "3 items"
//! ```
//!
//! ## Glossary marks
//!
//! A word a newcomer may not know is marked in the string: `[[relay]]`, or
//! `[[relays|relay]]` when the shown word differs from the glossary entry. `t`
//! returns the words alone ("relays"); `t_marked` keeps the marks for
//! `widgets::glossary_text`, which underlines the word and shows its definition
//! from `data/glossary.json`, the native twin of the website's `data-term`
//! overlay. `tests/i18n_lint.rs` fails on a mark whose term the glossary lacks.
//!
//! ## Plurals
//!
//! `tn(key, n, vars)` reads `key.one` when `n == 1` and `key.other` otherwise,
//! with `{{n}}` set to the number. Languages with more plural forms than two will
//! need more keys; that is a change to this function and to `i18n.tn` on the web,
//! made with the first such translation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// One language: its own words, by flattened key ("nav.home").
struct Lang {
    /// The file's code ("en", "es"): `meta.code`, else the file name.
    code: String,
    /// Its own name for itself ("Español"): `meta.language`.
    name: String,
    /// Words with the glossary marks taken out: what `t` returns.
    plain: HashMap<String, &'static str>,
    /// Words exactly as written, marks kept: what `t_marked` returns.
    marked: HashMap<String, &'static str>,
}

/// Every language this run knows, English always first (index 0, the fallback).
static CATALOG: OnceLock<Vec<Lang>> = OnceLock::new();
/// The language the person chose, as an index into `CATALOG`.
static CURRENT: AtomicUsize = AtomicUsize::new(0);
/// Keys asked for that no language has, kept so `t_dyn` can hand back a
/// `&'static str` without leaking one string per frame.
static MISSING: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

/// The built-in copies, compiled into the exe so a data folder without
/// `i18n/` still has every word (the glossary's lesson, v0.1030).
const BUILT_IN: &[(&str, &str)] = &[
    ("en", crate::embedded_data::I18N_EN_JSON),
    ("es", crate::embedded_data::I18N_ES_JSON),
    ("fr", crate::embedded_data::I18N_FR_JSON),
    ("ja", crate::embedded_data::I18N_JA_JSON),
    ("zh", crate::embedded_data::I18N_ZH_JSON),
];

/// Take the glossary marks out of a string: `[[relays|relay]]` becomes
/// "relays", `[[relay]]` becomes "relay". Text without marks comes back as it is.
pub fn plain(marked: &str) -> String {
    let mut out = String::with_capacity(marked.len());
    for seg in segments(marked) {
        match seg {
            Segment::Text(s) => out.push_str(s),
            Segment::Term { shown, .. } => out.push_str(shown),
        }
    }
    out
}

/// A piece of a marked string: plain words, or a glossary term to underline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment<'a> {
    Text(&'a str),
    /// `shown` is what the reader sees, `term` the glossary entry it opens.
    Term { shown: &'a str, term: &'a str },
}

/// Split a marked string into its plain words and its glossary terms. An
/// unclosed `[[` is plain text, so a stray bracket never hides words.
pub fn segments(marked: &str) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut rest = marked;
    while let Some(open) = rest.find("[[") {
        let Some(close_rel) = rest[open + 2..].find("]]") else { break };
        let inner = &rest[open + 2..open + 2 + close_rel];
        if open > 0 {
            out.push(Segment::Text(&rest[..open]));
        }
        let (shown, term) = match inner.split_once('|') {
            Some((s, t)) => (s, t),
            None => (inner, inner),
        };
        out.push(Segment::Term { shown, term });
        rest = &rest[open + 2 + close_rel + 2..];
    }
    if !rest.is_empty() {
        out.push(Segment::Text(rest));
    }
    out
}

/// Flatten a JSON object into dotted keys: `{"nav":{"home":"Home"}}` gives
/// ("nav.home", "Home"). Non-string leaves (numbers, arrays) are skipped: a
/// language file holds words only.
fn flatten(prefix: &str, v: &serde_json::Value, out: &mut Vec<(String, String)>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, child) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, child, out);
            }
        }
        serde_json::Value::String(s) => out.push((prefix.to_string(), s.clone())),
        _ => {}
    }
}

/// Read one language from its JSON text into flattened pairs. A file that does
/// not parse yields nothing (the caller says so in the log).
fn parse_pairs(json: &str) -> Option<Vec<(String, String)>> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let mut out = Vec::new();
    flatten("", &v, &mut out);
    Some(out)
}

impl Lang {
    fn new(code: &str) -> Self {
        Lang { code: code.to_string(), name: code.to_string(), plain: HashMap::new(), marked: HashMap::new() }
    }

    /// Add (or replace) words. Each string is leaked once, at load: the
    /// catalog lives as long as the process, and `t` can then hand out
    /// `&'static str` that a label borrows for free every frame.
    fn absorb(&mut self, pairs: Vec<(String, String)>) {
        for (k, v) in pairs {
            if k == "meta.language" {
                self.name = v.clone();
            }
            if k == "meta.code" && !v.is_empty() {
                self.code = v.clone();
            }
            let p: &'static str = Box::leak(plain(&v).into_boxed_str());
            let m: &'static str = Box::leak(v.into_boxed_str());
            self.plain.insert(k.clone(), p);
            self.marked.insert(k, m);
        }
    }
}

/// Build the catalog: the built-in copy of each language, then the file on disk
/// over it (so a hot edit or a mod wins, and a disk file from an older install
/// can never take away a word this exe knows), then any language that exists
/// only on disk (a new translation dropped into `data/i18n/`).
fn load() -> Vec<Lang> {
    let dir = crate::data_dir().join("i18n");
    let mut langs: Vec<Lang> = Vec::new();
    for (code, json) in BUILT_IN {
        let mut lang = Lang::new(code);
        match parse_pairs(json) {
            Some(pairs) => lang.absorb(pairs),
            None => log::warn!("i18n: the built-in {code}.json does not parse"),
        }
        // Say so when this language has no usable file on disk: the words shown
        // are then all the copy built into the exe (the rigs look for the line).
        let path = dir.join(format!("{code}.json"));
        match std::fs::read_to_string(&path) {
            Ok(text) if parse_pairs(&text).is_none() => crate::embedded_data::note_builtin_copy(
                &format!("i18n/{code}.json"),
                format_args!("{} does not parse", path.display()),
            ),
            Ok(_) => {}
            Err(e) => crate::embedded_data::note_builtin_copy(
                &format!("i18n/{code}.json"),
                format_args!("{} could not be read ({e})", path.display()),
            ),
        }
        langs.push(lang);
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Some(pairs) = parse_pairs(&text) else {
                log::warn!("i18n: {} does not parse; using the built-in words", path.display());
                continue;
            };
            match langs.iter_mut().find(|l| l.code == stem) {
                Some(lang) => lang.absorb(pairs),
                None => {
                    let mut lang = Lang::new(stem);
                    lang.absorb(pairs);
                    langs.push(lang);
                }
            }
        }
    }
    // English first, always: it is the fallback every lookup ends at.
    if let Some(i) = langs.iter().position(|l| l.code == "en") {
        langs.swap(0, i);
    }
    langs
}

fn catalog() -> &'static [Lang] {
    CATALOG.get_or_init(load)
}

/// Look `key` up in language `idx`, then English. `None` when neither has it.
fn lookup(langs: &[Lang], idx: usize, key: &str, marked: bool) -> Option<&'static str> {
    let pick = |l: &Lang| if marked { l.marked.get(key).copied() } else { l.plain.get(key).copied() };
    langs.get(idx).and_then(pick).or_else(|| langs.first().and_then(pick))
}

/// The words for `key` in the person's language, glossary marks taken out.
/// Falls back to English, then to the key itself (never blank, never a panic:
/// a missing word shows its key, which a person can report).
pub fn t(key: &'static str) -> &'static str {
    lookup(catalog(), CURRENT.load(Ordering::Relaxed), key, false).unwrap_or(key)
}

/// `t` for a key built at run time (`format!("nav.{id}")`). Same answer; a
/// key no language has is kept once rather than leaked every frame.
pub fn t_dyn(key: &str) -> &'static str {
    if let Some(s) = lookup(catalog(), CURRENT.load(Ordering::Relaxed), key, false) {
        return s;
    }
    let missing = MISSING.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = missing.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = map.get(key) {
        return *s;
    }
    let s: &'static str = Box::leak(key.to_string().into_boxed_str());
    map.insert(key.to_string(), s);
    s
}

/// The words for `key` with their `[[term]]` glossary marks kept, for
/// `widgets::glossary_text`.
pub fn t_marked(key: &'static str) -> &'static str {
    lookup(catalog(), CURRENT.load(Ordering::Relaxed), key, true).unwrap_or(key)
}

/// `t_marked` for a key read from data at run time (a tour step's words in
/// `data/onboarding/tour.json`). A key no language has shows as itself, kept
/// once as `t_dyn` keeps it.
pub fn t_marked_dyn(key: &str) -> &'static str {
    match lookup(catalog(), CURRENT.load(Ordering::Relaxed), key, true) {
        Some(s) => s,
        None => t_dyn(key),
    }
}

/// True when English (and so every language, through the fallback) has `key`.
pub fn has(key: &str) -> bool {
    lookup(catalog(), 0, key, false).is_some()
}

/// Put values into `{{name}}` placeholders. A placeholder with no value given
/// stays as written, so a missing value is visible rather than silently empty.
pub fn fill(template: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        let Some(close_rel) = rest[open + 2..].find("}}") else { break };
        let name = &rest[open + 2..open + 2 + close_rel];
        out.push_str(&rest[..open]);
        match vars.iter().find(|(n, _)| *n == name) {
            Some((_, v)) => out.push_str(&v.to_string()),
            None => {
                out.push_str("{{");
                out.push_str(name);
                out.push_str("}}");
            }
        }
        rest = &rest[open + 2 + close_rel + 2..];
    }
    out.push_str(rest);
    out
}

/// `t` with `{{name}}` placeholders filled: `tf("x.by", &[("name", &who)])`.
pub fn tf(key: &'static str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    fill(t(key), vars)
}

/// A count: `key.one` when `n == 1`, `key.other` otherwise, with `{{n}}` set
/// to the number and any other placeholders from `vars`.
pub fn tn(key: &'static str, n: i64, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    let form = if n == 1 { "one" } else { "other" };
    let full = format!("{key}.{form}");
    let mut all: Vec<(&str, &dyn std::fmt::Display)> = vec![("n", &n)];
    all.extend_from_slice(vars);
    fill(t_dyn(&full), &all)
}

/// Choose the language by code ("es"). False (and no change) for a code no
/// file provides. The choice is the person's; `config.rs` keeps it.
pub fn set_language(code: &str) -> bool {
    match catalog().iter().position(|l| l.code == code) {
        Some(i) => {
            CURRENT.store(i, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// The code of the language in use ("en").
pub fn language() -> &'static str {
    let langs = catalog();
    langs.get(CURRENT.load(Ordering::Relaxed)).or_else(|| langs.first()).map(|l| l.code.as_str()).unwrap_or("en")
}

/// Every language this run can show, as (code, its own name), English first:
/// the Settings picker's list. A new `data/i18n/<code>.json` appears here with
/// no code change.
pub fn languages() -> Vec<(&'static str, &'static str)> {
    catalog().iter().map(|l| (l.code.as_str(), l.name.as_str())).collect()
}

/// How much of English a language has translated, as (translated, total), for
/// the Settings picker ("Español, 12 of 340 translated").
pub fn coverage(code: &str) -> (usize, usize) {
    let langs = catalog();
    let total = langs.first().map(|en| en.plain.keys().filter(|k| !k.starts_with("meta.")).count()).unwrap_or(0);
    let done = langs
        .iter()
        .find(|l| l.code == code)
        .map(|l| l.plain.keys().filter(|k| !k.starts_with("meta.") && langs[0].plain.contains_key(*k)).count())
        .unwrap_or(0);
    (done, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A catalog built from strings, so these tests never touch the
    /// process-wide language the rest of the suite reads.
    fn catalog_of(files: &[(&str, &str)]) -> Vec<Lang> {
        files
            .iter()
            .map(|(code, json)| {
                let mut l = Lang::new(code);
                l.absorb(parse_pairs(json).expect("test json parses"));
                l
            })
            .collect()
    }

    /// THE FALLBACK (operator 2026-10-22: English now, every language ready). A
    /// language that has not translated a key shows the English words, never the
    /// key and never a blank; a key no language has shows the key.
    #[test]
    fn a_missing_word_falls_back_to_english_then_to_the_key() {
        let langs = catalog_of(&[
            ("en", r#"{"nav":{"home":"Home","files":"Files"},"only":{"english":"Only in English"}}"#),
            ("es", r#"{"nav":{"home":"Inicio"}}"#),
        ]);
        assert_eq!(lookup(&langs, 1, "nav.home", false), Some("Inicio"), "a translated word is the translation");
        assert_eq!(lookup(&langs, 1, "only.english", false), Some("Only in English"), "an untranslated word is English");
        assert_eq!(lookup(&langs, 1, "nav.files", false), Some("Files"));
        assert_eq!(lookup(&langs, 1, "no.such.key", false), None, "no language has it: the caller shows the key");
        assert_eq!(lookup(&langs, 7, "nav.home", false), Some("Home"), "an index past the end reads English");
    }

    /// The SHIPPED files: every language the exe carries falls back to English
    /// for each key it lacks, so a person who picks Español sees every page whole.
    #[test]
    fn every_shipped_language_shows_every_english_key() {
        let langs = catalog_of(BUILT_IN);
        let en_keys: Vec<&String> = langs[0].plain.keys().collect();
        assert!(en_keys.len() > 40, "English has {} keys", en_keys.len());
        for (i, lang) in langs.iter().enumerate() {
            for k in &en_keys {
                let got = lookup(&langs, i, k, false);
                assert!(got.is_some_and(|s| !s.is_empty()), "{}: key {k} shows nothing", lang.code);
            }
        }
        // And at least one shipped translation really is used (not English everywhere).
        let es = langs.iter().position(|l| l.code == "es").expect("es ships");
        assert_eq!(lookup(&langs, es, "nav.home", false), Some("Inicio"));
    }

    #[test]
    fn marks_are_taken_out_for_plain_and_kept_for_marked() {
        let langs = catalog_of(&[("en", r#"{"a":{"b":"Your words go to a [[relay]], then to the [[peers|peer]]."}}"#)]);
        assert_eq!(lookup(&langs, 0, "a.b", false), Some("Your words go to a relay, then to the peers."));
        assert_eq!(lookup(&langs, 0, "a.b", true), Some("Your words go to a [[relay]], then to the [[peers|peer]]."));
        assert_eq!(
            segments("a [[relays|relay]] b"),
            vec![Segment::Text("a "), Segment::Term { shown: "relays", term: "relay" }, Segment::Text(" b")]
        );
        assert_eq!(plain("an [[unclosed mark"), "an [[unclosed mark", "an unclosed mark stays as written");
    }

    #[test]
    fn placeholders_fill_and_a_missing_value_stays_visible() {
        let n = 3;
        assert_eq!(fill("{{n}} items for {{who}}", &[("n", &n), ("who", &"Ann")]), "3 items for Ann");
        assert_eq!(fill("Hello {{who}}", &[]), "Hello {{who}}");
        assert_eq!(fill("no placeholders", &[("x", &1)]), "no placeholders");
    }

    /// The real `t` reads the shipped English (this test does not change the
    /// language, so it is safe beside the others).
    #[test]
    fn t_reads_the_shipped_english() {
        assert_eq!(t("nav.home"), "Home");
        assert_eq!(t("no.such.key.anywhere"), "no.such.key.anywhere");
        assert!(languages().first().is_some_and(|(c, _)| *c == "en"), "English is listed first");
        assert!(languages().iter().any(|(c, _)| *c == "es"));
    }
}

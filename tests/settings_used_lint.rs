//! Settings-used lint (BUG-235, 2026-11-03): a control on the Settings page must CHANGE
//! something.
//!
//! `tests/settings_persistence_lint.rs` proves a setting bound to a control is saved and loaded.
//! That is not enough. Three settings were bound, saved and loaded faithfully, and read by
//! nothing else: Font Size (BUG-225, fixed 2026-11-03), Dark Mode (BUG-234) and "Profile Visible
//! to Others" (BUG-235, removed 2026-11-03). Moving such a control changes no pixel and no
//! message; worse, the persistence test held "a private profile must STAY private" green while
//! no profile was ever private. This lint finds that shape: a setting the Settings page binds
//! that no code other than its own control, its save and its load ever reads.
//!
//! HOW IT WORKS (std-only text scanning, like the other lints; it never links the native bin).
//! 1. THE SETTINGS PAGE'S FILES are found, not listed: `src/gui/pages/settings.rs`, and every
//!    file it (and each file found so far) hands a section to with a call whose arguments start
//!    `ui, theme, state` (`crate::gui::pages::privacy::draw_privacy_content(ui, theme, state)`,
//!    `calls::draw_calls(ui, theme, state)`, `super::settings_time::draw(ui, theme, state, hint)`).
//!    So a new section in a new file is covered the day it is drawn.
//! 2. A setting is BOUND when one of those files binds it to a control (`&mut <..>.settings.X`)
//!    or writes it (`<..>.settings.X = ..`, `+=` and the like) outside test code, and it is
//!    SAVED (a field on `AppConfig`). The page's own scratch state (which section is open, a
//!    one-frame scroll request, a typed recovery phrase) is not a setting: its page is its only
//!    reader by design (settings_persistence_lint's INTENTIONALLY_TRANSIENT). Today 88 settings
//!    are bound, from 13 files.
//! 3. A READ is any other mention of the value in `src/` outside test code: `<..>.settings.X`
//!    (a path split over lines included: the scan works on tokens, not lines), or `s.X` through
//!    a local that holds the settings (`let s = &state.gui_state.settings;`, a `&SettingsState`
//!    parameter). Comments and strings never count (`t("settings.font_size")` is a key, not a
//!    read). NOT reads:
//!    - the control itself: a `&mut` binding in a Settings page file, the control's working copy
//!      there (`let mut v = state.settings.X;` bound to the widget, then written back), and any
//!      mention inside the same function as one of the setting's own bindings or writes in that
//!      file (the control showing its own value);
//!    - a `&mut <..>.settings.X` handed to a widget anywhere else (a second control is not a
//!      use): the innermost call is one of `WIDGET_CALLS`;
//!    - writes (an assignment is not a use);
//!    - the save leg and the load leg (`AppConfig::from_gui_state`, `apply_to_gui_state` in
//!      `src/config.rs`);
//!    - test code: `#[cfg(test)]` and `#[test]` items, and whole files that are test modules
//!      (`#[cfg(test)] mod x;`, with or without `#[path = ".."]`, and every file under one).
//!    Everything else counts, a method call on the value included, so the lint never reports a
//!    setting something does read. Its blind spot is the other way: a setting whose only reader
//!    is code that shows it and does nothing with it passes. It catches the setting nothing
//!    reads at all, which is what all three were.
//! 4. A setting bound and never read FAILS, unless it is on one of two lists:
//!    - `KNOWN_OFFENDERS`, each with its BUG number: a list meant to SHRINK (the house pattern,
//!      like theme_token_lint's LEGACY_OFFENDERS). Fixing the bug (making the setting do what its
//!      words say, or taking the control out) makes the lint fail until its line is deleted. A
//!      line is never added to turn red green: a new unread setting is a bug to fix or log.
//!    - `APPLIED_WHERE_SET`: a setting whose control and load apply it on the spot through a
//!      call the scan cannot follow (the Language picker calls `i18n::set_language` with the
//!      code it shows, and the load does with the saved one), so its field is only the
//!      remembered choice. Each entry names the line that applies it, in its file: when that
//!      line goes, the excuse goes, and the lint fails.
//!
//! WHAT IT REPORTS ON TODAY'S TREE (2026-11-03): Dark Mode (BUG-234, in KNOWN_OFFENDERS), and
//! nothing else, among the 88 settings the Settings page binds. The rough scan BUG-235 records
//! listed eight more that are in fact read, through a local holding the settings (the four
//! frame-rate caps, through lib.rs's `let s = &state.gui_state.settings;`) or on a line of
//! their own; this scan finds each reader. `SETTINGS_USED_REPORT=1` prints every bound setting
//! with its first reader.
//!
//! SEEN RED, both on 2026-11-03:
//! - With BUG-235's field restored (src/gui/mod.rs, src/config.rs and src/gui/pages/settings.rs
//!   as they were before its removal: the Privacy card's "Profile Visible to Others" toggle on
//!   `state.settings.profile_visible`, the field, its save and its load),
//!   `every_setting_the_settings_page_binds_is_read` failed:
//!   "profile_visible: bound in src/gui/pages/settings.rs, read by nothing but its control, its
//!   save and its load".
//! - With Font Size's reader removed (the call in `src/lib.rs` that hands
//!   `&mut state.gui_state.settings.font_size` to `gui::font_size::before_frame`, which sets the
//!   app's zoom before every pass, replaced by `false`), it failed:
//!   "font_size: bound in src/gui/pages/settings.rs, read by nothing but its control, its save
//!   and its load".
//!
//! THE PRIVACY LEVELS' SWITCHES (BUG-238, 2026-11-03). A privacy level
//! (`data/gui/privacy_tiers.json`) is not a Settings-page control, so the scan above never saw the
//! Spotlight level's `promote_streaming`: in the file, on the desktop's `PrivacyTier`, read by
//! nothing in either app, behind the words "your live-stream status shown so viewers can find
//! you". `every_privacy_level_switch_is_read_by_both_apps` holds every true/false field a level
//! carries to a `.field` read in BOTH apps where levels are used: a src/ file naming
//! `PrivacyTier` (outside test code; the field must be on the struct too, or serde drops it) and
//! a web/ script naming `privacy_tiers.json` (outside comments and strings). The section above
//! that test says exactly what counts.
//! SEEN RED, 2026-11-03, with `promote_streaming` restored (the Spotlight level back in the file
//! with `"promote_streaming": true`, and `pub promote_streaming: bool` back on `PrivacyTier`
//! with `promote_streaming: false` in its fallback): it failed "promote_streaming: read by
//! nothing in the desktop app (no `.field` in a src/ file that names PrivacyTier); read by nothing
//! on the website (no `.field` in a web/ script that loads privacy_tiers.json)". With the field in
//! the file alone: "not a field of PrivacyTier in src/gui/pages/privacy.rs, so the desktop app
//! drops it as it loads the file".
//!
//! Std-only (no crate imports), about a second: `just lints` runs it, and CI. By hand:
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/settings_used_lint.rs

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Settings the Settings page binds that nothing reads today, each with its bug. MEANT TO
/// SHRINK: never add a line to make the lint pass.
const KNOWN_OFFENDERS: &[(&str, &str)] = &[(
    "dark_mode",
    "BUG-234: Settings > Appearance > Dark Mode; Theme::apply_to_egui always starts from Visuals::dark()",
)];

/// Settings applied by their control and their load through a call the scan cannot follow, so
/// the field is only the remembered choice: (setting, file, the line that applies it). The
/// lint fails if the line is not in the file.
const APPLIED_WHERE_SET: &[(&str, &str, &str)] = &[
    // Settings > Appearance > Language: the picker applies the code it shows, and the load
    // applies the saved one (a code no file provides falls back to English).
    ("language", "src/gui/pages/settings.rs", "i18n::set_language(code)"),
    ("language", "src/config.rs", "humanity_core::i18n::set_language(&self.language)"),
];

/// Widgets that take a setting by `&mut` to show and change it: a binding, not a read, wherever
/// it is drawn. Matched as the name of the innermost call around the `&mut` (`Slider::new` and
/// `DragValue::new` as their type's name before `::new`).
const WIDGET_CALLS: &[&str] = &[
    "toggle",
    "labeled_slider",
    "labeled_slider_entry",
    "checkbox",
    "custom_checkbox",
    "radio_value",
    "selectable_value",
    "singleline",
    "multiline",
    "Slider",
    "DragValue",
];

/// Where the scan starts: the Settings page itself.
const SETTINGS_ROOT: &str = "src/gui/pages/settings.rs";

/// The save leg and the load leg, in src/config.rs.
const SAVE_AND_LOAD: &[&str] = &["from_gui_state", "apply_to_gui_state"];

// ── A Rust tokenizer, just enough to see identifiers, punctuation and nesting ──────────────────

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str,
    Punct(char),
    Other,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    line: usize,
}

fn tokenize(src: &str) -> Vec<Token> {
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let is_id_start = |c: char| c == '_' || c.is_alphabetic();
    let is_id = |c: char| c == '_' || c.is_alphanumeric();
    while i < n {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < n && b[i + 1] == '/' {
            while i < n && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < n && b[i + 1] == '*' {
            let mut depth = 1;
            i += 2;
            while i < n && depth > 0 {
                if b[i] == '\n' {
                    line += 1;
                }
                if b[i] == '/' && i + 1 < n && b[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && i + 1 < n && b[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw strings r"..", r#".."#, br"..", cr"..".
        let raw = if (c == 'r' || ((c == 'b' || c == 'c') && i + 1 < n && b[i + 1] == 'r')) && !(i > 0 && is_id(b[i - 1])) {
            let mut j = if c == 'r' { i + 1 } else { i + 2 };
            let mut hashes = 0;
            while j < n && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && b[j] == '"' { Some((j + 1, hashes)) } else { None }
        } else {
            None
        };
        if let Some((start, hashes)) = raw {
            let start_line = line;
            let mut j = start;
            while j < n {
                if b[j] == '"' {
                    let mut k = 0;
                    while k < hashes && j + 1 + k < n && b[j + 1 + k] == '#' {
                        k += 1;
                    }
                    if k == hashes {
                        j += 1 + hashes;
                        break;
                    }
                }
                if b[j] == '\n' {
                    line += 1;
                }
                j += 1;
            }
            out.push(Token { tok: Tok::Str, line: start_line });
            i = j;
            continue;
        }
        let prefixed = (c == 'b' || c == 'c') && i + 1 < n && b[i + 1] == '"' && !(i > 0 && is_id(b[i - 1]));
        if c == '"' || prefixed {
            let start_line = line;
            let mut j = if prefixed { i + 2 } else { i + 1 };
            while j < n && b[j] != '"' {
                if b[j] == '\\' && j + 1 < n {
                    if b[j + 1] == '\n' {
                        line += 1;
                    }
                    j += 2;
                    continue;
                }
                if b[j] == '\n' {
                    line += 1;
                }
                j += 1;
            }
            out.push(Token { tok: Tok::Str, line: start_line });
            i = j + 1;
            continue;
        }
        if c == '\'' {
            // A char literal ('a', '\n', '\u{..}') or a lifetime / label ('a, 'static).
            if i + 1 < n && b[i + 1] == '\\' {
                let mut j = i + 2;
                while j < n && b[j] != '\'' {
                    j += 1;
                }
                out.push(Token { tok: Tok::Other, line });
                i = j + 1;
                continue;
            }
            if i + 2 < n && b[i + 2] == '\'' {
                out.push(Token { tok: Tok::Other, line });
                i += 3;
                continue;
            }
            let mut j = i + 1;
            while j < n && is_id(b[j]) {
                j += 1;
            }
            out.push(Token { tok: Tok::Other, line });
            i = j;
            continue;
        }
        if is_id_start(c) {
            let mut j = i;
            let mut s = String::new();
            while j < n && is_id(b[j]) {
                s.push(b[j]);
                j += 1;
            }
            if s == "r" && j + 1 < n && b[j] == '#' && is_id_start(b[j + 1]) {
                s.clear();
                j += 1;
                while j < n && is_id(b[j]) {
                    s.push(b[j]);
                    j += 1;
                }
            }
            out.push(Token { tok: Tok::Ident(s), line });
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < n && (b[j].is_alphanumeric() || b[j] == '_' || (b[j] == '.' && j + 1 < n && b[j + 1].is_ascii_digit())) {
                j += 1;
            }
            out.push(Token { tok: Tok::Other, line });
            i = j;
            continue;
        }
        out.push(Token { tok: Tok::Punct(c), line });
        i += 1;
    }
    out
}

fn ident(t: &Token) -> Option<&str> {
    match &t.tok {
        Tok::Ident(s) => Some(s),
        _ => None,
    }
}

fn is_p(toks: &[Token], i: usize, c: char) -> bool {
    toks.get(i).map(|t| t.tok == Tok::Punct(c)).unwrap_or(false)
}

fn is_id(toks: &[Token], i: usize, w: &str) -> bool {
    toks.get(i).and_then(ident) == Some(w)
}

/// The words of the attribute that starts at `i` (`#` `[`), and the index just past it.
fn attribute_at(toks: &[Token], i: usize) -> Option<(Vec<String>, usize)> {
    if !(is_p(toks, i, '#') && is_p(toks, i + 1, '[')) {
        return None;
    }
    let mut j = i + 2;
    let mut depth = 1;
    let mut words = Vec::new();
    while j < toks.len() && depth > 0 {
        match &toks[j].tok {
            Tok::Punct('[') => depth += 1,
            Tok::Punct(']') => depth -= 1,
            Tok::Ident(w) => words.push(w.clone()),
            _ => {}
        }
        j += 1;
    }
    Some((words, j))
}

fn is_test_attribute(words: &[String]) -> bool {
    (words.first().map(|w| w == "cfg").unwrap_or(false) && words.iter().any(|w| w == "test") && !words.iter().any(|w| w == "not"))
        || words == ["test"]
}

/// Mark the tokens of test-only items: `#[cfg(test)]` (or `cfg(all(test, ..))`) and `#[test]`,
/// through the end of the item's `{ .. }` block. An attribute on an item that ends in `;` before
/// any `{` (`#[cfg(test)] mod tests;`) masks nothing here: that module's FILE is test code,
/// which `test_module_files` finds.
fn test_mask(toks: &[Token]) -> Vec<bool> {
    let mut mask = vec![false; toks.len()];
    let mut i = 0;
    while i < toks.len() {
        if let Some((words, j)) = attribute_at(toks, i) {
            if is_test_attribute(&words) {
                let mut k = j;
                while k < toks.len() && !is_p(toks, k, '{') && !is_p(toks, k, ';') {
                    k += 1;
                }
                if is_p(toks, k, '{') {
                    let mut d = 0;
                    let mut m = k;
                    while m < toks.len() {
                        match toks[m].tok {
                            Tok::Punct('{') => d += 1,
                            Tok::Punct('}') => {
                                d -= 1;
                                if d == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        m += 1;
                    }
                    for x in mask.iter_mut().take(m.min(toks.len() - 1) + 1).skip(i) {
                        *x = true;
                    }
                    i = m + 1;
                    continue;
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    mask
}

// ── Files and modules ───────────────────────────────────────────────────────────────────────────

struct SrcFile {
    rel: String,
    toks: Vec<Token>,
    test: Vec<bool>,
    /// The innermost function each token is in (an index into `fn_names`), if any.
    fn_of: Vec<Option<usize>>,
    fn_names: Vec<String>,
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

fn rel_of(p: &Path) -> String {
    p.strip_prefix(repo()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// Which function each token sits in: `fn NAME` opens a function at its body's `{` (a `;` first
/// means a declaration with no body), and the matching `}` closes it. Closures stay in theirs.
fn functions(toks: &[Token]) -> (Vec<Option<usize>>, Vec<String>) {
    let mut fn_of = vec![None; toks.len()];
    let mut names: Vec<String> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (fn index, depth of its body)
    // The function whose signature is being read, and the bracket depth it started at.
    let mut pending: Option<(usize, usize)> = None;
    let mut depth = 0usize; // braces
    let mut brackets = 0usize; // parentheses and square brackets
    for i in 0..toks.len() {
        match toks[i].tok {
            Tok::Ident(ref w) if w == "fn" => {
                if let Some(name) = toks.get(i + 1).and_then(ident) {
                    names.push(name.to_string());
                    pending = Some((names.len() - 1, brackets));
                }
            }
            Tok::Punct('(') | Tok::Punct('[') => brackets += 1,
            Tok::Punct(')') | Tok::Punct(']') => brackets = brackets.saturating_sub(1),
            // `fn f();` in a trait: no body. A `;` inside the signature's brackets
            // (`[u8; 4]`) is not the end.
            Tok::Punct(';') if pending.map(|p| p.1 == brackets).unwrap_or(false) => pending = None,
            Tok::Punct('{') => {
                depth += 1;
                if let Some((f, _)) = pending.take() {
                    stack.push((f, depth));
                }
            }
            Tok::Punct('}') => {
                if stack.last().map(|s| s.1) == Some(depth) {
                    stack.pop();
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        fn_of[i] = stack.last().map(|s| s.0);
    }
    (fn_of, names)
}

fn parse(rel: &str, text: &str) -> SrcFile {
    let toks = tokenize(text);
    let test = test_mask(&toks);
    let (fn_of, fn_names) = functions(&toks);
    SrcFile { rel: rel.to_string(), toks, test, fn_of, fn_names }
}

/// A file's module path from `src/` (`src/gui/pages/settings.rs` -> gui::pages::settings,
/// `src/gui/mod.rs` -> gui, `src/lib.rs` -> the crate root).
fn module_of(rel: &str) -> Vec<String> {
    let p = rel.trim_start_matches("src/").trim_end_matches(".rs");
    let mut parts: Vec<String> = p.split('/').map(str::to_string).collect();
    if parts.last().map(|s| s == "mod" || s == "lib" || s == "main").unwrap_or(false) {
        parts.pop();
    }
    parts
}

/// The folder a file's child modules live in (`settings.rs` -> `settings/`, `mod.rs` -> its own).
fn child_dir(rel: &str) -> String {
    let p = rel.trim_end_matches(".rs");
    if p.ends_with("/mod") || p.ends_with("/lib") || p.ends_with("/main") {
        p.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default()
    } else {
        p.to_string()
    }
}

/// The file of a module path, if one exists.
fn file_of_module(module: &[String]) -> Option<String> {
    let base = if module.is_empty() { "src/lib".to_string() } else { format!("src/{}", module.join("/")) };
    for cand in [format!("{base}.rs"), format!("{base}/mod.rs")] {
        if repo().join(&cand).is_file() {
            return Some(cand);
        }
    }
    None
}

/// The modules a file declares with `mod NAME;` (no body), resolved to files, each with whether
/// its attributes make it test code. Read line by line, not tokenized: the attributes sit on the
/// lines above (`#[cfg(test)]`, `#[path = "x_tests.rs"]`) or before it on its own line, and
/// tokenizing all of src/ just for this was most of the scan's time.
fn declared_modules(rel: &str, text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut is_test = false;
    let mut path_attr: Option<String> = None;
    for line in text.lines() {
        let mut l = line.trim();
        // Attributes at the start of the line, each read up to its balanced `]`.
        while l.starts_with("#[") {
            let mut d = 0i32;
            let mut end = None;
            for (k, ch) in l.char_indices() {
                match ch {
                    '[' => d += 1,
                    ']' => {
                        d -= 1;
                        if d == 0 {
                            end = Some(k);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(end) = end else { break };
            let attr = &l[2..end];
            let words: Vec<String> = attr
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|w| !w.is_empty())
                .map(str::to_string)
                .collect();
            if is_test_attribute(&words) {
                is_test = true;
            }
            if words.first().map(|w| w == "path").unwrap_or(false) {
                if let (Some(a), Some(b)) = (attr.find('"'), attr.rfind('"')) {
                    if b > a {
                        path_attr = Some(attr[a + 1..b].to_string());
                    }
                }
            }
            l = l[end + 1..].trim_start();
        }
        if l.is_empty() || l.starts_with("//") {
            continue; // an attribute-only line, a blank or a comment keeps what was gathered
        }
        let decl = ["pub(crate) ", "pub(super) ", "pub(in crate) ", "pub "]
            .iter()
            .find_map(|p| l.strip_prefix(p))
            .unwrap_or(l);
        if let Some(name) = decl.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')).map(str::trim) {
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                let file = match &path_attr {
                    // `#[path]` on a module declared in a non-mod.rs file is relative to that
                    // file's own folder.
                    Some(p) => {
                        let folder = rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
                        format!("{folder}/{p}")
                    }
                    None => {
                        let dir = child_dir(rel);
                        let a = format!("{dir}/{name}.rs");
                        if repo().join(&a).is_file() { a } else { format!("{dir}/{name}/mod.rs") }
                    }
                };
                out.push((file, is_test));
            }
        }
        is_test = false;
        path_attr = None;
    }
    out
}

/// Every file that is test code by declaration: a `#[cfg(test)] mod x;` names one, and every
/// module below it is test code too. Plus the `_tests.rs` naming convention, as a backstop.
fn test_module_files(texts: &BTreeMap<String, String>) -> HashSet<String> {
    let mut test_files: HashSet<String> = HashSet::new();
    for (rel, text) in texts {
        for (file, is_test) in declared_modules(rel, text) {
            if is_test {
                test_files.insert(file);
            }
        }
    }
    // Below a test module: its child folder's files.
    let roots: Vec<String> = test_files.iter().cloned().collect();
    for r in roots {
        let dir = format!("{}/", child_dir(&r));
        for name in texts.keys() {
            if name.starts_with(&dir) {
                test_files.insert(name.clone());
            }
        }
    }
    for name in texts.keys() {
        if name.ends_with("_tests.rs") || name.ends_with("/tests.rs") {
            test_files.insert(name.clone());
        }
    }
    test_files
}

/// Every source file under src/, by its path from the repo, line endings normalised.
fn all_texts() -> BTreeMap<String, String> {
    let mut paths = Vec::new();
    walk(&repo().join("src"), &mut paths);
    paths
        .iter()
        .map(|p| (rel_of(p), fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())).replace("\r\n", "\n")))
        .collect()
}

// ── The Settings page's files ───────────────────────────────────────────────────────────────────

/// Calls in `f` whose arguments start `ui, theme, state`, as their paths (`["crate", "gui",
/// "pages", "privacy", "draw_privacy_content"]`), outside test code.
fn section_calls(f: &SrcFile) -> Vec<(Vec<String>, usize)> {
    let toks = &f.toks;
    let mut out = Vec::new();
    for i in 0..toks.len() {
        if f.test[i] || !is_p(toks, i, '(') {
            continue;
        }
        if !(is_id(toks, i + 1, "ui") && is_p(toks, i + 2, ',') && is_id(toks, i + 3, "theme") && is_p(toks, i + 4, ',') && is_id(toks, i + 5, "state")) {
            continue;
        }
        if !(is_p(toks, i + 6, ',') || is_p(toks, i + 6, ')')) {
            continue;
        }
        // The path before the `(`: IDENT (:: IDENT)*.
        let mut path = Vec::new();
        let mut k = i;
        while k >= 1 {
            let Some(w) = toks.get(k - 1).and_then(ident) else { break };
            path.push(w.to_string());
            if k >= 3 && is_p(toks, k - 2, ':') && is_p(toks, k - 3, ':') {
                k -= 3;
            } else {
                break;
            }
        }
        path.reverse();
        if path.len() >= 2 {
            out.push((path, toks[i].line));
        }
    }
    out
}

/// The file a section call's path names, or None for a path that is not rooted in this crate's
/// module tree (`widgets::card` through a `use`).
fn resolve_call(f: &SrcFile, path: &[String], declared: &HashSet<String>) -> Option<String> {
    let here = module_of(&f.rel);
    let (base, rest): (Vec<String>, &[String]) = match path[0].as_str() {
        "crate" => (Vec::new(), &path[1..]),
        "self" => (here.clone(), &path[1..]),
        "super" => {
            let mut b = here.clone();
            let mut r = path;
            while r.first().map(|s| s == "super").unwrap_or(false) {
                b.pop();
                r = &r[1..];
            }
            (b, r)
        }
        first if declared.contains(first) => (here.clone(), path),
        _ => return None,
    };
    // The longest module prefix that is a file: all but the function, then all but a type too.
    for drop in 1..rest.len() {
        let mut m = base.clone();
        m.extend(rest[..rest.len() - drop].iter().cloned());
        if let Some(file) = file_of_module(&m) {
            return Some(file);
        }
    }
    None
}

/// The names `f` declares as child modules (`mod calls;`), for resolving `calls::draw_calls`.
fn child_module_names(f: &SrcFile) -> HashSet<String> {
    let toks = &f.toks;
    let mut out = HashSet::new();
    for i in 0..toks.len() {
        if is_id(toks, i, "mod") && is_p(toks, i + 2, ';') {
            if let Some(n) = toks.get(i + 1).and_then(ident) {
                out.insert(n.to_string());
            }
        }
    }
    out
}

/// The Settings page's files: the root, and every file reached from it by section calls.
fn settings_page_files(texts: &BTreeMap<String, String>, parsed: &mut HashMap<String, SrcFile>) -> BTreeSet<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut todo = vec![SETTINGS_ROOT.to_string()];
    while let Some(rel) = todo.pop() {
        if !found.insert(rel.clone()) {
            continue;
        }
        let text = texts.get(&rel).unwrap_or_else(|| panic!("{rel} is not under src/"));
        let f = parsed.entry(rel.clone()).or_insert_with(|| parse(&rel, text));
        let declared = child_module_names(f);
        for (path, line) in section_calls(f) {
            match resolve_call(f, &path, &declared) {
                Some(file) => todo.push(file),
                None if matches!(path[0].as_str(), "crate" | "self" | "super") => {
                    panic!("{rel}:{line}: the section call `{}` names no file this lint can find", path.join("::"))
                }
                None => {}
            }
        }
    }
    found
}

// ── Mentions of a setting ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// `&mut ..settings.X` in a Settings page file, or handed to a widget anywhere.
    Binding,
    /// An assignment to it (or to something inside it).
    Write,
    /// The control's working copy: `let mut v = ..settings.X;` in a Settings page file.
    Copy,
    Read,
}

#[derive(Debug, Clone)]
struct Mention {
    setting: String,
    file: String,
    line: usize,
    func: Option<usize>,
    kind: Kind,
}

/// The tokens that name a local holding the settings, at a use followed by `.`:
/// `let [mut] s = [&[mut]] <..>.settings;` (also `.clone()`), and a parameter
/// `s: &[mut] [path::]SettingsState`, alive to the end of their block. A later `let s` of
/// something else hides it again. (Only uses are looked up, and only names that were ever bound
/// to the settings: asking at every token made the scan of src/ take 17 s instead of under 2.)
fn settings_aliases(toks: &[Token]) -> HashSet<usize> {
    let mut at: HashSet<usize> = HashSet::new();
    let mut ever: HashSet<String> = HashSet::new();
    let mut live: Vec<(String, usize, bool)> = Vec::new(); // (name, depth, is settings)
    let mut depth = 0usize;
    for i in 0..toks.len() {
        match toks[i].tok {
            Tok::Punct('{') => depth += 1,
            Tok::Punct('}') => {
                live.retain(|(_, d, _)| *d < depth);
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        if is_id(toks, i, "let") {
            let mut k = i + 1;
            if is_id(toks, k, "mut") {
                k += 1;
            }
            if let (Some(name), true) = (toks.get(k).and_then(ident), is_p(toks, k + 1, '=')) {
                // The right-hand side up to its `;`.
                let mut j = k + 2;
                let mut d = 0i32;
                let mut rhs: Vec<&Tok> = Vec::new();
                while j < toks.len() {
                    match toks[j].tok {
                        Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => d += 1,
                        Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => d -= 1,
                        Tok::Punct(';') if d <= 0 => break,
                        _ => {}
                    }
                    rhs.push(&toks[j].tok);
                    j += 1;
                }
                let words: Vec<&Tok> = rhs.iter().copied().filter(|t| !matches!(t, Tok::Punct('&')) && !matches!(t, Tok::Ident(w) if w == "mut")).collect();
                let ends_settings = |w: &[&Tok]| w.last() == Some(&&Tok::Ident("settings".into())) && (w.len() == 1 || w[w.len() - 2] == &Tok::Punct('.'));
                let is_settings = ends_settings(&words)
                    || (words.len() >= 5
                        && words[words.len() - 4..] == [&Tok::Punct('.'), &Tok::Ident("clone".into()), &Tok::Punct('('), &Tok::Punct(')')]
                        && ends_settings(&words[..words.len() - 4]));
                if is_settings {
                    ever.insert(name.to_string());
                }
                // Only names that are or were settings locals matter: a `let` of any other
                // name can neither be one nor hide one.
                if is_settings || ever.contains(name) {
                    live.push((name.to_string(), depth, is_settings));
                }
            }
        }
        // A parameter typed SettingsState: `name : & [mut] [path ::] SettingsState`, alive in the
        // function body that follows (its depth is the body's).
        if let (Some(name), true) = (toks.get(i).and_then(ident), is_p(toks, i + 1, ':') && !is_p(toks, i + 2, ':')) {
            let mut j = i + 2;
            while j < toks.len() && j < i + 12 && !is_p(toks, j, ',') && !is_p(toks, j, ')') {
                if is_id(toks, j, "SettingsState") {
                    ever.insert(name.to_string());
                    live.push((name.to_string(), depth + 1, true));
                    break;
                }
                j += 1;
            }
        }
        // A use: `name .` (not itself after a `.`). The innermost binding of the name decides.
        if let Some(name) = toks.get(i).and_then(ident) {
            if is_p(toks, i + 1, '.') && !is_p(toks, i.wrapping_sub(1), '.') && ever.contains(name) {
                if let Some((_, _, true)) = live.iter().rev().find(|(n, _, _)| n == name) {
                    at.insert(i);
                }
            }
        }
    }
    at
}

/// The name of the innermost call whose parentheses hold token `i` (`toggle` in
/// `widgets::toggle(ui, theme, "x", &mut v)`; `Slider` in `egui::Slider::new(&mut v, ..)`).
fn innermost_call(toks: &[Token], i: usize) -> Option<String> {
    let mut d = 0i32;
    let mut k = i;
    while k > 0 {
        k -= 1;
        match toks[k].tok {
            Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => d += 1,
            Tok::Punct('[') | Tok::Punct('{') => {
                if d == 0 {
                    return None;
                }
                d -= 1;
            }
            Tok::Punct('(') => {
                if d == 0 {
                    let name = toks.get(k.wrapping_sub(1)).and_then(ident)?;
                    if name == "new" && k >= 4 && is_p(toks, k - 2, ':') && is_p(toks, k - 3, ':') {
                        return toks.get(k - 4).and_then(ident).map(str::to_string);
                    }
                    return Some(name.to_string());
                }
                d -= 1;
            }
            Tok::Punct(';') if d == 0 => return None,
            _ => {}
        }
    }
    None
}

/// Every mention of a setting in `f`, outside test code.
fn mentions(f: &SrcFile, on_settings_page: bool) -> Vec<Mention> {
    let toks = &f.toks;
    let aliases = settings_aliases(toks);
    let mut out = Vec::new();
    for i in 0..toks.len() {
        if f.test[i] {
            continue;
        }
        let Some(w) = ident(&toks[i]) else { continue };
        // `<..>.settings.X` or `settings.X`, or `alias.X` (an alias right before a `.`, and not
        // itself after a `.`: `a.s.x` is not the local `s`).
        let via_path = w == "settings";
        let via_alias = !via_path && aliases.contains(&i);
        if !(via_path || via_alias) || !is_p(toks, i + 1, '.') {
            continue;
        }
        let Some(setting) = toks.get(i + 2).and_then(ident) else { continue };
        if is_p(toks, i + 3, '(') {
            continue; // a method on the settings themselves (`settings.clone()`)
        }
        // The start of the path (`state` in `&mut state.gui_state.settings.x`).
        let mut start = i;
        while start >= 2 && is_p(toks, start - 1, '.') && toks.get(start - 2).and_then(ident).is_some() {
            start -= 2;
        }
        // What follows X: fields, indexes and calls, then maybe an assignment.
        let mut j = i + 3;
        loop {
            if is_p(toks, j, '.') && toks.get(j + 1).and_then(ident).is_some() {
                j += 2;
            } else if is_p(toks, j, '(') || is_p(toks, j, '[') {
                let mut d = 0;
                while j < toks.len() {
                    match toks[j].tok {
                        Tok::Punct('(') | Tok::Punct('[') => d += 1,
                        Tok::Punct(')') | Tok::Punct(']') => {
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                j += 1;
            } else {
                break;
            }
        }
        let assign = (is_p(toks, j, '=') && !is_p(toks, j + 1, '='))
            || (['+', '-', '*', '/', '%', '|', '&', '^'].iter().any(|c| is_p(toks, j, *c)) && is_p(toks, j + 1, '=') && !is_p(toks, j + 2, '='));
        let by_mut = start >= 2 && is_id(toks, start - 1, "mut") && is_p(toks, start - 2, '&');
        let copy = on_settings_page
            && start >= 4
            && is_p(toks, start - 1, '=')
            && toks.get(start - 2).and_then(ident).is_some()
            && is_id(toks, start - 3, "mut")
            && is_id(toks, start - 4, "let")
            && j == i + 3
            && (is_p(toks, j, ';') || (is_id(toks, j, "as") && is_p(toks, j + 2, ';')));
        let copy_clone = on_settings_page
            && start >= 4
            && is_p(toks, start - 1, '=')
            && is_id(toks, start - 3, "mut")
            && is_id(toks, start - 4, "let")
            && is_p(toks, i + 3, '.')
            && is_id(toks, i + 4, "clone")
            && is_p(toks, i + 5, '(')
            && is_p(toks, i + 6, ')')
            && is_p(toks, i + 7, ';');
        let kind = if assign {
            Kind::Write
        } else if by_mut && (on_settings_page || innermost_call(toks, start - 2).map(|c| WIDGET_CALLS.contains(&c.as_str())).unwrap_or(false)) {
            Kind::Binding
        } else if copy || copy_clone {
            Kind::Copy
        } else {
            Kind::Read
        };
        out.push(Mention { setting: setting.to_string(), file: f.rel.clone(), line: toks[i + 2].line, func: f.fn_of[i], kind });
    }
    out
}

/// The whole scan: what the Settings page binds, and who reads each.
struct Scan {
    page_files: BTreeSet<String>,
    /// setting -> the Settings page files that bind or write it.
    bound: BTreeMap<String, BTreeSet<String>>,
    /// setting -> where it is read ("file:line").
    readers: BTreeMap<String, Vec<String>>,
}

/// The field names of `pub struct AppConfig` in src/config.rs: what is saved.
fn app_config_fields(config: &str) -> HashSet<String> {
    let start = config.find("pub struct AppConfig").expect("AppConfig in src/config.rs");
    let end = config[start..].find("\n}").map(|e| start + e).expect("AppConfig closes");
    let fields: HashSet<String> = config[start..end]
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub ").and_then(|r| r.split_once(':')).map(|(n, _)| n.trim().to_string()))
        .filter(|n| n.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect();
    assert!(fields.len() > 60, "sanity: AppConfig has only {} fields; the reader broke", fields.len());
    fields
}

fn scan() -> &'static Scan {
    static SCAN: OnceLock<Scan> = OnceLock::new();
    SCAN.get_or_init(scan_tree)
}

fn scan_tree() -> Scan {
    let texts = all_texts();
    let tests = test_module_files(&texts);
    let mut files: HashMap<String, SrcFile> = HashMap::new();
    let page_files = settings_page_files(&texts, &mut files);
    // Only a file that says `settings` or `SettingsState` can mention a setting (a local holding
    // the settings is bound from one or the other), so only those are tokenized.
    for (rel, text) in &texts {
        if !tests.contains(rel) && !files.contains_key(rel) && (text.contains("settings") || text.contains("SettingsState")) {
            files.insert(rel.clone(), parse(rel, text));
        }
    }
    let mut all: Vec<(Mention, bool)> = Vec::new();
    for (rel, f) in &files {
        if tests.contains(rel) {
            continue;
        }
        let on_page = page_files.contains(rel);
        for m in mentions(f, on_page) {
            all.push((m, on_page));
        }
    }
    // Only SAVED settings count (fields on AppConfig): the page's own scratch state (which section
    // is open, a one-frame scroll request, a typed recovery phrase) is not a setting, and its page
    // is its only reader by design (settings_persistence_lint's INTENTIONALLY_TRANSIENT).
    let saved = app_config_fields(&texts["src/config.rs"]);
    let mut bound: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // (file, function) pairs that bind or write each setting on the Settings page: the control's
    // own functions, whose mentions of it are the control showing itself.
    let mut control_fns: HashMap<String, HashSet<(String, Option<usize>)>> = HashMap::new();
    for (m, on_page) in &all {
        if *on_page && matches!(m.kind, Kind::Binding | Kind::Write | Kind::Copy) {
            if m.kind != Kind::Copy && saved.contains(&m.setting) {
                bound.entry(m.setting.clone()).or_default().insert(m.file.clone());
            }
            control_fns.entry(m.setting.clone()).or_default().insert((m.file.clone(), m.func));
        }
    }
    let mut readers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (m, on_page) in &all {
        if m.kind != Kind::Read && !(m.kind == Kind::Binding && !on_page) {
            continue;
        }
        // A widget elsewhere is a second control, not a use.
        if m.kind == Kind::Binding {
            continue;
        }
        let f = &files[&m.file];
        let func = m.func.map(|i| f.fn_names[i].as_str());
        if m.file == "src/config.rs" && func.map(|n| SAVE_AND_LOAD.contains(&n)).unwrap_or(false) {
            continue;
        }
        if *on_page && control_fns.get(&m.setting).map(|s| s.contains(&(m.file.clone(), m.func))).unwrap_or(false) {
            continue;
        }
        readers.entry(m.setting.clone()).or_default().push(format!("{}:{}", m.file, m.line));
    }
    Scan { page_files, bound, readers }
}

fn unread(s: &Scan) -> Vec<String> {
    let excused: HashSet<&str> = APPLIED_WHERE_SET.iter().map(|(n, _, _)| *n).collect();
    s.bound.keys().filter(|k| !s.readers.contains_key(*k) && !excused.contains(k.as_str())).cloned().collect()
}

// ── The tests ───────────────────────────────────────────────────────────────────────────────────

/// THE CHECK: every setting the Settings page binds is read by something other than its
/// control, its save and its load.
#[test]
fn every_setting_the_settings_page_binds_is_read() {
    let s = scan();
    assert!(s.bound.len() > 60, "sanity: the Settings page binds only {} settings; the scan broke", s.bound.len());
    let known: HashSet<&str> = KNOWN_OFFENDERS.iter().map(|(n, _)| *n).collect();
    let new: Vec<String> = unread(&s)
        .into_iter()
        .filter(|n| !known.contains(n.as_str()))
        .map(|n| {
            let files: Vec<&str> = s.bound[&n].iter().map(String::as_str).collect();
            format!("  {n}: bound in {}, read by nothing but its control, its save and its load", files.join(", "))
        })
        .collect();
    assert!(
        new.is_empty(),
        "\n\nThese settings have a control on the Settings page and are read by NOTHING else:\n\n{}\n\n\
         Moving the control changes nothing, while its words say it does (BUG-235: a privacy \
         switch that hid nothing). Make the setting do what its words say (read it where it \
         takes effect), or take the control out. Never add it to KNOWN_OFFENDERS to pass: \
         that list only shrinks. If the setting is applied on the spot by its control and its \
         load through a call this scan cannot follow, add it to APPLIED_WHERE_SET with the \
         line that applies it.\n",
        new.join("\n")
    );
}

/// A known offender that is fixed must leave the list (so it cannot quietly come back), and
/// one that no longer exists must too.
#[test]
fn known_offenders_only_shrink() {
    let s = scan();
    let unread: HashSet<String> = unread(&s).into_iter().collect();
    let mut stale = Vec::new();
    for (name, bug) in KNOWN_OFFENDERS {
        if !s.bound.contains_key(*name) {
            stale.push(format!("  {name} ({bug}): no longer bound on the Settings page; delete its line"));
        } else if !unread.contains(*name) {
            let at = s.readers.get(*name).map(|r| r.join(", ")).unwrap_or_default();
            stale.push(format!("  {name} ({bug}): now read ({at}); delete its line"));
        }
    }
    assert!(stale.is_empty(), "\n\nKNOWN_OFFENDERS lists settings that are fixed:\n\n{}\n", stale.join("\n"));
}

/// Each APPLIED_WHERE_SET excuse names a line that must exist: the call that applies it.
#[test]
fn applied_where_set_still_applies() {
    let s = scan();
    let mut broken = Vec::new();
    for (name, file, line) in APPLIED_WHERE_SET {
        let text = fs::read_to_string(repo().join(file)).unwrap_or_default();
        if !text.contains(line) {
            broken.push(format!("  {name}: {file} no longer has `{line}`"));
        }
        if !s.bound.contains_key(*name) {
            broken.push(format!("  {name}: no longer bound on the Settings page; delete its lines"));
        }
    }
    assert!(
        broken.is_empty(),
        "\n\nAPPLIED_WHERE_SET's excuses no longer hold:\n\n{}\n\nWithout that line the setting may do nothing.\n",
        broken.join("\n")
    );
}

/// `SETTINGS_USED_REPORT=1` prints what the scan decided: the Settings page's files, then each
/// setting they bind with its first reader (or why it has none). For checking the scan by eye.
#[test]
fn report_when_asked() {
    if std::env::var("SETTINGS_USED_REPORT").is_err() {
        return;
    }
    let s = scan();
    println!("Settings page files ({}):", s.page_files.len());
    for f in &s.page_files {
        println!("  {f}");
    }
    let excused: HashSet<&str> = APPLIED_WHERE_SET.iter().map(|(n, _, _)| *n).collect();
    println!("Bound settings ({}):", s.bound.len());
    for name in s.bound.keys() {
        let what = match s.readers.get(name) {
            Some(r) => format!("{} readers, first {}", r.len(), r[0]),
            None if excused.contains(name.as_str()) => "applied where set".to_string(),
            None => "UNREAD".to_string(),
        };
        println!("  {name}: {what}");
    }
}

/// The scan finds the Settings page's files from settings.rs itself; these are the ones it must
/// at least find, so a broken resolver fails here instead of quietly checking less.
#[test]
fn the_settings_page_files_are_found() {
    let s = scan();
    for must in [
        "src/gui/pages/settings.rs",
        "src/gui/pages/privacy.rs",
        "src/gui/pages/safety.rs",
        "src/gui/pages/settings/calls.rs",
        "src/gui/pages/settings/notifications.rs",
        "src/gui/pages/settings_time.rs",
        "src/gui/pages/files/in_messages.rs",
    ] {
        assert!(s.page_files.contains(must), "the Settings page's files should include {must}; found {:?}", s.page_files);
    }
}

/// The scanner's own rules, each able to fail, on small sources.
#[test]
fn the_scanner_tells_reads_from_controls() {
    let file = |src: &str, page: bool| -> Vec<(String, Kind)> {
        let toks = tokenize(src);
        let test = test_mask(&toks);
        let (fn_of, fn_names) = functions(&toks);
        let f = SrcFile { rel: "x.rs".into(), toks, test, fn_of, fn_names };
        mentions(&f, page).into_iter().map(|m| (m.setting, m.kind)).collect()
    };
    // A binding and a write on the page; a working copy; a read.
    let page = file(
        "fn draw(state: &mut GuiState) {\n\
           if widgets::toggle(ui, theme, \"A\", &mut state.settings.a) { }\n\
           state.settings.b = 3;\n\
           state.settings.c += 1;\n\
           let mut v = state.settings.d;\n\
           let mut h = state.settings.e as f32;\n\
           let mut s2 = state.settings.f.clone();\n\
           let shown = state.settings.g == 1;\n\
         }",
        true,
    );
    assert_eq!(
        page,
        vec![
            ("a".into(), Kind::Binding),
            ("b".into(), Kind::Write),
            ("c".into(), Kind::Write),
            ("d".into(), Kind::Copy),
            ("e".into(), Kind::Copy),
            ("f".into(), Kind::Copy),
            ("g".into(), Kind::Read),
        ]
    );
    // Elsewhere: a path split over lines, a local holding the settings, a parameter, `&mut`
    // handed to code that uses it, and a second control.
    let elsewhere = file(
        "fn frame(state: &mut State) {\n\
           let cap = state.gui_state.settings\n    .fps;\n\
           let s = &state.gui_state.settings;\n\
           let x = s.fov;\n\
           apply(&mut state.gui_state.settings.zoom);\n\
           ui.add(egui::Slider::new(&mut state.settings.speed, 0.0..=1.0));\n\
           let t = \"settings.font_size\"; // settings.dark_mode\n\
         }\n\
         fn take(st: &crate::gui::SettingsState) { let y = st.volume; }\n\
         #[cfg(test)] mod tests { fn t() { let z = state.settings.only_in_tests; } }",
        false,
    );
    assert_eq!(
        elsewhere,
        vec![
            ("fps".into(), Kind::Read),
            ("fov".into(), Kind::Read),
            ("zoom".into(), Kind::Read),
            ("speed".into(), Kind::Binding),
            ("volume".into(), Kind::Read),
        ]
    );
    // A local of another kind with the same name hides the settings one.
    let shadow = file("fn f() { let s = &state.settings; { let s = other(); s.fov; } s.fov; }", false);
    assert_eq!(shadow, vec![("fov".into(), Kind::Read)], "only the outer s.fov reads the settings");
}

// ── The privacy levels' switches (BUG-238, 2026-11-03) ──────────────────────────────────────────
//
// A privacy level (`data/gui/privacy_tiers.json`, the file both apps read) is a preset over
// switches, and its words say what each does. The scan above cannot see them: a level's field
// is not a setting bound to a control. So the Spotlight level's `promote_streaming`, behind the
// words "your live-stream status shown so viewers can find you the moment you go live", sat in
// the file and on the desktop's `PrivacyTier`, and nothing in either app read it. This check:
// every true/false field a level carries in that file is a switch, and BOTH apps must read it
// where levels are used (a level the website applies differently from the desktop is the same
// promise broken in one of the two).
// - THE DESKTOP: it is a field of `pub struct PrivacyTier` (src/gui/pages/privacy.rs; serde
//   ignores a field the struct lacks, so one only in the file is read by nothing), and some
//   src/ file that names `PrivacyTier` reads it as `.field` outside test code. Its declaration
//   (`pub field: bool`) and a struct literal's `field: true` are not reads.
// - THE WEBSITE: some web/ script that names the levels' file (`privacy_tiers.json`) reads it
//   as `.field` in code: comments and the inside of strings do not count (code inside a
//   template's `${ }` does).

/// The levels' file, and the desktop file that declares a level.
const TIERS_FILE: &str = "data/gui/privacy_tiers.json";
const TIER_STRUCT_FILE: &str = "src/gui/pages/privacy.rs";

/// The keys whose value is `true` or `false`, anywhere in `json` (a JSON text). Strings are read
/// with their escapes, so a description holding `"x": true` as words is not a key.
fn json_bool_keys(json: &str) -> BTreeSet<String> {
    let b: Vec<char> = json.chars().collect();
    let n = b.len();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < n {
        if b[i] != '"' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let mut s = String::new();
        while j < n && b[j] != '"' {
            if b[j] == '\\' {
                j += 1;
            }
            if j < n {
                s.push(b[j]);
            }
            j += 1;
        }
        let mut k = j + 1;
        while k < n && b[k].is_whitespace() {
            k += 1;
        }
        if k < n && b[k] == ':' {
            k += 1;
            while k < n && b[k].is_whitespace() {
                k += 1;
            }
            let rest: String = b[k..(k + 5).min(n)].iter().collect();
            if rest.starts_with("true") || rest.starts_with("false") {
                out.insert(s);
            }
        }
        i = j + 1;
    }
    out
}

/// The field names of `pub struct <name>` in `src`.
fn struct_fields(src: &str, name: &str) -> BTreeSet<String> {
    let Some(start) = src.find(&format!("pub struct {name}")) else { return BTreeSet::new() };
    let end = src[start..].find("\n}").map(|e| start + e).unwrap_or(src.len());
    src[start..end]
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub ").and_then(|r| r.split_once(':')).map(|(n, _)| n.trim().to_string()))
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect()
}

/// Where `f` reads `.field` outside test code, as line numbers.
fn rust_dot_reads(f: &SrcFile, field: &str) -> Vec<usize> {
    (1..f.toks.len()).filter(|&i| !f.test[i] && is_id(&f.toks, i, field) && is_p(&f.toks, i - 1, '.')).map(|i| f.toks[i].line).collect()
}

/// `src` (JavaScript) with its comments and the inside of its strings turned to spaces, newlines
/// kept so a line number still points at the line. A template literal is a string, except the
/// code inside its `${ }`.
fn js_code(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut out: Vec<char> = Vec::with_capacity(n);
    let mut depth = 0usize; // `{ }` nesting in code
    let mut resume: Vec<usize> = Vec::new(); // the depths at which a `${ }` closes back into its template
    let mut in_template = false;
    let mut i = 0;
    while i < n {
        let c = b[i];
        if in_template {
            if c == '\\' {
                out.push(' ');
                if i + 1 < n {
                    out.push(blank(b[i + 1]));
                }
                i += 2;
            } else if c == '`' {
                out.push(' ');
                in_template = false;
                i += 1;
            } else if c == '$' && i + 1 < n && b[i + 1] == '{' {
                out.push(' ');
                out.push(' ');
                resume.push(depth);
                depth += 1;
                in_template = false;
                i += 2;
            } else {
                out.push(blank(c));
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < n && b[i + 1] == '/' {
            while i < n && b[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && i + 1 < n && b[i + 1] == '*' {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i < n && !(b[i] == '*' && i + 1 < n && b[i + 1] == '/') {
                out.push(blank(b[i]));
                i += 1;
            }
            if i < n {
                out.push(' ');
                out.push(' ');
                i += 2;
            }
        } else if c == '\'' || c == '"' {
            out.push(' ');
            i += 1;
            while i < n && b[i] != c && b[i] != '\n' {
                if b[i] == '\\' && i + 1 < n {
                    out.push(' ');
                    out.push(blank(b[i + 1]));
                    i += 2;
                    continue;
                }
                out.push(' ');
                i += 1;
            }
            if i < n && b[i] == c {
                out.push(' ');
                i += 1;
            }
        } else if c == '`' {
            out.push(' ');
            in_template = true;
            i += 1;
        } else if c == '{' {
            depth += 1;
            out.push(c);
            i += 1;
        } else if c == '}' {
            depth = depth.saturating_sub(1);
            if resume.last() == Some(&depth) {
                resume.pop();
                in_template = true;
                out.push(' ');
            } else {
                out.push(c);
            }
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out.into_iter().collect()
}

/// Where `code` (from [`js_code`]) reads `.field`: a `.` after a name, `)` or `]`, then the
/// field's name whole. As line numbers.
fn js_dot_reads(code: &str, field: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let pat = format!(".{field}");
    let id = |c: char| c == '_' || c == '$' || c.is_alphanumeric();
    for (n, line) in code.lines().enumerate() {
        let mut from = 0;
        while let Some(at) = line[from..].find(&pat).map(|a| a + from) {
            let before = line[..at].chars().last();
            let after = line[at + pat.len()..].chars().next();
            if before.map(|c| id(c) || c == ')' || c == ']').unwrap_or(false) && !after.map(id).unwrap_or(false) {
                out.push(n + 1);
            }
            from = at + pat.len();
        }
    }
    out
}

/// Every .js file under web/, by its path from the repo.
fn web_scripts() -> BTreeMap<String, String> {
    fn walk_js(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                walk_js(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("js") {
                out.push(p);
            }
        }
    }
    let mut paths = Vec::new();
    walk_js(&repo().join("web"), &mut paths);
    paths.iter().filter_map(|p| fs::read_to_string(p).ok().map(|t| (rel_of(p), t.replace("\r\n", "\n")))).collect()
}

/// For each switch a level carries: where the desktop reads it, and where the website does.
struct TierScan {
    switches: BTreeSet<String>,
    declared: BTreeSet<String>,
    desktop: BTreeMap<String, Vec<String>>,
    website: BTreeMap<String, Vec<String>>,
}

fn tier_scan() -> TierScan {
    let tiers = fs::read_to_string(repo().join(TIERS_FILE)).unwrap_or_else(|e| panic!("read {TIERS_FILE}: {e}"));
    let switches = json_bool_keys(&tiers);
    let texts = all_texts();
    let declared = struct_fields(&texts[TIER_STRUCT_FILE], "PrivacyTier");
    let tests = test_module_files(&texts);
    let mut desktop: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (rel, text) in &texts {
        if tests.contains(rel) || !text.contains("PrivacyTier") {
            continue;
        }
        let f = parse(rel, text);
        for s in &switches {
            for line in rust_dot_reads(&f, s) {
                desktop.entry(s.clone()).or_default().push(format!("{rel}:{line}"));
            }
        }
    }
    let mut website: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (rel, text) in web_scripts() {
        if !text.contains("privacy_tiers.json") {
            continue;
        }
        let code = js_code(&text);
        for s in &switches {
            for line in js_dot_reads(&code, s) {
                website.entry(s.clone()).or_default().push(format!("{rel}:{line}"));
            }
        }
    }
    TierScan { switches, declared, desktop, website }
}

/// THE CHECK: every switch a privacy level carries is applied by both apps.
#[test]
fn every_privacy_level_switch_is_read_by_both_apps() {
    let s = tier_scan();
    assert!(s.switches.contains("hide_presence"), "sanity: {TIERS_FILE} has no hide_presence; the reader broke ({:?})", s.switches);
    let mut broken = Vec::new();
    for sw in &s.switches {
        let mut why = Vec::new();
        if !s.declared.contains(sw) {
            why.push(format!("not a field of PrivacyTier in {TIER_STRUCT_FILE}, so the desktop app drops it as it loads the file"));
        } else if !s.desktop.contains_key(sw) {
            why.push("read by nothing in the desktop app (no `.field` in a src/ file that names PrivacyTier)".to_string());
        }
        if !s.website.contains_key(sw) {
            why.push("read by nothing on the website (no `.field` in a web/ script that loads privacy_tiers.json)".to_string());
        }
        if !why.is_empty() {
            broken.push(format!("  {sw}: {}", why.join("; ")));
        }
    }
    assert!(
        broken.is_empty(),
        "\n\nA privacy level in {TIERS_FILE} carries a switch that an app does not apply:\n\n{}\n\n\
         A level's words promise what its switches do (BUG-238: Spotlight promised to show a live \
         stream to viewers, and its promote_streaming was read by nothing). Apply the switch in \
         both apps where a level is applied (the desktop's `apply_tier` in \
         src/gui/pages/privacy.rs; on the website, the script that loads privacy_tiers.json), \
         or take it, and the words that promise it, out of the file.\n",
        broken.join("\n")
    );
}

/// The levels' scan, each rule able to fail, on small sources.
#[test]
fn the_level_scan_tells_reads_from_declarations() {
    // Keys with true/false values; a string value, and words inside a string, are not.
    let keys = json_bool_keys(r#"{"tiers":[{"id":"a","name":"say \"x\": true","on":true,"off" : false,"n":1}]}"#);
    assert_eq!(keys.into_iter().collect::<Vec<_>>(), vec!["off".to_string(), "on".to_string()]);
    // The struct's fields, a reader, and what is not a read: the declaration and a literal.
    let src = "pub struct PrivacyTier {\n    pub id: String,\n    #[serde(default)]\n    pub on: bool,\n}\n\
               fn fallback() -> PrivacyTier { PrivacyTier { id: String::new(), on: true } }\n\
               fn apply(t: &PrivacyTier) { send(t.on); }\n\
               #[cfg(test)] mod tests { fn x(t: &PrivacyTier) { t.off; } }";
    assert_eq!(struct_fields(src, "PrivacyTier").into_iter().collect::<Vec<_>>(), vec!["id".to_string(), "on".to_string()]);
    let f = parse("x.rs", src);
    assert_eq!(rust_dot_reads(&f, "on"), vec![7], "only t.on in apply reads it");
    assert!(rust_dot_reads(&f, "off").is_empty(), "a read in test code is not one");
    // JavaScript: a read in code and inside a template's ${ }; none in a comment or a string.
    let js = "// tier.c in a comment\nconst a = tier.on;\nconst s = 'tier.off';\nconst t = `x ${tier.mid} y.end`;\n/* tier.block */ f(x).sw;\nconst q = tier.onward;";
    let code = js_code(js);
    assert_eq!(code.lines().count(), js.lines().count(), "line numbers are kept");
    assert_eq!(js_dot_reads(&code, "on"), vec![2], "tier.on; not tier.onward");
    assert!(js_dot_reads(&code, "off").is_empty(), "words in a string");
    assert!(js_dot_reads(&code, "c").is_empty(), "a comment");
    assert_eq!(js_dot_reads(&code, "mid"), vec![4], "code inside ${{ }}");
    assert!(js_dot_reads(&code, "end").is_empty(), "the template's own words");
    assert!(js_dot_reads(&code, "block").is_empty(), "a block comment");
    assert_eq!(js_dot_reads(&code, "sw"), vec![5], "after a call's )");
}

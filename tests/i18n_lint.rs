//! Ready for translation: the native app's words come from `data/i18n/en.json`
//! by key, and the count of words still written into the code only goes DOWN
//! (the ELI5 mandate, docs/PRIORITIES.md TIER 3; operator 2026-10-22: "let's
//! only focus on English for now but, make sure we prep for all the other
//! languages").
//!
//! What it checks, each a test of its own:
//!
//! 1. `native_ui_literals_only_shrink`, THE RATCHET. Every file under `src/gui/`
//!    has a BUDGET in `tests/i18n_literal_budget.txt`: how many words it still
//!    writes straight into a UI call (`ui.label("Save")`,
//!    `Button::primary("Send")`, `.on_hover_text("...")`, `RichText::new("...")`,
//!    a project widget such as `section_header(ui, theme, "...")`, and so on).
//!    More than its budget fails: put the words in `data/i18n/en.json` and write
//!    `t("page.key")`. FEWER than its budget fails too, asking for the click
//!    that locks the win in (`I18N_RATCHET_WRITE=1`, below), so a word moved out
//!    cannot quietly be spent on a new one. A file not on the list has a budget
//!    of ZERO: new UI files start translatable.
//! 2. `every_key_the_code_asks_for_is_in_english`. Every literal key passed to
//!    `t`, `t_marked`, `tf`, `tn` or `.hint(...)` under `src/` exists in
//!    `data/i18n/en.json` (for `tn`, both `.one` and `.other`). A typo is a
//!    failing build, not a key shown to a person.
//! 3. `every_language_file_is_a_subset_of_english`. Each `data/i18n/<code>.json`
//!    parses, names itself (`meta.code` equals the file name), holds no key
//!    English no longer has (a stale translation), and uses only the `{{name}}`
//!    placeholders its English string uses.
//!
//! WHAT COUNTS AS A WORD IN A UI CALL. A string literal with two letters in a
//! row (placeholders like `{}` and `{name}` removed first), not a URL and not an
//! identifier-like token (`nav.home`, `data/x.json`, `chat_input`), whose
//! innermost enclosing call is one of `TEXT_CALLS`, reached through only
//! "transparent" wrappers (`format!`, `.into()`, `String::from`, `Some`, an
//! array, an `if`/`match` block). A closure body (`|ui| { ... }`) stops the walk,
//! so `ui.collapsing("Title", |ui| { log::info!("x") })` counts "Title" and not
//! "x". Test code (`#[cfg(test)]` items, `*_tests.rs`, `ui_snapshots.rs`) is not
//! scanned. A line with `i18n-exempt: <reason>` is skipped: a brand name, a unit
//! symbol, a word that is the same in every language.
//!
//! It is a proxy, deliberately simple, and it misses words kept in a `const`
//! array or built into a variable before reaching a label. Those move when their
//! page is converted; the ratchet only promises the visible count never grows.
//!
//! THE BUDGET FILE. `tests/i18n_literal_budget.txt`, one `<count> <path>` line
//! per file. To lock in a win (only lowers, never raises, drops deleted files):
//!   I18N_RATCHET_WRITE=1 CARGO_MANIFEST_DIR=<repo> <this test> native_ui_literals_only_shrink
//! `I18N_RATCHET_WRITE=init` rewrites it from the current counts, raising
//! included: that is for installing the ratchet (2026-10-22) and for an
//! operator decision recorded in the commit message, never to make red go green.
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/i18n_lint.rs

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const BUDGET_FILE: &str = "tests/i18n_literal_budget.txt";
const EXEMPT: &str = "i18n-exempt";

// ── A Rust tokenizer, just enough to find string literals and calls ─────────

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
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
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let n = b.len();
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
        // Comments.
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
        // Raw strings r"..", r#".."#, br"..", and byte/c strings b"..", c"..".
        let raw_start = if (c == 'r' || ((c == 'b' || c == 'c') && i + 1 < n && b[i + 1] == 'r'))
            && !(i > 0 && is_id(b[i - 1]))
        {
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
        if let Some((start, hashes)) = raw_start {
            let start_line = line;
            let mut j = start;
            let mut s = String::new();
            loop {
                if j >= n {
                    break;
                }
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
                s.push(b[j]);
                j += 1;
            }
            out.push(Token { tok: Tok::Str(s), line: start_line });
            i = j;
            continue;
        }
        let plain_prefix = (c == 'b' || c == 'c') && i + 1 < n && b[i + 1] == '"' && !(i > 0 && is_id(b[i - 1]));
        if c == '"' || plain_prefix {
            let start_line = line;
            let mut j = if plain_prefix { i + 2 } else { i + 1 };
            let mut s = String::new();
            while j < n && b[j] != '"' {
                if b[j] == '\\' && j + 1 < n {
                    // Keep escapes roughly: "\n" -> newline, "\u{..}" -> one char, else the char.
                    let e = b[j + 1];
                    if e == '\n' {
                        line += 1;
                        j += 2;
                        while j < n && b[j].is_whitespace() {
                            if b[j] == '\n' {
                                line += 1;
                            }
                            j += 1;
                        }
                        continue;
                    }
                    if e == 'u' && j + 2 < n && b[j + 2] == '{' {
                        let mut k = j + 3;
                        let mut hex = String::new();
                        while k < n && b[k] != '}' {
                            hex.push(b[k]);
                            k += 1;
                        }
                        if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            s.push(ch);
                        }
                        j = k + 1;
                        continue;
                    }
                    s.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                    j += 2;
                    continue;
                }
                if b[j] == '\n' {
                    line += 1;
                }
                s.push(b[j]);
                j += 1;
            }
            out.push(Token { tok: Tok::Str(s), line: start_line });
            i = j + 1;
            continue;
        }
        // Char literal or lifetime.
        if c == '\'' {
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
            // A lifetime or label: skip the quote and the name.
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
            // Raw identifier r#name.
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

/// Mark the tokens of test-only items: `#[cfg(test)]` (or `cfg(all(test, ..))`)
/// and `#[test]`, through the end of the item's `{ ... }` block. An attribute
/// followed by `;` before any `{` (`#[cfg(test)] mod tests;`) masks nothing.
fn test_mask(toks: &[Token]) -> Vec<bool> {
    let mut mask = vec![false; toks.len()];
    let mut i = 0;
    while i < toks.len() {
        if toks[i].tok == Tok::Punct('#') && i + 1 < toks.len() && toks[i + 1].tok == Tok::Punct('[') {
            // Read the attribute up to its closing bracket.
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
            let is_test = (words.first().map(|w| w == "cfg").unwrap_or(false)
                && words.iter().any(|w| w == "test")
                && !words.iter().any(|w| w == "not"))
                || words == ["test"];
            if is_test {
                // Find the item's opening brace (or a `;` that ends it first).
                let mut k = j;
                while k < toks.len() && toks[k].tok != Tok::Punct('{') && toks[k].tok != Tok::Punct(';') {
                    k += 1;
                }
                if k < toks.len() && toks[k].tok == Tok::Punct('{') {
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
                    for x in i..=m.min(toks.len() - 1) {
                        mask[x] = true;
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

// ── Which calls carry words ─────────────────────────────────────────────────

/// Calls whose string arguments a person reads.
const TEXT_CALLS: &[&str] = &[
    // egui
    "label", "heading", "monospace", "small", "strong", "weak", "code", "button", "small_button",
    "checkbox", "radio", "radio_value", "selectable_label", "selectable_value", "toggle_value",
    "hyperlink", "hyperlink_to", "link", "collapsing", "menu_button", "on_hover_text",
    "on_hover_text_at_pointer", "on_disabled_hover_text", "hint_text", "selected_text", "from_label",
    "text", "labeled", "suffix", "prefix", "custom_formatter", "layout_no_wrap", "layout", "simple",
    "single_section", "append", "from_label_and_url",
    // the project's widgets (src/gui/widgets/)
    "tooltip", "tooltip_above", "section_header", "subsection_label", "subsection_header", "body_hint",
    "setting_hint", "settings_row", "toggle", "labeled_slider", "labeled_slider_entry", "detail_row",
    "detail_row_bold", "card_with_header", "badge", "badge_sm", "stat_card", "stat_row", "search_bar",
    "primary_button", "secondary_button", "danger_button", "btn_primary", "btn_secondary", "btn_danger",
    "btn_icon", "compact_button", "tab_bar", "sidebar_nav", "category_filter", "progress_bar",
    "hold_to_confirm", "hold_to_confirm_at", "section_disclosure", "tinted_section", "definition_text",
    "collapsible_section", "stepper_button", "empty_state", "help_button", "kv_row", "push_toast",
    "toast", "leaf", "branch", "selectable",
];

/// Constructors that carry words only under these type names (`Button::primary`,
/// `RichText::new`); `new` and `primary` alone mean too many other things.
const QUALIFIED_TEXT_CALLS: &[(&str, &str)] = &[
    ("RichText", "new"), ("Label", "new"), ("Button", "new"), ("Button", "primary"),
    ("Button", "secondary"), ("Button", "danger"), ("Button", "success"), ("Button", "ghost"),
    ("Button", "tab"), ("Button", "icon_only"), ("CollapsingHeader", "new"), ("SelectableLabel", "new"),
    ("Checkbox", "new"), ("RadioButton", "new"), ("SectionNavItem", "new"), ("Hyperlink", "from_label_and_url"),
    ("WidgetText", "from"), ("ComboBox", "from_label"),
];

/// Calls a word passes THROUGH on its way to a text call.
const TRANSPARENT: &[&str] = &[
    "format", "format_args", "concat", "from", "into", "to_string", "to_owned", "Some", "Ok", "Borrowed",
    "Owned", "unwrap_or", "map_or", "then_some", "push_str", "clone", "as_str", "trim",
];

/// Calls whose literal first argument is a KEY, not words.
const KEY_CALLS: &[&str] = &["t", "t_dyn", "t_marked", "tf", "tn", "hint", "hint_key", "has"];

/// What call a `(` opens: the identifier before it (skipping a macro `!`), with
/// its `Type::` qualifier when there is one.
fn call_name(toks: &[Token], open: usize) -> (Option<String>, Option<String>) {
    if open == 0 {
        return (None, None);
    }
    let mut k = open - 1;
    if toks[k].tok == Tok::Punct('!') {
        if k == 0 {
            return (None, None);
        }
        k -= 1;
    }
    // Skip a turbofish `::<T>` just before the paren.
    if toks[k].tok == Tok::Punct('>') {
        let mut d = 0;
        while k > 0 {
            match toks[k].tok {
                Tok::Punct('>') => d += 1,
                Tok::Punct('<') => {
                    d -= 1;
                    if d == 0 {
                        break;
                    }
                }
                _ => {}
            }
            k -= 1;
        }
        if k >= 3 && toks[k - 1].tok == Tok::Punct(':') && toks[k - 2].tok == Tok::Punct(':') {
            k -= 3;
        } else {
            return (None, None);
        }
    }
    let name = match &toks[k].tok {
        Tok::Ident(s) => s.clone(),
        _ => return (None, None),
    };
    let qual = if k >= 3 && toks[k - 1].tok == Tok::Punct(':') && toks[k - 2].tok == Tok::Punct(':') {
        match &toks[k - 3].tok {
            Tok::Ident(q) => Some(q.clone()),
            _ => None,
        }
    } else {
        None
    };
    (Some(name), qual)
}

#[derive(Debug, Clone)]
enum Frame {
    /// A call or a grouping paren. `name`/`qual` from `call_name`; `args_seen`
    /// counts commas, so a key call's FIRST argument can be told apart.
    Paren { name: Option<String>, qual: Option<String>, commas: usize },
    Bracket,
    /// `closure` when the brace opens right after `|`: a closure body.
    Brace { closure: bool },
}

/// Does a literal hold words a person reads?
fn is_words(s: &str) -> bool {
    // Remove format placeholders `{}` `{name}` `{:>5}` (and `{{`/`}}` escapes).
    let mut cleaned = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '{' => depth += 1,
            '}' => {
                if depth > 0 {
                    depth -= 1
                }
            }
            _ if depth == 0 => cleaned.push(c),
            _ => {}
        }
    }
    let t = cleaned.trim();
    if t.contains("://") || t.starts_with("www.") || t.starts_with("mailto:") {
        return false;
    }
    // Identifier-like: no spaces, lowercase/digits/punctuation, with a separator.
    let ident_like = !t.contains(' ')
        && t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.:/#-".contains(c))
        && t.chars().any(|c| "_.:/#".contains(c));
    if ident_like {
        return false;
    }
    let mut run = 0;
    for c in t.chars() {
        if c.is_alphabetic() {
            run += 1;
            if run >= 2 {
                return true;
            }
            // One CJK character is a word.
            if (c as u32) >= 0x3000 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Per line of a file, the string literals that are words in a UI call.
struct Found {
    line: usize,
    text: String,
}

fn ui_literals(src: &str) -> Vec<Found> {
    let toks = tokenize(src);
    let mask = test_mask(&toks);
    let lines: Vec<&str> = src.lines().collect();
    let mut frames: Vec<Frame> = Vec::new();
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match &t.tok {
            Tok::Punct('(') => {
                let (name, qual) = call_name(&toks, i);
                frames.push(Frame::Paren { name, qual, commas: 0 });
            }
            Tok::Punct('[') => frames.push(Frame::Bracket),
            Tok::Punct('{') => {
                let closure = i > 0 && toks[i - 1].tok == Tok::Punct('|');
                frames.push(Frame::Brace { closure });
            }
            Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => {
                frames.pop();
            }
            Tok::Punct(',') => {
                if let Some(Frame::Paren { commas, .. }) = frames.last_mut() {
                    *commas += 1;
                }
            }
            Tok::Str(s) => {
                if mask[i] || !is_words(s) {
                    continue;
                }
                let line_text = lines.get(t.line - 1).copied().unwrap_or("");
                if line_text.contains(EXEMPT) {
                    continue;
                }
                if counts_as_ui_text(&frames) {
                    out.push(Found { line: t.line, text: s.clone() });
                }
            }
            _ => {}
        }
    }
    out
}

/// Walk out from the innermost frame: a text call means words, a key or id call
/// or anything else means not, transparent wrappers and expression blocks pass.
fn counts_as_ui_text(frames: &[Frame]) -> bool {
    for (depth, f) in frames.iter().rev().enumerate() {
        match f {
            Frame::Brace { closure: true } => return false,
            Frame::Brace { closure: false } | Frame::Bracket => continue,
            Frame::Paren { name, qual, commas } => {
                let Some(name) = name else { continue }; // a grouping paren
                if KEY_CALLS.contains(&name.as_str()) && depth == 0 && *commas == 0 {
                    return false;
                }
                if let Some(q) = qual {
                    if QUALIFIED_TEXT_CALLS.iter().any(|(a, b)| a == q && b == name) {
                        return true;
                    }
                }
                if TEXT_CALLS.contains(&name.as_str()) {
                    return true;
                }
                if TRANSPARENT.contains(&name.as_str()) {
                    continue;
                }
                // `String::new`/`Cow::from` and friends pass words through too.
                if matches!(qual.as_deref(), Some("String") | Some("Cow")) {
                    continue;
                }
                return false;
            }
        }
    }
    false
}

// ── The files scanned ───────────────────────────────────────────────────────

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

fn rel(p: &Path) -> String {
    p.strip_prefix(repo()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The UI files the ratchet watches: `src/gui/**`, minus test files.
fn ui_files() -> Vec<PathBuf> {
    let mut all = Vec::new();
    walk(&repo().join("src/gui"), &mut all);
    all.into_iter()
        .filter(|p| {
            let r = rel(p);
            !r.ends_with("_tests.rs") && !r.ends_with("/tests.rs") && !r.ends_with("ui_snapshots.rs") && !r.contains("/tests/")
        })
        .collect()
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())).replace("\r\n", "\n")
}

fn current_counts() -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for p in ui_files() {
        let n = ui_literals(&read(&p)).len();
        m.insert(rel(&p), n);
    }
    m
}

fn read_budget() -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    let Ok(text) = fs::read_to_string(repo().join(BUDGET_FILE)) else { return m };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((n, path)) = line.split_once(' ') {
            if let Ok(n) = n.trim().parse::<usize>() {
                m.insert(path.trim().to_string(), n);
            }
        }
    }
    m
}

fn write_budget(m: &BTreeMap<String, usize>) {
    let total: usize = m.values().sum();
    let mut s = String::new();
    s.push_str("# The native i18n RATCHET's budgets: per file under src/gui/, how many words it\n");
    s.push_str("# still writes straight into a UI call instead of asking data/i18n/en.json by key.\n");
    s.push_str("# Checked by tests/i18n_lint.rs (its header says how to lower these). The numbers\n");
    s.push_str("# only go DOWN; a file not listed has a budget of zero.\n");
    s.push_str(&format!("# Total: {total}\n"));
    for (path, n) in m {
        if *n > 0 {
            s.push_str(&format!("{n} {path}\n"));
        }
    }
    fs::write(repo().join(BUDGET_FILE), s).expect("write the budget file");
}

#[test]
fn native_ui_literals_only_shrink() {
    // `I18N_DUMP=src/gui/pages/notes.rs` lists the words still in that file
    // (run with --nocapture): the to-do list for converting a page.
    if let Ok(path) = std::env::var("I18N_DUMP") {
        for f in ui_literals(&read(&repo().join(&path))) {
            println!("{path}:{}: {:?}", f.line, f.text);
        }
    }
    let now = current_counts();
    let budget = read_budget();
    let mode = std::env::var("I18N_RATCHET_WRITE").unwrap_or_default();
    if mode == "init" {
        write_budget(&now);
        return;
    }
    let mut over = Vec::new();
    let mut under = Vec::new();
    let mut gone = Vec::new();
    for (path, n) in &now {
        let b = budget.get(path).copied().unwrap_or(0);
        if *n > b {
            // Name the new words, so the author sees exactly what to move.
            let found = ui_literals(&read(&repo().join(path)));
            let sample: Vec<String> = found.iter().take(40).map(|f| format!("      line {}: {:?}", f.line, f.text)).collect();
            over.push(format!("  {path}: {n} words in UI calls, budget {b} (+{})\n{}", n - b, sample.join("\n")));
        } else if *n < b {
            under.push((path.clone(), *n, b));
        }
    }
    for path in budget.keys() {
        if !now.contains_key(path) {
            gone.push(path.clone());
        }
    }
    if mode == "1" {
        let mut lowered = budget.clone();
        for (path, n, _) in &under {
            lowered.insert(path.clone(), *n);
        }
        for path in &gone {
            lowered.remove(path);
        }
        write_budget(&lowered);
        under.clear();
        gone.clear();
    }
    assert!(
        over.is_empty(),
        "\n\nWORDS WRITTEN INTO UI CODE, past the file's budget:\n{}\n\n\
         Put the words in data/i18n/en.json under the page's own key and ask for them \
         by key: `ui.label(t(\"page.key\"))` (crate::i18n). A brand name or a \
         symbol that is the same in every language takes `// i18n-exempt: <reason>` \
         on its line. Do not raise the budget.\n",
        over.join("\n")
    );
    let mut msg = String::new();
    for (path, n, b) in &under {
        msg.push_str(&format!("  {path}: {n} words, budget {b}: lower it to {n}\n"));
    }
    for path in &gone {
        msg.push_str(&format!("  {path}: the file is gone; remove its line\n"));
    }
    assert!(
        msg.is_empty(),
        "\n\nRATCHET CLICK (words moved out; lock the win in so it cannot be spent):\n{msg}\n\
         Run: I18N_RATCHET_WRITE=1 with this test (see the header of tests/i18n_lint.rs).\n"
    );
}

// ── A small JSON reader (std-only) ──────────────────────────────────────────

#[derive(Debug, Clone)]
enum Json {
    Obj(Vec<(String, Json)>),
    Arr(Vec<Json>),
    Str(String),
    Other,
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
                    let hex: String = c[*i..*i + 4].iter().collect();
                    *i += 4;
                    let mut cp = u32::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                    if (0xD800..0xDC00).contains(&cp) && c.get(*i) == Some(&'\\') {
                        let hex2: String = c[*i + 2..*i + 6].iter().collect();
                        let lo = u32::from_str_radix(&hex2, 16).map_err(|e| e.to_string())?;
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                        *i += 6;
                    }
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
        Some('"') => Ok(Json::Str(parse_string(c, i)?)),
        Some(_) => {
            let start = *i;
            while *i < c.len() && !",}] \n\r\t".contains(c[*i]) {
                *i += 1;
            }
            if *i == start {
                return Err(format!("expected a value at {start}"));
            }
            Ok(Json::Other)
        }
        None => Err("unexpected end".into()),
    }
}

/// Flatten to dotted keys, strings only (the shape `src/i18n.rs` reads).
fn flatten(prefix: &str, v: &Json, out: &mut BTreeMap<String, String>) {
    match v {
        Json::Obj(m) => {
            for (k, child) in m {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, child, out);
            }
        }
        Json::Str(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        _ => {}
    }
}

fn language_file(code: &str) -> BTreeMap<String, String> {
    let p = repo().join("data/i18n").join(format!("{code}.json"));
    let text = read(&p);
    let v = parse_json(&text).unwrap_or_else(|e| panic!("data/i18n/{code}.json does not parse: {e}"));
    let mut m = BTreeMap::new();
    flatten("", &v, &mut m);
    m
}

fn placeholders(s: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = s;
    while let Some(o) = rest.find("{{") {
        let Some(c) = rest[o + 2..].find("}}") else { break };
        out.insert(rest[o + 2..o + 2 + c].to_string());
        rest = &rest[o + 2 + c + 2..];
    }
    out
}

/// Every literal key the code asks for: the first argument of a key call.
fn keys_asked_for(src: &str) -> Vec<(usize, String, String)> {
    let toks = tokenize(src);
    let mask = test_mask(&toks);
    let mut out = Vec::new();
    for i in 0..toks.len() {
        if mask[i] || toks[i].tok != Tok::Punct('(') {
            continue;
        }
        let (name, _) = call_name(&toks, i);
        let Some(name) = name else { continue };
        if !["t", "t_marked", "tf", "tn", "hint"].contains(&name.as_str()) {
            continue;
        }
        // `.hint(` only counts as a key call on a method chain, and `t(` only
        // as a free call (not `x.t(`), so other functions named alike pass.
        let is_method = i >= 2 && toks[i - 2].tok == Tok::Punct('.');
        if name == "hint" && !is_method {
            continue;
        }
        if name != "hint" && is_method {
            continue;
        }
        if let Some(Token { tok: Tok::Str(k), line }) = toks.get(i + 1) {
            out.push((*line, name.clone(), k.clone()));
        }
    }
    // A hint key written anywhere else (returned by a helper such as
    // calculator.rs's `key_hint`, passed to `with_hint`, kept in a table): a
    // literal shaped like `page.thing_hint` is a key, whoever passes it on.
    let asked: BTreeSet<usize> = out.iter().map(|(l, _, _)| *l).collect();
    for (i, t) in toks.iter().enumerate() {
        if mask[i] {
            continue;
        }
        if let Tok::Str(k) = &t.tok {
            let shaped = k.ends_with("_hint")
                && k.contains('.')
                && k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.');
            if shaped && !(asked.contains(&t.line) && out.iter().any(|(l, _, key)| *l == t.line && key == k)) {
                out.push((t.line, "hint key".to_string(), k.clone()));
            }
        }
    }
    out
}

#[test]
fn every_key_the_code_asks_for_is_in_english() {
    let en = language_file("en");
    let mut all = Vec::new();
    walk(&repo().join("src"), &mut all);
    let mut missing = Vec::new();
    for p in &all {
        let src = read(p);
        if !src.contains("i18n") && !src.contains(".hint(") {
            continue;
        }
        for (line, call, key) in keys_asked_for(&src) {
            let ok = if call == "tn" {
                en.contains_key(&format!("{key}.one")) && en.contains_key(&format!("{key}.other"))
            } else {
                en.contains_key(&key)
            };
            if !ok {
                missing.push(format!("  {}:{line}: {call}({key:?})", rel(p)));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "\n\nKEYS ASKED FOR THAT data/i18n/en.json DOES NOT HAVE (a person would see the key):\n{}\n\
         Add the words to en.json under that key (for tn, both KEY.one and KEY.other).\n",
        missing.join("\n")
    );
}

#[test]
fn every_language_file_is_a_subset_of_english() {
    let en = language_file("en");
    let mut problems = Vec::new();
    let dir = repo().join("data/i18n");
    let mut codes: Vec<String> = fs::read_dir(&dir)
        .expect("data/i18n exists")
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|x| x.to_str()) == Some("json"))
                .then(|| p.file_stem().unwrap().to_string_lossy().to_string())
        })
        .collect();
    codes.sort();
    assert!(codes.contains(&"en".to_string()), "data/i18n/en.json is the fallback and must exist");
    for code in &codes {
        let m = language_file(code);
        if m.get("meta.code").map(|s| s.as_str()) != Some(code.as_str()) {
            problems.push(format!("  {code}.json: meta.code is {:?}, not {code:?}", m.get("meta.code")));
        }
        if m.get("meta.language").map(|s| s.is_empty()).unwrap_or(true) {
            problems.push(format!("  {code}.json: meta.language (its own name) is missing"));
        }
        if code == "en" {
            continue;
        }
        for (k, v) in &m {
            if k.starts_with("meta.") {
                continue;
            }
            match en.get(k) {
                None => problems.push(format!("  {code}.json: {k} is not in English any more (a stale translation)")),
                Some(e) => {
                    let extra: Vec<String> = placeholders(v).difference(&placeholders(e)).cloned().collect();
                    if !extra.is_empty() {
                        problems.push(format!("  {code}.json: {k} uses {{{{{}}}}} which the English does not", extra.join(", ")));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "\n\nLANGUAGE FILE PROBLEMS:\n{}\n", problems.join("\n"));
}

// ── Hints: a plain-language tooltip on every control (phase 2) ──────────────
//
// `native_hint_coverage_only_grows`: per PAGE (a file under src/gui/pages/, or a
// folder there counted as one page; any other src/gui file is its own entry),
// how many CONTROLS have no plain-language hint. The count lives in
// `tests/hint_budget.txt` and only goes down, like the words above.
//
// A CONTROL is a call that draws something a person can press, type into or
// drag: `ui.button(..)`, `Button::primary(..)`, `ui.checkbox(..)`,
// `TextEdit::singleline(..)`, `Slider::new(..)`, `ComboBox::from_id_salt(..)`,
// the project helpers (`toggle`, `labeled_slider`, `compact_button`, ...), and
// any area made clickable with `Sense::click()`.
//
// It HAS A HINT when its statement (closure bodies and if/else bodies aside)
// calls `.hint(..)` / `.hint_dyn(..)`, `with_hint(..)`, `.tooltip(..)`, or one
// of egui's `.on_hover_text(..)` family; or when it is bound with `let r =`
// and `r.on_hover_text(..)` / `r.hint(..)` follows in the same block; or when
// the very next statement is `setting_hint(..)`, the Settings page's own
// description line (which the hint modes show inline, behind a (?), or not at
// all). `// hint-exempt: <reason>` on the control's line skips it: a helper's
// inner box whose caller's `with_hint` reaches it, a control that is its own
// explanation.

const HINT_BUDGET_FILE: &str = "tests/hint_budget.txt";
const HINT_EXEMPT: &str = "hint-exempt";

const METHOD_CONTROLS: &[&str] = &[
    "button", "small_button", "checkbox", "radio", "radio_value", "selectable_label", "selectable_value",
    "toggle_value", "link", "hyperlink", "hyperlink_to", "text_edit_singleline", "text_edit_multiline",
    "menu_button", "color_edit_button_srgba", "color_edit_button_rgba_unmultiplied", "color_edit_button_srgb",
    "drag_angle", "image_button",
];
const QUALIFIED_CONTROLS: &[(&str, &str)] = &[
    ("Button", "new"), ("Button", "primary"), ("Button", "secondary"), ("Button", "danger"), ("Button", "success"),
    ("Button", "ghost"), ("Button", "tab"), ("Button", "icon_only"), ("Button", "image_and_text"), ("Slider", "new"),
    ("DragValue", "new"), ("TextEdit", "singleline"), ("TextEdit", "multiline"), ("ComboBox", "from_id_salt"),
    ("ComboBox", "from_label"), ("ComboBox", "from_id_source"), ("Checkbox", "new"), ("SelectableLabel", "new"),
    ("RadioButton", "new"), ("Hyperlink", "new"), ("Hyperlink", "from_label_and_url"), ("ImageButton", "new"),
];
const FN_CONTROLS: &[&str] = &[
    "primary_button", "secondary_button", "danger_button", "btn_primary", "btn_secondary", "btn_danger", "btn_icon",
    "compact_button", "toggle", "custom_checkbox", "labeled_slider", "labeled_slider_entry", "custom_slider",
    "custom_slider_capped", "search_bar", "stepper_button", "hold_to_confirm", "hold_to_confirm_at", "card_clickable",
];
const HINT_CALLS: &[&str] = &[
    "hint", "hint_dyn", "with_hint", "tooltip", "tooltip_above", "on_hover_text", "on_hover_ui",
    "on_hover_text_at_pointer", "on_hover_ui_at_pointer", "on_disabled_hover_text", "on_disabled_hover_ui",
];

fn ident(t: &Token) -> Option<&str> {
    match &t.tok {
        Tok::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}
fn punct(t: &Token, c: char) -> bool {
    t.tok == Tok::Punct(c)
}

/// The token indexes where a control is drawn.
fn controls(toks: &[Token], mask: &[bool]) -> Vec<usize> {
    let mut out = Vec::new();
    for i in 0..toks.len() {
        if mask[i] || i + 1 >= toks.len() {
            continue;
        }
        let Some(name) = ident(&toks[i]) else { continue };
        let prev_dot = i >= 1 && punct(&toks[i - 1], '.');
        let prev_fn = i >= 1 && ident(&toks[i - 1]) == Some("fn");
        let qual = if i >= 3 && punct(&toks[i - 1], ':') && punct(&toks[i - 2], ':') { ident(&toks[i - 3]) } else { None };
        let opens = punct(&toks[i + 1], '(');
        if prev_fn {
            continue;
        }
        // Sense::click / Sense::click_and_drag: a hand-made clickable area.
        if qual == Some("Sense") && (name == "click" || name == "click_and_drag") {
            out.push(i);
            continue;
        }
        if !opens {
            continue;
        }
        if prev_dot && METHOD_CONTROLS.contains(&name) {
            out.push(i);
        } else if let Some(q) = qual {
            if QUALIFIED_CONTROLS.iter().any(|(a, b)| *a == q && *b == name) {
                out.push(i);
            } else if FN_CONTROLS.contains(&name) && (q == "widgets" || q == "button" || q == "super" || q == "crate") {
                out.push(i);
            }
        } else if !prev_dot && FN_CONTROLS.contains(&name) {
            out.push(i);
        }
    }
    out
}

/// The innermost `{` enclosing token i (with every bracket between closed), or None at file level.
fn enclosing_block(toks: &[Token], i: usize) -> Option<usize> {
    let mut depth = 0i32;
    for k in (0..i).rev() {
        match toks[k].tok {
            Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => depth += 1,
            // An unclosed paren around i: i is inside its arguments; keep walking out.
            Tok::Punct('(') | Tok::Punct('[') => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            Tok::Punct('{') => {
                if depth == 0 {
                    return Some(k);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// The matching close of the bracket opened at `open`.
fn matching(toks: &[Token], open: usize) -> usize {
    let mut depth = 0i32;
    for k in open..toks.len() {
        match toks[k].tok {
            Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => depth += 1,
            Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => {
                depth -= 1;
                if depth == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    toks.len().saturating_sub(1)
}

/// The statement holding token i, inside its enclosing block: [start, end),
/// split at the block's top-level `;` and `,` and after a top-level `}` that
/// closes an if/else/match body.
fn statement(toks: &[Token], i: usize) -> (usize, usize) {
    let block = enclosing_block(toks, i);
    let lo = block.map(|b| b + 1).unwrap_or(0);
    let hi = block.map(|b| matching(toks, b)).unwrap_or(toks.len());
    // Walk the block's top level, tracking where each statement starts.
    let mut start = lo;
    let mut k = lo;
    while k < hi {
        match toks[k].tok {
            Tok::Punct('(') | Tok::Punct('[') => {
                let m = matching(toks, k);
                if i > k && i < m {
                    // i is inside these parens: its statement continues past them.
                }
                k = m + 1;
                continue;
            }
            Tok::Punct('{') => {
                let m = matching(toks, k);
                let closure = k > 0 && punct(&toks[k - 1], '|');
                if i > k && i < m {
                    // i is inside a nested block: not possible (block is innermost), unless a closure arg.
                    k = m + 1;
                    continue;
                }
                k = m + 1;
                // A block body that ends a statement without `;` (if, match, for, while).
                if !closure && i < k && i >= start {
                    // i was in this statement's head; the statement ends after the body.
                    // Keep going only through `else` chains.
                    if k < hi && ident(&toks[k]) == Some("else") {
                        continue;
                    }
                    return (start, k);
                }
                if !closure && i >= k {
                    // A finished block statement before i, unless an `else` follows.
                    if !(k < hi && ident(&toks[k]) == Some("else")) && !(k < hi && punct(&toks[k], '.')) {
                        start = k;
                    }
                }
                continue;
            }
            Tok::Punct(';') | Tok::Punct(',') => {
                if i < k && i >= start {
                    return (start, k);
                }
                start = k + 1;
            }
            _ => {}
        }
        k += 1;
    }
    (start, hi)
}

/// Does [a, b) call a hint, skipping closure and block bodies that do not hold `skip_holder`?
fn has_hint_call(toks: &[Token], a: usize, b: usize, holder: usize) -> bool {
    let mut k = a;
    while k < b {
        if punct(&toks[k], '{') {
            let m = matching(toks, k);
            if !(holder > k && holder < m) {
                k = m + 1;
                continue;
            }
        }
        if let Some(name) = ident(&toks[k]) {
            if HINT_CALLS.contains(&name) && k + 1 < toks.len() && punct(&toks[k + 1], '(') {
                return true;
            }
        }
        k += 1;
    }
    false
}

/// Is the control at i given a hint?
fn hinted(toks: &[Token], i: usize) -> bool {
    let (s, e) = statement(toks, i);
    if has_hint_call(toks, s, e, i) {
        return true;
    }
    // Inside a closure handed to `with_hint(ui, key, |ui| { ... })`: the scope's
    // hint reaches every project control drawn in it.
    {
        let mut k = i;
        while let Some(b) = enclosing_block(toks, k) {
            if b > 0 && punct(&toks[b - 1], '|') {
                // The call whose arguments hold this closure.
                let mut depth = 0i32;
                let mut j = b;
                while j > 0 {
                    j -= 1;
                    match toks[j].tok {
                        Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => depth += 1,
                        Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => {
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                }
                if punct(&toks[j], '(') && j > 0 && ident(&toks[j - 1]) == Some("with_hint") {
                    return true;
                }
            }
            k = b;
        }
    }
    // `let r = <control>; ... r.on_hover_text(..)` later in the same block, also
    // through `r.response.hint(..)` (a ComboBox's InnerResponse) and a tuple
    // binding (`let (rect, resp) = ui.allocate_exact_size(.., Sense::click());`).
    let mut names: Vec<&str> = Vec::new();
    if ident(&toks[s]) == Some("let") {
        let n = if ident(&toks[s + 1]) == Some("mut") { s + 2 } else { s + 1 };
        if punct(&toks[n], '(') {
            let close = matching(toks, n);
            for t in &toks[n + 1..close] {
                if let Some(id) = ident(t) {
                    if id != "mut" {
                        names.push(id);
                    }
                }
            }
        } else if let Some(id) = ident(&toks[n]) {
            names.push(id);
        }
    }
    let block_end = enclosing_block(toks, i).map(|b| matching(toks, b)).unwrap_or(toks.len());
    for name in names {
        let mut k = e;
        while k + 2 < block_end {
            if ident(&toks[k]) == Some(name) && punct(&toks[k + 1], '.') {
                // r.hint(..), r.on_hover_text(..), r.clone().on_hover_text(..), r.response.hint(..)
                let mut j = k + 2;
                if ident(&toks[j]) == Some("clone") && j + 3 < block_end && punct(&toks[j + 3], '.') {
                    j += 4;
                } else if ident(&toks[j]) == Some("response") && j + 1 < block_end && punct(&toks[j + 1], '.') {
                    j += 2;
                }
                if ident(&toks[j]).is_some_and(|m| HINT_CALLS.contains(&m)) {
                    return true;
                }
            }
            k += 1;
        }
    }
    // The next statement is the Settings page's description line.
    let mut n = e;
    while n < block_end && (punct(&toks[n], ';') || punct(&toks[n], ',')) {
        n += 1;
    }
    let mut m = n;
    while m < block_end && m < n + 6 {
        if ident(&toks[m]) == Some("setting_hint") {
            return true;
        }
        if punct(&toks[m], '(') {
            break;
        }
        m += 1;
    }
    false
}

/// (controls, without a hint) in a source file, with the unhinted ones' lines.
fn hint_coverage(src: &str) -> (usize, Vec<usize>) {
    let toks = tokenize(src);
    let mask = test_mask(&toks);
    let lines: Vec<&str> = src.lines().collect();
    let mut total = 0;
    let mut missing = Vec::new();
    for i in controls(&toks, &mask) {
        let line = toks[i].line;
        if lines.get(line - 1).is_some_and(|l| l.contains(HINT_EXEMPT)) {
            continue;
        }
        total += 1;
        if !hinted(&toks, i) {
            missing.push(line);
        }
    }
    (total, missing)
}

/// The page a UI file belongs to.
fn page_of(rel: &str) -> String {
    if let Some(rest) = rel.strip_prefix("src/gui/pages/") {
        let first = rest.split('/').next().unwrap_or(rest);
        return format!("page {}", first.trim_end_matches(".rs"));
    }
    rel.to_string()
}

fn hint_counts() -> BTreeMap<String, (usize, usize)> {
    let mut m: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for p in ui_files() {
        let (total, missing) = hint_coverage(&read(&p));
        if total == 0 {
            continue;
        }
        let e = m.entry(page_of(&rel(&p))).or_insert((0, 0));
        e.0 += total;
        e.1 += missing.len();
    }
    m
}

fn read_hint_budget() -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    let Ok(text) = fs::read_to_string(repo().join(HINT_BUDGET_FILE)) else { return m };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((n, page)) = line.split_once(' ') {
            if let Ok(n) = n.trim().parse::<usize>() {
                m.insert(page.trim().to_string(), n);
            }
        }
    }
    m
}

fn write_hint_budget(m: &BTreeMap<String, usize>, counts: &BTreeMap<String, (usize, usize)>) {
    let total_controls: usize = counts.values().map(|c| c.0).sum();
    let missing: usize = m.values().sum();
    let mut s = String::new();
    s.push_str("# The native HINT ratchet's budgets: per page (a file or folder under src/gui/pages/, or\n");
    s.push_str("# another src/gui file), how many controls still have no plain-language hint. Checked by\n");
    s.push_str("# tests/i18n_lint.rs (native_hint_coverage_only_grows); the numbers only go DOWN and a page\n");
    s.push_str("# not listed has a budget of zero. HINT_REPORT=1 prints controls and coverage per page.\n");
    s.push_str(&format!("# Controls without a hint: {missing} of {total_controls}\n"));
    for (page, n) in m {
        if *n > 0 {
            s.push_str(&format!("{n} {page}\n"));
        }
    }
    fs::write(repo().join(HINT_BUDGET_FILE), s).expect("write the hint budget");
}

#[test]
fn native_hint_coverage_only_grows() {
    let now = hint_counts();
    if std::env::var("HINT_REPORT").is_ok() {
        for (page, (total, missing)) in &now {
            println!("{:>4} of {:>4} controls hinted  {page}", total - missing, total);
        }
        let t: usize = now.values().map(|c| c.0).sum();
        let m: usize = now.values().map(|c| c.1).sum();
        println!("TOTAL {} of {} controls hinted", t - m, t);
    }
    if let Ok(path) = std::env::var("HINT_DUMP") {
        let (_, missing) = hint_coverage(&read(&repo().join(&path)));
        for line in missing {
            println!("{path}:{line}: a control with no hint");
        }
    }
    let budget = read_hint_budget();
    let mode = std::env::var("I18N_RATCHET_WRITE").unwrap_or_default();
    let missing_now: BTreeMap<String, usize> = now.iter().map(|(p, c)| (p.clone(), c.1)).collect();
    if mode == "init" {
        write_hint_budget(&missing_now, &now);
        return;
    }
    let mut over = Vec::new();
    let mut under = Vec::new();
    for (page, n) in &missing_now {
        let b = budget.get(page).copied().unwrap_or(0);
        if *n > b {
            over.push(format!("  {page}: {n} controls without a hint, budget {b} (+{})", n - b));
        } else if *n < b {
            under.push((page.clone(), *n, b));
        }
    }
    let gone: Vec<String> = budget.keys().filter(|p| !missing_now.contains_key(*p)).cloned().collect();
    if mode == "1" {
        let mut lowered = budget.clone();
        for (p, n, _) in &under {
            lowered.insert(p.clone(), *n);
        }
        for p in &gone {
            lowered.remove(p);
        }
        write_hint_budget(&lowered, &now);
        under.clear();
    }
    assert!(
        over.is_empty(),
        "\n\nCONTROLS WITHOUT A PLAIN-LANGUAGE HINT, past the page's budget:\n{}\n\n\
         Give each one a hint by key: `.hint(\"page.thing_hint\")` on its Response or on a \
         widgets::Button, or `with_hint(ui, \"page.thing_hint\", |ui| widgets::toggle(..))` for a \
         helper that returns bool. Write the words in data/i18n/en.json (what it does, for a person \
         new to computers). HINT_DUMP=<file> lists the lines. Do not raise the budget.\n",
        over.join("\n")
    );
    let msg: Vec<String> = under
        .iter()
        .map(|(p, n, b)| format!("  {p}: {n} without a hint, budget {b}: lower it to {n}"))
        .chain(if mode == "1" { Vec::new() } else { gone.iter().map(|p| format!("  {p}: no controls left; remove its line")).collect() })
        .collect();
    assert!(
        msg.is_empty(),
        "\n\nRATCHET CLICK (hints added; lock the win in):\n{}\nRun: I18N_RATCHET_WRITE=1 with this test.\n",
        msg.join("\n")
    );
}

#[test]
fn the_hint_scanner_finds_controls_and_their_hints() {
    let src = r#"
fn draw(ui: &mut Ui) {
    if ui.button(t("a")).clicked() { go(); }                      // 1 no hint
    if ui.button(t("b")).hint("x.b_hint").clicked() { go(); }     // 2 hint
    let r = ui.add(egui::TextEdit::singleline(&mut s));           // 3 hint, later
    if r.changed() { go(); }
    r.on_hover_text("words");
    if Button::primary(t("c")).hint("x.c_hint").show(ui, theme) {} // 4 hint
    if widgets::toggle(ui, theme, t("d"), &mut v) { dirty(); }    // 5 description next
    widgets::setting_hint(ui, theme, hint, t("x.d_desc"));
    with_hint(ui, "x.e_hint", |ui| widgets::labeled_slider(ui, theme, t("e"), &mut f, 0.0..=1.0)); // 6 hint
    egui::ComboBox::from_id_salt("k").show_ui(ui, |ui| {          // 7 no hint (the combo)
        ui.selectable_value(&mut v, 1, t("f")).hint("x.f_hint");  // 8 hint
    });
    let (rect, resp) = ui.allocate_exact_size(sz, Sense::click()); // 9 no hint
    ui.checkbox(&mut b, t("g")); // hint-exempt: test of the escape hatch
}
fn widget_def() {}
pub fn toggle(ui: &mut Ui) {}
"#;
    let (total, missing) = hint_coverage(src);
    let lines: Vec<usize> = missing;
    assert_eq!(total, 9, "nine controls (the exempt one skipped, fn definitions not counted)");
    assert_eq!(lines, vec![3, 12, 15], "unhinted: the bare button, the combo, the click area");

    // Three ways found by the page conversions (2026-10-23) that ARE hinted.
    let more = r#"
fn draw(ui: &mut Ui) {
    let r = egui::ComboBox::from_id_salt("k").show_ui(ui, |ui| {});
    r.response.hint("x.combo_hint");
    let (rect, resp) = ui.allocate_exact_size(sz, Sense::click());
    let resp = resp.hint("x.cell_hint");
    with_hint(ui, "x.t_hint", |ui| {
        widgets::toggle(ui, theme, t("x.t"), &mut v);
    });
    let (_r2, bare) = ui.allocate_exact_size(sz, Sense::click());
}
"#;
    let (total, missing) = hint_coverage(more);
    assert_eq!(total, 4);
    assert_eq!(missing, vec![10], "only the last click area has no hint");
}

// ── The scanner's own checks (each must be able to fail) ────────────────────

#[test]
fn the_scanner_counts_words_in_ui_calls_and_nothing_else() {
    let src = r##"
fn draw(ui: &mut Ui) {
    ui.label("Hello there");                         // counts
    ui.label(RichText::new("Bold words").strong());  // counts once
    ui.label(format!("{} items", n));                // counts
    ui.label(format!("{}", n));                      // no words
    ui.label(t("calc.title"));                       // a key, not words
    if Button::primary("Send").show(ui, theme) {}    // counts
    let r = ui.button("Go").on_hover_text("Starts the thing"); // two
    ui.push_id("some_id", |ui| {});                  // an id
    ui.collapsing("Section", |ui| { log::info!("not ui"); }); // one
    ui.label(if on { "On" } else { "Off" });         // two (expression block)
    ui.hyperlink_to("Read more", "https://example.org"); // one (the URL is not words)
    ui.label("HumanityOS"); // i18n-exempt: the name
    egui::Id::new("Not words either");
    log::warn!("Not ui at all");
}
#[cfg(test)]
mod tests {
    fn t() { ui.label("In a test"); }
}
"##;
        let found: Vec<String> = ui_literals(src).into_iter().map(|f| f.text).collect();
        assert_eq!(
            found,
            vec![
                "Hello there", "Bold words", "{} items", "Send", "Go", "Starts the thing", "Section", "On", "Off",
                "Read more"
            ],
            "the scanner found {found:?}"
        );
    }

#[test]
fn the_key_scanner_reads_first_arguments_only() {
    let src = r#"
        ui.label(t("a.b"));
        let s = tf("c.d", &[("n", &n)]);
        let p = tn("e.f", 3, &[]);
        resp.hint("g.h");
        let x = other.t("not.a.key");
        hint("not.a.key.either");
        resp.hint("i.j_hint");
        fn key_for(c: &str) -> &'static str { match c { "C" => "calc.clear_hint", _ => "calc.digit_hint" } }
        let id = "Some_Hint";
    "#;
    let keys: Vec<(String, String)> = keys_asked_for(src).into_iter().map(|(_, c, k)| (c, k)).collect();
    assert_eq!(
        keys,
        vec![
            ("t".into(), "a.b".into()),
            ("tf".into(), "c.d".into()),
            ("tn".into(), "e.f".into()),
            ("hint".into(), "g.h".into()),
            ("hint".into(), "i.j_hint".into()),
            ("hint key".into(), "calc.clear_hint".into()),
            ("hint key".into(), "calc.digit_hint".into())
        ]
    );
}

#[test]
fn the_json_reader_reads_nested_keys_and_escapes() {
    let v = parse_json(r#"{"a":{"b":"x\"y","c":["z"],"d":1},"e":"é"}"#).unwrap();
    let mut m = BTreeMap::new();
    flatten("", &v, &mut m);
    assert_eq!(m.get("a.b").map(|s| s.as_str()), Some("x\"y"));
    assert_eq!(m.get("e").map(|s| s.as_str()), Some("\u{e9}"));
    assert!(!m.contains_key("a.d"));
    assert!(parse_json("{\"a\":}").is_err());
}

//! Em-dash ban enforcement (v0.364.0).
//!
//! Operator rule (2026-06-04, emphatic; REVISED 2026-09-18): em-dashes
//! ("\u{2014}", U+2014) are rare in user-facing copy, not forbidden. "People
//! see em dashes and immediately leave": they read as machine-written and cost
//! trust, and AI writing is saturated with them. Default to a comma, a colon, a
//! full stop or a rewrite, which is usually better prose anyway.
//!
//! WHY THIS IS NO LONGER A FLAT BAN: a flat ban corrupts QUOTED text. It bit on
//! the US Constitution, which is full of them and must be reproduced exactly.
//! Never rewrite punctuation inside a quotation, a statute, a licence or a
//! person's own words to satisfy a style rule; that is falsifying the quote.
//! Use the escape hatch below for those, and keep ordinary prose clear of them.
//!
//! This test is the rule's teeth for the native UI. It scans every string
//! literal under `src/gui/` for U+2014 and FAILS the build on any occurrence.
//! Code COMMENTS are intentionally exempt (they are never rendered to users);
//! hundreds of explanatory comments legitimately use em-dashes and rewriting
//! them would add churn with zero user benefit.
//!
//! Run: cargo test --test emdash_lint
//!
//! Escape hatch: put `// emdash-exempt: <reason>` on the offending line. Use it
//! only when a string genuinely must contain U+2014 (none should in GUI copy).

use std::fs;
use std::path::{Path, PathBuf};

const EM_DASH: char = '\u{2014}';
const EXEMPT: &str = "emdash-exempt";

#[test]
fn no_emdash_in_gui_strings() {
    let root = project_root();
    let gui = root.join("src").join("gui");
    let mut violations: Vec<(PathBuf, usize, String)> = Vec::new();

    for file in walk_rs(&gui) {
        let content = match fs::read_to_string(&file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        for line_no in string_emdash_lines(&content) {
            let text = lines.get(line_no - 1).copied().unwrap_or("");
            if text.contains(EXEMPT) {
                continue;
            }
            violations.push((file.clone(), line_no, text.trim().to_string()));
        }
    }

    if !violations.is_empty() {
        let mut msg = format!(
            "\n\n[FAIL] Found {} em-dash(es) inside string literals in src/gui/.\n\
             \n\
             Operator rule: no em-dashes (\"\u{2014}\", U+2014) in user-facing copy. They\n\
             read as machine-written and cost trust. Replace with a comma, period,\n\
             parentheses, or a plain hyphen.\n\
             \n\
             (Code comments are exempt; this only flags rendered strings. If a\n\
             string truly must keep U+2014, annotate the line `// emdash-exempt: why`.)\n\
             \n\
             Offending lines:\n",
            violations.len()
        );
        for (file, line_no, text) in &violations {
            msg.push_str(&format!(
                "  {}:{}\n    {}\n",
                file.strip_prefix(&root).unwrap_or(file).display(),
                line_no,
                text,
            ));
        }
        panic!("{}", msg);
    }
}

/// Keys whose words must keep U+2014 because they quote someone exactly (a statute, a licence, a
/// person's own words), each with its reason. JSON has no comments, so the `emdash-exempt` line
/// hatch cannot reach the word files; name the leaf key here instead. Empty on 2026-11-02.
const EXEMPT_KEYS: &[(&str, &str)] = &[];

/// The words people read live in `data/i18n/*.json` since the one-word-source mandate
/// (2026-10-22): native reads them with `t("page.key")`, the website with `i18n.t()`. A scan of
/// `src/gui/` string literals alone stopped seeing most of the app's words, and on 2026-11-02 an em
/// dash put in `wip.mute_hint` passed this lint. This scans every word file, the literal character
/// and its JSON escape both.
#[test]
fn no_emdash_in_the_word_files() {
    let root = project_root();
    let dir = root.join("data").join("i18n");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("data/i18n holds the word files")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no word files found in {}", dir.display());
    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if !(line.contains(EM_DASH) || line.contains("\\u2014")) {
                continue;
            }
            let leaf = line.trim_start();
            if EXEMPT_KEYS.iter().any(|(key, _)| leaf.starts_with(&format!("\"{key}\""))) {
                continue;
            }
            violations.push(format!(
                "  {}:{}\n    {}",
                file.strip_prefix(&root).unwrap_or(file).display(),
                i + 1,
                line.trim()
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "\n\n[FAIL] Found {} em-dash(es) in the word files (data/i18n). Replace each with a comma, a\n\
         colon, a full stop or a rewrite; a quotation that must keep one names its key in\n\
         EXEMPT_KEYS with the reason.\n\nOffending lines:\n{}\n",
        violations.len(),
        violations.join("\n")
    );
}

/// Scan `content` and return the 1-based line numbers where a U+2014 appears
/// INSIDE a string literal (not in a `//` line comment or a `/* */` block
/// comment). A lightweight state machine, good enough for GUI label code (it
/// does not model raw strings or char literals, which never carry user-facing
/// em-dashes here; the `emdash-exempt` hatch covers any edge case).
fn string_emdash_lines(content: &str) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut escaped = false;
    let mut line = 1usize;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\n' {
            line += 1;
            in_line_comment = false;
            continue;
        }
        if in_line_comment {
            continue;
        }
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            } else if c == EM_DASH && out.last() != Some(&line) {
                out.push(line);
            }
            continue;
        }
        // Outside any string / comment.
        match c {
            '"' => in_string = true,
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                in_line_comment = true;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                in_block_comment = true;
            }
            _ => {}
        }
    }
    out
}

fn walk_rs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_rs(&path));
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
    out
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Proves the scanner is not a silent no-op: it must FLAG an em-dash inside a
/// string literal and IGNORE one inside a line or block comment.
#[test]
fn scanner_flags_strings_not_comments() {
    // Inside a string literal -> flagged (line 1).
    assert_eq!(string_emdash_lines("let x = \"a \u{2014} b\";"), vec![1]);
    // Inside a line comment -> ignored.
    assert!(string_emdash_lines("// a \u{2014} b").is_empty());
    // Inside a block comment -> ignored.
    assert!(string_emdash_lines("/* a \u{2014} b */").is_empty());
    // Mixed file: comment dash on line 2 ignored, string dash on line 3 flagged.
    let src = "fn f() {\n  // ok \u{2014} comment\n  let s = \"bad \u{2014} dash\";\n}";
    assert_eq!(string_emdash_lines(src), vec![3]);
}

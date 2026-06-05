//! Em-dash ban enforcement (v0.364.0).
//!
//! Operator rule (2026-06-04, emphatic): NO em-dashes ("\u{2014}", U+2014) in
//! any user-facing copy. "People see em dashes and immediately leave." They
//! read as machine-written and cost trust. This applies to every UI string the
//! app renders.
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

//! REPO ROOT lint (the crate split's rung 1, 2026-11-01; docs/design/crate-split.md).
//!
//! The Rust source names the repo root ONE way: `env!("HUMANITY_REPO_ROOT")`, which every member of
//! the workspace sets for its own code through its build script (the root package's build.rs and
//! crates/repo_root.rs, the one every crate under crates/ shares). Two older ways broke the day the
//! first files moved into crates/<name>/src/:
//!
//! 1. `env!("CARGO_MANIFEST_DIR")` meant the repo root only while there was one package at the
//!    root. In a workspace it is each member's own folder, so in crates/humanity-core it names
//!    crates/humanity-core and `join("data")` finds nothing. 566 of them in 255 files were changed.
//! 2. `include_str!("../data/x")` (and `include_bytes!`, `include!`) is relative to the FILE, so it
//!    names a different place, or nothing, once the file moves. 253 in 77 files were changed to
//!    `include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/x"))`. A path inside the file's own
//!    folder (`include_str!("pq_kat_vote_sig.hex")`) moves with it and stays.
//!
//! This lint fails on either in any `.rs` file under src/ or crates/*/src/, in code (a comment or a
//! string that mentions them is fine). tests/ and examples/ are the root package's own targets, whose
//! CARGO_MANIFEST_DIR is the repo root (and the std-only lints like this one are compiled by rustc
//! alone, with only CARGO_MANIFEST_DIR set): they keep it.
//!
//! The scanner is run on a synthetic source first (a positive control), and the walk must find the
//! crates' files as well as src/'s, so a walk that stopped seeing a folder cannot pass by accident.
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/repo_root_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file of the library code: under src/, and under each crates/<name>/src/.
fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map_or(false, |x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo().join("src"), &mut out);
    if let Ok(rd) = fs::read_dir(repo().join("crates")) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                walk(&e.path().join("src"), &mut out);
            }
        }
    }
    out.sort();
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(repo()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The source with comments blanked and the insides of string literals kept apart: (code, strings),
/// where `code` has every comment and every string's inside turned to spaces (newlines kept) and
/// `strings` is each literal's inside with the offset of its opening quote. Raw strings and nested
/// block comments are handled; a char literal's quote is not mistaken for a string.
fn lex(src: &str) -> (String, Vec<(usize, String)>) {
    let b: Vec<char> = src.chars().collect();
    let mut code: Vec<char> = b.clone();
    let mut strings = Vec::new();
    let n = b.len();
    let blank = |code: &mut Vec<char>, a: usize, z: usize| {
        for k in a..z.min(n) {
            if code[k] != '\n' && code[k] != '\r' {
                code[k] = ' ';
            }
        }
    };
    let mut i = 0;
    while i < n {
        let c = b[i];
        let d = if i + 1 < n { b[i + 1] } else { '\0' };
        if c == '/' && d == '/' {
            let mut j = i;
            while j < n && b[j] != '\n' {
                j += 1;
            }
            blank(&mut code, i, j);
            i = j;
            continue;
        }
        if c == '/' && d == '*' {
            let (mut depth, mut j) = (1, i + 2);
            while j < n && depth > 0 {
                if b[j] == '/' && j + 1 < n && b[j + 1] == '*' {
                    depth += 1;
                    j += 2;
                } else if b[j] == '*' && j + 1 < n && b[j + 1] == '/' {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            blank(&mut code, i, j);
            i = j;
            continue;
        }
        let ident_before = i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_');
        if (c == 'r' || (c == 'b' && d == 'r')) && !ident_before {
            let mut j = if c == 'b' { i + 2 } else { i + 1 };
            let mut hashes = 0;
            while j < n && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && b[j] == '"' {
                let open = j;
                let mut k = j + 1;
                'scan: while k < n {
                    if b[k] == '"' && (0..hashes).all(|h| k + 1 + h < n && b[k + 1 + h] == '#') {
                        break 'scan;
                    }
                    k += 1;
                }
                strings.push((open, b[open + 1..k.min(n)].iter().collect()));
                blank(&mut code, open + 1, k);
                i = k + 1 + hashes;
                continue;
            }
        }
        if c == '"' {
            let mut j = i + 1;
            while j < n && b[j] != '"' {
                if b[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            strings.push((i, b[i + 1..j.min(n)].iter().collect()));
            blank(&mut code, i + 1, j);
            i = j + 1;
            continue;
        }
        if c == '\'' {
            // 'x', '\n', '\'' and '\u{..}' are char literals; 'a (a lifetime) is not.
            if i + 2 < n && b[i + 1] != '\\' && b[i + 2] == '\'' {
                blank(&mut code, i + 1, i + 2);
                i += 3;
                continue;
            }
            if i + 1 < n && b[i + 1] == '\\' {
                let mut j = i + 2;
                while j < n && j < i + 12 && b[j] != '\'' {
                    j += 1;
                }
                if j < n && b[j] == '\'' {
                    blank(&mut code, i + 1, j);
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    (code.into_iter().collect(), strings)
}

/// The problems in one file's source: (line, what), one per offending use in code.
fn problems_in(src: &str) -> Vec<(usize, String)> {
    let (code, strings) = lex(src);
    let chars: Vec<char> = code.chars().collect();
    let line_of = |at: usize| chars[..at.min(chars.len())].iter().filter(|&&c| c == '\n').count() + 1;
    let mut out = Vec::new();
    for (open, text) in &strings {
        // What the code just before the literal's opening quote says (skipping spaces): the macro
        // whose argument this string is.
        let mut k = *open;
        while k > 0 && chars[k - 1].is_whitespace() {
            k -= 1;
        }
        // Spaces dropped, so `include_bytes! ( "../x" )` reads as `include_bytes!(`.
        let before: String = chars[k.saturating_sub(32)..k].iter().filter(|c| !c.is_whitespace()).collect();
        if text == "CARGO_MANIFEST_DIR" && (before.ends_with("env!(") || before.ends_with("option_env!(")) {
            out.push((line_of(*open), "env!(\"CARGO_MANIFEST_DIR\"): name the repo root as env!(\"HUMANITY_REPO_ROOT\")".to_string()));
        }
        if text.starts_with("../")
            && (before.ends_with("include_str!(") || before.ends_with("include_bytes!(") || before.ends_with("include!("))
        {
            out.push((
                line_of(*open),
                format!("include path \"{text}\" is relative to the file and breaks when it moves: use concat!(env!(\"HUMANITY_REPO_ROOT\"), \"/<path from the repo root>\")"),
            ));
        }
    }
    out
}

#[test]
fn the_scanner_catches_what_it_is_for() {
    let src = "const A: &str = include_str!(\"../data/a.ron\");\n\
               let r = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"));\n\
               // env!(\"CARGO_MANIFEST_DIR\") and include_str!(\"../x\") in a comment are prose\n\
               let s = \"env!(CARGO_MANIFEST_DIR) in a string\";\n\
               const B: &[u8] = include_bytes! ( \"../../assets/b.png\" );\n\
               const C: &str = include_str!(\"same_folder.hex\");\n\
               const D: &str = include_str!(concat!(env!(\"HUMANITY_REPO_ROOT\"), \"/data/d.ron\"));\n\
               let t = 'x'; let u = '\\''; fn f<'a>(x: &'a str) {}\n\
               include!(\"../gen.rs\");\n";
    let got: Vec<usize> = problems_in(src).iter().map(|p| p.0).collect();
    assert_eq!(got, vec![1, 2, 5, 9], "got {:?}", problems_in(src));
}

#[test]
fn the_source_names_the_repo_root_one_way() {
    let files = rust_sources();
    // The walk must see both halves of the library code: a walk that stopped reading src/ or
    // crates/ would pass on nothing.
    let in_src = files.iter().filter(|p| rel(p).starts_with("src/")).count();
    let in_crates = files.iter().filter(|p| rel(p).starts_with("crates/")).count();
    assert!(in_src > 500, "the walk found only {in_src} files under src/");
    assert!(in_crates > 50, "the walk found only {in_crates} files under crates/*/src/");
    let mut problems = Vec::new();
    for p in &files {
        let src = fs::read_to_string(p).unwrap_or_default();
        // Most files name neither; lexing only the rest keeps the lint under a second.
        if !src.contains("CARGO_MANIFEST_DIR") && !src.contains("\"../") {
            continue;
        }
        for (line, what) in problems_in(&src) {
            problems.push(format!("  {}:{line}: {what}", rel(p)));
        }
    }
    assert!(
        problems.is_empty(),
        "\n\nThe repo root named a way that breaks in the workspace ({} places):\n{}\n\n\
         env!(\"HUMANITY_REPO_ROOT\") is the repo root in every member (each one's build script sets it); \
         CARGO_MANIFEST_DIR is the member's own folder, and a \"../\" include path is relative to its file. \
         docs/design/crate-split.md, rung 1.\n",
        problems.len(),
        problems.join("\n")
    );
}

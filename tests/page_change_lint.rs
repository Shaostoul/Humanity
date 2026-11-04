//! EVERY CHANGE OF PAGE GOES THROUGH THE HISTORY (navigation rung 4, 2026-11-04;
//! docs/design/navigation.md section 10.2): "One function moves between places
//! (`GuiState::go(place, how)`) ... a lint then allows `active_page` to be written only
//! there (and in the world's own entry and exit), so a new page change cannot skip history."
//!
//! So `active_page` is assigned in exactly one file, `src/gui/history.rs`, which holds `go`,
//! `go_to`, Back and Forward, and the world's entry and exit. Anywhere else a page change is
//! `state.go(page, How::Beside)` (a menu click, a link to another page), `How::From` (a page
//! opened from inside another, which Escape backs out of) or `How::Replace` (a place the app
//! opens by itself), or `enter_world()` / `leave_world()` / `show_title()`.
//!
//! TEST CODE IS LEFT ALONE: a test sets the page it starts on. A file declared under
//! `#[cfg(test)]` (with or without a `#[path]`) is not read, and an inline `#[cfg(test)] mod`
//! ends the part of its file that is read (they sit at the end of their files).
//!
//! RED FIRST (2026-11-04): run before the writes moved, this lint named 40 assignments in 18
//! files (src/lib.rs, src/engine/ipc.rs, src/gui/pages/main_menu.rs, ...); the
//! `the_lint_catches_a_write_and_skips_tests` case below fails if the matcher stops seeing
//! `g.active_page = GuiPage::Chat;` or starts reading a test module.
//!
//! Std-only, compiled standalone like the other lints (`just lints`):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/page_change_lint.rs

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The one file that may assign `active_page`.
const ALLOWED: &[&str] = &["src/gui/history.rs"];

/// Every `.rs` file under `dir`.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

/// The files `src` (at `file`) declares as test-only modules: `#[cfg(test)]` (or
/// `#[cfg(all(test, ...))]`), any other attributes, then `mod name;`.
fn test_children(file: &Path, src: &str) -> Vec<PathBuf> {
    let dir = file.parent().unwrap_or(Path::new(""));
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let own_dir = if matches!(stem, "mod" | "lib" | "main") { dir.to_path_buf() } else { dir.join(stem) };
    let lines: Vec<&str> = src.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if !(l.starts_with("#[cfg(test)]") || l.starts_with("#[cfg(all(test")) {
            continue;
        }
        let mut path_attr: Option<String> = None;
        for next in lines.iter().skip(i + 1).take(4) {
            if let Some(rest) = next.strip_prefix("#[path = \"") {
                path_attr = rest.split('"').next().map(str::to_string);
                continue;
            }
            if next.starts_with("#[") {
                continue;
            }
            let decl = next.trim_start_matches("pub(crate) ").trim_start_matches("pub(super) ").trim_start_matches("pub ");
            if let Some(name) = decl.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
                match &path_attr {
                    Some(p) => out.push(dir.join(p)),
                    None => {
                        out.push(own_dir.join(format!("{name}.rs")));
                        out.push(own_dir.join(name).join("mod.rs"));
                    }
                }
            }
            break;
        }
    }
    out
}

/// The byte just past the `}` that closes the `{` at `open`, skipping braces inside comments,
/// strings (raw ones too) and char literals; the end of `src` when it never closes.
fn block_end(src: &str, open: usize) -> usize {
    let b = src.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            b'r' if matches!(b.get(i + 1), Some(b'#') | Some(b'"')) && !b[..i].last().is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') => {
                let mut j = i + 1;
                let mut hashes = 0;
                while b.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if b.get(j) == Some(&b'"') {
                    j += 1;
                    let close: Vec<u8> = std::iter::once(b'"').chain(std::iter::repeat(b'#').take(hashes)).collect();
                    while j < b.len() && !b[j..].starts_with(&close) {
                        j += 1;
                    }
                    i = j + close.len();
                    continue;
                }
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'\'' => {
                // A char literal ('x', '\n', '\u{..}') is skipped; a lifetime ('a) is not one.
                if b.get(i + 1) == Some(&b'\\') {
                    i += 2;
                    while i < b.len() && b[i] != b'\'' {
                        i += 1;
                    }
                } else if b.get(i + 2) == Some(&b'\'') {
                    i += 2;
                } else if let Some(c) = src[i + 1..].chars().next() {
                    if src[i + 1 + c.len_utf8()..].starts_with('\'') {
                        i += c.len_utf8() + 1;
                    }
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// `src` with every inline test module (`#[cfg(test)]`, other attributes, `mod name {`) blanked
/// to its newlines, so the lines left keep their numbers.
fn production_part(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        let line_end = rest.find('\n').map_or(src.len(), |n| i + n + 1);
        let t = src[i..line_end].trim();
        if t.starts_with("#[cfg(test)]") || t.starts_with("#[cfg(all(test") {
            // The item the attribute is on: past any further attributes.
            let mut j = line_end;
            while j < src.len() {
                let e = src[j..].find('\n').map_or(src.len(), |n| j + n + 1);
                let l = src[j..e].trim();
                if l.starts_with("#[") || l.is_empty() {
                    j = e;
                    continue;
                }
                let decl = l.trim_start_matches("pub(crate) ").trim_start_matches("pub(super) ").trim_start_matches("pub ");
                if decl.starts_with("mod ") && decl.ends_with('{') {
                    let open = j + src[j..e].find('{').unwrap_or(0);
                    let end = block_end(src, open);
                    out.extend(src[i..end].chars().filter(|c| *c == '\n'));
                    i = end;
                }
                break;
            }
            if i < line_end && i != line_end {
                out.push_str(&src[i..line_end]);
                i = line_end;
            }
            continue;
        }
        out.push_str(&src[i..line_end]);
        i = line_end;
    }
    out
}

/// Every line of `src` that assigns `active_page` (`x.active_page = ...`, not `==`), comments
/// left out: (line number, the line).
fn writes(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, line) in production_part(src).lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        let mut rest = code;
        while let Some(i) = rest.find("active_page") {
            let after = rest[i + "active_page".len()..].trim_start();
            let named_alone = !rest[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_');
            if named_alone && after.starts_with('=') && !after.starts_with("==") && !after.starts_with("=>") {
                out.push((n + 1, line.trim().to_string()));
                break;
            }
            rest = &rest[i + "active_page".len()..];
        }
    }
    out
}

#[test]
fn active_page_is_written_only_by_the_history() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    if let Ok(crates) = fs::read_dir(repo.join("crates")) {
        for c in crates.flatten() {
            rust_files(&c.path().join("src"), &mut files);
        }
    }
    let mut tests: HashSet<PathBuf> = HashSet::new();
    for f in &files {
        if let Ok(src) = fs::read_to_string(f) {
            tests.extend(test_children(f, &src));
        }
    }
    let mut bad = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(repo).unwrap_or(f).to_string_lossy().replace('\\', "/");
        // A test module's own submodules (a folder of tests, src/engine/crossing_tests/) are tests.
        let in_test_dir = tests.iter().any(|t| {
            let dir = if t.file_name().and_then(|n| n.to_str()) == Some("mod.rs") { t.parent().map(Path::to_path_buf) } else { Some(t.with_extension("")) };
            dir.is_some_and(|d| f.starts_with(&d))
        });
        if ALLOWED.contains(&rel.as_str()) || tests.contains(f) || in_test_dir {
            continue;
        }
        let Ok(src) = fs::read_to_string(f) else { continue };
        for (n, line) in writes(&src) {
            bad.push(format!("{rel}:{n}: {line}"));
        }
    }
    assert!(
        bad.is_empty(),
        "\n\n`active_page` is assigned outside src/gui/history.rs ({} places):\n  {}\n\n\
         A page change goes through the history so Back, Forward and Escape know about it\n\
         (docs/design/navigation.md section 10.2): `state.go(page, How::Beside)` for a menu click\n\
         or a link to another page, `How::From` for a page opened from inside another (Escape\n\
         backs out of it), `How::Replace` for a place the app opens by itself; going into the\n\
         world is `enter_world()`, coming out `leave_world()`, the title screen `show_title()`.\n",
        bad.len(),
        bad.join("\n  ")
    );
}

/// The matcher itself, on made-up sources: it sees a write in production code, not a
/// comparison, a comment or a longer name, and never reads a test module.
#[test]
fn the_lint_catches_a_write_and_skips_tests() {
    // A test module in the middle of the file, with a brace in a string and in a char: the
    // write after it is still read (main_menu.rs has its tests at line 222 of 900).
    let src = "fn f(g: &mut G) {\n    g.active_page = GuiPage::Chat;\n    if g.active_page == GuiPage::None {}\n    // g.active_page = x;\n    g.active_page_hint = 1;\n    let p = active_page; match p { _ => 0 };\n}\n#[cfg(test)]\nmod tests {\n    fn t(g: &mut G) { let s = \"}\"; let c = '}'; g.active_page = GuiPage::Files; }\n}\nfn after<'a>(g: &'a mut G) {\n    g.active_page = GuiPage::Maps;\n}\n";
    let found = writes(src);
    assert_eq!(found.iter().map(|w| w.0).collect::<Vec<_>>(), vec![2, 13], "the two writes in production code: {found:?}");
    let decl = "#[cfg(test)]\n#[path = \"x_tests.rs\"]\nmod tests;\n#[cfg(test)]\nmod ui_snapshots;\n";
    let kids = test_children(Path::new("src/gui/mod.rs"), decl);
    assert!(kids.contains(&PathBuf::from("src/gui/x_tests.rs")), "{kids:?}");
    assert!(kids.contains(&PathBuf::from("src/gui/ui_snapshots.rs")), "{kids:?}");
    let kids = test_children(Path::new("src/gui/pages/apps.rs"), "#[cfg(test)]\nmod tests;\n");
    assert!(kids.contains(&PathBuf::from("src/gui/pages/apps/tests.rs")), "{kids:?}");
}

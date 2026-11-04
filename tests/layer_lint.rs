//! EVERY SURFACE PICKS ITS LAYER FROM src/gui/layers.rs (navigation rung 3, 2026-11-04;
//! docs/design/navigation.md section 6, docs/design/ui-system.md "The layer rule"). The operator's
//! order (a popup's shade over both menus and the warning, an urgent card over the popup, the
//! notices and help panels under the shade) holds only if every floating thing is drawn on it, so
//! egui's `Area::new`, `Window::new` and `Order` are named in one file, `src/gui/layers.rs`, and
//! everywhere else a surface is `layers::area(ctx, id, Layer::...)`, `layers::window(ctx, id,
//! title, Layer::...)`, `layers::painter(ctx, id, Layer::...)`, or a full-screen popup's
//! `layers::shade` with its window seated on it (`layers::seat`).
//!
//! TEST CODE IS LEFT ALONE (a test reads egui's orders to check what is over what), the same way
//! `tests/page_change_lint.rs` leaves it: a file declared under `#[cfg(test)]` is not read, and an
//! inline `#[cfg(test)] mod` ends the part of its file that is read.
//!
//! OUTSIDE src/: `crates/humanity-core/src/debug.rs` draws the F12 debug console in egui's
//! Foreground, over everything, on purpose (a developer's console must never hide under what it is
//! inspecting), and a crate cannot see src/gui/layers.rs; so the lint reads src/ only.
//!
//! RED FIRST (2026-11-04): run before the surfaces moved (the dialog widget already on layers.rs),
//! it said "73 places in 28 files build an egui surface or name an egui Order outside
//! src/gui/layers.rs" (the notices panel's `.order(egui::Order::Foreground)`, the image viewer's
//! `egui::Window::new`, the incoming call's, the toasts', ...);
//! `the_lint_catches_a_surface_and_skips_tests` fails if the matcher stops seeing one or starts
//! reading a test module.
//!
//! Std-only, compiled standalone like the other lints (`just lints`):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/layer_lint.rs

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The one file that may build egui surfaces and name an egui Order.
const ALLOWED: &[&str] = &["src/gui/layers.rs"];

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

/// What a surface written outside layers.rs looks like.
const NAMES: &[&str] = &["Area::new(", "Window::new(", "Order::"];

/// Every line of `src` that builds egui's Area or Window or names an `Order` itself, comments
/// left out: (line number, the line). `SortOrder::` and other longer names are not it.
fn surfaces(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, line) in production_part(src).lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        let hit = NAMES.iter().any(|name| {
            let mut rest = code;
            while let Some(i) = rest.find(name) {
                let before = &rest[..i];
                let alone = !before.ends_with(|c: char| c.is_alphanumeric() || c == '_');
                // `egui::Order::` and a bare `Order::` (after `use egui::Order`); not `layers::Order`
                // (there is none) nor `SortOrder::`, which the `alone` test already refuses.
                if alone {
                    return true;
                }
                rest = &rest[i + name.len()..];
            }
            false
        });
        if hit {
            out.push((n + 1, line.trim().to_string()));
        }
    }
    out
}

#[test]
fn every_surface_is_drawn_on_the_layer_order() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    let mut tests: HashSet<PathBuf> = HashSet::new();
    for f in &files {
        if let Ok(src) = fs::read_to_string(f) {
            tests.extend(test_children(f, &src));
        }
    }
    let mut bad = Vec::new();
    let mut in_files = HashSet::new();
    for f in &files {
        let rel = f.strip_prefix(repo).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let in_test_dir = tests.iter().any(|t| {
            let dir = if t.file_name().and_then(|n| n.to_str()) == Some("mod.rs") { t.parent().map(Path::to_path_buf) } else { Some(t.with_extension("")) };
            dir.is_some_and(|d| f.starts_with(&d))
        });
        if ALLOWED.contains(&rel.as_str()) || tests.contains(f) || in_test_dir || rel.ends_with("_tests.rs") {
            continue;
        }
        let Ok(src) = fs::read_to_string(f) else { continue };
        for (n, line) in surfaces(&src) {
            in_files.insert(rel.clone());
            bad.push(format!("{rel}:{n}: {line}"));
        }
    }
    assert!(
        bad.is_empty(),
        "\n\n{} places in {} files build an egui surface or name an egui Order outside src/gui/layers.rs:\n  {}\n\n\
         Every floating surface takes its place in the one layer order (docs/design/navigation.md\n\
         section 6): `layers::area(ctx, id, Layer::...)` for an Area, `layers::window(ctx, id, title,\n\
         Layer::...)` for a Window, `layers::painter(ctx, id, Layer::...)` to paint, and a full-screen\n\
         popup is `layers::shade(ctx, id, Close::..., dim)` with its window on `Layer::PopupWindow`\n\
         seated on it (`layers::seat`). The table of layers is at the head of src/gui/layers.rs.\n",
        bad.len(),
        in_files.len(),
        bad.join("\n  ")
    );
}

/// The matcher itself, on made-up sources: it sees a surface in production code, not a longer
/// name or a comment, and never reads a test module.
#[test]
fn the_lint_catches_a_surface_and_skips_tests() {
    let src = "fn f(ctx: &Ctx) {\n    egui::Area::new(id).show(ctx, |_| {});\n    let o = SortOrder::Newest;\n    // egui::Window::new(\"x\")\n    let w = egui::Window::new(\"x\");\n    let l = egui::LayerId::new(egui::Order::Foreground, id);\n    let a = layers::area(ctx, id, Layer::PageFloat);\n}\n#[cfg(test)]\nmod tests {\n    fn t() { let s = \"}\"; let o = egui::Order::Tooltip; }\n}\nfn after() {\n    let p = Area::new(id);\n}\n";
    let found = surfaces(src);
    assert_eq!(found.iter().map(|w| w.0).collect::<Vec<_>>(), vec![2, 5, 6, 14], "the four surfaces in production code: {found:?}");
    let kids = test_children(Path::new("src/gui/layers.rs"), "#[cfg(test)]\n#[path = \"layers_tests.rs\"]\nmod tests;\n");
    assert!(kids.contains(&PathBuf::from("src/gui/layers_tests.rs")), "{kids:?}");
}

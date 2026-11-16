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

/// What may stand on the tooltip layer (layer 1) as an egui Area or Window: (the file, a piece of
/// the call's text that names it, why it must be an Area). EMPTY on purpose: nothing on layer 1
/// takes a click, and an Area there takes the click of whatever is under it (BUG-316), so
/// everything there is PAINTED (`layers::painter`, `widgets::painted_card`). Something that must
/// be clicked is not a tooltip: it moves to the layer the order gives clickable things.
const TOOLTIP_AREAS_ALLOWED: &[(&str, &str, &str)] = &[];

/// Every `layers::area(...)` and `layers::window(...)` call in production code, comments left
/// out: (line number of the call, "area" or "window", its last argument, the call on one line).
fn surface_calls(src: &str) -> Vec<(usize, &'static str, String, String)> {
    // Comments out first (a call written in a comment is not a call), lines kept.
    let code: String = production_part(src).lines().map(|l| format!("{}\n", l.split("//").next().unwrap_or(""))).collect();
    let mut out = Vec::new();
    for (name, kind) in [("layers::area(", "area"), ("layers::window(", "window")] {
        let mut from = 0;
        while let Some(i) = code[from..].find(name) {
            let start = from + i;
            let open = start + name.len() - 1;
            // The `)` that closes it, past nested calls and string literals.
            let b = code.as_bytes();
            let (mut depth, mut j) = (0usize, open);
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        j += 1;
                        while j < b.len() && b[j] != b'"' {
                            j += if b[j] == b'\\' { 2 } else { 1 };
                        }
                    }
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let args = &code[open + 1..j.min(code.len())];
            let last = args.split(',').map(str::trim).filter(|a| !a.is_empty()).last().unwrap_or("").to_string();
            let line = code[..start].matches('\n').count() + 1;
            let one_line = code[start..(j + 1).min(code.len())].split_whitespace().collect::<Vec<_>>().join(" ");
            out.push((line, kind, last, one_line));
            from = start + name.len();
        }
    }
    out.sort();
    out
}

/// The layer an argument names, when it is `Layer::Name` or a path ending in it
/// (`crate::gui::layers::Layer::Tooltip`); None for anything else (a variable, a call).
fn layer_named(arg: &str) -> Option<&str> {
    let (path, name) = arg.rsplit_once("Layer::")?;
    let path_ok = path.is_empty() || (path.ends_with("::") && path.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':'));
    (path_ok && !name.is_empty() && name.chars().all(|c| c.is_alphanumeric())).then_some(name)
}

/// NOTHING ON THE TOOLTIP LAYER TAKES A CLICK (2026-11-15, BUG-316's follow-up). Layer 1 holds
/// tooltips, hints and toasts, over everything, and its promise is that they never take a click.
/// egui 0.31 gives every Area a widget rect the size of what it holds, `interactable(false)`
/// included, and its hit test stops at the topmost layer with a widget over the pointer: seven
/// toasts (Areas then) buried the first-run privacy window's only button for 12 s. So an Area or
/// a Window may not be built on `Layer::Tooltip` (outside `TOOLTIP_AREAS_ALLOWED`, each entry with
/// its reason), and every `layers::area` and `layers::window` names its layer as `Layer::...`
/// itself, so this lint can read it (a layer passed in a variable could hide a tooltip).
///
/// RED FIRST, 2026-11-15, run on main at c5b4520bf (the toasts already painted): "6 Areas or
/// Windows built on the tooltip layer": the drop overlay's card and the drop note
/// (gui/drop_files.rs), the tip above a bar (widgets/button.rs), the chat's full timestamp
/// (pages/chat/rows.rs), the F2/F3/F4 readouts (pages/diagnostics.rs) and the planet card
/// (engine/frame_ui_run.rs).
#[test]
fn nothing_on_the_tooltip_layer_takes_a_click() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    let mut tests: HashSet<PathBuf> = HashSet::new();
    for f in &files {
        if let Ok(src) = fs::read_to_string(f) {
            tests.extend(test_children(f, &src));
        }
    }
    let (mut on_tooltip, mut unreadable) = (Vec::new(), Vec::new());
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
        for (n, kind, last, call) in surface_calls(&src) {
            let Some(layer) = layer_named(&last) else {
                unreadable.push(format!("{rel}:{n}: {kind} on `{last}`: {call}"));
                continue;
            };
            let allowed = TOOLTIP_AREAS_ALLOWED.iter().any(|(file, piece, _)| *file == rel && call.contains(piece));
            if layer == "Tooltip" && !allowed {
                on_tooltip.push(format!("{rel}:{n}: {call}"));
            }
        }
    }
    assert!(
        unreadable.is_empty(),
        "\n\n{} layers::area or layers::window calls do not name their layer as `Layer::...`:\n  {}\n\n\
         Name the layer in the call itself, so this lint can see whether it is the tooltip layer.\n",
        unreadable.len(),
        unreadable.join("\n  ")
    );
    assert!(
        on_tooltip.is_empty(),
        "\n\n{} Areas or Windows built on the tooltip layer:\n  {}\n\n\
         Nothing on layer 1 takes a click, and an egui Area there takes the click of whatever is\n\
         under it, `interactable(false)` or not (BUG-316). PAINT it instead: `layers::painter(ctx,\n\
         id, Layer::Tooltip)`, or a card of words with `widgets::painted_card::PaintedCard`. If it\n\
         must be clicked it is not a tooltip: move it to the layer the order gives clickable\n\
         things (src/gui/layers.rs). An Area that truly must stay goes in TOOLTIP_AREAS_ALLOWED\n\
         (tests/layer_lint.rs) with its reason.\n",
        on_tooltip.len(),
        on_tooltip.join("\n  ")
    );
}

/// The tooltip matcher on a made-up source: a call over several lines, a painter (allowed), a
/// layer in a variable (unreadable), a call in a comment and one in a test module (not read).
#[test]
fn the_tooltip_matcher_reads_each_call_and_its_layer() {
    let src = "fn f(ctx: &Ctx) {\n    crate::gui::layers::area(\n        ctx,\n        egui::Id::new((\"tip\", 1)),\n        crate::gui::layers::Layer::Tooltip,\n    )\n    .show(ctx, |_| {});\n    let p = layers::painter(ctx, id, Layer::Tooltip);\n    // layers::area(ctx, id, Layer::Tooltip)\n    let w = layers::window(ctx, id, \"a, b\", layer);\n    let a = layers::area(ctx, id, Layer::PageFloat);\n}\n#[cfg(test)]\nmod tests {\n    fn t() { let a = layers::area(ctx, id, Layer::Tooltip); }\n}\n";
    let found = surface_calls(src);
    let seen: Vec<(usize, &str, &str)> = found.iter().map(|(n, k, l, _)| (*n, *k, l.as_str())).collect();
    assert_eq!(
        seen,
        vec![(2, "area", "crate::gui::layers::Layer::Tooltip"), (10, "window", "layer"), (11, "area", "Layer::PageFloat")],
        "the three calls in production code, each with its last argument: {found:?}"
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

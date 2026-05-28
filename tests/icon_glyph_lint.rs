//! Icon glyph lint (v0.188.x).
//!
//! The egui font loaded by HumanityOS has spotty Unicode coverage outside
//! the basic Latin / Latin-1 / Arrows ranges. Some glyphs in **Math
//! Operators** (U+2200..U+22FF) and **Dingbats** (U+2700..U+27BF) render
//! as tofu (▢) — operator-confirmed examples include `⊠` U+22A0 and
//! `✎` U+270E. The U+FE0F variation selector ALSO always renders as
//! tofu (it appears as a trailing square next to the emoji it modifies,
//! e.g. `❤️1` displays as `❤▢1`).
//!
//! This test scans `src/gui/` for Rust string literals that contain any
//! known-broken codepoint, in any of the rendering call sites:
//! `painter.text(...)`, `RichText::new(...)`, `ui.button(...)`,
//! `egui::Button::new(...)`, `widgets::Button::*("...")`, etc. New
//! violations FAIL the test. Allowed escape hatches:
//!
//! 1. **Painter shape calls** are skipped — `painter.circle_filled`,
//!    `painter.rect_filled` etc. don't render text.
//! 2. **Lines marked `// glyph-exempt: <reason>`** bypass the check.
//!    Use sparingly and only for glyphs you've personally verified
//!    render correctly in this font (operator screenshot evidence).
//!
//! When you discover a new tofu-rendering glyph: add its codepoint to
//! `BROKEN_GLYPHS` below and the test fails on every existing usage,
//! forcing a fix. When a previously-broken glyph starts working
//! (e.g. font upgrade), remove it from the list.

use std::fs;
use std::path::PathBuf;

/// Codepoints that render as tofu in the currently-loaded egui font.
/// Every entry has been observed broken in operator screenshots OR is
/// in a block that's known-spotty AND has no working evidence.
const BROKEN_GLYPHS: &[(char, &str)] = &[
    // Variation selector — always trails its emoji as a square.
    ('\u{FE0F}', "U+FE0F variation selector — always renders as tofu"),
    // Math Operators block (some work like ∞, but these don't):
    ('\u{22A0}', "⊠ U+22A0 SQUARED TIMES — confirmed broken (was used for pin)"),
    ('\u{2261}', "≡ U+2261 IDENTICAL TO — confirmed broken (was used for nav layout toggle)"),
    // Dingbats block (mostly broken):
    ('\u{270E}', "✎ U+270E LOWER RIGHT PENCIL — confirmed broken (was used for edit)"),
    ('\u{270F}', "✏ U+270F PENCIL — same block, assume broken"),
    ('\u{2711}', "✑ U+2711 WHITE NIB — same block, assume broken"),
    ('\u{2712}', "✒ U+2712 BLACK NIB — same block, assume broken"),
    // Geometric Shapes — mixed support:
    ('\u{25A4}', "▤ U+25A4 SQUARE WITH HORIZONTAL FILL — confirmed broken"),
    // Misc Symbols & Pictographs (emoji) — egui default font has no coverage:
    ('\u{1F512}', "🔒 U+1F512 LOCK — emoji, tofus in egui (use the word 'encrypted' or paint_lock instead)"),
];

/// Files scanned. Only UI-rendering trees.
const SCAN_DIRS: &[&str] = &["src/gui"];

#[test]
fn no_known_broken_unicode_glyphs_in_ui() {
    let project_root = project_root();
    let mut violations: Vec<String> = Vec::new();

    for dir in SCAN_DIRS {
        let dir_path = project_root.join(dir);
        if !dir_path.exists() { continue; }
        for file in walk_rs(&dir_path) {
            let body = match fs::read_to_string(&file) {
                Ok(b) => b,
                Err(_) => continue,
            };
            for (line_no_zero, line) in body.lines().enumerate() {
                if line.contains("glyph-exempt") { continue; }
                // Skip lines that are clearly comments — rough heuristic
                // (full block comments could slip through, acceptable
                // tradeoff vs. parsing Rust).
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with("//!") {
                    continue;
                }
                // Also skip lines in BROKEN_GLYPHS itself (this file).
                if file.file_name().map(|n| n == "icon_glyph_lint.rs").unwrap_or(false) {
                    continue;
                }
                for (bad_char, reason) in BROKEN_GLYPHS {
                    if line.contains(*bad_char) {
                        let rel = file.strip_prefix(&project_root).unwrap_or(&file);
                        violations.push(format!(
                            "  {}:{} — contains {} ({})\n      line: {}",
                            rel.display(),
                            line_no_zero + 1,
                            bad_char,
                            reason,
                            line.trim(),
                        ));
                    }
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\n❌ Found {} known-broken Unicode glyph(s) in UI code.\n\
             \n\
             These codepoints render as tofu (▢) in the currently-loaded\n\
             egui font. Replace with one of:\n\
             - Plain text labels (e.g. \"Edit\" instead of ✎)\n\
             - A confirmed-working glyph from the same family\n\
               (Latin / Latin-1 / Arrows U+2190..U+21FF / ❤ ⭐ ∞ ✓ ⚠)\n\
             - A SVG-style painter call (painter.circle_filled, etc.)\n\
             \n\
             Or, if you're sure the glyph renders in the loaded font (you\n\
             have operator screenshot evidence), append `// glyph-exempt:\n\
             <reason>` to the line.\n\
             \n\
             Offending lines:\n{}\n",
            violations.len(),
            violations.join("\n"),
        );
    }
}

fn walk_rs(dir: &std::path::Path) -> Vec<PathBuf> {
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

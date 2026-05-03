//! Theme token enforcement (v0.175.0).
//!
//! HumanityOS rule: every color, font size, spacing value, and similar
//! styling token used by UI code MUST come from `data/gui/theme.ron`
//! (via `theme.X()` accessors). Hardcoded literals like
//! `Color32::from_rgb(231, 76, 60)` bypass the Settings page color
//! editor — once they accumulate the "100% of theme tokens editable
//! in-app" promise breaks silently.
//!
//! This test is the rule's teeth. CLAUDE.md is just docs; the next AI
//! will skim docs but it cannot skip a failing test. Run via:
//!
//! ```
//! cargo test --test theme_token_lint
//! ```
//!
//! How violations are handled:
//! - **New violations** (in files not on the legacy allowlist): test FAILS
//!   and prints the offending file:line locations + a hint. Fix by
//!   adding a token to `theme.ron` + accessor to `theme.rs`, or by
//!   annotating the line with `// theme-exempt: <reason>`.
//! - **Legacy violations** (in files on the allowlist): test PASSES but
//!   prints a warning summary. As legacy offenders are migrated, remove
//!   their entry from `LEGACY_OFFENDERS`. Eventually the allowlist
//!   shrinks to zero and the test becomes strict everywhere.
//!
//! Escape hatch: lines containing `// theme-exempt:` are always allowed.
//! Use this for transient state visualization (e.g. a debug overlay), or
//! when a pure-math helper computes a Color32 from runtime values
//! (e.g. HSV→RGB). Keep the reason short and specific.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Files known to predate the v0.175.0 enforcement push. They contain
/// hardcoded color literals that need migration to theme tokens. Listed
/// here so the test passes today; remove entries as you migrate them
/// (one per release is the goal). Aim: shrink this list to zero.
///
/// To audit one of these:
/// 1. Identify each `Color32::from_rgb(...)` call in the file
/// 2. Add a token to `data/gui/theme.ron` + accessor in `src/gui/theme.rs`
/// 3. Replace the call with `theme.X()`
/// 4. Or, if it's genuinely transient/diagnostic, annotate with
///    `// theme-exempt: <reason>` on the same line.
/// 5. Remove the file from this list.
const LEGACY_OFFENDERS: &[&str] = &[
    // Transient debug visualization, low priority. Remove after audit.
    "src/debug.rs",
    // Inline avatar placeholder colors derived from name hash. Could move
    // to a palette in theme.ron. Medium priority.
    "src/gui/mod.rs",
    "src/gui/widgets/icons.rs",
    // Voice-meter state colors + status dots — semantic transient state
    // visualization. Roadmap calls these "mostly legitimate". Audit then
    // either annotate `theme-exempt` or move the palette to theme.ron.
    "src/gui/pages/chat.rs",
    "src/gui/widgets/row.rs",
    "src/gui/widgets/passphrase_modal.rs",
    "src/gui/widgets/mod.rs",
    "src/gui/widgets/alert.rs",
    "src/gui/widgets/image_cache_view.rs",
    // Page-level palettes (category colors, avatar placeholders, semantic
    // domain markers like map biomes). High priority — these are exactly
    // the theme-token rule's intended target.
    "src/gui/pages/donate.rs",
    "src/gui/pages/inventory.rs",
    "src/gui/pages/hud.rs",
    "src/gui/pages/maps.rs",
    "src/gui/pages/market.rs",
    "src/gui/pages/placeholder.rs",
    "src/gui/pages/profile.rs",
    "src/gui/pages/settings.rs",
    "src/gui/pages/server_settings.rs",
    "src/gui/pages/studio.rs",
    "src/gui/pages/wallet.rs",
    // theme.rs is the authoritative source; literal calls in `c32` and
    // accessor implementations are by design. Always allowed.
    "src/gui/theme.rs",
    // lib.rs has one literal in the engine bootstrap (clear color or
    // similar). Audit + likely add a renderer/clear-color token.
    "src/lib.rs",
];

/// Directories scanned for violations. Anything outside these trees
/// (e.g. `src/relay/`, `src/ecs/`, `src/systems/`) doesn't render UI
/// and is exempt.
const SCAN_DIRS: &[&str] = &["src/gui", "src/renderer"];

#[test]
fn theme_tokens_only_no_hardcoded_colors_in_ui() {
    let project_root = project_root();
    let legacy: HashSet<PathBuf> = LEGACY_OFFENDERS
        .iter()
        .map(|p| project_root.join(p))
        .collect();

    let mut new_violations: Vec<Violation> = Vec::new();
    let mut legacy_violation_count: usize = 0;

    for dir in SCAN_DIRS {
        let dir_path = project_root.join(dir);
        if !dir_path.exists() { continue; }
        for file in walk_rs(&dir_path) {
            let is_legacy = legacy.contains(&file);
            let body = match fs::read_to_string(&file) {
                Ok(b) => b,
                Err(_) => continue,
            };
            for (idx, line) in body.lines().enumerate() {
                if !looks_like_hardcoded_color(line) { continue; }
                if line.contains("theme-exempt") { continue; }
                if is_legacy {
                    legacy_violation_count += 1;
                } else {
                    new_violations.push(Violation {
                        file: file.clone(),
                        line_number: idx + 1,
                        line: line.trim().to_string(),
                    });
                }
            }
        }
    }

    if legacy_violation_count > 0 {
        // Warn-only: print summary so we can see the cleanup runway, but
        // don't fail the build until each legacy file is migrated.
        eprintln!(
            "theme_tokens lint — {} pre-existing color literals in {} legacy file(s) (allowlisted; migrate one at a time and remove from LEGACY_OFFENDERS)",
            legacy_violation_count,
            LEGACY_OFFENDERS.len(),
        );
    }

    if !new_violations.is_empty() {
        let mut msg = format!(
            "\n\n❌ Found {} new hardcoded color literal(s) in UI code.\n\
             \n\
             HumanityOS rule: every color in `src/gui/` and `src/renderer/` MUST come\n\
             from a theme token (data/gui/theme.ron) so the Settings page color editor\n\
             can tune it. See docs/design/ui-system.md.\n\
             \n\
             Fix one of these ways:\n\
             1. Add a token to `data/gui/theme.ron` + accessor in `src/gui/theme.rs`,\n\
                then replace the literal with `theme.X()`.\n\
             2. If it's a genuinely transient/computed color (debug overlay,\n\
                programmatic gradient, HSV math), annotate the line with\n\
                `// theme-exempt: <short reason>`.\n\
             \n\
             Offending lines:\n",
            new_violations.len()
        );
        for v in &new_violations {
            msg.push_str(&format!(
                "  {}:{}\n    {}\n",
                v.file.strip_prefix(&project_root).unwrap_or(&v.file).display(),
                v.line_number,
                v.line,
            ));
        }
        panic!("{}", msg);
    }
}

struct Violation {
    file: PathBuf,
    line_number: usize,
    line: String,
}

/// Pattern-match for a hardcoded color literal call. Catches the four
/// constructors that take literal numbers; deliberately ignores
/// `Color32::WHITE`/`BLACK`/named constants (those are semantic
/// primitives), and ignores constructors fed from runtime variables
/// (e.g. `Color32::from_rgb(r, g, b)` when r/g/b are bindings).
fn looks_like_hardcoded_color(line: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "Color32::from_rgb(",
        "Color32::from_rgba(",
        "Color32::from_rgba_unmultiplied(",
        "Color32::from_rgba_premultiplied(",
    ];
    for needle in NEEDLES {
        if let Some(idx) = line.find(needle) {
            // Look at the character right after the `(` — if it's a digit
            // (or whitespace before a digit), this is a literal call.
            let after = &line[idx + needle.len()..];
            let first_non_ws = after.chars().find(|c| !c.is_whitespace());
            if matches!(first_non_ws, Some(c) if c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
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

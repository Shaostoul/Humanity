//! Theme editor coverage test (v0.176.0).
//!
//! HumanityOS rule: every color token in `src/gui/theme.rs` MUST appear
//! as an editable row in the Settings → Appearance page. The promise is
//! "100% of theme tokens editable in-app"; if a token is added but never
//! wired into the editor, the promise quietly breaks (operator hits this
//! when they look for the new token and it's missing — see v0.175 →
//! v0.176 nav_* incident).
//!
//! This test parses both files and asserts every color field
//! (`pub <name>: C,`) in `theme.rs` is referenced by at least one
//! `&mut theme.<name> as *mut _` in `settings.rs`. New token + missing
//! editor row → test fails with the missing names. Run via:
//!
//! ```
//! cargo test --test theme_editor_coverage
//! ```
//!
//! Sizing tokens (f32) follow the same rule but live in the Widgets
//! section of the editor, referenced as `&mut theme.<name>`. They're
//! checked separately so we get clean diagnostics per-category.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[test]
fn every_theme_color_token_has_editor_row() {
    let root = project_root();
    let theme_src = fs::read_to_string(root.join("src/gui/theme.rs"))
        .expect("read src/gui/theme.rs");
    let editor_src = fs::read_to_string(root.join("src/gui/pages/settings.rs"))
        .expect("read src/gui/pages/settings.rs");

    let color_tokens = extract_color_fields(&theme_src);
    let editor_refs = extract_editor_color_refs(&editor_src);

    let missing: Vec<String> = color_tokens
        .iter()
        .filter(|t| !editor_refs.contains(*t))
        .cloned()
        .collect();

    if !missing.is_empty() {
        let mut msg = format!(
            "\n\n❌ {} color token(s) defined in src/gui/theme.rs are NOT editable in src/gui/pages/settings.rs:\n\n",
            missing.len()
        );
        for name in &missing {
            msg.push_str(&format!("  - theme.{}\n", name));
        }
        msg.push_str(
            "\n\
             Fix: add a row in `draw_appearance_content` (settings.rs) like:\n\
             \n\
                 (\"Friendly Label\", &mut theme.<name> as *mut _),\n\
             \n\
             The Settings page color editor uses raw-pointer arrays in\n\
             two-column layouts; pick the column with the lighter list\n\
             so they balance. Match the section appropriate to the token's\n\
             purpose (palette / panel / nav / etc).\n",
        );
        panic!("{}", msg);
    }
}

#[test]
fn every_theme_size_token_has_editor_row() {
    let root = project_root();
    let theme_src = fs::read_to_string(root.join("src/gui/theme.rs"))
        .expect("read src/gui/theme.rs");
    let editor_src = fs::read_to_string(root.join("src/gui/pages/settings.rs"))
        .expect("read src/gui/pages/settings.rs");

    let size_tokens = extract_size_fields(&theme_src);
    // Sizing editor uses `&mut theme.<name>` (no `as *mut _` because the
    // styled_slider helper takes `&mut f32` directly).
    let editor_refs = extract_editor_direct_refs(&editor_src);

    // Some size-shaped fields are intentionally not user-editable
    // (computed from others, or context-bound). List them here with a
    // reason and they'll be excluded from the coverage requirement.
    let intentionally_omitted: HashSet<&str> = ["max_messages"].iter().copied().collect();

    let missing: Vec<String> = size_tokens
        .iter()
        .filter(|t| !editor_refs.contains(*t) && !intentionally_omitted.contains(t.as_str()))
        .cloned()
        .collect();

    if !missing.is_empty() {
        let mut msg = format!(
            "\n\n❌ {} numeric token(s) defined in src/gui/theme.rs are NOT editable in src/gui/pages/settings.rs:\n\n",
            missing.len()
        );
        for name in &missing {
            msg.push_str(&format!("  - theme.{}\n", name));
        }
        msg.push_str(
            "\n\
             Fix: add a `styled_slider(ui, &ss, \"Friendly Label\", &mut theme.<name>, MIN..=MAX, label_color);`\n\
             call inside the appropriate `make_card(ui, \"Section\", ...)` block in\n\
             `draw_widgets_content` (settings.rs).\n\
             \n\
             If the token is intentionally not user-editable, add it to\n\
             `intentionally_omitted` in this test with a reason comment.\n",
        );
        panic!("{}", msg);
    }
}

/// Parse `pub <name>: C,` lines from theme.rs to get color field names.
fn extract_color_fields(theme_src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in theme_src.lines() {
        let l = line.trim();
        // Match either `pub <name>: C,` or `pub <name>: C` (last field).
        if !(l.starts_with("pub ") && (l.contains(": C,") || l.ends_with(": C"))) {
            continue;
        }
        // Skip serde attrs / comments.
        if l.starts_with("//") { continue; }
        // Extract name between `pub ` and `:`.
        let after_pub = &l["pub ".len()..];
        if let Some(colon_idx) = after_pub.find(':') {
            let name = after_pub[..colon_idx].trim().to_string();
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// Parse `pub <name>: f32,` lines to get sizing/spacing/font-size fields.
fn extract_size_fields(theme_src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in theme_src.lines() {
        let l = line.trim();
        if !(l.starts_with("pub ")
            && (l.contains(": f32,") || l.contains(": usize,")
                || l.ends_with(": f32") || l.ends_with(": usize")))
        {
            continue;
        }
        if l.starts_with("//") { continue; }
        let after_pub = &l["pub ".len()..];
        if let Some(colon_idx) = after_pub.find(':') {
            let name = after_pub[..colon_idx].trim().to_string();
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// Find every `&mut theme.<name> as *mut _` reference in settings.rs.
/// These are the rows in the color-picker grid (see `color_row` callsites).
fn extract_editor_color_refs(editor_src: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for line in editor_src.lines() {
        // Pattern: "&mut theme.<name> as *mut _"
        if let Some(start) = line.find("&mut theme.") {
            let after = &line[start + "&mut theme.".len()..];
            if let Some(end) = after.find(|c: char| !(c.is_alphanumeric() || c == '_')) {
                if line[start..].contains("as *mut _") {
                    out.insert(after[..end].to_string());
                }
            }
        }
    }
    out
}

/// Find every `&mut theme.<name>` reference NOT followed by `as *mut _`.
/// These are the styled_slider callsites in draw_widgets_content.
fn extract_editor_direct_refs(editor_src: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for line in editor_src.lines() {
        let mut idx = 0;
        while let Some(rel) = line[idx..].find("&mut theme.") {
            let start = idx + rel;
            let after = &line[start + "&mut theme.".len()..];
            if let Some(end) = after.find(|c: char| !(c.is_alphanumeric() || c == '_')) {
                let name = after[..end].to_string();
                // Skip if this same reference is followed by `as *mut _`
                // (color-row pattern handled separately).
                let tail = &line[start..];
                let is_pointer_cast = tail.contains("as *mut _")
                    && tail.find(&name).map(|p| p < tail.find("as *mut _").unwrap_or(usize::MAX)).unwrap_or(false);
                if !is_pointer_cast {
                    out.insert(name);
                }
                idx = start + "&mut theme.".len() + end;
            } else {
                break;
            }
        }
    }
    out
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

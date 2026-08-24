//! Settings-persistence lint (2026-07-30).
//!
//! A setting is only real if it survives a restart. That means making the whole
//! round trip:
//!
//!   bound to a widget in src/gui/pages/settings.rs
//!     -> field on `AppConfig`            (src/config.rs)
//!     -> written by `from_gui_state`     (the save leg)
//!     -> read by `apply_to_state`        (the load leg)
//!
//! Miss any leg and the control works, takes effect immediately, and then
//! silently reverts on the next launch. Nothing errors, nothing logs, and the
//! person has no way to tell. An audit on 2026-07-30 found SEVEN settings in
//! that state, including both privacy toggles (someone who made themselves
//! private was public again next launch) and the font-size accessibility
//! slider.
//!
//! Std-only file scanner (no crate imports), compiled standalone like the other
//! lints so it never links the native bin (Windows LNK1318 gotcha):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/settings_persistence_lint.rs
//! or run it through `just lints`.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Settings that are deliberately transient: scratch UI state that must NOT be
/// written to disk. `seed_phrase_*` especially: persisting a recovery phrase to
/// a plaintext config would be a security bug, not a convenience.
const INTENTIONALLY_TRANSIENT: &[&str] = &[
    "category",                    // which settings section is open
    "scroll_to_section",           // one-frame scroll request
    "seed_phrase_input",           // typed recovery phrase, never persist
    "seed_phrase_visible",         // reveal toggle for the above
    "seed_phrase_show_recover",    // panel open/closed
    "seed_phrase_recovery_status", // transient status line
];

/// Pull every `state.settings.<field>` that settings.rs binds to a widget or
/// assigns to. Those are the fields a person can actually change.
fn bound_settings() -> HashSet<String> {
    let src = read("src/gui/pages/settings.rs");
    let mut out = HashSet::new();
    for pat in ["&mut state.settings.", "state.settings."] {
        let mut from = 0usize;
        while let Some(i) = src[from..].find(pat) {
            let start = from + i + pat.len();
            let name: String = src[start..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            from = start;
            if name.is_empty() {
                continue;
            }
            // Only count it if this really is a binding or an assignment, not a
            // read like `state.settings.font_size` used to size another widget.
            let rest = src[start + name.len()..].trim_start();
            let is_binding = src[..start].ends_with("&mut state.settings.");
            let is_assign = rest.starts_with('=') && !rest.starts_with("==");
            if is_binding || is_assign {
                out.insert(name);
            }
        }
    }
    assert!(
        out.len() > 30,
        "sanity: found only {} bound settings, the scanner probably broke",
        out.len()
    );
    out
}

/// True if `haystack` contains an assignment to `lhs` (`lhs = ...`), ignoring
/// comparisons (`lhs ==`). Whitespace between the path and `=` is allowed.
fn assigns_in(haystack: &str, lhs: &str) -> bool {
    let mut from = 0usize;
    while let Some(i) = haystack[from..].find(lhs) {
        let after = from + i + lhs.len();
        from = after;
        let rest = haystack[after..].trim_start();
        // Reject a longer identifier that merely starts with `lhs`
        // (`state.settings.fov` must not match `state.settings.fov_locked`).
        let next = haystack[after..].chars().next().unwrap_or(' ');
        if next.is_ascii_alphanumeric() || next == '_' {
            continue;
        }
        if rest.starts_with('=') && !rest.starts_with("==") {
            return true;
        }
    }
    false
}

/// Every user-changeable setting must complete the save/load round trip.
#[test]
fn every_setting_the_ui_exposes_is_persisted() {
    let config = read("src/config.rs");

    // AppConfig field list.
    let struct_start = config.find("pub struct AppConfig").expect("AppConfig present");
    let struct_end = config[struct_start..].find("\n}").expect("struct closes") + struct_start;
    let struct_body = &config[struct_start..struct_end];

    // The save leg and the load leg.
    let save_start = config.find("pub fn from_gui_state").expect("from_gui_state present");
    let save_body = &config[save_start..save_start + 8000.min(config.len() - save_start)];

    let transient: HashSet<&str> = INTENTIONALLY_TRANSIENT.iter().copied().collect();
    let mut broken = Vec::new();

    for field in bound_settings() {
        if transient.contains(field.as_str()) {
            continue;
        }
        let on_config = struct_body.contains(&format!("pub {field}:"));
        let saved = save_body.contains(&format!("{field}: state.settings.{field}"))
            || save_body.contains(&format!("{field}: state.{field}"));
        // Match the ASSIGNMENT, not a particular right-hand side. Many fields
        // are loaded through a conditional or a match that spans lines
        // (`state.settings.sky_glow_tier = if self.sky_glow_tier == "ultra" {`),
        // and an exact `= self.<field>` match reports all of those as missing.
        let loaded = assigns_in(&config, &format!("state.settings.{field}"))
            || assigns_in(&config, &format!("state.{field}"));

        if !on_config || !saved || !loaded {
            let mut missing = Vec::new();
            if !on_config {
                missing.push("not a field on AppConfig");
            }
            if !saved {
                missing.push("never written by from_gui_state");
            }
            if !loaded {
                missing.push("never read by apply_to_state");
            }
            broken.push(format!("  {field}: {}", missing.join(", ")));
        }
    }

    broken.sort();
    assert!(
        broken.is_empty(),
        "\n\nThese settings have a working control but do NOT survive a restart:\n\n{}\n\n\
         The control will move, take effect, and silently revert on the next launch.\n\
         Fix by adding the field to AppConfig, writing it in from_gui_state, and \
         reading it in apply_to_state. If it is genuinely scratch UI state that must \
         not be written to disk, add it to INTENTIONALLY_TRANSIENT with a reason.\n",
        broken.join("\n")
    );
}

/// A setting that never marks the settings dirty is never saved either, because
/// the save is driven by that flag in the render loop. This catches the other
/// half of the same bug: the privacy toggles were discarding the `changed`
/// return of `widgets::toggle`, so nothing ever asked for a write.
#[test]
fn privacy_toggles_mark_settings_dirty() {
    // profile_visible stays in settings.rs; online_status_visible moved to
    // pages/privacy.rs (2026-08-23, privacy tiers) where the SAME toggle is
    // now also server-enforced via privacy_update. Each is checked where it
    // actually renders.
    let checks: [(&str, &str, &str); 2] = [
        ("profile_visible", "src/gui/pages/settings.rs", "fn draw_privacy_content"),
        ("online_status_visible", "src/gui/pages/privacy.rs", "fn draw_privacy_content"),
    ];
    for (field, file, anchor) in checks {
        let src = read(file);
        let start = src.find(anchor).unwrap_or_else(|| panic!("{anchor} present in {file}"));
        let body = &src[start..(start + 4000).min(src.len())];
        let idx = body
            .find(field)
            .unwrap_or_else(|| panic!("{field} should be rendered in {file}'s privacy section"));
        // Window is wide enough to span the toggle's hover-text string
        // between the binding and the dirty-flag assignment.
        let after = &body[idx..(idx + 700).min(body.len())];
        assert!(
            after.contains("settings_dirty = true"),
            "\n\n`{field}` is a PRIVACY toggle that does not set settings_dirty, so \
             changing it is never written to the config. Someone who makes themselves \
             private will be public again on the next launch, with no indication it \
             happened.\n"
        );
    }
}

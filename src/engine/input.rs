//! Key rules for the F10 Cloud dev sidebar, extracted from lib.rs's raw
//! winit keyboard arm (2026-09-18) so that the key, the headless tests and
//! the dev IPC (`debug/ui_request.json`, see `engine::ipc`) all run the SAME
//! code. Nothing here touches winit or the OS cursor: these functions only
//! flip `GuiState` flags, and lib.rs `reconcile_cursor` derives the cursor
//! from those flags every frame (the single-authority rule, v0.460), so a
//! test can prove the rules without a window.
//!
//! What the winit layer between the key and these functions still does on
//! its own (and what the IPC therefore does NOT exercise): the modifier
//! trackers, the keybind-capture gate, and the ordering of the Escape rules
//! relative to the in-world modal and screen-focus gates. The headless
//! harness and the operator's own key are what cover that layer.

use crate::engine::state::EngineState;
use crate::gui::{GuiPage, GuiState};

/// The F10 key. On an EXPANDED sidebar it closes it (the cursor re-grabs
/// through `reconcile_cursor`, same as before it opened); on a closed OR
/// collapsed sidebar it opens it expanded, so the key always works even
/// when the slim tab cannot be clicked because the cursor is grabbed.
/// (2026-09-05, operator: "Pressing F10 should free the cursor and bring
/// up the menu so I don't have to hold alt".)
pub fn toggle_cloud_dev_panel(gui: &mut GuiState) {
    if gui.cloud_dev_sidebar_expanded() {
        gui.show_cloud_dev_panel = false;
    } else {
        gui.show_cloud_dev_panel = true;
        gui.cloud_dev_collapsed = false;
    }
}

/// The Escape key, FIRST in lib.rs's Escape ordering (operator, 2026-09-18:
/// "when I press esc it closes the F10 menu only instead of also opening up
/// the menu, but only while the F10 menu is up. If the F10 menu isn't open
/// then ESC should open the menu"). While the sidebar is expanded it is the
/// thing the operator opened last and the thing under their cursor, so
/// dismissing it is what Escape means; the caller must then do NOTHING ELSE
/// with that press (no help-panel unpin, no menu cascade). Returns true only
/// when it closed the sidebar. A closed or COLLAPSED sidebar returns false:
/// collapsed means the cursor is already grabbed and only the tab remains,
/// which is the state the game would be in anyway, so Escape falls through
/// to its usual meaning (the menu).
pub fn escape_closes_cloud_dev(gui: &mut GuiState) -> bool {
    if gui.cloud_dev_sidebar_expanded() {
        gui.show_cloud_dev_panel = false;
        true
    } else {
        false
    }
}

/// Whether the OS cursor should be FREE (visible + ungrabbed) this frame,
/// derived from the flags alone. lib.rs `reconcile_cursor` is the only
/// place that acts on it; the dev IPC reports it so a rig can tell "the
/// cursor authority wants it free" apart from "the window actually freed
/// it" (a background rig never touches the cursor, so the two differ
/// there by design, see `EngineState::background_no_cursor`).
///
/// Free for any menu page, the showroom, the construction editor, while
/// Alt is held (v0.735), while the F10 sidebar is expanded (2026-09-05, the
/// held-Alt rule made sticky), while an in-world modal is open (chat v0.772,
/// creature editor v0.778, talk card v0.797) and while dead (the Respawn
/// button needs the cursor). Everything else is first-person play: grabbed.
pub fn cursor_want_free(state: &EngineState) -> bool {
    state.gui_state.active_page != GuiPage::None
        || state.gui_state.showroom_active
        || state.gui_state.construction_active
        || state.alt_held
        || state.gui_state.cloud_dev_sidebar_expanded()
        || state.gui_state.in_world_modal_open()
        || state.gui_state.player_death_cause.is_some()
}

/// Read one of the F10 sidebar's flags by its `GuiState` field name, as JSON
/// (a bool, or a number for the sliders and pickers), for the dev IPC's
/// before/after report. `None` for a name that is not a sidebar flag. Rust
/// has no field reflection, so this is a hand-kept table; the test below
/// scans `src/gui/mod.rs` for every `cloud_dev_*` field and fails the moment
/// one is added here without a row, so the table cannot drift.
pub fn cloud_dev_flag(gui: &GuiState, name: &str) -> Option<serde_json::Value> {
    use serde_json::json;
    Some(match name {
        "show_cloud_dev_panel" => json!(gui.show_cloud_dev_panel),
        "cloud_dev_collapsed" => json!(gui.cloud_dev_collapsed),
        "cloud_dev_dither_off" => json!(gui.cloud_dev_dither_off),
        "cloud_dev_temporal_off" => json!(gui.cloud_dev_temporal_off),
        "cloud_dev_map_diag" => json!(gui.cloud_dev_map_diag),
        "cloud_dev_clock_pin" => json!(gui.cloud_dev_clock_pin),
        "cloud_dev_chord_foot" => json!(gui.cloud_dev_chord_foot),
        "cloud_dev_world_shape_lod" => json!(gui.cloud_dev_world_shape_lod),
        "cloud_dev_ring_cure_off" => json!(gui.cloud_dev_ring_cure_off),
        "cloud_dev_uniform_step" => json!(gui.cloud_dev_uniform_step),
        "cloud_dev_wide_edge" => json!(gui.cloud_dev_wide_edge),
        "cloud_dev_edge_mul" => json!(gui.cloud_dev_edge_mul),
        "cloud_dev_rind_wide_m" => json!(gui.cloud_dev_rind_wide_m),
        "cloud_dev_step_m" => json!(gui.cloud_dev_step_m),
        "cloud_dev_shear" => json!(gui.cloud_dev_shear),
        "cloud_dev_hv_km" => json!(gui.cloud_dev_hv_km),
        "cloud_dev_sigma_mul" => json!(gui.cloud_dev_sigma_mul),
        "cloud_dev_est" => json!(gui.cloud_dev_est),
        "cloud_dev_warp_bl" => json!(gui.cloud_dev_warp_bl),
        "cloud_dev_norm_floor" => json!(gui.cloud_dev_norm_floor),
        "cloud_dev_iso_step" => json!(gui.cloud_dev_iso_step),
        "cloud_dev_thin_deck" => json!(gui.cloud_dev_thin_deck),
        "cloud_dev_hv_warp" => json!(gui.cloud_dev_hv_warp),
        "cloud_dev_no_detail" => json!(gui.cloud_dev_no_detail),
        "cloud_dev_no_puff" => json!(gui.cloud_dev_no_puff),
        "cloud_dev_no_cell" => json!(gui.cloud_dev_no_cell),
        "cloud_dev_no_fray" => json!(gui.cloud_dev_no_fray),
        "cloud_dev_no_bdrop" => json!(gui.cloud_dev_no_bdrop),
        "cloud_dev_sharp_base" => json!(gui.cloud_dev_sharp_base),
        "cloud_dev_relief_fade" => json!(gui.cloud_dev_relief_fade),
        "cloud_dev_deep_rung" => json!(gui.cloud_dev_deep_rung),
        "cloud_dev_checker" => json!(gui.cloud_dev_checker),
        "cloud_dev_ms" => json!(gui.cloud_dev_ms),
        "cloud_dev_field" => json!(gui.cloud_dev_field),
        "cloud_dev_body_cache" => json!(gui.cloud_dev_body_cache),
        "cloud_dev_step_eco" => json!(gui.cloud_dev_step_eco),
        "cloud_dev_light" => json!(gui.cloud_dev_light),
        "cloud_dev_profile_knob" => json!(gui.cloud_dev_profile_knob),
        "cloud_dev_top_bound" => json!(gui.cloud_dev_top_bound),
        "cloud_dev_ms_gain" => json!(gui.cloud_dev_ms_gain),
        "cloud_dev_int_sat" => json!(gui.cloud_dev_int_sat),
        "cloud_dev_res_div" => json!(gui.cloud_dev_res_div),
        "cloud_dev_shape_off" => json!(gui.cloud_dev_shape_off),
        "cloud_dev_discard_diag" => json!(gui.cloud_dev_discard_diag),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed() -> GuiState {
        let mut g = GuiState::default();
        g.show_cloud_dev_panel = false;
        g.cloud_dev_collapsed = false;
        g
    }
    fn expanded() -> GuiState {
        let mut g = GuiState::default();
        g.show_cloud_dev_panel = true;
        g.cloud_dev_collapsed = false;
        g
    }
    fn collapsed() -> GuiState {
        let mut g = GuiState::default();
        g.show_cloud_dev_panel = true;
        g.cloud_dev_collapsed = true;
        g
    }

    #[test]
    fn f10_opens_a_closed_sidebar_expanded() {
        let mut g = closed();
        toggle_cloud_dev_panel(&mut g);
        assert!(g.show_cloud_dev_panel && !g.cloud_dev_collapsed);
        assert!(g.cloud_dev_sidebar_expanded());
    }

    #[test]
    fn f10_closes_an_expanded_sidebar() {
        let mut g = expanded();
        toggle_cloud_dev_panel(&mut g);
        assert!(!g.show_cloud_dev_panel);
        assert!(!g.cloud_dev_sidebar_expanded());
    }

    #[test]
    fn f10_expands_a_collapsed_sidebar_instead_of_closing_it() {
        // The tab is not drawn while the cursor is grabbed, so the key must
        // be the way back to the expanded state, never a close.
        let mut g = collapsed();
        toggle_cloud_dev_panel(&mut g);
        assert!(g.show_cloud_dev_panel && !g.cloud_dev_collapsed);
    }

    #[test]
    fn escape_closes_only_an_expanded_sidebar_and_says_so() {
        let mut g = expanded();
        assert!(escape_closes_cloud_dev(&mut g), "Escape must report that it closed something");
        assert!(!g.show_cloud_dev_panel);
        // A second Escape has nothing to close: the caller falls through to
        // the menu cascade, exactly as before the sidebar existed.
        assert!(!escape_closes_cloud_dev(&mut g));
    }

    #[test]
    fn escape_ignores_a_closed_or_collapsed_sidebar() {
        let mut g = closed();
        assert!(!escape_closes_cloud_dev(&mut g));
        assert!(!g.show_cloud_dev_panel);
        let mut g = collapsed();
        assert!(!escape_closes_cloud_dev(&mut g), "collapsed = the cursor is grabbed already; Escape means the menu");
        assert!(g.show_cloud_dev_panel && g.cloud_dev_collapsed, "Escape must not touch a collapsed sidebar");
    }

    #[test]
    fn f10_then_escape_round_trips_through_the_same_flags() {
        // The key and the IPC share these two functions, so this is the
        // exact sequence the runtime verifier sends: f10, escape, f10.
        let mut g = closed();
        toggle_cloud_dev_panel(&mut g);
        assert!(g.cloud_dev_sidebar_expanded());
        assert!(escape_closes_cloud_dev(&mut g));
        assert!(!g.cloud_dev_sidebar_expanded());
        toggle_cloud_dev_panel(&mut g);
        assert!(g.cloud_dev_sidebar_expanded());
    }

    /// Every `cloud_dev_*` field on GuiState (plus the panel's own two
    /// flags) must be readable by name, so the IPC can report any switch
    /// the sidebar grows. Scans the struct source rather than trusting a
    /// second hand-kept list.
    #[test]
    fn every_cloud_dev_field_is_readable_by_name() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gui/mod.rs"))
            .expect("read src/gui/mod.rs");
        let gui = GuiState::default();
        let mut names: Vec<&str> = vec!["show_cloud_dev_panel"];
        for line in src.lines() {
            let t = line.trim_start();
            if let Some(rest) = t.strip_prefix("pub cloud_dev_") {
                if let Some(end) = rest.find(':') {
                    // Reassemble the full field name from the prefix we split on.
                    let name = &t["pub ".len()..("pub ".len() + "cloud_dev_".len() + end)];
                    names.push(name);
                }
            }
        }
        assert!(names.len() > 40, "the scan found only {} cloud_dev fields; is the struct still in src/gui/mod.rs?", names.len());
        let missing: Vec<&str> = names.iter().copied().filter(|n| cloud_dev_flag(&gui, n).is_none()).collect();
        assert!(missing.is_empty(), "cloud_dev_flag has no row for: {missing:?}");
        assert!(cloud_dev_flag(&gui, "not_a_flag").is_none());
        // Types survive: a bool reads as a bool, a slider as a number.
        assert!(cloud_dev_flag(&gui, "cloud_dev_dither_off").unwrap().is_boolean());
        assert!(cloud_dev_flag(&gui, "cloud_dev_step_eco").unwrap().is_number());
        assert!(cloud_dev_flag(&gui, "cloud_dev_res_div").unwrap().is_number());
    }
}

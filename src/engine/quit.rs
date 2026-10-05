//! Leaving the app: what is written before it closes (first-hour audit 2026-10-04,
//! docs/design/first-hour-audit-2026-10-04.md, Blocker 5).
//!
//! A running game has more than one way out: the window's close button, the main menu
//! hub's Quit button and the updater's "Restart to Apply" (both of those last two set
//! `GuiState::quit_requested`, src/gui/pages/main_menu.rs and settings.rs), and the GPU
//! running out of memory. Only the close button saved. The others exited at once, so up to
//! two minutes of play since the periodic save were lost (`save_load::periodic_save_due`,
//! every 120 s), with any build edits since the editor's last autosave. Every way out of
//! the frame loop now runs `save_before_exit` first, and the test below reads the loop
//! (src/lib.rs) to keep it that way: a quit cannot be driven in a unit test.

use crate::engine::state::EngineState;

/// Save what the session changed, the way the window's close button always has: the build edits
/// not yet written (v0.791: quitting without the editor's Save button used to drop every wall,
/// light, strip and corridor edit since the last click; in the Dev mode they go to the data
/// files, in Normal and Creative into the character's own home, engine/own_home.rs), then the
/// active home (inventory, skills, crops, builds, the clock and the character's own home; only
/// the character while "Start every session from the default home" is on,
/// `save_load::save_active_home`). The edits first, so the save carries them, and a paid
/// machine removed in the last frame gives its item back before either.
pub(crate) fn save_before_exit(state: &mut EngineState) {
    crate::engine::own_home::refund_removed_machines(state);
    crate::engine::editor::autosave_ship_structure(state, true);
    crate::engine::own_home::refresh_own_home(&mut state.gui_state);
    crate::save_load::save_active_home(
        &state.game_world.world,
        &state.gui_state.placed_items,
        &state.data_store,
        !state.gui_state.settings.fresh_world_each_launch,
        state.gui_state.own_home.as_ref(),
    );
}

#[cfg(test)]
mod tests {
    /// The call every way out of the frame loop makes before it exits.
    const SAVE: &str = "crate::engine::quit::save_before_exit(state)";

    /// Every `event_loop.exit()` in the frame loop (src/lib.rs) has the save in the few lines
    /// before it. Read from the source because a quit cannot be driven in a unit test (the
    /// pattern of gui/connections.rs `only_the_chat_pages_connect_signs_up_again`).
    ///
    /// Seen red 2026-10-04 on 8e400d7ed (the close button saving inline, the hub's Quit, the
    /// updater's restart and the GPU's out of memory not at all): "the game exits without
    /// saving at src/lib.rs:2152 (`event_loop.exit();`, after `autosave_ship_structure(state,
    /// true);`), src/lib.rs:15639 (`event_loop.exit();`, after `if
    /// state.gui_state.quit_requested {`), src/lib.rs:16377 (`event_loop.exit();`, after
    /// `log::error!("Out of GPU memory");`)".
    #[test]
    fn every_way_out_of_the_game_saves_first() {
        let lib = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("read src/lib.rs");
        let lines: Vec<&str> = lib.lines().collect();
        let mut exits = 0;
        let mut unsaved = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains("event_loop.exit()") {
                continue;
            }
            exits += 1;
            let before = &lines[i.saturating_sub(6)..i];
            if !before.iter().any(|l| l.contains(SAVE)) {
                unsaved.push(format!("src/lib.rs:{} (`{}`, after `{}`)", i + 1, line.trim(), lines[i.saturating_sub(1)].trim()));
            }
        }
        // The close button, the quit request (the hub's Quit and the updater's restart) and
        // the GPU's out of memory: fewer means the scan stopped matching, not that they went.
        assert!(exits >= 3, "found only {exits} `event_loop.exit()` in src/lib.rs: the scan broke");
        assert!(unsaved.is_empty(), "the game exits without saving at {}", unsaved.join(", "));
        // And the quit request is still answered in the frame loop at all.
        assert!(lib.contains("state.gui_state.quit_requested"), "nothing in the frame loop answers the hub's Quit any more");
    }
}

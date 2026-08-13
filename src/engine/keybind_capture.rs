//! Keybind CAPTURE step (Settings > Controls, 2026-08-12), extracted from
//! lib.rs's raw winit keyboard arm per the monolith ratchet.
//!
//! While a bind cell on the Controls page is armed (`GuiState::keybind_capture`
//! is Some), the next key press becomes that bind:
//!   - Esc cancels the capture,
//!   - Delete clears the slot (unbinds),
//!   - reserved keys (the F row, Ctrl/Alt/OS modifiers) are refused with a
//!     visible reason (see `input::bindings::reserved_reason`),
//!   - a key another action holds arms `keybind_conflict` so the page can ask
//!     before moving it (never a silent double-bind),
//!   - anything else binds and marks the settings dirty (the render loop's
//!     settings_dirty block persists it via `AppConfig::from_gui_state`).
//!
//! Key identity is the KeyCode debug name ("Escape", "Delete", "KeyT"), the
//! same convention as the rest of the keymap, so this module needs no winit
//! import and its tests run headless.

use crate::gui::GuiState;
use crate::input::bindings::{reserved_reason, BindOutcome};

/// Apply one keyboard event to an armed capture. Returns true when the event
/// was CONSUMED (a capture was armed and this was a press): the caller must
/// then skip every other key handler for this event. Releases are never
/// consumed, so held-state trackers (voice push key, reveal peek, movement)
/// clear correctly while a capture is armed -- the same hygiene as the
/// in-world modal guard.
pub fn handle_keybind_capture(gui: &mut GuiState, key_name: &str, pressed: bool) -> bool {
    let Some((action, secondary)) = gui.keybind_capture else {
        return false;
    };
    if !pressed {
        return false;
    }
    match key_name {
        "Escape" => {
            gui.keybind_capture = None;
            gui.keybind_status.clear();
        }
        "Delete" => {
            gui.keybinds.clear_slot(action, secondary);
            gui.keybind_capture = None;
            gui.keybind_status.clear();
            gui.settings_dirty = true;
        }
        _ => {
            if let Some(reason) = reserved_reason(key_name) {
                gui.keybind_status = reason.to_string();
                gui.keybind_capture = None;
            } else {
                match gui.keybinds.try_bind(action, secondary, key_name) {
                    BindOutcome::Bound | BindOutcome::AlreadyBound => {
                        gui.keybind_capture = None;
                        // A game bind that shadows the voice push key still
                        // works, but both fire; say so instead of leaving the
                        // user to discover it mid-call.
                        if key_name == gui.voice_ptt_key {
                            gui.keybind_status = "Note: this key is also the voice push key \
                                                  (Settings > Audio), so both will trigger."
                                .to_string();
                        } else {
                            gui.keybind_status.clear();
                        }
                        gui.settings_dirty = true;
                    }
                    BindOutcome::Conflict(holder) => {
                        gui.keybind_conflict =
                            Some((action, secondary, key_name.to_string(), holder));
                        gui.keybind_capture = None;
                        gui.keybind_status.clear();
                    }
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::bindings::GameAction;

    fn armed(action: GameAction, secondary: bool) -> GuiState {
        let mut gui = GuiState::default();
        gui.keybind_capture = Some((action, secondary));
        gui.settings_dirty = false;
        gui
    }

    #[test]
    fn nothing_armed_consumes_nothing() {
        let mut gui = GuiState::default();
        gui.keybind_capture = None;
        assert!(!handle_keybind_capture(&mut gui, "KeyT", true));
    }

    #[test]
    fn escape_cancels_without_binding() {
        let mut gui = armed(GameAction::Inventory, false);
        assert!(handle_keybind_capture(&mut gui, "Escape", true));
        assert_eq!(gui.keybind_capture, None);
        assert!(gui.keybinds.is(GameAction::Inventory, "KeyI"), "bind unchanged");
        assert!(!gui.settings_dirty);
    }

    #[test]
    fn delete_clears_the_slot() {
        let mut gui = armed(GameAction::Inventory, false);
        assert!(handle_keybind_capture(&mut gui, "Delete", true));
        assert!(!gui.keybinds.is(GameAction::Inventory, "KeyI"));
        assert!(gui.settings_dirty, "an unbind must persist");
    }

    #[test]
    fn reserved_key_is_refused_with_a_reason() {
        let mut gui = armed(GameAction::Inventory, false);
        assert!(handle_keybind_capture(&mut gui, "F2", true));
        assert_eq!(gui.keybind_capture, None);
        assert!(!gui.keybind_status.is_empty(), "the user must see why");
        assert!(gui.keybinds.is(GameAction::Inventory, "KeyI"), "bind unchanged");
        assert!(!gui.settings_dirty);
    }

    #[test]
    fn free_key_binds_and_marks_dirty() {
        let mut gui = armed(GameAction::Inventory, false);
        assert!(handle_keybind_capture(&mut gui, "KeyT", true));
        assert!(gui.keybinds.is(GameAction::Inventory, "KeyT"));
        assert!(!gui.keybinds.is(GameAction::Inventory, "KeyI"));
        assert!(gui.settings_dirty);
        assert_eq!(gui.keybind_capture, None);
    }

    #[test]
    fn taken_key_raises_the_confirm_instead_of_binding() {
        let mut gui = armed(GameAction::Inventory, false);
        assert!(handle_keybind_capture(&mut gui, "KeyE", true)); // Interact's key
        assert!(gui.keybinds.is(GameAction::Interact, "KeyE"), "map unchanged");
        assert!(!gui.keybinds.is(GameAction::Inventory, "KeyE"));
        let (action, secondary, key, holder) =
            gui.keybind_conflict.clone().expect("conflict armed");
        assert_eq!(action, GameAction::Inventory);
        assert!(!secondary);
        assert_eq!(key, "KeyE");
        assert_eq!(holder, GameAction::Interact);
        assert!(!gui.settings_dirty);
    }

    #[test]
    fn releases_fall_through_unconsumed() {
        let mut gui = armed(GameAction::Inventory, false);
        assert!(!handle_keybind_capture(&mut gui, "KeyT", false));
        assert_eq!(gui.keybind_capture, Some((GameAction::Inventory, false)));
    }

    #[test]
    fn binding_the_voice_push_key_warns_but_binds() {
        let mut gui = armed(GameAction::Inventory, false);
        gui.voice_ptt_key = "KeyT".to_string();
        assert!(handle_keybind_capture(&mut gui, "KeyT", true));
        assert!(gui.keybinds.is(GameAction::Inventory, "KeyT"));
        assert!(gui.keybind_status.contains("voice push key"));
    }
}

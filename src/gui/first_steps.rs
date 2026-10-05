//! A new player's first steps in the world (first-hour audit 2026-10-04,
//! docs/design/first-hour-audit-2026-10-04.md, Friction 1: nothing teaches the game).
//!
//! The only tips anywhere were the last onboarding page's "Press Escape anytime to open the
//! menu. Press Enter to toggle chat." Nothing said that I opens the inventory, E uses what you
//! face, holding Alt frees the mouse so a card's buttons can be clicked (every in-world button
//! needs it), or that holding F1 lists every key. So the first time a player stands in the
//! world, one notice names those four, through the game's notice path (`pending_notices`, the
//! toast that stays up long enough to read), and the config remembers it was shown. The keys
//! named are the player's own binds (Settings > Controls); Alt and F1 are fixed keys.

use crate::gui::{GuiPage, GuiState, LauncherWhere};
use crate::input::bindings::{pretty_key_name, GameAction, Keybinds};

/// The key an action is bound to, as the player reads it ("I", "E"): its first key, else its
/// second; an action with neither says where to set one.
fn bound_key(kb: &Keybinds, action: GameAction) -> String {
    match kb.pair(action) {
        ("", "") => "its key (set one in Settings > Controls)".to_string(),
        ("", second) => pretty_key_name(second),
        (first, _) => pretty_key_name(first),
    }
}

/// The controls hint for these binds: the inventory and use keys as the player has them, then
/// the two fixed keys, hold Alt and hold F1 (input/bindings.rs `FIXED_BINDS`).
pub(crate) fn controls_hint(kb: &Keybinds) -> String {
    format!(
        "Press {} for your inventory and {} to use what you are facing. Hold Alt to free the mouse and click, and hold F1 to see every key.",
        bound_key(kb, GameAction::Inventory),
        bound_key(kb, GameAction::Interact),
    )
}

impl GuiState {
    /// Where onboarding starts a new player (first-hour audit 2026-10-04, Blocker 1): in their
    /// own home, ALONE, and that home is the pairing Play repeats (`record_pairing`: "home:" with
    /// no name, the homestead this install plays). The config saved next carries it
    /// (`last_world`), and boot reads it back as solo (config.rs `apply_to_gui_state`), so a
    /// relaunch stays out of the shared world too. Chat still connects by itself as before;
    /// joining a server's world is a pick in Characters (Open Net).
    pub fn start_at_home_alone(&mut self) {
        self.launcher_where_kind = LauncherWhere::Home;
        self.launcher_selected_world.clear();
        self.launcher_who.clear();
        self.copresence_solo = true;
        self.record_pairing();
    }

    /// Queue the controls hint on the first frame the player stands in the world, once ever
    /// (`controls_hint_shown`): the world loaded, onboarding done, and nothing over the view (a
    /// page, the character picker, the build editor, the passphrase prompt). True when it
    /// queued it: the caller then saves the config, so a relaunch does not show it again (a
    /// test never writes the real config).
    pub fn queue_controls_hint_once(&mut self, world_loaded: bool) -> bool {
        let standing_in_the_world = world_loaded
            && self.onboarding_complete
            && self.active_page == GuiPage::None
            && !self.showroom_active
            && !self.construction_active
            && !self.passphrase_needed;
        if self.controls_hint_shown || !standing_in_the_world {
            return false;
        }
        self.controls_hint_shown = true;
        self.pending_notices.push(controls_hint(&self.keybinds));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::controls_hint;
    use crate::config::AppConfig;
    use crate::gui::{GuiPage, GuiState};
    use crate::input::bindings::{GameAction, Keybinds};

    /// A player standing in the loaded world after onboarding, nothing over it.
    fn in_world() -> GuiState {
        let mut s = GuiState::default();
        s.onboarding_complete = true;
        s.active_page = GuiPage::None;
        s
    }

    /// The first entry into the world shows one notice naming I (inventory), E (use), Alt
    /// (free the mouse to click) and F1 (every key), and never a second time.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed (`queue_controls_hint_once` queuing nothing, as the
    /// game said nothing): "the first entry into the world showed no controls hint".
    #[test]
    fn the_first_entry_into_the_world_names_the_four_keys_once() {
        let mut s = in_world();
        assert!(s.queue_controls_hint_once(true), "the first entry into the world showed no controls hint");
        assert_eq!(s.pending_notices.len(), 1, "{:?}", s.pending_notices);
        let hint = s.pending_notices[0].clone();
        for words in ["Press I for your inventory", "E to use what you are facing", "Hold Alt to free the mouse", "hold F1 to see every key"] {
            assert!(hint.contains(words), "the controls hint does not say {words:?}: {hint}");
        }
        assert!(s.controls_hint_shown);
        assert!(!s.queue_controls_hint_once(true), "the controls hint is shown on every entry");
        assert_eq!(s.pending_notices.len(), 1, "shown twice: {:?}", s.pending_notices);
    }

    /// It waits until the player really stands in the world: not while the world loads, nor
    /// under a page, the character picker, the build editor, the passphrase prompt or the
    /// unfinished onboarding.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed (nothing ever queued): "never shown once the world is
    /// up"; and broken on purpose the same day with the picker check taken out: "shown while
    /// the character picker is open".
    #[test]
    fn the_controls_hint_waits_until_the_player_stands_in_the_world() {
        let covered: [(&str, fn(&mut GuiState)); 5] = [
            ("a page is open", |s| s.active_page = GuiPage::Chat),
            ("the character picker is open", |s| s.showroom_active = true),
            ("the build editor is open", |s| s.construction_active = true),
            ("the passphrase prompt is up", |s| s.passphrase_needed = true),
            ("onboarding is not finished", |s| {
                s.onboarding_complete = false;
                s.active_page = GuiPage::MainMenu;
            }),
        ];
        for (why, cover) in covered {
            let mut s = in_world();
            cover(&mut s);
            assert!(!s.queue_controls_hint_once(true), "shown while {why}");
            assert!(s.pending_notices.is_empty() && !s.controls_hint_shown, "spent while {why}");
        }
        let mut s = in_world();
        assert!(!s.queue_controls_hint_once(false), "shown before the world loaded");
        assert!(s.queue_controls_hint_once(true), "never shown once the world is up");
    }

    /// Shown once EVER: a relaunch reads it from the config and does not show it again.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed with `from_gui_state` writing `controls_hint_shown:
    /// false` and the load leg missing: "the shown flag is not written".
    #[test]
    fn the_controls_hint_stays_shown_after_a_relaunch() {
        let mut s = in_world();
        s.controls_hint_shown = true;
        let json = serde_json::to_string(&AppConfig::from_gui_state(&s)).expect("config serializes");
        assert!(json.contains("\"controls_hint_shown\":true"), "the shown flag is not written");
        let back: AppConfig = serde_json::from_str(&json).expect("config parses");
        let mut fresh = in_world();
        back.apply_to_gui_state(&mut fresh);
        fresh.onboarding_complete = true;
        fresh.active_page = GuiPage::None;
        assert!(fresh.controls_hint_shown, "the controls hint is shown again after a relaunch");
        assert!(!fresh.queue_controls_hint_once(true));
        // A config from before the flag existed: not shown yet, so it shows once.
        let old: AppConfig = serde_json::from_str("{}").expect("an old config parses");
        assert!(!old.controls_hint_shown);
    }

    /// The keys named are the player's own: a rebind in Settings > Controls changes the hint.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed (no hint text at all): `""`.
    #[test]
    fn the_controls_hint_names_the_players_own_keys() {
        let defaults = controls_hint(&Keybinds::default());
        assert!(defaults.starts_with("Press I for your inventory and E to use"), "{defaults:?}");
        let mut kb = Keybinds::default();
        kb.force_bind(GameAction::Inventory, false, "KeyT");
        kb.force_bind(GameAction::Interact, false, "KeyX");
        let rebound = controls_hint(&kb);
        assert!(rebound.contains("Press T for your inventory and X to use"), "{rebound}");
    }
}

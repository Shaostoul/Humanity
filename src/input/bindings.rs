//! Rebindable game keybinds (Settings > Controls, 2026-08-12).
//!
//! The single source of truth for which physical key drives which game
//! action. lib.rs's raw winit handler resolves every key event through
//! [`Keybinds`] instead of comparing against hardcoded `KeyCode`s, so a
//! player can rebind movement, interaction, camera, and world toggles from
//! Settings > Controls and the game honours it the same frame.
//!
//! Key identity convention: the `format!("{:?}", winit::keyboard::KeyCode)`
//! debug name, e.g. "KeyW", "Space", "ShiftLeft", "Digit1", "CapsLock".
//! This matches the voice push-to-talk key convention established in v0.490
//! (`GuiState::voice_ptt_key`), and keeps this module free of a winit
//! dependency so it compiles under every feature set (relay included,
//! because `AppConfig` carries the override list).
//!
//! Each action has a PRIMARY and an optional SECONDARY key. Defaults live in
//! [`ACTIONS`]; only NON-default binds persist to the config
//! ([`Keybinds::overrides`]), so a future default change reaches everyone who
//! has not overridden that action.
//!
//! Invariant: one key resolves to at most ONE action. [`Keybinds::try_bind`]
//! refuses a key another action holds (returning the holder so the UI can
//! offer a confirmed steal via [`Keybinds::force_bind`]), which is what makes
//! `action_for` a function and not a broadcast.
//!
//! NOT routed through this map (fixed, listed in [`FIXED_BINDS`] so the
//! Controls page can say so): Esc (menu/back/cancel), the F1..F12
//! dev/diagnostic row, hold-Alt free-look, the build-mode Ctrl shortcuts,
//! and Ctrl+V chat paste. Their handlers in lib.rs still compare raw
//! `KeyCode`s; converting them is tracked work, not silent debt.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Every rebindable game action. One variant per handler site that consults
/// the map; an action only exists here if code somewhere honours it (this is
/// why the list is code, not a data file: each variant is welded to an input
/// handler, exactly like a settings field is welded to its consumer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameAction {
    // Movement (held)
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    /// Space: jump on foot, ascend in dev flight.
    Jump,
    /// Shift: sprint on foot, descend in dev flight.
    Sprint,
    // Interaction
    /// E: talk / doors / machines / vehicles; banks right in dev flight.
    Interact,
    /// F: swing the held tool (or bare hands) at the faced creature.
    AttackSwing,
    Inventory,
    /// Tab (hold): peek hidden labels through walls.
    RevealPeek,
    /// Enter: open the in-world chat panel.
    OpenChat,
    // Hotbar (HUD ability bar slots 1..9)
    Hotbar1,
    Hotbar2,
    Hotbar3,
    Hotbar4,
    Hotbar5,
    Hotbar6,
    Hotbar7,
    Hotbar8,
    Hotbar9,
    // Camera
    /// V: toggle first / third person. (Was F or V; F now belongs to
    /// AttackSwing alone, so one key means one thing.)
    ToggleView,
    /// M: enter / leave the orbit camera.
    OrbitCamera,
    /// O: perspective / orthographic toggle (orbit camera only).
    ToggleOrtho,
    /// Q: swap the third-person shoulder; banks left in dev flight.
    ShoulderSwap,
    // World / build
    /// B: open / close the construction editor.
    BuildEditor,
    /// R: show / hide the home roof.
    ToggleRoof,
    /// H: show / hide the ship hull wrap.
    ToggleHull,
    /// G: walk-up creature editor (Dev play mode + cheats only).
    CreatureEditor,
}

/// Static description of one action: stable config id, UI label, UI group,
/// and the default keys. `default_secondary` empty = no secondary default.
pub struct ActionInfo {
    pub action: GameAction,
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub default_primary: &'static str,
    pub default_secondary: &'static str,
}

/// The canonical action table, in the order the Controls page displays.
pub const ACTIONS: &[ActionInfo] = &[
    ActionInfo { action: GameAction::MoveForward, id: "move_forward", label: "Move forward", category: "Movement", default_primary: "KeyW", default_secondary: "" },
    ActionInfo { action: GameAction::MoveBack, id: "move_back", label: "Move back", category: "Movement", default_primary: "KeyS", default_secondary: "" },
    ActionInfo { action: GameAction::MoveLeft, id: "move_left", label: "Move left", category: "Movement", default_primary: "KeyA", default_secondary: "" },
    ActionInfo { action: GameAction::MoveRight, id: "move_right", label: "Move right", category: "Movement", default_primary: "KeyD", default_secondary: "" },
    ActionInfo { action: GameAction::Jump, id: "jump", label: "Jump / fly up", category: "Movement", default_primary: "Space", default_secondary: "" },
    ActionInfo { action: GameAction::Sprint, id: "sprint", label: "Sprint / fly down", category: "Movement", default_primary: "ShiftLeft", default_secondary: "ShiftRight" },
    ActionInfo { action: GameAction::Interact, id: "interact", label: "Interact / talk", category: "Interaction", default_primary: "KeyE", default_secondary: "" },
    ActionInfo { action: GameAction::AttackSwing, id: "attack_swing", label: "Swing tool / attack", category: "Interaction", default_primary: "KeyF", default_secondary: "" },
    ActionInfo { action: GameAction::Inventory, id: "inventory", label: "Inventory", category: "Interaction", default_primary: "KeyI", default_secondary: "" },
    ActionInfo { action: GameAction::RevealPeek, id: "reveal_peek", label: "Reveal labels (hold)", category: "Interaction", default_primary: "Tab", default_secondary: "" },
    ActionInfo { action: GameAction::OpenChat, id: "open_chat", label: "Open chat panel", category: "Interaction", default_primary: "Enter", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar1, id: "hotbar_1", label: "Ability 1", category: "Hotbar", default_primary: "Digit1", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar2, id: "hotbar_2", label: "Ability 2", category: "Hotbar", default_primary: "Digit2", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar3, id: "hotbar_3", label: "Ability 3", category: "Hotbar", default_primary: "Digit3", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar4, id: "hotbar_4", label: "Ability 4", category: "Hotbar", default_primary: "Digit4", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar5, id: "hotbar_5", label: "Ability 5", category: "Hotbar", default_primary: "Digit5", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar6, id: "hotbar_6", label: "Ability 6", category: "Hotbar", default_primary: "Digit6", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar7, id: "hotbar_7", label: "Ability 7", category: "Hotbar", default_primary: "Digit7", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar8, id: "hotbar_8", label: "Ability 8", category: "Hotbar", default_primary: "Digit8", default_secondary: "" },
    ActionInfo { action: GameAction::Hotbar9, id: "hotbar_9", label: "Ability 9", category: "Hotbar", default_primary: "Digit9", default_secondary: "" },
    ActionInfo { action: GameAction::ToggleView, id: "toggle_view", label: "First / third person", category: "Camera", default_primary: "KeyV", default_secondary: "" },
    ActionInfo { action: GameAction::OrbitCamera, id: "orbit_camera", label: "Orbit camera", category: "Camera", default_primary: "KeyM", default_secondary: "" },
    ActionInfo { action: GameAction::ToggleOrtho, id: "toggle_ortho", label: "Ortho view (orbit cam)", category: "Camera", default_primary: "KeyO", default_secondary: "" },
    ActionInfo { action: GameAction::ShoulderSwap, id: "shoulder_swap", label: "Shoulder swap / roll", category: "Camera", default_primary: "KeyQ", default_secondary: "" },
    ActionInfo { action: GameAction::BuildEditor, id: "build_editor", label: "Construction editor", category: "World", default_primary: "KeyB", default_secondary: "" },
    ActionInfo { action: GameAction::ToggleRoof, id: "toggle_roof", label: "Toggle roof", category: "World", default_primary: "KeyR", default_secondary: "" },
    ActionInfo { action: GameAction::ToggleHull, id: "toggle_hull", label: "Toggle hull", category: "World", default_primary: "KeyH", default_secondary: "" },
    ActionInfo { action: GameAction::CreatureEditor, id: "creature_editor", label: "Creature editor (dev)", category: "World", default_primary: "KeyG", default_secondary: "" },
];

/// Display order of the category groups on the Controls page.
pub const CATEGORIES: &[&str] = &["Movement", "Interaction", "Camera", "World", "Hotbar"];

/// Hotbar actions in slot order, so lib.rs can map a key press to a slot
/// index without spelling out nine comparisons.
pub const HOTBAR: &[GameAction] = &[
    GameAction::Hotbar1,
    GameAction::Hotbar2,
    GameAction::Hotbar3,
    GameAction::Hotbar4,
    GameAction::Hotbar5,
    GameAction::Hotbar6,
    GameAction::Hotbar7,
    GameAction::Hotbar8,
    GameAction::Hotbar9,
];

/// Fixed (not rebindable) shortcuts, shown on the Controls page so the list
/// there is a truthful, complete reference: (keys, what it does). Converting
/// one of these to a rebindable action means moving its row into [`ACTIONS`]
/// and routing its lib.rs handler through the map.
pub const FIXED_BINDS: &[(&str, &str)] = &[
    ("Esc", "Menu / back / close panel; cancels a key capture"),
    ("F1 (hold)", "Keymap overlay for the current screen"),
    ("F2", "Performance overlay (FPS, frame graph)"),
    ("F3", "Network overlay"),
    ("F4", "System overlay (RAM, uptime)"),
    ("F6", "Save location bookmark (dev)"),
    ("F9", "Dev flight on / off (cheats)"),
    ("F10", "Cloud dev panel (dev)"),
    ("F11", "Weather panel"),
    ("F12", "Debug console"),
    ("Alt (hold)", "Free the cursor while in-world"),
    ("Ctrl+Z / Ctrl+Y", "Undo / redo (construction editor)"),
    ("Ctrl+D", "Duplicate selection (construction editor)"),
    ("[ / ]", "Rotate held piece 15 degrees (construction editor)"),
    ("Ctrl+V", "Paste a clipboard image (Chat page)"),
    ("Enter", "Send message (while the chat input is open)"),
    ("1-9", "See Hotbar binds above; cast HUD abilities"),
];

/// Keys the capture flow refuses outright, with the reason shown to the user.
/// Esc and Delete are capture controls (cancel / clear); the F row is the
/// dev/diagnostic surface whose handlers run before the game keymap; Ctrl,
/// Alt and the OS key are shortcut modifiers tracked separately in lib.rs, so
/// binding a game action to them would double-fire. Shift stays bindable on
/// purpose: it IS the default sprint key.
pub fn reserved_reason(key_name: &str) -> Option<&'static str> {
    match key_name {
        "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "F10" | "F11" | "F12" => {
            Some("Function keys are reserved for the dev and diagnostic tools.")
        }
        "ControlLeft" | "ControlRight" | "SuperLeft" | "SuperRight" => {
            Some("Ctrl and the OS key are shortcut modifiers and cannot be game binds.")
        }
        "AltLeft" | "AltRight" => {
            Some("Alt is the hold-to-free-the-cursor key and cannot be a game bind.")
        }
        _ => None,
    }
}

/// One persisted override row: the action id plus the full bind pair as the
/// user left it. Only actions that differ from their defaults are stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct KeybindOverride {
    pub action: String,
    #[serde(default)]
    pub primary: String,
    #[serde(default)]
    pub secondary: String,
}

/// Outcome of a bind attempt. `Conflict` carries the action currently
/// holding the key so the UI can name it and offer a confirmed steal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindOutcome {
    Bound,
    /// The key is already on this same action (either slot); nothing changed
    /// beyond possibly moving it between slots.
    AlreadyBound,
    Conflict(GameAction),
}

/// The live bind map: every action to its (primary, secondary) key names.
/// Empty string = that slot is unbound.
#[derive(Debug, Clone, PartialEq)]
pub struct Keybinds {
    binds: HashMap<GameAction, (String, String)>,
}

impl Default for Keybinds {
    fn default() -> Self {
        let mut binds = HashMap::with_capacity(ACTIONS.len());
        for info in ACTIONS {
            binds.insert(
                info.action,
                (info.default_primary.to_string(), info.default_secondary.to_string()),
            );
        }
        Self { binds }
    }
}

impl Keybinds {
    /// Static info row for an action (label, defaults, id).
    pub fn info(action: GameAction) -> &'static ActionInfo {
        ACTIONS
            .iter()
            .find(|i| i.action == action)
            .expect("every GameAction variant has an ACTIONS row")
    }

    /// Look up an action by its stable config id.
    pub fn action_by_id(id: &str) -> Option<GameAction> {
        ACTIONS.iter().find(|i| i.id == id).map(|i| i.action)
    }

    /// The bind pair for an action: (primary, secondary), "" = unbound slot.
    pub fn pair(&self, action: GameAction) -> (&str, &str) {
        self.binds
            .get(&action)
            .map(|(p, s)| (p.as_str(), s.as_str()))
            .unwrap_or(("", ""))
    }

    /// Does `key_name` currently trigger `action`? Consults the LIVE map
    /// (defaults + user overrides), never the defaults table directly; the
    /// red-proof for this is `rebind_changes_what_the_resolver_returns`,
    /// which fails if resolution ignores the override map.
    pub fn is(&self, action: GameAction, key_name: &str) -> bool {
        let (p, s) = self.pair(action);
        (!key_name.is_empty()) && (p == key_name || s == key_name)
    }

    /// The single action `key_name` resolves to, if any. One key holds at
    /// most one action (try_bind/force_bind maintain the invariant).
    pub fn action_for(&self, key_name: &str) -> Option<GameAction> {
        if key_name.is_empty() {
            return None;
        }
        // Deliberately iterate ACTIONS (stable order) rather than the map, so
        // resolution is deterministic even if the invariant is ever violated
        // by a hand-edited config.
        ACTIONS.iter().map(|i| i.action).find(|a| self.is(*a, key_name))
    }

    /// Hotbar slot index (0-based) for a key, if it is bound to one.
    pub fn hotbar_slot(&self, key_name: &str) -> Option<usize> {
        HOTBAR.iter().position(|a| self.is(*a, key_name))
    }

    /// Attempt to put `key_name` in `action`'s primary or secondary slot.
    /// Refuses (without changing anything) when another action holds the key.
    /// If the SAME action already holds the key in its other slot, the key
    /// moves to the requested slot (the old slot clears).
    pub fn try_bind(&mut self, action: GameAction, secondary: bool, key_name: &str) -> BindOutcome {
        if key_name.is_empty() {
            return BindOutcome::AlreadyBound; // nothing to do
        }
        if let Some(holder) = self.action_for(key_name) {
            if holder != action {
                return BindOutcome::Conflict(holder);
            }
            // Same action: move between slots (or no-op if already there).
            let (p, s) = self.pair(action);
            let already_there = if secondary { s == key_name } else { p == key_name };
            if already_there {
                return BindOutcome::AlreadyBound;
            }
            let entry = self.binds.entry(action).or_default();
            if secondary {
                entry.0 = String::new();
                entry.1 = key_name.to_string();
            } else {
                entry.1 = String::new();
                entry.0 = key_name.to_string();
            }
            return BindOutcome::AlreadyBound;
        }
        let entry = self.binds.entry(action).or_default();
        if secondary {
            entry.1 = key_name.to_string();
        } else {
            entry.0 = key_name.to_string();
        }
        BindOutcome::Bound
    }

    /// Bind unconditionally: first strip `key_name` from whatever action
    /// holds it, then set it. This is the confirmed side of a Conflict.
    pub fn force_bind(&mut self, action: GameAction, secondary: bool, key_name: &str) {
        if key_name.is_empty() {
            return;
        }
        for (_, pair) in self.binds.iter_mut() {
            if pair.0 == key_name {
                pair.0 = String::new();
            }
            if pair.1 == key_name {
                pair.1 = String::new();
            }
        }
        let entry = self.binds.entry(action).or_default();
        if secondary {
            entry.1 = key_name.to_string();
        } else {
            entry.0 = key_name.to_string();
        }
    }

    /// Clear one slot of an action (unbind).
    pub fn clear_slot(&mut self, action: GameAction, secondary: bool) {
        if let Some(entry) = self.binds.get_mut(&action) {
            if secondary {
                entry.1 = String::new();
            } else {
                entry.0 = String::new();
            }
        }
    }

    /// Is this action still on its defaults?
    pub fn is_default(&self, action: GameAction) -> bool {
        let info = Self::info(action);
        let (p, s) = self.pair(action);
        p == info.default_primary && s == info.default_secondary
    }

    /// Reset one action to its defaults. If a default key is now held by
    /// ANOTHER action (the user moved it there), the reset strips it from
    /// that holder first, keeping the one-key-one-action invariant.
    pub fn reset(&mut self, action: GameAction) {
        let info = Self::info(action);
        // Take the keys back from any current holder.
        let (dp, ds) = (info.default_primary.to_string(), info.default_secondary.to_string());
        for key in [dp.as_str(), ds.as_str()] {
            if key.is_empty() {
                continue;
            }
            for (_, pair) in self.binds.iter_mut() {
                if pair.0 == key {
                    pair.0 = String::new();
                }
                if pair.1 == key {
                    pair.1 = String::new();
                }
            }
        }
        self.binds.insert(action, (dp, ds));
    }

    /// Reset every action to its defaults.
    pub fn reset_all(&mut self) {
        *self = Self::default();
    }

    /// The persisted form: only actions that differ from their defaults.
    pub fn overrides(&self) -> Vec<KeybindOverride> {
        ACTIONS
            .iter()
            .filter(|i| !self.is_default(i.action))
            .map(|i| {
                let (p, s) = self.pair(i.action);
                KeybindOverride {
                    action: i.id.to_string(),
                    primary: p.to_string(),
                    secondary: s.to_string(),
                }
            })
            .collect()
    }

    /// Apply persisted overrides on top of the defaults. Unknown action ids
    /// (a removed action, a hand-edited config) are ignored. A key claimed by
    /// an override is stripped from whichever default held it, keeping the
    /// one-key-one-action invariant no matter what the config says.
    pub fn apply_overrides(&mut self, list: &[KeybindOverride]) {
        for ov in list {
            let Some(action) = Self::action_by_id(&ov.action) else {
                continue;
            };
            // Free both incoming keys from any current holder, then set the
            // pair exactly as stored (empty strings clear slots).
            for key in [ov.primary.as_str(), ov.secondary.as_str()] {
                if key.is_empty() {
                    continue;
                }
                for (_, pair) in self.binds.iter_mut() {
                    if pair.0 == key {
                        pair.0 = String::new();
                    }
                    if pair.1 == key {
                        pair.1 = String::new();
                    }
                }
            }
            self.binds.insert(action, (ov.primary.clone(), ov.secondary.clone()));
        }
    }
}

/// Human-friendly display for a stored key name: "KeyW" -> "W",
/// "Digit1" -> "1", "ShiftLeft" -> "Left Shift", "Space" stays "Space".
/// Mirrors `config::pretty_ptt_key_name` for the voice key; kept here so the
/// Controls page and any future overlay share one prettifier.
pub fn pretty_key_name(raw: &str) -> String {
    if raw.is_empty() {
        return "(none)".to_string();
    }
    if let Some(rest) = raw.strip_prefix("Key") {
        return rest.to_string();
    }
    if let Some(rest) = raw.strip_prefix("Digit") {
        return rest.to_string();
    }
    match raw {
        "ShiftLeft" => "Left Shift".to_string(),
        "ShiftRight" => "Right Shift".to_string(),
        "ControlLeft" => "Left Ctrl".to_string(),
        "ControlRight" => "Right Ctrl".to_string(),
        "AltLeft" => "Left Alt".to_string(),
        "AltRight" => "Right Alt".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        "ArrowUp" => "Up".to_string(),
        "ArrowDown" => "Down".to_string(),
        "ArrowLeft" => "Left".to_string(),
        "ArrowRight" => "Right".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults resolve: the shipped map answers the keys the handlers
    /// actually receive (the KeyCode debug names).
    #[test]
    fn defaults_resolve() {
        let kb = Keybinds::default();
        assert!(kb.is(GameAction::MoveForward, "KeyW"));
        assert!(kb.is(GameAction::Inventory, "KeyI"));
        assert!(kb.is(GameAction::Sprint, "ShiftLeft"));
        // Secondary slot works: both shifts sprint, like the hardcoded era.
        assert!(kb.is(GameAction::Sprint, "ShiftRight"));
        assert_eq!(kb.action_for("KeyE"), Some(GameAction::Interact));
        assert_eq!(kb.action_for("Escape"), None, "Esc is fixed, never a game bind");
        assert_eq!(kb.hotbar_slot("Digit3"), Some(2));
        assert_eq!(kb.hotbar_slot("KeyW"), None);
    }

    /// One key, one action: no default key appears twice across the table.
    /// (This is what makes `action_for` well-defined, and it is the invariant
    /// try_bind/force_bind maintain at runtime.)
    #[test]
    fn no_default_key_is_shared() {
        let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
        for info in ACTIONS {
            for key in [info.default_primary, info.default_secondary] {
                if key.is_empty() {
                    continue;
                }
                if let Some(prev) = seen.insert(key, info.id) {
                    panic!("default key {key} is bound to both {prev} and {}", info.id);
                }
            }
        }
    }

    /// THE REBIND TEST: after binding Inventory to T, the resolver must
    /// answer T and stop answering I. This is the test that fails when the
    /// override map is ignored (resolution reading only the defaults table).
    #[test]
    fn rebind_changes_what_the_resolver_returns() {
        let mut kb = Keybinds::default();
        assert_eq!(kb.try_bind(GameAction::Inventory, false, "KeyT"), BindOutcome::Bound);
        assert!(kb.is(GameAction::Inventory, "KeyT"), "new bind must resolve");
        assert!(!kb.is(GameAction::Inventory, "KeyI"), "old default must stop resolving");
        assert_eq!(kb.action_for("KeyT"), Some(GameAction::Inventory));
        assert_eq!(kb.action_for("KeyI"), None);
    }

    /// A key another action holds is refused and NOTHING changes.
    #[test]
    fn conflicting_bind_is_refused_and_map_unchanged() {
        let mut kb = Keybinds::default();
        let before = kb.clone();
        // KeyE belongs to Interact; Inventory may not silently take it.
        assert_eq!(
            kb.try_bind(GameAction::Inventory, false, "KeyE"),
            BindOutcome::Conflict(GameAction::Interact)
        );
        assert_eq!(kb, before, "a refused bind must not corrupt the map");
        // The confirmed steal moves the key and clears the old holder.
        kb.force_bind(GameAction::Inventory, false, "KeyE");
        assert_eq!(kb.action_for("KeyE"), Some(GameAction::Inventory));
        assert!(!kb.is(GameAction::Interact, "KeyE"));
    }

    /// Overrides round-trip: what `overrides()` stores, `apply_overrides()`
    /// restores, including through the serde JSON the config file uses.
    #[test]
    fn overrides_round_trip_through_config_form() {
        let mut kb = Keybinds::default();
        kb.force_bind(GameAction::Inventory, false, "KeyT");
        kb.force_bind(GameAction::Sprint, true, "CapsLock"); // secondary slot
        kb.clear_slot(GameAction::ToggleRoof, false);

        let stored = kb.overrides();
        // Only the changed actions persist; untouched ones ride the defaults.
        assert!(stored.iter().any(|o| o.action == "inventory"));
        assert!(!stored.iter().any(|o| o.action == "move_forward"));

        let json = serde_json::to_string(&stored).expect("serialize");
        let parsed: Vec<KeybindOverride> = serde_json::from_str(&json).expect("parse");

        let mut fresh = Keybinds::default();
        fresh.apply_overrides(&parsed);
        assert_eq!(fresh, kb, "a restart must restore the exact same map");
        assert!(fresh.is(GameAction::Sprint, "CapsLock"));
        assert!(!fresh.is(GameAction::ToggleRoof, "KeyR"));
    }

    /// Reset returns an action to defaults even when its default key has
    /// wandered onto another action, and reset_all clears everything.
    #[test]
    fn reset_reclaims_default_keys() {
        let mut kb = Keybinds::default();
        kb.force_bind(GameAction::ToggleRoof, false, "KeyI"); // steal I from Inventory
        assert!(!kb.is(GameAction::Inventory, "KeyI"));
        kb.reset(GameAction::Inventory);
        assert!(kb.is(GameAction::Inventory, "KeyI"));
        assert!(!kb.is(GameAction::ToggleRoof, "KeyI"), "reset strips the key back");
        kb.reset_all();
        assert_eq!(kb, Keybinds::default());
    }

    /// Reserved keys are refused with a reason; Shift is NOT reserved
    /// (it is the sprint default).
    #[test]
    fn reserved_keys_have_reasons() {
        assert!(reserved_reason("F2").is_some());
        assert!(reserved_reason("ControlLeft").is_some());
        assert!(reserved_reason("AltRight").is_some());
        assert!(reserved_reason("ShiftLeft").is_none());
        assert!(reserved_reason("KeyW").is_none());
    }

    /// Pretty names for the UI.
    #[test]
    fn pretty_names() {
        assert_eq!(pretty_key_name("KeyW"), "W");
        assert_eq!(pretty_key_name("Digit1"), "1");
        assert_eq!(pretty_key_name("ShiftLeft"), "Left Shift");
        assert_eq!(pretty_key_name("Space"), "Space");
        assert_eq!(pretty_key_name(""), "(none)");
    }
}

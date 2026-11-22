//! What the death screen shows of where you can wake, and what it asked for
//! (docs/design/game-setting-and-intro.md 5.6, rung 2, 2026-11-21).
//!
//! The engine fills `places` once per death from the respawners the ship and the home hold
//! (src/engine/wake.rs `refresh_view`, crates/humanity-ship/src/respawners.rs), the player's usual
//! place first; the death screen (gui/pages/hud.rs `draw_death_screen`) lists them, and a press
//! sets `asked` and asks for the respawn. Kept in its own file because src/gui/mod.rs is at its
//! size budget (tests/file_size_ratchet.rs).

/// One place the dead player may wake, as the death screen lists it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WakeEntry {
    /// The place id: "home", or the zone id of a public respawner.
    pub place: String,
    /// The words for it: "your home's respawner", or the zone's label ("General Services").
    pub name: String,
    /// True for the player's usual place (the save's `wake_at`), listed first.
    pub usual: bool,
    /// Why this place's pod is not drawn in this household's world, said under its button (the key
    /// `hud.wake_not_drawn`, engine/wake.rs `entries`); None when it is drawn.
    pub note: Option<String>,
}

/// The death screen's list of where to wake.
#[derive(Debug, Clone, Default)]
pub struct WakeView {
    /// Where the player may wake, the usual place first. Empty when the ship has no respawner
    /// they may use: then they wake at their front door, as before respawners.
    pub places: Vec<WakeEntry>,
    /// True once `places` is filled for this death (it is filled once per death, and emptied at
    /// the respawn).
    pub filled: bool,
    /// The place the player pressed on the death screen, read by the respawn.
    pub asked: Option<String>,
    /// The `wake` field the next `game_join` carries (engine/wake.rs `join_field`): set at a
    /// respawn in the shared world, taken by the join it causes, so the relay stands the player at
    /// the respawner they chose rather than at their door.
    pub pending_join: Option<serde_json::Value>,
    /// The place the last respawn woke the player at, and where their eye was put (the probe's
    /// `wake` block, engine/wake.rs `probe_json`). None until a respawn at a respawner.
    pub last: Option<(String, glam::Vec3)>,
}

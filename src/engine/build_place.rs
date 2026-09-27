//! Placing a built piece in the world (2026-09-27): the ghost, the keys, the
//! hint.
//!
//! Build on the Crafting page used to drop the piece 4 m ahead on the floor,
//! unturned, with no preview, so walls only ran east-west and a roof could
//! never go overhead. Now Build puts the blueprint IN HAND: the page closes,
//! a see-through scaffold (the ghost) follows the crosshair, the Toggle roof
//! key (R by default) turns it a quarter, Interact (E) builds it where the
//! ghost stands and keeps the piece in hand for the next one, and Esc puts
//! it down. Where it lands is `construction::placement::placement_pose`, the
//! same function the ConstructionSystem builds with, so the ghost is where
//! the piece goes: x and z on the metre grid, y on the floor, or on top of
//! the walls for a roof (`mount: OnTop` in the blueprint data).
//!
//! R is the roof toggle when nothing is in hand; while placing it turns the
//! piece (the Controls page says so), because one key holds one action and R
//! is where every building game puts rotate. lib.rs calls `frame` once a
//! frame and `key` from its key handler, ahead of the E chain and the menu
//! Escape.

use crate::ecs::components::Transform;
use crate::engine::state::EngineState;
use crate::gui::{BuildPlacing, GuiPage, GuiState};
use crate::input::bindings::{pretty_key_name, GameAction};
use crate::systems::construction::{placement, BlueprintRegistry, BuildRequest};

/// What a key press does to the piece in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaceKey {
    /// Build it where the ghost stands (Interact).
    Build,
    /// Turn it a quarter (the Toggle roof key).
    Turn,
    /// Put it down (Esc).
    Stop,
}

/// The placement meaning of a key press, or None when nothing is in hand, a
/// page is open, or the key is not one of placing's (so it keeps its usual
/// meaning). Pure over the GUI state, so it is tested without a window.
pub(crate) fn key_action(gui: &GuiState, key_name: &str, escape: bool) -> Option<PlaceKey> {
    if gui.build_placing.is_none() || gui.active_page != GuiPage::None {
        return None;
    }
    if escape {
        Some(PlaceKey::Stop)
    } else if gui.keybinds.is(GameAction::ToggleRoof, key_name) {
        Some(PlaceKey::Turn)
    } else if gui.keybinds.is(GameAction::Interact, key_name) {
        Some(PlaceKey::Build)
    } else {
        None
    }
}

/// A key press (lib.rs's key handler, presses only). Returns true when
/// placing used the key, so the handler stops there. A held key's repeats
/// are swallowed without acting, so holding E cannot build a stack of walls
/// in one spot.
pub(crate) fn key(state: &mut EngineState, key_name: &str, escape: bool, repeat: bool) -> bool {
    let Some(action) = key_action(&state.gui_state, key_name, escape) else { return false };
    if repeat {
        return true;
    }
    match action {
        PlaceKey::Stop => state.gui_state.build_placing = None,
        PlaceKey::Turn => {
            if let Some(p) = state.gui_state.build_placing.as_mut() {
                p.quarter_turns = (p.quarter_turns + 1) % 4;
            }
        }
        PlaceKey::Build => {
            // Only where a ghost stands: in first person, not driving.
            let Some(p) = state.gui_state.build_placing.as_ref().filter(|p| p.ghost.is_some()) else { return false };
            let request = BuildRequest {
                blueprint_id: p.blueprint_id.clone(),
                at: aim(state),
                quarter_turns: p.quarter_turns,
            };
            if let Some(chan) = state.data_store.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request") {
                if let Ok(mut c) = chan.lock() {
                    c.push(request);
                }
            }
        }
    }
    true
}

/// Once a frame: pick up what Build was clicked on, drop it when the player
/// can no longer place (the build editor, the showroom, death), and move the
/// ghost and the hint.
pub(crate) fn frame(state: &mut EngineState) {
    if let Some(id) = state.gui_state.pending_build.take() {
        let name = state
            .data_store
            .get::<BlueprintRegistry>("blueprint_registry")
            .and_then(|r| r.get(&id))
            .map_or_else(|| id.clone(), |bp| bp.name.clone());
        // Picking up another piece keeps the turn: the next wall of a room
        // usually runs the way the last one did.
        let quarter_turns = state.gui_state.build_placing.as_ref().map_or(0, |p| p.quarter_turns);
        state.gui_state.build_placing =
            Some(BuildPlacing { blueprint_id: id, name, quarter_turns, ghost: None, hint: String::new() });
        state.gui_state.active_page = GuiPage::None;
    }
    let g = &state.gui_state;
    if g.construction_active || g.showroom_active || g.player_death_cause.is_some() {
        state.gui_state.build_placing = None;
        return;
    }
    let Some(p) = state.gui_state.build_placing.as_ref() else { return };
    let can_place = state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson && state.driving_vehicle.is_none();
    let ghost = if can_place { ghost_pose(state, p) } else { None };
    let floor = floor_y(state);
    let keys = &state.gui_state.keybinds;
    let hint = placing_hint(
        &p.name,
        p.quarter_turns,
        ghost.as_ref().map(|g| g.position.y - floor),
        &pretty_key_name(keys.pair(GameAction::Interact).0),
        &pretty_key_name(keys.pair(GameAction::ToggleRoof).0),
    );
    if let Some(p) = state.gui_state.build_placing.as_mut() {
        p.ghost = ghost;
        p.hint = hint;
    }
}

/// The line under the crosshair while placing. `above_floor` is how high the
/// ghost rests above the floor (a roof on walls), None when there is no
/// ghost because the player cannot place from here.
pub(crate) fn placing_hint(name: &str, quarter_turns: u8, above_floor: Option<f32>, build_key: &str, turn_key: &str) -> String {
    let Some(up) = above_floor else {
        return format!("Placing {name}: go to first person, on foot, to place it   [Esc] done");
    };
    let turned = match quarter_turns % 4 {
        0 => String::new(),
        q => format!(", turned {} degrees", u32::from(q) * 90),
    };
    let on_top = if up > 0.01 { format!(", on top at {up:.1} m") } else { String::new() };
    format!("Placing {name}{turned}{on_top}   [{build_key}] build here   [{turn_key}] turn   [Esc] done")
}

/// Where the piece in hand would be built this frame.
fn ghost_pose(state: &EngineState, p: &BuildPlacing) -> Option<Transform> {
    let reg = state.data_store.get::<BlueprintRegistry>("blueprint_registry")?;
    let bp = reg.get(&p.blueprint_id)?;
    Some(placement::placement_pose(bp, aim(state), p.quarter_turns, &state.game_world.world, reg))
}

/// The floor point the crosshair is on.
fn aim(state: &EngineState) -> glam::Vec3 {
    placement::aim_point(state.camera.position, state.camera.forward(), floor_y(state))
}

/// The floor under the player: the room floor the controller rests on while
/// aboard (the old build placed at world y 0, which is that floor in the
/// home), else the feet.
fn floor_y(state: &EngineState) -> f32 {
    if state.aboard_station {
        state.controller.ground_floor()
    } else {
        state.camera.position.y - state.controller.eye_height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding() -> GuiState {
        let mut g = GuiState::default();
        g.active_page = GuiPage::None;
        g.build_placing = Some(BuildPlacing {
            blueprint_id: "wood_wall".into(),
            name: "Wood Wall".into(),
            quarter_turns: 0,
            ghost: None,
            hint: String::new(),
        });
        g
    }

    /// With a piece in hand and no page open, E builds, R turns and Esc
    /// stops; a rebound roof key turns instead of R; with nothing in hand or
    /// a page open, no key is placing's. Red check, run: dropping the
    /// `build_placing.is_none()` guard makes R turn nothing-in-hand and the
    /// last assertion fails.
    #[test]
    fn placing_keys_follow_the_binds_and_only_while_placing() {
        let mut g = holding();
        assert_eq!(key_action(&g, "KeyE", false), Some(PlaceKey::Build));
        assert_eq!(key_action(&g, "KeyR", false), Some(PlaceKey::Turn));
        assert_eq!(key_action(&g, "Escape", true), Some(PlaceKey::Stop));
        assert_eq!(key_action(&g, "KeyW", false), None, "walking still walks");
        g.keybinds.force_bind(GameAction::ToggleRoof, false, "KeyT");
        assert_eq!(key_action(&g, "KeyT", false), Some(PlaceKey::Turn));
        assert_eq!(key_action(&g, "KeyR", false), None);
        g.active_page = GuiPage::Inventory;
        assert_eq!(key_action(&g, "KeyE", false), None, "a page open: keys are the page's");
        let mut empty = holding();
        empty.build_placing = None;
        assert_eq!(key_action(&empty, "KeyR", false), None, "nothing in hand: R toggles the roof");
    }

    /// The hint names the piece, its turn, whether it rests on top, and the
    /// live keys.
    #[test]
    fn the_hint_says_the_turn_and_the_keys() {
        assert_eq!(
            placing_hint("Wood Wall", 1, Some(0.0), "E", "R"),
            "Placing Wood Wall, turned 90 degrees   [E] build here   [R] turn   [Esc] done"
        );
        assert_eq!(
            placing_hint("Wood Roof", 4, Some(3.0), "E", "T"),
            "Placing Wood Roof, on top at 3.0 m   [E] build here   [T] turn   [Esc] done"
        );
        assert!(placing_hint("Bed", 0, None, "E", "R").contains("first person"));
    }
}

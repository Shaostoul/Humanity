//! Placing a built piece in the world (2026-09-27): the ghost, the keys, the
//! hint.
//!
//! Build on the Crafting page used to drop the piece 4 m ahead on the floor,
//! unturned, with no preview, so walls only ran east-west and a roof could
//! never go overhead. Now Build puts the blueprint IN HAND: the page closes,
//! a see-through scaffold (the ghost) follows the crosshair, the Toggle roof
//! key (R by default) turns it a quarter, Interact (E) builds it where the
//! ghost stands and keeps the piece in hand for the next one, and Esc puts
//! it down. Where it lands is `construction::placement::placement_pose`: x
//! and z on the metre grid, y on the floor, or on top of the walls for a
//! roof (`mount: OnTop` in the blueprint data). The ghost's pose is kept on
//! the placing state every frame and E builds exactly that pose, so the
//! piece goes where the ghost stood.
//!
//! WHERE (2026-09-27, the real fix for BUG-102): aboard, in the home frame,
//! on the deck; on a planet's ground, in the build site the player stands
//! in, on the ground under the crosshair (`engine::planet_build::ghost`).
//! Where a piece cannot go (open space, a vehicle, flying, the sea) the hint
//! says so. A piece whose box already stands there is shown but not built
//! again (no double spend).
//!
//! R is the roof toggle when nothing is in hand; while placing it turns the
//! piece (the Controls page says so), because one key holds one action and R
//! is where every building game puts rotate. lib.rs calls `frame` once a
//! frame and `key` from its key handler, ahead of the E chain and the menu
//! Escape.

use crate::engine::planet_build;
use crate::engine::state::EngineState;
use crate::gui::{BuildPlacing, GuiPage, GuiState};
use crate::input::bindings::{pretty_key_name, GameAction};
use crate::systems::construction::{BlueprintRegistry, BuildRequest};

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
            // Only where a ghost stands (first person, on foot, aboard or on
            // a planet's ground), and never twice in one spot.
            let Some(p) = state.gui_state.build_placing.as_ref() else { return false };
            let Some(pose) = p.ghost.clone() else { return false };
            if p.occupied || p.short {
                return true;
            }
            let request = BuildRequest::new(p.blueprint_id.clone(), pose).on(p.site.clone());
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
        state.gui_state.build_placing = Some(BuildPlacing {
            blueprint_id: id,
            name,
            quarter_turns,
            ghost: None,
            site: None,
            occupied: false,
            short: false,
            hint: String::new(),
        });
        state.gui_state.active_page = GuiPage::None;
    }
    let g = &state.gui_state;
    if g.construction_active || g.showroom_active || g.player_death_cause.is_some() {
        state.gui_state.build_placing = None;
        return;
    }
    let Some(p) = state.gui_state.build_placing.as_ref() else { return };
    // Where the piece would go, in the frame the player is in: the home
    // aboard, the build site they stand in on a planet (planet_build).
    let (name, turns) = (p.name.clone(), p.quarter_turns);
    let placed = planet_build::ghost(state, &p.blueprint_id, turns);
    // On a planet only the pack counts (the home's storage is in orbit).
    let short = match &placed {
        Ok(g) if g.site.is_some() => carried_short(state, &p.blueprint_id),
        _ => None,
    };
    let keys = &state.gui_state.keybinds;
    let hint = match (&placed, &short) {
        (Ok(_), Some((item, more))) => short_hint(&name, item, *more),
        (Ok(g), None) => placing_hint(
            &name,
            turns,
            g.above_floor,
            g.occupied,
            &pretty_key_name(keys.pair(GameAction::Interact).0),
            &pretty_key_name(keys.pair(GameAction::ToggleRoof).0),
        ),
        (Err(why), _) => planet_build::cannot_build_hint(&name, *why),
    };
    if let Some(p) = state.gui_state.build_placing.as_mut() {
        p.hint = hint;
        p.short = short.is_some();
        match placed {
            Ok(g) => {
                p.ghost = Some(g.pose);
                p.site = g.site;
                p.occupied = g.occupied;
            }
            Err(_) => {
                p.ghost = None;
                p.site = None;
                p.occupied = false;
            }
        }
    }
}

/// What the player's pack is short of for `blueprint_id`: the item's name
/// and how many more, or None when they carry enough
/// (`construction::materials_short` with no home storage, the rule the
/// ConstructionSystem applies to a planet build).
fn carried_short(state: &EngineState, blueprint_id: &str) -> Option<(String, u32)> {
    use crate::ecs::components::Controllable;
    use crate::systems::inventory::{Inventory, ItemRegistry};
    let bp = state.data_store.get::<BlueprintRegistry>("blueprint_registry")?.get(blueprint_id)?;
    let world = &state.game_world.world;
    let mut q = world.query::<(&Inventory, &Controllable)>();
    let (_e, (inv, _)) = q.iter().next()?;
    let (id, more) = crate::systems::construction::materials_short(bp, |id| inv.count_item(id), None)?;
    let name = state
        .data_store
        .get::<ItemRegistry>("item_registry")
        .and_then(|r| r.items.get(&id).map(|d| d.name.clone()))
        .unwrap_or(id);
    Some((name, more))
}

/// The line under the crosshair on a planet when the pack holds too little:
/// what is missing, and why the home's storage does not count.
pub(crate) fn short_hint(name: &str, item: &str, more: u32) -> String {
    format!("Placing {name}: carry {more} more {item} to build it here (on a planet you build from what you carry)   [Esc] done")
}

/// The line under the crosshair while placing. `above_floor` is how high the
/// ghost rests above the floor it is aimed at (a roof on walls); `occupied`
/// says the same piece already stands there, so E will not build it again.
pub(crate) fn placing_hint(name: &str, quarter_turns: u8, above_floor: f32, occupied: bool, build_key: &str, turn_key: &str) -> String {
    let turned = match quarter_turns % 4 {
        0 => String::new(),
        q => format!(", turned {} degrees", u32::from(q) * 90),
    };
    let on_top = if above_floor > 0.01 { format!(", on top at {above_floor:.1} m") } else { String::new() };
    if occupied {
        return format!("Placing {name}{turned}{on_top}: already built here   [{turn_key}] turn   [Esc] done");
    }
    format!("Placing {name}{turned}{on_top}   [{build_key}] build here   [{turn_key}] turn   [Esc] done")
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
            site: None,
            occupied: false,
            short: false,
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
    /// live keys; over a piece already built it says so and offers no build
    /// key; and each place a piece cannot go has its own plain reason.
    #[test]
    fn the_hint_says_the_turn_and_the_keys() {
        assert_eq!(
            placing_hint("Wood Wall", 1, 0.0, false, "E", "R"),
            "Placing Wood Wall, turned 90 degrees   [E] build here   [R] turn   [Esc] done"
        );
        assert_eq!(
            placing_hint("Wood Roof", 4, 3.0, false, "E", "T"),
            "Placing Wood Roof, on top at 3.0 m   [E] build here   [T] turn   [Esc] done"
        );
        let twice = placing_hint("Wood Wall", 0, 0.0, true, "E", "R");
        assert!(twice.contains("already built here") && !twice.contains("[E]"), "{twice}");
        use planet_build::{cannot_build_hint, CannotBuild};
        assert!(cannot_build_hint("Bed", CannotBuild::NotFirstPerson).contains("first person"));
        assert!(cannot_build_hint("Bed", CannotBuild::Driving).contains("vehicle"));
        assert!(cannot_build_hint("Bed", CannotBuild::OpenSpace).contains("open space"));
        assert!(cannot_build_hint("Bed", CannotBuild::NotOnGround).contains("stand on the ground"));
        assert!(cannot_build_hint("Bed", CannotBuild::OnWater).contains("water"));
        let short = short_hint("Wood Wall", "Wood Plank", 4);
        assert!(short.contains("carry 4 more Wood Plank") && short.contains("what you carry") && !short.contains("build here"), "{short}");
    }
}

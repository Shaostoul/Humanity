//! The player's carried load, applied once a frame (BUG-136, 2026-10-04).
//!
//! `systems::encumbrance` holds the rules (the limit follows gravity; in the
//! Realistic carrying mode an overload slows walking and stops jumps, and a
//! load's mass weighs on every jump; the Forgiving mode warns only). This file
//! is the wiring: it reads what the inventory system measured, the gravity the
//! walk is applying, and the Settings switch, then
//!   * multiplies the controller's speed multiplier by the walking factor
//!     (the homestead walk and the planet surface walk both read it),
//!   * sets the controller's jump scale (the homestead jump reads it, and
//!     lib.rs gates the planet surface's Space through
//!     `surface_move::carry_gated_radial`),
//!   * publishes the local gravity for the inventory system's `encumbered`
//!     flag (`encumbrance::LOCAL_G_KEY`), and
//!   * publishes the whole state as `GuiState::carry`, which the Inventory
//!     page's Weight tile and the HUD's overload line read.
//!
//! Lives here, not in lib.rs, for the monolith ratchet: lib.rs only calls it.

use crate::ecs::components::Controllable;
use crate::engine::state::EngineState;
use crate::systems::encumbrance::{self as enc, CarryInput, CarryMode, CarryState};
use crate::systems::inventory::Inventory;

/// The gravity the player's walk is applying, m/s^2: the planet surface's at
/// the player's altitude while a surface is engaged (lib.rs captures it as
/// `surface_gravity_now` at the walk integrator, so this reads last frame's,
/// a frame of a constant), otherwise the homestead's interior gravity, which
/// the first-person controller applies everywhere else.
pub(crate) fn walk_gravity(surface_g: Option<f32>, interior_g: f32) -> f32 {
    surface_g.unwrap_or(interior_g)
}

/// What the controlled player carries, as the inventory system measured it on
/// its last tick. None before a player inventory exists (the menus).
pub(crate) fn player_carry_input(world: &hecs::World) -> Option<CarryInput> {
    world
        .query::<(&Inventory, &Controllable)>()
        .iter()
        .next()
        .map(|(_, (inv, _))| CarryInput {
            carried_kg: inv.weight_current,
            capacity_kg: inv.weight_capacity,
            bonus_kg: inv.carry_bonus_kg,
            volume_l: inv.volume_current_l,
            volume_capacity_l: inv.volume_capacity_l,
        })
}

/// Once a frame, right after the status-effect and gear speed multipliers
/// are set (it multiplies onto them).
pub(crate) fn apply(state: &mut EngineState) {
    let g = walk_gravity(state.gui_state.surface_gravity_now, state.controller.interior_gravity());
    state.data_store.insert(enc::LOCAL_G_KEY, g);
    let mode = CarryMode::from_realistic(state.gui_state.settings.carry_realistic);
    let carry = match player_carry_input(&state.game_world.world) {
        Some(input) => enc::evaluate(input, g, mode),
        None => CarryState::default(),
    };
    state.controller.speed_multiplier *= carry.speed_factor;
    state.controller.jump_scale = carry.jump_scale;
    state.gui_state.carry = carry;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a planet the walk's gravity is the planet's; anywhere else it is
    /// the homestead's. Seen red against a stub that always returned the
    /// interior gravity: "assertion `left == right` failed: on Mars the
    /// planet's gravity; left: 9.81, right: 3.72".
    #[test]
    fn the_walk_gravity_is_the_surface_one_when_there_is_one() {
        assert_eq!(walk_gravity(Some(3.72), 9.81), 3.72, "on Mars the planet's gravity");
        assert_eq!(walk_gravity(None, 9.81), 9.81, "aboard, the homestead's");
    }

    /// The input is the CONTROLLED entity's inventory with its recorded gear
    /// bonus, never another inventory (an NPC's or a chest's). Seen red
    /// against a stub returning None: "the player's load is read".
    #[test]
    fn the_carry_input_is_the_controlled_players() {
        let mut world = hecs::World::new();
        assert_eq!(player_carry_input(&world), None, "no player, no load");
        let mut npc = Inventory::new(4);
        npc.weight_current = 999.0;
        world.spawn((npc,));
        let mut inv = Inventory::new(4);
        inv.weight_current = 62.0;
        inv.carry_bonus_kg = 25.0;
        world.spawn((inv, Controllable));
        let input = player_carry_input(&world).expect("the player's load is read");
        assert_eq!((input.carried_kg, input.capacity_kg, input.bonus_kg), (62.0, 50.0, 25.0));
        assert_eq!(input.volume_capacity_l, 65.0);
    }
}

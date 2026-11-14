//! Hot reload and the movement setup (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: the data watcher's regeneration gates, the player's
//! status-effect speed modifiers, the dev travel sync, and stepping out of the shared world.

use crate::engine::frame_prelude::*;

/// Applies hot-reloaded data, then sets up this frame's movement.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.hot_reload");
    // Hot-reload: poll the watcher and apply every data-file
    // regeneration gate (planets, plants, hull, game.csv).
    // The gates live in engine/hot_reload_gates.rs - add new
    // ones THERE, not here (monolith ratchet, v0.1119).
    crate::engine::hot_reload_gates::poll_and_apply(state);

    humanity_core::frame_costs::span("cpu.span.move_setup");
    // Apply the player's active status-effect SPEED modifiers to movement
    // (well_nourished speeds you up; thirsty/flu slow you down). Look is
    // unaffected. Collect the effect ids first (owned → releases the world
    // borrow), then resolve the net multiplier from the registry.
    {
        let ids: Vec<String> = state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::StatusEffects,
                &Controllable,
            )>()
            .iter()
            .next()
            .map(|(_, (fx, _))| fx.active.iter().map(|e| e.id.clone()).collect())
            .unwrap_or_default();
        let mult = state
            .data_store
            .get::<humanity_systems::status_effects::StatusEffectRegistry>(
                "status_effect_registry",
            )
            .map(|reg| reg.net_stat_multiplier(ids.iter().map(|s| s.as_str()), "speed"))
            .unwrap_or(1.0);
        // Gear speed modifiers stack with effects (v0.750,
        // ladder rung 8): hiking boots x1.05, same grammar,
        // same fold — one modifier system, ever.
        let worn: Vec<String> = state
            .game_world
            .world
            .query::<(&humanity_core::ecs::components::Outfit, &Controllable)>()
            .iter()
            .next()
            .map(|(_, (o, _))| o.equipped.values().cloned().collect())
            .unwrap_or_default();
        let gear_mult = state
            .data_store
            .get::<humanity_systems::economy::EquipmentRegistry>("equipment_registry")
            .map(|reg| {
                reg.net_stat_multiplier(worn.iter().map(|s| s.as_str()), "speed")
            })
            .unwrap_or(1.0);
        state.controller.speed_multiplier = mult * gear_mult;
        // The carried load (BUG-136): walking speed + jump.
        crate::engine::carry_load::apply(state);
    }

    // Dev travel sync (v0.791.x): mirror the Dev page's fly/FTL
    // state into the camera controller each frame (same pattern
    // as speed_multiplier above). Forced OFF when dev tooling is
    // off (Dev play mode + cheats switch, task #50 -- a LIVE
    // per-frame gate, so switching the mode in Settings lands
    // you back on your feet the same frame). Engaging fly while
    // JOINED to the shared world STEPS OUT instead of being
    // refused (v0.801, admin moderation): copresence_solo sends
    // game_leave, so nobody ever sees a frozen ghost avatar and
    // the relay's position sync is never fought.
    if !state.gui_state.dev_cheats_active(&state.theme) {
        state.gui_state.dev_fly_mode = false;
    }
    if state.gui_state.dev_fly_mode
        && state.gui_state.copresence_active
        && !state.gui_state.copresence_solo
    {
        state.gui_state.copresence_solo = true;
        state.dev_travel_stepped_out = true;
        log::info!("Dev travel: fly mode engaged in the shared world -- stepping out");
    }
    state.controller.fly_mode = state.gui_state.dev_fly_mode;
    state.controller.fly_speed_mult =
        state.gui_state.dev_fly_speed_mult.clamp(1.0, 1.0e9);

}

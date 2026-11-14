//! The ECS tick (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as it was: home storage mirrored in, the game world's systems ticked once, then what
//! they used and made settled back into home storage, deaths and backpack overflow.

use crate::engine::frame_prelude::*;

/// Runs the game world's systems for this frame.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = f.dt;
    humanity_core::frame_costs::span("cpu.span.ecs");
    // Home-storage stock (v0.737 machines; hand crafts since
    // BUG-147): mirror the organize-layer placed items into a
    // shared id -> qty map the systems count and consume from
    // during the tick; the after-tick diff below drains what
    // they took back out of the GUI containers.
    let home_stock_before = crate::engine::stock_piles::publish_home_stock(state);

    // Tick all ECS systems: by the frame's time once the person has entered the
    // world, by none before that or while the character picker is open (BUG-253,
    // engine/world_hold.rs). `world_dt` is the time the world moved on by.
    let world_dt = crate::engine::world_hold::tick(
        &mut state.system_runner,
        &mut state.game_world.world,
        &state.data_store,
        &mut state.gui_state,
        state.world_loaded,
        dt,
    );

    // What the systems used FROM home storage this tick comes out
    // of the placed containers (v0.737; planet chests excluded).
    crate::engine::stock_piles::take_consumed_home_stock(state, &home_stock_before);
    // Home > Animals: each farm animal, how it is and what it needs (first-hour S6).
    state.gui_state.herd = humanity_systems::animal_care::herd_rows(&state.game_world.world, &state.data_store);
    crate::engine::stock_piles::receive_machine_outputs(state);
    crate::engine::stock_piles::age_home_storage(state); // stored food spoils too (S6)
    // A death the tick recorded, and the packs Realistic deaths leave (before the
    // periodic save below, so no save holds a death with a full backpack).
    crate::engine::death_pack::after_tick(state, world_dt);
    // Backpack overflow from "Take to backpack" goes back to the
    // container it came from (2026-09-25; it used to vanish).
    let returned: Vec<(String, u32)> = state
        .data_store
        .get::<std::sync::Mutex<Vec<(String, u32)>>>("inventory_transfer_returns")
        .and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v)))
        .unwrap_or_default();
    // What the systems said: shown, or held for the person's entry (BUG-253).
    crate::engine::world_hold::take_notices(&state.data_store, &mut state.gui_state);
    let origins = std::mem::take(&mut state.gui_state.inflight_take_origins);
    for (key, qty) in returned {
        let origin = origins.iter().find(|o| o.key == key).cloned();
        let name = state
            .data_store
            .get::<humanity_systems::inventory::ItemRegistry>("item_registry")
            .and_then(|r| r.items.get(&key).map(|d| d.name.clone()))
            .unwrap_or_else(|| key.clone());
        let msg = crate::gui::return_to_storage(
            &mut state.gui_state.placed_items,
            &key,
            qty,
            origin.as_ref(),
            &name,
        );
        state
            .gui_state
            .pending_toasts
            .push((msg, crate::gui::ToastKind::Info));
    }

}

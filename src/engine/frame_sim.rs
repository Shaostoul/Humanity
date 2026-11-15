//! One step of the world's systems, from the simulation's parts (the headless sim runner's
//! increment 2, 2026-11-15; docs/design/headless-sim-runner.md). The windowed frame calls
//! `systems_slice` from its ECS stage (engine/frame_ecs.rs) exactly where this body ran before,
//! and the soak (`HumanityOS --sim soak`, engine/sim_soak.rs) and the lib tests that step a game
//! day with no GPU (frame_sim_tests.rs) call the same function: one step, two drivers.
//!
//! What a step does, in this order (moved verbatim from frame_ecs.rs, which had moved it out of
//! lib.rs `window_event` on 2026-11-14): home storage mirrored in for the systems to count and
//! draw on, the systems ticked once (held still until the person enters a world, BUG-253), what
//! they drew taken back out of the containers, the herd's rows, what machines made filed into
//! storage, stored food aged, a death and the packs' clocks, a backpack's overflow returned to
//! the container it came from, and the systems' notices shown or held.
//!
//! No GPU, window, painter or `EngineState` in here (tests/sim_gpu_free_lint.rs).

use humanity_core::ecs::systems::SystemRunner;
use humanity_core::hot_reload::data_store::DataStore;

use crate::engine::death_pack::FallSite;
use crate::gui::GuiState;

/// What one step of the systems reads and writes: the simulation's own parts.
pub(crate) struct SimParts<'a> {
    /// The registered systems, in tick order (engine/sim_boot.rs).
    pub runner: &'a mut SystemRunner,
    pub world: &'a mut hecs::World,
    pub data: &'a mut DataStore,
    /// The person's side of the world the systems answer to: home storage (the placed items),
    /// the pages' requests, the notices, the Death setting.
    pub gui: &'a mut GuiState,
    /// Whether the person has entered a world: the systems run only then (world_hold.rs).
    pub world_loaded: bool,
}

/// One step of the world's systems by `dt` real seconds (the clock turns them into game seconds
/// by the time speed, humanity_systems::time). `site` is where the player is, for the pack a
/// Realistic death leaves. Returns the time the world moved on by: `dt` while the world runs,
/// none while it is held.
pub(crate) fn systems_slice(parts: SimParts<'_>, dt: f32, site: &FallSite<'_>) -> f32 {
    let SimParts { runner, world, data, gui, world_loaded } = parts;
    // Home-storage stock (v0.737 machines; hand crafts since
    // BUG-147): mirror the organize-layer placed items into a
    // shared id -> qty map the systems count and consume from
    // during the tick; the after-tick diff below drains what
    // they took back out of the GUI containers.
    let home_stock_before = crate::engine::stock_piles::publish_home_stock(world, data, gui);

    // Tick all ECS systems: by the frame's time once the person has entered the
    // world, by none before that or while the character picker is open (BUG-253,
    // engine/world_hold.rs). `world_dt` is the time the world moved on by.
    let world_dt = crate::engine::world_hold::tick(runner, world, data, gui, world_loaded, dt);

    // What the systems used FROM home storage this tick comes out
    // of the placed containers (v0.737; planet chests excluded).
    crate::engine::stock_piles::take_consumed_home_stock(world, data, gui, &home_stock_before);
    // Home > Animals: each farm animal, how it is and what it needs (first-hour S6).
    gui.herd = humanity_systems::animal_care::herd_rows(world, data);
    crate::engine::stock_piles::receive_machine_outputs(data, gui);
    crate::engine::stock_piles::age_home_storage(world, data, &mut gui.placed_items); // stored food spoils too (S6)
    // A death the tick recorded, and the packs Realistic deaths leave (before the
    // periodic save below, so no save holds a death with a full backpack).
    crate::engine::death_pack::after_tick(world, data, gui, world_loaded, world_dt, site);
    // Backpack overflow from "Take to backpack" goes back to the
    // container it came from (2026-09-25; it used to vanish).
    let returned: Vec<(String, u32)> = data
        .get::<std::sync::Mutex<Vec<(String, u32)>>>("inventory_transfer_returns")
        .and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v)))
        .unwrap_or_default();
    // What the systems said: shown, or held for the person's entry (BUG-253).
    crate::engine::world_hold::take_notices(data, gui);
    let origins = std::mem::take(&mut gui.inflight_take_origins);
    for (key, qty) in returned {
        let origin = origins.iter().find(|o| o.key == key).cloned();
        let name = data
            .get::<humanity_systems::inventory::ItemRegistry>("item_registry")
            .and_then(|r| r.items.get(&key).map(|d| d.name.clone()))
            .unwrap_or_else(|| key.clone());
        let msg = crate::gui::return_to_storage(&mut gui.placed_items, &key, qty, origin.as_ref(), &name);
        gui.pending_toasts.push((msg, crate::gui::ToastKind::Info));
    }
    world_dt
}

#[cfg(test)]
#[path = "frame_sim_tests.rs"]
mod tests;

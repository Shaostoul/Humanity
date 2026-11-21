//! One step of the world from the simulation's parts (the headless sim runner, increments 2 and 3,
//! 2026-11-15; docs/design/headless-sim-runner.md). The windowed frame calls `bridges_slice` from
//! its bridges stage (engine/frame_bridges.rs) and `systems_slice` from its ECS stage
//! (engine/frame_ecs.rs), exactly where those bodies ran before, and the soak (`HumanityOS --sim
//! soak`, engine/sim_soak.rs, through engine/sim_home.rs) and the lib tests that step game days with
//! no GPU (frame_sim_tests.rs) call the same two in the same order: one step, two drivers. Both
//! advance one `FrameClock` and hand it to the step.
//!
//! What `bridges_slice` does, in this order (increment 3; the order the frame's bridges ran it):
//! the body's environment published from where the person is (`BodyPlace`, plain values the driver
//! reads before the step: the body the frame lock holds, the sealed home's air around the body or
//! the weather outside, the felt gravity), the mining drone's panel, and the GUI's requests carried
//! into the systems' channels (eat, drink, use, plant, harvest, craft, the tank's Fill, the
//! backpack's transfers; engine/frame_sim_requests.rs).
//!
//! What `systems_slice` does, in this order (moved verbatim from frame_ecs.rs, which had moved it
//! out of lib.rs `window_event` on 2026-11-14): home storage mirrored in for the systems to count
//! and draw on, the systems ticked once (held still until the person enters a world, BUG-253), what
//! they drew taken back out of the containers, the herd's rows, what machines made filed into
//! storage, stored food aged, a death and the packs' clocks, a backpack's overflow returned to the
//! container it came from, and the systems' notices shown or held.
//!
//! No GPU, window, painter or `EngineState` in here (tests/sim_gpu_free_lint.rs).

use humanity_core::ecs::systems::SystemRunner;
use humanity_core::hot_reload::data_store::DataStore;

use crate::engine::death_pack::FallSite;
use crate::engine::planet_build::{GroundSources, PlayerView};
use crate::gui::GuiState;

/// The step's clock, the one both drivers advance (the design's `FrameClock`, increment 3): the
/// windowed frame from the wall clock (engine/frame_clock.rs), the soak from its script
/// (engine/sim_home.rs). The sim reads its time from here, never from `Instant` or `SystemTime`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct FrameClock {
    /// The step the simulation runs on, real seconds: the windowed frame's, capped at 0.1 s so a
    /// hitch cannot spiral (the movie's fixed step while recording); the soak's game step divided
    /// by the time speed the next tick runs at (`real_step_for`).
    pub dt: f32,
    /// Real time, uncapped (the movie's step while recording): the shared-world clocks run on it.
    pub clock_dt: f32,
    /// The true frame time, for the perf counters only.
    pub raw_dt: f32,
    /// Real seconds since 2000-01-01 12:00 the planets are placed and turned by.
    pub sim_epoch_s: f64,
    /// Steps since the driver started.
    pub frame: u64,
    /// Simulation seconds since the driver started: every step's `dt` summed.
    pub elapsed_s: f64,
}

impl FrameClock {
    /// One step on: its `dt` (the clamped step the sim runs), `clock_dt` (real, uncapped), `raw_dt`
    /// (the counters') and the real clock the bodies are placed by.
    pub(crate) fn advance(&mut self, dt: f32, clock_dt: f32, raw_dt: f32, sim_epoch_s: f64) {
        self.dt = dt;
        self.clock_dt = clock_dt;
        self.raw_dt = raw_dt;
        self.sim_epoch_s = sim_epoch_s;
        self.frame += 1;
        self.elapsed_s += f64::from(dt);
    }

    /// One step of a scripted driver (the soak): `dt` real seconds, the real clock moved on by as
    /// much, so the planets turn with it.
    pub(crate) fn advance_by(&mut self, dt: f32) {
        let epoch = self.sim_epoch_s + f64::from(dt);
        self.advance(dt, dt, dt, epoch);
    }
}

/// The real seconds that move the game clock `game_s` seconds at `scale` game seconds a real
/// second (the time speed the next tick runs at, `humanity_systems::time::next_tick_scale`: a
/// night's sleep runs it 7,200 times faster). A clock held still (scale 0) takes the step as real
/// seconds, so a scripted run still moves.
pub(crate) fn real_step_for(game_s: f64, scale: f32) -> f32 {
    if scale > 0.0 { (game_s / f64::from(scale)) as f32 } else { game_s as f32 }
}

/// Where the person is and what their body is doing, as plain values the driver reads before the
/// step (the windowed frame from the camera, the controller, the frame lock and the drive,
/// engine/frame_bridges.rs; the soak from where its person stands, engine/sim_home.rs).
pub(crate) struct BodyPlace<'a> {
    /// The person's view: aboard, the eye in the home frame and the look; on a planet, the body
    /// the frame lock holds and the eye in that body's frame.
    pub view: PlayerView,
    /// Walking and sprinting (`CameraController::gait`): the body's activity.
    pub moving: bool,
    pub sprinting: bool,
    /// At a vehicle's controls.
    pub driving: bool,
    /// Dev fly: the vacuum, the cold and the burn suspended.
    pub fly_mode: bool,
    /// What the drive and any spin do to a body now, after the dampeners
    /// (`humanity_systems::flight::felt_gravity`).
    pub felt: humanity_systems::flight::FeltGravity,
    /// Whose air each place breathes (`survival_env::air_spaces`; None before anything generated).
    pub air: Option<humanity_ship::ship_space::AirSpaces>,
    /// The planets' definitions, heightmaps and Earth's sea mask (None: no planet loaded).
    pub ground: Option<GroundSources<'a>>,
}

/// The frame's bridges' world-feeding half for one step (increment 3): the body's environment from
/// where the person is, and the GUI's requests into the systems' channels. Run before
/// `systems_slice`, so this step's tick reads this step's environment and requests, as the
/// windowed frame's bridges stage always ran before its ECS stage.
pub(crate) fn bridges_slice(parts: SimParts<'_>, place: &BodyPlace<'_>) {
    let SimParts { world, data, gui, .. } = parts;
    // Per-body environment snapshot (artificial-planet increment 4): publish which world the
    // player is on so the ECS systems simulate THAT body instead of an Earth-global one. Full
    // contract + the latitude/altitude derivations: frame_lock::publish_body_environment.
    crate::engine::frame_lock::publish_body_environment(data, &place.view, place.ground);
    // Survival environment context (sealed home or the weather) and the body heat mode:
    // engine::survival_env (moved out of lib.rs 2026-09-27).
    crate::engine::survival_env::publish(world, data, gui, place);
    // The mining drone: its Settings mode, the Mining panel's Repair/Cancel, its battery and wear
    // on the panel (first-hour audit S5).
    crate::engine::drone::bridge_gui(gui, data);
    // The GUI's requests into the systems' channels (engine/frame_sim_requests.rs).
    crate::engine::frame_sim_requests::carry(gui, data, world);
    // The quests' glue: views that came on screen and guides opened, as quest events
    // (engine/quest_hooks.rs; on the step so the soak's routine counts them too, 2026-11-20).
    crate::engine::quest_hooks::report(gui, data);
}

/// What one step reads and writes: the simulation's own parts.
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

/// One step of the world's systems by the clock's `dt` real seconds (the game clock turns them
/// into game seconds by the time speed, humanity_systems::time). `site` is where the player is,
/// for the pack a Realistic death leaves. Returns the time the world moved on by: `dt` while the
/// world runs, none while it is held.
pub(crate) fn systems_slice(parts: SimParts<'_>, clock: &FrameClock, site: &FallSite<'_>) -> f32 {
    let dt = clock.dt;
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

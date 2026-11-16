//! The homestead's simulation with no GPU, entered and stepped (the headless sim runner's
//! increment 2, 2026-11-15; docs/design/headless-sim-runner.md). `enter` makes the same calls the
//! windowed game makes at startup and at world entry (lib.rs `resumed`: `sim_boot::boot`, the
//! data registries, the starter vehicles, the save's progress; world_load.rs `load_world`:
//! `world_sim::build`); `Homestead::step` runs `frame_sim::systems_slice`, the step the windowed
//! frame runs; `Homestead::observe` reads what each system holds, by name, for the soak's report
//! and the step ladder (engine/sim_soak.rs, frame_sim_tests.rs).
//!
//! No GPU, window, painter or `EngineState` in here (tests/sim_gpu_free_lint.rs).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::engine::death_pack::FallSite;
use crate::engine::frame_sim::{bridges_slice, real_step_for, systems_slice, BodyPlace, FrameClock, SimParts};
use crate::engine::sim_boot::{self, SimBoot, SimBootOptions};
use crate::engine::sim_routine::{Routine, RoutineRun};
use crate::engine::world_sim::{self, WorldSim};
use crate::gui::GuiState;

/// How far the arrival point (`world_sim::Spawn`, an eye) stands over the floor, metres: the
/// fallback spawns' 1.7 m eye over a floor at 0.
const ARRIVAL_EYE_M: f32 = 1.7;

/// The homestead's simulation, entered: the systems, the world, the DataStore and the GUI's side
/// of it, and what the world entry built.
pub(crate) struct Homestead {
    pub sim: SimBoot,
    pub gui: GuiState,
    pub built: WorldSim,
    /// What the systems said over the run (shown by the windowed GUI and emptied each frame;
    /// counted and emptied by `step`, so a long run holds no queue that only grows).
    pub notices: Vec<String>,
    /// The run's clock, advanced once a step as the windowed frame advances its own (increment 3).
    pub clock: FrameClock,
    /// The person's daily routine, when the run has one (`--routine`, engine/sim_routine.rs): its
    /// acts become the requests a player's clicks make, before each step.
    pub routine: Option<RoutineRun>,
}

/// Enter the homestead the way the windowed game does, with no GPU: the game's data from
/// `data_dir`, saves read from (and written to) `saves_dir` only.
pub(crate) fn enter(data_dir: &Path, saves_dir: PathBuf) -> Homestead {
    let _ = humanity_core::storage::DATA_DIR.set(data_dir.to_path_buf());
    let mut sim = sim_boot::boot(SimBootOptions { data_dir: data_dir.to_path_buf(), saves_dir, config: None });
    let mut gui = GuiState::default();
    crate::engine::registries::load_data_registries(&mut sim.data_store, data_dir);
    sim_boot::starter_vehicles(&mut sim.game_world, &sim.data_store);
    let home_save = sim.home_save.take();
    sim_boot::apply_home_save(&mut sim.game_world, &mut sim.data_store, &mut gui, home_save.as_ref(), data_dir);
    let built = world_sim::build(world_sim::Parts {
        data_dir,
        world: &mut sim.game_world.world,
        data_store: &mut sim.data_store,
        gui: &mut gui,
    });
    // The person steps into the world: no page over it and the picker closed (lib.rs's
    // showroom_confirm), so the first step marks the world entered and the systems run
    // (GuiState::note_world_frame, BUG-253). Without this the systems are held and nothing moves.
    // Through the history, as every page change goes (tests/page_change_lint.rs); the sim's world
    // exists on any device, so not `enter_world`'s phone answer.
    gui.enter_world_if(true);
    gui.showroom_active = false;
    // Settings > Gameplay's levels (body heat, illness, animal care, death, the garden's) reach
    // their systems every step through the bridges' slice, as in the windowed frame.
    Homestead { sim, gui, built, notices: Vec::new(), clock: FrameClock::default(), routine: None }
}

impl Homestead {
    /// Plant the first tower design, one plant per cup, the way the Garden page's Plant button
    /// does (it sends the design and its cups to the farm, "plant_tower_request"; the opening
    /// quest asks for it in the first hour). Each variety takes a seed from the backpack, as in
    /// play. Returns the design planted, or None with no tower design in the data.
    pub(crate) fn plant_first_tower(&mut self, data_dir: &Path) -> Option<String> {
        let towers = crate::gui::load_tower_configs(data_dir);
        let tower = towers.first()?;
        let cups: Vec<String> = tower
            .plantings
            .iter()
            .flat_map(|p| std::iter::repeat(p.plant.clone()).take(p.slots.max(1) as usize))
            .collect();
        let slot = self.sim.data_store.get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")?;
        *slot.lock().ok()? = Some((tower.id.clone(), cups));
        Some(tower.id.clone())
    }

    /// One step of the world by `dt` real seconds, as the windowed frame steps it (the clock's time
    /// speed applies; it is 1 unless a night's sleep speeds it, so a second of `dt` is a game
    /// second): the clock advanced, the routine's requests made, the frame's bridges (the body's
    /// environment where the person stands, the requests carried into the systems), then the
    /// systems, the person in the world and standing where they arrived. Returns the time the
    /// world moved on by.
    pub(crate) fn step(&mut self, dt: f32) -> f32 {
        self.clock.advance_by(dt);
        if let Some(r) = self.routine.as_mut() {
            r.before_step(&self.sim.game_world.world, &self.sim.data_store, &mut self.gui);
        }
        let eye = self.eye();
        let place = self.place(eye);
        bridges_slice(self.parts(), &place);
        let site = FallSite::aboard(eye, ARRIVAL_EYE_M);
        let clock = self.clock;
        let moved = systems_slice(self.parts(), &clock, &site);
        if let Some(r) = self.routine.as_mut() {
            r.after_step(&self.sim.game_world.world, &self.sim.data_store);
        }
        self.notices.append(&mut self.gui.pending_notices);
        self.notices.extend(self.gui.pending_toasts.drain(..).map(|(msg, _)| msg));
        moved
    }

    /// One step that moves the game clock `game_s` seconds: the real step at the time speed the
    /// next tick runs at (`time::next_tick_scale`; 7,200 through a night's sleep, as the bed's
    /// sleep runs the windowed game's clock), so a soak steps game time evenly whatever the
    /// person does. Returns the time the world moved on by, real seconds.
    pub(crate) fn step_game(&mut self, game_s: f64) -> f32 {
        let scale = humanity_systems::time::next_tick_scale(&self.sim.data_store);
        self.step(real_step_for(game_s, scale))
    }

    /// Run `routine` before every step from now on.
    pub(crate) fn follow(&mut self, routine: Routine) {
        self.routine = Some(RoutineRun::new(routine));
    }

    /// Where the person's eye is: where they arrived.
    fn eye(&self) -> glam::Vec3 {
        self.built.spawn.map_or(glam::Vec3::new(0.0, ARRIVAL_EYE_M, 0.0), |s| s.position)
    }

    /// The person standing still at `eye` aboard, facing the way they arrived, in the parked ship:
    /// what the windowed frame reads from its camera, controller and drive (frame_bridges.rs).
    fn place(&self, eye: glam::Vec3) -> BodyPlace<'static> {
        let (yaw, pitch) = self.built.spawn.map_or((0.0, 0.0), |s| (s.yaw, s.pitch));
        let forward = glam::Vec3::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos());
        let dampener = self
            .sim
            .data_store
            .get::<humanity_systems::flight::FlightData>("flight_data")
            .map(|f| f.dampener.clone())
            .unwrap_or_default();
        BodyPlace {
            view: crate::engine::planet_build::PlayerView {
                aboard: true,
                camera_pos: eye,
                forward,
                eye_height: ARRIVAL_EYE_M,
                body: None,
                anchor: glam::DVec3::ZERO,
                rot: glam::DQuat::IDENTITY,
            },
            moving: false,
            sprinting: false,
            driving: false,
            fly_mode: false,
            felt: humanity_systems::flight::felt_gravity(&humanity_systems::flight::FlightState::default(), &dampener),
            air: crate::engine::survival_env::air_spaces(self.gui.ship_structure.is_some(), &self.built.ship_air, self.built.homestead_bounds),
            ground: None,
        }
    }

    /// The simulation's parts, for one slice of the step.
    fn parts(&mut self) -> SimParts<'_> {
        SimParts {
            runner: &mut self.sim.system_runner,
            world: &mut self.sim.game_world.world,
            data: &mut self.sim.data_store,
            gui: &mut self.gui,
            world_loaded: true,
        }
    }

    /// What each system holds now, by name (`OBSERVABLES`; the units are in the names'
    /// descriptions there). The farm is brought exactly to now first, the way the Garden page
    /// and the save read it (`stock_piles::settle_farm`, inside its own home-storage window).
    pub(crate) fn observe(&mut self) -> BTreeMap<&'static str, f64> {
        crate::engine::stock_piles::settle_farm(
            &mut self.sim.game_world.world,
            &mut self.sim.data_store,
            &mut self.gui.placed_items,
            &mut self.gui.home_stock,
            0.0,
        );
        self.read()
    }

    /// What each system holds now, read as it stands, the farm not brought to now (a reading
    /// that must not touch the run: the baseline the tolerances measure movement from).
    pub(crate) fn read(&self) -> BTreeMap<&'static str, f64> {
        let at = Reading { world: &self.sim.game_world.world, data: &self.sim.data_store, gui: &self.gui };
        OBSERVABLES.iter().filter_map(|o| (o.read)(&at).map(|v| (o.id, v))).collect()
    }
}

/// What an observable reads from.
pub(crate) struct Reading<'a> {
    pub world: &'a hecs::World,
    pub data: &'a humanity_core::hot_reload::data_store::DataStore,
    pub gui: &'a GuiState,
}

/// One named quantity a system holds, and how to read it. The tolerances it is judged by at a
/// coarse step are data (data/sim/step_tolerances.ron); the reading is code.
pub(crate) struct Observable {
    pub id: &'static str,
    /// The system that owns it, for the ladder's "which systems hold at which step".
    pub system: &'static str,
    /// What it is, with its unit.
    pub what: &'static str,
    pub read: fn(&Reading<'_>) -> Option<f64>,
}

/// The player's body's component `T`, read through `f`.
fn player<T: hecs::Component, R>(r: &Reading<'_>, f: impl Fn(&T) -> R) -> Option<R> {
    let mut q = r.world.query::<(&humanity_core::ecs::components::Controllable, &T)>();
    q.iter().next().map(|(_, (_, c))| f(c))
}

/// The sum of `f` over every entity with a `T`, or None when there is none.
fn sum_of<T: hecs::Component>(r: &Reading<'_>, f: impl Fn(&T) -> f64) -> Option<f64> {
    let mut q = r.world.query::<&T>();
    let mut any = false;
    let total = q.iter().fold(0.0, |sum, (_, c)| {
        any = true;
        sum + f(c)
    });
    any.then_some(total)
}

/// The home's own air (`HomeAirState`, kept in the farm's `SoilMemory` with the soil, life_support.rs).
fn home_air<R>(r: &Reading<'_>, f: impl Fn(&humanity_core::ecs::components::HomeAirState) -> R) -> Option<R> {
    let mut q = r.world.query::<&humanity_core::ecs::components::SoilMemory>();
    q.iter().next().map(|(_, m)| f(&m.home_air))
}

/// Every quantity the soak reports and the ladder judges. One row per quantity a system holds
/// that a day changes; a new system's quantity is a new row here and a tolerance row in
/// data/sim/step_tolerances.ron.
pub(crate) const OBSERVABLES: &[Observable] = &[
    Observable {
        id: "clock.elapsed_s",
        system: "TimeSystem",
        what: "the game clock, game seconds since the world began",
        read: |r| r.data.get::<std::sync::Mutex<humanity_systems::time::GameTime>>("game_time").and_then(|g| g.lock().ok().map(|g| g.elapsed_seconds)),
    },
    Observable {
        id: "vitals.satiation",
        system: "FoodSystem",
        what: "the player's fullness, 0 to its maximum",
        read: |r| player::<humanity_core::ecs::components::Vitals, _>(r, |v| f64::from(v.satiation)),
    },
    Observable {
        id: "vitals.hydration",
        system: "FoodSystem",
        what: "the player's hydration, 0 to its maximum",
        read: |r| player::<humanity_core::ecs::components::Vitals, _>(r, |v| f64::from(v.hydration)),
    },
    Observable {
        id: "vitals.energy",
        system: "FoodSystem",
        what: "the player's rest, 0 to its maximum",
        read: |r| player::<humanity_core::ecs::components::Vitals, _>(r, |v| f64::from(v.energy)),
    },
    Observable {
        id: "vitals.oxygen",
        system: "FoodSystem",
        what: "the player's blood oxygen, 0 to its maximum",
        read: |r| player::<humanity_core::ecs::components::Vitals, _>(r, |v| f64::from(v.oxygen)),
    },
    Observable {
        id: "vitals.body_temp_c",
        system: "FoodSystem",
        what: "the player's core temperature, Celsius",
        read: |r| player::<humanity_core::ecs::components::Vitals, _>(r, |v| f64::from(v.body_temp_c)),
    },
    Observable {
        id: "vitals.waste",
        system: "FoodSystem",
        what: "the player's organic waste, 0 to its maximum",
        read: |r| player::<humanity_core::ecs::components::Vitals, _>(r, |v| f64::from(v.waste)),
    },
    Observable {
        id: "body.health",
        system: "FoodSystem",
        what: "the player's health points",
        read: |r| player::<humanity_core::ecs::components::Health, _>(r, |h| f64::from(h.current)),
    },
    Observable {
        id: "crops.count",
        system: "FarmingSystem",
        what: "living crops in the home's grow machines",
        read: |r| sum_of::<humanity_core::ecs::components::CropInstance>(r, |_| 1.0),
    },
    Observable {
        id: "crops.growing_s",
        system: "FarmingSystem",
        what: "game seconds of growth summed over every crop",
        read: |r| sum_of::<humanity_core::ecs::components::CropInstance>(r, |c| c.growing_seconds),
    },
    Observable {
        id: "crops.health",
        system: "FarmingSystem",
        what: "crop health summed over every crop, 0 to 100 each",
        read: |r| sum_of::<humanity_core::ecs::components::CropInstance>(r, |c| f64::from(c.health)),
    },
    Observable {
        id: "power.battery_wh",
        system: "ElectricalSystem",
        what: "the batteries' charge summed, watt-hours",
        read: |r| sum_of::<humanity_core::ecs::components::Battery>(r, |b| f64::from(b.charge_wh)),
    },
    Observable {
        id: "water.tank_l",
        system: "PlumbingSystem",
        what: "the water tanks' levels summed, litres",
        read: |r| sum_of::<humanity_core::ecs::components::WaterTank>(r, |t| f64::from(t.liters)),
    },
    Observable {
        id: "air.o2_g_m3",
        system: "AtmosphereSystem",
        what: "the home air's oxygen, grams per cubic metre",
        read: |r| home_air(r, |a| a.o2_g_m3),
    },
    Observable {
        id: "air.co2_g_m3",
        system: "AtmosphereSystem",
        what: "the home air's carbon dioxide, grams per cubic metre",
        read: |r| home_air(r, |a| a.co2_g_m3),
    },
    Observable {
        id: "air.vapour_g_m3",
        system: "AtmosphereSystem",
        what: "the home air's water vapour, grams per cubic metre",
        read: |r| home_air(r, |a| a.vapour_g_m3),
    },
    Observable {
        id: "storage.items",
        system: "home storage",
        what: "items in home storage, every stack's count summed",
        read: |r| Some(r.gui.placed_items.iter().fold(0.0, |sum, p| sum + f64::from(p.qty))),
    },
    Observable {
        id: "herd.animals",
        system: "LivestockSystem",
        what: "farm animals in the herd",
        read: |r| Some(r.gui.herd.len() as f64),
    },
    // The frame's bridges' quantities (increment 3): the environment they publish around the body,
    // and the backpack the requests they carry change (what is eaten leaves it, the tank's Fill
    // puts water in its bottles).
    Observable {
        id: "env.air_c",
        system: "the bridges (survival_env)",
        what: "the air around the body, Celsius, as the frame's bridges publish it",
        read: |r| env(r, |e| f64::from(e.ambient_temp_c)),
    },
    Observable {
        id: "env.humidity",
        system: "the bridges (survival_env)",
        what: "the air's relative humidity around the body, 0 to 1",
        read: |r| env(r, |e| f64::from(e.relative_humidity)),
    },
    Observable {
        id: "env.oxygenated",
        system: "the bridges (survival_env)",
        what: "whether the air around the body is breathable, 1 or 0",
        read: |r| env(r, |e| if e.oxygenated { 1.0 } else { 0.0 }),
    },
    Observable {
        id: "env.g_load",
        system: "the bridges (survival_env)",
        what: "the gravity the body feels, g",
        read: |r| env(r, |e| f64::from(e.g_load)),
    },
    Observable {
        id: "pack.items",
        system: "InventorySystem",
        what: "items in the backpack, every stack's count summed (a meal leaves it)",
        read: |r| Some(crate::engine::sim_routine::pack_counts(r.world).values().map(|n| f64::from(*n)).sum()),
    },
    Observable {
        id: "pack.water_l",
        system: "the bridges (the tank's Fill)",
        what: "litres of drinking water carried in bottles and jugs",
        read: |r| Some(crate::engine::sim_routine::full_vessel_litres(r.world, r.data)),
    },
];

/// The survival context the frame's bridges published, read through `f` (None before the first step).
fn env(r: &Reading<'_>, f: impl Fn(&humanity_core::ecs::components::EnvironmentContext) -> f64) -> Option<f64> {
    r.data.get::<humanity_core::ecs::components::EnvironmentContext>("environment_context").map(f)
}

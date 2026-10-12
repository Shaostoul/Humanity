//! Mining system — autonomous drones that fly to asteroids, extract ore over time,
//! return home, and drop the raw material into the player's inventory.
//!
//! The operator's core acquisition loop: commission a drone for an ore → it spends
//! time travelling + mining a FINITE asteroid → returns the raw ore. When an asteroid
//! is fully consumed its entity is deleted. (The MMO swarm + abandoned-deletion is the
//! server-authoritative #5b follow-up; this is the single-player loop.)
//!
//! RUNNING COSTS (first-hour audit S5, 2026-10-12). The drone used to fly for nothing
//! but a click. Now it is a real machine with a battery and parts that wear, its numbers
//! in data/mining/drone.ron (`DroneSpec`, a DJI FlyCart 100, the cargo drone that
//! carries the hold):
//!
//! - ENERGY. Each trip takes a full charge of its battery (`trip_wh`). Between trips its
//!   dock charges it from the home's power: the dock is an optional load on the home's
//!   grid (`DroneCharger`, a `PowerConsumer` the ElectricalSystem feeds or sheds like any
//!   other), so a trip's energy comes out of the home's panels and batteries, and from the
//!   ship's reactor (metered) where the home is tied to it. It takes off only with a
//!   trip's charge on board.
//! - WEAR. Each trip wears it; after `service_trips` it stays docked until it is
//!   serviced from the Mining panel with `service_parts`, taken from the backpack and
//!   then home storage (the way a hand craft draws, `crafting::home_store`).
//! - TWO MODES (CLAUDE.md "Dual modes"; Settings > Gameplay > Mining drone, `MODE_KEY`).
//!   Realistic: a dock the home cannot power does not charge it, and a Launch without a
//!   trip's charge is refused with the reason. Simplified (the default): the dock charges
//!   it anyway, so a flat home only delays the trip (a Launch waits for the charge and
//!   then goes by itself), and it wears at `simplified_wear_share` of the rate.
//! - SAVED with the home (`WorldSave::mining_drone`): its charge, its wear and a launch
//!   waiting for its charge. A save without it gets a new drone: full, unworn.
//!
//! With no `DroneSpec` registered (the tests that predate it, a headless relay) the
//! drone runs as it always did, free.

use serde::{Deserialize, Serialize};

use crate::ecs::components::{AsteroidBody, Controllable, Drone, DronePhase, PowerCircuit, PowerConsumer};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::{Inventory, ItemRegistry};
use std::sync::Mutex;

/// DataStore key of the drone's data (`DroneSpec`), registered at startup by `register`.
pub const SPEC_KEY: &str = "mining_drone_spec";
/// DataStore key of the drone's own state (`Mutex<DroneUnit>`): saved with the home.
pub const UNIT_KEY: &str = "mining_drone_unit";
/// DataStore key of the Mining panel's request (`Mutex<Option<DroneRequest>>`).
pub const REQUEST_KEY: &str = "mining_drone_request";
/// DataStore key of what the Mining panel shows (`Mutex<Option<DroneReadout>>`), written
/// by the DroneSystem every tick.
pub const READOUT_KEY: &str = "mining_drone_readout";
/// DataStore key of the Settings mode (`Mutex<bool>`, true = Realistic), published each
/// frame from Settings > Gameplay > Mining drone (`publish_mode`).
pub const MODE_KEY: &str = "mining_drone_realistic";

/// The two modes of the house rule for deep systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The default: the dock charges the drone even when the home cannot, so a flat home
    /// only delays a trip, and it wears at `DroneSpec::simplified_wear_share` of the rate.
    #[default]
    Simplified,
    /// A trip needs a charge the home really gave, and every trip wears it a full trip.
    Realistic,
}

impl Mode {
    /// The Settings mode. Absent (headless tests, the relay) is Realistic: the physics
    /// the tests measure, the convention `life_support::is_realistic` uses.
    pub fn from_store(data: &DataStore) -> Self {
        let realistic = data.get::<Mutex<bool>>(MODE_KEY).and_then(|m| m.lock().ok().map(|v| *v)).unwrap_or(true);
        if realistic {
            Mode::Realistic
        } else {
            Mode::Simplified
        }
    }
}

/// The drone's running costs, data/mining/drone.ron (whose comments carry the sources).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DroneSpec {
    /// The real machine the numbers come from, for the panel and the log.
    pub model: String,
    /// Kilograms it lifts at its most: the hold's heaviest load must fit (a test holds
    /// `DRONE_CAPACITY` units of the heaviest ore in items.csv to it).
    pub payload_kg: f32,
    /// Watt-hours one round trip takes out of its battery.
    pub trip_wh: f32,
    /// Watt-hours its battery holds.
    pub pack_wh: f32,
    /// Watts its dock draws from the home's grid while charging it.
    pub charger_w: f32,
    /// The dock's place in the home's load shedding (1 critical, 5 optional).
    pub charger_priority: u8,
    /// Trips between services.
    pub service_trips: f32,
    /// Share of a trip's wear each trip adds in the Simplified mode.
    pub simplified_wear_share: f32,
    /// What a service takes: (item id, quantity).
    pub service_parts: Vec<(String, u32)>,
}

impl DroneSpec {
    /// Path of the file, relative to the data directory.
    pub const FILE: &'static str = "mining/drone.ron";

    /// Parse and check the file's text: a trip fits in the battery, the dock charges,
    /// and a service takes something.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let s: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        if !(s.trip_wh > 0.0 && s.trip_wh <= s.pack_wh) {
            return Err(format!("trip_wh {} must be above 0 and no more than pack_wh {}", s.trip_wh, s.pack_wh));
        }
        if !(s.charger_w > 0.0) {
            return Err(format!("charger_w {} must be above 0", s.charger_w));
        }
        if !(s.service_trips >= 1.0) {
            return Err(format!("service_trips {} must be at least 1", s.service_trips));
        }
        if !(s.simplified_wear_share > 0.0 && s.simplified_wear_share <= 1.0) {
            return Err(format!("simplified_wear_share {} must be above 0 and at most 1", s.simplified_wear_share));
        }
        if s.service_parts.is_empty() || s.service_parts.iter().any(|(_, q)| *q == 0) {
            return Err("service_parts must name at least one part, each at least 1".to_string());
        }
        Ok(s)
    }

    /// Disk first (modding), the copy built into the game when the disk copy is missing
    /// or this version cannot read it (`embedded_data::load_text_or_embedded`). None when
    /// neither loads: the drone then flies free, which the log says.
    pub fn load(data_dir: &std::path::Path) -> Option<Self> {
        match crate::embedded_data::load_text_or_embedded(data_dir, Self::FILE, Self::from_ron) {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!("{e}; the mining drone flies free (no energy, no wear) until it is fixed");
                None
            }
        }
    }

    /// The wear one trip adds in `mode`, in trips.
    pub fn wear_per_trip(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Realistic => 1.0,
            Mode::Simplified => self.simplified_wear_share,
        }
    }
}

/// The drone's own state between trips, saved with the home.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroneUnit {
    /// Watt-hours in its battery now. In flight it has already spent the trip's.
    pub pack_wh: f32,
    /// Wear since its last service, in trips.
    #[serde(default)]
    pub wear_trips: f32,
    /// A Launch waiting for the battery to charge (the Simplified mode): (asteroid id,
    /// manifest). It takes off by itself once the battery holds a trip.
    #[serde(default)]
    pub held: Option<(String, Vec<(String, u32)>)>,
    /// The player has been told it is due for its service (said once per wear-out).
    #[serde(default)]
    pub told_worn: bool,
}

impl DroneUnit {
    /// A new drone: a full battery, no wear.
    pub fn new(spec: &DroneSpec) -> Self {
        DroneUnit { pack_wh: spec.pack_wh, wear_trips: 0.0, held: None, told_worn: false }
    }

    /// Due for its service: it does not take off until it has one.
    pub fn worn_out(&self, spec: &DroneSpec) -> bool {
        self.wear_trips >= spec.service_trips - 1e-4
    }

    /// Holds a trip's charge.
    pub fn charged(&self, spec: &DroneSpec) -> bool {
        self.pack_wh >= spec.trip_wh - 1e-3
    }
}

/// What the Mining panel asks of the drone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DroneRequest {
    /// Service it with the parts `DroneSpec::service_parts` names.
    Repair,
    /// Drop a Launch that is waiting for the battery.
    CancelHeld,
}

/// One part of a service, with how many the player can reach now.
#[derive(Debug, Clone, PartialEq)]
pub struct ServicePart {
    pub id: String,
    pub name: String,
    pub need: u32,
    /// In the backpack and, where it counts, home storage.
    pub have: u32,
}

/// What the Mining panel shows of the drone, rebuilt every tick.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DroneReadout {
    pub realistic: bool,
    /// At its dock (no trip in the air).
    pub docked: bool,
    pub trip_wh: f32,
    pub pack_wh: f32,
    pub pack_capacity_wh: f32,
    /// Real seconds until the battery holds a trip at the rate it charges now; None when
    /// it already holds one or is not charging.
    pub ready_in_s: Option<f64>,
    /// What its battery is doing, in plain words.
    pub power_line: String,
    /// Wear since the last service and the service interval, in trips.
    pub wear_trips: f32,
    pub service_trips: f32,
    /// Whole trips it can still fly before its service.
    pub trips_left: u32,
    pub service_parts: Vec<ServicePart>,
    /// A service would go through now: some wear, and every part on hand.
    pub can_service: bool,
    /// The asteroid a Launch waiting for the battery is for.
    pub held_target: Option<String>,
    /// Why it cannot take off now, in plain words; None when it can.
    pub blocked: Option<String>,
}

impl DroneReadout {
    /// Share of its service interval still to fly, 0 to 1.
    pub fn wear_left(&self) -> f32 {
        if self.service_trips <= 0.0 {
            return 1.0;
        }
        (1.0 - self.wear_trips / self.service_trips).clamp(0.0, 1.0)
    }
}

/// Register the drone's data, its state (a new drone), the panel's request and readout,
/// and the mode (Simplified until the Settings value is published). lib.rs, at startup.
pub fn register(data_store: &mut DataStore) {
    let Some(spec) = DroneSpec::load(&crate::data_dir()) else { return };
    data_store.insert(UNIT_KEY, Mutex::new(DroneUnit::new(&spec)));
    data_store.insert(SPEC_KEY, spec);
    data_store.insert(REQUEST_KEY, Mutex::new(Option::<DroneRequest>::None));
    data_store.insert(READOUT_KEY, Mutex::new(Option::<DroneReadout>::None));
    data_store.insert(MODE_KEY, Mutex::new(false));
}

/// Publish the Settings mode (each frame, and before a save's time away is run).
pub fn publish_mode(data: &DataStore, realistic: bool) {
    if let Some(Ok(mut v)) = data.get::<Mutex<bool>>(MODE_KEY).map(|m| m.lock()) {
        *v = realistic;
    }
}

/// The drone's state as it stands, for the save. None with no drone data registered.
pub fn unit(data: &DataStore) -> Option<DroneUnit> {
    data.get::<Mutex<DroneUnit>>(UNIT_KEY).and_then(|m| m.lock().ok().map(|u| u.clone()))
}

/// Put back the state a save carried; None (a save from before) is a new drone.
pub fn set_unit(data: &DataStore, saved: Option<DroneUnit>) {
    let Some(spec) = data.get::<DroneSpec>(SPEC_KEY) else { return };
    if let Some(Ok(mut u)) = data.get::<Mutex<DroneUnit>>(UNIT_KEY).map(|m| m.lock()) {
        *u = saved.unwrap_or_else(|| DroneUnit::new(spec));
    }
}

/// Ask the drone for something (the Mining panel's Repair and Cancel, through the main loop).
pub fn request(data: &DataStore, req: DroneRequest) {
    if let Some(Ok(mut r)) = data.get::<Mutex<Option<DroneRequest>>>(REQUEST_KEY).map(|m| m.lock()) {
        *r = Some(req);
    }
}

/// What the Mining panel shows, as of the last tick.
pub fn readout(data: &DataStore) -> Option<DroneReadout> {
    data.get::<Mutex<Option<DroneReadout>>>(READOUT_KEY).and_then(|m| m.lock().ok().and_then(|r| r.clone()))
}

/// The drone's dock on the home's grid: an optional load the ElectricalSystem feeds or
/// sheds. Spawned by the DroneSystem on the home's main power island (the one holding
/// the most battery capacity) and moved if the home's islands change.
#[derive(Debug, Clone, Copy, Default)]
pub struct DroneCharger;

/// Settings > Gameplay > Mining drone's hint: what each mode does, with the numbers read
/// from the data it describes, so the two cannot drift.
pub fn mode_hint() -> String {
    static SHIPPED: std::sync::OnceLock<Option<DroneSpec>> = std::sync::OnceLock::new();
    hint_text(SHIPPED.get_or_init(|| DroneSpec::load(&crate::data_dir())).as_ref())
}

/// The hint's words for `spec` (none: the drone flies free in both modes).
fn hint_text(spec: Option<&DroneSpec>) -> String {
    let Some(s) = spec else {
        return "No drone data is loaded, so the drone flies free in both modes.".to_string();
    };
    let charge_s = f64::from(s.trip_wh) * 3600.0 / f64::from(s.charger_w);
    let rate = if (s.simplified_wear_share - 0.5).abs() < 1e-3 {
        "half as fast".to_string()
    } else {
        format!("at {:.0}% of the rate", s.simplified_wear_share * 100.0)
    };
    format!(
        "Each trip takes a full charge of the drone's battery, {}, which its dock takes from the \
         home's power at {:.1} kW: {} of game time for a trip's charge. Every {:.0} trips it needs \
         a service, with parts you can make. Realistic: a dock the home cannot power does not \
         charge it, and without a trip's charge it cannot take off. Simplified: the dock charges \
         it anyway, so a flat home only delays a trip, and it wears {rate} ({:.0} trips a service).",
        kwh(s.trip_wh),
        s.charger_w / 1000.0,
        duration_words(charge_s).trim_start_matches("about ").to_string(),
        s.service_trips,
        s.service_trips / s.simplified_wear_share,
    )
}

/// Kilowatt-hours, for the player: "4.3 kWh".
pub fn kwh(wh: f32) -> String {
    format!("{:.1} kWh", wh / 1000.0)
}

/// Minutes or hours, for the player: "about 43 min", "about 2.5 h".
pub fn duration_words(secs: f64) -> String {
    let mins = (secs / 60.0).ceil().max(1.0);
    if mins < 90.0 {
        format!("about {mins:.0} min")
    } else {
        format!("about {:.1} h", mins / 60.0)
    }
}

/// Mission phase durations (real seconds). Dev-scale; tune later / move to data.
const OUTBOUND_SECS: f32 = 5.0;
const MINING_SECS: f32 = 5.0;
const RETURNING_SECS: f32 = 5.0;
/// Total ore units a drone's hold carries per trip (a manifest's units sum to this).
/// Exposed so the Mining UI can cap the allocation.
pub const DRONE_CAPACITY: u32 = 10;

/// Real seconds the given mission phase lasts — exposed so the Mining UI can draw
/// a per-stage progress bar (the operator's "show the drone is working" cue).
pub fn phase_secs(phase: &DronePhase) -> f32 {
    match phase {
        DronePhase::Outbound => OUTBOUND_SECS,
        DronePhase::Mining => MINING_SECS,
        DronePhase::Returning => RETURNING_SECS,
        DronePhase::Done => 0.0,
    }
}

/// What a drone needs done to OTHER entities this tick — computed while iterating
/// drones (a `&mut Drone` query) and applied afterwards, so the cross-entity
/// `&mut World` borrows never overlap the drone query.
enum DroneIntent {
    /// Fill the drone's hold per its `manifest`, pulling each ore from its ONE target
    /// asteroid (bounded by what that asteroid holds; no cross-asteroid spillover).
    Mine {
        drone: hecs::Entity,
        target: String,
        manifest: Vec<(String, u32)>,
    },
    /// Deliver the drone's whole `cargo` into `home`'s inventory, then despawn it.
    /// `target` and `manifest` are the trip's, for when it comes back empty.
    Deliver {
        drone: hecs::Entity,
        home: u64,
        target: String,
        manifest: Vec<(String, u32)>,
        cargo: Vec<(String, u32)>,
    },
}

pub struct DroneSystem;

impl DroneSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DroneSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for DroneSystem {
    fn name(&self) -> &str {
        "DroneSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let item_registry = data.get::<ItemRegistry>("item_registry");
        // Running costs (first-hour audit S5): with no drone data the drone flies free.
        let spec: Option<DroneSpec> = data.get::<DroneSpec>(SPEC_KEY).cloned();
        let mode = Mode::from_store(data);
        let sdt = crate::systems::time::scaled_dt(dt, data);
        let mut unit: Option<DroneUnit> = spec.as_ref().map(|s| unit(data).unwrap_or_else(|| DroneUnit::new(s)));
        let mut charge = Charging::Aboard;
        if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_mut()) {
            // The panel's Repair and Cancel.
            let req = data.get::<Mutex<Option<DroneRequest>>>(REQUEST_KEY).and_then(|m| m.lock().ok().and_then(|mut r| r.take()));
            match req {
                Some(DroneRequest::Repair) => service(world, data, spec, u, item_registry),
                Some(DroneRequest::CancelHeld) => {
                    if u.held.take().is_some() {
                        notice(data, "The drone's waiting launch is cancelled.".to_string());
                    }
                }
                None => {}
            }
            // What the dock gave the battery since the last tick.
            charge = charge_at_dock(world, spec, mode, u, sdt);
        }

        // ── COMMISSION: drain the channel (the Mining panel writes a TARGET asteroid id
        //    + a manifest) and launch ONE drone — home = the player, target = that asteroid.
        let order: Option<(String, Vec<(String, u32)>)> = data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("commission_drone")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some((target, manifest)) = order {
            // ONE drone per player: skip a new launch if one is already in flight.
            let already_flying = world.query::<&Drone>().iter().next().is_some();
            let manifest: Vec<(String, u32)> =
                manifest.into_iter().filter(|(_, u)| *u > 0).collect();
            if already_flying {
                log::info!("[Mining] a drone is already in flight (one per player)");
            } else if manifest.is_empty() {
                log::info!("[Mining] empty manifest; drone not launched");
            } else {
                match (spec.as_ref(), unit.as_mut()) {
                    // Only launch if the TARGET asteroid still exists — a depleted/stale id
                    // shouldn't burn the player's single drone slot on a dud trip. Its
                    // position scales the travel time + the map dot.
                    (None, _) | (_, None) => {
                        launch(world, data, &target, &manifest);
                    }
                    (Some(spec), Some(u)) => {
                        commission_with_costs(world, data, spec, mode, u, &charge, &target, &manifest);
                    }
                }
            }
        }

        // ── A LAUNCH WAITING FOR THE BATTERY (Simplified) goes once it holds a trip.
        if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_mut()) {
            let docked = world.query::<&Drone>().iter().next().is_none();
            if docked && u.charged(spec) && !u.worn_out(spec) {
                if let Some((target, manifest)) = u.held.take() {
                    if launch(world, data, &target, &manifest) {
                        pay_for_trip(spec, mode, u);
                    }
                }
            }
        }

        // ── STANDING ORDER refire (economy automation Phase 1, v0.663): while a
        //    standing order exists and NOTHING is flying or queued, re-commission
        //    it. Living at tick level (not inside the Deliver arm) makes the loop
        //    self-healing: it resumes after a world reload that despawned an
        //    in-flight drone, not just after a clean delivery. Runs AFTER the
        //    drain above, so it takes effect next tick (no same-tick relaunch).
        //    With running costs it waits at the dock for a trip's charge and for
        //    a service when one is due, so it never asks for a trip that would be
        //    refused (and never repeats a refusal's notice every tick).
        {
            let any_drone = world.query::<&Drone>().iter().next().is_some();
            let ready = match (spec.as_ref(), unit.as_ref()) {
                (Some(spec), Some(u)) => u.held.is_none() && u.charged(spec) && !u.worn_out(spec),
                _ => true,
            };
            if !any_drone && ready {
                let standing: Option<(String, Vec<(String, u32)>)> = data
                    .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>(
                        "auto_mine_order",
                    )
                    .and_then(|m| m.lock().ok().and_then(|s| s.clone()));
                if let Some(order) = standing {
                    if let Some(slot) = data
                        .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>(
                            "commission_drone",
                        )
                    {
                        if let Ok(mut s) = slot.lock() {
                            if s.is_none() {
                                log::info!("[Mining] standing order: re-commissioning {}", order.0);
                                *s = Some(order);
                            }
                        }
                    }
                }
            }
        }

        // ── ADVANCE: tick each drone's phase machine, recording cross-entity intents.
        // Phase timers run on GAME time (v0.663, `sdt` above): accelerated testing
        // speeds the trips too, not just the clock. Absent game_time (unit tests) = raw dt.
        let mut intents: Vec<DroneIntent> = Vec::new();
        for (entity, drone) in world.query_mut::<&mut Drone>() {
            drone.phase_time += sdt;
            let dur = drone.phase_duration(drone.phase);
            match drone.phase {
                DronePhase::Outbound if drone.phase_time >= dur => {
                    drone.phase = DronePhase::Mining;
                    drone.phase_time = 0.0;
                    intents.push(DroneIntent::Mine {
                        drone: entity,
                        target: drone.target.clone(),
                        manifest: drone.manifest.clone(),
                    });
                }
                DronePhase::Mining if drone.phase_time >= dur => {
                    drone.phase = DronePhase::Returning;
                    drone.phase_time = 0.0;
                }
                DronePhase::Returning if drone.phase_time >= dur => {
                    drone.phase = DronePhase::Done;
                    drone.phase_time = 0.0;
                    intents.push(DroneIntent::Deliver {
                        drone: entity,
                        home: drone.home,
                        target: drone.target.clone(),
                        manifest: drone.manifest.clone(),
                        cargo: drone.cargo.clone(),
                    });
                }
                _ => {}
            }
        }

        // ── APPLY: mutate the asteroid / home inventory / despawn the drone (the drone
        //    query borrow is released now, so these &mut World gets are conflict-free).
        for intent in intents {
            match intent {
                DroneIntent::Mine { drone, target, manifest } => {
                    let collected = mine(world, &target, &manifest);
                    if let Ok(mut d) = world.get::<&mut Drone>(drone) {
                        d.cargo = collected;
                    }
                }
                DroneIntent::Deliver { drone, home, target, manifest, cargo } => {
                    deliver_haul(world, data, item_registry, home, &cargo);
                    let _ = world.despawn(drone);
                    // (Standing-order relaunch happens at TICK level above, not
                    // here -- see the refire block after the commission drain.)
                    if !cargo.iter().any(|(_, q)| *q > 0) {
                        came_back_empty(world, data, item_registry, &target, &manifest);
                    }
                    if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_mut()) {
                        tell_if_worn(data, spec, u, item_registry);
                    }
                }
            }
        }

        remove_mined_out(world);

        // ── The dock's draw for the next tick, what the panel shows, and the state kept.
        if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_ref()) {
            let docked = world.query::<&Drone>().iter().next().is_none();
            set_charger_draw(world, spec, docked && u.pack_wh < spec.pack_wh - 1e-3);
            let shown = build_readout(world, data, spec, mode, u, docked, &charge, item_registry);
            if let Some(Ok(mut r)) = data.get::<Mutex<Option<DroneReadout>>>(READOUT_KEY).map(|m| m.lock()) {
                *r = Some(shown);
            }
            if let Some(Ok(mut slot)) = data.get::<Mutex<DroneUnit>>(UNIT_KEY).map(|m| m.lock()) {
                *slot = u.clone();
            }
        }
    }
}

// ── Running costs (first-hour audit S5, 2026-10-12) ─────────────────

/// What the dock did for the battery this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Charging {
    /// The drone is out: its battery is aboard.
    Aboard,
    /// Docked and holding a full battery.
    Full,
    /// Charging from the home's grid at these watts.
    FromHome(f32),
    /// The home's grid shed the dock (it is short of power). Simplified charges
    /// anyway at the dock's rate, Realistic does not charge.
    Shed,
    /// The home has no battery bank or ship tie for a dock to draw on.
    NoGrid,
}

/// Post one line for the player (the HUD's notices).
fn notice(data: &DataStore, line: String) {
    log::info!("[Mining] {line}");
    if let Some(Ok(mut n)) = data.get::<Mutex<Vec<String>>>("player_notices").map(|m| m.lock()) {
        n.push(line);
    }
}

/// A part list for the player: "2 Carbon Fiber Sheet and 1 Rubber Sheet".
fn parts_words(parts: &[(String, u32)], item_registry: Option<&ItemRegistry>) -> String {
    let named: Vec<String> = parts
        .iter()
        .map(|(id, q)| format!("{q} {}", item_registry.and_then(|r| r.items.get(id)).map(|d| d.name.clone()).unwrap_or_else(|| id.clone())))
        .collect();
    match named.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The home's main power island, where the dock goes: the island holding the most
/// battery capacity (the lowest such island number on a tie), else the first island
/// tied to the ship's bus. `None` when the home has neither: nothing to charge from.
/// `Some(None)` is the bucket of power entities without a circuit (tests, older spawns).
fn dock_island(world: &hecs::World) -> Option<Option<u32>> {
    let mut by: std::collections::BTreeMap<Option<u32>, f32> = std::collections::BTreeMap::new();
    for (_e, (b, pc)) in world.query::<(&crate::ecs::components::Battery, Option<&PowerCircuit>)>().iter() {
        *by.entry(pc.map(|p| p.island)).or_default() += b.capacity_wh.max(0.0);
    }
    let best = by.iter().fold(None::<(Option<u32>, f32)>, |acc, (k, v)| match acc {
        Some((_, bv)) if bv >= *v => acc,
        _ => Some((*k, *v)),
    });
    if let Some((island, _)) = best {
        return Some(island);
    }
    let taps = crate::systems::ship_power::taps(world);
    let mut islands: Vec<Option<u32>> = taps.keys().copied().collect();
    islands.sort();
    islands.into_iter().next()
}

/// The dock's entity, put on the home's main island (spawned the first time, moved
/// when the home's islands change, removed when the home has none). Returns it.
fn ensure_charger(world: &mut hecs::World, spec: &DroneSpec) -> Option<hecs::Entity> {
    let island = dock_island(world);
    let existing: Vec<hecs::Entity> = world.query::<&DroneCharger>().iter().map(|(e, _)| e).collect();
    let Some(island) = island else {
        for e in existing {
            let _ = world.despawn(e);
        }
        return None;
    };
    let e = match existing.first() {
        Some(e) => *e,
        None => world.spawn((DroneCharger, PowerConsumer { draw_watts: 0.0, priority: spec.charger_priority, enabled: true })),
    };
    for extra in existing.iter().skip(1) {
        let _ = world.despawn(*extra);
    }
    let on = world.get::<&PowerCircuit>(e).ok().map(|p| p.island);
    if on != island {
        match island {
            Some(i) => {
                let _ = world.insert_one(e, PowerCircuit { island: i });
            }
            None => {
                let _ = world.remove_one::<PowerCircuit>(e);
            }
        }
    }
    Some(e)
}

/// Set what the dock asks of the home's grid for the next tick: its charger's watts
/// while the docked battery is not full, nothing otherwise.
fn set_charger_draw(world: &mut hecs::World, spec: &DroneSpec, wants: bool) {
    for (_e, (_c, pc)) in world.query_mut::<(&DroneCharger, &mut PowerConsumer)>() {
        pc.draw_watts = if wants { spec.charger_w } else { 0.0 };
    }
}

/// Charge the docked battery for one tick of `sdt` game seconds and say how. The dock
/// asked the grid for its watts last tick (`set_charger_draw`); the ElectricalSystem,
/// which ticks first, has fed it (the home's generation, then its batteries, then the
/// ship where the home is tied to it, metered) or shed it, and charged the home for what
/// it fed. The battery gets exactly that. A shed dock charges it anyway in Simplified.
fn charge_at_dock(world: &mut hecs::World, spec: &DroneSpec, mode: Mode, u: &mut DroneUnit, sdt: f32) -> Charging {
    let charger = ensure_charger(world, spec);
    if world.query::<&Drone>().iter().next().is_some() {
        return Charging::Aboard;
    }
    if u.pack_wh >= spec.pack_wh - 1e-3 {
        return Charging::Full;
    }
    let fed = charger
        .and_then(|e| world.get::<&PowerConsumer>(e).ok().map(|c| (c.enabled, c.draw_watts)));
    let how = match fed {
        Some((true, w)) if w > 0.0 => Charging::FromHome(w),
        // Asked for nothing last tick (it just came home): it asks this tick.
        Some((true, _)) => Charging::FromHome(spec.charger_w),
        Some((false, _)) => Charging::Shed,
        None => Charging::NoGrid,
    };
    let watts = match (how, mode) {
        (Charging::FromHome(w), _) => fed.map_or(0.0, |(_, d)| d).min(w),
        (Charging::Shed | Charging::NoGrid, Mode::Simplified) => spec.charger_w,
        _ => 0.0,
    };
    u.pack_wh = (u.pack_wh + watts.max(0.0) * sdt.max(0.0) / 3600.0).min(spec.pack_wh);
    how
}

/// A trip leaves: its charge comes out of the battery and it wears the drone.
fn pay_for_trip(spec: &DroneSpec, mode: Mode, u: &mut DroneUnit) {
    u.pack_wh = (u.pack_wh - spec.trip_wh).max(0.0);
    u.wear_trips += spec.wear_per_trip(mode);
}

/// A Launch from the Mining panel (or a standing order's refire) with running costs:
/// a drone due for its service is refused, one without a trip's charge is refused in
/// Realistic and waits for it in Simplified, and otherwise it takes off and pays.
#[allow(clippy::too_many_arguments)]
fn commission_with_costs(
    world: &mut hecs::World,
    data: &DataStore,
    spec: &DroneSpec,
    mode: Mode,
    u: &mut DroneUnit,
    charge: &Charging,
    target: &str,
    manifest: &[(String, u32)],
) {
    let item_registry = data.get::<ItemRegistry>("item_registry");
    if u.worn_out(spec) {
        notice(
            data,
            format!(
                "The drone stays docked: it is due for its service, which takes {}. Repair it from the Mining panel.",
                parts_words(&spec.service_parts, item_registry)
            ),
        );
        return;
    }
    if !u.charged(spec) {
        let rate = match (charge, mode) {
            (Charging::FromHome(w), _) => *w,
            (Charging::Shed | Charging::NoGrid, Mode::Simplified) => spec.charger_w,
            _ => 0.0,
        };
        let wait = (rate > 0.0).then(|| f64::from(spec.trip_wh - u.pack_wh) * 3600.0 / f64::from(rate));
        let scale = crate::systems::time::scaled_dt(1.0, data).max(1e-6) as f64;
        let when = wait.map(|w| duration_words(w / scale));
        match mode {
            Mode::Simplified => {
                u.held = Some((target.to_string(), manifest.to_vec()));
                notice(
                    data,
                    format!(
                        "The drone takes off when its battery holds a trip's {}, in {}.",
                        kwh(spec.trip_wh),
                        when.unwrap_or_else(|| "a while".to_string())
                    ),
                );
            }
            Mode::Realistic => {
                let keep = standing_order(data).is_some_and(|(t, _)| t == target);
                let why = match when {
                    Some(w) => format!("its battery is charging and holds a trip's {} in {w}", kwh(spec.trip_wh)),
                    None => format!(
                        "its battery holds {} of the {} a trip needs, and the home has no power to spare to charge it",
                        kwh(u.pack_wh),
                        kwh(spec.trip_wh)
                    ),
                };
                let then = if keep { " Keep mining sends it when it is charged." } else { " Launch it again when it is charged." };
                notice(data, format!("The drone cannot take off yet: {why}.{then}"));
            }
        }
        return;
    }
    if launch(world, data, target, manifest) {
        pay_for_trip(spec, mode, u);
        u.held = None;
    }
}

/// When the trip just home has worn the drone out, say so once.
fn tell_if_worn(data: &DataStore, spec: &DroneSpec, u: &mut DroneUnit, item_registry: Option<&ItemRegistry>) {
    if u.worn_out(spec) && !u.told_worn {
        u.told_worn = true;
        notice(
            data,
            format!(
                "The drone is due for its service and stays docked until it gets one: {}. Repair it from the Mining panel.",
                parts_words(&spec.service_parts, item_registry)
            ),
        );
    }
}

/// Is Creative's free-resources switch on (the systems that take items skip it then)?
fn free_resources(data: &DataStore) -> bool {
    data.get::<Mutex<bool>>("creative_mode").and_then(|m| m.lock().ok().map(|v| *v)).unwrap_or(false)
}

/// How many of each service part the player can reach now: the backpack, then home
/// storage where it counts (`crafting::home_store::HomeStore`).
fn parts_on_hand(world: &hecs::World, data: &DataStore, spec: &DroneSpec, item_registry: Option<&ItemRegistry>) -> Vec<ServicePart> {
    let store = crate::systems::crafting::home_store::HomeStore::here(data);
    let pack = world.query::<(&Inventory, &Controllable)>().iter().next().map(|(_, (inv, _))| inv.clone());
    spec.service_parts
        .iter()
        .map(|(id, need)| ServicePart {
            id: id.clone(),
            name: item_registry.and_then(|r| r.items.get(id)).map(|d| d.name.clone()).unwrap_or_else(|| id.clone()),
            need: *need,
            have: pack.as_ref().map_or(0, |p| p.count_item(id)) + store.count(id),
        })
        .collect()
}

/// Service the drone: take its parts (the backpack first, then home storage) and
/// clear its wear. Nothing is taken unless every part is there, and the player is
/// told what is missing.
fn service(world: &mut hecs::World, data: &DataStore, spec: &DroneSpec, u: &mut DroneUnit, item_registry: Option<&ItemRegistry>) {
    if u.wear_trips <= 0.0 {
        notice(data, "The drone needs no service yet.".to_string());
        return;
    }
    if !free_resources(data) {
        let parts = parts_on_hand(world, data, spec, item_registry);
        let missing: Vec<(String, u32)> =
            parts.iter().filter(|p| p.have < p.need).map(|p| (p.id.clone(), p.need - p.have)).collect();
        if !missing.is_empty() {
            notice(data, format!("The drone's service needs {} more.", parts_words(&missing, item_registry)));
            return;
        }
        let store = crate::systems::crafting::home_store::HomeStore::here(data);
        let player = world.query::<(&Inventory, &Controllable)>().iter().next().map(|(e, _)| e);
        for (id, need) in &spec.service_parts {
            let in_pack = player.and_then(|e| world.get::<&Inventory>(e).ok().map(|i| i.count_item(id))).unwrap_or(0);
            let from_pack = in_pack.min(*need);
            if let Some(Ok(mut inv)) = player.map(|e| world.get::<&mut Inventory>(e)) {
                inv.remove_item(id, from_pack);
            }
            store.take(id, need - from_pack);
        }
    }
    u.wear_trips = 0.0;
    u.told_worn = false;
    notice(
        data,
        format!(
            "The drone is serviced ({}): good for {:.0} more trips.",
            parts_words(&spec.service_parts, item_registry),
            spec.service_trips
        ),
    );
}

/// What the Mining panel shows, in numbers and plain words.
#[allow(clippy::too_many_arguments)]
fn build_readout(
    world: &hecs::World,
    data: &DataStore,
    spec: &DroneSpec,
    mode: Mode,
    u: &DroneUnit,
    docked: bool,
    charge: &Charging,
    item_registry: Option<&ItemRegistry>,
) -> DroneReadout {
    let scale = crate::systems::time::scaled_dt(1.0, data).max(1e-6) as f64;
    let rate = match (charge, mode) {
        (Charging::FromHome(w), _) => *w,
        (Charging::Shed | Charging::NoGrid, Mode::Simplified) => spec.charger_w,
        _ => 0.0,
    };
    let ready_in_s = (docked && !u.charged(spec) && rate > 0.0)
        .then(|| f64::from(spec.trip_wh - u.pack_wh) * 3600.0 / f64::from(rate) / scale);
    let when = ready_in_s.map(duration_words);
    let rate_words = format!("{:.1} kW", spec.charger_w / 1000.0);
    let power_line = match charge {
        Charging::Aboard => "Its battery is aboard; the dock charges it when it is home.".to_string(),
        Charging::Full => "Battery full: ready for a trip.".to_string(),
        Charging::FromHome(_) => match &when {
            Some(w) => format!("Charging from the home's power at {rate_words}: a trip's charge in {w}."),
            None => format!("Charging from the home's power at {rate_words}."),
        },
        Charging::Shed => match mode {
            Mode::Realistic => "Not charging: the home has no power to spare. It charges again when the home's panels or batteries can give it power.".to_string(),
            Mode::Simplified => format!(
                "The home has no power to spare, so the dock charges it anyway (Simplified): a trip's charge in {}.",
                when.clone().unwrap_or_else(|| "a while".to_string())
            ),
        },
        Charging::NoGrid => match mode {
            Mode::Realistic => "Not charging: the home has no battery bank for the dock to charge it from.".to_string(),
            Mode::Simplified => format!(
                "The home has no battery bank, so the dock charges it anyway (Simplified): a trip's charge in {}.",
                when.clone().unwrap_or_else(|| "a while".to_string())
            ),
        },
    };
    let parts = parts_on_hand(world, data, spec, item_registry);
    let free = free_resources(data);
    let can_service = u.wear_trips > 0.0 && (free || parts.iter().all(|p| p.have >= p.need));
    let per = spec.wear_per_trip(mode).max(1e-6);
    let trips_left = ((spec.service_trips - u.wear_trips).max(0.0) / per + 1e-3).floor() as u32;
    let blocked = if u.worn_out(spec) {
        Some("Due for its service: it stays docked until it is repaired.".to_string())
    } else if docked && !u.charged(spec) && mode == Mode::Realistic {
        Some(format!("Its battery holds {} of the {} a trip needs.", kwh(u.pack_wh), kwh(spec.trip_wh)))
    } else {
        None
    };
    let held_target = u.held.as_ref().map(|(t, _)| {
        world.query::<&AsteroidBody>().iter().find(|(_, a)| a.id == *t).map(|(_, a)| a.name.clone()).unwrap_or_else(|| t.clone())
    });
    DroneReadout {
        realistic: mode == Mode::Realistic,
        docked,
        trip_wh: spec.trip_wh,
        pack_wh: u.pack_wh,
        pack_capacity_wh: spec.pack_wh,
        ready_in_s,
        power_line,
        wear_trips: u.wear_trips,
        service_trips: spec.service_trips,
        trips_left,
        service_parts: parts,
        can_service,
        held_target,
        blocked,
    }
}

/// Launch the drone at `target` with `manifest`, home = the player, when that
/// asteroid still exists. A standing order aimed at an asteroid that is gone
/// (mined out and deleted) is ended here, or it would refire a dead
/// commission every trip forever (v0.663). Shared by the tick's commission
/// and the time away (`advance_away`). Returns whether a drone launched.
fn launch(world: &mut hecs::World, data: &DataStore, target: &str, manifest: &[(String, u32)]) -> bool {
    // Only launch if the TARGET asteroid still exists: a depleted or stale id
    // shouldn't burn the player's single drone slot on a dud trip. Its
    // position scales the travel time + the map dot.
    let target_pos = world
        .query::<&AsteroidBody>()
        .iter()
        .find(|(_, a)| a.id == target)
        .map(|(_, a)| a.position);
    let home: Option<u64> = world
        .query::<(&Inventory, &Controllable)>()
        .iter()
        .next()
        .map(|(e, _)| e.to_bits().into());
    match (target_pos, home) {
        (Some(target_pos), Some(home)) => {
            world.spawn((Drone {
                home,
                target: target.to_string(),
                manifest: manifest.to_vec(),
                phase: DronePhase::Outbound,
                phase_time: 0.0,
                cargo: Vec::new(),
                home_pos: [0.0, 0.0, 0.0],
                target_pos,
            },));
            log::info!("[Mining] commissioned a drone for {target}: {manifest:?}");
            true
        }
        (None, _) => {
            log::info!("[Mining] target asteroid '{target}' not found; not launching");
            if let Some(slot) =
                data.get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
            {
                if let Ok(mut s) = slot.lock() {
                    if s.as_ref().map_or(false, |(t, _)| *t == target) {
                        log::info!("[Mining] standing order for '{target}' ended (target gone)");
                        *s = None;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Pull each requested ore from the ONE target asteroid, bounded by what it
/// holds. No spillover to other asteroids: one run mines one asteroid, so the
/// haul is capped by that asteroid's stock.
fn mine(world: &mut hecs::World, target: &str, manifest: &[(String, u32)]) -> Vec<(String, u32)> {
    let target_e = world
        .query::<&AsteroidBody>()
        .iter()
        .find(|(_, a)| a.id == target)
        .map(|(e, _)| e);
    let mut collected: Vec<(String, u32)> = Vec::new();
    if let Some(aid) = target_e {
        for (ore, units) in manifest {
            if let Ok(mut body) = world.get::<&mut AsteroidBody>(aid) {
                let took = body.take(ore, *units as f32);
                if took > 0 {
                    collected.push((ore.clone(), took));
                }
            }
        }
    }
    log::info!("[Mining] drone extracted {collected:?} from {target}");
    collected
}

/// Land a haul at home and train Mining for it. Returns the units delivered.
///
/// HOME STORAGE takes it whenever the home has storage (BUG-150, 2026-10-04):
/// the haul goes on the "home_stock_outputs" channel, which the main loop
/// files into the Barn after the tick exactly as it files an automated
/// machine's batch (engine::stock_piles::receive_machine_outputs). A drone
/// that brings ore home unloads it into the home's store, not into the
/// player's pockets, and home storage is the only store the home's automated
/// machines draw on since BUG-150, so drone ore still becomes ingots and
/// hammers untouched. Where there is no home storage (headless, the older
/// tests) it lands in the backpack (`home` = the player's entity bits) as it
/// always did.
fn deliver_haul(
    world: &mut hecs::World,
    data: &DataStore,
    item_registry: Option<&ItemRegistry>,
    home: u64,
    cargo: &[(String, u32)],
) -> u32 {
    let Some(home_e) = hecs::Entity::from_bits(home) else { return 0 };
    if let Some(slot) = data.get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs") {
        let landed: Vec<(String, u32)> = cargo.iter().filter(|(_, q)| *q > 0).cloned().collect();
        let total: u32 = landed.iter().map(|(_, q)| q).sum();
        if let Ok(mut out) = slot.lock() {
            out.extend(landed);
        }
        if total > 0 {
            log::info!("[Mining] drone delivered {total} units to home storage");
            // A delivered haul trains Mining (1 XP per ore unit).
            crate::systems::skills::award_skill_xp(data, "mining", total);
        }
        return total;
    }
    let mut total = 0u32;
    for (ore, qty) in cargo {
        if *qty == 0 {
            continue;
        }
        let max_stack = item_registry.map(|r| r.max_stack_for(ore)).unwrap_or(99);
        if let Ok(mut inv) = world.get::<&mut Inventory>(home_e) {
            // Deliberately NOT volume-gated (Stage A slice 2): the operator
            // ruling below (never vanish a haul) predates and outranks the
            // volume gate here; the home stock behaves as base storage.
            // Revisit when home storage gets its own Container volumes.
            let overflow = inv.add_item(ore, *qty, max_stack);
            if overflow > 0 {
                // A hauled load must NEVER vanish because the backpack is
                // packed (operator field report 2026-07-04: a 36/36
                // seed-filled backpack silently ate an entire iron haul,
                // starving the smelter). Grow the home stock -- the same
                // ensure_slots the dev-stock path uses -- and land the rest.
                let occupied = inv.slots.iter().filter(|s| s.is_some()).count();
                let extra = (overflow as usize).div_ceil(max_stack.max(1) as usize);
                inv.ensure_slots(occupied + extra);
                inv.add_item(ore, overflow, max_stack);
            }
            total += *qty;
        }
    }
    if total > 0 {
        log::info!("[Mining] drone delivered {total} units home");
        // A delivered haul trains Mining (1 XP per ore unit).
        crate::systems::skills::award_skill_xp(data, "mining", total);
    }
    total
}

/// A trip came home with nothing: its asteroid holds none of what the trip
/// asked for, so every trip after it would come home empty too. A standing
/// order for that asteroid ends here, and the player is told either way
/// (first-hour audit 2026-10-04, Friction 5: an asteroid outlives one of its
/// ores, since it is deleted only when every ore is gone, and Keep mining used
/// to relaunch empty trips forever, with the drone never home). The order is
/// matched by its asteroid, as `launch` ends one whose asteroid is gone.
fn came_back_empty(
    world: &hecs::World,
    data: &DataStore,
    item_registry: Option<&ItemRegistry>,
    target: &str,
    manifest: &[(String, u32)],
) {
    let mut ended = false;
    if let Some(slot) = data.get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order") {
        if let Ok(mut s) = slot.lock() {
            if s.as_ref().is_some_and(|(t, _)| t == target) {
                *s = None;
                ended = true;
            }
        }
    }
    let rock = world
        .query::<&AsteroidBody>()
        .iter()
        .find(|(_, a)| a.id == target)
        .map(|(_, a)| a.name.clone())
        .unwrap_or_else(|| target.to_string());
    let ores: Vec<String> = manifest
        .iter()
        .map(|(ore, _)| item_registry.and_then(|r| r.items.get(ore)).map(|d| d.name.clone()).unwrap_or_else(|| ore.clone()))
        .collect();
    let what = match ores.as_slice() {
        [] => "ore of the kind it was sent for".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    };
    let mut line = format!("The drone came back empty from {rock}: it holds no more {what}.");
    if ended {
        line.push_str(" Keep mining is off.");
    }
    log::info!("[Mining] {line}");
    if let Some(slot) = data.get::<std::sync::Mutex<Vec<String>>>("player_notices") {
        if let Ok(mut n) = slot.lock() {
            n.push(line);
        }
    }
}

/// DELETE fully-consumed asteroids (the operator's "deleted when consumed").
fn remove_mined_out(world: &mut hecs::World) {
    let depleted: Vec<hecs::Entity> = world
        .query::<&AsteroidBody>()
        .iter()
        .filter(|(_, a)| a.total_remaining() < 1.0)
        .map(|(e, _)| e)
        .collect();
    for e in depleted {
        let _ = world.despawn(e);
        log::info!("[Mining] asteroid depleted and removed");
    }
}

// ── The time away (offline progression, 2026-09-27) ─────────────────

/// The drone's standing order ("Keep mining"), as the save stores it.
pub fn standing_order(data: &DataStore) -> Option<(String, Vec<(String, u32)>)> {
    data.get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
        .and_then(|m| m.lock().ok().and_then(|s| s.clone()))
}

/// Put back the standing order a save carried (save_load::resume_home). It is
/// the player's own setting, like a craft batch they started, so it returns
/// whether or not the time away counts.
pub fn set_standing_order(data: &DataStore, order: Option<(String, Vec<(String, u32)>)>) {
    if let Some(slot) = data.get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order") {
        if let Ok(mut s) = slot.lock() {
            *s = order;
        }
    }
}

/// Most trips one return may catch up. A trip is at least nine seconds and
/// every one that brings ore home takes it from a finite asteroid, so a real
/// home never comes near this; it only bounds a pathological save.
const MAX_AWAY_TRIPS: usize = 100_000;

/// Fly the drone through `secs` of time the player was away (offline
/// progression, docs/design/offline-progression.md), with exactly the
/// session's rules: the trip in flight finishes, and while a standing order
/// is set it keeps flying the same trip until its asteroid is mined out. Ore
/// comes only out of the asteroid, so a haul is bounded by what is really
/// there, and it lands where a session haul does (home storage, BUG-150).
/// A trip still in the air when the time runs out is left mid-flight, where
/// the player finds it.
///
/// Returns each haul as (seconds into the time away, cargo), so the
/// automated machines, which run through the same hours afterwards
/// (crafting::away), use ore only from the moment it arrived.
///
/// RUNNING COSTS (first-hour audit S5, 2026-10-12), with the drone's data
/// registered: while it is docked its battery charges, at its dock's watts
/// but no faster than `spare_w`, the power the home could give it while away
/// (the ship's reactor where the home is tied to it, else the home's own day
/// average past its loads, `crafting::away::day_power_balance`; the
/// Simplified mode charges at the dock's rate whatever the home can spare).
/// A trip leaves only with a trip's charge, pays it and wears the drone, a
/// drone due for its service stays docked, and a Launch that was waiting for
/// the battery goes first. What the home gave the dock comes back in
/// `Away::drawn_wh`, for the machines' share of the same spare power.
///
/// Nothing here destroys anything: the wear only counts toward a service, and
/// the charge is the home's spare power.
pub fn advance_away(world: &mut hecs::World, data: &DataStore, secs: f64, spare_w: f64) -> Away {
    let mut away = Away::default();
    if secs <= 0.0 {
        return away;
    }
    let item_registry = data.get::<ItemRegistry>("item_registry");
    let spec: Option<DroneSpec> = data.get::<DroneSpec>(SPEC_KEY).cloned();
    let mode = Mode::from_store(data);
    let mut unit: Option<DroneUnit> = spec.as_ref().map(|s| unit(data).unwrap_or_else(|| DroneUnit::new(s)));
    // The dock's rate while away, W: what the home pays, and in all (Simplified
    // makes up the rest).
    let (paid_w, rate_w) = match &spec {
        Some(s) => {
            let paid = f64::from(s.charger_w).min(spare_w.max(0.0));
            (paid, if mode == Mode::Simplified { f64::from(s.charger_w) } else { paid })
        }
        None => (0.0, 0.0),
    };
    // Charge the docked battery for `span` seconds.
    let charge = |u: &mut DroneUnit, spec: &DroneSpec, span: f64, drawn: &mut f64| {
        if span <= 0.0 || rate_w <= 0.0 {
            return;
        }
        let added = (rate_w * span / 3600.0).min(f64::from((spec.pack_wh - u.pack_wh).max(0.0)));
        u.pack_wh = (u.pack_wh + added as f32).min(spec.pack_wh);
        *drawn += added * paid_w / rate_w;
    };
    let mut t = 0.0_f64;
    let first_drone = |world: &hecs::World| world.query::<&Drone>().iter().next().map(|(e, _)| e);
    'trips: for _ in 0..MAX_AWAY_TRIPS {
        let in_flight = first_drone(world);
        let drone_e = match in_flight {
            Some(e) => e,
            None => {
                // In a session the standing order relaunches the trip on the
                // next frame; here, at once. A Launch waiting for the battery
                // goes first.
                let held = unit.as_ref().and_then(|u| u.held.clone());
                let Some((target, manifest)) = held.or_else(|| standing_order(data)) else { break };
                if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_mut()) {
                    if u.worn_out(spec) {
                        break;
                    }
                    let need = f64::from((spec.trip_wh - u.pack_wh).max(0.0));
                    if need > 1e-3 {
                        if rate_w <= 0.0 {
                            break;
                        }
                        let wait = need * 3600.0 / rate_w;
                        if t + wait > secs {
                            break;
                        }
                        charge(u, spec, wait, &mut away.drawn_wh);
                        t += wait;
                    }
                }
                if !launch(world, data, &target, &manifest) {
                    if let Some(u) = unit.as_mut() {
                        u.held = None;
                    }
                    break;
                }
                if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_mut()) {
                    pay_for_trip(spec, mode, u);
                    u.held = None;
                }
                match first_drone(world) {
                    Some(e) => e,
                    None => break,
                }
            }
        };
        // Step this drone phase by phase until it is home or the time is up.
        let delivered = loop {
            let Ok(d) = world.get::<&Drone>(drone_e).map(|d| (*d).clone()) else { break None };
            let left = f64::from((d.phase_duration(d.phase) - d.phase_time).max(0.0));
            if t + left > secs {
                if let Ok(mut live) = world.get::<&mut Drone>(drone_e) {
                    live.phase_time += (secs - t) as f32;
                }
                t = secs;
                break 'trips;
            }
            t += left;
            match d.phase {
                DronePhase::Outbound => {
                    let cargo = mine(world, &d.target, &d.manifest);
                    if let Ok(mut live) = world.get::<&mut Drone>(drone_e) {
                        live.phase = DronePhase::Mining;
                        live.phase_time = 0.0;
                        live.cargo = cargo;
                    }
                }
                DronePhase::Mining => {
                    if let Ok(mut live) = world.get::<&mut Drone>(drone_e) {
                        live.phase = DronePhase::Returning;
                        live.phase_time = 0.0;
                    }
                }
                DronePhase::Returning | DronePhase::Done => {
                    deliver_haul(world, data, item_registry, d.home, &d.cargo);
                    let _ = world.despawn(drone_e);
                    break Some(d);
                }
            }
        };
        let Some(d) = delivered else {
            remove_mined_out(world);
            break;
        };
        if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_mut()) {
            tell_if_worn(data, spec, u, item_registry);
        }
        if !d.cargo.iter().any(|(_, q)| *q > 0) {
            // An empty trip: the asteroid holds none of what the order asks
            // for, so every further trip would come home empty too. It ends
            // the order, as in a session, rather than leaving the session one
            // more empty trip to fly on its first frame back.
            came_back_empty(world, data, item_registry, &d.target, &d.manifest);
            remove_mined_out(world);
            break;
        }
        remove_mined_out(world);
        away.hauls.push((t, d.cargo));
    }
    // The rest of the time at the dock charges the battery (a trip still in
    // the air carries its own, already paid).
    if let (Some(spec), Some(u)) = (spec.as_ref(), unit.as_mut()) {
        if first_drone(world).is_none() {
            charge(u, spec, secs - t, &mut away.drawn_wh);
        }
        if let Some(Ok(mut slot)) = data.get::<Mutex<DroneUnit>>(UNIT_KEY).map(|m| m.lock()) {
            *slot = u.clone();
        }
    }
    away
}

/// What the drone did while the player was away (`advance_away`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Away {
    /// Each haul as (seconds into the time away, cargo).
    pub hauls: Vec<(f64, Vec<(String, u32)>)>,
    /// Watt-hours its dock took from the home (and through it the ship's bus) while away.
    pub drawn_wh: f64,
}

#[cfg(test)]
mod drone_tests {
    use super::*;

    fn make_store() -> DataStore {
        let mut data = DataStore::new();
        let items = ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .expect("items.csv");
        data.insert("item_registry", items);
        data.insert(
            "commission_drone",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
        );
        data
    }

    fn commission(data: &DataStore, target: &str, manifest: Vec<(&str, u32)>) {
        *data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("commission_drone")
            .unwrap()
            .lock()
            .unwrap() = Some((
            target.to_string(),
            manifest.into_iter().map(|(o, u)| (o.to_string(), u)).collect(),
        ));
    }

    fn asteroid(id: &str, ores: Vec<(&str, f32)>) -> AsteroidBody {
        AsteroidBody {
            id: id.to_string(),
            name: id.to_string(),
            classification: "M".into(),
            ores: ores.into_iter().map(|(o, q)| (o.to_string(), q)).collect(),
            position: [0.0, 0.0, 0.0],
        }
    }

    /// Full loop: commission a manifest for an asteroid → the drone flies out, fills its
    /// hold, returns → ore delivered; a fully-mined asteroid is deleted.
    #[test]
    fn commission_manifest_mines_and_delivers() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        let ast = world.spawn((asteroid("rock", vec![("iron_ore_0", 8.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 8)]);
        sys.tick(&mut world, 1.0, &data); // launch (Outbound)
        assert_eq!(world.query::<&Drone>().iter().count(), 1, "drone launched");

        for _ in 0..18 {
            sys.tick(&mut world, 1.0, &data);
        }

        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(iron >= 8, "manifest ore delivered (got {iron})");
        assert_eq!(world.query::<&Drone>().iter().count(), 0, "completed drone despawned");
        assert!(world.get::<&AsteroidBody>(ast).is_err(), "depleted asteroid removed");
    }

    /// The drone unloads into HOME STORAGE when the home has storage (BUG-150,
    /// 2026-10-04): it is the store the home's automated machines are fed
    /// from, and the only one they draw on now, so a haul left in the
    /// backpack would never reach the smelter. The haul goes on the channel
    /// the main loop files into the Barn, as an automated machine's batch
    /// does; the backpack is untouched. Mining is still trained for it.
    ///
    /// Seen red before the fix: "the haul is filed for home storage" (left:
    /// 0, right: 8): it landed in the backpack.
    #[test]
    fn the_drone_unloads_into_home_storage_when_the_home_has_it() {
        let mut data = make_store();
        data.insert("home_stock_outputs", std::sync::Mutex::new(Vec::<(String, u32)>::new()));
        data.insert("xp_grants", std::sync::Mutex::new(Vec::<crate::systems::skills::SkillXPEvent>::new()));
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 8.0)]),));
        commission(&data, "rock", vec![("iron_ore_0", 8)]);
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let filed: u32 = data
            .get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs")
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id == "iron_ore_0")
            .map(|(_, q)| *q)
            .sum();
        assert_eq!(filed, 8, "the haul is filed for home storage");
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"), 0, "not into the backpack");
        let xp: u32 = data
            .get::<std::sync::Mutex<Vec<crate::systems::skills::SkillXPEvent>>>("xp_grants")
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.skill_id == "mining")
            .map(|e| e.amount)
            .sum();
        assert_eq!(xp, 8, "a haul home still trains Mining, 1 XP per unit");
    }

    /// Standing order (economy automation Phase 1, v0.663): with an auto_mine_order
    /// set, the Deliver arm re-commissions the SAME trip after each haul -- the
    /// drone keeps cycling until the asteroid depletes, with zero further clicks.
    #[test]
    fn standing_order_relaunches_the_drone_after_delivery() {
        let mut data = make_store();
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
        );
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        // Big enough that the asteroid cannot deplete during the test window --
        // depletion legitimately ENDS the standing-order loop (target removed),
        // which is its own designed exit, not what this test measures.
        world.spawn((asteroid("rock", vec![("iron_ore_0", 100.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 8)]);
        // The standing order mirrors the commissioned trip ("Keep mining" checked).
        *data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
            .unwrap()
            .lock()
            .unwrap() = Some(("rock".to_string(), vec![("iron_ore_0".to_string(), 8)]));

        // First full trip (launch + fly + mine + return + deliver)...
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let after_first = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(after_first >= 8, "first haul delivered (got {after_first})");

        // ...and WITHOUT any new commission, a second trip runs on the standing order.
        for _ in 0..25 {
            sys.tick(&mut world, 1.0, &data);
        }
        let after_second = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(
            after_second > after_first,
            "standing order should have relaunched and delivered a second haul \
             ({after_first} -> {after_second})"
        );
    }

    /// Review fix (2026-07-01): the standing-order refire lives at TICK level, so
    /// auto-mining resumes even when the in-flight drone was lost without a
    /// delivery (a world reload despawns drones) -- a standing order + no drone +
    /// no commission must relaunch by itself.
    #[test]
    fn standing_order_resumes_after_drone_loss() {
        let mut data = make_store();
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::Some((
                "rock".to_string(),
                vec![("iron_ore_0".to_string(), 4u32)],
            ))),
        );
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 40.0)]),));

        // NO commission was ever written -- the refire alone must launch.
        for _ in 0..3 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert_eq!(
            world.query::<&Drone>().iter().count(),
            1,
            "the standing order alone relaunches a lost trip"
        );
        // And it delivers like any trip.
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert!(iron >= 4, "refired trip delivered (got {iron})");
    }

    /// Review fix (2026-07-01): when the standing order's target asteroid no
    /// longer exists (mined out and deleted), the order is CLEARED instead of
    /// refiring a dead commission every trip forever.
    #[test]
    fn depleted_target_ends_the_standing_order() {
        let mut data = make_store();
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::Some((
                "gone".to_string(),
                vec![("iron_ore_0".to_string(), 4u32)],
            ))),
        );
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        // No asteroid named "gone" exists.

        for _ in 0..3 {
            sys.tick(&mut world, 1.0, &data);
        }
        let order = data
            .get::<std::sync::Mutex<Option<(String, Vec<(String, u32)>)>>>("auto_mine_order")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(order.is_none(), "a standing order aimed at a gone target must end");
        assert_eq!(world.query::<&Drone>().iter().count(), 0, "nothing launched");
    }

    /// Loot is BOUNDED by the target asteroid's stock: requesting more than it holds
    /// returns only what was there (the operator's "loot from one asteroid is limited").
    #[test]
    fn loot_bounded_by_target_asteroid() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 3.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 10)]); // ask 10, only 3 there
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert_eq!(iron, 3, "only the asteroid's 3 units delivered (got {iron})");
    }

    /// One asteroid per run: a manifest mines ONLY the targeted asteroid; a second
    /// asteroid holding the same ore is left untouched.
    #[test]
    fn mines_only_the_target_asteroid() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("a", vec![("iron_ore_0", 5.0)]),));
        let other = world.spawn((asteroid("b", vec![("iron_ore_0", 50.0)]),));

        commission(&data, "a", vec![("iron_ore_0", 10)]); // target "a" (has 5)
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let iron = world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0");
        assert_eq!(iron, 5, "only target 'a' mined (got {iron})");
        let other_left = world.get::<&AsteroidBody>(other).unwrap().total_remaining();
        assert_eq!(other_left, 50.0, "the other asteroid is untouched");
    }

    /// A multi-ore manifest pulls EACH ore from the one target asteroid.
    #[test]
    fn multi_ore_manifest_from_one_asteroid() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0), ("copper_ore_0", 50.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 6), ("copper_ore_0", 4)]);
        for _ in 0..20 {
            sys.tick(&mut world, 1.0, &data);
        }
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("iron_ore_0"), 6, "6 iron delivered");
        assert_eq!(inv.count_item("copper_ore_0"), 4, "4 copper delivered");
    }

    /// One drone per player: a second commission while one is in flight is ignored.
    #[test]
    fn one_drone_per_player() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));

        commission(&data, "rock", vec![("iron_ore_0", 5)]);
        sys.tick(&mut world, 1.0, &data); // one drone now Outbound
        commission(&data, "rock", vec![("iron_ore_0", 5)]); // try a second
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(world.query::<&Drone>().iter().count(), 1, "still exactly one drone");
    }

    /// A commission for an asteroid that doesn't exist launches NO drone (no dud trip
    /// that burns the single drone slot for nothing).
    #[test]
    fn missing_target_launches_nothing() {
        let data = make_store();
        let mut sys = DroneSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));

        commission(&data, "ghost", vec![("iron_ore_0", 5)]); // no asteroid "ghost"
        sys.tick(&mut world, 1.0, &data);
        assert_eq!(world.query::<&Drone>().iter().count(), 0, "no drone for a missing target");
    }

    /// A hauled load must never vanish because the home stock is full (operator
    /// field report 2026-07-04: a 36/36 seed-packed backpack silently ate an
    /// entire iron haul -- add_item's overflow return was discarded -- so the
    /// smelter starved while the player watched the drone "deliver"). Delivery
    /// now grows the inventory (dev-stock's ensure_slots pattern) and lands the
    /// remainder.
    #[test]
    fn delivery_grows_a_full_backpack_instead_of_losing_the_haul() {
        let data = make_store();
        let mut world = hecs::World::new();
        // A 2-slot inventory PACKED full (unstackable junk), like the seed-full backpack.
        let mut inv = Inventory::new(2);
        inv.add_item("hammer_0", 1, 1);
        inv.add_item("bandage_0", 1, 1);
        assert_eq!(inv.slots.iter().filter(|s| s.is_none()).count(), 0, "no free slot");
        let player = world.spawn((inv, crate::ecs::components::Controllable));
        world.spawn((AsteroidBody {
            id: "rock".to_string(),
            name: "rock".to_string(),
            classification: "M".into(),
            ores: [("iron_ore_0".to_string(), 5.0)].into_iter().collect(),
            position: [0.0, 0.0, 0.0],
        },));

        commission(&data, "rock", vec![("iron_ore_0", 5)]);
        let mut sys = DroneSystem::new();
        for _ in 0..60 {
            sys.tick(&mut world, 1.0, &data);
        }

        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(
            inv.count_item("iron_ore_0"),
            5,
            "the full haul landed -- the backpack grew instead of eating the ore"
        );
    }

    fn set_order(data: &mut DataStore, order: Option<(&str, Vec<(&str, u32)>)>) {
        data.insert(
            "auto_mine_order",
            std::sync::Mutex::new(order.map(|(t, m)| {
                (t.to_string(), m.into_iter().map(|(o, u)| (o.to_string(), u)).collect::<Vec<_>>())
            })),
        );
    }

    /// While the player is away the standing order keeps the drone flying,
    /// out of the asteroid's real ore and no more: 25 iron comes home as
    /// 10, 10 and 5, the mined-out asteroid is deleted and the order ends,
    /// all as a session would. Each haul carries the moment it landed. Seen
    /// red with `advance_away` returning at once (no hauls, no ore).
    #[test]
    fn a_standing_order_keeps_mining_while_away_until_the_asteroid_is_empty() {
        let mut data = make_store();
        set_order(&mut data, Some(("rock", vec![("iron_ore_0", 10)])));
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 25.0)]),));

        let hauls = advance_away(&mut world, &data, 8.0 * 3600.0, 0.0).hauls;

        let got: Vec<u32> = hauls.iter().map(|(_, c)| c.iter().map(|(_, q)| q).sum()).collect();
        assert_eq!(got, vec![10, 10, 5]);
        // A trip at the origin is 2 s out, 5 s mining, 2 s back.
        let times: Vec<f64> = hauls.iter().map(|(t, _)| *t).collect();
        assert_eq!(times, vec![9.0, 18.0, 27.0]);
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"), 25);
        assert_eq!(world.query::<&AsteroidBody>().iter().count(), 0, "mined out and deleted");
        assert_eq!(standing_order(&data), None, "the order ended with its asteroid");
        assert_eq!(world.query::<&Drone>().iter().count(), 0);
    }

    /// Without a standing order only the trip in flight comes home; a short
    /// absence leaves the next one in the air where the player finds it.
    /// Seen red with `advance_away` returning at once (no haul).
    #[test]
    fn the_trip_in_flight_finishes_and_the_next_is_left_in_the_air() {
        let mut data = make_store();
        set_order(&mut data, None);
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));
        commission(&data, "rock", vec![("iron_ore_0", 4)]);
        DroneSystem::new().tick(&mut world, 0.0, &data); // launched, 0 s into the trip

        let hauls = advance_away(&mut world, &data, 3600.0, 0.0).hauls;
        assert_eq!(hauls.len(), 1, "one trip, no order to send another");
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"), 4);

        // With the order, 12 s away: one trip home (9 s), the next 3 s out.
        set_order(&mut data, Some(("rock", vec![("iron_ore_0", 4)])));
        let hauls = advance_away(&mut world, &data, 12.0, 0.0).hauls;
        assert_eq!(hauls.len(), 1);
        let (_, d) = world.query::<&Drone>().iter().next().map(|(e, d)| (e, d.clone())).expect("in the air");
        assert_eq!((d.phase, d.phase_time), (DronePhase::Mining, 1.0), "2 s out, then 1 s of mining");
        assert_eq!(d.cargo, vec![("iron_ore_0".to_string(), 4)], "its hold already filled");
    }

    /// KEEP MINING ENDS WHEN A TRIP COMES BACK EMPTY, and says so (first-hour
    /// audit 2026-10-04, Friction 5). An asteroid is deleted only when EVERY
    /// ore in it is gone, so once the ore an order asks for runs out the rock
    /// stays, and the standing order used to relaunch an empty trip forever:
    /// the drone was never home, so the asteroid cards (which ignore clicks
    /// while a drone is out) never opened and the Keep mining box could not be
    /// reached from the Mining page. Here the rock holds 4 iron and 50 nickel:
    /// the first trip brings the 4 iron, the second comes back empty, the
    /// order ends with a line naming the rock and the ore, and nothing flies
    /// again while the nickel stays where it is.
    ///
    /// Seen red 2026-10-04 on the code before the fix: "the order ends when a
    /// trip comes back empty / left: Some((\"rock\", [(\"iron_ore_0\", 10)])) / right: None".
    #[test]
    fn an_empty_trip_ends_keep_mining_and_says_so() {
        let mut data = make_store();
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        set_order(&mut data, Some(("rock", vec![("iron_ore_0", 10)])));
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((AsteroidBody {
            id: "rock".to_string(),
            name: "Asteroid R-1".to_string(),
            classification: "M".into(),
            ores: vec![("iron_ore_0".to_string(), 4.0), ("nickel_ore_0".to_string(), 50.0)],
            position: [0.0, 0.0, 0.0],
        },));
        let mut sys = DroneSystem::new();
        // A trip at the origin is 2 s out, 5 s mining and 2 s back, plus a tick to
        // launch: 60 s is the first trip, the empty second one, and time to spare.
        for _ in 0..60 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0"), 4, "the first trip's iron");
        let order = standing_order(&data);
        assert_eq!(order, None, "the order ends when a trip comes back empty / left: {order:?} / right: None");
        let notices = data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone();
        assert!(
            notices.iter().any(|n| n.contains("came back empty")
                && n.contains("Asteroid R-1")
                && n.contains("Iron Ore")
                && n.contains("Keep mining is off")),
            "the player is told why the drone stopped: {notices:?}"
        );
        for _ in 0..60 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert_eq!(world.query::<&Drone>().iter().count(), 0, "no more empty trips");
        assert_eq!(world.query::<&AsteroidBody>().iter().count(), 1, "the rock keeps its nickel");
    }

    /// The same while the player is away: the empty trip that ends the
    /// catch-up ends the order too, and says so, so the session does not fly
    /// one more empty trip on its first frame back. Seen red 2026-10-04 on the
    /// code before the fix: "the empty trip away ended the order".
    #[test]
    fn an_empty_trip_while_away_ends_keep_mining() {
        let mut data = make_store();
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        set_order(&mut data, Some(("rock", vec![("gold_ore_0", 4)])));
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));
        assert!(advance_away(&mut world, &data, 3600.0, 0.0).hauls.is_empty());
        assert_eq!(standing_order(&data), None, "the empty trip away ended the order");
        let notices = data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone();
        assert!(notices.iter().any(|n| n.contains("came back empty") && n.contains("Gold Ore")), "{notices:?}");
    }

    /// A drone asking for an ore its asteroid does not hold comes home empty,
    /// and so would every trip after: the catch-up stops there rather than
    /// flying empty trips for the whole absence.
    #[test]
    fn an_empty_trip_ends_the_catch_up() {
        let mut data = make_store();
        set_order(&mut data, Some(("rock", vec![("gold_ore_0", 4)])));
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), Controllable));
        world.spawn((asteroid("rock", vec![("iron_ore_0", 50.0)]),));
        assert!(advance_away(&mut world, &data, 30.0 * 86_400.0, 0.0).hauls.is_empty());
    }
}

/// The running costs (first-hour audit S5, 2026-10-12).
#[cfg(test)]
#[path = "mining_costs_tests.rs"]
mod costs_tests;

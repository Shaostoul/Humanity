//! Core ECS components shared across all game systems.
//!
//! Everything in the game is an entity with a combination of these components.
//! A human, a cow, an alien, and a mech are all entities with different component sets.

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

// ── Transform & Physics ──────────────────────────────────────

/// 3D transform: position, rotation, scale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

/// Linear and angular velocity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

/// Links an entity to a Rapier3d rigid body for physics simulation.
/// The handle indexes into PhysicsWorld's RigidBodySet.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsBody {
    pub handle: rapier3d::dynamics::RigidBodyHandle,
}

// ── Rendering ────────────────────────────────────────────────

/// Links an entity to a mesh and material for rendering.
/// mesh_id and material_id are string keys into the asset registries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Renderable {
    pub mesh_id: String,
    pub material_id: String,
}

// ── Identity & Stats ─────────────────────────────────────────

/// Human-readable name for any entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name(pub String);

/// Health pool with current and max values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
        }
    }
}

// ── Survival vitals & status effects ─────────────────────────

/// Survival baseline: satiation (fullness) and hydration. Both decay over time
/// and are replenished by eating/drinking. When either hits zero, Health drains
/// (starvation / dehydration). Low levels apply the `hungry` / `thirsty` conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vitals {
    /// Fullness, 0..=satiation_max. 0 = starving.
    pub satiation: f32,
    /// Hydration, 0..=hydration_max. 0 = dehydrated.
    pub hydration: f32,
    /// Energy / rest, 0..=energy_max. Drains while awake; restored by resting.
    /// Low energy applies the `fatigued` speed debuff.
    #[serde(default = "default_vital_full")]
    pub energy: f32,
    /// Blood oxygen, 0..=oxygen_max. Drains when exposed to vacuum / no breathable
    /// air; low = hypoxia, 0 = suffocation.
    #[serde(default = "default_vital_full")]
    pub oxygen: f32,
    /// Core body temperature (Celsius, ~37 healthy). Drifts toward ambient when
    /// exposed; far from baseline → hypothermia / heat exhaustion.
    #[serde(default = "default_body_temp")]
    pub body_temp_c: f32,
    /// Accumulated organic waste, 0..=waste_max. RISES over time + when eating;
    /// high waste → the `unsanitary` debuff. Compost it into fertilizer to clear it.
    #[serde(default)]
    pub waste: f32,
    pub satiation_max: f32,
    pub hydration_max: f32,
    #[serde(default = "default_vital_full")]
    pub energy_max: f32,
    #[serde(default = "default_vital_full")]
    pub oxygen_max: f32,
    #[serde(default = "default_vital_full")]
    pub waste_max: f32,
}

/// Default for a vital that starts full (serde fallback for older saves).
fn default_vital_full() -> f32 {
    100.0
}

/// Healthy baseline core body temperature (Celsius).
fn default_body_temp() -> f32 {
    37.0
}

impl Default for Vitals {
    fn default() -> Self {
        // Start comfortably fed (not full) so the player has headroom but will need
        // to eat/drink/rest within a session; oxygen full + body temperature healthy.
        Self {
            satiation: 80.0,
            hydration: 80.0,
            energy: 100.0,
            oxygen: 100.0,
            body_temp_c: 37.0,
            waste: 0.0,
            satiation_max: 100.0,
            hydration_max: 100.0,
            energy_max: 100.0,
            oxygen_max: 100.0,
            waste_max: 100.0,
        }
    }
}

/// The player's current environment, recomputed each frame from their position vs the
/// sealed homestead volume. Lives in the DataStore under "environment_context" and
/// drives oxygen + body temperature in FoodSystem. Its `Default` is safe
/// (sealed/oxygenated/comfortable), so absent data never harms the player.
#[derive(Debug, Clone)]
pub struct EnvironmentContext {
    /// True inside a sealed/pressurized space (the homestead); outside = vacuum.
    pub sealed: bool,
    /// True if breathable oxygen is available at the player's location.
    pub oxygenated: bool,
    /// Ambient temperature (Celsius) the body drifts toward when exposed.
    pub ambient_temp_c: f32,
}

impl Default for EnvironmentContext {
    fn default() -> Self {
        Self {
            sealed: true,
            oxygenated: true,
            ambient_temp_c: 21.0,
        }
    }
}

/// One active status effect on an entity, with its remaining duration in seconds.
/// Timed buffs/debuffs count down to 0 and expire; condition-type effects (e.g.
/// `hungry`) are refreshed each tick by the system that owns their trigger and
/// fade a few seconds after the trigger clears. Always finite (JSON-save-safe).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEffect {
    /// Effect id from `data/status_effects.csv`.
    pub id: String,
    /// Seconds remaining before this effect expires.
    pub remaining: f32,
}

/// Active buffs / debuffs / conditions on an entity (food, environment, medical…).
/// Systems that own a stat (movement, vision, regen) read this plus the
/// `StatusEffectRegistry` to apply each effect's `stat:value:op` modifier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusEffects {
    pub active: Vec<ActiveEffect>,
}

impl StatusEffects {
    /// True if an effect with this id is currently active.
    pub fn has(&self, id: &str) -> bool {
        self.active.iter().any(|e| e.id == id)
    }

    /// Add the effect, or refresh its timer to at least `duration` if already present.
    pub fn apply(&mut self, id: &str, duration: f32) {
        if let Some(e) = self.active.iter_mut().find(|e| e.id == id) {
            e.remaining = e.remaining.max(duration);
        } else {
            self.active.push(ActiveEffect {
                id: id.to_string(),
                remaining: duration,
            });
        }
    }

    /// Remove an effect by id (e.g. when its triggering condition clears).
    pub fn remove(&mut self, id: &str) {
        self.active.retain(|e| e.id != id);
    }

    /// Count down every effect and drop the ones that have expired.
    pub fn tick(&mut self, dt: f32) {
        for e in &mut self.active {
            e.remaining -= dt;
        }
        self.active.retain(|e| e.remaining > 0.0);
    }
}

/// Faction/allegiance for PvP/PvE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Faction {
    pub id: String,
}

// ── Player & Control ─────────────────────────────────────────

/// Marks the entity the player is currently controlling.
/// Only one entity should have this at a time.
/// Moving this component to a different entity = possessing that entity
/// (mech, alien, vehicle, creature for PvP modes, etc.).
#[derive(Debug, Clone, Default)]
pub struct Controllable;

// ── AI & Behavior ────────────────────────────────────────────

/// AI behavior for NPCs and creatures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIBehavior {
    /// Behavior type from data file (e.g., "passive", "aggressive", "herd", "predator").
    pub behavior_type: String,
    /// Current state (e.g., "idle", "wandering", "fleeing", "attacking", "following").
    pub state: String,
    /// Target entity (if chasing/following).
    pub target: Option<u64>,
}

// ── Interaction ──────────────────────────────────────────────

/// Makes an entity interactable by the player (click/hover).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interactable {
    /// What kind of interaction (e.g., "harvest", "mine", "open", "use", "build", "talk").
    pub interaction_type: String,
    /// Max distance from which the player can interact (meters).
    pub range: f32,
}

/// Resource that can be harvested from an entity (milk, wool, fruit, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Harvestable {
    /// Resource ID from items.csv.
    pub resource: String,
    /// Amount per harvest.
    pub amount: f32,
    /// Seconds until resource regenerates.
    pub regrow_time: f32,
    /// Seconds since last harvest (for regrowth tracking).
    pub time_since_harvest: f32,
}

// ── Farming ──────────────────────────────────────────────────

/// Default growth stages used when a plant definition doesn't specify its own.
pub const DEFAULT_GROWTH_STAGES: &[&str] = &[
    "seed", "sprout", "vegetative", "flowering", "fruiting", "harvest",
];

/// Reserved stage name for dead crops (set when health reaches zero).
pub const STAGE_DEAD: &str = "dead";

/// A planted crop instance tied to a crop definition from data files.
/// Growth stage is a String so each plant species can define its own
/// stage names in plants.csv (e.g. "spore|mycelium|fruiting_body|spore_release").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropInstance {
    /// Crop definition ID from plants.csv.
    pub crop_def_id: String,
    /// Current growth stage name (data-driven, from PlantDef.growth_stages).
    pub growth_stage: String,
    /// When this crop was planted (game time seconds).
    pub planted_at: f64,
    /// Current water level (0.0 = dry, 1.0 = saturated).
    pub water_level: f32,
    /// Current health (affected by disease, pests, weather).
    pub health: f32,
    /// The aeroponic tower this crop was planted in (its config id), if any. Lets
    /// the inventory group crops collapsibly by tower (v0.389). Seed-planted crops
    /// are None; #[serde(default)] keeps pre-v0.389 saves loading.
    #[serde(default)]
    pub tower_id: Option<String>,
    /// Which SLOT of the tower this crop occupies (0-based), if planted in a tower.
    /// Lets "Plant this tower" FILL fixed slots (idempotent) instead of stacking new
    /// crops on every replant (v0.410). Seed-planted crops are None.
    #[serde(default)]
    pub tower_slot: Option<u32>,
}

// ── Vehicles & Mechs ─────────────────────────────────────────

/// A deployed vehicle standing in the world (economy Phase 2 Stage 1, v0.677).
/// Spawned by VehicleSystem's deploy arm when a vehicle KIT item is deployed
/// from the inventory. `item_id` is the ASSEMBLED vehicle's items.csv id (e.g.
/// "truck_pickup_0"); the kit registry (data/vehicles/kits.ron) maps it to the
/// primitive body proportions the renderer draws. Persisted across restarts
/// via WorldSave.deployed_vehicles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vehicle {
    pub item_id: String,
}

/// A vehicle in TRANSIT (economy Phase 2 Stage 3, v0.680): drives itself in a
/// straight line toward `dest` at `speed_mps` on game time, yawing to face its
/// travel direction; VehicleSystem removes this on arrival (within
/// `arrive_radius` so a summoned vehicle pulls up NEXT to you, not onto you).
/// Deliberately NOT persisted: a save mid-transit restores the vehicle parked
/// where it was -- transit is ephemeral motion, not owned state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleRoute {
    pub dest: Vec3,
    pub speed_mps: f32,
    pub arrive_radius: f32,
}

/// A seat in a vehicle/mech that a player can occupy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleSeat {
    /// Public key of the occupant (empty = unoccupied).
    pub occupant_key: Option<String>,
    /// Seat type: "pilot", "gunner", "passenger".
    pub seat_type: String,
}

/// Weapon/tool mount point on a mech or ship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardpointSlot {
    /// Slot name (e.g., "left_arm", "right_arm", "torso", "turret_1").
    pub name: String,
    /// Item ID of the mounted weapon/tool (None = empty).
    pub mounted_item: Option<String>,
    /// Offset from entity center (for rendering the weapon).
    pub offset: Vec3,
}

/// Collection of hardpoints on a mech, ship, or turret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hardpoints {
    pub slots: Vec<HardpointSlot>,
}

// ── Voxel & Terrain ──────────────────────────────────────────

/// Marks an entity as a voxel body (asteroid or modified terrain chunk).
#[derive(Debug, Clone)]
pub struct VoxelBody {
    /// Unique ID for looking up voxel data in the voxel store.
    pub voxel_id: String,
    /// Whether this voxel body has been modified by the player.
    pub modified: bool,
}

// ── Aging & Life Stage ──────────────────────────────────────

/// Tracks an entity's age in game-years and current life stage.
/// Driven by `AgingSystem::tick`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Age {
    /// Current age in game-years (1 game day = 20 real minutes; 1 game year = 365 game days).
    pub years: f32,
    /// Current life stage (e.g. "child", "teen", "young_adult", "adult", "senior", "elder").
    pub life_stage: String,
}

impl Default for Age {
    fn default() -> Self {
        Self { years: 25.0, life_stage: "young_adult".into() }
    }
}

// ── Waste Management ────────────────────────────────────────

/// Marks an entity as a waste-producing source. Rate is kg per game-day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasteSource {
    /// kg of waste produced per game-day.
    pub rate_per_day: f32,
    /// Category id from data/waste_management.ron (e.g. "organic", "metal", "plastic").
    pub category: String,
}

/// Accumulates waste of various categories on an entity (typically a room or container).
/// `WasteSystem::tick` adds emissions from nearby `WasteSource` entities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WasteAccumulator {
    /// Per-category accumulated kg.
    pub by_category: std::collections::HashMap<String, f32>,
    /// Total capacity in kg before pollution effects apply.
    pub capacity: f32,
}

// ── Manufacturing ───────────────────────────────────────────

/// A production facility producing one recipe at a time.
/// `ManufacturingSystem::tick` advances `progress` toward 1.0; when it
/// reaches 1.0, `output_count` increments and progress resets to 0.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionFacility {
    /// Recipe id from `data/recipes.csv`.
    pub recipe_id: String,
    /// Progress toward completing one unit (0.0 to 1.0).
    pub progress: f32,
    /// Recipe duration in game-days. 1.0 = one unit per day.
    pub days_per_unit: f32,
    /// Total units produced since the facility started.
    pub output_count: u32,
    /// Whether the facility is currently running (off if missing inputs/power).
    pub running: bool,
}

impl Default for ProductionFacility {
    fn default() -> Self {
        Self {
            recipe_id: String::new(),
            progress: 0.0,
            days_per_unit: 1.0,
            output_count: 0,
            running: true,
        }
    }
}

// ── Plumbing & Water ────────────────────────────────────────
// The live water components (WaterTank, WaterProducer, WaterConsumer, PlumbingCircuit) live further
// down next to PowerCircuit (v0.608). The old distance-based WaterTank/WaterFixture scaffold was
// replaced by the island-coupled PlumbingSystem.

// ── HVAC & Room Environment ─────────────────────────────────

/// Per-room atmospheric state. `HvacSystem` mutates this each tick based on
/// nearby HVAC units, room occupancy (CO2 emission), and outside conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEnvironment {
    pub temp_c: f32,
    pub humidity: f32,    // 0.0 to 1.0
    pub co2_ppm: f32,     // healthy < 1000, drowsy > 1500, dangerous > 5000
}

impl Default for RoomEnvironment {
    fn default() -> Self {
        Self { temp_c: 20.0, humidity: 0.45, co2_ppm: 420.0 }
    }
}

/// An HVAC unit. Heats, cools, or vents; affects nearby `RoomEnvironment`s
/// each tick. `mode` is "heat" / "cool" / "vent" / "off".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HvacUnit {
    pub mode: String,
    /// Target temperature in Celsius for heat/cool modes.
    pub target_temp: f32,
    /// Output power (kW). Higher = faster temperature change + faster ventilation.
    pub power_kw: f32,
}

// ── Fire ────────────────────────────────────────────────────

/// An active fire on an entity. `FireSystem` consumes fuel each tick;
/// when `fuel_remaining` reaches zero, the fire is removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fire {
    /// 0.0 to 1.0. Higher = hotter, more damage, longer spread reach.
    pub intensity: f32,
    /// Game-seconds of fuel left before the fire dies naturally.
    pub fuel_remaining: f32,
}

/// Marks an entity as flammable. `FireSystem` may ignite this entity if a
/// nearby `Fire` rolls a spread check within `ignition_dist`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flammable {
    /// Distance from a Fire at which spread becomes possible (meters).
    pub ignition_dist: f32,
    /// Game-seconds of fuel this entity provides if ignited.
    pub fuel_seconds: f32,
}

/// A fire suppressor (sprinkler, foam nozzle, fire extinguisher mount).
/// `FireSystem` reduces intensity of nearby fires each tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireSuppressor {
    /// Suppression range (meters).
    pub range: f32,
    /// Intensity reduction per game-second when active.
    pub strength: f32,
}

// ── Combat: Armor & Death ───────────────────────────────────

/// Per-damage-type damage reduction (0.0 = no resistance, 1.0 = immune).
/// `CombatSystem::tick` applies these before subtracting from `Health`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Armor {
    /// Resistance per damage type id ("kinetic", "thermal", "energy", "chemical", "radiation").
    pub resistance: std::collections::HashMap<String, f32>,
}

/// Marks an entity as dead. Death-handling systems remove rendering, drop loot,
/// and may schedule respawn. `CombatSystem` adds this when Health hits zero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dead {
    /// Game-seconds since death (used by respawn timers + cleanup).
    pub since: f32,
    /// Whether loot has been dropped (prevents double-drop).
    pub looted: bool,
}

impl Default for Dead {
    fn default() -> Self {
        Self { since: 0.0, looted: false }
    }
}

/// Loot table — items dropped on death. Each entry is (item_id, drop_chance, count).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LootTable {
    pub entries: Vec<(String, f32, u32)>,
}

// ── Medical ─────────────────────────────────────────────────

/// An active medical condition on an entity (injury, illness, infection).
/// `MedicalSystem::tick` decrements `ticks_remaining` and applies effects
/// (e.g. health regen reduction, status modifier). Cleared when remaining hits 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicalConditions {
    pub active: Vec<ActiveCondition>,
}

impl Default for MedicalConditions {
    fn default() -> Self { Self { active: Vec::new() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCondition {
    /// Condition id from `data/medical.ron` (e.g. "burn_minor", "infection").
    pub id: String,
    /// Severity 0.0 to 1.0 — multiplies effect strength.
    pub severity: f32,
    /// Game-seconds until this condition resolves naturally.
    pub seconds_remaining: f32,
    /// Per-second health change while active (negative = damage, positive = regen).
    pub health_per_sec: f32,
}

// ── Electrical ──────────────────────────────────────────────

/// A power source. Outputs `output_watts` while `active` and consumes fuel
/// at `fuel_per_second` (kg/sec, 0 = solar/wind/grid). `ElectricalSystem::tick`
/// shuts down generators when their fuel inventory is empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerGenerator {
    pub output_watts: f32,
    pub fuel_per_second: f32,
    pub active: bool,
}

impl Default for PowerGenerator {
    fn default() -> Self {
        Self { output_watts: 100.0, fuel_per_second: 0.0, active: true }
    }
}

/// A power consumer. Draws `draw_watts` while `enabled`. Higher `priority`
/// stays on first when supply < demand (1 = critical, 5 = optional).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConsumer {
    pub draw_watts: f32,
    pub priority: u8,
    pub enabled: bool,
}

impl Default for PowerConsumer {
    fn default() -> Self {
        Self { draw_watts: 50.0, priority: 3, enabled: true }
    }
}

/// Marks a `PowerGenerator` as solar: its `output_watts` is set each frame to
/// `peak_watts * sun_factor(hour)` by `SolarSystem`, so it climbs at noon and falls
/// to zero at night. `peak_watts` is the nameplate output at full sun.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolarPanel {
    pub peak_watts: f32,
}

/// A battery bank that buffers the home's power: it CHARGES from a surplus and DISCHARGES into a
/// deficit, so the day/night solar swing becomes consequential (the "2.8 days autonomy" stops being
/// a static string and starts draining). `charge_wh` is the live state of charge in watt-hours;
/// `ElectricalSystem` integrates the grid balance into it each tick (clamped by capacity + the
/// charge/discharge power limits). (v0.473, live-home-sim increment 1b)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Battery {
    pub charge_wh: f32,
    pub capacity_wh: f32,
    pub max_charge_w: f32,
    pub max_discharge_w: f32,
}

/// Marks an ECS entity as belonging to the placed home layout (spawned from
/// `data/machines/home.ron`). Used to despawn the old set before re-spawning so
/// re-entering the world never duplicates the live machine entities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HomeMachine;

/// The placed machine's instance id (`data/machines/home.ron` `id`) carried on
/// its ECS entity, so per-frame UI (the walk-up cards) can find THIS machine's
/// live state — WaterTank fill, Battery charge — instead of the static RON
/// placeholder stats. (v0.724, machine info-window overhaul)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInstanceId(pub String);

/// A home machine that CONTINUOUSLY runs one recipe against the HOME inventory
/// (economy automation Phase 1, v0.663): whenever the recipe's inputs are
/// present in the player/home stock and this machine has no craft already in
/// flight, `CraftingSystem` consumes the inputs and queues a timed craft whose
/// outputs land back in the home stock. Data-driven: spawned when a machine def
/// in `data/machines/*.ron` carries `auto_recipe` (e.g. the smelter auto-runs
/// `smelt_iron`, the workbench `craft_hammer`). Deliberately NOT gated on the
/// player's skill level: owning the machine is the unlock -- the machine does
/// the work, not the player's hands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoRefine {
    /// Recipe id from `data/recipes.csv` this machine keeps running.
    pub recipe_id: String,
}

/// The electrical ISLAND (connected power component) a power entity belongs to (v0.607). Attached to
/// every spawned generator/consumer/battery from `MachineHome::electrical_islands`, so the
/// `ElectricalSystem` balances + sheds PER ISLAND instead of summing the whole world -- power flows
/// only along real cabling, no magic transmission across unconnected circuits. Entities WITHOUT this
/// component (legacy / test spawns) fall into one shared bucket, preserving the old global behaviour.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PowerCircuit {
    pub island: u32,
}

/// A bulk water store (a cistern/tank) (v0.608). `liters` is the live level; `PlumbingSystem` fills it
/// from powered producers and drains it for consumers, so the day's water budget is a draining number.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WaterTank {
    pub liters: f32,
    pub capacity_l: f32,
}

/// Produces `lpm` litres/min of water into its plumbing island (a purifier/well) (v0.608). If
/// `needs_power`, it only produces while the SAME entity's `PowerConsumer` is enabled -- the first real
/// power -> water consequence chain (cut the power, the pump stops, the cistern stops filling).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WaterProducer {
    pub lpm: f32,
    pub needs_power: bool,
}

/// Draws `lpm` litres/min of water from its plumbing island (household / irrigation / a tower) (v0.608).
/// If `needs_power`, it only draws while the same entity is powered.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WaterConsumer {
    pub lpm: f32,
    pub needs_power: bool,
}

/// The plumbing ISLAND (connected water-pipe component) a water entity belongs to (v0.608). Parallels
/// `PowerCircuit`: the `PlumbingSystem` balances production/storage/demand PER ISLAND so water cannot
/// teleport between unconnected pipe runs. Entities without it share the `None` bucket.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlumbingCircuit {
    pub island: u32,
}

/// A radio-frequency emitter (v0.620): a wireless device (a WiFi router) bathes the home in RF while
/// powered. `strength` is a 0..1 level; `needs_power` gates it on the same entity's PowerConsumer. The
/// FarmingSystem reads the summed powered RF and harms sensitive crops -- so a player who wants a clean
/// grow runs WIRED (Cat6/fibre, zero RF) instead. The bridge to the emissions-as-detection signature
/// layer (docs/design/telecom.md).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RfEmitter {
    pub strength: f32,
    pub needs_power: bool,
}

/// The player's visual appearance (v0.440, the avatar foundation). Drives the blockman
/// avatar mesh and, later, the character-select showroom + the wetroom mirror editor + the
/// bedroom wardrobe. `body_type` / `hair_style` are ids into future data/appearance/*.ron;
/// colors are linear RGB. Designed to persist in WorldSave (the public half can later ride
/// the signed-profile gossip so visitors render each other).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appearance {
    pub body_type: String,
    /// Avatar height multiplier (1.0 = ~1.7 m).
    pub height_scale: f32,
    pub skin_tone: [f32; 3],
    pub hair_style: String,
    pub hair_color: [f32; 3],
    pub eye_color: [f32; 3],
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            body_type: "default".to_string(),
            height_scale: 1.0,
            skin_tone: [0.80, 0.62, 0.50],
            hair_style: "short".to_string(),
            hair_color: [0.25, 0.18, 0.10],
            eye_color: [0.30, 0.45, 0.55],
        }
    }
}

/// The player's equipped cosmetic outfit (v0.440): slot id -> cosmetic item id. Slots are
/// the ids in data/inventory/equipment_slots.json (head/chest/legs/feet/hands/back) -- reuse,
/// never fork. Persisted in WorldSave; the public half can later ride the signed-profile
/// gossip so visitors see what you wear. Empty = the default blockman with no cosmetics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Outfit {
    #[serde(default)]
    pub equipped: std::collections::HashMap<String, String>,
}

// ── Geology / Mining ────────────────────────────────────────

/// An ore deposit attached to a terrain entity. `GeologySystem` doesn't
/// deplete this on its own — `MiningInteraction` handlers extract from it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OreDeposit {
    /// Ore type id from `data/geology.ron::ore_veins[].id`.
    pub ore_id: String,
    /// Remaining yield in kg.
    pub yield_remaining: f32,
    /// Original yield (so depletion can compute `0.0..1.0` progress).
    pub yield_initial: f32,
}

/// A mineable asteroid as an ECS entity — a FINITE multi-ore body. A mining drone
/// depletes its `ores`; when nothing remains (fully consumed) the entity is deleted
/// by `DroneSystem`. (The 3D voxel `terrain::asteroid::Asteroid` octree is a separate
/// visualization layer for #5b; this is the logical resource the mining loop runs on.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsteroidBody {
    /// Stable id used to TARGET this asteroid for mining (e.g. "m12"). A drone mines
    /// exactly one asteroid per trip, identified by this.
    #[serde(default)]
    pub id: String,
    /// Display name (e.g. "Asteroid X-12").
    pub name: String,
    /// Spectral class: C (carbonaceous), S (silicaceous), M (metallic).
    pub classification: String,
    /// Remaining ore by item id, e.g. ("iron_ore_0", 80.0). Units = inventory items.
    pub ores: Vec<(String, f32)>,
    /// World position (km, arbitrary frame) for travel distance + the map. (0,0,0) if unset.
    #[serde(default)]
    pub position: [f32; 3],
}

impl AsteroidBody {
    /// Total ore remaining across all types.
    pub fn total_remaining(&self) -> f32 {
        self.ores.iter().map(|(_, q)| *q).sum()
    }

    /// Mine up to `amount` of one ore; returns the whole units actually extracted
    /// and decrements the remaining stock.
    pub fn take(&mut self, ore_id: &str, amount: f32) -> u32 {
        if let Some(slot) = self.ores.iter_mut().find(|(id, _)| id == ore_id) {
            let taken = slot.1.min(amount).max(0.0);
            slot.1 -= taken;
            taken.floor() as u32
        } else {
            0
        }
    }

    /// True if this asteroid still holds at least one unit of a given ore.
    pub fn has_ore(&self, ore_id: &str) -> bool {
        self.ores.iter().any(|(id, q)| id == ore_id && *q >= 1.0)
    }
}

/// A mining drone's mission phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DronePhase {
    /// Flying out to the target asteroid.
    Outbound,
    /// At the asteroid, extracting ore.
    Mining,
    /// Flying home with cargo.
    Returning,
    /// Arrived home; cargo delivered — ready to despawn.
    Done,
}

/// An autonomous mining drone — ONE per player. Commissioned with a MANIFEST (which
/// ores + how many units of its fixed capacity to allocate to each), it flies out,
/// fills its hold per the manifest from available asteroids, returns, and drops the
/// cargo into the home entity's inventory. `DroneSystem` drives the phase machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drone {
    /// Entity bits of the home to deliver cargo to (the player's inventory).
    pub home: u64,
    /// The asteroid id this drone is mining. ONE asteroid per trip: the drone pulls
    /// its manifest ores only from this asteroid, bounded by what it holds.
    #[serde(default)]
    pub target: String,
    /// The fetch order: `(ore_id, requested units)`. The units sum to ≤ the drone's
    /// capacity. The drone pulls each ore from its target asteroid.
    pub manifest: Vec<(String, u32)>,
    /// Current mission phase.
    pub phase: DronePhase,
    /// Seconds elapsed in the current phase.
    pub phase_time: f32,
    /// Ore collected so far: `(ore_id, units)`.
    pub cargo: Vec<(String, u32)>,
    /// Home + target world positions, so travel time scales with distance and the map
    /// can animate the drone along its journey.
    #[serde(default)]
    pub home_pos: [f32; 3],
    #[serde(default)]
    pub target_pos: [f32; 3],
}

impl Drone {
    /// Total units currently in the hold (across all ores).
    pub fn cargo_total(&self) -> u32 {
        self.cargo.iter().map(|(_, q)| q).sum()
    }

    /// Straight-line distance from home to the target asteroid.
    pub fn distance(&self) -> f32 {
        let d = [
            self.target_pos[0] - self.home_pos[0],
            self.target_pos[1] - self.home_pos[1],
            self.target_pos[2] - self.home_pos[2],
        ];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    /// Seconds the given phase lasts for THIS drone: travel scales with distance
    /// (a base plus a per-km factor), mining is flat.
    pub fn phase_duration(&self, phase: DronePhase) -> f32 {
        match phase {
            DronePhase::Outbound | DronePhase::Returning => 2.0 + self.distance() * 0.05,
            DronePhase::Mining => 5.0,
            DronePhase::Done => 0.0,
        }
    }

    /// Current world position, interpolated along the journey (for the map dot).
    pub fn current_pos(&self) -> [f32; 3] {
        let dur = self.phase_duration(self.phase).max(0.01);
        let t = (self.phase_time / dur).clamp(0.0, 1.0);
        let lerp = |a: [f32; 3], b: [f32; 3]| {
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
        };
        match self.phase {
            DronePhase::Outbound => lerp(self.home_pos, self.target_pos),
            DronePhase::Mining => self.target_pos,
            DronePhase::Returning => lerp(self.target_pos, self.home_pos),
            DronePhase::Done => self.home_pos,
        }
    }
}

/// A soil patch — slowly accumulates nutrients from organic matter.
/// `GeologySystem::tick` increments fertility based on adjacent decomposing waste.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilPatch {
    /// Nutrient density 0.0 to 1.0. Affects farming yield.
    pub fertility: f32,
    /// Soil type id from `data/geology.ron::soil_types[].id`.
    pub soil_type: String,
}

impl Default for SoilPatch {
    fn default() -> Self { Self { fertility: 0.5, soil_type: "loam".into() } }
}

// ── Oceanography / Marine Resources ─────────────────────────

/// A marine resource population (fish stock, kelp bed, oyster reef).
/// `OceanographySystem::tick` regenerates the population toward `carrying_capacity`
/// at `regen_rate_per_day` while not at cap. Harvest interactions deplete it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarineResource {
    /// Resource id from `data/oceanography.ron::marine_resources[].id`.
    pub resource_id: String,
    /// Current population in kg (or count, as the resource type defines).
    pub current: f32,
    /// Maximum sustainable population.
    pub carrying_capacity: f32,
    /// Per-day regeneration rate when below capacity (kg/day).
    pub regen_per_day: f32,
}

// ── Astronomy / Telescopes ──────────────────────────────────

/// A telescope or sensor array. Each tick, it accumulates `observation_seconds`
/// while pointed at a target. Useful for science / navigation gameplay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telescope {
    /// Currently aimed target id (e.g. "earth", "mars", "alpha_centauri") or empty.
    pub target_id: String,
    /// Game-seconds of accumulated observation time on the current target.
    pub observation_seconds: f32,
    /// Magnification / sensitivity (1.0 = naked-eye, 1000.0 = research-grade).
    pub power: f32,
}

// ── Governance ──────────────────────────────────────────────

/// An active vote/proposal in a settlement. `GovernanceSystem::tick` resolves
/// it when `deadline_seconds_remaining` reaches 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveVote {
    /// Proposal text or id from data/governance.ron.
    pub proposal: String,
    /// Game-seconds until the vote closes.
    pub deadline_seconds_remaining: f32,
    /// Yes votes accumulated so far.
    pub yes: u32,
    /// No votes.
    pub no: u32,
    /// Whether the vote has been resolved (set to true on close).
    #[serde(default)]
    pub resolved: bool,
}

// ── Creative Arts ───────────────────────────────────────────

/// An artistic work in progress (painting, song, sculpture, performance).
/// `CreativeArtsSystem::tick` advances `progress` toward 1.0 at
/// `progress_per_day` rate while `working` is true. On completion, `quality`
/// is computed from creator skill (game code sets this when starting work).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreativeWork {
    /// Output type id from data/creative_arts.ron::outputs[].id.
    pub work_type: String,
    /// Progress toward completion (0.0 to 1.0).
    pub progress: f32,
    /// Days to complete one unit of work at full speed.
    pub days_to_complete: f32,
    /// Quality 0.0 to 1.0 — set on creation by skill check.
    pub quality: f32,
    /// Whether the artist is actively working on it.
    pub working: bool,
    /// Whether the work has been finished.
    #[serde(default)]
    pub completed: bool,
}

// ── Docking & Airlocks ──────────────────────────────────────

/// An airlock chamber. `DockingSystem::tick` runs the cycle state machine:
/// open_outer → sealed → cycling → other_side_open → sealed → ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirlockChamber {
    /// Current state: "open_outer", "open_inner", "sealed_pressurized",
    /// "sealed_vacuum", "cycling_to_pressurized", "cycling_to_vacuum".
    pub state: String,
    /// 0.0 to 1.0 progress within the current cycling state.
    pub cycle_progress: f32,
    /// Game-seconds for one full cycle (vacuum to pressurized or vice versa).
    pub cycle_seconds: f32,
}

impl Default for AirlockChamber {
    fn default() -> Self {
        Self { state: "sealed_pressurized".into(), cycle_progress: 0.0, cycle_seconds: 8.0 }
    }
}

// ── Transportation / Cargo Vehicles ─────────────────────────

/// A cargo vehicle traveling along a route. `TransportationSystem` advances
/// `progress` from 0.0 (origin) to 1.0 (destination) at `speed_per_day`.
/// On arrival, the vehicle is marked `arrived` and game code unloads cargo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoVehicle {
    /// Route id from `data/transportation.ron::roads[].id` or similar.
    pub route_id: String,
    /// Progress along route (0.0 to 1.0).
    pub progress: f32,
    /// Speed in route-fractions per game-day (1.0 = full route per day).
    pub speed_per_day: f32,
    /// Payload — list of (item_id, count).
    pub payload: Vec<(String, u32)>,
    /// Whether this vehicle has reached its destination.
    #[serde(default)]
    pub arrived: bool,
}

// ── Offline / Autonomous Agents ─────────────────────────────

/// An autonomous task scheduled to run on an entity periodically.
/// `OfflineSystem` increments `seconds_since_last` each tick and fires the
/// task action when `interval_seconds` is reached. Used for AFK NPC chores
/// (patrol, gather, build) without needing a full BehaviorTree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousTask {
    /// Preset id from `data/offline_behaviors.ron::presets[].id`
    /// (e.g. "patrol", "gather_food", "build_shelter").
    pub preset_id: String,
    /// Game-seconds between task firings.
    pub interval_seconds: f32,
    /// Time accumulated since the last firing.
    pub seconds_since_last: f32,
    /// Total times the task has fired (lifetime stat for this agent).
    #[serde(default)]
    pub fire_count: u32,
}

// ── Genetics ────────────────────────────────────────────────

/// Diploid genome — one allele pair per trait.
/// `GeneticsSystem` is event-driven; `breed(parent_a, parent_b)` returns
/// a child Genome rather than mutating during tick.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Genome {
    /// trait_id → (allele_a, allele_b). Allele strings are domain-defined
    /// (e.g. "tall"/"short", "red"/"green", "fast"/"slow").
    pub alleles: std::collections::HashMap<String, (String, String)>,
}

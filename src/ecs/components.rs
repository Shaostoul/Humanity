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
}

// ── Vehicles & Mechs ─────────────────────────────────────────

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

/// A water storage tank. Capacity in liters; current is the live level.
/// `PlumbingSystem` drains tanks to satisfy nearby `WaterFixture` demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterTank {
    pub current: f32,
    pub capacity: f32,
}

impl Default for WaterTank {
    fn default() -> Self {
        Self { current: 0.0, capacity: 1000.0 }
    }
}

/// A water-consuming fixture (sink, shower, hydroponics tray, livestock trough).
/// `demand_per_day` is target liters/day; `supplied_today` accumulates as
/// water is delivered from a nearby `WaterTank`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterFixture {
    pub demand_per_day: f32,
    pub supplied_today: f32,
    /// Whether the fixture got water on the last tick (drives "no water" UI feedback).
    #[serde(default)]
    pub satisfied: bool,
}

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

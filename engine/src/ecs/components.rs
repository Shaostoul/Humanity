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

/// Growth stage for crops and plants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowthStage {
    Seed,
    Sprout,
    Vegetative,
    Flowering,
    Fruiting,
    Harvest,
    Dead,
}

/// A planted crop instance tied to a crop definition from data files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropInstance {
    /// Crop definition ID from plants.csv.
    pub crop_def_id: String,
    pub growth_stage: GrowthStage,
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

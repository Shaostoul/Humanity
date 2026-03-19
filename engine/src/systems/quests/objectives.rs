//! Objective types and tracking — what the player needs to accomplish.

use serde::{Deserialize, Serialize};

/// Types of quest objectives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectiveType {
    /// Collect N of item_id.
    Collect { item_id: String, count: u32 },
    /// Reach a location.
    ReachLocation { x: f32, y: f32, z: f32, radius: f32 },
    /// Defeat N of enemy_type.
    Defeat { enemy_type: String, count: u32 },
    /// Build a structure matching blueprint_id.
    Build { blueprint_id: String },
    /// Talk to an NPC.
    TalkTo { npc_id: String },
}

/// A single quest objective with completion state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub description: String,
    pub objective_type: ObjectiveType,
    pub completed: bool,
}

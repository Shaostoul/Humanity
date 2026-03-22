//! Quest objective types — what the player needs to accomplish for each step.

use serde::{Deserialize, Serialize};

/// A single step in a quest chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestStep {
    /// Human-readable description of what to do (e.g., "Gather 10 wood").
    pub description: String,
    /// The objective that must be satisfied to complete this step.
    pub objective: QuestObjective,
}

/// Specific objective types that the quest system can evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuestObjective {
    /// Collect items — checked against player inventory.
    Gather { item_id: String, quantity: u32 },
    /// Craft items — tracked via progress counter from crafting system.
    Craft { recipe_id: String, quantity: u32 },
    /// Harvest crops — tracked via progress counter from farming system.
    Harvest { crop_id: String, quantity: u32 },
    /// Build a structure — tracked via progress counter from construction system.
    Build { blueprint_id: String },
    /// Travel to a destination — tracked via progress counter from navigation system.
    Travel { destination: String },
    /// Talk to an NPC — tracked via progress counter from interaction system.
    Talk { npc_id: String },
}

//! Quest system — data-driven quest progression loaded from RON files.
//!
//! Quest definitions live in `data/quests/*.ron` and are deserialized into `QuestDef`.
//! The `QuestSystem` checks active quest objectives each tick, advances steps when
//! objectives are met, and awards item rewards on completion.

pub mod objectives;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

pub use objectives::{QuestObjective, QuestStep};

// ── Quest definition (deserialized from RON) ────────────────

/// A complete quest definition loaded from data files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestDef {
    /// Unique quest identifier (e.g., "tutorial_first_habitat").
    pub id: String,
    /// Human-readable quest name.
    pub name: String,
    /// Longer description shown in the quest journal.
    pub description: String,
    /// Ordered list of steps the player must complete.
    pub steps: Vec<QuestStep>,
    /// Item rewards granted on quest completion: (item_id, quantity).
    pub rewards: Vec<(String, u32)>,
    /// Quest ID that must be completed before this quest can be accepted.
    pub prerequisite: Option<String>,
}

/// Registry of all quest definitions, keyed by quest ID.
/// Stored in DataStore under key "quest_registry".
#[derive(Debug, Clone, Default)]
pub struct QuestRegistry {
    pub quests: HashMap<String, QuestDef>,
}

impl QuestRegistry {
    pub fn get(&self, id: &str) -> Option<&QuestDef> {
        self.quests.get(id)
    }
}

// ── Player quest state (ECS component) ──────────────────────

/// Tracks a single active quest's progress for a player entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveQuest {
    /// Which quest definition this tracks.
    pub quest_id: String,
    /// Index into QuestDef::steps for the current step (0-based).
    pub current_step: usize,
    /// Progress counters keyed by objective description (for count-based objectives).
    pub progress: HashMap<String, u32>,
}

/// Attach to the player entity to track quest state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuestTracker {
    /// Quests currently in progress.
    pub active_quests: Vec<ActiveQuest>,
    /// IDs of completed quests.
    pub completed_quests: Vec<String>,
}

impl QuestTracker {
    /// Whether a quest has been completed.
    pub fn is_completed(&self, quest_id: &str) -> bool {
        self.completed_quests.iter().any(|id| id == quest_id)
    }

    /// Whether a quest is currently active.
    pub fn is_active(&self, quest_id: &str) -> bool {
        self.active_quests.iter().any(|q| q.quest_id == quest_id)
    }

    /// Start tracking a new quest (no-op if already active or completed).
    pub fn accept_quest(&mut self, quest_id: &str) {
        if self.is_active(quest_id) || self.is_completed(quest_id) {
            return;
        }
        self.active_quests.push(ActiveQuest {
            quest_id: quest_id.to_string(),
            current_step: 0,
            progress: HashMap::new(),
        });
        log::info!("Quest accepted: {}", quest_id);
    }
}

// ── Reward granting ─────────────────────────────────────────

/// Pending quest reward — queued for the inventory system to process.
/// Stored in DataStore under "quest_rewards" as Vec<PendingReward>.
#[derive(Debug, Clone)]
pub struct PendingReward {
    pub entity: hecs::Entity,
    pub item_id: String,
    pub quantity: u32,
}

// ── Quest system ────────────────────────────────────────────

/// Checks active quest objectives each tick, advances steps, awards rewards.
pub struct QuestSystem {
    _initialized: bool,
}

impl QuestSystem {
    pub fn new() -> Self {
        Self {
            _initialized: false,
        }
    }

    /// Check if a single objective is met given the player's inventory and progress map.
    fn check_objective(
        objective: &QuestObjective,
        inventory: Option<&Inventory>,
        progress: &HashMap<String, u32>,
    ) -> bool {
        match objective {
            QuestObjective::Gather { item_id, quantity } => {
                // Check inventory for required items
                inventory
                    .map(|inv| inv.count_item(item_id) >= *quantity)
                    .unwrap_or(false)
            }
            QuestObjective::Craft { recipe_id, quantity } => {
                // Track via progress counter (crafting system increments this)
                let key = format!("craft_{}", recipe_id);
                progress.get(&key).copied().unwrap_or(0) >= *quantity
            }
            QuestObjective::Harvest { crop_id, quantity } => {
                // Track via progress counter (farming system increments this)
                let key = format!("harvest_{}", crop_id);
                progress.get(&key).copied().unwrap_or(0) >= *quantity
            }
            QuestObjective::Build { blueprint_id } => {
                // Track via progress counter (construction system sets this)
                let key = format!("build_{}", blueprint_id);
                progress.get(&key).copied().unwrap_or(0) >= 1
            }
            QuestObjective::Travel { destination } => {
                // Track via progress counter (navigation system sets this)
                let key = format!("travel_{}", destination);
                progress.get(&key).copied().unwrap_or(0) >= 1
            }
            QuestObjective::Talk { npc_id } => {
                // Track via progress counter (interaction system sets this)
                let key = format!("talk_{}", npc_id);
                progress.get(&key).copied().unwrap_or(0) >= 1
            }
        }
    }
}

impl System for QuestSystem {
    fn name(&self) -> &str {
        "QuestSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        let registry = match data.get::<QuestRegistry>("quest_registry") {
            Some(r) => r,
            None => return, // No quests loaded yet
        };

        // Collect entities with QuestTracker to process
        let mut updates: Vec<(hecs::Entity, QuestTracker, Vec<(String, Vec<(String, u32)>)>)> =
            Vec::new();

        for (entity, (tracker, inventory)) in
            world.query_mut::<(&QuestTracker, Option<&Inventory>)>()
        {
            let mut tracker = tracker.clone();
            let mut completed_this_tick: Vec<(String, Vec<(String, u32)>)> = Vec::new();
            let mut quests_to_advance: Vec<(usize, usize)> = Vec::new(); // (quest_index, new_step)
            let mut quests_to_complete: Vec<usize> = Vec::new();

            for (qi, active) in tracker.active_quests.iter().enumerate() {
                let quest_def = match registry.get(&active.quest_id) {
                    Some(def) => def,
                    None => continue, // Quest definition not found
                };

                // Check if current step is within bounds
                if active.current_step >= quest_def.steps.len() {
                    // All steps done — mark for completion
                    quests_to_complete.push(qi);
                    completed_this_tick
                        .push((active.quest_id.clone(), quest_def.rewards.clone()));
                    continue;
                }

                let step = &quest_def.steps[active.current_step];
                if Self::check_objective(&step.objective, inventory, &active.progress) {
                    let next_step = active.current_step + 1;
                    if next_step >= quest_def.steps.len() {
                        // Final step completed
                        quests_to_complete.push(qi);
                        completed_this_tick
                            .push((active.quest_id.clone(), quest_def.rewards.clone()));
                    } else {
                        quests_to_advance.push((qi, next_step));
                    }
                }
            }

            // Apply step advances (do this before removals to keep indices valid)
            for (qi, new_step) in &quests_to_advance {
                tracker.active_quests[*qi].current_step = *new_step;
                log::info!(
                    "Quest '{}': advanced to step {}",
                    tracker.active_quests[*qi].quest_id,
                    new_step
                );
            }

            // Complete quests (remove in reverse order to preserve indices)
            quests_to_complete.sort_unstable();
            for qi in quests_to_complete.into_iter().rev() {
                let quest_id = tracker.active_quests[qi].quest_id.clone();
                tracker.active_quests.remove(qi);
                tracker.completed_quests.push(quest_id.clone());
                log::info!("Quest completed: {}", quest_id);
            }

            if !completed_this_tick.is_empty()
                || !quests_to_advance.is_empty()
            {
                updates.push((entity, tracker, completed_this_tick));
            }
        }

        // Apply tracker updates and grant rewards directly to inventory
        for (entity, tracker, completed) in updates {
            if let Ok(mut t) = world.get::<&mut QuestTracker>(entity) {
                *t = tracker;
            }

            // Grant item rewards for completed quests
            for (_quest_id, rewards) in completed {
                for (item_id, quantity) in rewards {
                    if let Ok(mut inv) = world.get::<&mut Inventory>(entity) {
                        let overflow = inv.add_item(&item_id, quantity, 99);
                        if overflow > 0 {
                            log::warn!(
                                "Quest reward overflow: {} of {} could not fit in inventory",
                                overflow,
                                item_id
                            );
                        }
                    }
                }
            }
        }

        self._initialized = true;
    }
}

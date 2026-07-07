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
    /// Skill XP granted on completion: (skill_id, amount). serde-default so
    /// existing quest files load unchanged (v0.747.x, ladder rung 4 — the
    /// Quests page always promised XP; QuestDef finally carries it).
    #[serde(default)]
    pub xp_rewards: Vec<(String, u32)>,
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

    /// Load every `*.ron` quest file in a directory (each a `Vec<QuestDef>`) and
    /// merge them into one registry. Data-driven (infinite-of-X): drop a new
    /// `.ron` into `data/quests/` and its quests appear. Malformed or unreadable
    /// files are logged + skipped — never panics (same degradation policy as the
    /// CSV registries). This is the constructor the runtime calls to populate
    /// `DataStore["quest_registry"]`; without it QuestSystem finds no quests.
    pub fn from_ron_dir(dir: &std::path::Path) -> Self {
        let mut quests = HashMap::new();
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|x| x == "ron").unwrap_or(false) {
                        match std::fs::read_to_string(&path) {
                            Ok(text) => match ron::from_str::<Vec<QuestDef>>(&text) {
                                Ok(defs) => {
                                    for def in defs {
                                        quests.insert(def.id.clone(), def);
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Quest file {} parse error: {e}", path.display())
                                }
                            },
                            Err(e) => {
                                log::warn!("Quest file {} read error: {e}", path.display())
                            }
                        }
                    }
                }
            }
            Err(e) => log::warn!("Quests dir {} unreadable: {e}", dir.display()),
        }
        log::info!("Loaded {} quest definitions", quests.len());
        Self { quests }
    }
}

/// Push a quest-progress event key (e.g. `"craft_smelt_iron"`, `"harvest_potato"`)
/// onto the shared `"quest_events"` DataStore channel. Action systems call this on
/// completion; [`QuestSystem`] drains it each tick and bumps matching progress
/// counters so count-based objectives (Craft/Harvest/…) advance. No-ops cleanly if
/// the channel is absent (e.g. a headless/test world that never registered it).
pub fn push_quest_event(data: &DataStore, key: String) {
    if let Some(lock) = data.get::<std::sync::Mutex<Vec<String>>>("quest_events") {
        if let Ok(mut events) = lock.lock() {
            events.push(key);
        }
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

        // Drain quest-progress events the action systems pushed this frame
        // ("craft_<recipe>", "harvest_<crop>", ...). Applied to every active
        // quest's progress map below so count-based objectives advance. (Gather
        // objectives are checked against live inventory and need no events.)
        let events: Vec<String> = data
            .get::<std::sync::Mutex<Vec<String>>>("quest_events")
            .and_then(|m| m.lock().ok().map(|mut e| e.drain(..).collect()))
            .unwrap_or_default();

        // Collect entities with QuestTracker to process. Completed tuples carry
        // (quest_id, item rewards, xp rewards).
        #[allow(clippy::type_complexity)]
        let mut updates: Vec<(
            hecs::Entity,
            QuestTracker,
            Vec<(String, Vec<(String, u32)>, Vec<(String, u32)>)>,
        )> = Vec::new();

        for (entity, (tracker, inventory)) in
            world.query_mut::<(&QuestTracker, Option<&Inventory>)>()
        {
            let mut tracker = tracker.clone();
            // Apply this frame's progress events to every active quest's counters
            // (e.g. a "craft_smelt_iron" event bumps progress["craft_smelt_iron"]).
            let events_applied = !events.is_empty() && !tracker.active_quests.is_empty();
            if events_applied {
                for active in tracker.active_quests.iter_mut() {
                    for key in &events {
                        *active.progress.entry(key.clone()).or_insert(0) += 1;
                    }
                }
            }
            let mut completed_this_tick: Vec<(String, Vec<(String, u32)>, Vec<(String, u32)>)> =
                Vec::new();
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
                    completed_this_tick.push((
                        active.quest_id.clone(),
                        quest_def.rewards.clone(),
                        quest_def.xp_rewards.clone(),
                    ));
                    continue;
                }

                let step = &quest_def.steps[active.current_step];
                if Self::check_objective(&step.objective, inventory, &active.progress) {
                    let next_step = active.current_step + 1;
                    if next_step >= quest_def.steps.len() {
                        // Final step completed
                        quests_to_complete.push(qi);
                        completed_this_tick.push((
                            active.quest_id.clone(),
                            quest_def.rewards.clone(),
                            quest_def.xp_rewards.clone(),
                        ));
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

            // Prerequisite chaining: completing a quest auto-accepts any quest whose
            // prerequisite it satisfies (and that isn't already active or completed).
            for (completed_id, _, _) in &completed_this_tick {
                for def in registry.quests.values() {
                    if def.prerequisite.as_deref() == Some(completed_id.as_str())
                        && !tracker.is_active(&def.id)
                        && !tracker.is_completed(&def.id)
                    {
                        tracker.accept_quest(&def.id);
                    }
                }
            }

            if events_applied
                || !completed_this_tick.is_empty()
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

            // Grant item + XP rewards for completed quests (XP via the shared
            // xp_grants channel, drained by SkillSystem later this same frame).
            for (_quest_id, rewards, xp_rewards) in completed {
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
                for (skill_id, amount) in xp_rewards {
                    crate::systems::skills::award_skill_xp(data, &skill_id, amount);
                }
            }
        }

        self._initialized = true;
    }
}

#[cfg(test)]
mod quest_tests {
    use super::*;

    fn quest(id: &str, obj: QuestObjective, reward: Vec<(String, u32)>, prereq: Option<&str>) -> QuestDef {
        QuestDef {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            steps: vec![QuestStep {
                description: String::new(),
                objective: obj,
            }],
            rewards: reward,
            xp_rewards: Vec::new(),
            prerequisite: prereq.map(|s| s.to_string()),
        }
    }

    /// v0.748 (ladder rung 4): completing a quest grants its xp_rewards through
    /// the shared xp_grants channel (drained by SkillSystem the same frame).
    #[test]
    fn completion_grants_xp_rewards() {
        let mut reg = QuestRegistry::default();
        let mut q = quest(
            "xp_quest",
            QuestObjective::Gather { item_id: "stick_0".into(), quantity: 1 },
            vec![],
            None,
        );
        q.xp_rewards = vec![("farming".to_string(), 25)];
        reg.quests.insert(q.id.clone(), q);

        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert(
            "xp_grants",
            std::sync::Mutex::new(Vec::<crate::systems::skills::SkillXPEvent>::new()),
        );
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(4);
        inv.add_item("stick_0", 1, 99);
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("xp_quest");
        world.spawn((inv, tracker));

        let mut sys = QuestSystem::new();
        sys.tick(&mut world, 0.1, &data);

        let grants = data
            .get::<std::sync::Mutex<Vec<crate::systems::skills::SkillXPEvent>>>("xp_grants")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(
            grants.iter().any(|g| g.skill_id == "farming" && g.amount == 25),
            "quest completion pushed the XP grant, got {grants:?}"
        );
    }

    #[test]
    fn from_ron_dir_loads_the_real_quests() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/quests");
        let reg = QuestRegistry::from_ron_dir(&dir);
        assert!(reg.get("gs_first_steps").is_some(), "the getting-started chain loads");
        assert!(reg.quests.len() >= 4, "all quest files merged, got {}", reg.quests.len());
    }

    #[test]
    fn gather_quest_completes_from_inventory_and_grants_reward() {
        let mut reg = QuestRegistry::default();
        reg.quests.insert(
            "q_gather".into(),
            quest(
                "q_gather",
                QuestObjective::Gather { item_id: "iron_ore_0".into(), quantity: 3 },
                vec![("iron_ingot_0".into(), 2)],
                None,
            ),
        );
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);

        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("q_gather");
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 99); // one short
        let player = world.spawn((tracker, inv));
        let mut sys = QuestSystem::new();

        sys.tick(&mut world, 0.0, &data);
        assert!(
            !world.get::<&QuestTracker>(player).unwrap().is_completed("q_gather"),
            "2 < 3 ore → incomplete"
        );

        world.get::<&mut Inventory>(player).unwrap().add_item("iron_ore_0", 1, 99);
        sys.tick(&mut world, 0.0, &data);
        let t = world.get::<&QuestTracker>(player).unwrap();
        assert!(t.is_completed("q_gather"), "3 ore → quest completes");
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("iron_ingot_0"),
            2,
            "completion granted the reward"
        );
    }

    #[test]
    fn craft_event_completes_quest_and_chains_prerequisite() {
        let mut reg = QuestRegistry::default();
        reg.quests.insert(
            "q_craft".into(),
            quest(
                "q_craft",
                QuestObjective::Craft { recipe_id: "smelt_iron".into(), quantity: 1 },
                vec![],
                None,
            ),
        );
        reg.quests.insert(
            "q_next".into(),
            quest(
                "q_next",
                QuestObjective::Gather { item_id: "iron_ingot_0".into(), quantity: 1 },
                vec![],
                Some("q_craft"),
            ),
        );
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));

        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("q_craft");
        let player = world.spawn((tracker, Inventory::new(16)));
        let mut sys = QuestSystem::new();

        sys.tick(&mut world, 0.0, &data);
        assert!(!world.get::<&QuestTracker>(player).unwrap().is_completed("q_craft"));

        // Emit the craft event CraftingSystem would push on completing smelt_iron.
        push_quest_event(&data, "craft_smelt_iron".to_string());
        sys.tick(&mut world, 0.0, &data);

        let t = world.get::<&QuestTracker>(player).unwrap();
        assert!(t.is_completed("q_craft"), "craft event completed the Craft quest");
        assert!(
            t.is_active("q_next"),
            "completing q_craft auto-accepts its dependent q_next"
        );
    }
}

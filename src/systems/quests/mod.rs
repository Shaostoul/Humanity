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

// ── Travel destinations (v0.979, the Travel-objective emitter) ──────

/// One named world destination a Travel objective can point at. Positions are
/// XZ in the homestead-world frame (the same frame `entities/wild_spawns.ron`
/// uses), so a destination can mark a spawn cluster, a field, or any landmark.
#[derive(Debug, Clone, Deserialize)]
pub struct DestinationDef {
    /// The id Travel(destination: ...) references; the emitter fires
    /// "travel_<id>" when the player enters the radius.
    pub id: String,
    /// Player-facing name (quest journal copy can reference it).
    pub label: String,
    /// World XZ centre.
    pub pos: (f32, f32),
    /// Arrival radius in metres.
    pub radius: f32,
}

/// The destination list. DataStore: `"quest_destinations"`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DestinationList {
    pub destinations: Vec<DestinationDef>,
}

impl DestinationList {
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        ron::from_str(text).map_err(|e| e.to_string())
    }
}

/// Edge-triggered arrival detection, pure for tests: returns the ids of
/// destinations the player just ENTERED this tick (fired as
/// "travel_<id>" events by the caller), updating `inside` (the set of
/// destinations the player currently stands in). Leaving removes the id, so
/// walking out and back re-fires - a quest accepted after a first visit still
/// completes on the next walk-through (quest progress only records events
/// while the quest is active).
pub fn travel_transitions(
    player_xz: (f32, f32),
    dests: &DestinationList,
    inside: &mut std::collections::HashSet<String>,
) -> Vec<String> {
    let mut entered = Vec::new();
    for d in &dests.destinations {
        let dx = player_xz.0 - d.pos.0;
        let dz = player_xz.1 - d.pos.1;
        let in_radius = dx * dx + dz * dz <= d.radius * d.radius;
        if in_radius && !inside.contains(&d.id) {
            inside.insert(d.id.clone());
            entered.push(d.id.clone());
        } else if !in_radius {
            inside.remove(&d.id);
        }
    }
    entered
}

/// Where the player stands, as (x, z) in the frame destinations use, for the
/// Travel emitter (2026-10-04, the first-hour audit's B5). In first person the
/// camera IS the player: walking moves the camera, and the player entity's
/// Transform keeps wherever spawning or a teleport last put it, so reading the
/// Transform meant a Travel step never fired however far the player walked.
/// The game publishes "camera_position" every frame (lib.rs, the home-local
/// camera its proximity checks read, the walk-up interaction among them), and
/// this reads it; the controlled entity's Transform is the fallback only where
/// nothing publishes one (a headless world, a test).
pub fn player_xz(world: &hecs::World, data: &DataStore) -> Option<(f32, f32)> {
    if let Some(p) = data.get::<glam::Vec3>("camera_position") {
        return Some((p.x, p.z));
    }
    world
        .query::<(&crate::ecs::components::Transform, &crate::ecs::components::Controllable)>()
        .iter()
        .next()
        .map(|(_, (tf, _))| (tf.position.x, tf.position.z))
}

/// Stable quest key for an NPC display name: lowercase, every non-alphanumeric
/// run collapsed to one underscore, trimmed. "Mira Chen" -> "mira_chen", so a
/// quest authors Talk(npc_id: "mira_chen") no matter how the relay styles the
/// name. The talk emitter (lib.rs dialogue-open) fires "talk_<this>".
pub fn npc_talk_key(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_us = true; // suppress a leading underscore
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_us = false;
        } else if !last_us {
            out.push('_');
            last_us = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// Push a quest-progress event key (e.g. `"craft_smelt_iron"`, `"harvest_potato"`)
/// onto the shared `"quest_events"` DataStore channel. Action systems call this on
/// completion; [`QuestSystem`] drains it each tick and bumps matching progress
/// counters so count-based objectives (Craft/Harvest/…) advance. No-ops cleanly if
/// the channel is absent (e.g. a headless/test world that never registered it).
/// The quest event one unit of `item_id` made by a recipe reports, read by
/// `Make` objectives (2026-10-02).
pub fn make_event_key(item_id: &str) -> String {
    format!("make_{item_id}")
}

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
    /// Destinations the player is currently standing inside (edge-trigger
    /// state for the Travel emitter, v0.979).
    inside_destinations: std::collections::HashSet<String>,
}

impl QuestSystem {
    pub fn new() -> Self {
        Self {
            inside_destinations: std::collections::HashSet::new(),
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
            QuestObjective::Make { item_id, quantity } => {
                // Units made by any recipe (crafting pushes one event per unit)
                progress.get(&make_event_key(item_id)).copied().unwrap_or(0) >= *quantity
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

        // Travel emitter (v0.979): fire "travel_<id>" the moment the player
        // steps into a destination radius (data/entities/destinations.ron).
        // Runs before the drain below, so an arrival advances its Travel
        // objective in the SAME tick. Where the player is comes from the
        // walking camera (`player_xz`, 2026-10-04): the entity's Transform
        // never moved when they walked.
        if let Some(dests) = data.get::<DestinationList>("quest_destinations") {
            if let Some(xz) = player_xz(world, data) {
                for id in travel_transitions(xz, dests, &mut self.inside_destinations) {
                    push_quest_event(data, format!("travel_{id}"));
                }
            }
        }

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

    /// Travel emitter (v0.979): the edge trigger fires exactly on entry,
    /// stays quiet while standing inside, and re-fires after leave + return
    /// (a quest accepted after a first visit completes on the next pass).
    #[test]
    fn travel_transitions_edge_trigger() {
        let dests = DestinationList {
            destinations: vec![DestinationDef {
                id: "fields".into(),
                label: "the fields".into(),
                pos: (10.0, 10.0),
                radius: 5.0,
            }],
        };
        let mut inside = std::collections::HashSet::new();
        // Approach from outside: no fire.
        assert!(travel_transitions((30.0, 30.0), &dests, &mut inside).is_empty());
        // Entry fires once.
        assert_eq!(travel_transitions((11.0, 11.0), &dests, &mut inside), vec!["fields"]);
        // Standing inside stays quiet.
        assert!(travel_transitions((9.0, 12.0), &dests, &mut inside).is_empty());
        // Leave, then return: fires again.
        assert!(travel_transitions((30.0, 30.0), &dests, &mut inside).is_empty());
        assert_eq!(travel_transitions((10.0, 10.0), &dests, &mut inside), vec!["fields"]);
    }

    #[test]
    fn npc_talk_key_slugs_names_stably() {
        assert_eq!(npc_talk_key("Mira Chen"), "mira_chen");
        assert_eq!(npc_talk_key("  D'Arcy-Lou  "), "d_arcy_lou");
        assert_eq!(npc_talk_key("R2"), "r2");
        assert_eq!(npc_talk_key("___"), "");
    }

    /// THE quest-data lockstep (v0.981, post-audit item 1 made permanent):
    /// every id any shipped quest objective references must exist in its
    /// source-of-truth data file. The 2026-07-20 audit found quests ~80%
    /// dead-id; the rewrite fixed them, and this test keeps them fixed - a
    /// quest naming a nonexistent item/recipe/blueprint/crop/destination can
    /// never advance and now fails the build instead of failing the player.
    /// (Talk ids are exempt: crew names come from the RELAY at runtime, not
    /// from static data - the npc_talk_key convention is their contract.)
    #[test]
    fn shipped_quest_objective_ids_all_resolve() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let csv_ids = |rel: &str| -> std::collections::HashSet<String> {
            std::fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("{rel} readable: {e}"))
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .filter_map(|l| l.split(',').next().map(|s| s.trim().to_string()))
                .collect()
        };
        let items = csv_ids("data/items.csv");
        let recipes = csv_ids("data/recipes.csv");
        // Every item some recipe outputs (column 5, "id:qty|id:qty").
        let made: std::collections::HashSet<String> = std::fs::read_to_string(root.join("data/recipes.csv"))
            .expect("recipes.csv readable")
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split(',').nth(4))
            .flat_map(|outs| outs.split('|').filter_map(|o| o.split(':').next()).map(|s| s.trim().to_string()))
            .collect();
        let plants = csv_ids("data/plants.csv");
        let blueprints = crate::systems::construction::BlueprintRegistry::from_ron(
            &std::fs::read(root.join("data/blueprints/basic.ron")).unwrap(),
        )
        .expect("blueprints parse");
        let dests = DestinationList::from_ron(
            &std::fs::read(root.join("data/entities/destinations.ron")).unwrap(),
        )
        .expect("destinations parse");
        let dest_ids: std::collections::HashSet<&str> =
            dests.destinations.iter().map(|d| d.id.as_str()).collect();

        let reg = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        assert!(reg.quests.len() >= 10, "quest chains load, got {}", reg.quests.len());
        for q in reg.quests.values() {
            for s in &q.steps {
                match &s.objective {
                    QuestObjective::Gather { item_id, .. } => assert!(
                        items.contains(item_id),
                        "quest {}: Gather names unknown item {item_id}",
                        q.id
                    ),
                    QuestObjective::Craft { recipe_id, .. } => assert!(
                        recipes.contains(recipe_id),
                        "quest {}: Craft names unknown recipe {recipe_id}",
                        q.id
                    ),
                    // A Make item must exist AND some recipe must produce it,
                    // or the step can never count a unit.
                    QuestObjective::Make { item_id, .. } => {
                        assert!(items.contains(item_id), "quest {}: Make names unknown item {item_id}", q.id);
                        assert!(made.contains(item_id), "quest {}: no recipe makes {item_id}", q.id);
                    }
                    QuestObjective::Harvest { crop_id, .. } => assert!(
                        plants.contains(crop_id),
                        "quest {}: Harvest names unknown crop {crop_id}",
                        q.id
                    ),
                    QuestObjective::Build { blueprint_id } => assert!(
                        blueprints.blueprints.contains_key(blueprint_id),
                        "quest {}: Build names unknown blueprint {blueprint_id}",
                        q.id
                    ),
                    QuestObjective::Travel { destination } => assert!(
                        dest_ids.contains(destination.as_str()),
                        "quest {}: Travel names unknown destination {destination}",
                        q.id
                    ),
                    QuestObjective::Talk { .. } => {} // relay-runtime names, see doc
                }
                // Reward items must exist too - a completed quest that grants
                // a phantom item would vanish the reward silently.
            }
            for (item_id, _) in &q.rewards {
                assert!(
                    items.contains(item_id),
                    "quest {}: reward names unknown item {item_id}",
                    q.id
                );
            }
        }
    }

    /// Lockstep guard: every Travel(destination) referenced by any shipped
    /// quest exists in data/entities/destinations.ron - a quest pointing at a
    /// nonexistent place could never advance and would fail here, not in play.
    #[test]
    fn shipped_travel_destinations_all_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let dests = DestinationList::from_ron(
            &std::fs::read(root.join("data/entities/destinations.ron")).unwrap(),
        )
        .unwrap();
        assert!(dests.destinations.len() >= 4, "shipped destination list loads");
        let ids: std::collections::HashSet<&str> =
            dests.destinations.iter().map(|d| d.id.as_str()).collect();
        let reg = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        let mut travel_steps = 0;
        for q in reg.quests.values() {
            for s in &q.steps {
                if let QuestObjective::Travel { destination } = &s.objective {
                    travel_steps += 1;
                    assert!(
                        ids.contains(destination.as_str()),
                        "quest {} references unknown destination {destination}",
                        q.id
                    );
                }
            }
        }
        assert!(travel_steps >= 3, "the restored Travel steps load, got {travel_steps}");
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

    /// MAKE COUNTS EVERY ROUTE TO THE ITEM (2026-10-02). The first quest's
    /// iron step named `smelt_iron` (coal), so a player who smelted with
    /// graphite made the ingot and the quest did not move. A Make step counts
    /// units of the item from any recipe. Red check, run: with the starter
    /// quest's step as `Craft(smelt_iron)`, the graphite event below left it
    /// incomplete.
    #[test]
    fn make_objective_counts_any_recipe_that_produces_the_item() {
        let mut reg = QuestRegistry::default();
        reg.quests.insert(
            "q_make".into(),
            quest("q_make", QuestObjective::Make { item_id: "iron_ingot_0".into(), quantity: 2 }, vec![], None),
        );
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("q_make");
        let player = world.spawn((tracker, Inventory::new(16)));
        let mut sys = QuestSystem::new();

        // Slag is made alongside; it must not count toward iron.
        push_quest_event(&data, make_event_key("slag_0"));
        push_quest_event(&data, make_event_key("iron_ingot_0"));
        sys.tick(&mut world, 0.0, &data);
        assert!(!world.get::<&QuestTracker>(player).unwrap().is_completed("q_make"), "1 of 2 ingots");
        push_quest_event(&data, make_event_key("iron_ingot_0"));
        sys.tick(&mut world, 0.0, &data);
        assert!(world.get::<&QuestTracker>(player).unwrap().is_completed("q_make"), "2 of 2 ingots");

        // The shipped starter quest takes the graphite route.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let shipped = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        let steps = shipped.get("gs_first_steps").expect("gs_first_steps ships").steps.len();
        let mut data = DataStore::new();
        data.insert("quest_registry", shipped);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("gs_first_steps");
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 3, 99);
        let player = world.spawn((tracker, inv));
        sys.tick(&mut world, 0.0, &data);
        // What crafting reports for one smelt_iron_graphite batch.
        push_quest_event(&data, "craft_smelt_iron_graphite".to_string());
        push_quest_event(&data, make_event_key("iron_ingot_0"));
        push_quest_event(&data, make_event_key("slag_0"));
        for _ in 0..steps + 1 {
            sys.tick(&mut world, 0.0, &data);
        }
        assert!(
            world.get::<&QuestTracker>(player).unwrap().is_completed("gs_first_steps"),
            "smelting with graphite finishes First Steps"
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

    /// TRAVEL STEPS READ WHERE THE PLAYER IS (2026-10-04, the first-hour audit's
    /// B5). The emitter read the player entity's Transform, which only spawning
    /// and a teleport set: walking moves the camera (in first person the camera
    /// IS the player, lib.rs's walk), so a Travel step never fired however far
    /// the player walked. It now reads "camera_position", the home-local
    /// position the game publishes every frame for its proximity checks (the
    /// walk-up interaction reads the same). Here the player entity stays at the
    /// front door, where spawning left it, while the camera walks to the shipped
    /// outdoor fields.
    ///
    /// Red, run on the emitter before this: "walking to the outdoor fields
    /// completes Initial Survey's Travel step" (left: 1, right: 2).
    #[test]
    fn walking_to_a_destination_completes_its_travel_step() {
        use crate::ecs::components::{Controllable, Transform};
        use glam::Vec3;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let reg = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        let travel = reg
            .get("exploration_first_survey")
            .expect("Initial Survey ships")
            .steps
            .iter()
            .position(|s| matches!(&s.objective, QuestObjective::Travel { destination } if destination == "outdoor_fields"))
            .expect("Initial Survey walks to the outdoor fields");
        let dests = DestinationList::from_ron(&std::fs::read(root.join("data/entities/destinations.ron")).unwrap()).unwrap();
        let fields = dests.destinations.iter().find(|d| d.id == "outdoor_fields").expect("the fields are a destination").pos;
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert("quest_destinations", dests);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));

        let door = Vec3::new(53.5, 1.7, 40.5);
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("exploration_first_survey");
        tracker.active_quests[0].current_step = travel; // the compass is made
        let mut world = hecs::World::new();
        let player = world.spawn((Transform { position: door, ..Default::default() }, Controllable, tracker, Inventory::new(8)));
        let mut sys = QuestSystem::new();

        data.insert("camera_position", door);
        sys.tick(&mut world, 0.1, &data);
        assert_eq!(world.get::<&QuestTracker>(player).unwrap().active_quests[0].current_step, travel, "nothing fires at the door");
        // Walk there: only the camera moves, as in play.
        data.insert("camera_position", Vec3::new(fields.0, 1.7, fields.1));
        sys.tick(&mut world, 0.1, &data);
        assert_eq!(
            world.get::<&QuestTracker>(player).unwrap().active_quests[0].current_step,
            travel + 1,
            "walking to the outdoor fields completes Initial Survey's Travel step"
        );
    }

    /// The ore two exploration quests gather comes from a node a player can
    /// walk to (2026-10-04, B5). "Initial Survey" asks for 5 ore samples and
    /// "Distant Expeditions" for 3 rare ore, which only two creatures dropped
    /// that nothing spawns (tests/recipe_sources_lint.rs now fails on any
    /// Gather item with no source). Their sources are resource nodes in
    /// data/entities/wild_spawns.ron; this holds each such placement, scatter
    /// radius and all, inside a room of the home or a zone of the ship (ship
    /// metres, the home on plot p1 where offline play puts it), so the source
    /// is somewhere a player can stand, not in a wall or the void between.
    ///
    /// Red, run on the shipped data before this: "no placed node yields
    /// ore_sample_0" (nothing in wild_spawns.ron gave either ore).
    #[test]
    fn quest_ore_comes_from_nodes_a_player_can_walk_to() {
        use crate::systems::livestock::{CreatureRegistry, WildSpawnList};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let creatures = CreatureRegistry::from_csv(&std::fs::read(root.join("data/creatures.csv")).unwrap()).unwrap();
        let spawns = WildSpawnList::from_ron(&std::fs::read(root.join("data/entities/wild_spawns.ron")).unwrap()).unwrap();
        let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&root.join("data"), None)
            .expect("the shipped ship assembles");
        // Walkable boxes in ship metres, (id, x0, x1, z0, z1): the home's rooms and every other zone.
        let mut boxes: Vec<(String, f32, f32, f32, f32)> = Vec::new();
        let home_i = ship.home_zone_index();
        for (i, z) in ship.zones.iter().enumerate() {
            if i == home_i {
                for r in z.body.zones.iter().filter(|r| r.origin.0 >= 0.0 && r.origin.2 >= 0.0 && r.origin.0 + r.size.0 <= z.body.width && r.origin.2 + r.size.2 <= z.body.depth) {
                    let (x0, z0) = (z.origin.0 + r.origin.0, z.origin.2 + r.origin.2);
                    boxes.push((r.id.clone(), x0, x0 + r.size.0, z0, z0 + r.size.2));
                }
            } else {
                boxes.push((z.id.clone(), z.origin.0, z.origin.0 + z.body.width, z.origin.2, z.origin.2 + z.body.depth));
            }
        }
        assert!(boxes.len() > 20, "the home's rooms and the ship's zones, got {}", boxes.len());

        for item in ["ore_sample_0", "rare_ore_0"] {
            let nodes: Vec<_> = spawns
                .spawns
                .iter()
                .filter(|s| creatures.get(&s.creature).and_then(|d| d.renewable()).is_some_and(|p| p.item == item))
                .collect();
            assert!(!nodes.is_empty(), "no placed node yields {item}");
            for s in nodes {
                let inside = boxes.iter().find(|(_, x0, x1, z0, z1)| {
                    s.pos.0 - s.radius >= *x0 && s.pos.0 + s.radius <= *x1 && s.pos.1 - s.radius >= *z0 && s.pos.1 + s.radius <= *z1
                });
                assert!(
                    inside.is_some(),
                    "{} ({item}) at ({}, {}) with radius {} is not inside any room of the home or zone of the ship",
                    s.creature,
                    s.pos.0,
                    s.pos.1,
                    s.radius
                );
            }
        }
    }
}

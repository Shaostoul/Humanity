//! Skill progression: learn-by-doing with XP curves and level unlocks.
//!
//! Skill definitions loaded from `data/skills/skills.csv`.


use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use serde::Deserialize;
use std::collections::HashMap;

/// Skill definition loaded from CSV.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillDef {
    pub id: String,
    pub name: String,
    pub category: String,
    pub max_level: u32,
    pub xp_per_level: u32,
    pub description: String,
}

impl SkillDef {
    /// XP needed to reach a given level (exponential curve).
    pub fn xp_for_level(&self, level: u32) -> u32 {
        // Each level needs progressively more XP: base * level^1.5
        let base = self.xp_per_level as f64;
        (base * (level as f64).powf(1.5)) as u32
    }
}

/// Player's progress in a single skill.
#[derive(Debug, Clone)]
pub struct SkillProgress {
    pub level: u32,
    pub xp: u32,
}

/// Component tracking all skills for an entity.
pub struct PlayerSkills {
    pub skills: HashMap<String, SkillProgress>,
}

impl PlayerSkills {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Get current level for a skill.
    pub fn level(&self, skill_id: &str) -> u32 {
        self.skills.get(skill_id).map(|s| s.level).unwrap_or(0)
    }

    /// Get current XP for a skill.
    pub fn xp(&self, skill_id: &str) -> u32 {
        self.skills.get(skill_id).map(|s| s.xp).unwrap_or(0)
    }
}

/// Registry of all skill definitions.
pub struct SkillRegistry {
    pub skills: HashMap<String, SkillDef>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn register(&mut self, def: SkillDef) {
        self.skills.insert(def.id.clone(), def);
    }

    pub fn get(&self, id: &str) -> Option<&SkillDef> {
        self.skills.get(id)
    }

    /// Build the registry from raw `data/skills/skills.csv` bytes.
    ///
    /// `SkillDef`'s fields map 1:1 onto the CSV columns
    /// (id,name,category,max_level,xp_per_level,description), so the shared CSV
    /// loader deserializes rows straight into `SkillDef`. This is the constructor
    /// the runtime calls to populate `DataStore["skill_registry"]`; without it the
    /// SkillSystem would find no defs and every XP award would silently no-op.
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let defs: Vec<SkillDef> = crate::assets::loader::parse_csv(data)?;
        let mut skills = HashMap::new();
        for def in defs {
            skills.insert(def.id.clone(), def);
        }
        Ok(Self { skills })
    }
}

/// XP gain event queued for processing.
#[derive(Debug, Clone)]
pub struct SkillXPEvent {
    pub skill_id: String,
    pub amount: u32,
}

/// Push a skill-XP grant onto the shared `"xp_grants"` DataStore channel that
/// [`SkillSystem`] drains each tick. This is the decoupled path any action system
/// uses to award XP: they hold only `&DataStore` (not `&mut SkillSystem`), so they
/// can't call `award_xp` directly. No-ops cleanly if the channel is absent (e.g.
/// a headless/test world that never registered it) or the amount is zero.
pub fn award_skill_xp(data: &DataStore, skill_id: &str, amount: u32) {
    if amount == 0 {
        return;
    }
    if let Some(lock) = data.get::<std::sync::Mutex<Vec<SkillXPEvent>>>("xp_grants") {
        if let Ok(mut grants) = lock.lock() {
            grants.push(SkillXPEvent {
                skill_id: skill_id.to_string(),
                amount,
            });
        }
    }
}

/// Skill system processes XP events and levels up skills.
pub struct SkillSystem {
    pending_xp: Vec<SkillXPEvent>,
    level_ups: Vec<(String, u32)>,
}

impl SkillSystem {
    pub fn new() -> Self {
        Self {
            pending_xp: Vec::new(),
            level_ups: Vec::new(),
        }
    }

    /// Queue an XP gain event.
    pub fn award_xp(&mut self, skill_id: String, amount: u32) {
        self.pending_xp.push(SkillXPEvent { skill_id, amount });
    }

    /// Get and clear recent level-ups (for UI notifications).
    pub fn drain_level_ups(&mut self) -> Vec<(String, u32)> {
        self.level_ups.drain(..).collect()
    }
}

impl System for SkillSystem {
    fn name(&self) -> &str {
        "Skills"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        // XP arrives two ways: direct award_xp() calls (used by tests) and the
        // shared "xp_grants" DataStore channel that the action systems
        // (crafting/farming/mining) push to — they hold only `&DataStore`, not
        // `&mut SkillSystem`, so the channel is the decoupling. SkillSystem is
        // registered LAST in the runner, so grants pushed earlier THIS frame are
        // drained + applied the same frame.
        let registry = data.get::<SkillRegistry>("skill_registry");

        // Dev: max ALL skills (testing affordance — preserves the "develop as if
        // 100% unlocked" posture even with skill-gated crafting). Drains the
        // dev_max_skills bool channel (set by the profile Skills panel's button).
        let dev_max = data
            .get::<std::sync::Mutex<bool>>("dev_max_skills")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        if dev_max {
            if let Some(reg) = registry {
                for (_e, skills) in world.query_mut::<&mut PlayerSkills>() {
                    for (id, def) in reg.skills.iter() {
                        skills
                            .skills
                            .insert(id.clone(), SkillProgress { level: def.max_level, xp: 0 });
                    }
                    break;
                }
                log::info!("Dev: maxed all player skills");
            }
        }

        let mut events: Vec<SkillXPEvent> = self.pending_xp.drain(..).collect();
        if let Some(lock) = data.get::<std::sync::Mutex<Vec<SkillXPEvent>>>("xp_grants") {
            if let Ok(mut grants) = lock.lock() {
                events.append(&mut grants);
            }
        }
        if events.is_empty() {
            return;
        }

        // Find the entity with PlayerSkills (usually the local player)
        for (_entity, skills) in world.query_mut::<&mut PlayerSkills>() {
            for event in &events {
                let def = match registry.as_ref().and_then(|r| r.get(&event.skill_id)) {
                    Some(d) => d,
                    None => continue,
                };

                let progress = skills
                    .skills
                    .entry(event.skill_id.clone())
                    .or_insert(SkillProgress { level: 0, xp: 0 });

                if progress.level >= def.max_level {
                    continue;
                }

                progress.xp += event.amount;

                // Check for level up
                loop {
                    let needed = def.xp_for_level(progress.level + 1);
                    if progress.xp >= needed && progress.level < def.max_level {
                        progress.xp -= needed;
                        progress.level += 1;
                        self.level_ups
                            .push((event.skill_id.clone(), progress.level));
                        log::info!(
                            "Skill '{}' leveled up to {}!",
                            def.name,
                            progress.level
                        );
                    } else {
                        break;
                    }
                }
            }
            break; // Only process first entity with PlayerSkills
        }
    }
}

#[cfg(test)]
mod skill_tests {
    use super::*;

    fn skills_csv() -> &'static [u8] {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/skills/skills.csv"))
    }

    #[test]
    fn from_csv_parses_the_real_skill_registry() {
        let reg = SkillRegistry::from_csv(skills_csv()).expect("skills.csv parses");
        assert!(
            reg.skills.len() >= 20,
            "expected the full skill set, got {}",
            reg.skills.len()
        );
        let mining = reg.get("mining").expect("mining skill present");
        assert_eq!(mining.name, "Mining");
        assert_eq!(mining.xp_per_level, 100);
        // Exponential curve: level 1 needs xp_base * 1^1.5 = 100.
        assert_eq!(mining.xp_for_level(1), 100);
    }

    #[test]
    fn channel_xp_grant_levels_up_the_player() {
        let mut data = DataStore::new();
        data.insert(
            "skill_registry",
            SkillRegistry::from_csv(skills_csv()).unwrap(),
        );
        data.insert("xp_grants", std::sync::Mutex::new(Vec::<SkillXPEvent>::new()));

        let mut world = hecs::World::new();
        let player = world.spawn((PlayerSkills::new(),));
        let mut sys = SkillSystem::new();

        // 50 XP — below the 100 needed for level 1 → no level-up yet.
        award_skill_xp(&data, "mining", 50);
        sys.tick(&mut world, 0.0, &data);
        {
            let sk = world.get::<&PlayerSkills>(player).unwrap();
            assert_eq!(sk.level("mining"), 0);
            assert_eq!(sk.xp("mining"), 50);
        }

        // Another 50 → 100 total → reaches level 1 (leftover XP resets to 0).
        award_skill_xp(&data, "mining", 50);
        sys.tick(&mut world, 0.0, &data);
        {
            let sk = world.get::<&PlayerSkills>(player).unwrap();
            assert_eq!(sk.level("mining"), 1, "100 XP reaches mining level 1");
            assert_eq!(sk.xp("mining"), 0, "leftover XP after the level-up is 0");
        }
    }

    /// Drift guard (the recipe-skill lint): every non-empty `skill_required` in
    /// recipes.csv MUST resolve to a real skill id in skills.csv, or the XP award
    /// silently no-ops on the registry lookup miss. The recipe vocabulary was
    /// reconciled to the canonical skills in v0.340.0; this keeps it reconciled.
    #[test]
    fn every_recipe_skill_is_a_real_skill() {
        let skills = SkillRegistry::from_csv(skills_csv()).unwrap();
        let recipes = crate::systems::crafting::RecipeRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/recipes.csv"
        )))
        .expect("recipes.csv parses");
        let mut orphans: Vec<String> = recipes
            .recipes
            .values()
            .filter_map(|r| {
                r.skill_required
                    .as_ref()
                    .filter(|s| skills.get(s).is_none())
                    .map(|s| format!("{} -> {}", r.id, s))
            })
            .collect();
        orphans.sort();
        assert!(
            orphans.is_empty(),
            "recipes reference skills absent from data/skills/skills.csv (XP would \
             silently vanish). Fix recipes.csv skill_required or add the skill:\n  {}",
            orphans.join("\n  ")
        );
    }

    /// #8b dev affordance: the dev_max_skills command maxes EVERY skill on the
    /// player (so skill-gated recipes stay testable under the "100% unlocked" posture).
    #[test]
    fn dev_max_skills_maxes_every_skill() {
        let mut data = DataStore::new();
        data.insert(
            "skill_registry",
            SkillRegistry::from_csv(skills_csv()).unwrap(),
        );
        data.insert("dev_max_skills", std::sync::Mutex::new(false));

        let mut world = hecs::World::new();
        let player = world.spawn((PlayerSkills::new(),));
        let mut sys = SkillSystem::new();

        *data
            .get::<std::sync::Mutex<bool>>("dev_max_skills")
            .unwrap()
            .lock()
            .unwrap() = true;
        sys.tick(&mut world, 0.0, &data);

        let sk = world.get::<&PlayerSkills>(player).unwrap();
        assert_eq!(sk.level("mining"), 25, "mining maxed to its max_level");
        assert_eq!(sk.level("foraging"), 20, "foraging maxed to its max_level");
        assert!(sk.skills.len() >= 20, "every skill present after dev-max");
    }
}

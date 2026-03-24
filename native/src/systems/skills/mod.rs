//! Skill progression: learn-by-doing with XP curves and level unlocks.
//!
//! Skill definitions loaded from `data/skills/skills.csv`.

pub mod learning;

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
}

/// XP gain event queued for processing.
pub struct SkillXPEvent {
    pub skill_id: String,
    pub amount: u32,
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
        if self.pending_xp.is_empty() {
            return;
        }

        let events: Vec<SkillXPEvent> = self.pending_xp.drain(..).collect();
        let registry = data.get::<SkillRegistry>("skill_registry");

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

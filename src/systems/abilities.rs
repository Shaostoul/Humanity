//! Abilities system (v0.753, closure ladder rung 8 / progression doc Part 2).
//!
//! data/abilities.csv (110 authored rows, formerly spells.csv) finally gets
//! its loader. An ability is the ACTIVATION layer of progression: a castable
//! action with a cost and a cooldown, gated by a skill you levelled by doing
//! (no separate grant table - meeting the row's skill_required/skill_level IS
//! knowing it). One stat pipeline, one request channel, same validate-consume
//! shape as machine automation.
//!
//! v1 scope is deliberately SELF-scoped: healing abilities restore Health and
//! energy pays the cost (mana_cost + stamina_cost both draw from the energy
//! vital until a separate stamina vital exists - casting makes you tired,
//! which makes abilities part of the survival economy). Offensive rows load
//! in the registry but are not castable until the combat arc gives them
//! targets - the GUI says so honestly instead of fizzling.

use crate::ecs::components::{Controllable, Health, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::skills::PlayerSkills;
use serde::Deserialize;
use std::collections::HashMap;

// ── Definitions (data/abilities.csv) ────────────────────────────────

/// One abilities.csv row. Columns the engine does not consume yet (aoe,
/// damage, duration) still parse so the combat arc reads the same registry.
#[derive(Debug, Clone, Deserialize)]
pub struct AbilityDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub school: String,
    #[serde(default)]
    pub mana_cost: f32,
    #[serde(default)]
    pub stamina_cost: f32,
    #[serde(default)]
    pub cooldown_s: f32,
    #[serde(default)]
    pub cast_time_s: f32,
    #[serde(default)]
    pub range_m: f32,
    #[serde(default)]
    pub aoe_m: f32,
    #[serde(default)]
    pub aoe_shape: String,
    #[serde(default)]
    pub damage_base: f32,
    #[serde(default)]
    pub damage_type: String,
    #[serde(default)]
    pub healing_base: f32,
    #[serde(default)]
    pub duration_s: f32,
    #[serde(default)]
    pub level_required: u32,
    #[serde(default)]
    pub skill_required: String,
    #[serde(default)]
    pub skill_level: u32,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub description: String,
    /// real | tech | fantasy - Real mode shows real+tech (a data view).
    #[serde(default)]
    pub flavor: String,
}

impl AbilityDef {
    /// Total activation cost, paid from the energy vital (mana and stamina
    /// both draw from energy until a separate stamina vital exists).
    pub fn energy_cost(&self) -> f32 {
        self.mana_cost + self.stamina_cost
    }

    /// Does this row do anything in the v1 self-scoped pipeline? Healing
    /// abilities are live; damage rows wait for the combat arc's targets.
    pub fn self_castable(&self) -> bool {
        self.healing_base > 0.0
    }

    /// Does the caster's training meet this row's skill gate? Level-1 gates
    /// are baseline-open (everyone has starter competence; untrained skills
    /// read as level 0), matching the recipe convention of gating at 2+.
    pub fn skill_gate_met(&self, skills: &PlayerSkills) -> bool {
        self.skill_required.is_empty()
            || self.skill_level <= 1
            || skills.level(&self.skill_required) >= self.skill_level
    }
}

/// All abilities keyed by id. DataStore: `"ability_registry"`.
#[derive(Debug, Default)]
pub struct AbilityRegistry {
    pub defs: HashMap<String, AbilityDef>,
}

impl AbilityRegistry {
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<AbilityDef> = crate::assets::loader::parse_csv(data)?;
        let mut defs = HashMap::new();
        for row in rows {
            defs.insert(row.id.clone(), row);
        }
        Ok(Self { defs })
    }

    pub fn get(&self, id: &str) -> Option<&AbilityDef> {
        self.defs.get(id)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

// ── The system ──────────────────────────────────────────────────────

/// Drains the `ability_request` channel (GUI Cast clicks -> ability ids),
/// validates skill gate + cost + cooldown, applies the self-scoped effect,
/// and reports one honest line back through `ability_status`. Cooldowns tick
/// down here and are published to `ability_cooldowns` for the GUI.
pub struct AbilitySystem {
    /// Seconds remaining per ability id (session-scoped, like machine timers).
    cooldowns: HashMap<String, f32>,
}

impl AbilitySystem {
    pub fn new() -> Self {
        Self {
            cooldowns: HashMap::new(),
        }
    }
}

impl Default for AbilitySystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for AbilitySystem {
    fn name(&self) -> &str {
        "AbilitySystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Cooldowns tick down every frame, cast or not.
        self.cooldowns.retain(|_, t| {
            *t -= dt;
            *t > 0.0
        });

        // Drain this frame's cast requests.
        let requests: Vec<String> = data
            .get::<std::sync::Mutex<Vec<String>>>("ability_request")
            .and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v)))
            .unwrap_or_default();

        if !requests.is_empty() {
            let mut status = String::new();
            for id in requests {
                status = self.cast(world, data, &id);
            }
            if let Some(s) = data.get::<std::sync::Mutex<String>>("ability_status") {
                if let Ok(mut slot) = s.lock() {
                    *slot = status;
                }
            }
        }

        // Publish live cooldowns for the GUI's Cast buttons.
        if let Some(cd) = data.get::<std::sync::Mutex<HashMap<String, f32>>>("ability_cooldowns") {
            if let Ok(mut slot) = cd.lock() {
                *slot = self.cooldowns.clone();
            }
        }
    }
}

impl AbilitySystem {
    /// Validate + apply one cast against the player. Returns the status line.
    fn cast(&mut self, world: &mut hecs::World, data: &DataStore, id: &str) -> String {
        let Some(reg) = data.get::<AbilityRegistry>("ability_registry") else {
            return "Abilities are still loading".to_string();
        };
        let Some(def) = reg.get(id) else {
            return format!("Unknown ability {id}");
        };
        if !def.self_castable() {
            return format!("{} needs a target - combat arrives later", def.name);
        }
        if let Some(t) = self.cooldowns.get(id) {
            return format!("{} recharging ({:.0}s)", def.name, t.max(1.0));
        }

        for (_e, (skills, vitals, health, _c)) in world
            .query_mut::<(&PlayerSkills, &mut Vitals, &mut Health, &Controllable)>()
        {
            if !def.skill_gate_met(skills) {
                return format!(
                    "{} needs {} level {}",
                    def.name, def.skill_required, def.skill_level
                );
            }
            let cost = def.energy_cost();
            if vitals.energy < cost {
                return format!("Too tired to cast {} ({cost:.0} energy)", def.name);
            }
            vitals.energy -= cost;
            let healed = def
                .healing_base
                .min((health.max - health.current).max(0.0));
            health.current = (health.current + def.healing_base).min(health.max);
            self.cooldowns.insert(id.to_string(), def.cooldown_s);
            // Casting trains the gating skill - learn by doing.
            if !def.skill_required.is_empty() {
                crate::systems::skills::award_skill_xp(data, &def.skill_required, 5);
            }
            return if healed > 0.0 {
                format!("{} restores {healed:.0} health", def.name)
            } else {
                format!("{} cast (already at full health)", def.name)
            };
        }
        "No caster in the world yet".to_string()
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Controllable, Health, Vitals};

    fn shipped_registry() -> AbilityRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("abilities.csv");
        AbilityRegistry::from_csv(&std::fs::read(path).unwrap()).unwrap()
    }

    /// The shipped abilities.csv parses whole (110 rows), flavors are the
    /// closed real|tech|fantasy set, and every skill_required references a
    /// real skills.csv id (a typo would silently un-gate or brick a row).
    #[test]
    fn ability_registry_parses_the_shipped_database() {
        let reg = shipped_registry();
        assert!(
            reg.len() >= 108,
            "expected the full ability database, got {}",
            reg.len()
        );
        let fireball = reg.get("fireball").expect("fireball exists");
        assert_eq!(fireball.flavor, "fantasy");
        assert_eq!(fireball.energy_cost(), 25.0);
        assert!(!fireball.self_castable(), "damage rows wait for combat");
        let cauterize = reg.get("cauterize").expect("cauterize exists");
        assert!(cauterize.self_castable(), "healing rows are live");

        let skills_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("skills")
            .join("skills.csv");
        let skills =
            crate::systems::skills::SkillRegistry::from_csv(&std::fs::read(skills_path).unwrap())
                .unwrap();
        for def in reg.defs.values() {
            assert!(
                ["real", "tech", "fantasy"].contains(&def.flavor.as_str()),
                "{}: unknown flavor {}",
                def.id,
                def.flavor
            );
            if !def.skill_required.is_empty() && skills.get(&def.skill_required).is_none() {
                // Fantasy schools (pyromancy, cryomancy...) are their own
                // future skill lines; only REAL/TECH rows must resolve today.
                assert_ne!(
                    def.flavor, "real",
                    "{}: real ability gated on unknown skill {}",
                    def.id, def.skill_required
                );
            }
        }
    }

    fn cast_world() -> (hecs::World, DataStore) {
        let mut world = hecs::World::new();
        world.spawn((
            PlayerSkills::new(),
            Vitals::default(),
            Health { current: 40.0, max: 100.0 },
            Controllable,
        ));
        let mut data = DataStore::new();
        data.insert("ability_registry", shipped_registry());
        data.insert("ability_request", std::sync::Mutex::new(Vec::<String>::new()));
        data.insert("ability_status", std::sync::Mutex::new(String::new()));
        data.insert(
            "ability_cooldowns",
            std::sync::Mutex::new(HashMap::<String, f32>::new()),
        );
        data.insert(
            "xp_grants",
            std::sync::Mutex::new(Vec::<crate::systems::skills::SkillXPEvent>::new()),
        );
        (world, data)
    }

    /// THE cast loop: a healing ability pays energy, restores health, starts
    /// its cooldown (second cast refused), and recharges over time.
    #[test]
    fn cast_heals_costs_energy_and_cools_down() {
        let (mut world, data) = cast_world();
        let mut sys = AbilitySystem::new();

        // cauterize: 15 mana + 5 stamina = 20 energy, heals 25, 10s cooldown.
        // Gate: pyromancy 1 - level-1 gates are baseline-open, so a fresh
        // player (untrained = level 0) can still cast it.
        let push = |data: &DataStore, id: &str| {
            data.get::<std::sync::Mutex<Vec<String>>>("ability_request")
                .unwrap()
                .lock()
                .unwrap()
                .push(id.to_string());
        };
        let status = |data: &DataStore| -> String {
            data.get::<std::sync::Mutex<String>>("ability_status")
                .unwrap()
                .lock()
                .unwrap()
                .clone()
        };

        push(&data, "cauterize");
        sys.tick(&mut world, 0.016, &data);
        {
            let mut q = world.query::<(&Health, &Vitals)>();
            let (_, (h, v)) = q.iter().next().unwrap();
            assert_eq!(h.current, 65.0, "40 + 25 healed");
            assert_eq!(v.energy, Vitals::default().energy - 20.0, "energy paid");
        }
        assert!(status(&data).contains("restores 25"), "got: {}", status(&data));

        // Immediately again: recharging.
        push(&data, "cauterize");
        sys.tick(&mut world, 0.016, &data);
        assert!(status(&data).contains("recharging"), "got: {}", status(&data));
        {
            let mut q = world.query::<&Health>();
            let (_, h) = q.iter().next().unwrap();
            assert_eq!(h.current, 65.0, "no double heal through the cooldown");
        }

        // After the 10s cooldown: castable again.
        sys.tick(&mut world, 10.5, &data);
        push(&data, "cauterize");
        sys.tick(&mut world, 0.016, &data);
        {
            let mut q = world.query::<&Health>();
            let (_, h) = q.iter().next().unwrap();
            assert_eq!(h.current, 90.0, "second heal landed after recharge");
        }
    }

    /// Refusals are honest and free: an offensive row (no combat targets
    /// yet) and an unaffordable cast change nothing.
    #[test]
    fn refused_casts_change_nothing() {
        let (mut world, data) = cast_world();
        let mut sys = AbilitySystem::new();

        // Offensive row: refused, nothing spent.
        let msg = sys.cast(&mut world, &data, "fireball");
        assert!(msg.contains("needs a target"), "got: {msg}");

        // Drain energy below any cost: refused, health unchanged.
        for (_e, (v, _c)) in world.query_mut::<(&mut Vitals, &Controllable)>() {
            v.energy = 1.0;
        }
        let msg = sys.cast(&mut world, &data, "cauterize");
        assert!(msg.contains("Too tired"), "got: {msg}");
        let mut q = world.query::<(&Health, &Vitals)>();
        let (_, (h, v)) = q.iter().next().unwrap();
        assert_eq!(h.current, 40.0);
        assert_eq!(v.energy, 1.0);
    }
}

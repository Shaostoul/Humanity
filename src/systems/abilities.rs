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
//!
//! BUILDING ABILITIES (BUG-153, 2026-10-05): a row whose `builds` column names
//! a blueprint builds that piece in front of the caster, through the one
//! build path a piece placed by hand takes (`construction::begin_build`), at
//! the spot the engine found where the player stands and looks
//! ([`BUILD_SPOT_SLOT`]). The Campfire builds a campfire; it used to heal 3
//! health and build nothing. A build that is refused (aboard the ship, under
//! a roof, too few materials, nowhere to stand) spends nothing.

use crate::ecs::components::{Controllable, Health, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::construction::BuildRequest;
use crate::systems::skills::PlayerSkills;
use serde::Deserialize;
use std::collections::HashMap;

/// Where a building ability builds (BUG-153, 2026-10-05): the ability's id
/// and the build request at the spot in front of the player, or why there is
/// no spot from where they stand (open space, flying, in a vehicle, on the
/// sea, not in first person). Only the engine knows where the player stands
/// and looks (aboard, in the home frame; on a planet, in the build site
/// under the crosshair), so it fills this for the cast waiting to go, from
/// the same ghost a piece in hand is placed by
/// (`engine::build_place::publish_cast_spot`).
pub type BuildSpot = (String, Result<BuildRequest, String>);

/// The DataStore slot holding the [`BuildSpot`] for the cast waiting to go:
/// a `Mutex<Option<BuildSpot>>`, taken by the cast.
pub const BUILD_SPOT_SLOT: &str = "ability_build_spot";

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
    /// The blueprint this ability builds in front of the caster (BUG-153:
    /// the Campfire builds `campfire`), or None. The `builds` column, last in
    /// data/abilities.csv and empty on every other row: an Option, because a
    /// row that stops before a trailing column reads as None only for an
    /// Option (the CSV reader is flexible about row length).
    #[serde(default)]
    pub builds: Option<String>,
}

impl AbilityDef {
    /// Total activation cost, paid from the energy vital (mana and stamina
    /// both draw from energy until a separate stamina vital exists).
    pub fn energy_cost(&self) -> f32 {
        self.mana_cost + self.stamina_cost
    }

    /// Does this row do anything in the v1 self-scoped pipeline? Healing
    /// abilities are live, and so are building ones (BUG-153); damage rows
    /// wait for the combat arc's targets.
    pub fn self_castable(&self) -> bool {
        self.healing_base > 0.0 || self.builds.is_some()
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

        // Drain this frame's cast requests: (ability id, optional target
        // entity bits - the creature the caster faces, v0.760).
        let requests: Vec<(String, Option<u64>)> = data
            .get::<std::sync::Mutex<Vec<(String, Option<u64>)>>>("ability_request")
            .and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v)))
            .unwrap_or_default();

        if !requests.is_empty() {
            let mut status = String::new();
            for (id, target) in requests {
                status = self.cast(world, data, &id, target);
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

/// Map an abilities.csv damage_type string onto the combat system's damage
/// categories (Armor.resistance keys). Fantasy elements collapse into the
/// physical categories they behave like; "true" damage picks kinetic, which
/// creatures carry no resistance against today.
fn combat_damage_type(s: &str) -> crate::systems::combat::damage::DamageType {
    use crate::systems::combat::damage::DamageType as D;
    match s {
        "fire" | "ice" => D::Thermal,
        "lightning" | "psychic" | "holy" | "shadow" => D::Energy,
        "poison" => D::Chemical,
        _ => D::Kinetic, // physical, true, none, unknown
    }
}

impl AbilitySystem {
    /// Validate + apply one cast against the player. Returns the status line.
    fn cast(
        &mut self,
        world: &mut hecs::World,
        data: &DataStore,
        id: &str,
        target: Option<u64>,
    ) -> String {
        let Some(reg) = data.get::<AbilityRegistry>("ability_registry") else {
            return "Abilities are still loading".to_string();
        };
        let Some(def) = reg.get(id) else {
            return format!("Unknown ability {id}");
        };
        let offensive = def.damage_base > 0.0 && !def.self_castable();
        if !def.self_castable() && !offensive {
            return format!("{} does nothing yet - a later arc wires it", def.name);
        }
        if let Some(t) = self.cooldowns.get(id) {
            return format!("{} recharging ({:.0}s)", def.name, t.max(1.0));
        }
        // A building ability builds where the engine said (BUG-153).
        if let Some(blueprint) = def.builds.as_deref() {
            return self.cast_build(world, data, def, blueprint);
        }

        // Offensive casts need a living target within range (v0.760).
        let mut target_entity: Option<hecs::Entity> = None;
        let mut target_name = String::new();
        if offensive {
            let Some(bits) = target else {
                return format!("{} needs a target - face a creature", def.name);
            };
            let Some(e) = hecs::Entity::from_bits(bits) else {
                return format!("{} needs a target - face a creature", def.name);
            };
            if !world.contains(e)
                || world.get::<&crate::ecs::components::Dead>(e).is_ok()
                || world.get::<&Health>(e).is_err()
            {
                return format!("{}: that target is gone", def.name);
            }
            // Range gate: caster position vs target position, with a small
            // tolerance since the player Transform trails the camera.
            if def.range_m > 0.0 {
                let caster_pos = world
                    .query_mut::<(&crate::ecs::components::Transform, &Controllable)>()
                    .into_iter()
                    .next()
                    .map(|(_e, (t, _c))| t.position);
                if let (Some(cp), Ok(tt)) = (
                    caster_pos,
                    world.get::<&crate::ecs::components::Transform>(e),
                ) {
                    if (tt.position - cp).length() > def.range_m + 2.0 {
                        return format!("{}: out of range ({:.0}m)", def.name, def.range_m);
                    }
                }
            }
            target_name = world
                .get::<&crate::ecs::components::Name>(e)
                .map(|n| n.0.clone())
                .unwrap_or_else(|_| "the target".to_string());
            target_entity = Some(e);
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
            if !offensive {
                let healed = def
                    .healing_base
                    .min((health.max - health.current).max(0.0));
                health.current = (health.current + def.healing_base).min(health.max);
                self.cooldowns.insert(id.to_string(), def.cooldown_s);
                if !def.skill_required.is_empty() {
                    crate::systems::skills::award_skill_xp(data, &def.skill_required, 5);
                }
                return if healed > 0.0 {
                    format!("{} restores {healed:.0} health", def.name)
                } else {
                    format!("{} cast (already at full health)", def.name)
                };
            }
            // Offensive: queue the hit for CombatSystem (armor mitigation +
            // death + loot all live there - one damage pipeline).
            self.cooldowns.insert(id.to_string(), def.cooldown_s);
            if !def.skill_required.is_empty() {
                crate::systems::skills::award_skill_xp(data, &def.skill_required, 5);
            }
            if let (Some(e), Some(chan)) = (
                target_entity,
                data.get::<std::sync::Mutex<
                    Vec<(u64, crate::systems::combat::damage::DamageEvent)>,
                >>("damage_events"),
            ) {
                if let Ok(mut q) = chan.lock() {
                    q.push((
                        e.to_bits().into(),
                        crate::systems::combat::damage::DamageEvent {
                            damage_type: combat_damage_type(&def.damage_type),
                            amount: def.damage_base,
                            source_name: None,
                            source_is_player: true,
                        },
                    ));
                }
            }
            return format!("{} hits {} for {:.0}", def.name, target_name, def.damage_base);
        }
        "No caster in the world yet".to_string()
    }

    /// A building ability's cast (BUG-153, 2026-10-05): build `blueprint` at
    /// the spot the engine found for this cast ([`BUILD_SPOT_SLOT`]), through
    /// `construction::begin_build`, the path a piece placed by hand takes.
    /// The skill gate and the energy are checked first and the spot is taken
    /// either way (it belongs to this one cast); the energy is paid, the
    /// cooldown started and the skill trained only when the build starts, so
    /// a refusal (aboard, under a roof, too few materials, nowhere to stand)
    /// costs nothing and says why.
    fn cast_build(&mut self, world: &mut hecs::World, data: &DataStore, def: &AbilityDef, blueprint: &str) -> String {
        let spot = data
            .get::<std::sync::Mutex<Option<BuildSpot>>>(BUILD_SPOT_SLOT)
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()))
            .filter(|(id, _)| *id == def.id);
        let caster = world
            .query::<(&PlayerSkills, &Vitals, &Controllable)>()
            .iter()
            .next()
            .map(|(e, (skills, vitals, _c))| (e, def.skill_gate_met(skills), vitals.energy));
        let Some((caster, gate_met, energy)) = caster else {
            return "No caster in the world yet".to_string();
        };
        if !gate_met {
            return format!("{} needs {} level {}", def.name, def.skill_required, def.skill_level);
        }
        let cost = def.energy_cost();
        if energy < cost {
            return format!("Too tired to cast {} ({cost:.0} energy)", def.name);
        }
        let mut request = match spot {
            None => return format!("{}: stand on the ground in first person to build it", def.name),
            Some((_, Err(why))) => return format!("{}: {why}", def.name),
            Some((_, Ok(request))) => request,
        };
        // The row says what is built; the engine says where.
        request.blueprint_id = blueprint.to_string();
        match crate::systems::construction::begin_build(world, data, request) {
            Err(refused) => refused,
            Ok(started) => {
                if let Ok(mut v) = world.get::<&mut Vitals>(caster) {
                    v.energy -= cost;
                }
                self.cooldowns.insert(def.id.clone(), def.cooldown_s);
                if !def.skill_required.is_empty() {
                    crate::systems::skills::award_skill_xp(data, &def.skill_required, 5);
                }
                started
            }
        }
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
        assert!(!fireball.self_castable(), "damage rows target creatures");
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
            crate::ecs::components::Transform::default(),
        ));
        let mut data = DataStore::new();
        data.insert("ability_registry", shipped_registry());
        data.insert(
            "ability_request",
            std::sync::Mutex::new(Vec::<(String, Option<u64>)>::new()),
        );
        data.insert("ability_status", std::sync::Mutex::new(String::new()));
        data.insert(
            "ability_cooldowns",
            std::sync::Mutex::new(HashMap::<String, f32>::new()),
        );
        data.insert(
            "xp_grants",
            std::sync::Mutex::new(Vec::<crate::systems::skills::SkillXPEvent>::new()),
        );
        data.insert(
            "damage_events",
            std::sync::Mutex::new(
                Vec::<(u64, crate::systems::combat::damage::DamageEvent)>::new(),
            ),
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
            data.get::<std::sync::Mutex<Vec<(String, Option<u64>)>>>("ability_request")
                .unwrap()
                .lock()
                .unwrap()
                .push((id.to_string(), None));
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

    /// Refusals are honest and free: an offensive row without a target and
    /// an unaffordable cast change nothing.
    #[test]
    fn refused_casts_change_nothing() {
        let (mut world, data) = cast_world();
        let mut sys = AbilitySystem::new();

        // Offensive row with no target: refused, nothing spent.
        let msg = sys.cast(&mut world, &data, "fireball", None);
        assert!(msg.contains("needs a target"), "got: {msg}");

        // Drain energy below any cost: refused, health unchanged.
        for (_e, (v, _c)) in world.query_mut::<(&mut Vitals, &Controllable)>() {
            v.energy = 1.0;
        }
        let msg = sys.cast(&mut world, &data, "cauterize", None);
        assert!(msg.contains("Too tired"), "got: {msg}");
        let mut q = world.query::<(&Health, &Vitals)>();
        let (_, (h, v)) = q.iter().next().unwrap();
        assert_eq!(h.current, 40.0);
        assert_eq!(v.energy, 1.0);
    }

    /// The attack path (v0.760): an offensive cast at a living creature pays
    /// energy, queues the hit on the damage_events channel for CombatSystem,
    /// and range/dead gates refuse honestly.
    #[test]
    fn offensive_cast_queues_damage_at_a_target() {
        use crate::ecs::components::{Name, Transform};
        use glam::Vec3;

        let (mut world, data) = cast_world();
        let mut sys = AbilitySystem::new();

        // A chicken 5m away (ember_shot: 18 kinetic-mapped fire, range 25m).
        let hen = world.spawn((
            Name("Chicken".to_string()),
            Health { current: 15.0, max: 15.0 },
            Transform {
                position: Vec3::new(5.0, 0.0, 0.0),
                ..Default::default()
            },
        ));

        let msg = sys.cast(&mut world, &data, "ember_shot", Some(hen.to_bits().into()));
        assert!(msg.contains("hits Chicken for 18"), "got: {msg}");
        let queued = data
            .get::<std::sync::Mutex<Vec<(u64, crate::systems::combat::damage::DamageEvent)>>>(
                "damage_events",
            )
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, u64::from(hen.to_bits()));
        assert_eq!(queued[0].1.amount, 18.0);

        // Out of range: a far target refuses without spending.
        sys.tick(&mut world, 5.0, &data); // clear ember_shot's 1s cooldown
        {
            let mut t = world.get::<&mut Transform>(hen).unwrap();
            t.position = Vec3::new(100.0, 0.0, 0.0);
        }
        let msg = sys.cast(&mut world, &data, "ember_shot", Some(hen.to_bits().into()));
        assert!(msg.contains("out of range"), "got: {msg}");

        // A dead target refuses.
        world.insert_one(hen, crate::ecs::components::Dead::default()).unwrap();
        let msg = sys.cast(&mut world, &data, "ember_shot", Some(hen.to_bits().into()));
        assert!(msg.contains("gone"), "got: {msg}");
    }

    // ── BUG-153: the Campfire ability builds a campfire ────────────────
    //
    // The DataStore slot the engine fills with where a building ability
    // builds (`BUILD_SPOT_SLOT`): the ability's id and the build request at
    // the spot in front of the player, or why there is no spot. Spelled out
    // as the plain tuple here so these tests compile against the code from
    // before the fix, where they were seen failing (the cast healed 3).

    const STONE: &str = "stone_raw_0";
    const LOG: &str = "wood_log_0";

    /// A player holding `stones` Raw Stone and `logs` Wood Logs, with the
    /// blueprint catalog and the build channels a cast needs, standing on
    /// Earth (breathable air: a fire can burn).
    fn campfire_world(stones: u32, logs: u32) -> (hecs::World, DataStore, hecs::Entity) {
        use crate::systems::inventory::Inventory;
        let (mut world, mut data) = cast_world();
        let player = world.query::<&Controllable>().iter().next().map(|(e, _)| e).unwrap();
        let mut inv = Inventory::new(16);
        if stones > 0 {
            inv.add_item(STONE, stones, 99);
        }
        if logs > 0 {
            inv.add_item(LOG, logs, 99);
        }
        world.insert_one(player, inv).unwrap();
        let bp = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("blueprints").join("basic.ron");
        data.insert(
            "blueprint_registry",
            crate::systems::construction::BlueprintRegistry::from_ron(&std::fs::read(bp).unwrap()).unwrap(),
        );
        data.insert("build_request", std::sync::Mutex::new(Vec::<crate::systems::construction::BuildRequest>::new()));
        data.insert("build_status", std::sync::Mutex::new(String::new()));
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        data.insert(
            "item_registry",
            crate::systems::inventory::ItemRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/items.csv"))).unwrap(),
        );
        data.insert(
            "body_environment",
            crate::systems::body_environment::BodyEnvironment { locked: true, ..Default::default() },
        );
        data.insert(
            "ability_build_spot",
            std::sync::Mutex::new(None::<(String, Result<crate::systems::construction::BuildRequest, String>)>),
        );
        (world, data, player)
    }

    /// A build site on Earth's ground.
    fn earth_site() -> crate::systems::construction::PlanetSite {
        crate::systems::construction::PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6_371_000.0, 0.0) }
    }

    /// Where the engine says the campfire goes: 2 m ahead on the ground of
    /// `site` (None = aboard, in the home frame).
    fn put_spot(data: &DataStore, site: Option<crate::systems::construction::PlanetSite>) {
        use crate::systems::construction::BuildRequest;
        let pose = crate::ecs::components::Transform {
            position: glam::Vec3::new(0.0, 0.0, -2.0),
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::new(1.0, 0.6, 1.0),
        };
        let spot: (String, Result<BuildRequest, String>) = ("campfire".to_string(), Ok(BuildRequest::new("campfire", pose).on(site)));
        *data
            .get::<std::sync::Mutex<Option<(String, Result<BuildRequest, String>)>>>("ability_build_spot")
            .unwrap()
            .lock()
            .unwrap() = Some(spot);
    }

    fn cast_status(sys: &mut AbilitySystem, world: &mut hecs::World, data: &DataStore) -> String {
        data.get::<std::sync::Mutex<Vec<(String, Option<u64>)>>>("ability_request")
            .unwrap()
            .lock()
            .unwrap()
            .push(("campfire".to_string(), None));
        sys.tick(world, 0.016, data);
        data.get::<std::sync::Mutex<String>>("ability_status").unwrap().lock().unwrap().clone()
    }

    /// What the player has: energy, health, Raw Stone and Wood Logs carried.
    fn holdings(world: &hecs::World, player: hecs::Entity) -> (f32, f32, u32, u32) {
        use crate::systems::inventory::Inventory;
        let v = world.get::<&Vitals>(player).unwrap().energy;
        let h = world.get::<&Health>(player).unwrap().current;
        let inv = world.get::<&Inventory>(player).unwrap();
        (v, h, inv.count_item(STONE), inv.count_item(LOG))
    }

    /// BUG-153. CASTING CAMPFIRE OUTDOORS BUILDS A CAMPFIRE FROM THE PACK.
    /// Standing on Earth's ground with 6 Raw Stone and 3 Wood Logs, the cast
    /// starts the campfire where the engine said (its site, its pose), takes
    /// the stones and the logs, costs its 15 energy, and heals nothing; the
    /// scaffold finishes into a finished campfire. Seen red 2026-10-05 on the
    /// code before the fix: the cast restored 3 health and built nothing
    /// ("Campfire restores 3 health").
    #[test]
    fn the_campfire_ability_builds_a_campfire_outdoors_from_the_pack() {
        use crate::systems::construction::{Construction, ConstructionSystem, PlanetSite, Structure};
        use crate::ecs::components::Transform;
        let (mut world, data, player) = campfire_world(6, 3);
        put_spot(&data, Some(earth_site()));
        let mut sys = AbilitySystem::new();
        let status = cast_status(&mut sys, &mut world, &data);
        assert!(status.contains("Campfire") && !status.contains("restores"), "got: {status}");
        let started: Vec<(String, Transform, PlanetSite)> = world
            .query::<(&Construction, &Transform, &PlanetSite)>()
            .iter()
            .map(|(_e, (c, t, s))| (c.blueprint_id.clone(), t.clone(), s.clone()))
            .collect();
        assert_eq!(started.len(), 1, "one campfire going up: {status}");
        assert_eq!(started[0].0, "campfire");
        assert_eq!(started[0].1.position, glam::Vec3::new(0.0, 0.0, -2.0), "where the engine said");
        assert_eq!(started[0].2, earth_site(), "in the site on the ground");
        let (energy, health, stones, logs) = holdings(&world, player);
        assert_eq!((stones, logs), (0, 0), "the ring's stones and its logs came out of the pack");
        assert_eq!(energy, Vitals::default().energy - 15.0, "15 energy to build it");
        assert_eq!(health, 40.0, "a fire heals nothing");

        // The scaffold finishes into a campfire.
        let mut build = ConstructionSystem::new();
        build.tick(&mut world, 60.0, &data);
        let fires: Vec<String> = world.query::<&Structure>().iter().map(|(_e, s)| s.blueprint_id.clone()).collect();
        assert_eq!(fires, vec!["campfire".to_string()], "a finished campfire");

        // The description names what it takes, by the blueprint's own numbers
        // and the items' own names, so the two cannot drift apart.
        let desc = shipped_registry().get("campfire").unwrap().description.clone();
        let blueprints = data.get::<crate::systems::construction::BlueprintRegistry>("blueprint_registry").unwrap();
        let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry").unwrap();
        for (id, qty) in &blueprints.get("campfire").unwrap().materials {
            let named = format!("{qty} {}", items.items[id].name);
            assert!(desc.contains(&named), "the description says {named}: {desc}");
        }
        assert!(desc.contains("outdoors") && !desc.contains("heal"), "{desc}");
    }

    /// BUG-153. A CAMPFIRE CAST THAT CANNOT BUILD SPENDS NOTHING. With no
    /// stones or logs in the pack; aboard the ship (the home frame, indoors);
    /// and under a built roof on a planet: each is refused with the reason,
    /// and no energy, health, stone or log changes, and no cooldown starts
    /// (the same cast succeeds the moment it can). Seen red 2026-10-05 on the
    /// code before the fix: each cast spent 15 energy and restored 3 health.
    #[test]
    fn a_campfire_cast_that_cannot_build_spends_nothing() {
        use crate::systems::construction::{placement, BlueprintRegistry, Construction, Structure};
        // Nothing in the pack, outdoors.
        let (mut world, data, player) = campfire_world(0, 0);
        put_spot(&data, Some(earth_site()));
        let mut sys = AbilitySystem::new();
        let before = holdings(&world, player);
        let status = cast_status(&mut sys, &mut world, &data);
        assert!(status.contains("Raw Stone") && status.contains("Wood Log"), "names what is missing: {status}");
        assert_eq!(holdings(&world, player), before, "nothing spent: {status}");
        assert_eq!(world.query::<&Construction>().iter().count(), 0);

        // Aboard the ship: the spot is in the home frame.
        let (mut world, data, player) = campfire_world(6, 3);
        put_spot(&data, None);
        let mut sys = AbilitySystem::new();
        let before = holdings(&world, player);
        let status = cast_status(&mut sys, &mut world, &data);
        assert!(status.contains("outdoors") && status.contains("aboard"), "says why: {status}");
        assert_eq!(holdings(&world, player), before, "nothing spent aboard: {status}");
        assert_eq!(world.query::<&Construction>().iter().count(), 0);
        // No cooldown started: on the ground the same cast builds at once.
        put_spot(&data, Some(earth_site()));
        let status = cast_status(&mut sys, &mut world, &data);
        assert_eq!(world.query::<&Construction>().iter().count(), 1, "the refusal started no cooldown: {status}");

        // Indoors on a planet: under a roof on three walls.
        let (mut world, data, player) = campfire_world(6, 3);
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let site = earth_site();
        for (id, x, z, turns) in [("wood_wall", 0.0, -4.0, 0), ("wood_wall", -2.0, -2.0, 1), ("wood_wall", 2.0, -2.0, 1), ("roof", 0.0, -2.0, 0)] {
            let bp = reg.get(id).unwrap();
            let tf = placement::placement_pose(bp, glam::Vec3::new(x, 0.0, z), turns, &world, &reg, Some(&site));
            world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 }, site.clone()));
        }
        put_spot(&data, Some(site));
        let mut sys = AbilitySystem::new();
        let before = holdings(&world, player);
        let status = cast_status(&mut sys, &mut world, &data);
        assert!(status.contains("roof"), "says why: {status}");
        assert_eq!(holdings(&world, player), before, "nothing spent under a roof: {status}");
        assert_eq!(world.query::<&Construction>().iter().count(), 0);
    }
}

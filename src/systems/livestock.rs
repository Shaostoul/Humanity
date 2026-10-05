//! Livestock system (v0.751, closure ladder rung 7 - creatures, passive first).
//!
//! data/creatures.csv (92 species) finally gets its loader. This module keeps
//! deliberately to the PASSIVE half: farm animals that wander near the fields
//! and yield a renewable product (egg, milk, wool) on walk-up + E, tracked by
//! the previously-unconsumed Harvestable component. Hostile spawning, combat,
//! and taming build on this same registry later.
//!
//! Placement comes from data/entities/livestock.ron (which animals, how many,
//! near which home machine); what an animal yields comes from the creature's
//! renewable_product column. Adding a species = a CSV row; placing it at the
//! homestead = a RON row. No code changes.

use crate::ecs::components::{Creature, Harvestable, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use glam::{Quat, Vec3};
use serde::Deserialize;
use std::collections::HashMap;

// ── Creature definitions (data/creatures.csv) ──────────────────────

/// One creatures.csv row. Columns the engine does not consume yet (loot_table,
/// habitat_biomes, spawn_weight) still parse so later systems read the same
/// registry instead of re-parsing the file.
#[derive(Debug, Clone, Deserialize)]
pub struct CreatureDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub species: String,
    #[serde(default)]
    pub health_base: f32,
    #[serde(default)]
    pub stamina_base: f32,
    #[serde(default)]
    pub mana_base: f32,
    #[serde(default)]
    pub size_category: String,
    #[serde(default)]
    pub weight_kg: f32,
    #[serde(default)]
    pub movement_speed: f32,
    #[serde(default)]
    pub movement_types: String,
    #[serde(default)]
    pub diet: String,
    #[serde(default)]
    pub hostility: String,
    #[serde(default)]
    pub habitat_biomes: String,
    #[serde(default)]
    pub loot_table: String,
    #[serde(default)]
    pub ai_behavior: String,
    #[serde(default)]
    pub domesticable: String,
    #[serde(default)]
    pub spawn_weight: u32,
    #[serde(default)]
    pub description: String,
    /// `item_id:amount:regrow_seconds` collected from the LIVING animal on a
    /// cooldown; empty for species with no renewable yield.
    #[serde(default)]
    pub renewable_product: String,
}

/// What a living animal yields on a cooldown, parsed from renewable_product.
#[derive(Debug, Clone, PartialEq)]
pub struct RenewableProduct {
    pub item: String,
    pub amount: u32,
    pub regrow_s: f32,
}

impl CreatureDef {
    /// Parse the renewable_product column (`egg_0:1:300`). None when the
    /// column is empty or malformed - a bad row loses its yield, not the game.
    pub fn renewable(&self) -> Option<RenewableProduct> {
        let mut parts = self.renewable_product.split(':');
        let item = parts.next().filter(|s| !s.is_empty())?.to_string();
        let amount = parts.next()?.parse().ok()?;
        let regrow_s = parts.next()?.parse().ok()?;
        Some(RenewableProduct {
            item,
            amount,
            regrow_s,
        })
    }

    /// Placeholder body-box side length (metres) from the species' mass at
    /// roughly water density: chicken ~0.15, sheep ~0.43, cow ~0.89. Clamped
    /// so a beetle is still visible and a whale still fits on screen.
    pub fn body_side(&self) -> f32 {
        (self.weight_kg.max(0.1) / 1000.0).cbrt().clamp(0.12, 1.2)
    }

    /// Parse the loot_table column (`raw_poultry:100:1:2|feather:80:1:3`)
    /// into LootTable entries: (item id, chance 0..1, min, max). Item ids in
    /// creatures.csv predate the `_0` suffix convention, so each resolves
    /// against items.csv: exact id first, then `{id}_0`. Malformed segments
    /// are skipped (a bad row loses a drop, not the game). (v0.760)
    pub fn loot_entries(
        &self,
        items: Option<&crate::systems::inventory::ItemRegistry>,
    ) -> Vec<(String, f32, u32, u32)> {
        self.loot_table
            .split('|')
            .filter_map(|seg| {
                let mut parts = seg.split(':');
                let raw = parts.next().filter(|s| !s.is_empty())?;
                let chance: f32 = parts.next()?.parse().ok()?;
                let min: u32 = parts.next()?.parse().ok()?;
                let max: u32 = parts.next()?.parse().ok()?;
                let id = match items {
                    Some(reg) if reg.items.contains_key(raw) => raw.to_string(),
                    Some(reg) => {
                        let suffixed = format!("{raw}_0");
                        if reg.items.contains_key(&suffixed) {
                            suffixed
                        } else {
                            raw.to_string()
                        }
                    }
                    None => raw.to_string(),
                };
                Some((id, (chance / 100.0).clamp(0.0, 1.0), min, max.max(min)))
            })
            .collect()
    }
}

/// All creature species keyed by id. DataStore: `"creature_registry"`.
#[derive(Debug, Default)]
pub struct CreatureRegistry {
    pub defs: HashMap<String, CreatureDef>,
}

impl CreatureRegistry {
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<CreatureDef> = crate::assets::loader::parse_csv(data)?;
        let mut defs = HashMap::new();
        for row in rows {
            defs.insert(row.id.clone(), row);
        }
        Ok(Self { defs })
    }

    pub fn get(&self, id: &str) -> Option<&CreatureDef> {
        self.defs.get(id)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

// ── Homestead placement (data/entities/livestock.ron) ──────────────

/// One livestock.ron row: place `count` of a species near a home machine
/// instance (the outdoor fields), scattered within `spread` metres.
#[derive(Debug, Clone, Deserialize)]
pub struct LivestockPlacement {
    pub creature: String,
    pub count: u32,
    /// Machine instance id from data/machines/home.ron to anchor near.
    pub near: String,
    pub spread: f32,
    /// Placeholder body colour until real models land.
    pub tint: (f32, f32, f32),
}

/// The homestead's starter-animal list. DataStore: `"livestock_spawn_list"`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LivestockSpawnList {
    pub animals: Vec<LivestockPlacement>,
}

impl LivestockSpawnList {
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        ron::from_str(text).map_err(|e| e.to_string())
    }
}

// ── Wild spawns (data/entities/wild_spawns.ron, v0.761) ─────────────

/// One wild-creature placement: `count` of a species scattered around an
/// absolute world position (hostiles live away from the homestead).
#[derive(Debug, Clone, Deserialize)]
pub struct WildSpawn {
    pub creature: String,
    pub count: u32,
    /// World-space (x, z) center; y sits at ground level.
    pub pos: (f32, f32),
    pub radius: f32,
    pub tint: (f32, f32, f32),
}

/// The wild-spawn list. DataStore: `"wild_spawn_list"`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WildSpawnList {
    pub spawns: Vec<WildSpawn>,
}

impl WildSpawnList {
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        ron::from_str(text).map_err(|e| e.to_string())
    }
}

/// Spawn a fully-formed creature of `def` into the world at `pos` (v0.777, the
/// dev spawn tool). Gives it the full treatment so it both behaves and can be
/// harvested/fought: Creature + Transform + Name + Health + LootTable +
/// Velocity, plus AIBehavior for NON-passive species only (passive farm
/// animals use the LivestockSystem's anchored amble, exactly like the placed
/// homestead bundle -- see the comment at the insert below), plus Harvestable
/// when the species has a renewable product. Mirrors load_world's homestead +
/// wild bundles (src/lib.rs) so a dev-spawned animal is identical to a placed
/// one -- rendering + walk-up + combat all work with no extra registration.
/// Returns the new entity.
pub fn spawn_creature_at(
    world: &mut hecs::World,
    def: &CreatureDef,
    items: Option<&crate::systems::inventory::ItemRegistry>,
    pos: Vec3,
    tint: [f32; 3],
) -> hecs::Entity {
    use crate::ecs::components::{AIBehavior, Health, LootTable, Name, Velocity};
    let hp = def.health_base.max(1.0);
    let e = world.spawn((
        Creature {
            def_id: def.id.clone(),
            anchor: pos,
            range: 6.0,
            phase: 0.0,
            speed: (def.movement_speed * 0.35).max(0.2),
            tint,
            body_side: def.body_side(),
        },
        Transform {
            position: pos,
            ..Default::default()
        },
        Name(def.name.clone()),
        Health {
            current: hp,
            max: hp,
        },
        LootTable {
            entries: def.loot_entries(items),
        },
        Velocity::default(),
    ));
    // AIBehavior ONLY for genuinely non-passive species (v0.779): the
    // LivestockSystem's anchored graze skips any creature that has AIBehavior
    // ("AI-driven creatures are the AISystem's to move"), and the AISystem's
    // passive tick is an UNANCHORED wander -- so giving a hen AIBehavior made
    // dev-spawned farm animals drift through walls and away from where they
    // were placed, unlike placed ones. Passive species now match the placed
    // homestead bundle exactly (no AIBehavior = anchored amble around `pos`).
    let behavior = behavior_type_for(def);
    if behavior != "passive" {
        let _ = world.insert_one(
            e,
            AIBehavior {
                behavior_type: behavior.to_string(),
                state: "idle".to_string(),
                target: None,
            },
        );
    }
    // Renewable-yield species (hen/goat/sheep) also get the Harvestable so a
    // dev-spawned one supports walk-up + E collection like a placed one.
    if let Some(p) = def.renewable() {
        let _ = world.insert_one(
            e,
            Harvestable {
                resource: p.item,
                amount: p.amount as f32,
                regrow_time: p.regrow_s,
                time_since_harvest: p.regrow_s,
            },
        );
    }
    e
}

/// Map a creatures.csv ai_behavior string onto the AISystem's behavior_type
/// state machine. hunt-class rows become predators (they count the player as
/// prey); ambush/aggressive rows are aggressive; everything else wanders as
/// passive. (v0.761)
pub fn behavior_type_for(def: &CreatureDef) -> &'static str {
    match def.ai_behavior.as_str() {
        "hunt" => "predator",
        "ambush" | "swarm" => "aggressive",
        "guard" | "patrol" => "guard",
        // Rooted forage flora (v0.978, the 2026-07-20 data-agent gap): a
        // berry bush is a Creature row (renewable_product drives the same
        // [E]-collect loop as eggs/milk/wool) that must never move. The
        // AISystem has no "stationary" arm, so it idles at zero velocity,
        // and every spawn path attaches AIBehavior for non-passive types,
        // which exempts it from the LivestockSystem graze amble too.
        "stationary" => "stationary",
        _ => "passive",
    }
}

// ── Harvest ─────────────────────────────────────────────────────────

/// Collect from a Harvestable if its product has regrown: resets the timer and
/// returns how many items to hand over; None while still regrowing. Pure over
/// the component so lib.rs's E-press bridge and the tests share one rule.
pub fn collect(h: &mut Harvestable) -> Option<u32> {
    if h.time_since_harvest + f32::EPSILON < h.regrow_time {
        return None;
    }
    h.time_since_harvest = 0.0;
    Some((h.amount.round() as u32).max(1))
}

// ── The herd across restarts and the time away (2026-09-27) ──────────

/// Which homestead animal this is, stable across restarts: the species and
/// its place among that species' animals in data/entities/livestock.ron, as
/// "chicken#0". The herd is spawned anew from the RON on every world entry,
/// so the entity changes every launch; the save matches on this instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HerdSlot(pub String);

/// The slot of the `n`th animal of `creature` in the spawn list.
pub fn herd_slot(creature: &str, n: u32) -> String {
    format!("{creature}#{n}")
}

/// DataStore key: saved herd timers waiting for the herd to be spawned
/// (world entry comes after the save is applied at startup).
pub const HERD_RESTORE: &str = "livestock_herd_restore";

/// Put the herd's restore channel in the DataStore (lib.rs, at boot).
pub fn register(store: &mut DataStore) {
    store.insert(HERD_RESTORE, std::sync::Mutex::new(Option::<Vec<(String, f32)>>::None));
}

/// Each living homestead animal's yield timer, (slot, seconds since it was
/// last collected from), for the save. Dead animals are left out.
pub fn herd_timers(world: &hecs::World) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = world
        .query::<(&HerdSlot, &Harvestable, Option<&crate::ecs::components::Dead>)>()
        .iter()
        .filter(|(_, (_, _, dead))| dead.is_none())
        .map(|(_, (slot, h, _))| (slot.0.clone(), h.time_since_harvest))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Give the herd its saved timers, each capped at ready: an animal holds one
/// yield until it is collected, away or not. Returns how many animals it matched.
pub fn apply_herd_timers(world: &mut hecs::World, timers: &[(String, f32)]) -> usize {
    let mut matched = 0;
    for (_e, (slot, h)) in world.query_mut::<(&HerdSlot, &mut Harvestable)>() {
        if let Some((_, t)) = timers.iter().find(|(s, _)| *s == slot.0) {
            h.time_since_harvest = t.clamp(0.0, h.regrow_time);
            matched += 1;
        }
    }
    matched
}

/// The saved timers after `away_secs` of time away (offline progression,
/// docs/design/offline-progression.md): an animal's egg, milk or wool keeps
/// coming while the player is out, exactly as it would while they stood in
/// the yard and never collected, so the same one-yield cap applies (it is
/// applied with the animal's regrow time in `apply_herd_timers`). Nothing
/// here can make an animal hungry, ill or dead: the livestock system has no
/// such state today, and when it gets one it belongs to the doc's "must NOT
/// advance offline" class, kept by the character's upkeep.
pub fn timers_after_away(timers: &[(String, f32)], away_secs: f64) -> Vec<(String, f32)> {
    timers
        .iter()
        .map(|(s, t)| (s.clone(), (f64::from(*t) + away_secs.max(0.0)).min(f64::from(f32::MAX)) as f32))
        .collect()
}

/// How many of the saved animals were still regrowing when the player left
/// and are ready again after `away_secs`: the "while you were away" count.
/// The species' regrow time comes from the creature registry by the slot's
/// species half.
pub fn readied_by_away(timers: &[(String, f32)], away_secs: f64, reg: &CreatureRegistry) -> usize {
    timers
        .iter()
        .filter(|(slot, t)| {
            let species = slot.split('#').next().unwrap_or_default();
            reg.get(species).and_then(|d| d.renewable()).is_some_and(|p| {
                f64::from(*t) < f64::from(p.regrow_s) && f64::from(*t) + away_secs >= f64::from(p.regrow_s)
            })
        })
        .count()
}

/// Put saved timers on the herd: at once when the herd is already in the
/// world (a character select), else held for world entry to spawn it with
/// (`take_pending_herd`). Replaces whatever was waiting.
pub fn restore_herd(world: &mut hecs::World, data: &DataStore, timers: Vec<(String, f32)>) {
    if world.query::<&HerdSlot>().iter().next().is_some() {
        apply_herd_timers(world, &timers);
        set_pending_herd(data, None);
    } else {
        set_pending_herd(data, Some(timers));
    }
}

fn set_pending_herd(data: &DataStore, timers: Option<Vec<(String, f32)>>) {
    if let Some(Ok(mut v)) = data.get::<std::sync::Mutex<Option<Vec<(String, f32)>>>>(HERD_RESTORE).map(|m| m.lock()) {
        *v = timers;
    }
}

/// The saved timers still waiting for the herd, if any, without taking them
/// (a save written before world entry keeps them rather than forgetting).
pub fn pending_herd(data: &DataStore) -> Option<Vec<(String, f32)>> {
    data.get::<std::sync::Mutex<Option<Vec<(String, f32)>>>>(HERD_RESTORE)
        .and_then(|m| m.lock().ok().and_then(|v| v.clone()))
}

/// Take the saved timers for the herd being spawned (world entry).
pub fn take_pending_herd(data: &DataStore) -> Option<Vec<(String, f32)>> {
    data.get::<std::sync::Mutex<Option<Vec<(String, f32)>>>>(HERD_RESTORE)
        .and_then(|m| m.lock().ok().and_then(|mut v| v.take()))
}

// ── The system ──────────────────────────────────────────────────────

/// Ages every Harvestable toward ready and ambles Creature entities around
/// their anchors on a per-animal lissajous graze path (deterministic, no
/// physics, no lockstep thanks to the phase offset).
pub struct LivestockSystem {
    /// Accumulated sim time driving the graze paths.
    t: f32,
}

impl LivestockSystem {
    pub fn new() -> Self {
        Self { t: 0.0 }
    }
}

impl Default for LivestockSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for LivestockSystem {
    fn name(&self) -> &str {
        "LivestockSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        self.t += dt;
        // Yields ripen on the GAME clock (2026-09-27), like crafting and the
        // power grid: a night slept at 120x ripens a night's eggs. Movement
        // below stays on real dt, so a herd never sprints while you sleep.
        let game_dt = crate::systems::time::scaled_dt(dt, data);

        // Regrowth: every Harvestable in the world ages toward ready (animals
        // today; wild berry bushes ride the same pass when they land). Clamped
        // at ready so the float never grows unbounded across long sessions.
        // Dead animals stop producing (v0.760).
        for (_e, (h, dead)) in
            world.query_mut::<(&mut Harvestable, Option<&crate::ecs::components::Dead>)>()
        {
            if dead.is_some() {
                continue;
            }
            if h.time_since_harvest < h.regrow_time {
                h.time_since_harvest = (h.time_since_harvest + game_dt).min(h.regrow_time);
            }
        }

        // Predator positions this frame, for the flee override below
        // (v0.762): a living hunt-class creature nearby spooks the herd.
        let threats: Vec<Vec3> = world
            .query::<(
                &crate::ecs::components::AIBehavior,
                &Transform,
                Option<&crate::ecs::components::Dead>,
            )>()
            .iter()
            .filter(|(_, (ai, _, dead))| {
                dead.is_none()
                    && matches!(ai.behavior_type.as_str(), "predator" | "aggressive")
            })
            .map(|(_, (_, tf, _))| tf.position)
            .collect();

        // Graze amble: ease each animal toward a slowly-orbiting target around
        // its anchor. The two incommensurate frequencies trace a lissajous
        // loop, so the herd drifts naturally instead of circling. The dead
        // stay where they fell (v0.760); AI-driven creatures (hostiles) are
        // the AISystem's to move, not this amble's (v0.761). A predator
        // within FLEE_RADIUS overrides the amble: run directly away at
        // double speed - no more grazing while being eaten (v0.762).
        const FLEE_RADIUS: f32 = 12.0;
        for (_e, (c, tf, dead, ai)) in world.query_mut::<(
            &Creature,
            &mut Transform,
            Option<&crate::ecs::components::Dead>,
            Option<&crate::ecs::components::AIBehavior>,
        )>() {
            if dead.is_some() || ai.is_some() {
                continue;
            }
            let nearest_threat = threats
                .iter()
                .map(|p| (*p, (*p - tf.position).length()))
                .filter(|(_, d)| *d < FLEE_RADIUS)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            if let Some((threat_pos, _)) = nearest_threat {
                let away = (tf.position - threat_pos).normalize_or_zero();
                if away.length_squared() > 0.0 {
                    tf.position += away * (c.speed * 2.0) * dt;
                    tf.rotation = Quat::from_rotation_y(f32::atan2(away.x, away.z));
                }
                continue;
            }
            let t = self.t * 0.22 + c.phase;
            let target = c.anchor
                + Vec3::new(t.sin(), 0.0, (t * 0.63 + 1.7).cos()) * c.range;
            let to = target - tf.position;
            let dist = to.length();
            if dist > 0.15 {
                let step = (c.speed * dt).min(dist);
                let dir = to / dist;
                tf.position += dir * step;
                // Face travel direction: yaw 0 looks down +Z, matching the
                // render pass placing the head at rotation * +Z.
                tf.rotation = Quat::from_rotation_y(f32::atan2(dir.x, dir.z));
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped_registry() -> CreatureRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("creatures.csv");
        CreatureRegistry::from_csv(&std::fs::read(path).unwrap()).unwrap()
    }

    fn shipped_items() -> crate::systems::inventory::ItemRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("items.csv");
        crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(path).unwrap()).unwrap()
    }

    fn shipped_spawn_list() -> LivestockSpawnList {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("entities")
            .join("livestock.ron");
        LivestockSpawnList::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    /// Dev-spawned creatures match placed ones (v0.779 regression): PASSIVE
    /// species must NOT get AIBehavior (it would knock them out of the
    /// LivestockSystem's anchored graze into the AISystem's unanchored wander,
    /// drifting through walls), while hunt-class species must (predator).
    /// Renewable-yield species also carry a ready Harvestable.
    #[test]
    fn dev_spawn_matches_placed_bundles() {
        use crate::ecs::components::{AIBehavior, Harvestable};
        let reg = shipped_registry();
        let items = shipped_items();
        let mut world = hecs::World::new();

        let hen = reg.get("chicken").expect("chicken in creatures.csv");
        let e_hen = spawn_creature_at(&mut world, hen, Some(&items), Vec3::ZERO, [1.0; 3]);
        assert!(
            world.get::<&AIBehavior>(e_hen).is_err(),
            "passive species must have NO AIBehavior (anchored graze, like placed livestock)"
        );
        assert!(
            world.get::<&Harvestable>(e_hen).is_ok(),
            "renewable species should spawn with a ready Harvestable"
        );

        let wolf = reg.get("wolf").expect("wolf in creatures.csv");
        let e_wolf = spawn_creature_at(&mut world, wolf, Some(&items), Vec3::ZERO, [1.0; 3]);
        let ai = world.get::<&AIBehavior>(e_wolf).expect("hunt species get AIBehavior");
        assert_eq!(ai.behavior_type, "predator");
    }

    /// Forage flora (v0.978): stationary rows must get AIBehavior "stationary"
    /// (which exempts them from the graze amble AND idles at zero velocity in
    /// the AISystem - rooted forever) plus a ready Harvestable so walk-up [E]
    /// collection works from the first encounter.
    #[test]
    fn forage_flora_spawns_rooted_and_collectable() {
        use crate::ecs::components::{AIBehavior, Harvestable};
        let reg = shipped_registry();
        let items = shipped_items();
        let mut world = hecs::World::new();
        for (id, product) in [
            ("berry_bush", "fruit_berries_0"),
            ("wild_flax", "fiber_flax_0"),
            // Resource nodes (v0.982): the forage faucet rides the same rails.
            ("fallen_log", "wood_log_0"),
            ("stone_outcrop", "stone_raw_0"),
            ("clay_pit", "clay_raw_0"),
            ("salt_flat", "salt_food_0"),
            ("sand_pit", "sand_0"),
        ] {
            let def = reg.get(id).unwrap_or_else(|| panic!("{id} in creatures.csv"));
            assert_eq!(behavior_type_for(def), "stationary", "{id} must be stationary");
            let e = spawn_creature_at(&mut world, def, Some(&items), Vec3::ZERO, [1.0; 3]);
            let ai = world.get::<&AIBehavior>(e).expect("stationary flora get AIBehavior");
            assert_eq!(ai.behavior_type, "stationary");
            let h = world.get::<&Harvestable>(e).expect("flora carry a Harvestable");
            assert_eq!(h.resource, product, "{id} renewable product");
            assert!(
                h.time_since_harvest + f32::EPSILON >= h.regrow_time,
                "flora spawn ready to collect"
            );
        }
    }

    /// The shipped creatures.csv parses whole: all 92 species survive the
    /// row-resilient reader (a serde-eaten row here would vanish silently).
    #[test]
    fn creature_registry_parses_the_shipped_database() {
        let reg = shipped_registry();
        assert!(
            reg.len() >= 90,
            "expected the full creature database, got {}",
            reg.len()
        );
        let chicken = reg.get("chicken").expect("chicken exists");
        assert_eq!(chicken.hostility, "passive");
        assert_eq!(chicken.name, "Chicken");
        assert!(chicken.movement_speed > 0.0);
        // Renewable products parse for the farm trio.
        assert_eq!(
            chicken.renewable(),
            Some(RenewableProduct {
                item: "egg_0".into(),
                amount: 1,
                regrow_s: 300.0
            })
        );
        assert_eq!(reg.get("sheep").unwrap().renewable().unwrap().item, "wool_0");
        assert_eq!(reg.get("goat").unwrap().renewable().unwrap().item, "milk_0");
        // A wild species has no renewable yield.
        assert_eq!(reg.get("wolf").and_then(|d| d.renewable()), None);
    }

    /// Every loot-table drop across the WHOLE creature database resolves to
    /// a real items.csv id (directly or via the `_0` suffix) - a kill that
    /// drops a non-item would vanish silently. (v0.760, combat arc)
    #[test]
    fn every_loot_drop_resolves_to_a_real_item() {
        let reg = shipped_registry();
        let items = shipped_items();
        for def in reg.defs.values() {
            for (id, chance, min, max) in def.loot_entries(Some(&items)) {
                assert!(
                    items.items.contains_key(&id),
                    "{}: loot item {} is not in items.csv (even with _0)",
                    def.id,
                    id
                );
                assert!((0.0..=1.0).contains(&chance), "{}: chance {}", def.id, chance);
                assert!(max >= min, "{}: max < min on {}", def.id, id);
            }
            if !def.loot_table.is_empty() {
                assert!(
                    !def.loot_entries(Some(&items)).is_empty(),
                    "{}: authored loot_table parsed to nothing",
                    def.id
                );
            }
        }
    }

    /// Every renewable product across the WHOLE database resolves to a real
    /// items.csv id - the same zero-drop guarantee plants.csv harvest items
    /// got in v0.749 (an egg that is not an item would harvest into nothing).
    #[test]
    fn every_renewable_product_resolves_to_a_real_item() {
        let reg = shipped_registry();
        let items = shipped_items();
        for def in reg.defs.values() {
            if let Some(p) = def.renewable() {
                assert!(
                    items.items.contains_key(&p.item),
                    "{}: renewable product {} is not in items.csv",
                    def.id,
                    p.item
                );
                assert!(p.amount >= 1, "{}: zero-amount yield", def.id);
                assert!(p.regrow_s > 0.0, "{}: zero regrow time", def.id);
            }
        }
    }

    /// The homestead spawn list references only real species (with a real
    /// renewable yield - a starter animal you cannot collect from is a bug)
    /// and real home.ron machine instances to anchor near.
    #[test]
    fn shipped_spawn_list_resolves_species_and_anchors() {
        let reg = shipped_registry();
        let list = shipped_spawn_list();
        assert!(!list.animals.is_empty(), "starter livestock exist");

        let home_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("machines")
            .join("home.ron");
        let home = crate::machines::MachineHome::load(&home_path).expect("home.ron parses");
        let instance_ids: std::collections::HashSet<String> = home
            .all_instances()
            .iter()
            .map(|i| i.id.clone())
            .collect();

        for p in &list.animals {
            let def = reg
                .get(&p.creature)
                .unwrap_or_else(|| panic!("{} is not in creatures.csv", p.creature));
            assert!(
                def.renewable().is_some(),
                "{}: starter animal has no renewable product",
                p.creature
            );
            assert!(p.count >= 1, "{}: zero-count placement", p.creature);
            assert!(
                instance_ids.contains(&p.near),
                "{}: anchor {} is not a home.ron instance",
                p.creature,
                p.near
            );
        }
    }

    /// THE regrow cycle: ready yields once and only once, then the system
    /// ticks it back to ready over regrow_time.
    #[test]
    fn collect_yields_once_then_regrows() {
        let mut world = hecs::World::new();
        let data = DataStore::new();
        let hen = world.spawn((
            Creature {
                def_id: "chicken".into(),
                anchor: Vec3::ZERO,
                range: 2.0,
                phase: 0.0,
                speed: 0.5,
                tint: [1.0, 1.0, 1.0],
                body_side: 0.15,
            },
            Transform::default(),
            Harvestable {
                resource: "egg_0".into(),
                amount: 1.0,
                regrow_time: 300.0,
                time_since_harvest: 300.0, // spawned ready
            },
        ));

        // Ready: collect yields and resets the timer.
        {
            let mut h = world.get::<&mut Harvestable>(hen).unwrap();
            assert_eq!(collect(&mut h), Some(1));
            assert_eq!(h.time_since_harvest, 0.0);
            // Immediately again: still regrowing, nothing yielded.
            assert_eq!(collect(&mut h), None);
        }

        // Tick just short of regrown: still not ready.
        let mut sys = LivestockSystem::new();
        sys.tick(&mut world, 299.0, &data);
        {
            let mut h = world.get::<&mut Harvestable>(hen).unwrap();
            assert_eq!(collect(&mut h), None);
        }

        // Past the threshold: ready again, yields again.
        sys.tick(&mut world, 2.0, &data);
        {
            let mut h = world.get::<&mut Harvestable>(hen).unwrap();
            assert_eq!(collect(&mut h), Some(1));
        }
    }

    /// A living predator nearby overrides the amble: the animal runs
    /// directly AWAY at double speed; a dead predator spooks nobody. (v0.762)
    #[test]
    fn livestock_flee_living_predators() {
        use crate::ecs::components::AIBehavior;

        let mut world = hecs::World::new();
        let data = DataStore::new();
        let anchor = Vec3::new(0.0, 0.0, 0.0);
        let hen = world.spawn((
            Creature {
                def_id: "chicken".into(),
                anchor,
                range: 2.0,
                phase: 0.0,
                speed: 0.5,
                tint: [1.0, 1.0, 1.0],
                body_side: 0.15,
            },
            Transform { position: anchor, ..Default::default() },
        ));
        // A wolf 5m east - inside the flee radius.
        let wolf = world.spawn((
            AIBehavior {
                behavior_type: "predator".into(),
                state: "hunting".into(),
                target: None,
            },
            Transform { position: Vec3::new(5.0, 0.0, 0.0), ..Default::default() },
            crate::ecs::components::Dead::default(),
        ));
        // Dead wolf first: the hen ambles normally (no westward panic).
        let mut sys = LivestockSystem::new();
        sys.tick(&mut world, 0.1, &data);
        let calm_x = world.get::<&Transform>(hen).unwrap().position.x;
        assert!(calm_x > -0.06, "a dead predator spooks nobody (x = {calm_x})");

        // Revive the wolf: the hen bolts west (away), faster than the amble.
        world.remove_one::<crate::ecs::components::Dead>(wolf).unwrap();
        for _ in 0..10 {
            sys.tick(&mut world, 0.1, &data);
        }
        let fled = world.get::<&Transform>(hen).unwrap().position;
        assert!(
            fled.x < -0.5,
            "the hen ran away from the wolf (x = {})",
            fled.x
        );
    }

    /// Yields ripen on the game clock (2026-09-27): at 120x, as while the
    /// player sleeps, one real second ripens 120 game seconds of egg. Seen
    /// red with the regrowth back on raw dt (the hen 1 s along, not 120 s).
    #[test]
    fn yields_ripen_on_the_game_clock() {
        use crate::ecs::systems::System;
        let mut data = DataStore::new();
        let mut gt = crate::systems::time::GameTime::default();
        gt.time_scale = 120.0;
        data.insert("game_time", std::sync::Mutex::new(gt));
        let mut world = hecs::World::new();
        let hen = herd_hen(&mut world, "chicken#0", 0.0);
        let mut sys = LivestockSystem::new();
        for _ in 0..10 {
            sys.tick(&mut world, 0.1, &data);
        }
        let since = world.get::<&Harvestable>(hen).unwrap().time_since_harvest;
        assert!((since - 120.0).abs() < 0.5, "one real second at 120x ripens 120 s: {since}");
    }

    fn herd_hen(world: &mut hecs::World, slot: &str, since: f32) -> hecs::Entity {
        world.spawn((
            HerdSlot(slot.to_string()),
            Harvestable { resource: "egg_0".into(), amount: 1.0, regrow_time: 300.0, time_since_harvest: since },
        ))
    }

    /// The herd's yield timers survive a restart, and the time away moves
    /// them on, to one yield waiting and no more (the same cap as a player at
    /// home who never collects). Seen red with `timers_after_away` returning
    /// the timers unchanged (the hen still 100 s short after an hour).
    #[test]
    fn herd_timers_come_back_and_move_on_by_the_time_away() {
        let mut world = hecs::World::new();
        herd_hen(&mut world, "chicken#0", 100.0);
        herd_hen(&mut world, "chicken#1", 300.0);
        let dead = herd_hen(&mut world, "chicken#2", 0.0);
        world.insert_one(dead, crate::ecs::components::Dead::default()).unwrap();
        let saved = herd_timers(&world);
        assert_eq!(saved, vec![("chicken#0".to_string(), 100.0), ("chicken#1".to_string(), 300.0)], "the dead left out");

        // A new launch: the herd respawns ready; the save puts the timers back.
        let mut world = hecs::World::new();
        let hen = herd_hen(&mut world, "chicken#0", 300.0);
        assert_eq!(apply_herd_timers(&mut world, &saved), 1);
        assert_eq!(world.get::<&Harvestable>(hen).unwrap().time_since_harvest, 100.0, "still regrowing, as saved");

        // An hour away: ready, and one egg waits, not twelve.
        apply_herd_timers(&mut world, &timers_after_away(&saved, 3600.0));
        let mut h = world.get::<&mut Harvestable>(hen).unwrap();
        assert_eq!(h.time_since_harvest, 300.0);
        assert_eq!(collect(&mut h), Some(1));
        assert_eq!(collect(&mut h), None);
    }

    /// Only the animals that were still regrowing and are ready now count
    /// for the notice; one ready at the save is not news.
    #[test]
    fn the_notice_counts_the_animals_the_time_away_made_ready() {
        let reg = shipped_registry();
        let saved = vec![("chicken#0".to_string(), 100.0), ("chicken#1".to_string(), 300.0), ("goat#0".to_string(), 0.0)];
        assert_eq!(readied_by_away(&saved, 60.0, &reg), 0);
        assert_eq!(readied_by_away(&saved, 200.0, &reg), 1, "the hen, not the goat (400 s)");
        assert_eq!(readied_by_away(&saved, 3600.0, &reg), 2);
    }

    /// Timers restored before the herd exists wait for world entry, and are
    /// put on a herd that exists at once (a character select).
    #[test]
    fn restored_timers_wait_for_the_herd_or_apply_at_once() {
        let mut data = DataStore::new();
        register(&mut data);
        let mut world = hecs::World::new();
        restore_herd(&mut world, &data, vec![("chicken#0".to_string(), 50.0)]);
        assert_eq!(pending_herd(&data), Some(vec![("chicken#0".to_string(), 50.0)]), "no herd yet: held");
        let hen = herd_hen(&mut world, "chicken#0", 300.0);
        restore_herd(&mut world, &data, vec![("chicken#0".to_string(), 20.0)]);
        assert_eq!(world.get::<&Harvestable>(hen).unwrap().time_since_harvest, 20.0);
        assert_eq!(take_pending_herd(&data), None, "applied, nothing left waiting");
    }

    /// The graze amble moves an animal toward its wander target and never
    /// teleports it (bounded by speed * dt).
    #[test]
    fn graze_amble_moves_within_speed_limit() {
        let mut world = hecs::World::new();
        let data = DataStore::new();
        let anchor = Vec3::new(27.0, 0.0, 65.0);
        let goat = world.spawn((
            Creature {
                def_id: "goat".into(),
                anchor,
                range: 3.0,
                phase: 1.3,
                speed: 0.6,
                tint: [0.6, 0.5, 0.4],
                body_side: 0.39,
            },
            Transform {
                position: anchor,
                ..Default::default()
            },
        ));

        let mut sys = LivestockSystem::new();
        let mut last = anchor;
        for _ in 0..60 {
            sys.tick(&mut world, 0.1, &data);
            let tf = world.get::<&Transform>(goat).unwrap();
            let step = (tf.position - last).length();
            assert!(step <= 0.6 * 0.1 + 1e-4, "moved {step} m in 0.1 s");
            assert!(
                (tf.position - anchor).length() <= 3.0 + 0.5,
                "wandered out of range"
            );
            last = tf.position;
        }
        assert_ne!(last, anchor, "the goat actually went somewhere");
    }
}

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

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        self.t += dt;

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
                h.time_since_harvest = (h.time_since_harvest + dt).min(h.regrow_time);
            }
        }

        // Graze amble: ease each animal toward a slowly-orbiting target around
        // its anchor. The two incommensurate frequencies trace a lissajous
        // loop, so the herd drifts naturally instead of circling. The dead
        // stay where they fell (v0.760).
        for (_e, (c, tf, dead)) in world.query_mut::<(
            &Creature,
            &mut Transform,
            Option<&crate::ecs::components::Dead>,
        )>() {
            if dead.is_some() {
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

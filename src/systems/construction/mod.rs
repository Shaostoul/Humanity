//! Construction system: blueprint placement, building progress, snap grid.
//!
//! Blueprints define buildable structures loaded from RON files.
//! Construction progresses over time and consumes inventory materials.

pub mod structural;
pub mod routing;
pub mod solver;

use crate::ecs::components::Transform;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use glam::Vec3;
use serde::Deserialize;
use std::collections::HashMap;

/// A buildable structure definition loaded from data files.
#[derive(Debug, Clone, Deserialize)]
pub struct Blueprint {
    pub id: String,
    pub name: String,
    pub category: String,
    pub materials: Vec<(String, u32)>,
    pub build_time: f32,
    pub size: [f32; 3],
    pub snap_to: Vec<String>,
    pub health: f32,
    pub provides: Option<String>,
}

/// Registry of all available blueprints.
pub struct BlueprintRegistry {
    pub blueprints: HashMap<String, Blueprint>,
}

impl BlueprintRegistry {
    pub fn new() -> Self {
        Self {
            blueprints: HashMap::new(),
        }
    }

    pub fn register(&mut self, bp: Blueprint) {
        self.blueprints.insert(bp.id.clone(), bp);
    }

    pub fn get(&self, id: &str) -> Option<&Blueprint> {
        self.blueprints.get(id)
    }

    /// Parse a RON array of `Blueprint` entries (the shape `data/blueprints/basic.ron` already
    /// ships, e.g. `[(id: "wood_wall", ...), ...]`) into a populated registry. Was never called
    /// anywhere before this (`ConstructionSystem` was registered but had nothing to build from --
    /// `queue_build` always missed the registry lookup and silently skipped every pending build).
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let blueprints: Vec<Blueprint> = ron::from_str(text).map_err(|e| e.to_string())?;
        let mut reg = Self::new();
        for bp in blueprints {
            reg.register(bp);
        }
        Ok(reg)
    }
}

/// Component for entities currently under construction.
pub struct Construction {
    pub blueprint_id: String,
    pub progress: f32,
    pub build_time: f32,
    pub builder_key: Option<String>,
}

/// Component for completed structures.
pub struct Structure {
    pub blueprint_id: String,
    pub health: f32,
    pub max_health: f32,
    pub provides: Option<String>,
}

/// Construction system processes active builds each frame.
pub struct ConstructionSystem {
    /// Pending build commands (blueprint_id, position).
    pending_builds: Vec<(String, Vec3)>,
}

impl ConstructionSystem {
    pub fn new() -> Self {
        Self {
            pending_builds: Vec::new(),
        }
    }

    /// Queue a build command.
    pub fn queue_build(&mut self, blueprint_id: String, position: Vec3) {
        self.pending_builds.push((blueprint_id, position));
    }

    /// Snap a position to the 1m grid.
    fn snap_to_grid(pos: Vec3) -> Vec3 {
        Vec3::new(pos.x.round(), pos.y.round(), pos.z.round())
    }
}

impl System for ConstructionSystem {
    fn name(&self) -> &str {
        "Construction"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Process pending build commands: the internal queue (tests/API) PLUS the
        // "build_request" DataStore channel the GUI writes (v0.746, closure ladder
        // rung 2 — queue_build had zero callers before this channel existed).
        let mut builds: Vec<(String, Vec3)> = self.pending_builds.drain(..).collect();
        if let Some(chan) = data.get::<std::sync::Mutex<Vec<(String, Vec3)>>>("build_request") {
            if let Ok(mut c) = chan.lock() {
                builds.append(&mut c);
            }
        }
        let registry = data.get::<BlueprintRegistry>("blueprint_registry");
        let mut status: Option<String> = None;

        for (bp_id, pos) in builds {
            let bp = match registry.as_ref().and_then(|r| r.get(&bp_id)) {
                Some(bp) => bp.clone(),
                None => {
                    status = Some(format!("Unknown blueprint '{bp_id}'"));
                    continue;
                }
            };

            // MATERIALS ARE REAL (v0.746): the doc header always said "consumes
            // inventory materials" but nothing ever did. Count backpack + home
            // storage (the same home_stock mirror auto-machines use, v0.737),
            // refuse honestly when short, consume BACKPACK-FIRST when not.
            let home_stock =
                data.get::<std::sync::Mutex<std::collections::HashMap<String, u32>>>("home_stock");
            let home_count = |id: &str| -> u32 {
                home_stock
                    .as_ref()
                    .and_then(|m| m.lock().ok().map(|s| s.get(id).copied().unwrap_or(0)))
                    .unwrap_or(0)
            };
            let mut player_inv: Option<hecs::Entity> = None;
            for (e, (_inv, _ctrl)) in world
                .query::<(
                    &crate::systems::inventory::Inventory,
                    &crate::ecs::components::Controllable,
                )>()
                .iter()
            {
                player_inv = Some(e);
                break;
            }
            let Some(player) = player_inv else {
                status = Some("No builder inventory".to_string());
                continue;
            };
            let missing: Option<String> = {
                let inv = world
                    .get::<&crate::systems::inventory::Inventory>(player)
                    .expect("player inventory queried above");
                bp.materials.iter().find_map(|(id, qty)| {
                    let have = inv.count_item(id) + home_count(id);
                    (have < *qty).then(|| format!("need {}x {} to build {}", qty - have, id, bp.name))
                })
            };
            if let Some(m) = missing {
                status = Some(m);
                continue;
            }
            if let Ok(mut inv) = world.get::<&mut crate::systems::inventory::Inventory>(player) {
                for (id, qty) in &bp.materials {
                    let from_pack = inv.count_item(id).min(*qty);
                    if from_pack > 0 {
                        inv.remove_item(id, from_pack);
                    }
                    let remainder = qty - from_pack;
                    if remainder > 0 {
                        if let Some(m) = home_stock.as_ref() {
                            if let Ok(mut s) = m.lock() {
                                if let Some(c) = s.get_mut(id) {
                                    *c = c.saturating_sub(remainder);
                                }
                            }
                        }
                    }
                }
            }

            let snapped = Self::snap_to_grid(pos);
            status = Some(format!("Building {}...", bp.name));
            world.spawn((
                Transform {
                    position: snapped,
                    rotation: glam::Quat::IDENTITY,
                    scale: Vec3::from_array(bp.size),
                },
                Construction {
                    blueprint_id: bp.id.clone(),
                    progress: 0.0,
                    build_time: bp.build_time,
                    builder_key: None,
                },
            ));
        }

        // Advance active constructions
        let mut completed = Vec::new();

        for (entity, construction) in world.query_mut::<&mut Construction>() {
            construction.progress += dt;
            if construction.progress >= construction.build_time {
                completed.push((entity, construction.blueprint_id.clone()));
            }
        }

        // Convert completed constructions to structures
        for (entity, bp_id) in completed {
            let _ = world.remove_one::<Construction>(entity);

            let (health, provides, name) = registry
                .as_ref()
                .and_then(|r| r.get(&bp_id))
                .map(|bp| (bp.health, bp.provides.clone(), bp.name.clone()))
                .unwrap_or((100.0, None, bp_id.clone()));

            let _ = world.insert_one(
                entity,
                Structure {
                    blueprint_id: bp_id.clone(),
                    health,
                    max_health: health,
                    provides,
                },
            );
            // Completion is PROGRESS (v0.746): the construction quest chain's
            // Build objectives finally advance, and building trains the builder.
            crate::systems::quests::push_quest_event(data, format!("build_{bp_id}"));
            crate::systems::skills::award_skill_xp(data, "shelter_building", 15);
            status = Some(format!("{name} complete"));
        }

        // One honest status line for the GUI (missing materials, in-progress,
        // completed) — same pattern as auto_craft_status.
        if let Some(s) = status {
            if let Some(slot) = data.get::<std::sync::Mutex<String>>("build_status") {
                if let Ok(mut b) = slot.lock() {
                    *b = s;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `data/blueprints/basic.ron` shipped a real foundation/wall/door/window/roof/
    /// furniture/machine catalog with nothing loading it (registered 2026-07-01, see
    /// lib.rs's load_data_registries) -- pins that `from_ron` actually parses the real
    /// shipped file, not just a synthetic fixture.
    #[test]
    fn from_ron_parses_the_real_shipped_blueprint_catalog() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("basic.ron");
        let bytes = std::fs::read(&path).expect("data/blueprints/basic.ron exists");
        let reg = BlueprintRegistry::from_ron(&bytes).expect("basic.ron parses as Vec<Blueprint>");
        assert!(reg.blueprints.len() >= 10, "expected a real multi-entry catalog, got {}", reg.blueprints.len());

        let wall = reg.get("wood_wall").expect("wood_wall is in the shipped catalog");
        assert_eq!(wall.category, "wall");
        assert!(!wall.materials.is_empty(), "a wall must cost real materials");
        assert!(wall.build_time > 0.0);

        let furnace = reg.get("furnace").expect("furnace is in the shipped catalog");
        assert_eq!(furnace.provides.as_deref(), Some("smelting"));
    }

    /// A registry built from garbage bytes must fail cleanly (Err), not panic --
    /// `load_data_registries` logs a warning and leaves ConstructionSystem idle on a
    /// bad/missing file rather than crashing world init.
    #[test]
    fn from_ron_rejects_malformed_data_without_panicking() {
        let result = BlueprintRegistry::from_ron(b"not valid ron at all {{{");
        assert!(result.is_err());
    }

    fn shipped_registry() -> BlueprintRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("basic.ron");
        BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    fn build_store(reg: BlueprintRegistry, request: Vec<(String, Vec3)>) -> DataStore {
        let mut data = DataStore::new();
        data.insert("blueprint_registry", reg);
        data.insert("build_request", std::sync::Mutex::new(request));
        data.insert("build_status", std::sync::Mutex::new(String::new()));
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        data
    }

    /// v0.746 (closure ladder rung 2): THE BUILD LOOP. A build_request consumes
    /// the blueprint's materials from the builder's inventory, spawns a timed
    /// Construction, converts it to a Structure at completion, and fires the
    /// build_<id> quest event the authored construction quests wait on.
    #[test]
    fn build_request_consumes_materials_and_completes() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;

        let reg = shipped_registry();
        let wall = reg.get("wood_wall").unwrap().clone();
        let data = build_store(
            reg,
            vec![("wood_wall".to_string(), Vec3::new(1.2, 0.0, 3.7))],
        );
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        for (id, qty) in &wall.materials {
            inv.add_item(id, *qty, 99);
        }
        let player = world.spawn((inv, Controllable));

        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);

        {
            let inv = world.get::<&Inventory>(player).unwrap();
            for (id, _qty) in &wall.materials {
                assert_eq!(inv.count_item(id), 0, "{id} consumed at build start");
            }
        }
        {
            let mut q = world.query::<(&Construction, &Transform)>();
            let (_, (_, tf)) = q.iter().next().expect("a Construction spawned");
            assert_eq!(tf.position, Vec3::new(1.0, 0.0, 4.0), "snapped to the metre grid");
        }

        // Run past the build time: the scaffold becomes a real Structure.
        sys.tick(&mut world, wall.build_time + 1.0, &data);
        assert_eq!(world.query::<&Structure>().iter().count(), 1, "structure completed");
        assert_eq!(world.query::<&Construction>().iter().count(), 0);
        let events = data
            .get::<std::sync::Mutex<Vec<String>>>("quest_events")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(
            events.iter().any(|e| e == "build_wood_wall"),
            "build quest event fired, got {events:?}"
        );
    }

    /// Building with nothing in the pack is REFUSED with an honest status line
    /// (nothing spawns, nothing is consumed).
    #[test]
    fn build_refused_without_materials() {
        use crate::ecs::components::Controllable;
        use crate::systems::inventory::Inventory;

        let data = build_store(
            shipped_registry(),
            vec![("wood_wall".to_string(), Vec3::ZERO)],
        );
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(8), Controllable));

        let mut sys = ConstructionSystem::new();
        sys.tick(&mut world, 0.05, &data);

        assert_eq!(world.query::<&Construction>().iter().count(), 0, "no scaffold");
        let status = data
            .get::<std::sync::Mutex<String>>("build_status")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(status.contains("need"), "status explains the shortage: {status}");
    }
}

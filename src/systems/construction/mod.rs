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
        // Process pending build commands
        let builds: Vec<(String, Vec3)> = self.pending_builds.drain(..).collect();
        let registry = data.get::<BlueprintRegistry>("blueprint_registry");

        for (bp_id, pos) in builds {
            let bp = match registry.as_ref().and_then(|r| r.get(&bp_id)) {
                Some(bp) => bp.clone(),
                None => continue,
            };

            let snapped = Self::snap_to_grid(pos);

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

            let (health, provides) = registry
                .as_ref()
                .and_then(|r| r.get(&bp_id))
                .map(|bp| (bp.health, bp.provides.clone()))
                .unwrap_or((100.0, None));

            let _ = world.insert_one(
                entity,
                Structure {
                    blueprint_id: bp_id,
                    health,
                    max_health: health,
                    provides,
                },
            );
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
}

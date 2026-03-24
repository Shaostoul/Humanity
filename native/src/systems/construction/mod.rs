//! Construction system: blueprint placement, building progress, snap grid.
//!
//! Blueprints define buildable structures loaded from RON files.
//! Construction progresses over time and consumes inventory materials.

pub mod csg;
pub mod blueprint;
pub mod structural;
pub mod routing;

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

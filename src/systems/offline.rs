//! Offline behaviors system — fires `AutonomousTask` actions on a periodic
//! schedule. Used for AFK NPC chores (patrol, gather, build) that don't need
//! a full BehaviorTree.
//!
//! Per-tick: every entity with `AutonomousTask` gets `seconds_since_last`
//! incremented. When that exceeds `interval_seconds`, `fire_count` increments
//! and a log line records the firing. Game code reads `fire_count` deltas to
//! apply task effects (NPC-driven harvest, scheduled deliveries, etc.).
//!
//! Future expansion: each preset id should map to a real action callback so
//! tasks can mutate world state. For now this is the scheduling primitive.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::AutonomousTask;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/offline_behaviors.ron`.
#[derive(Debug, Deserialize)]
pub struct OfflineData {
    #[serde(default)] pub presets: Vec<ron::Value>,
    #[serde(default)] pub autonomy_rules: Vec<ron::Value>,
}

/// Manages autonomous agent presets for offline/away play.
pub struct OfflineSystem {
    pub data: OfflineData,
    /// Total task firings across all agents (lifetime stat).
    pub lifetime_fires: u64,
}

impl OfflineSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("offline_behaviors.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(presets:[],autonomy_rules:[])".to_string()
        });
        let data: OfflineData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse offline_behaviors.ron: {e}");
            OfflineData { presets: vec![], autonomy_rules: vec![] }
        });
        log::info!("Loaded offline data: {} presets, {} rules", data.presets.len(), data.autonomy_rules.len());
        Self { data, lifetime_fires: 0 }
    }
}

impl System for OfflineSystem {
    fn name(&self) -> &str { "OfflineSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }

        let mut fires_this_tick = 0_u64;

        for (entity, task) in world.query_mut::<&mut AutonomousTask>() {
            if task.interval_seconds <= 0.0 { continue; }
            task.seconds_since_last += dt;
            // Fire as many times as the elapsed interval covers (catches up
            // after a long pause / fast-forward).
            while task.seconds_since_last >= task.interval_seconds {
                task.seconds_since_last -= task.interval_seconds;
                task.fire_count += 1;
                fires_this_tick += 1;
                log::trace!(
                    "Offline: agent {:?} fired '{}' (count={})",
                    entity, task.preset_id, task.fire_count
                );
            }
        }

        self.lifetime_fires += fires_this_tick;
    }
}

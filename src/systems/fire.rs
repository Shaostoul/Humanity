//! Fire system -- ignition, fire spread, and suppression.
//!
//! Loads ignition sources, fire behaviors, suppression systems, and damage
//! effects from `data/fire_system.ron`. Tracks active fires per tile.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/fire_system.ron`.
#[derive(Debug, Deserialize)]
pub struct FireData {
    pub ignition_sources: Vec<ron::Value>,
    pub fire_behaviors: Vec<ron::Value>,
    pub suppression_systems: Vec<ron::Value>,
    pub fire_damage_effects: Vec<ron::Value>,
}

/// Tracks ignition sources, fire spread, and suppression.
pub struct FireSystem {
    pub data: FireData,
}

impl FireSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("fire_system.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(ignition_sources:[],fire_behaviors:[],suppression_systems:[],fire_damage_effects:[])".to_string()
        });
        let data: FireData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse fire_system.ron: {e}");
            FireData { ignition_sources: vec![], fire_behaviors: vec![], suppression_systems: vec![], fire_damage_effects: vec![] }
        });
        log::info!("Loaded fire data: {} ignition sources, {} behaviors", data.ignition_sources.len(), data.fire_behaviors.len());
        Self { data }
    }
}

impl System for FireSystem {
    fn name(&self) -> &str {
        "FireSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: implement fire ignition, spread, and suppression simulation
    }
}

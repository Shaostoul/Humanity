//! Offline behaviors system -- autonomous agent presets for offline play.
//!
//! Loads behavior presets and autonomy rules from `data/offline_behaviors.ron`.
//! Drives NPC and player-delegated actions while the player is away.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/offline_behaviors.ron`.
#[derive(Debug, Deserialize)]
pub struct OfflineData {
    pub presets: Vec<ron::Value>,
    pub autonomy_rules: Vec<ron::Value>,
}

/// Manages autonomous agent presets for offline/away play.
pub struct OfflineSystem {
    pub data: OfflineData,
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
        Self { data }
    }
}

impl System for OfflineSystem {
    fn name(&self) -> &str { "OfflineSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: apply behavior presets to delegated agents, simulate offline progress
    }
}

//! Waste management system -- waste categories, recycling, pollution.
//!
//! Loads waste categories, recycling methods, and pollution thresholds from
//! `data/waste_management.ron`. Tracks waste accumulation and recycling rates.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/waste_management.ron`.
#[derive(Debug, Deserialize)]
pub struct WasteData {
    pub categories: Vec<ron::Value>,
    pub recycling: Vec<ron::Value>,
    pub pollution: Vec<ron::Value>,
}

/// Manages waste categories, recycling processes, and pollution tracking.
pub struct WasteSystem {
    pub data: WasteData,
}

impl WasteSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("waste_management.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(categories:[],recycling:[],pollution:[])".to_string()
        });
        let data: WasteData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse waste_management.ron: {e}");
            WasteData { categories: vec![], recycling: vec![], pollution: vec![] }
        });
        log::info!("Loaded waste data: {} categories, {} recycling methods", data.categories.len(), data.recycling.len());
        Self { data }
    }
}

impl System for WasteSystem {
    fn name(&self) -> &str { "WasteSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: accumulate waste, process recycling, update pollution levels
    }
}

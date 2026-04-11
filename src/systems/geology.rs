//! Geology system -- rock types, ore veins, soil composition, tectonic events.
//!
//! Loads rock classifications, ore vein definitions, and soil data from
//! `data/geology.ron`. Drives mining yields and terrain composition.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/geology.ron`.
#[derive(Debug, Deserialize)]
pub struct GeologyData {
    pub rock_types: Vec<ron::Value>,
    pub ore_veins: Vec<ron::Value>,
    pub soil_types: Vec<ron::Value>,
    pub tectonic_events: Vec<ron::Value>,
}

/// Manages rock types, ore veins, soil composition, and tectonic events.
pub struct GeologySystem {
    pub data: GeologyData,
}

impl GeologySystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("geology.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(rock_types:[],ore_veins:[],soil_types:[],tectonic_events:[])".to_string()
        });
        let data: GeologyData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse geology.ron: {e}");
            GeologyData { rock_types: vec![], ore_veins: vec![], soil_types: vec![], tectonic_events: vec![] }
        });
        log::info!("Loaded geology data: {} rock types, {} ore veins", data.rock_types.len(), data.ore_veins.len());
        Self { data }
    }
}

impl System for GeologySystem {
    fn name(&self) -> &str { "GeologySystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: update ore vein depletion, soil composition changes, trigger tectonic events
    }
}

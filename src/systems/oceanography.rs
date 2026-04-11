//! Oceanography system -- ocean zones, currents, marine resources.
//!
//! Loads ocean zone definitions, current patterns, and marine resource data from
//! `data/oceanography.ron`. Drives marine ecosystems and resource harvesting.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/oceanography.ron`.
#[derive(Debug, Deserialize)]
pub struct OceanographyData {
    pub zones: Vec<ron::Value>,
    pub currents: Vec<ron::Value>,
    pub marine_resources: Vec<ron::Value>,
}

/// Manages ocean zones, currents, and marine resources.
pub struct OceanographySystem {
    pub data: OceanographyData,
}

impl OceanographySystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("oceanography.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(zones:[],currents:[],marine_resources:[])".to_string()
        });
        let data: OceanographyData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse oceanography.ron: {e}");
            OceanographyData { zones: vec![], currents: vec![], marine_resources: vec![] }
        });
        log::info!("Loaded oceanography data: {} zones, {} currents", data.zones.len(), data.currents.len());
        Self { data }
    }
}

impl System for OceanographySystem {
    fn name(&self) -> &str { "OceanographySystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: simulate ocean currents, update marine resource availability
    }
}

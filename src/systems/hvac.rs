//! HVAC system -- heating, ventilation, air conditioning, and life support.
//!
//! Loads heating, cooling, ventilation, life support, and sensor definitions
//! from `data/hvac.ron`. Tracks per-room temperature and air quality.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/hvac.ron`.
#[derive(Debug, Deserialize)]
pub struct HvacData {
    pub heating: Vec<ron::Value>,
    pub cooling: Vec<ron::Value>,
    pub ventilation: Vec<ron::Value>,
    pub life_support: Vec<ron::Value>,
    pub sensors: Vec<ron::Value>,
}

/// Tracks heating, cooling, and ventilation per room.
pub struct HvacSystem {
    pub data: HvacData,
}

impl HvacSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("hvac.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(heating:[],cooling:[],ventilation:[],life_support:[],sensors:[])".to_string()
        });
        let data: HvacData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse hvac.ron: {e}");
            HvacData { heating: vec![], cooling: vec![], ventilation: vec![], life_support: vec![], sensors: vec![] }
        });
        log::info!("Loaded HVAC data: {} heating, {} cooling", data.heating.len(), data.cooling.len());
        Self { data }
    }
}

impl System for HvacSystem {
    fn name(&self) -> &str {
        "HvacSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: implement per-room temperature and air quality simulation
    }
}

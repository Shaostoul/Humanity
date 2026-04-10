//! Medical system -- conditions, treatments, procedures, and prosthetics.
//!
//! Loads medical conditions, procedures, support treatments, and prosthetics
//! from `data/medical.ron`. Tracks injuries, recovery timers, and augmentations.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/medical.ron`.
#[derive(Debug, Deserialize)]
pub struct MedicalData {
    pub conditions: Vec<ron::Value>,
    pub procedures: Vec<ron::Value>,
    pub support_procedures: Vec<ron::Value>,
    pub prosthetics: Vec<ron::Value>,
}

/// Tracks medical conditions, treatments, and recovery.
pub struct MedicalSystem {
    pub data: MedicalData,
}

impl MedicalSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("medical.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(conditions:[],procedures:[],support_procedures:[],prosthetics:[])".to_string()
        });
        let data: MedicalData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse medical.ron: {e}");
            MedicalData { conditions: vec![], procedures: vec![], support_procedures: vec![], prosthetics: vec![] }
        });
        log::info!("Loaded medical data: {} conditions, {} procedures", data.conditions.len(), data.procedures.len());
        Self { data }
    }
}

impl System for MedicalSystem {
    fn name(&self) -> &str {
        "MedicalSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: implement condition tracking, treatment application, and recovery
    }
}

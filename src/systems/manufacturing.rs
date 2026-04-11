//! Manufacturing system -- production stages, assembly lines, quality control.
//!
//! Loads production stage definitions and assembly line configurations from
//! `data/manufacturing.ron`. Drives industrial production and quality checks.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/manufacturing.ron`.
#[derive(Debug, Deserialize)]
pub struct ManufacturingData {
    pub stages: Vec<ron::Value>,
    pub assembly_lines: Vec<ron::Value>,
    pub quality_checks: Vec<ron::Value>,
}

/// Manages production stages, assembly lines, and quality control.
pub struct ManufacturingSystem {
    pub data: ManufacturingData,
}

impl ManufacturingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("manufacturing.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(stages:[],assembly_lines:[],quality_checks:[])".to_string()
        });
        let data: ManufacturingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse manufacturing.ron: {e}");
            ManufacturingData { stages: vec![], assembly_lines: vec![], quality_checks: vec![] }
        });
        log::info!("Loaded manufacturing data: {} stages, {} assembly lines", data.stages.len(), data.assembly_lines.len());
        Self { data }
    }
}

impl System for ManufacturingSystem {
    fn name(&self) -> &str { "ManufacturingSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: advance production stages, run assembly lines, perform quality checks
    }
}

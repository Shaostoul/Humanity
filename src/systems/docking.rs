//! Docking system -- docking ports, airlocks, EVA mechanics.
//!
//! Loads port types, airlock definitions, and EVA equipment from
//! `data/docking.ron`. Manages docking sequences and EVA state.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/docking.ron`.
#[derive(Debug, Deserialize)]
pub struct DockingData {
    pub ports: Vec<ron::Value>,
    pub airlocks: Vec<ron::Value>,
    pub eva_equipment: Vec<ron::Value>,
    pub procedures: Vec<ron::Value>,
}

/// Manages docking ports, airlocks, and EVA mechanics.
pub struct DockingSystem {
    pub data: DockingData,
}

impl DockingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("docking.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(ports:[],airlocks:[],eva_equipment:[],procedures:[])".to_string()
        });
        let data: DockingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse docking.ron: {e}");
            DockingData { ports: vec![], airlocks: vec![], eva_equipment: vec![], procedures: vec![] }
        });
        log::info!("Loaded docking data: {} ports, {} airlocks", data.ports.len(), data.airlocks.len());
        Self { data }
    }
}

impl System for DockingSystem {
    fn name(&self) -> &str { "DockingSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: manage docking sequences, airlock cycling, EVA state transitions
    }
}

//! Plumbing system -- water supply, drainage, pressure, and fixture demand.
//!
//! Loads pipe, fixture, treatment, storage, and valve definitions from
//! `data/plumbing.ron`. Tracks water flow and pressure per network.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/plumbing.ron`.
#[derive(Debug, Deserialize)]
pub struct PlumbingData {
    pub pipes: Vec<ron::Value>,
    pub fixtures: Vec<ron::Value>,
    pub treatment: Vec<ron::Value>,
    pub storage: Vec<ron::Value>,
    pub valves: Vec<ron::Value>,
}

/// Tracks water flow, pressure, and fixture demand.
pub struct PlumbingSystem {
    pub data: PlumbingData,
}

impl PlumbingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("plumbing.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(pipes:[],fixtures:[],treatment:[],storage:[],valves:[])".to_string()
        });
        let data: PlumbingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse plumbing.ron: {e}");
            PlumbingData { pipes: vec![], fixtures: vec![], treatment: vec![], storage: vec![], valves: vec![] }
        });
        log::info!("Loaded plumbing data: {} pipes, {} fixtures", data.pipes.len(), data.fixtures.len());
        Self { data }
    }
}

impl System for PlumbingSystem {
    fn name(&self) -> &str {
        "PlumbingSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: implement water flow, pressure, and fixture demand simulation
    }
}

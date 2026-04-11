//! Transportation system -- roads, rail, space infrastructure.
//!
//! Loads road types, rail networks, and space infrastructure definitions from
//! `data/transportation.ron`. Manages transit routes and cargo logistics.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/transportation.ron`.
#[derive(Debug, Deserialize)]
pub struct TransportationData {
    pub roads: Vec<ron::Value>,
    pub rail: Vec<ron::Value>,
    pub space_infrastructure: Vec<ron::Value>,
}

/// Manages roads, rail networks, and space infrastructure.
pub struct TransportationSystem {
    pub data: TransportationData,
}

impl TransportationSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("transportation.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(roads:[],rail:[],space_infrastructure:[])".to_string()
        });
        let data: TransportationData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse transportation.ron: {e}");
            TransportationData { roads: vec![], rail: vec![], space_infrastructure: vec![] }
        });
        log::info!("Loaded transportation data: {} roads, {} rail", data.roads.len(), data.rail.len());
        Self { data }
    }
}

impl System for TransportationSystem {
    fn name(&self) -> &str { "TransportationSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: update transit routes, manage cargo movement, maintain infrastructure
    }
}

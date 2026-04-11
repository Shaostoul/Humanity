//! Astronomy system -- telescopes, navigation, communication, sensors.
//!
//! Loads telescope types, navigation instruments, and sensor arrays from
//! `data/astronomy_tools.ron`. Supports celestial observation and space navigation.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/astronomy_tools.ron`.
#[derive(Debug, Deserialize)]
pub struct AstronomyData {
    pub telescopes: Vec<ron::Value>,
    pub navigation: Vec<ron::Value>,
    pub communication: Vec<ron::Value>,
    pub sensors: Vec<ron::Value>,
}

/// Manages telescopes, navigation instruments, communication, and sensors.
pub struct AstronomySystem {
    pub data: AstronomyData,
}

impl AstronomySystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("astronomy_tools.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(telescopes:[],navigation:[],communication:[],sensors:[])".to_string()
        });
        let data: AstronomyData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse astronomy_tools.ron: {e}");
            AstronomyData { telescopes: vec![], navigation: vec![], communication: vec![], sensors: vec![] }
        });
        log::info!("Loaded astronomy data: {} telescopes, {} sensors", data.telescopes.len(), data.sensors.len());
        Self { data }
    }
}

impl System for AstronomySystem {
    fn name(&self) -> &str { "AstronomySystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: update telescope observations, navigation calculations, sensor sweeps
    }
}

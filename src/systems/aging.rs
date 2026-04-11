//! Aging system -- age stages, fitness, exercise, sleep.
//!
//! Loads age stages, fitness levels, exercise types, and sleep mechanics from
//! `data/aging_fitness.ron`. Tracks per-entity aging and fitness state.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/aging_fitness.ron`.
#[derive(Debug, Deserialize)]
pub struct AgingData {
    pub age_stages: Vec<ron::Value>,
    pub fitness_levels: Vec<ron::Value>,
    pub exercises: Vec<ron::Value>,
    pub sleep: Vec<ron::Value>,
}

/// Tracks age stages, fitness, exercise, and sleep for entities.
pub struct AgingSystem {
    pub data: AgingData,
}

impl AgingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("aging_fitness.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(age_stages:[],fitness_levels:[],exercises:[],sleep:[])".to_string()
        });
        let data: AgingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse aging_fitness.ron: {e}");
            AgingData { age_stages: vec![], fitness_levels: vec![], exercises: vec![], sleep: vec![] }
        });
        log::info!("Loaded aging data: {} stages, {} exercises", data.age_stages.len(), data.exercises.len());
        Self { data }
    }
}

impl System for AgingSystem {
    fn name(&self) -> &str { "AgingSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: advance age stages, update fitness from exercise, manage sleep cycles
    }
}

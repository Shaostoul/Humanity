//! Astronomy system — accumulates `Telescope.observation_seconds` while a
//! telescope has a target. Game code interprets the accumulated seconds:
//! more time → better resolution → more research data points / unlocked
//! discoveries / clearer navigation fixes.
//!
//! Each tick: every telescope with a non-empty `target_id` gains
//! `dt * power / 1000.0` observation seconds (so a 1000x scope earns
//! 1 game-second of observation per real second; weaker scopes earn less).

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::Telescope;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/astronomy_tools.ron`.
#[derive(Debug, Deserialize)]
pub struct AstronomyData {
    #[serde(default)] pub telescopes: Vec<ron::Value>,
    #[serde(default)] pub navigation: Vec<ron::Value>,
    #[serde(default)] pub communication: Vec<ron::Value>,
    #[serde(default)] pub sensors: Vec<ron::Value>,
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

    /// Re-aim a telescope. Resets observation accumulator.
    pub fn aim(world: &mut hecs::World, entity: hecs::Entity, target_id: &str) {
        if let Ok(mut t) = world.get::<&mut Telescope>(entity) {
            t.target_id = target_id.to_string();
            t.observation_seconds = 0.0;
        }
    }
}

impl System for AstronomySystem {
    fn name(&self) -> &str { "AstronomySystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }

        for (_, t) in world.query_mut::<&mut Telescope>() {
            if t.target_id.is_empty() { continue; }
            // power 1000 → 1 obs-sec per real-sec; power 100 → 0.1 obs-sec / sec.
            let gain = dt * (t.power / 1000.0);
            t.observation_seconds += gain;
        }
    }
}

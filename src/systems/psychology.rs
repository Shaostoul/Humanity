//! Psychology system -- Maslow needs, morale, and personality traits.
//!
//! Loads needs hierarchy, morale modifiers, and personality traits from
//! `data/psychology.ron`. Tracks per-entity need levels and morale.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/psychology.ron`.
#[derive(Debug, Deserialize)]
pub struct PsychologyData {
    pub needs: Vec<ron::Value>,
    pub morale_modifiers: Vec<ron::Value>,
    pub personality_traits: Vec<ron::Value>,
}

/// Tracks Maslow needs, morale, and personality per entity.
pub struct PsychologySystem {
    pub data: PsychologyData,
}

impl PsychologySystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("psychology.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(needs:[],morale_modifiers:[],personality_traits:[])".to_string()
        });
        let data: PsychologyData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse psychology.ron: {e}");
            PsychologyData { needs: vec![], morale_modifiers: vec![], personality_traits: vec![] }
        });
        log::info!("Loaded psychology data: {} needs, {} traits", data.needs.len(), data.personality_traits.len());
        Self { data }
    }
}

impl System for PsychologySystem {
    fn name(&self) -> &str {
        "PsychologySystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: implement need decay, morale calculation, and personality effects
    }
}

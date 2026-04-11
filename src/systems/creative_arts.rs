//! Creative arts system -- music, art creation, performances.
//!
//! Loads instruments, art tools, creative outputs, and performance types from
//! `data/creative_arts.ron`. Manages artistic activities and skill progression.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/creative_arts.ron`.
#[derive(Debug, Deserialize)]
pub struct CreativeArtsData {
    pub instruments: Vec<ron::Value>,
    pub art_tools: Vec<ron::Value>,
    pub outputs: Vec<ron::Value>,
    pub performances: Vec<ron::Value>,
}

/// Manages music instruments, art creation, and performances.
pub struct CreativeArtsSystem {
    pub data: CreativeArtsData,
}

impl CreativeArtsSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("creative_arts.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(instruments:[],art_tools:[],outputs:[],performances:[])".to_string()
        });
        let data: CreativeArtsData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse creative_arts.ron: {e}");
            CreativeArtsData { instruments: vec![], art_tools: vec![], outputs: vec![], performances: vec![] }
        });
        log::info!("Loaded creative arts data: {} instruments, {} performances", data.instruments.len(), data.performances.len());
        Self { data }
    }
}

impl System for CreativeArtsSystem {
    fn name(&self) -> &str { "CreativeArtsSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: manage artistic activities, skill progression, performance events
    }
}

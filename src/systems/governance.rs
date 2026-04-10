//! Governance system -- laws, civic roles, voting, and dispute resolution.
//!
//! Loads government types, laws, civic roles, and dispute resolution methods
//! from `data/governance.ron`. Tracks settlement governance state.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/governance.ron`.
#[derive(Debug, Deserialize)]
pub struct GovernanceData {
    pub government_types: Vec<ron::Value>,
    pub laws: Vec<ron::Value>,
    pub civic_roles: Vec<ron::Value>,
    pub dispute_resolution: Vec<ron::Value>,
}

/// Tracks laws, roles, votes, and settlement governance.
pub struct GovernanceSystem {
    pub data: GovernanceData,
}

impl GovernanceSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("governance.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(government_types:[],laws:[],civic_roles:[],dispute_resolution:[])".to_string()
        });
        let data: GovernanceData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse governance.ron: {e}");
            GovernanceData { government_types: vec![], laws: vec![], civic_roles: vec![], dispute_resolution: vec![] }
        });
        log::info!("Loaded governance data: {} gov types, {} laws", data.government_types.len(), data.laws.len());
        Self { data }
    }
}

impl System for GovernanceSystem {
    fn name(&self) -> &str {
        "GovernanceSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: implement law enforcement, voting, and governance simulation
    }
}

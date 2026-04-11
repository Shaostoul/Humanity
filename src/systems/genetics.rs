//! Genetics system -- breeding, traits, mutations, diseases.
//!
//! Loads trait definitions, mutation rates, and disease vectors from
//! `data/genetics.ron`. Manages heredity, trait expression, and genetic disorders.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/genetics.ron`.
#[derive(Debug, Deserialize)]
pub struct GeneticsData {
    pub traits: Vec<ron::Value>,
    pub mutations: Vec<ron::Value>,
    pub diseases: Vec<ron::Value>,
}

/// Manages breeding, trait inheritance, mutations, and genetic diseases.
pub struct GeneticsSystem {
    pub data: GeneticsData,
}

impl GeneticsSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("genetics.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(traits:[],mutations:[],diseases:[])".to_string()
        });
        let data: GeneticsData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse genetics.ron: {e}");
            GeneticsData { traits: vec![], mutations: vec![], diseases: vec![] }
        });
        log::info!("Loaded genetics data: {} traits, {} mutations", data.traits.len(), data.mutations.len());
        Self { data }
    }
}

impl System for GeneticsSystem {
    fn name(&self) -> &str { "GeneticsSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: process breeding events, apply trait inheritance, trigger mutations
    }
}

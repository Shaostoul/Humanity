//! Waste management system — accumulates waste from `WasteSource` entities
//! into `WasteAccumulator` entities (rooms, bins, dumpsters).
//!
//! Pairing logic: each `WasteSource` deposits into the **nearest**
//! `WasteAccumulator` within `MAX_DEPOSIT_DIST` meters. If no accumulator
//! is in range, the waste is logged as "atmospheric" pollution (drops to
//! the ecosystem — handled by EcologySystem if present).

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::{Transform, WasteAccumulator, WasteSource};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Max distance (meters) a WasteSource will travel to deposit into an
/// accumulator. Beyond this, waste is treated as ambient pollution.
const MAX_DEPOSIT_DIST: f32 = 8.0;

/// 1 game day = 1200 real seconds.
const REAL_SECONDS_PER_GAME_DAY: f32 = 1200.0;

/// Top-level RON schema for `data/waste_management.ron`.
#[derive(Debug, Deserialize)]
pub struct WasteData {
    #[serde(default)] pub waste_categories: Vec<ron::Value>,
    #[serde(default)] pub recycling: Vec<ron::Value>,
    #[serde(default)] pub pollution: Vec<ron::Value>,
}

/// Manages waste accumulation, recycling, and pollution tracking.
pub struct WasteSystem {
    pub data: WasteData,
    /// Cumulative atmospheric pollution (kg) — waste with no nearby accumulator.
    /// Read by EcologySystem to degrade biome health.
    pub atmospheric_pollution: f32,
}

impl WasteSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("waste_management.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(waste_categories:[],recycling:[],pollution:[])".to_string()
        });
        let data: WasteData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse waste_management.ron: {e}");
            WasteData { waste_categories: vec![], recycling: vec![], pollution: vec![] }
        });
        log::info!("Loaded waste data: {} categories, {} recycling methods",
            data.waste_categories.len(), data.recycling.len());
        Self { data, atmospheric_pollution: 0.0 }
    }
}

impl System for WasteSystem {
    fn name(&self) -> &str { "WasteSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let day_fraction = dt / REAL_SECONDS_PER_GAME_DAY;
        if day_fraction <= 0.0 { return; }

        // Snapshot accumulator positions so we can mutate accumulators below
        // without alias issues.
        let accumulator_positions: Vec<(hecs::Entity, glam::Vec3)> = world
            .query::<(&Transform, &WasteAccumulator)>()
            .iter()
            .map(|(e, (t, _))| (e, t.position))
            .collect();

        // For each WasteSource, compute deposit and find nearest accumulator.
        // We collect (target_entity_or_none, category, kg) pairs first to
        // avoid borrow checker issues with concurrent mutation.
        let mut deposits: Vec<(Option<hecs::Entity>, String, f32)> = Vec::new();
        for (_e, (src, src_t)) in world.query::<(&WasteSource, &Transform)>().iter() {
            let kg = src.rate_per_day * day_fraction;
            if kg <= 0.0 { continue; }

            // Find nearest accumulator within MAX_DEPOSIT_DIST.
            let max_d2 = MAX_DEPOSIT_DIST * MAX_DEPOSIT_DIST;
            let mut best: Option<(hecs::Entity, f32)> = None;
            for (acc_entity, acc_pos) in &accumulator_positions {
                let d2 = src_t.position.distance_squared(*acc_pos);
                if d2 > max_d2 { continue; }
                if best.map(|(_, b)| d2 < b).unwrap_or(true) {
                    best = Some((*acc_entity, d2));
                }
            }
            deposits.push((best.map(|(e, _)| e), src.category.clone(), kg));
        }

        // Apply deposits.
        for (target, category, kg) in deposits {
            match target {
                Some(entity) => {
                    if let Ok(mut acc) = world.get::<&mut WasteAccumulator>(entity) {
                        *acc.by_category.entry(category).or_insert(0.0) += kg;
                    }
                }
                None => {
                    self.atmospheric_pollution += kg;
                }
            }
        }
    }
}

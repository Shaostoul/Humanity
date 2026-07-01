//! Manufacturing system — drives `ProductionFacility` entities through their
//! recipes. Each tick advances `progress` by `dt / (days_per_unit * SECONDS_PER_DAY)`.
//! When progress hits 1.0, `output_count` increments and progress resets.
//!
//! For now this is the minimal "factory ticks out widgets over time" loop.
//! Future expansion (per `data/manufacturing.ron`):
//!   - assembly chains (multi-stage facilities)
//!   - quality checks based on operator skill
//!   - waste byproduct emission (feeds WasteSystem)

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::ProductionFacility;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// 1 game day = 1200 real seconds.
const REAL_SECONDS_PER_GAME_DAY: f32 = 1200.0;

/// Top-level RON schema for `data/manufacturing.ron`.
/// (Field name is `production_stages` in the data file, not `stages`.)
#[derive(Debug, Deserialize)]
pub struct ManufacturingData {
    #[serde(default)] pub production_stages: Vec<ron::Value>,
    #[serde(default)] pub assembly_lines: Vec<ron::Value>,
    #[serde(default)] pub quality_levels: Vec<ron::Value>,
    #[serde(default)] pub waste_products: Vec<ron::Value>,
}

/// Manages production stages, assembly lines, and quality control.
pub struct ManufacturingSystem {
    pub data: ManufacturingData,
    /// Total units produced across all facilities since the system started
    /// (lifetime counter — useful for stats / civilization metrics).
    pub lifetime_units_produced: u64,
}

impl ManufacturingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("manufacturing.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(production_stages:[],assembly_lines:[],quality_levels:[],waste_products:[])".to_string()
        });
        let data: ManufacturingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse manufacturing.ron: {e}");
            ManufacturingData {
                production_stages: vec![],
                assembly_lines: vec![],
                quality_levels: vec![],
                waste_products: vec![],
            }
        });
        log::info!(
            "Loaded manufacturing data: {} stages, {} assembly lines",
            data.production_stages.len(), data.assembly_lines.len()
        );
        Self { data, lifetime_units_produced: 0 }
    }
}

impl System for ManufacturingSystem {
    fn name(&self) -> &str { "ManufacturingSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        if dt <= 0.0 { return; }
        // Production timers run on GAME time (v0.663): "accelerated for testing"
        // speeds facilities too. Absent game_time (unit tests) = raw dt.
        let day_fraction = crate::systems::time::scaled_dt(dt, data) / REAL_SECONDS_PER_GAME_DAY;

        let mut completions: Vec<(hecs::Entity, String, u32)> = Vec::new();

        for (entity, facility) in world.query_mut::<&mut ProductionFacility>() {
            if !facility.running { continue; }
            if facility.days_per_unit <= 0.0 { continue; }

            facility.progress += day_fraction / facility.days_per_unit;
            // Multiple units may complete in a single tick at very high time_scale.
            while facility.progress >= 1.0 {
                facility.progress -= 1.0;
                facility.output_count += 1;
                completions.push((entity, facility.recipe_id.clone(), facility.output_count));
            }
        }

        for (entity, recipe, count) in completions {
            log::debug!(
                "Manufacturing: facility {:?} completed unit {} of recipe '{}'",
                entity, count, recipe
            );
            self.lifetime_units_produced += 1;
        }
    }
}

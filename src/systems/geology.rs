//! Geology system — ore vein depletion tracking, soil fertility evolution,
//! and tectonic event triggering.
//!
//! Per-tick behavior:
//!   1. Soil patches slowly recover fertility toward 1.0 (organic decomposition).
//!      Faster recovery near `WasteAccumulator` containing organic waste.
//!   2. Ore deposits don't deplete on their own — that's driven by mining
//!      interactions. We expose `extract_ore()` for those handlers.
//!   3. Tectonic events are stub — would roll a per-day chance from
//!      `data/geology.ron::tectonic_events`. Future expansion.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::{OreDeposit, SoilPatch, Transform, WasteAccumulator};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// 1 game day = 1200 real seconds.
const REAL_SECONDS_PER_GAME_DAY: f32 = 1200.0;
/// Baseline fertility recovery per game-day (toward 1.0).
const FERTILITY_RECOVERY_PER_DAY: f32 = 0.005;
/// Bonus recovery per kg of organic waste within range.
const ORGANIC_BONUS_PER_KG_PER_DAY: f32 = 0.001;
/// Distance from a soil patch where a WasteAccumulator's organic content fertilizes it.
const ORGANIC_RANGE: f32 = 6.0;

/// Top-level RON schema for `data/geology.ron`.
#[derive(Debug, Deserialize)]
pub struct GeologyData {
    #[serde(default)] pub rock_types: Vec<ron::Value>,
    #[serde(default)] pub ore_veins: Vec<ron::Value>,
    #[serde(default)] pub soil_types: Vec<ron::Value>,
    #[serde(default)] pub tectonic_events: Vec<ron::Value>,
}

/// Manages rock types, ore veins, soil composition, and tectonic events.
pub struct GeologySystem {
    pub data: GeologyData,
    /// Total kg extracted across all deposits since startup (lifetime stat).
    pub lifetime_kg_extracted: f64,
}

impl GeologySystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("geology.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(rock_types:[],ore_veins:[],soil_types:[],tectonic_events:[])".to_string()
        });
        let data: GeologyData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse geology.ron: {e}");
            GeologyData { rock_types: vec![], ore_veins: vec![], soil_types: vec![], tectonic_events: vec![] }
        });
        log::info!("Loaded geology data: {} rock types, {} ore veins", data.rock_types.len(), data.ore_veins.len());
        Self { data, lifetime_kg_extracted: 0.0 }
    }

    /// Mining interaction handler: extract `kg` from a deposit. Returns the
    /// actual amount extracted (may be less if deposit is depleted).
    pub fn extract_ore(world: &mut hecs::World, entity: hecs::Entity, kg: f32) -> f32 {
        let mut extracted = 0.0;
        if let Ok(mut deposit) = world.get::<&mut OreDeposit>(entity) {
            extracted = deposit.yield_remaining.min(kg);
            deposit.yield_remaining -= extracted;
        }
        extracted
    }
}

impl System for GeologySystem {
    fn name(&self) -> &str { "GeologySystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let day_fraction = dt / REAL_SECONDS_PER_GAME_DAY;
        if day_fraction <= 0.0 { return; }

        // Snapshot organic-waste sources (entity_pos, kg of "organic" category).
        let organic_sources: Vec<(glam::Vec3, f32)> = world
            .query::<(&Transform, &WasteAccumulator)>()
            .iter()
            .filter_map(|(_, (t, acc))| {
                let kg = acc.by_category.get("organic").copied().unwrap_or(0.0);
                if kg > 0.0 { Some((t.position, kg)) } else { None }
            })
            .collect();

        // Snapshot soil patch positions.
        let soils: Vec<(hecs::Entity, glam::Vec3)> = world
            .query::<(&Transform, &SoilPatch)>()
            .iter()
            .map(|(e, (t, _))| (e, t.position))
            .collect();

        for (entity, soil_pos) in soils {
            // Compute organic bonus from nearby waste.
            let bonus_per_day: f32 = organic_sources
                .iter()
                .filter(|(p, _)| p.distance(soil_pos) <= ORGANIC_RANGE)
                .map(|(_, kg)| kg * ORGANIC_BONUS_PER_KG_PER_DAY)
                .sum();

            let total_recovery = (FERTILITY_RECOVERY_PER_DAY + bonus_per_day) * day_fraction;
            if let Ok(mut soil) = world.get::<&mut SoilPatch>(entity) {
                soil.fertility = (soil.fertility + total_recovery).min(1.0);
            }
        }
    }
}

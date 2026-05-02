//! Oceanography system — regenerates marine resource populations toward
//! their carrying capacity. Logistic-growth model: regen rate scales with
//! `current * (1 - current/carrying_capacity)` so populations recover fast
//! at moderate levels and slow down as they approach cap.
//!
//! Harvest interactions decrement `current`; this system never depletes
//! anything on its own.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::MarineResource;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// 1 game day = 1200 real seconds.
const REAL_SECONDS_PER_GAME_DAY: f32 = 1200.0;

/// Top-level RON schema for `data/oceanography.ron`.
#[derive(Debug, Deserialize)]
pub struct OceanographyData {
    #[serde(default)] pub zones: Vec<ron::Value>,
    #[serde(default)] pub currents: Vec<ron::Value>,
    #[serde(default)] pub marine_resources: Vec<ron::Value>,
}

/// Manages ocean zones, currents, and marine resources.
pub struct OceanographySystem {
    pub data: OceanographyData,
    /// Total kg regrown across all populations since startup (lifetime stat).
    pub lifetime_kg_regrown: f64,
}

impl OceanographySystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("oceanography.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(zones:[],currents:[],marine_resources:[])".to_string()
        });
        let data: OceanographyData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse oceanography.ron: {e}");
            OceanographyData { zones: vec![], currents: vec![], marine_resources: vec![] }
        });
        log::info!("Loaded oceanography data: {} zones, {} currents", data.zones.len(), data.currents.len());
        Self { data, lifetime_kg_regrown: 0.0 }
    }

    /// Harvest interaction handler: extract `kg` from a marine resource.
    /// Returns the actual amount extracted (may be less if depleted).
    pub fn harvest(world: &mut hecs::World, entity: hecs::Entity, kg: f32) -> f32 {
        let mut extracted = 0.0;
        if let Ok(mut res) = world.get::<&mut MarineResource>(entity) {
            extracted = res.current.min(kg);
            res.current -= extracted;
        }
        extracted
    }
}

impl System for OceanographySystem {
    fn name(&self) -> &str { "OceanographySystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let day_fraction = dt / REAL_SECONDS_PER_GAME_DAY;
        if day_fraction <= 0.0 { return; }

        let mut total_grown = 0.0_f32;
        for (_, res) in world.query_mut::<&mut MarineResource>() {
            if res.current >= res.carrying_capacity || res.carrying_capacity <= 0.0 {
                continue;
            }
            // Logistic growth: dN/dt = r * N * (1 - N/K).
            let r = res.regen_per_day;
            let n = res.current.max(0.01);  // avoid zero-stuck at empty population
            let k = res.carrying_capacity;
            let growth = r * n * (1.0 - n / k) * day_fraction;
            res.current = (res.current + growth).clamp(0.0, k);
            total_grown += growth;
        }

        self.lifetime_kg_regrown += total_grown as f64;
    }
}

//! Farming system — crop growth simulation driven by time, water, and plant data.
//!
//! Queries all entities with `CropInstance` and advances growth stages.
//! Plant definitions loaded from `data/plants.csv`.

pub mod crops;
pub mod soil;
pub mod automation;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::components::{CropInstance, GrowthStage};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Plant definition loaded from plants.csv — cached in DataStore as "plant_registry".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantDef {
    /// Unique plant ID (e.g., "tomato").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Total real-world days from seed to harvest.
    pub growth_days: f32,
    /// Water consumption in liters per day per plant.
    pub water_per_day: f32,
    /// Preferred growing seasons.
    pub seasons: Vec<String>,
}

/// Registry of all plant definitions, keyed by plant ID.
#[derive(Debug, Clone, Default)]
pub struct PlantRegistry {
    pub plants: HashMap<String, PlantDef>,
}

impl PlantRegistry {
    /// Look up a plant definition by ID.
    pub fn get(&self, id: &str) -> Option<&PlantDef> {
        self.plants.get(id)
    }
}

/// Rate at which water_level decreases per second (base dehydration).
const DEHYDRATION_RATE: f32 = 0.002;

/// Water level below which crop health starts dropping.
const WATER_STRESS_THRESHOLD: f32 = 0.2;

/// Health recovery rate per second when well-watered.
const HEALTH_RECOVERY_RATE: f32 = 0.5;

/// Health decay rate per second when water-stressed.
const HEALTH_DECAY_RATE: f32 = 1.0;

/// Seconds per in-game day (must match time system).
const SECONDS_PER_DAY: f64 = 1200.0;

/// Growth stage thresholds as fractions of total growth time.
/// Seed: 0.0-0.05, Sprout: 0.05-0.15, Vegetative: 0.15-0.45,
/// Flowering: 0.45-0.65, Fruiting: 0.65-0.95, Harvest: 0.95+
const STAGE_THRESHOLDS: [(GrowthStage, f32); 6] = [
    (GrowthStage::Seed, 0.0),
    (GrowthStage::Sprout, 0.05),
    (GrowthStage::Vegetative, 0.15),
    (GrowthStage::Flowering, 0.45),
    (GrowthStage::Fruiting, 0.65),
    (GrowthStage::Harvest, 0.95),
];

/// Simulates crop growth based on elapsed time and environmental factors.
pub struct FarmingSystem {
    _initialized: bool,
}

impl FarmingSystem {
    pub fn new() -> Self {
        Self {
            _initialized: false,
        }
    }

    /// Determine growth stage from progress fraction (0.0 to 1.0+).
    fn stage_from_progress(progress: f32) -> GrowthStage {
        let mut current_stage = GrowthStage::Seed;
        for &(stage, threshold) in &STAGE_THRESHOLDS {
            if progress >= threshold {
                current_stage = stage;
            } else {
                break;
            }
        }
        current_stage
    }
}

impl System for FarmingSystem {
    fn name(&self) -> &str {
        "FarmingSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let plant_registry = data.get::<PlantRegistry>("plant_registry");

        // Get current elapsed time from TimeSystem's GameTime if available
        let elapsed_seconds = data
            .get::<crate::systems::time::GameTime>("game_time")
            .map(|gt| gt.elapsed_seconds)
            .unwrap_or(0.0);

        // Collect entities to update (avoid borrow conflict with world)
        let mut updates: Vec<(hecs::Entity, CropInstance)> = Vec::new();

        for (entity, crop) in world.query_mut::<&CropInstance>() {
            // Skip dead crops
            if crop.growth_stage == GrowthStage::Dead {
                continue;
            }
            // Skip already-harvestable crops (they sit until harvested)
            if crop.growth_stage == GrowthStage::Harvest {
                continue;
            }

            let mut crop = crop.clone();

            // Dehydration: water level drops over time
            crop.water_level = (crop.water_level - DEHYDRATION_RATE * dt).max(0.0);

            // Health effects from water level
            if crop.water_level < WATER_STRESS_THRESHOLD {
                // Water stress — health decays
                crop.health = (crop.health - HEALTH_DECAY_RATE * dt).max(0.0);
            } else {
                // Well watered — health recovers toward 100
                crop.health = (crop.health + HEALTH_RECOVERY_RATE * dt).min(100.0);
            }

            // If health hits zero, crop dies
            if crop.health <= 0.0 {
                crop.growth_stage = GrowthStage::Dead;
                updates.push((entity, crop));
                continue;
            }

            // Calculate growth progress based on elapsed time since planting
            if let Some(registry) = plant_registry {
                if let Some(plant_def) = registry.get(&crop.crop_def_id) {
                    // Total growth time in game seconds
                    let growth_seconds = plant_def.growth_days as f64 * SECONDS_PER_DAY;

                    if growth_seconds > 0.0 {
                        let age = elapsed_seconds - crop.planted_at;
                        let progress = (age / growth_seconds) as f32;

                        // Health-weighted progress: unhealthy crops grow slower
                        let health_factor = (crop.health / 100.0).max(0.1);
                        let effective_progress = progress * health_factor;

                        let new_stage = Self::stage_from_progress(effective_progress);

                        // Only advance forward, never regress (except to Dead)
                        if new_stage as u8 > crop.growth_stage as u8 {
                            crop.growth_stage = new_stage;
                            log::debug!(
                                "Crop {} advanced to {:?}",
                                crop.crop_def_id,
                                crop.growth_stage
                            );
                        }
                    }
                }
            }

            updates.push((entity, crop));
        }

        // Apply updates back to the world
        for (entity, crop) in updates {
            if let Ok(mut existing) = world.get::<&mut CropInstance>(entity) {
                *existing = crop;
            }
        }

        self._initialized = true;
    }
}

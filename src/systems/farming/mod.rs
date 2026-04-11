//! Farming system -- crop growth simulation driven by time, water, and plant data.
//!
//! Queries all entities with `CropInstance` and advances growth stages.
//! Plant definitions loaded from `data/plants.csv`.
//! Growth stages are data-driven: each plant species defines its own stage
//! names in plants.csv (colon-separated). Default stages are used when missing.

pub mod crops;
pub mod soil;
pub mod automation;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ecs::components::{CropInstance, DEFAULT_GROWTH_STAGES, STAGE_DEAD};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Plant definition loaded from plants.csv -- cached in DataStore as "plant_registry".
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
    /// Ordered growth stage names for this plant species.
    /// Loaded from plants.csv `growth_stages` column (colon-separated).
    /// Falls back to DEFAULT_GROWTH_STAGES when empty.
    pub growth_stages: Vec<String>,
}

impl PlantDef {
    /// Returns this plant's growth stages, falling back to defaults if empty.
    pub fn stages(&self) -> Vec<&str> {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES.iter().copied().collect()
        } else {
            self.growth_stages.iter().map(|s| s.as_str()).collect()
        }
    }

    /// Returns the first stage name (the initial stage when planted).
    pub fn first_stage(&self) -> &str {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES[0]
        } else {
            &self.growth_stages[0]
        }
    }

    /// Returns the last stage name (the harvest-ready stage).
    pub fn last_stage(&self) -> &str {
        if self.growth_stages.is_empty() {
            DEFAULT_GROWTH_STAGES[DEFAULT_GROWTH_STAGES.len() - 1]
        } else {
            &self.growth_stages[self.growth_stages.len() - 1]
        }
    }
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

/// Determine growth stage from progress fraction (0.0 to 1.0+) using
/// a data-driven stage list. Stages are evenly distributed across the
/// 0.0-1.0 range unless custom thresholds are added later.
fn stage_from_progress<'a>(progress: f32, stages: &'a [&'a str]) -> &'a str {
    if stages.is_empty() {
        return DEFAULT_GROWTH_STAGES[0];
    }
    let n = stages.len();
    // Each stage occupies an equal fraction of the 0.0-1.0 range.
    // stage[i] starts at i/n and runs until (i+1)/n.
    let idx = ((progress * n as f32).floor() as usize).min(n - 1);
    stages[idx]
}

/// Returns the index of a stage name in the stage list, or None if not found.
fn stage_index(stage: &str, stages: &[&str]) -> Option<usize> {
    stages.iter().position(|s| *s == stage)
}

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

        // Build default stages vec once for plants without custom stages
        let default_stages: Vec<&str> = DEFAULT_GROWTH_STAGES.iter().copied().collect();

        // Collect entities to update (avoid borrow conflict with world)
        let mut updates: Vec<(hecs::Entity, CropInstance)> = Vec::new();

        for (entity, crop) in world.query_mut::<&CropInstance>() {
            // Skip dead crops
            if crop.growth_stage == STAGE_DEAD {
                continue;
            }

            // Resolve this plant's stage list
            let plant_stages: Vec<&str> = plant_registry
                .as_ref()
                .and_then(|reg| reg.get(&crop.crop_def_id))
                .map(|def| def.stages())
                .unwrap_or_else(|| default_stages.clone());

            // Skip crops already at their final stage (they sit until harvested)
            if let Some(last) = plant_stages.last() {
                if crop.growth_stage == *last {
                    continue;
                }
            }

            let mut crop = crop.clone();

            // Dehydration: water level drops over time
            crop.water_level = (crop.water_level - DEHYDRATION_RATE * dt).max(0.0);

            // Health effects from water level
            if crop.water_level < WATER_STRESS_THRESHOLD {
                // Water stress -- health decays
                crop.health = (crop.health - HEALTH_DECAY_RATE * dt).max(0.0);
            } else {
                // Well watered -- health recovers toward 100
                crop.health = (crop.health + HEALTH_RECOVERY_RATE * dt).min(100.0);
            }

            // If health hits zero, crop dies
            if crop.health <= 0.0 {
                crop.growth_stage = STAGE_DEAD.to_string();
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

                        let new_stage =
                            stage_from_progress(effective_progress, &plant_stages);

                        // Only advance forward, never regress (except to Dead)
                        let current_idx = stage_index(&crop.growth_stage, &plant_stages);
                        let new_idx = stage_index(new_stage, &plant_stages);

                        if let (Some(cur), Some(nxt)) = (current_idx, new_idx) {
                            if nxt > cur {
                                crop.growth_stage = new_stage.to_string();
                                log::debug!(
                                    "Crop {} advanced to {}",
                                    crop.crop_def_id,
                                    crop.growth_stage
                                );
                            }
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

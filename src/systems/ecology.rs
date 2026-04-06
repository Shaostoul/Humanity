//! Ecology system — simple ecosystem simulation with disease, population tracking, and seasons.
//!
//! Simulates disease spread between nearby entities, natural recovery, seasonal growth
//! modifiers, and population monitoring by species. Reads `GameTime` from DataStore
//! for seasonal effects.

use std::collections::HashMap;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::ecs::components::{Health, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::time::{GameTime, Season};

// ── Components ──────────────────────────────────────────────

/// Disease affecting an entity — reduces health over time and can spread to nearby entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disease {
    /// Identifier for the disease type (e.g., "blight", "flu").
    pub disease_id: String,
    /// How severe the disease is (0.0 = negligible, 1.0 = critical).
    pub severity: f32,
    /// Seconds remaining before symptoms manifest and health starts dropping.
    pub incubation: f32,
    /// Whether this disease can spread to nearby entities.
    pub contagious: bool,
}

/// Species tag for population tracking — attach to any living entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Species {
    /// Species identifier (e.g., "cow", "chicken", "wolf", "oak_tree").
    pub species_id: String,
}

// ── Constants ───────────────────────────────────────────────

/// Max distance (meters) for disease transmission between entities.
const CONTAGION_RADIUS: f32 = 5.0;

/// Base probability per second that a contagious entity transmits to a neighbor.
/// Scaled by the source's severity.
const BASE_TRANSMISSION_RATE: f32 = 0.05;

/// Health damage per second per unit of severity (after incubation).
const DISEASE_DAMAGE_RATE: f32 = 8.0;

/// Natural recovery rate: severity decreases by this much per second.
const NATURAL_RECOVERY_RATE: f32 = 0.01;

/// Severity threshold below which the disease is cured and removed.
const CURE_THRESHOLD: f32 = 0.01;

/// How often (in seconds) to log population warnings. Prevents log spam.
const POPULATION_LOG_INTERVAL: f32 = 10.0;

// ── Seasonal multipliers ────────────────────────────────────

/// Growth rate multiplier by season (affects farming indirectly via DataStore).
fn seasonal_growth_multiplier(season: Season) -> f32 {
    match season {
        Season::Spring => 1.2,
        Season::Summer => 1.5,
        Season::Autumn => 0.8,
        Season::Winter => 0.3,
    }
}

/// Disease spread multiplier by season — diseases spread faster in winter.
fn seasonal_disease_multiplier(season: Season) -> f32 {
    match season {
        Season::Spring => 1.0,
        Season::Summer => 0.7,
        Season::Autumn => 1.2,
        Season::Winter => 1.8,
    }
}

// ── System ──────────────────────────────────────────────────

/// Simulates disease spread, recovery, population tracking, and seasonal effects.
pub struct EcologySystem {
    /// Timer for throttling population log warnings.
    population_log_timer: f32,
    /// Cached population counts from last check.
    population_counts: HashMap<String, u32>,
}

impl EcologySystem {
    pub fn new() -> Self {
        Self {
            population_log_timer: 0.0,
            population_counts: HashMap::new(),
        }
    }
}

impl System for EcologySystem {
    fn name(&self) -> &str {
        "EcologySystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Read current season from GameTime in DataStore
        let season = data
            .get::<GameTime>("game_time")
            .map(|gt| gt.season)
            .unwrap_or(Season::Spring);

        let disease_mult = seasonal_disease_multiplier(season);

        // Store seasonal growth multiplier in DataStore for other systems to read
        // (DataStore requires &mut, but we only have &DataStore — systems that need
        // this can read GameTime directly instead. We document the multiplier here.)
        let _growth_mult = seasonal_growth_multiplier(season);

        // ── Phase 1: Disease progression and health damage ──────────
        // Collect entities with Disease to update (avoids borrow conflicts).
        let mut disease_updates: Vec<(hecs::Entity, Disease, f32)> = Vec::new();
        let mut diseases_to_remove: Vec<hecs::Entity> = Vec::new();

        for (entity, (disease, health)) in world.query_mut::<(&Disease, &Health)>() {
            let mut disease = disease.clone();
            let mut health_delta: f32 = 0.0;

            // Tick incubation
            if disease.incubation > 0.0 {
                disease.incubation = (disease.incubation - dt).max(0.0);
            } else {
                // Incubation over — disease damages health based on severity
                health_delta = -DISEASE_DAMAGE_RATE * disease.severity * dt;
            }

            // Natural recovery: severity decreases over time
            disease.severity = (disease.severity - NATURAL_RECOVERY_RATE * dt).max(0.0);

            if disease.severity < CURE_THRESHOLD {
                // Cured — mark for removal
                diseases_to_remove.push(entity);
            } else {
                disease_updates.push((entity, disease, health_delta));
            }
        }

        // Apply disease updates
        for (entity, disease, health_delta) in disease_updates {
            if let Ok(mut d) = world.get::<&mut Disease>(entity) {
                *d = disease;
            }
            if health_delta != 0.0 {
                if let Ok(mut h) = world.get::<&mut Health>(entity) {
                    h.current = (h.current + health_delta).clamp(0.0, h.max);
                }
            }
        }

        // Remove cured diseases
        for entity in diseases_to_remove {
            let _ = world.remove_one::<Disease>(entity);
            log::debug!("Entity {:?}: disease cured", entity);
        }

        // ── Phase 2: Disease transmission ───────────────────────────
        // Collect all contagious sources with their positions.
        let mut contagious_sources: Vec<(Vec3, f32, String)> = Vec::new();

        for (_entity, (transform, disease)) in world.query_mut::<(&Transform, &Disease)>() {
            if disease.contagious && disease.incubation <= 0.0 {
                contagious_sources.push((
                    transform.position,
                    disease.severity,
                    disease.disease_id.clone(),
                ));
            }
        }

        // Find entities near contagious sources that don't already have Disease.
        // We collect candidates first, then add Disease to them.
        let mut infection_candidates: Vec<(hecs::Entity, String, f32)> = Vec::new();

        // Collect healthy entity positions first to avoid borrow conflict
        let healthy_entities: Vec<(hecs::Entity, glam::Vec3)> = world
            .query_mut::<(&Transform, &Health)>()
            .into_iter()
            .map(|(e, (t, _))| (e, t.position))
            .collect();

        for (entity, pos) in &healthy_entities {
            // Skip entities that already have a disease
            if world.get::<&Disease>(*entity).is_ok() {
                continue;
            }

            for (source_pos, severity, disease_id) in &contagious_sources {
                let dist = pos.distance(*source_pos);
                if dist < CONTAGION_RADIUS && dist > 0.001 {
                    // Probability scales with severity, season, and inverse distance
                    let distance_factor = 1.0 - (dist / CONTAGION_RADIUS);
                    let chance = BASE_TRANSMISSION_RATE * severity * disease_mult * distance_factor * dt;

                    // Pseudo-random check using entity ID bits and frame position
                    let hash = (entity.id() as f32 * 0.618) % 1.0;
                    if hash < chance {
                        infection_candidates.push((*entity, disease_id.clone(), severity * 0.8));
                        break; // Only one infection per tick
                    }
                }
            }
        }

        // Apply new infections
        for (entity, disease_id, severity) in infection_candidates {
            let new_disease = Disease {
                disease_id,
                severity,
                incubation: 5.0, // 5 second incubation for newly infected
                contagious: true,
            };
            let _ = world.insert_one(entity, new_disease);
            log::debug!("Entity {:?}: infected", entity);
        }

        // ── Phase 3: Population tracking ────────────────────────────
        self.population_log_timer += dt;

        if self.population_log_timer >= POPULATION_LOG_INTERVAL {
            self.population_log_timer = 0.0;
            self.population_counts.clear();

            for (_entity, species) in world.query_mut::<&Species>() {
                *self
                    .population_counts
                    .entry(species.species_id.clone())
                    .or_insert(0) += 1;
            }

            // Warn if any tracked species has gone extinct
            for (species_id, count) in &self.population_counts {
                if *count == 0 {
                    log::warn!(
                        "ECOLOGY: Species '{}' has gone extinct!",
                        species_id
                    );
                }
            }

            if !self.population_counts.is_empty() {
                log::debug!(
                    "Population census: {:?} (season: {:?})",
                    self.population_counts,
                    season,
                );
            }
        }
    }
}

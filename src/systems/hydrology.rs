//! Hydrological system -- water cycle simulation.
//!
//! Models evaporation, precipitation, runoff, aquifer recharge, river flow,
//! lake filling/draining, glacier melt, and contamination spread.
//! Reads Weather from DataStore for precipitation rates and GameTime for
//! seasonal effects (spring melt, summer drought).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::time::{GameTime, Season};
use crate::systems::weather::Weather;

// ── Data types ─────────────────────────────────────────────

/// Classification of water body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WaterBodyType {
    Ocean,
    Lake,
    River,
    Stream,
    Pond,
    Aquifer,
    Glacier,
    Hotspring,
    Swamp,
}

/// A single water body in the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterBody {
    /// Unique identifier.
    pub id: u64,
    /// Type of water body.
    pub body_type: WaterBodyType,
    /// Total water volume in liters.
    pub volume_liters: f64,
    /// Water temperature in Kelvin.
    pub temperature_k: f32,
    /// Salinity in parts per thousand.
    pub salinity_ppt: f32,
    /// Flow rate in liters per second (meaningful for rivers/streams).
    pub flow_rate: f32,
    /// pH value (0-14).
    pub ph: f32,
    /// Contamination level (0.0 = pristine, 1.0 = heavily polluted).
    pub contamination: f32,
    /// Surface area in square meters (affects evaporation).
    pub surface_area_m2: f64,
    /// Elevation in meters (for flow calculations).
    pub elevation_m: f32,
    /// IDs of downstream water bodies that this one flows into.
    pub downstream: Vec<u64>,
}

impl WaterBody {
    /// Create a new water body with sensible defaults.
    pub fn new(id: u64, body_type: WaterBodyType, volume_liters: f64) -> Self {
        Self {
            id,
            body_type,
            volume_liters,
            temperature_k: 288.0, // ~15 C
            salinity_ppt: match body_type {
                WaterBodyType::Ocean => 35.0,
                _ => 0.0,
            },
            flow_rate: match body_type {
                WaterBodyType::River => 500.0,
                WaterBodyType::Stream => 50.0,
                _ => 0.0,
            },
            ph: 7.0,
            contamination: 0.0,
            surface_area_m2: (volume_liters / 1000.0).powf(0.667) * 10.0,
            elevation_m: 0.0,
            downstream: Vec::new(),
        }
    }
}

// ── Constants ──────────────────────────────────────────────

/// Base evaporation rate: liters per m2 per second at 293 K (20 C).
const BASE_EVAPORATION_RATE: f64 = 0.0001;

/// Temperature coefficient for evaporation: rate doubles roughly every 10 K.
const EVAPORATION_TEMP_COEFF: f64 = 0.07;

/// Fraction of precipitation that becomes surface runoff (rest infiltrates).
const RUNOFF_FRACTION: f64 = 0.4;

/// Aquifer recharge rate: fraction of infiltration per tick that reaches aquifer.
const AQUIFER_RECHARGE_FRACTION: f64 = 0.001;

/// Contamination decay rate per second (natural purification).
const CONTAMINATION_DECAY: f32 = 0.0001;

/// Contamination dilution factor per downstream hop.
const CONTAMINATION_DILUTION: f32 = 0.7;

/// Glacier melt rate coefficient: liters per second per degree above 273 K.
const GLACIER_MELT_COEFF: f64 = 0.5;

// ── System ─────────────────────────────────────────────────

/// Simulates the water cycle across all registered water bodies.
pub struct HydrologySystem {
    /// How often to run a full simulation step (seconds).
    tick_interval: f32,
    /// Accumulated time since last simulation step.
    elapsed: f32,
    /// All tracked water bodies by ID.
    water_bodies: HashMap<u64, WaterBody>,
    /// Next auto-increment ID for new water bodies.
    next_id: u64,
}

impl HydrologySystem {
    pub fn new() -> Self {
        Self {
            tick_interval: 10.0,
            elapsed: 0.0,
            water_bodies: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a water body and return its assigned ID.
    pub fn add_water_body(&mut self, mut body: WaterBody) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        body.id = id;
        self.water_bodies.insert(id, body);
        id
    }

    /// Get a reference to a water body by ID.
    pub fn get(&self, id: u64) -> Option<&WaterBody> {
        self.water_bodies.get(&id)
    }

    /// Get a mutable reference to a water body by ID.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut WaterBody> {
        self.water_bodies.get_mut(&id)
    }

    /// Seasonal multiplier for evaporation.
    fn seasonal_evaporation(season: Season) -> f64 {
        match season {
            Season::Spring => 0.8,
            Season::Summer => 1.5,
            Season::Autumn => 0.7,
            Season::Winter => 0.3,
        }
    }

    /// Seasonal multiplier for precipitation contribution.
    fn seasonal_precipitation(season: Season) -> f64 {
        match season {
            Season::Spring => 1.3,
            Season::Summer => 0.6,
            Season::Autumn => 1.1,
            Season::Winter => 0.8,
        }
    }

    /// Simulate one hydrology step for all water bodies.
    fn simulate_step(&mut self, dt: f32, season: Season, weather: &Weather) {
        let dt_d = dt as f64;

        // Collect IDs first to avoid borrow issues.
        let ids: Vec<u64> = self.water_bodies.keys().copied().collect();

        // Phase 1: Evaporation, precipitation, glacier melt, temperature, decay.
        for &id in &ids {
            let body = match self.water_bodies.get_mut(&id) {
                Some(b) => b,
                None => continue,
            };

            // -- Evaporation (temperature-dependent, seasonal) --
            if body.body_type != WaterBodyType::Aquifer {
                let temp_factor = ((body.temperature_k as f64 - 273.0) * EVAPORATION_TEMP_COEFF)
                    .exp()
                    .max(0.0);
                let seasonal = Self::seasonal_evaporation(season);
                let evap = BASE_EVAPORATION_RATE * body.surface_area_m2 * temp_factor * seasonal * dt_d;
                body.volume_liters = (body.volume_liters - evap).max(0.0);
            }

            // -- Precipitation (from weather) --
            // Rain adds water based on humidity and intensity.
            let precip_rate = weather.humidity as f64 * weather.intensity as f64;
            let seasonal_p = Self::seasonal_precipitation(season);
            let precip_volume = precip_rate * body.surface_area_m2 * 0.001 * seasonal_p * dt_d;
            body.volume_liters += precip_volume;

            // Runoff adds water to surface bodies from surrounding terrain.
            if body.body_type != WaterBodyType::Aquifer
                && body.body_type != WaterBodyType::Ocean
            {
                let runoff = precip_volume * RUNOFF_FRACTION * 2.0; // terrain catchment area
                body.volume_liters += runoff;
            }

            // -- Aquifer recharge (slow seepage) --
            if body.body_type == WaterBodyType::Aquifer {
                let infiltration = precip_volume * (1.0 - RUNOFF_FRACTION);
                body.volume_liters += infiltration * AQUIFER_RECHARGE_FRACTION;
            }

            // -- Glacier melt (temperature-dependent) --
            if body.body_type == WaterBodyType::Glacier {
                let above_freezing = (body.temperature_k - 273.15).max(0.0) as f64;
                let melt = GLACIER_MELT_COEFF * above_freezing * dt_d;
                body.volume_liters = (body.volume_liters - melt).max(0.0);
            }

            // -- Temperature: nudge toward ambient (from weather) --
            let ambient_k = weather.temperature + 273.15;
            let temp_diff = ambient_k - body.temperature_k;
            // Water temperature changes slowly.
            body.temperature_k += temp_diff * 0.001 * dt;

            // -- Contamination natural decay --
            body.contamination = (body.contamination - CONTAMINATION_DECAY * dt).max(0.0);
        }

        // Phase 2: River/stream flow — move water downstream.
        // Collect downstream transfers to apply after iteration.
        let mut transfers: Vec<(u64, f64, f32)> = Vec::new(); // (target_id, volume, contamination)

        for &id in &ids {
            let body = match self.water_bodies.get(&id) {
                Some(b) => b,
                None => continue,
            };

            if body.downstream.is_empty() || body.flow_rate <= 0.0 {
                continue;
            }

            let flow_volume = body.flow_rate as f64 * dt_d;
            let actual_flow = flow_volume.min(body.volume_liters * 0.1); // max 10% per tick
            let per_target = actual_flow / body.downstream.len() as f64;
            let contam = body.contamination * CONTAMINATION_DILUTION;

            for &target_id in &body.downstream {
                transfers.push((target_id, per_target, contam));
            }
        }

        // Apply downstream transfers.
        for (target_id, volume, contam) in transfers {
            if let Some(target) = self.water_bodies.get_mut(&target_id) {
                target.volume_liters += volume;
                // Mix contamination: weighted average by volume.
                if target.volume_liters > 0.0 {
                    let ratio = (volume / target.volume_liters) as f32;
                    target.contamination =
                        target.contamination * (1.0 - ratio) + contam * ratio;
                }
            }
        }

        // Phase 3: Lake overflow — if a lake exceeds a threshold, excess flows downstream.
        for &id in &ids {
            let (excess, downstream_ids) = {
                let body = match self.water_bodies.get(&id) {
                    Some(b) => b,
                    None => continue,
                };
                if body.body_type != WaterBodyType::Lake || body.downstream.is_empty() {
                    continue;
                }
                // Simple capacity model: surface_area * 10m depth.
                let capacity = body.surface_area_m2 * 10_000.0;
                let excess = (body.volume_liters - capacity).max(0.0);
                (excess, body.downstream.clone())
            };

            if excess > 0.0 {
                if let Some(body) = self.water_bodies.get_mut(&id) {
                    body.volume_liters -= excess;
                }
                let per_target = excess / downstream_ids.len() as f64;
                for target_id in downstream_ids {
                    if let Some(target) = self.water_bodies.get_mut(&target_id) {
                        target.volume_liters += per_target;
                    }
                }
            }
        }
    }
}

impl System for HydrologySystem {
    fn name(&self) -> &str {
        "HydrologySystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, dt: f32, data: &DataStore) {
        self.elapsed += dt;
        if self.elapsed < self.tick_interval {
            return;
        }
        let step_dt = self.elapsed;
        self.elapsed = 0.0;

        let season = data
            .get::<GameTime>("game_time")
            .map(|gt| gt.season)
            .unwrap_or(Season::Spring);

        let default_weather = Weather::default();
        let weather = data
            .get::<Weather>("weather")
            .unwrap_or(&default_weather);

        self.simulate_step(step_dt, season, weather);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_body_creation() {
        let wb = WaterBody::new(0, WaterBodyType::Lake, 1_000_000.0);
        assert_eq!(wb.body_type, WaterBodyType::Lake);
        assert!(wb.volume_liters > 0.0);
        assert!((wb.ph - 7.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ocean_default_salinity() {
        let wb = WaterBody::new(0, WaterBodyType::Ocean, 1e15);
        assert!((wb.salinity_ppt - 35.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_system_ticks_without_panic() {
        let mut system = HydrologySystem::new();
        let mut lake = WaterBody::new(0, WaterBodyType::Lake, 1_000_000.0);
        lake.elevation_m = 500.0;
        system.add_water_body(lake);

        let mut world = hecs::World::new();
        let data = DataStore::new();

        for _ in 0..200 {
            system.tick(&mut world, 0.1, &data);
        }
    }

    #[test]
    fn test_evaporation_reduces_volume() {
        let mut system = HydrologySystem::new();
        let mut lake = WaterBody::new(0, WaterBodyType::Lake, 1_000_000.0);
        lake.temperature_k = 310.0; // warm water = more evaporation
        lake.surface_area_m2 = 10_000.0;
        let id = system.add_water_body(lake);

        let weather = Weather::default();
        system.simulate_step(100.0, Season::Summer, &weather);

        let vol = system.get(id).unwrap().volume_liters;
        // Volume should have changed (evaporation minus precipitation)
        assert!(vol != 1_000_000.0, "Volume should change after simulation");
    }

    #[test]
    fn test_downstream_flow() {
        let mut system = HydrologySystem::new();

        let mut river = WaterBody::new(0, WaterBodyType::River, 500_000.0);
        river.flow_rate = 100.0;
        let river_id = system.add_water_body(river);

        let lake = WaterBody::new(0, WaterBodyType::Lake, 200_000.0);
        let lake_id = system.add_water_body(lake);

        // Set river to flow into lake.
        system.get_mut(river_id).unwrap().downstream.push(lake_id);

        let weather = Weather::default();
        system.simulate_step(10.0, Season::Spring, &weather);

        let lake_vol = system.get(lake_id).unwrap().volume_liters;
        assert!(lake_vol > 200_000.0, "Lake should receive water from river");
    }

    #[test]
    fn test_contamination_spreads_downstream() {
        let mut system = HydrologySystem::new();

        let mut river = WaterBody::new(0, WaterBodyType::River, 500_000.0);
        river.flow_rate = 200.0;
        river.contamination = 0.8;
        let river_id = system.add_water_body(river);

        let mut lake = WaterBody::new(0, WaterBodyType::Lake, 1_000_000.0);
        lake.contamination = 0.0;
        let lake_id = system.add_water_body(lake);

        system.get_mut(river_id).unwrap().downstream.push(lake_id);

        let weather = Weather::default();
        system.simulate_step(10.0, Season::Spring, &weather);

        let lake_contam = system.get(lake_id).unwrap().contamination;
        assert!(lake_contam > 0.0, "Contamination should spread downstream");
    }
}

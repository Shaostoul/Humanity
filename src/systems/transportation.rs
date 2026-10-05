//! Transportation system — advances `CargoVehicle.progress` toward 1.0
//! at `speed_per_day` rate. When progress reaches 1.0, vehicle is marked
//! `arrived` and a log line fires. Game code reads arrived vehicles to
//! unload payload at the destination.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::CargoVehicle;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Game seconds in a day: 24 hours of the one game clock (2026-09-27).
const REAL_SECONDS_PER_GAME_DAY: f32 = crate::systems::time::EARTH_DAY_S as f32;

/// Top-level RON schema for `data/transportation.ron`.
#[derive(Debug, Deserialize)]
pub struct TransportationData {
    #[serde(default)] pub roads: Vec<ron::Value>,
    #[serde(default)] pub rail: Vec<ron::Value>,
    #[serde(default)] pub space_infrastructure: Vec<ron::Value>,
}

/// Manages roads, rail networks, and space infrastructure.
pub struct TransportationSystem {
    pub data: TransportationData,
    /// Total deliveries completed since startup.
    pub lifetime_deliveries: u64,
}

impl TransportationSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("transportation.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(roads:[],rail:[],space_infrastructure:[])".to_string()
        });
        let data: TransportationData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse transportation.ron: {e}");
            TransportationData { roads: vec![], rail: vec![], space_infrastructure: vec![] }
        });
        log::info!("Loaded transportation data: {} roads, {} rail", data.roads.len(), data.rail.len());
        Self { data, lifetime_deliveries: 0 }
    }
}

impl System for TransportationSystem {
    fn name(&self) -> &str { "TransportationSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let day_fraction = dt / REAL_SECONDS_PER_GAME_DAY;
        if day_fraction <= 0.0 { return; }

        let mut arrivals: Vec<(hecs::Entity, String)> = Vec::new();

        for (entity, vehicle) in world.query_mut::<&mut CargoVehicle>() {
            if vehicle.arrived { continue; }
            vehicle.progress += vehicle.speed_per_day * day_fraction;
            if vehicle.progress >= 1.0 {
                vehicle.progress = 1.0;
                vehicle.arrived = true;
                arrivals.push((entity, vehicle.route_id.clone()));
            }
        }

        for (entity, route) in arrivals {
            log::debug!(
                "Transportation: cargo vehicle {:?} arrived on route '{}'",
                entity, route
            );
            self.lifetime_deliveries += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BUG-157 (2026-10-05): data/transportation.ron wrote `space` while this loader reads
    /// `space_infrastructure`; the field is `serde(default)`, so the list loaded EMPTY with no
    /// error. Every list the shipped file writes must arrive.
    #[test]
    fn the_shipped_transportation_file_fills_every_list() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let s = TransportationSystem::new(&dir);
        assert!(!s.data.roads.is_empty(), "data/transportation.ron's roads arrive");
        assert!(!s.data.rail.is_empty(), "data/transportation.ron's rail arrives");
        assert!(!s.data.space_infrastructure.is_empty(), "data/transportation.ron's space infrastructure arrives");
    }
}

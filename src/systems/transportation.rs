//! Transportation system — advances `CargoVehicle.progress` toward 1.0
//! at `speed_per_day` rate. When progress reaches 1.0, vehicle is marked
//! `arrived` and a log line fires. Game code reads arrived vehicles to
//! unload payload at the destination.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::CargoVehicle;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// 1 game day = 1200 real seconds.
const REAL_SECONDS_PER_GAME_DAY: f32 = 1200.0;

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

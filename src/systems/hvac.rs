//! HVAC system — heating, ventilation, air conditioning, and life support.
//!
//! Each tick:
//!   1. Each `RoomEnvironment` drifts toward the ambient outdoor temperature
//!      (slow exponential decay — represents thermal bleed through hull/walls).
//!   2. Each `HvacUnit` adjusts the nearest `RoomEnvironment` within range,
//!      pushing temp toward `target_temp` proportional to `power_kw`.
//!   3. Vent-mode units reduce CO2 (atmosphere swap with outside).
//!   4. CO2 from occupants accumulates (handled crudely — every Player or
//!      AIBehavior entity in a room contributes a constant ppm/s).

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::{AIBehavior, Controllable, HvacUnit, RoomEnvironment, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Max distance an HVAC unit can affect a room (meters).
const MAX_AFFECT_DIST: f32 = 10.0;
/// Max distance an entity counts as "occupying" a room for CO2 emission (meters).
const ROOM_OCCUPANCY_DIST: f32 = 8.0;
/// Default outdoor temperature when no weather/biome data is available (°C).
const DEFAULT_OUTSIDE_TEMP: f32 = 12.0;
/// Per-second thermal bleed rate (fraction of difference closed each second).
const BLEED_RATE: f32 = 0.0005;
/// Per-second CO2 ppm emitted by each occupant.
const CO2_PER_OCCUPANT_PER_SEC: f32 = 0.05;

/// Top-level RON schema for `data/hvac.ron`.
#[derive(Debug, Deserialize)]
pub struct HvacData {
    #[serde(default)] pub heating: Vec<ron::Value>,
    #[serde(default)] pub cooling: Vec<ron::Value>,
    #[serde(default)] pub ventilation: Vec<ron::Value>,
    #[serde(default)] pub life_support: Vec<ron::Value>,
    #[serde(default)] pub sensors: Vec<ron::Value>,
}

/// Tracks heating, cooling, and ventilation per room.
pub struct HvacSystem {
    pub data: HvacData,
}

impl HvacSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("hvac.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(heating:[],cooling:[],ventilation:[],life_support:[],sensors:[])".to_string()
        });
        let data: HvacData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse hvac.ron: {e}");
            HvacData { heating: vec![], cooling: vec![], ventilation: vec![], life_support: vec![], sensors: vec![] }
        });
        log::info!("Loaded HVAC data: {} heating, {} cooling", data.heating.len(), data.cooling.len());
        Self { data }
    }
}

impl System for HvacSystem {
    fn name(&self) -> &str { "HvacSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }

        // Snapshot HVAC unit positions + state.
        let units: Vec<(glam::Vec3, String, f32, f32)> = world
            .query::<(&Transform, &HvacUnit)>()
            .iter()
            .map(|(_, (t, u))| (t.position, u.mode.clone(), u.target_temp, u.power_kw))
            .collect();

        // Snapshot occupant positions (Players + NPCs).
        let occupant_positions: Vec<glam::Vec3> = world
            .query::<(&Transform, hecs::Or<&Controllable, &AIBehavior>)>()
            .iter()
            .map(|(_, (t, _))| t.position)
            .collect();

        // Snapshot room positions (we'll re-borrow to mutate).
        let rooms: Vec<(hecs::Entity, glam::Vec3)> = world
            .query::<(&Transform, &RoomEnvironment)>()
            .iter()
            .map(|(e, (t, _))| (e, t.position))
            .collect();

        for (room_entity, room_pos) in rooms {
            // Sum HVAC influence: temp shift toward target, vent reduces CO2.
            let mut temp_target_shift = 0.0_f32;
            let mut temp_target_weight = 0.0_f32;
            let mut vent_strength = 0.0_f32;
            for (unit_pos, mode, target, power) in &units {
                let dist = room_pos.distance(*unit_pos);
                if dist > MAX_AFFECT_DIST { continue; }
                // Linear falloff with distance.
                let weight = (1.0 - dist / MAX_AFFECT_DIST).max(0.0) * power;
                match mode.as_str() {
                    "heat" | "cool" => {
                        temp_target_shift += target * weight;
                        temp_target_weight += weight;
                    }
                    "vent" => {
                        vent_strength += weight;
                    }
                    _ => {}
                }
            }

            // Count occupants in this room.
            let occupants = occupant_positions
                .iter()
                .filter(|p| p.distance(room_pos) <= ROOM_OCCUPANCY_DIST)
                .count() as f32;

            if let Ok(mut env) = world.get::<&mut RoomEnvironment>(room_entity) {
                // Thermal bleed toward outside.
                env.temp_c += (DEFAULT_OUTSIDE_TEMP - env.temp_c) * BLEED_RATE * dt;

                // HVAC heat/cool pull.
                if temp_target_weight > 0.0 {
                    let weighted_target = temp_target_shift / temp_target_weight;
                    // Approach target at rate proportional to total power weight.
                    let rate = (temp_target_weight * 0.001).min(0.05);
                    env.temp_c += (weighted_target - env.temp_c) * rate * dt;
                }

                // CO2 emission from occupants (ppm/sec) + vent reduction.
                env.co2_ppm += occupants * CO2_PER_OCCUPANT_PER_SEC * dt;
                if vent_strength > 0.0 {
                    let outside_co2 = 420.0;
                    let vent_rate = (vent_strength * 0.002).min(0.1);
                    env.co2_ppm += (outside_co2 - env.co2_ppm) * vent_rate * dt;
                }
                env.co2_ppm = env.co2_ppm.max(380.0);

                // Humidity drift toward 0.45 (comfortable indoor). Not driven yet.
                env.humidity += (0.45 - env.humidity) * 0.0001 * dt;
                env.humidity = env.humidity.clamp(0.0, 1.0);
            }
        }
    }
}

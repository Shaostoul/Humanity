//! Plumbing system — drains `WaterTank` entities to satisfy nearby
//! `WaterFixture` demand. Each tick, each fixture tries to draw enough water
//! to cover its share of `demand_per_day` from the closest tank within range.
//!
//! No pipe topology yet — distance check is the only routing.
//! Future: real pipe network with valves + pressure simulation.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::{Transform, WaterFixture, WaterTank};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Max distance (meters) a fixture will draw from a tank.
const MAX_DRAW_DIST: f32 = 12.0;

/// 1 game day = 1200 real seconds.
const REAL_SECONDS_PER_GAME_DAY: f32 = 1200.0;

/// Top-level RON schema for `data/plumbing.ron`.
#[derive(Debug, Deserialize)]
pub struct PlumbingData {
    #[serde(default)] pub pipes: Vec<ron::Value>,
    #[serde(default)] pub fixtures: Vec<ron::Value>,
    #[serde(default)] pub treatment: Vec<ron::Value>,
    #[serde(default)] pub storage: Vec<ron::Value>,
    #[serde(default)] pub valves: Vec<ron::Value>,
}

/// Tracks water flow, pressure, and fixture demand.
pub struct PlumbingSystem {
    pub data: PlumbingData,
    /// Total liters delivered system-wide since startup (lifetime stat).
    pub lifetime_liters_delivered: f64,
}

impl PlumbingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("plumbing.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(pipes:[],fixtures:[],treatment:[],storage:[],valves:[])".to_string()
        });
        let data: PlumbingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse plumbing.ron: {e}");
            PlumbingData { pipes: vec![], fixtures: vec![], treatment: vec![], storage: vec![], valves: vec![] }
        });
        log::info!("Loaded plumbing data: {} pipes, {} fixtures", data.pipes.len(), data.fixtures.len());
        Self { data, lifetime_liters_delivered: 0.0 }
    }
}

impl System for PlumbingSystem {
    fn name(&self) -> &str { "PlumbingSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let day_fraction = dt / REAL_SECONDS_PER_GAME_DAY;
        if day_fraction <= 0.0 { return; }

        // Snapshot tank positions so we can mutate tank levels without alias issues.
        let tank_positions: Vec<(hecs::Entity, glam::Vec3)> = world
            .query::<(&Transform, &WaterTank)>()
            .iter()
            .map(|(e, (t, _))| (e, t.position))
            .collect();

        // Compute draws: for each fixture, find the nearest tank within range,
        // then collect (tank, fixture, requested_liters).
        let mut draws: Vec<(hecs::Entity, hecs::Entity, f32)> = Vec::new();
        let max_d2 = MAX_DRAW_DIST * MAX_DRAW_DIST;

        for (fix_entity, (fix, fix_t)) in world.query::<(&WaterFixture, &Transform)>().iter() {
            let requested = fix.demand_per_day * day_fraction;
            if requested <= 0.0 { continue; }

            let mut best: Option<(hecs::Entity, f32)> = None;
            for (tank_entity, tank_pos) in &tank_positions {
                let d2 = fix_t.position.distance_squared(*tank_pos);
                if d2 > max_d2 { continue; }
                if best.map(|(_, b)| d2 < b).unwrap_or(true) {
                    best = Some((*tank_entity, d2));
                }
            }
            if let Some((tank_entity, _)) = best {
                draws.push((tank_entity, fix_entity, requested));
            }
        }

        // Apply draws: deplete tanks, mark fixtures satisfied/unsatisfied.
        let mut delivered_total = 0.0;
        for (tank_entity, fix_entity, requested) in draws {
            let mut delivered = 0.0;
            if let Ok(mut tank) = world.get::<&mut WaterTank>(tank_entity) {
                delivered = tank.current.min(requested);
                tank.current -= delivered;
            }
            if let Ok(mut fix) = world.get::<&mut WaterFixture>(fix_entity) {
                fix.supplied_today += delivered;
                fix.satisfied = delivered >= requested * 0.99;
            }
            delivered_total += delivered;
        }

        // Mark unmet fixtures as unsatisfied (no nearby tank).
        let mut unmet: Vec<hecs::Entity> = Vec::new();
        for (entity, (fix, _)) in world.query::<(&WaterFixture, &Transform)>().iter() {
            if !fix.satisfied {
                unmet.push(entity);
            }
        }
        for entity in unmet {
            if let Ok(mut fix) = world.get::<&mut WaterFixture>(entity) {
                if fix.supplied_today <= 0.0 {
                    fix.satisfied = false;
                }
            }
        }

        self.lifetime_liters_delivered += delivered_total as f64;
    }
}

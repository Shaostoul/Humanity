//! Aging system — advances entity age over game-time and transitions
//! life stages at the thresholds defined in `data/aging_fitness.ron`.
//!
//! Time conversion: 1 game day = 20 real minutes (`SECONDS_PER_GAME_DAY`).
//! 1 game year = 365 game days. So 1 real second ≈ 2.28e-6 game years.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::Age;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// 20 real minutes per game day, 365 days per game year.
const REAL_SECONDS_PER_GAME_YEAR: f32 = 1200.0 * 365.0;

/// Top-level RON schema for `data/aging_fitness.ron`.
#[derive(Debug, Deserialize)]
pub struct AgingData {
    pub age_stages: Vec<ron::Value>,
    pub fitness_levels: Vec<ron::Value>,
    pub exercises: Vec<ron::Value>,
    pub sleep: Vec<ron::Value>,
}

/// (life_stage_id, lower_age_inclusive, upper_age_exclusive). Used by `tick`
/// to map current age to a life stage. Falls back to a hardcoded ladder
/// if the data file isn't loaded or has no `age_stages` we can decode.
fn default_stage_ladder() -> Vec<(&'static str, f32, f32)> {
    vec![
        ("child",       0.0,  12.0),
        ("teen",        12.0, 18.0),
        ("young_adult", 18.0, 30.0),
        ("adult",       30.0, 50.0),
        ("senior",      50.0, 70.0),
        ("elder",       70.0, f32::INFINITY),
    ]
}

/// Tracks age stages, fitness, exercise, and sleep for entities.
pub struct AgingSystem {
    pub data: AgingData,
    /// Resolved (id, lower, upper) ladder built from the data file or the default.
    stage_ladder: Vec<(String, f32, f32)>,
}

impl AgingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("aging_fitness.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(age_stages:[],fitness_levels:[],exercises:[],sleep:[])".to_string()
        });
        let data: AgingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse aging_fitness.ron: {e}");
            AgingData { age_stages: vec![], fitness_levels: vec![], exercises: vec![], sleep: vec![] }
        });
        log::info!("Loaded aging data: {} stages, {} exercises", data.age_stages.len(), data.exercises.len());

        // Build the stage ladder from data, falling back to the default if parsing
        // any individual stage entry fails.
        let mut ladder: Vec<(String, f32, f32)> = data.age_stages.iter()
            .filter_map(|v| Self::parse_stage(v))
            .collect();
        if ladder.is_empty() {
            ladder = default_stage_ladder().into_iter()
                .map(|(id, lo, hi)| (id.to_string(), lo, hi))
                .collect();
        }
        ladder.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        Self { data, stage_ladder: ladder }
    }

    /// Try to extract `(id, lower, upper)` from an `age_stages[].(id, age_range)` entry.
    fn parse_stage(v: &ron::Value) -> Option<(String, f32, f32)> {
        let map = v.clone().into_rust::<std::collections::HashMap<String, ron::Value>>().ok()?;
        let id = map.get("id")?.clone().into_rust::<String>().ok()?;
        let range_seq = match map.get("age_range")? {
            ron::Value::Seq(s) => s,
            _ => return None,
        };
        if range_seq.len() < 2 { return None; }
        let lo = range_seq[0].clone().into_rust::<f64>().ok()? as f32;
        let hi = range_seq[1].clone().into_rust::<f64>().ok()? as f32;
        Some((id, lo, hi))
    }

    /// Resolve an age in years to a life-stage id.
    fn life_stage_for_age(&self, age: f32) -> &str {
        for (id, lo, hi) in &self.stage_ladder {
            if age >= *lo && age < *hi {
                return id;
            }
        }
        // Past the last threshold — return the last stage.
        self.stage_ladder.last().map(|(id, _, _)| id.as_str()).unwrap_or("adult")
    }
}

impl System for AgingSystem {
    fn name(&self) -> &str { "AgingSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let years_per_tick = dt / REAL_SECONDS_PER_GAME_YEAR;
        if years_per_tick <= 0.0 { return; }

        // Collect transitions outside the borrow so we can log them after.
        let mut transitions: Vec<(hecs::Entity, String, String)> = Vec::new();

        for (entity, age) in world.query_mut::<&mut Age>() {
            age.years += years_per_tick;
            let new_stage = self.life_stage_for_age(age.years);
            if new_stage != age.life_stage {
                transitions.push((entity, age.life_stage.clone(), new_stage.to_string()));
                age.life_stage = new_stage.to_string();
            }
        }

        for (entity, old, new) in transitions {
            log::info!("Aging: entity {:?} transitioned {} -> {}", entity, old, new);
        }
    }
}

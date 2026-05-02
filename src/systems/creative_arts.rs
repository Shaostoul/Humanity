//! Creative arts system — drives `CreativeWork` entities (paintings, songs,
//! sculptures, performances) toward completion at a `progress_per_day` rate.
//!
//! Per-tick: every active `CreativeWork` gains progress proportional to dt.
//! When progress reaches 1.0, `completed` flips true and a log line fires.
//! Game code reads completed works to award skill XP, distribute outputs to
//! inventories, and trigger performance events.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::CreativeWork;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// 1 game day = 1200 real seconds.
const REAL_SECONDS_PER_GAME_DAY: f32 = 1200.0;

/// Top-level RON schema for `data/creative_arts.ron`.
#[derive(Debug, Deserialize)]
pub struct CreativeArtsData {
    #[serde(default)] pub instruments: Vec<ron::Value>,
    #[serde(default)] pub art_tools: Vec<ron::Value>,
    #[serde(default)] pub outputs: Vec<ron::Value>,
    #[serde(default)] pub performances: Vec<ron::Value>,
}

/// Manages music instruments, art creation, and performances.
pub struct CreativeArtsSystem {
    pub data: CreativeArtsData,
    /// Total works completed since startup (lifetime stat).
    pub lifetime_works_completed: u64,
}

impl CreativeArtsSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("creative_arts.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(instruments:[],art_tools:[],outputs:[],performances:[])".to_string()
        });
        let data: CreativeArtsData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse creative_arts.ron: {e}");
            CreativeArtsData { instruments: vec![], art_tools: vec![], outputs: vec![], performances: vec![] }
        });
        log::info!(
            "Loaded creative arts data: {} instruments, {} performances",
            data.instruments.len(), data.performances.len()
        );
        Self { data, lifetime_works_completed: 0 }
    }
}

impl System for CreativeArtsSystem {
    fn name(&self) -> &str { "CreativeArtsSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let day_fraction = dt / REAL_SECONDS_PER_GAME_DAY;
        if day_fraction <= 0.0 { return; }

        let mut completions: Vec<(hecs::Entity, String, f32)> = Vec::new();

        for (entity, work) in world.query_mut::<&mut CreativeWork>() {
            if work.completed || !work.working { continue; }
            if work.days_to_complete <= 0.0 { continue; }

            work.progress += day_fraction / work.days_to_complete;
            if work.progress >= 1.0 {
                work.progress = 1.0;
                work.completed = true;
                work.working = false;
                completions.push((entity, work.work_type.clone(), work.quality));
            }
        }

        for (entity, work_type, quality) in completions {
            log::info!(
                "CreativeArts: {:?} completed '{}' work (quality {:.2})",
                entity, work_type, quality
            );
            self.lifetime_works_completed += 1;
        }
    }
}

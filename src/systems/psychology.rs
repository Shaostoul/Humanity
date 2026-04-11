//! Psychology system -- Maslow needs, morale, and personality traits.
//!
//! Loads needs hierarchy, morale modifiers, and personality traits from
//! `data/psychology.ron`. Tracks per-entity need levels and morale.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/psychology.ron`.
#[derive(Debug, Deserialize)]
pub struct PsychologyData {
    pub needs: Vec<ron::Value>,
    pub morale_modifiers: Vec<ron::Value>,
    pub personality_traits: Vec<ron::Value>,
}

// TODO: Add a Needs component to ecs/components.rs so needs live on the entity itself:
//   pub struct Needs { pub hunger: f32, pub thirst: f32, pub sleep: f32, pub morale: f32 }
// Then replace the HashMap-based tracking below with proper component queries.

/// Per-entity basic needs state (0.0 = depleted, 100.0 = fully satisfied).
#[derive(Debug, Clone)]
struct NeedsState {
    hunger: f32,
    thirst: f32,
    sleep: f32,
    morale: f32,
}

impl Default for NeedsState {
    fn default() -> Self {
        Self { hunger: 100.0, thirst: 100.0, sleep: 100.0, morale: 50.0 }
    }
}

/// Decay rates per real-time second (tuned so 1 game-hour ~ 60 real seconds at 1x speed).
const HUNGER_DECAY_PER_SEC: f32 = 0.5 / 60.0;   // ~0.5 per game-hour
const THIRST_DECAY_PER_SEC: f32 = 0.8 / 60.0;    // ~0.8 per game-hour
const SLEEP_DECAY_PER_SEC: f32 = 0.3 / 60.0;     // ~0.3 per game-hour
/// Threshold below which a need is considered critical.
const CRITICAL_THRESHOLD: f32 = 20.0;
/// Threshold above which all needs are considered "satisfied" for morale.
const SATISFIED_THRESHOLD: f32 = 80.0;

/// Tracks Maslow needs, morale, and personality per entity.
pub struct PsychologySystem {
    pub data: PsychologyData,
    /// Per-entity needs state, keyed by hecs Entity bits (u64).
    /// TODO: migrate to a proper Needs ECS component.
    needs_map: HashMap<u64, NeedsState>,
    /// Accumulator to throttle per-entity log spam.
    log_cooldown: f32,
}

impl PsychologySystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("psychology.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(needs:[],morale_modifiers:[],personality_traits:[])".to_string()
        });
        let data: PsychologyData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse psychology.ron: {e}");
            PsychologyData { needs: vec![], morale_modifiers: vec![], personality_traits: vec![] }
        });
        log::info!("Loaded psychology data: {} needs, {} traits", data.needs.len(), data.personality_traits.len());
        Self { data, needs_map: HashMap::new(), log_cooldown: 0.0 }
    }
}

impl System for PsychologySystem {
    fn name(&self) -> &str {
        "PsychologySystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        use crate::ecs::components::{Health, Name};

        // Collect entity IDs first to avoid borrow conflict with needs_map
        let entities: Vec<(hecs::Entity, String)> = world
            .query::<(&Health, Option<&Name>)>()
            .iter()
            .filter(|(_, (h, _))| h.current > 0.0) // only alive entities
            .map(|(e, (_, name))| {
                let label = name.map_or_else(|| format!("entity_{}", e.to_bits()), |n| n.0.clone());
                (e, label)
            })
            .collect();

        let should_log = self.log_cooldown <= 0.0;
        if should_log {
            self.log_cooldown = 10.0; // log status every 10 seconds
        }
        self.log_cooldown -= dt;

        for (entity, name) in &entities {
            let key = entity.to_bits().into();
            let needs = self.needs_map.entry(key).or_insert_with(NeedsState::default);

            // Decay basic needs over time
            needs.hunger = (needs.hunger - HUNGER_DECAY_PER_SEC * dt).max(0.0);
            needs.thirst = (needs.thirst - THIRST_DECAY_PER_SEC * dt).max(0.0);
            needs.sleep = (needs.sleep - SLEEP_DECAY_PER_SEC * dt).max(0.0);

            // Count how many needs are critical vs satisfied
            let critical_count = [needs.hunger, needs.thirst, needs.sleep]
                .iter()
                .filter(|&&v| v < CRITICAL_THRESHOLD)
                .count();
            let all_satisfied = needs.hunger >= SATISFIED_THRESHOLD
                && needs.thirst >= SATISFIED_THRESHOLD
                && needs.sleep >= SATISFIED_THRESHOLD;

            // Adjust morale based on need satisfaction
            if all_satisfied {
                // Morale slowly rises when all needs are met (cap at 100)
                needs.morale = (needs.morale + 0.1 * dt).min(100.0);
                if should_log {
                    log::debug!("[Psychology] {name}: all needs satisfied, morale {:.0}", needs.morale);
                }
            } else if critical_count > 0 {
                // Morale drops for each critical need (floor at 0)
                needs.morale = (needs.morale - critical_count as f32 * 0.5 * dt).max(0.0);
                if should_log {
                    log::warn!(
                        "[Psychology] {name}: {critical_count} critical need(s) \
                         (h:{:.0} t:{:.0} s:{:.0}), morale {:.0}",
                        needs.hunger, needs.thirst, needs.sleep, needs.morale,
                    );
                }
            }
        }

        // Garbage-collect needs for despawned entities
        let live_keys: std::collections::HashSet<u64> =
            entities.iter().map(|(e, _)| e.to_bits().into()).collect();
        self.needs_map.retain(|k, _| live_keys.contains(k));
    }
}

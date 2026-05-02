//! Fire system — fire ignition, intensity, spread, suppression.
//!
//! Each tick:
//!   1. Active `Fire` entities consume `fuel_remaining` proportional to intensity * dt.
//!   2. Fires that exhaust fuel are removed.
//!   3. Each fire rolls a spread check against nearby `Flammable` entities
//!      within `Flammable.ignition_dist`. Hotter fires spread faster.
//!   4. `FireSuppressor` entities reduce intensity of fires within range.
//!   5. Fires damage `Health` of co-located entities (1.0 hp/sec at full intensity).

use std::path::Path;

use rand::Rng;
use serde::Deserialize;

use crate::ecs::components::{Fire, FireSuppressor, Flammable, Health, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Per-second fuel consumption multiplier (at intensity = 1.0).
const FUEL_BURN_RATE: f32 = 1.0;
/// Spread chance per second per nearby flammable, scaled by intensity.
const SPREAD_CHANCE_PER_SEC: f32 = 0.05;
/// Damage per second to entities at the same position as a Fire (at intensity = 1.0).
const DAMAGE_PER_SEC: f32 = 1.0;
/// Distance threshold for "co-located" damage (meters).
const DAMAGE_DIST: f32 = 1.5;

/// Top-level RON schema for `data/fire_system.ron`.
#[derive(Debug, Deserialize)]
pub struct FireData {
    #[serde(default)] pub ignition_sources: Vec<ron::Value>,
    #[serde(default)] pub fire_behaviors: Vec<ron::Value>,
    #[serde(default)] pub suppression_systems: Vec<ron::Value>,
    #[serde(default)] pub fire_damage_effects: Vec<ron::Value>,
}

/// Tracks ignition sources, fire spread, and suppression.
pub struct FireSystem {
    pub data: FireData,
    /// Entities scheduled for new Fire components on next tick (queued from spread checks).
    pending_ignitions: Vec<(hecs::Entity, f32)>,
}

impl FireSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("fire_system.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(ignition_sources:[],fire_behaviors:[],suppression_systems:[],fire_damage_effects:[])".to_string()
        });
        let data: FireData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse fire_system.ron: {e}");
            FireData {
                ignition_sources: vec![],
                fire_behaviors: vec![],
                suppression_systems: vec![],
                fire_damage_effects: vec![],
            }
        });
        log::info!(
            "Loaded fire data: {} ignition sources, {} behaviors",
            data.ignition_sources.len(), data.fire_behaviors.len()
        );
        Self { data, pending_ignitions: Vec::new() }
    }
}

impl System for FireSystem {
    fn name(&self) -> &str { "FireSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }
        let mut rng = rand::thread_rng();

        // Apply pending ignitions from previous tick (deferred to avoid alias).
        for (entity, fuel) in self.pending_ignitions.drain(..) {
            let _ = world.insert_one(entity, Fire { intensity: 0.3, fuel_remaining: fuel });
        }

        // Snapshot fire positions, intensities, and entities.
        let active_fires: Vec<(hecs::Entity, glam::Vec3, f32)> = world
            .query::<(&Transform, &Fire)>()
            .iter()
            .map(|(e, (t, f))| (e, t.position, f.intensity))
            .collect();

        // Snapshot suppressors.
        let suppressors: Vec<(glam::Vec3, f32, f32)> = world
            .query::<(&Transform, &FireSuppressor)>()
            .iter()
            .map(|(_, (t, s))| (t.position, s.range, s.strength))
            .collect();

        // Snapshot flammables (entity, position, ignition_dist, fuel_seconds).
        let flammables: Vec<(hecs::Entity, glam::Vec3, f32, f32)> = world
            .query::<(&Transform, &Flammable)>()
            .iter()
            .filter(|(e, _)| world.get::<&Fire>(*e).is_err())  // not already burning
            .map(|(e, (t, f))| (e, t.position, f.ignition_dist, f.fuel_seconds))
            .collect();

        // Snapshot health-bearers for damage.
        let damageables: Vec<(hecs::Entity, glam::Vec3)> = world
            .query::<(&Transform, &Health)>()
            .iter()
            .map(|(e, (t, _))| (e, t.position))
            .collect();

        // Compute per-fire updates: intensity changes, fuel consumption, spread targets.
        let mut intensity_updates: Vec<(hecs::Entity, f32)> = Vec::new();
        let mut fuel_consumes: Vec<(hecs::Entity, f32)> = Vec::new();
        let mut deaths: Vec<hecs::Entity> = Vec::new();

        for (fire_entity, fire_pos, intensity) in &active_fires {
            // Fuel consumption.
            let consume = intensity * FUEL_BURN_RATE * dt;
            fuel_consumes.push((*fire_entity, consume));

            // Suppression.
            let mut suppression = 0.0_f32;
            for (sup_pos, range, strength) in &suppressors {
                if fire_pos.distance(*sup_pos) <= *range {
                    suppression += strength * dt;
                }
            }
            intensity_updates.push((*fire_entity, -suppression));

            // Spread check.
            for (target, target_pos, ignition_dist, fuel) in &flammables {
                let dist = fire_pos.distance(*target_pos);
                if dist > *ignition_dist { continue; }
                let chance = SPREAD_CHANCE_PER_SEC * intensity * dt;
                if rng.gen::<f32>() < chance {
                    self.pending_ignitions.push((*target, *fuel));
                }
            }

            // Damage co-located health-bearers.
            for (target, t_pos) in &damageables {
                if fire_pos.distance(*t_pos) > DAMAGE_DIST { continue; }
                if let Ok(mut h) = world.get::<&mut Health>(*target) {
                    h.current = (h.current - DAMAGE_PER_SEC * intensity * dt).max(0.0);
                }
            }
        }

        // Apply intensity changes + fuel consumption + collect deaths.
        for (entity, delta) in intensity_updates {
            if let Ok(mut fire) = world.get::<&mut Fire>(entity) {
                fire.intensity = (fire.intensity + delta).clamp(0.0, 1.0);
            }
        }
        for (entity, consume) in fuel_consumes {
            if let Ok(mut fire) = world.get::<&mut Fire>(entity) {
                fire.fuel_remaining -= consume;
                if fire.fuel_remaining <= 0.0 || fire.intensity <= 0.0 {
                    deaths.push(entity);
                }
            }
        }

        // Remove dead fires.
        for entity in deaths {
            let _ = world.remove_one::<Fire>(entity);
            log::debug!("Fire: {:?} extinguished", entity);
        }
    }
}

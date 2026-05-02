//! Medical system — tracks active medical conditions on entities and applies
//! their per-second effects (damage from infection, regen from healing, etc.).
//!
//! Each tick:
//!   1. For every entity with `MedicalConditions`, decrement each active
//!      condition's `seconds_remaining`.
//!   2. Apply `health_per_sec * dt * severity` to the entity's `Health`
//!      (negative = damage, positive = regen).
//!   3. Conditions with `seconds_remaining <= 0` are dropped.
//!
//! Does NOT directly add conditions — that's the job of the systems that
//! create them (e.g. FireSystem applies "burn", AISystem applies "infection",
//! interaction handlers apply treatments). Provide `apply_condition()` as a
//! convenience for those callers.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::{ActiveCondition, Health, MedicalConditions};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/medical.ron`.
#[derive(Debug, Deserialize)]
pub struct MedicalData {
    #[serde(default)] pub conditions: Vec<ron::Value>,
    #[serde(default)] pub procedures: Vec<ron::Value>,
    #[serde(default)] pub support_procedures: Vec<ron::Value>,
    #[serde(default)] pub prosthetics: Vec<ron::Value>,
}

/// Tracks medical conditions, treatments, and recovery.
pub struct MedicalSystem {
    pub data: MedicalData,
}

impl MedicalSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("medical.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(conditions:[],procedures:[],support_procedures:[],prosthetics:[])".to_string()
        });
        let data: MedicalData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse medical.ron: {e}");
            MedicalData { conditions: vec![], procedures: vec![], support_procedures: vec![], prosthetics: vec![] }
        });
        log::info!("Loaded medical data: {} conditions, {} procedures", data.conditions.len(), data.procedures.len());
        Self { data }
    }

    /// Convenience for other systems: add a condition to an entity. Inserts
    /// `MedicalConditions` if not already present.
    pub fn apply_condition(world: &mut hecs::World, entity: hecs::Entity, condition: ActiveCondition) {
        let has_conditions = world.get::<&MedicalConditions>(entity).is_ok();
        if has_conditions {
            if let Ok(mut conds) = world.get::<&mut MedicalConditions>(entity) {
                conds.active.push(condition);
            }
        } else {
            let _ = world.insert_one(entity, MedicalConditions { active: vec![condition] });
        }
    }
}

impl System for MedicalSystem {
    fn name(&self) -> &str { "MedicalSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }

        // Collect entity health deltas separately to avoid double-borrowing.
        let mut health_deltas: Vec<(hecs::Entity, f32)> = Vec::new();
        let mut to_clear: Vec<hecs::Entity> = Vec::new();

        for (entity, conds) in world.query_mut::<&mut MedicalConditions>() {
            let mut total_health_delta = 0.0_f32;
            conds.active.retain_mut(|c| {
                c.seconds_remaining -= dt;
                total_health_delta += c.health_per_sec * c.severity * dt;
                c.seconds_remaining > 0.0
            });
            if total_health_delta != 0.0 {
                health_deltas.push((entity, total_health_delta));
            }
            if conds.active.is_empty() {
                to_clear.push(entity);
            }
        }

        // Apply health changes. Health.current is clamped to [0, max].
        for (entity, delta) in health_deltas {
            if let Ok(mut h) = world.get::<&mut Health>(entity) {
                h.current = (h.current + delta).clamp(0.0, h.max);
            }
        }

        // Remove empty MedicalConditions to keep the world tidy.
        for entity in to_clear {
            let _ = world.remove_one::<MedicalConditions>(entity);
        }
    }
}

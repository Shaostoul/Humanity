//! Docking system — runs the airlock state machine.
//!
//! State machine (`AirlockChamber.state`):
//!   - "open_outer"             : outer door open, chamber matches outside (vacuum or atmo)
//!   - "open_inner"             : inner door open, chamber matches inside (pressurized)
//!   - "sealed_pressurized"     : both doors closed, chamber at 1 atm
//!   - "sealed_vacuum"          : both doors closed, chamber at 0 atm
//!   - "cycling_to_pressurized" : pumping atmosphere in
//!   - "cycling_to_vacuum"      : pumping atmosphere out
//!
//! Game code requests state changes (e.g. when a player enters the chamber
//! and clicks "cycle"). This system advances `cycle_progress` for cycling
//! states and transitions to the sealed end-state on completion.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::AirlockChamber;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/docking.ron`.
#[derive(Debug, Deserialize)]
pub struct DockingData {
    #[serde(default)] pub ports: Vec<ron::Value>,
    #[serde(default)] pub airlocks: Vec<ron::Value>,
    #[serde(default)] pub eva_equipment: Vec<ron::Value>,
    #[serde(default)] pub procedures: Vec<ron::Value>,
}

/// Manages docking ports, airlocks, and EVA mechanics.
pub struct DockingSystem {
    pub data: DockingData,
}

impl DockingSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("docking.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(ports:[],airlocks:[],eva_equipment:[],procedures:[])".to_string()
        });
        let data: DockingData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse docking.ron: {e}");
            DockingData { ports: vec![], airlocks: vec![], eva_equipment: vec![], procedures: vec![] }
        });
        log::info!("Loaded docking data: {} ports, {} airlocks", data.ports.len(), data.airlocks.len());
        Self { data }
    }

    /// Game interaction: start cycling the airlock to the requested target state.
    /// `target` is "vacuum" or "pressurized".
    pub fn cycle(world: &mut hecs::World, entity: hecs::Entity, target: &str) {
        if let Ok(mut chamber) = world.get::<&mut AirlockChamber>(entity) {
            // Only allow cycling when both doors are closed.
            if chamber.state.starts_with("sealed_") {
                chamber.state = match target {
                    "vacuum" => "cycling_to_vacuum".into(),
                    "pressurized" => "cycling_to_pressurized".into(),
                    _ => return,
                };
                chamber.cycle_progress = 0.0;
            }
        }
    }
}

impl System for DockingSystem {
    fn name(&self) -> &str { "DockingSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }

        let mut transitions: Vec<(hecs::Entity, String)> = Vec::new();

        for (entity, chamber) in world.query_mut::<&mut AirlockChamber>() {
            match chamber.state.as_str() {
                "cycling_to_vacuum" | "cycling_to_pressurized" => {
                    if chamber.cycle_seconds <= 0.0 {
                        chamber.cycle_seconds = 8.0;
                    }
                    chamber.cycle_progress += dt / chamber.cycle_seconds;
                    if chamber.cycle_progress >= 1.0 {
                        let next = if chamber.state == "cycling_to_vacuum" {
                            "sealed_vacuum"
                        } else {
                            "sealed_pressurized"
                        };
                        chamber.cycle_progress = 0.0;
                        chamber.state = next.to_string();
                        transitions.push((entity, next.to_string()));
                    }
                }
                _ => { /* Static states don't change without external input. */ }
            }
        }

        for (entity, new_state) in transitions {
            log::debug!("Docking: airlock {:?} cycle complete, now {}", entity, new_state);
        }
    }
}

//! Electrical system — power generation, distribution, and consumption.
//!
//! Each tick:
//!   1. Sum all `PowerGenerator.output_watts` where `active`.
//!   2. Sum all `PowerConsumer.draw_watts` where `enabled`.
//!   3. If supply >= demand: every consumer stays enabled.
//!   4. If supply < demand: shed load by `priority` (highest priority off first)
//!      until supply >= remaining demand.
//!   5. Throttle log spam to once per 5 seconds.
//!
//! Fuel consumption is currently a stub — generators with `fuel_per_second > 0`
//! should drain their inventory; that's a future tie-in to the Inventory system.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::{PowerConsumer, PowerGenerator};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Live power readout, published to the DataStore each tick (key `power_status`) so the
/// GUI can show the home's running electrical balance. Watts.
#[derive(Debug, Clone, Copy, Default)]
pub struct PowerStatus {
    pub generation: f32,
    pub consumption: f32,
    pub balance: f32,
}

/// Top-level RON schema for `data/electrical.ron`.
#[derive(Debug, Deserialize)]
pub struct ElectricalData {
    #[serde(default)] pub wires: Vec<ron::Value>,
    #[serde(default)] pub generators: Vec<ron::Value>,
    #[serde(default)] pub distribution: Vec<ron::Value>,
    #[serde(default)] pub consumers: Vec<ron::Value>,
}

/// Tracks power generation, distribution, and consumption.
pub struct ElectricalSystem {
    pub data: ElectricalData,
    /// Net power balance from the last tick (watts). Positive = surplus, negative = deficit.
    pub power_balance: f32,
    /// Total generation capacity from last tick (watts).
    pub total_generation: f32,
    /// Total consumption from last tick (watts).
    pub total_consumption: f32,
    /// Accumulator to throttle log spam (seconds since last log).
    log_cooldown: f32,
}

impl ElectricalSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("electrical.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(wires:[],generators:[],distribution:[],consumers:[])".to_string()
        });
        let data: ElectricalData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse electrical.ron: {e}");
            ElectricalData { wires: vec![], generators: vec![], distribution: vec![], consumers: vec![] }
        });
        log::info!("Loaded electrical data: {} wires, {} generators", data.wires.len(), data.generators.len());
        Self {
            data,
            power_balance: 0.0,
            total_generation: 0.0,
            total_consumption: 0.0,
            log_cooldown: 0.0,
        }
    }
}

impl System for ElectricalSystem {
    fn name(&self) -> &str { "ElectricalSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Sum supply.
        let total_gen: f32 = world.query::<&PowerGenerator>().iter()
            .filter_map(|(_, g)| if g.active { Some(g.output_watts) } else { None })
            .sum();

        // Collect consumers (entity, draw, priority) sorted by priority desc.
        let mut consumers: Vec<(hecs::Entity, f32, u8)> = world.query::<&PowerConsumer>().iter()
            .map(|(e, c)| (e, if c.enabled { c.draw_watts } else { 0.0 }, c.priority))
            .collect();
        // Sort priority desc — highest priority is shed first (counter-intuitive,
        // but matches the convention "priority 5 = optional, 1 = critical": shed
        // optionals first by sorting descending).
        consumers.sort_by(|a, b| b.2.cmp(&a.2));

        let total_demand: f32 = consumers.iter().map(|(_, w, _)| *w).sum();

        // Decide who's enabled this frame.
        let mut to_disable: Vec<hecs::Entity> = Vec::new();
        let mut to_enable: Vec<hecs::Entity> = Vec::new();
        let mut remaining_supply = total_gen;
        let mut consumed = 0.0_f32;

        if total_demand <= total_gen {
            // Plenty of supply — make sure all consumers are enabled.
            for (entity, draw, _) in &consumers {
                let was_enabled = world.get::<&PowerConsumer>(*entity).map(|c| c.enabled).unwrap_or(true);
                if !was_enabled {
                    to_enable.push(*entity);
                }
                consumed += *draw;
            }
        } else {
            // Deficit — go through consumers shedding load.
            // We sorted with HIGHEST priority first; those get shed first.
            for (entity, draw, _) in &consumers {
                if remaining_supply >= *draw && *draw > 0.0 {
                    remaining_supply -= *draw;
                    consumed += *draw;
                    let was_enabled = world.get::<&PowerConsumer>(*entity).map(|c| c.enabled).unwrap_or(true);
                    if !was_enabled {
                        to_enable.push(*entity);
                    }
                } else {
                    to_disable.push(*entity);
                }
            }
        }

        // Apply enabled changes.
        for entity in to_disable {
            if let Ok(mut c) = world.get::<&mut PowerConsumer>(entity) { c.enabled = false; }
        }
        for entity in to_enable {
            if let Ok(mut c) = world.get::<&mut PowerConsumer>(entity) { c.enabled = true; }
        }

        self.total_generation = total_gen;
        self.total_consumption = consumed;
        self.power_balance = total_gen - consumed;

        // Publish the live readout to the DataStore for the GUI (same Mutex pattern as
        // game_time). The home's electrical balance is now a running number, not a string.
        if let Some(status) = data.get::<std::sync::Mutex<PowerStatus>>("power_status") {
            if let Ok(mut s) = status.lock() {
                s.generation = total_gen;
                s.consumption = consumed;
                s.balance = self.power_balance;
            }
        }

        // Throttle log output.
        self.log_cooldown -= dt;
        if self.log_cooldown <= 0.0 {
            self.log_cooldown = 5.0;
            if total_demand > total_gen && total_gen > 0.0 {
                log::warn!(
                    "Power deficit: shedding {:.0}W (demand {:.0}W, supply {:.0}W)",
                    total_demand - total_gen, total_demand, total_gen
                );
            } else if total_gen > 0.0 {
                log::debug!(
                    "Power OK: surplus {:.0}W (gen {:.0}W, draw {:.0}W)",
                    self.power_balance, self.total_generation, self.total_consumption
                );
            }
        }
    }
}

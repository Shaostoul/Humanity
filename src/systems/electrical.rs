//! Electrical system -- power generation, distribution, and consumption.
//!
//! Loads wire, generator, distribution, and consumer definitions from
//! `data/electrical.ron`. Tracks power budgets per room/structure.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/electrical.ron`.
#[derive(Debug, Deserialize)]
pub struct ElectricalData {
    pub wires: Vec<ron::Value>,
    pub generators: Vec<ron::Value>,
    pub distribution: Vec<ron::Value>,
    pub consumers: Vec<ron::Value>,
}

// TODO: Add PowerGenerator component to ecs/components.rs:
//   pub struct PowerGenerator { pub output_watts: f32, pub fuel_per_second: f32, pub active: bool }
// TODO: Add PowerConsumer component to ecs/components.rs:
//   pub struct PowerConsumer { pub draw_watts: f32, pub priority: u8 }

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
        Self { data, power_balance: 0.0, total_generation: 0.0, total_consumption: 0.0, log_cooldown: 0.0 }
    }
}

impl System for ElectricalSystem {
    fn name(&self) -> &str {
        "ElectricalSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        use crate::ecs::components::Name;
        use crate::systems::inventory::Inventory;

        let mut total_gen: f32 = 0.0;
        let mut total_draw: f32 = 0.0;

        // TODO: Replace this Interactable-based scan with proper PowerGenerator/PowerConsumer
        // components once they exist. For now we use Interactable.interaction_type as a proxy:
        //   "generator" entities produce power, "machine" entities consume it.
        for (_entity, (interactable, inv)) in
            world.query::<(&crate::ecs::components::Interactable, Option<&Inventory>)>().iter()
        {
            match interactable.interaction_type.as_str() {
                "generator" => {
                    // Check if the generator has fuel in its inventory
                    let has_fuel = inv.map_or(false, |inv| inv.has_item("fuel", 1));
                    // Each generator produces a base 100W when fueled
                    if has_fuel {
                        total_gen += 100.0;
                    }
                }
                "machine" => {
                    // Each machine draws a base 50W
                    total_draw += 50.0;
                }
                _ => {}
            }
        }

        self.total_generation = total_gen;
        self.total_consumption = total_draw;
        self.power_balance = total_gen - total_draw;

        // Throttle log output to once every 5 seconds to avoid spam
        self.log_cooldown -= dt;
        if self.log_cooldown <= 0.0 {
            self.log_cooldown = 5.0;

            if self.power_balance < 0.0 {
                log::warn!(
                    "Power deficit: {:.0}W (gen {:.0}W, draw {:.0}W)",
                    self.power_balance.abs(),
                    self.total_generation,
                    self.total_consumption,
                );
            } else if self.total_generation > 0.0 {
                log::debug!(
                    "Power OK: surplus {:.0}W (gen {:.0}W, draw {:.0}W)",
                    self.power_balance,
                    self.total_generation,
                    self.total_consumption,
                );
            }

            // Log entity count for debugging
            let _gen_name_count = world.query::<&Name>().iter().count();
        }
    }
}

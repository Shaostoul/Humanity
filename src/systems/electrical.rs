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
    /// Total live battery charge across all banks (watt-hours). (v0.473)
    pub battery_wh: f32,
    /// Total battery capacity (watt-hours). 0 = no batteries on the grid.
    pub battery_capacity_wh: f32,
    /// Hours the stored charge would run the current load with zero generation.
    pub autonomy_hours: f32,
}

/// Integrate one tick of grid balance into a single battery. `balance_w` = generation - demand
/// (positive charges, negative discharges). Returns `(new_charge_wh, applied_w)` where `applied_w`
/// > 0 means the battery DREW that many watts from the surplus (charging) and < 0 means it SUPPLIED
/// that many to the deficit (discharging). Clamped by capacity, available charge, and the
/// charge/discharge power limits. Pure + deterministic, so it is unit-tested directly. (v0.473)
pub fn integrate_battery(
    charge_wh: f32,
    capacity_wh: f32,
    max_charge_w: f32,
    max_discharge_w: f32,
    balance_w: f32,
    dt: f32,
) -> (f32, f32) {
    let dt_h = dt / 3600.0;
    if dt_h <= 0.0 {
        return (charge_wh, 0.0);
    }
    if balance_w >= 0.0 {
        let rate = balance_w.min(max_charge_w.max(0.0));
        let headroom = (capacity_wh - charge_wh).max(0.0);
        let add = (rate * dt_h).min(headroom);
        ((charge_wh + add).min(capacity_wh), add / dt_h)
    } else {
        let rate = (-balance_w).min(max_discharge_w.max(0.0));
        let avail = charge_wh.max(0.0);
        let remove = (rate * dt_h).min(avail);
        ((charge_wh - remove).max(0.0), -(remove / dt_h))
    }
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

        // Battery banks (v0.473): integrate the grid surplus/deficit into each bank so the day/night
        // solar swing actually drains + refills storage. Processed sequentially: each bank takes a
        // bite of the remaining balance (so N banks share one surplus/deficit, no double counting).
        // (A future increment lets battery discharge PREVENT the load-shedding above; today it tracks
        // state against the true generation-vs-demand balance.)
        let mut grid_balance = total_gen - total_demand;
        let battery_ents: Vec<hecs::Entity> =
            world.query::<&crate::ecs::components::Battery>().iter().map(|(e, _)| e).collect();
        let (mut battery_wh, mut battery_cap) = (0.0_f32, 0.0_f32);
        for e in battery_ents {
            if let Ok(mut b) = world.get::<&mut crate::ecs::components::Battery>(e) {
                let (new_charge, applied_w) = integrate_battery(
                    b.charge_wh, b.capacity_wh, b.max_charge_w, b.max_discharge_w, grid_balance, dt,
                );
                b.charge_wh = new_charge;
                grid_balance -= applied_w; // charging consumes surplus; discharging fills the deficit
                battery_wh += b.charge_wh;
                battery_cap += b.capacity_wh;
            }
        }
        let autonomy_hours = if total_demand > 1.0 { battery_wh / total_demand } else { 0.0 };

        // Publish the live readout to the DataStore for the GUI (same Mutex pattern as
        // game_time). The home's electrical balance is now a running number, not a string.
        if let Some(status) = data.get::<std::sync::Mutex<PowerStatus>>("power_status") {
            if let Ok(mut s) = status.lock() {
                s.generation = total_gen;
                s.consumption = consumed;
                s.balance = self.power_balance;
                s.battery_wh = battery_wh;
                s.battery_capacity_wh = battery_cap;
                s.autonomy_hours = autonomy_hours;
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

#[cfg(test)]
mod tests {
    use super::integrate_battery;

    const CAP: f32 = 10_000.0; // 10 kWh
    const RATE: f32 = 5_000.0; // 5 kW charge + discharge

    #[test]
    fn charges_on_surplus_and_discharges_on_deficit() {
        // 1 h of +2000 W surplus from half charge -> +2000 Wh, charging at 2000 W.
        let (c, w) = integrate_battery(5000.0, CAP, RATE, RATE, 2000.0, 3600.0);
        assert!((c - 7000.0).abs() < 1.0 && (w - 2000.0).abs() < 1.0, "charge {c} at {w} W");
        // 1 h of -3000 W deficit -> -3000 Wh, discharging at 3000 W (negative applied).
        let (c, w) = integrate_battery(7000.0, CAP, RATE, RATE, -3000.0, 3600.0);
        assert!((c - 4000.0).abs() < 1.0 && (w + 3000.0).abs() < 1.0, "discharge {c} at {w} W");
    }

    #[test]
    fn clamps_to_rate_capacity_and_empty() {
        // Charge rate cap: +10 kW surplus but 5 kW max -> only 5000 Wh in 1 h.
        let (c, _) = integrate_battery(0.0, CAP, RATE, RATE, 10_000.0, 3600.0);
        assert!((c - 5000.0).abs() < 1.0, "rate-capped charge {c}");
        // Capacity cap: never exceed capacity.
        let (c, _) = integrate_battery(9_500.0, CAP, RATE, RATE, 5000.0, 3600.0);
        assert!((c - CAP).abs() < 1.0, "capacity-capped {c}");
        // Empty cap: never discharge below 0.
        let (c, _) = integrate_battery(500.0, CAP, RATE, RATE, -5000.0, 3600.0);
        assert!(c.abs() < 1.0, "empty-capped {c}");
    }

    #[test]
    fn zero_dt_is_a_noop() {
        let (c, w) = integrate_battery(5000.0, CAP, RATE, RATE, 9999.0, 0.0);
        assert!((c - 5000.0).abs() < 1e-6 && w == 0.0);
    }
}

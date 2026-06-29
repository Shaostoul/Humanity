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
        use crate::ecs::components::{Battery, PowerCircuit};
        use std::collections::HashMap;

        // Power flows PER ISLAND (v0.607): generation, loads, and batteries are grouped by their
        // PowerCircuit.island, so a generator only feeds loads on its own wired circuit -- no magic
        // transmission across unconnected wiring. Entities WITHOUT a PowerCircuit (legacy/test spawns)
        // all share the `None` bucket, which reproduces the old whole-world summing exactly.
        // The published PowerStatus aggregates across islands so the Home card still shows the home.

        // 1. Gather per-island aggregates (Entity is Copy, so the query borrows release after collect).
        let mut gen_by: HashMap<Option<u32>, f32> = HashMap::new();
        for (_, (g, pc)) in world.query::<(&PowerGenerator, Option<&PowerCircuit>)>().iter() {
            if g.active {
                *gen_by.entry(pc.map(|p| p.island)).or_default() += g.output_watts;
            }
        }
        let mut cons_by: HashMap<Option<u32>, Vec<(hecs::Entity, f32, u8)>> = HashMap::new();
        for (e, (c, pc)) in world.query::<(&PowerConsumer, Option<&PowerCircuit>)>().iter() {
            cons_by
                .entry(pc.map(|p| p.island))
                .or_default()
                .push((e, if c.enabled { c.draw_watts } else { 0.0 }, c.priority));
        }
        let mut batt_by: HashMap<Option<u32>, Vec<hecs::Entity>> = HashMap::new();
        for (e, (_b, pc)) in world.query::<(&Battery, Option<&PowerCircuit>)>().iter() {
            batt_by.entry(pc.map(|p| p.island)).or_default().push(e);
        }

        // Every island that has anything electrical.
        let mut keys: std::collections::BTreeSet<Option<u32>> = std::collections::BTreeSet::new();
        keys.extend(gen_by.keys().copied());
        keys.extend(cons_by.keys().copied());
        keys.extend(batt_by.keys().copied());

        let mut to_disable: Vec<hecs::Entity> = Vec::new();
        let mut to_enable: Vec<hecs::Entity> = Vec::new();
        let (mut total_gen_all, mut consumed_all, mut demand_all) = (0.0_f32, 0.0_f32, 0.0_f32);
        let (mut battery_wh, mut battery_cap) = (0.0_f32, 0.0_f32);

        // 2. Balance + shed + integrate batteries, ONE ISLAND AT A TIME.
        for key in keys {
            let total_gen = gen_by.get(&key).copied().unwrap_or(0.0);
            let mut consumers = cons_by.remove(&key).unwrap_or_default();
            // Highest priority shed FIRST (convention: priority 5 = optional, 1 = critical).
            consumers.sort_by(|a, b| b.2.cmp(&a.2));
            let total_demand: f32 = consumers.iter().map(|(_, w, _)| *w).sum();
            demand_all += total_demand;

            let mut remaining = total_gen;
            let mut consumed = 0.0_f32;
            if total_demand <= total_gen {
                for (e, draw, _) in &consumers {
                    if world.get::<&PowerConsumer>(*e).map(|c| !c.enabled).unwrap_or(false) {
                        to_enable.push(*e);
                    }
                    consumed += *draw;
                }
            } else {
                for (e, draw, _) in &consumers {
                    if remaining >= *draw && *draw > 0.0 {
                        remaining -= *draw;
                        consumed += *draw;
                        if world.get::<&PowerConsumer>(*e).map(|c| !c.enabled).unwrap_or(false) {
                            to_enable.push(*e);
                        }
                    } else {
                        to_disable.push(*e);
                    }
                }
            }
            total_gen_all += total_gen;
            consumed_all += consumed;

            // Batteries on THIS island buffer this island's surplus/deficit (sequential bite, no
            // double counting). Discharge tracks state; it does not yet prevent the shed above.
            let mut grid_balance = total_gen - total_demand;
            if let Some(batts) = batt_by.remove(&key) {
                for e in batts {
                    if let Ok(mut b) = world.get::<&mut Battery>(e) {
                        let (new_charge, applied_w) = integrate_battery(
                            b.charge_wh, b.capacity_wh, b.max_charge_w, b.max_discharge_w, grid_balance, dt,
                        );
                        b.charge_wh = new_charge;
                        grid_balance -= applied_w;
                        battery_wh += b.charge_wh;
                        battery_cap += b.capacity_wh;
                    }
                }
            }
        }

        // 3. Apply the enabled/disabled changes (deferred so the borrows above stay short).
        for entity in to_disable {
            if let Ok(mut c) = world.get::<&mut PowerConsumer>(entity) { c.enabled = false; }
        }
        for entity in to_enable {
            if let Ok(mut c) = world.get::<&mut PowerConsumer>(entity) { c.enabled = true; }
        }

        self.total_generation = total_gen_all;
        self.total_consumption = consumed_all;
        self.power_balance = total_gen_all - consumed_all;
        let autonomy_hours = if demand_all > 1.0 { battery_wh / demand_all } else { 0.0 };

        // 4. Publish the aggregated live readout to the DataStore for the GUI.
        if let Some(status) = data.get::<std::sync::Mutex<PowerStatus>>("power_status") {
            if let Ok(mut s) = status.lock() {
                s.generation = total_gen_all;
                s.consumption = consumed_all;
                s.balance = self.power_balance;
                s.battery_wh = battery_wh;
                s.battery_capacity_wh = battery_cap;
                s.autonomy_hours = autonomy_hours;
            }
        }

        // Throttle log output (whole-home aggregate).
        self.log_cooldown -= dt;
        if self.log_cooldown <= 0.0 {
            self.log_cooldown = 5.0;
            if demand_all > total_gen_all && total_gen_all > 0.0 {
                log::warn!(
                    "Power deficit: shedding {:.0}W (demand {:.0}W, supply {:.0}W)",
                    demand_all - total_gen_all, demand_all, total_gen_all
                );
            } else if total_gen_all > 0.0 {
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

    /// The full tick computes + publishes a live PowerStatus from the world's power
    /// entities -- the foundation of the MENU-mode home sim (v0.518): with the home's
    /// power entities spawned at startup, generation sums active generators, consumption
    /// sums enabled consumers, balance is their difference. This is what makes the Home
    /// page's "Live power" card non-zero before Enter World.
    #[test]
    fn tick_publishes_power_status_from_entities() {
        use super::{ElectricalSystem, PowerStatus};
        use crate::ecs::components::{PowerConsumer, PowerGenerator};
        use crate::ecs::systems::System;
        use crate::hot_reload::data_store::DataStore;

        let mut data = DataStore::new();
        data.insert("power_status", std::sync::Mutex::new(PowerStatus::default()));
        let mut world = hecs::World::new();
        world.spawn((PowerGenerator { output_watts: 2000.0, fuel_per_second: 0.0, active: true },));
        world.spawn((PowerGenerator { output_watts: 1000.0, fuel_per_second: 0.0, active: true },));
        world.spawn((PowerConsumer { draw_watts: 1800.0, priority: 1, enabled: true },));

        let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
        sys.tick(&mut world, 1.0, &data);

        let ps = data
            .get::<std::sync::Mutex<PowerStatus>>("power_status")
            .unwrap()
            .lock()
            .unwrap();
        assert!((ps.generation - 3000.0).abs() < 1.0, "generation {}", ps.generation);
        assert!((ps.consumption - 1800.0).abs() < 1.0, "consumption {}", ps.consumption);
        assert!((ps.balance - 1200.0).abs() < 1.0, "balance {}", ps.balance);
    }

    /// v0.607: power flows PER ISLAND. Island 0 has a generator + a load (the load runs). Island 1 has
    /// a load but NO generator (it is shed -- no magic transmission from island 0). The published
    /// PowerStatus aggregates: generation = island 0's, consumption = only the powered load.
    #[test]
    fn tick_gates_power_per_island() {
        use super::{ElectricalSystem, PowerStatus};
        use crate::ecs::components::{PowerCircuit, PowerConsumer, PowerGenerator};
        use crate::ecs::systems::System;
        use crate::hot_reload::data_store::DataStore;

        let mut data = DataStore::new();
        data.insert("power_status", std::sync::Mutex::new(PowerStatus::default()));
        let mut world = hecs::World::new();
        // Island 0: 1000 W generator + a 200 W load -> powered.
        world.spawn((PowerGenerator { output_watts: 1000.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 0 }));
        let powered = world.spawn((PowerConsumer { draw_watts: 200.0, priority: 1, enabled: true }, PowerCircuit { island: 0 }));
        // Island 1: a 200 W load with NO generator on its circuit -> must be shed.
        let isolated = world.spawn((PowerConsumer { draw_watts: 200.0, priority: 1, enabled: true }, PowerCircuit { island: 1 }));

        let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
        sys.tick(&mut world, 1.0, &data);

        assert!(world.get::<&PowerConsumer>(powered).unwrap().enabled, "island-0 load stays powered");
        assert!(!world.get::<&PowerConsumer>(isolated).unwrap().enabled, "island-1 load is shed (no generator on its circuit)");
        let ps = data.get::<std::sync::Mutex<PowerStatus>>("power_status").unwrap().lock().unwrap();
        assert!((ps.generation - 1000.0).abs() < 1.0, "generation {}", ps.generation);
        assert!((ps.consumption - 200.0).abs() < 1.0, "only the powered load draws: {}", ps.consumption);
    }
}

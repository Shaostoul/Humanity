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
//! Fueled BACKSTOP gensets (v0.733): a generator with `fuel_per_second > 0`
//! runs ONLY when its island needs it — free-source supply short of demand
//! AND island batteries low — and burns from the machine's own fuel drum
//! (its `Container` component, flammable-class contents). An empty drum means
//! no watts: the deep-tail backstop is a real consequence, not a stat line.

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
    /// Fractional litres burned per fueled genset, carried between ticks so a
    /// slow burn (1.5 L/h at 60 fps) still consumes whole fuel UNITS from the
    /// drum once enough has accumulated. Keyed by the genset entity. (v0.733)
    fuel_accum: std::collections::HashMap<hecs::Entity, f32>,
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
            fuel_accum: std::collections::HashMap::new(),
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

        // 0. Backstop gensets (v0.733): decide per fueled generator whether it
        // RUNS this tick — its island's free sources short of raw demand AND
        // the island batteries low — and burn drum fuel while it does. This
        // runs before the aggregation so an active genset counts as supply.
        {
            // Free (unfueled) supply, raw demand, and battery fraction per island.
            let mut free_gen: HashMap<Option<u32>, f32> = HashMap::new();
            for (_, (g, pc)) in world.query::<(&PowerGenerator, Option<&PowerCircuit>)>().iter() {
                if g.active && g.fuel_per_second <= 0.0 {
                    *free_gen.entry(pc.map(|p| p.island)).or_default() += g.output_watts;
                }
            }
            let mut raw_demand: HashMap<Option<u32>, f32> = HashMap::new();
            for (_, (c, pc)) in world.query::<(&PowerConsumer, Option<&PowerCircuit>)>().iter() {
                *raw_demand.entry(pc.map(|p| p.island)).or_default() += c.draw_watts;
            }
            let mut batt: HashMap<Option<u32>, (f32, f32)> = HashMap::new();
            for (_, (b, pc)) in world.query::<(&Battery, Option<&PowerCircuit>)>().iter() {
                let e = batt.entry(pc.map(|p| p.island)).or_default();
                e.0 += b.charge_wh;
                e.1 += b.capacity_wh;
            }
            let item_reg =
                data.get::<crate::systems::inventory::ItemRegistry>("item_registry");
            let mut updates: Vec<(hecs::Entity, bool)> = Vec::new();
            for (e, (g, cont, pc)) in world
                .query::<(
                    &PowerGenerator,
                    Option<&crate::systems::inventory::containers::Container>,
                    Option<&PowerCircuit>,
                )>()
                .iter()
            {
                if g.fuel_per_second <= 0.0 {
                    continue;
                }
                let island = pc.map(|p| p.island);
                let shortfall = free_gen.get(&island).copied().unwrap_or(0.0)
                    < raw_demand.get(&island).copied().unwrap_or(0.0);
                let (wh, cap) = batt.get(&island).copied().unwrap_or((0.0, 0.0));
                let batteries_low = cap <= 0.0 || wh / cap < 0.25;
                // Only flammable-class contents burn (grain in the drum does
                // not power the house).
                let fuel_ok = cont
                    .and_then(|c| c.current_content_item.as_ref().map(|i| (i, c.current_qty)))
                    .map(|(item, qty)| {
                        qty > 0
                            && item_reg
                                .map(|r| r.class_for(item) == "flammable")
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                updates.push((e, shortfall && batteries_low && fuel_ok));
            }
            for (e, run) in updates {
                if let Ok(mut g) = world.get::<&mut PowerGenerator>(e) {
                    g.active = run;
                }
                if run {
                    // Burn: accumulate fractional litres; consume whole fuel
                    // units from the drum as the accumulator crosses each
                    // unit's volume.
                    let rate = world
                        .get::<&PowerGenerator>(e)
                        .map(|g| g.fuel_per_second)
                        .unwrap_or(0.0);
                    let accum = self.fuel_accum.entry(e).or_insert(0.0);
                    *accum += rate * dt;
                    if let Ok(mut c) = world
                        .get::<&mut crate::systems::inventory::containers::Container>(e)
                    {
                        if let Some(item) = c.current_content_item.clone() {
                            let unit_vol = item_reg
                                .map(|r| r.volume_for(&item))
                                .unwrap_or(0.0)
                                .max(0.01);
                            while *accum >= unit_vol && c.current_qty > 0 {
                                c.current_qty -= 1;
                                c.used_liters = (c.used_liters - unit_vol).max(0.0);
                                *accum -= unit_vol;
                            }
                            if c.current_qty == 0 {
                                c.current_content_item = None;
                                c.used_liters = 0.0;
                                log::info!("Backstop genset ran its drum dry");
                            }
                        }
                    }
                }
            }
        }

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

    /// Backstop genset (v0.733): with the island short (no free generation, a
    /// live load, no batteries) and flammable fuel in its OWN drum, the genset
    /// runs and BURNS units; a dry drum stops it on the next tick.
    #[test]
    fn backstop_genset_runs_when_needed_and_burns_its_drum_dry() {
        use super::{ElectricalSystem, PowerStatus};
        use crate::ecs::components::{PowerConsumer, PowerGenerator};
        use crate::ecs::systems::System;
        use crate::hot_reload::data_store::DataStore;
        use crate::systems::inventory::containers::Container;

        let mut data = DataStore::new();
        data.insert("power_status", std::sync::Mutex::new(PowerStatus::default()));
        let reg = crate::systems::inventory::ItemRegistry::from_csv(
            b"id,name,weight_kg,stack_size,volume_l,content_class\nfuel_refined_0,Refined Fuel,0.8,10,1.0,flammable\n",
        )
        .expect("registry");
        data.insert("item_registry", reg);

        let mut world = hecs::World::new();
        world.spawn((PowerConsumer { draw_watts: 100.0, priority: 1, enabled: true },));
        let mut drum = Container::new("steel_fuel_drum", 200.0);
        drum.current_content_item = Some("fuel_refined_0".to_string());
        drum.current_qty = 2;
        drum.used_liters = 2.0;
        // 1 L/s burn for test speed (real gensets are fuel_lph / 3600).
        let gen = world.spawn((
            PowerGenerator { output_watts: 2000.0, fuel_per_second: 1.0, active: false },
            drum,
        ));

        let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
        sys.tick(&mut world, 1.5, &data); // burns 1.5 L -> one whole unit
        {
            let g = world.get::<&PowerGenerator>(gen).unwrap();
            assert!(g.active, "island short + no batteries + fuel -> genset runs");
        }
        {
            let c = world.get::<&Container>(gen).unwrap();
            assert_eq!(c.current_qty, 1, "1.5 L at 1 L/unit consumes one unit");
        }
        sys.tick(&mut world, 1.0, &data); // accum reaches 1.5 -> second unit -> dry
        sys.tick(&mut world, 0.1, &data); // dry drum observed -> genset stops
        let g = world.get::<&PowerGenerator>(gen).unwrap();
        let c = world.get::<&Container>(gen).unwrap();
        assert!(c.is_empty(), "drum drained");
        assert!(!g.active, "dry drum -> the backstop is gone");
    }

    /// Backstop genset (v0.733): healthy batteries keep the genset IDLE even
    /// with an instantaneous shortfall — no nightly fuel-burn while the banks
    /// carry the load. No fuel is consumed while idle.
    #[test]
    fn backstop_genset_idles_when_batteries_are_healthy() {
        use super::{ElectricalSystem, PowerStatus};
        use crate::ecs::components::{Battery, PowerConsumer, PowerGenerator};
        use crate::ecs::systems::System;
        use crate::hot_reload::data_store::DataStore;
        use crate::systems::inventory::containers::Container;

        let mut data = DataStore::new();
        data.insert("power_status", std::sync::Mutex::new(PowerStatus::default()));
        let reg = crate::systems::inventory::ItemRegistry::from_csv(
            b"id,name,weight_kg,stack_size,volume_l,content_class\nfuel_refined_0,Refined Fuel,0.8,10,1.0,flammable\n",
        )
        .expect("registry");
        data.insert("item_registry", reg);

        let mut world = hecs::World::new();
        world.spawn((PowerConsumer { draw_watts: 100.0, priority: 1, enabled: true },));
        world.spawn((Battery {
            charge_wh: 3600.0,
            capacity_wh: 4000.0,
            max_charge_w: 2000.0,
            max_discharge_w: 2000.0,
        },));
        let mut drum = Container::new("steel_fuel_drum", 200.0);
        drum.current_content_item = Some("fuel_refined_0".to_string());
        drum.current_qty = 2;
        drum.used_liters = 2.0;
        let gen = world.spawn((
            PowerGenerator { output_watts: 2000.0, fuel_per_second: 1.0, active: false },
            drum,
        ));

        let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
        sys.tick(&mut world, 5.0, &data);
        let g = world.get::<&PowerGenerator>(gen).unwrap();
        let c = world.get::<&Container>(gen).unwrap();
        assert!(!g.active, "90% batteries -> the backstop idles");
        assert_eq!(c.current_qty, 2, "no fuel burned while idle");
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

//! Plumbing system -- water production, storage, and demand, PER PLUMBING ISLAND (v0.608).
//!
//! The water mirror of `ElectricalSystem`, and the first real POWER -> WATER consequence chain: a
//! `WaterProducer`/`WaterConsumer` flagged `needs_power` only flows while the SAME ECS entity's
//! `PowerConsumer` is enabled. Cut the power (or shed it in a deficit) and the pump stops, the cistern
//! stops filling, and `days_autonomy` starts ticking down -- exactly like a real off-grid home.
//!
//! Each tick, per `PlumbingCircuit.island`:
//!   1. Sum production from powered `WaterProducer`s (L/min).
//!   2. Sum demand from powered `WaterConsumer`s (L/min).
//!   3. Integrate the net flow into the island's `WaterTank`s (clamped 0..capacity), sequential bite.
//!   4. Aggregate across islands into a live `WaterStatus` for the GUI.
//!
//! Entities WITHOUT a `PlumbingCircuit` share the `None` bucket (one global island), so a test or a
//! legacy spawn behaves as a single connected system.
//!
//! (Replaced the v0.x distance-based `WaterFixture`/`WaterTank` scaffold, which was never registered.)

use crate::ecs::components::{PlumbingCircuit, PowerConsumer, WaterConsumer, WaterProducer, WaterTank};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Live water readout, published to the DataStore each tick (key `water_status`) so the GUI can show the
/// home's running water balance. Litres + litres/min.
#[derive(Debug, Clone, Copy, Default)]
pub struct WaterStatus {
    /// Total production from powered producers (L/min).
    pub production_lpm: f32,
    /// Total demand from powered consumers (L/min).
    pub demand_lpm: f32,
    /// production - demand (L/min). Positive fills storage, negative drains it.
    pub balance_lpm: f32,
    /// Total water currently stored across all cisterns/tanks (litres).
    pub stored_l: f32,
    /// Total storage capacity (litres). 0 = no tanks.
    pub capacity_l: f32,
    /// Days the stored water would meet the current demand with zero production.
    pub days_autonomy: f32,
}

/// Tracks the home's water production / storage / demand.
pub struct PlumbingSystem {
    pub status: WaterStatus,
    /// Throttle log spam (seconds since last log).
    log_cooldown: f32,
}

impl PlumbingSystem {
    pub fn new() -> Self {
        Self { status: WaterStatus::default(), log_cooldown: 0.0 }
    }
}

impl Default for PlumbingSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for PlumbingSystem {
    fn name(&self) -> &str {
        "PlumbingSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        use std::collections::HashMap;

        // 1. Per-island production from powered producers. A producer that needs power only counts when
        //    the SAME entity's PowerConsumer is enabled (the power -> water consequence chain).
        let mut prod_by: HashMap<Option<u32>, f32> = HashMap::new();
        for (_, (p, pc, power)) in
            world.query::<(&WaterProducer, Option<&PlumbingCircuit>, Option<&PowerConsumer>)>().iter()
        {
            let powered = !p.needs_power || power.map(|c| c.enabled).unwrap_or(false);
            if powered {
                *prod_by.entry(pc.map(|c| c.island)).or_default() += p.lpm;
            }
        }
        // 2. Per-island demand from powered consumers.
        let mut dem_by: HashMap<Option<u32>, f32> = HashMap::new();
        for (_, (c, pc, power)) in
            world.query::<(&WaterConsumer, Option<&PlumbingCircuit>, Option<&PowerConsumer>)>().iter()
        {
            let powered = !c.needs_power || power.map(|p| p.enabled).unwrap_or(false);
            if powered {
                *dem_by.entry(pc.map(|c| c.island)).or_default() += c.lpm;
            }
        }
        // Tanks grouped by island (Entity is Copy; the query borrow releases after collect).
        let mut tanks_by: HashMap<Option<u32>, Vec<hecs::Entity>> = HashMap::new();
        for (e, (_t, pc)) in world.query::<(&WaterTank, Option<&PlumbingCircuit>)>().iter() {
            tanks_by.entry(pc.map(|c| c.island)).or_default().push(e);
        }

        let mut keys: std::collections::BTreeSet<Option<u32>> = std::collections::BTreeSet::new();
        keys.extend(prod_by.keys().copied());
        keys.extend(dem_by.keys().copied());
        keys.extend(tanks_by.keys().copied());

        let dt_min = dt / 60.0;
        let (mut prod_all, mut dem_all, mut stored_all, mut cap_all) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);

        // 3. Integrate the net flow into each island's tanks (sequential bite, no double counting).
        for key in keys {
            let prod = prod_by.get(&key).copied().unwrap_or(0.0);
            let dem = dem_by.get(&key).copied().unwrap_or(0.0);
            prod_all += prod;
            dem_all += dem;
            let mut delta_l = (prod - dem) * dt_min; // +fills, -drains
            if let Some(tanks) = tanks_by.get(&key) {
                for e in tanks {
                    if let Ok(mut t) = world.get::<&mut WaterTank>(*e) {
                        if delta_l >= 0.0 {
                            let add = delta_l.min((t.capacity_l - t.liters).max(0.0));
                            t.liters += add;
                            delta_l -= add;
                        } else {
                            let take = (-delta_l).min(t.liters.max(0.0));
                            t.liters -= take;
                            delta_l += take;
                        }
                        stored_all += t.liters;
                        cap_all += t.capacity_l;
                    }
                }
            }
        }

        let days_autonomy = if dem_all > 0.001 { stored_all / (dem_all * 1440.0) } else { 0.0 };
        self.status = WaterStatus {
            production_lpm: prod_all,
            demand_lpm: dem_all,
            balance_lpm: prod_all - dem_all,
            stored_l: stored_all,
            capacity_l: cap_all,
            days_autonomy,
        };

        if let Some(status) = data.get::<std::sync::Mutex<WaterStatus>>("water_status") {
            if let Ok(mut s) = status.lock() {
                *s = self.status;
            }
        }

        self.log_cooldown -= dt;
        if self.log_cooldown <= 0.0 {
            self.log_cooldown = 5.0;
            if dem_all > prod_all && cap_all > 0.0 {
                log::debug!(
                    "Water deficit: draining {:.1} L/min (demand {:.1}, production {:.1}); {:.1} days left",
                    dem_all - prod_all, dem_all, prod_all, days_autonomy
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{PlumbingCircuit, PowerConsumer, WaterConsumer, WaterProducer, WaterTank};
    use crate::ecs::systems::System;

    fn status(data: &DataStore) -> WaterStatus {
        *data.get::<std::sync::Mutex<WaterStatus>>("water_status").unwrap().lock().unwrap()
    }

    /// A powered producer fills the tank; cutting its power stops production so the tank drains -- the
    /// power -> water consequence chain.
    #[test]
    fn power_loss_stops_water_production() {
        let mut data = DataStore::new();
        data.insert("water_status", std::sync::Mutex::new(WaterStatus::default()));
        let mut world = hecs::World::new();
        // A purifier: produces 10 L/min but NEEDS power; its PowerConsumer starts enabled.
        let purifier = world.spawn((
            WaterProducer { lpm: 10.0, needs_power: true },
            PowerConsumer { draw_watts: 50.0, priority: 2, enabled: true },
            PlumbingCircuit { island: 0 },
        ));
        // A household draw of 2 L/min (passive).
        world.spawn((WaterConsumer { lpm: 2.0, needs_power: false }, PlumbingCircuit { island: 0 }));
        // A 1000 L cistern starting at 500 L.
        let cistern = world.spawn((WaterTank { liters: 500.0, capacity_l: 1000.0 }, PlumbingCircuit { island: 0 }));

        let mut sys = PlumbingSystem::new();
        // 60 s powered: net +8 L/min -> +8 L into the tank.
        sys.tick(&mut world, 60.0, &data);
        let s = status(&data);
        assert!((s.production_lpm - 10.0).abs() < 0.01, "powered production {}", s.production_lpm);
        assert!(world.get::<&WaterTank>(cistern).unwrap().liters > 500.0, "tank filled while powered");

        // Cut the purifier's power: production stops, only the 2 L/min draw remains -> tank drains.
        world.get::<&mut PowerConsumer>(purifier).unwrap().enabled = false;
        let before = world.get::<&WaterTank>(cistern).unwrap().liters;
        sys.tick(&mut world, 60.0, &data);
        let s = status(&data);
        assert!(s.production_lpm.abs() < 0.01, "no production once unpowered: {}", s.production_lpm);
        assert!(world.get::<&WaterTank>(cistern).unwrap().liters < before, "tank drains with power cut");
    }

    /// Water flows PER ISLAND: a producer on island 0 does not fill island 1's tank.
    #[test]
    fn water_does_not_cross_islands() {
        let mut data = DataStore::new();
        data.insert("water_status", std::sync::Mutex::new(WaterStatus::default()));
        let mut world = hecs::World::new();
        // Island 0: passive producer 5 L/min + a tank.
        world.spawn((WaterProducer { lpm: 5.0, needs_power: false }, PlumbingCircuit { island: 0 }));
        let t0 = world.spawn((WaterTank { liters: 100.0, capacity_l: 1000.0 }, PlumbingCircuit { island: 0 }));
        // Island 1: a tank with NO producer -- it must not fill from island 0.
        let t1 = world.spawn((WaterTank { liters: 100.0, capacity_l: 1000.0 }, PlumbingCircuit { island: 1 }));

        let mut sys = PlumbingSystem::new();
        sys.tick(&mut world, 60.0, &data);
        assert!(world.get::<&WaterTank>(t0).unwrap().liters > 100.0, "island 0 tank fills");
        assert!((world.get::<&WaterTank>(t1).unwrap().liters - 100.0).abs() < 0.01, "island 1 tank unchanged");
    }

    /// End-to-end with the SEED home's exact island numbers (v0.610): cistern (8000 L) + passive rain
    /// 0.1 L/min + a POWERED well pump 2.0 + POWERED irrigation 0.5 + a NON-powered household tap 0.17.
    /// Powered -> the cistern fills; cut the power (pump + irrigation shed) -> rain 0.1 < tap 0.17 -> it
    /// drains. Proves the seed actually demonstrates the power -> water consequence the card advertises.
    #[test]
    fn seed_island_fills_when_powered_and_drains_when_cut() {
        let mut data = DataStore::new();
        data.insert("water_status", std::sync::Mutex::new(WaterStatus::default()));
        let mut world = hecs::World::new();
        // Cistern: tank + passive rain producer (no power).
        let cistern = world.spawn((
            WaterTank { liters: 4000.0, capacity_l: 8000.0 },
            WaterProducer { lpm: 0.1, needs_power: false },
            PlumbingCircuit { island: 0 },
        ));
        // Well pump: powered producer.
        let pump = world.spawn((
            WaterProducer { lpm: 2.0, needs_power: true },
            PowerConsumer { draw_watts: 10.0, priority: 2, enabled: true },
            PlumbingCircuit { island: 0 },
        ));
        // Irrigation: powered consumer.
        let irrigation = world.spawn((
            WaterConsumer { lpm: 0.5, needs_power: true },
            PowerConsumer { draw_watts: 7.0, priority: 3, enabled: true },
            PlumbingCircuit { island: 0 },
        ));
        // Household tap: NON-powered consumer.
        world.spawn((WaterConsumer { lpm: 0.17, needs_power: false }, PlumbingCircuit { island: 0 }));

        let mut sys = PlumbingSystem::new();
        // Powered: net +1.43 L/min -> the cistern fills.
        let before = world.get::<&WaterTank>(cistern).unwrap().liters;
        sys.tick(&mut world, 60.0, &data);
        let after_powered = world.get::<&WaterTank>(cistern).unwrap().liters;
        assert!(after_powered > before, "powered cistern fills ({before} -> {after_powered})");
        assert!(status(&data).balance_lpm > 0.0, "powered balance is positive");

        // Cut the grid: a power deficit sheds the pump + irrigation.
        world.get::<&mut PowerConsumer>(pump).unwrap().enabled = false;
        world.get::<&mut PowerConsumer>(irrigation).unwrap().enabled = false;
        sys.tick(&mut world, 60.0, &data);
        let after_cut = world.get::<&WaterTank>(cistern).unwrap().liters;
        assert!(after_cut < after_powered, "power cut -> cistern drains ({after_powered} -> {after_cut})");
        let s = status(&data);
        assert!(s.balance_lpm < 0.0, "power-cut balance is negative (draining): {}", s.balance_lpm);
        assert!(s.days_autonomy > 0.0 && s.days_autonomy.is_finite(), "finite days of water left: {}", s.days_autonomy);
    }

    /// A tank never overfills or drains below zero.
    #[test]
    fn tank_clamps_to_capacity_and_empty() {
        let mut data = DataStore::new();
        data.insert("water_status", std::sync::Mutex::new(WaterStatus::default()));
        let mut sys = PlumbingSystem::new();

        let mut world = hecs::World::new();
        world.spawn((WaterProducer { lpm: 1000.0, needs_power: false }, PlumbingCircuit { island: 0 }));
        let full = world.spawn((WaterTank { liters: 990.0, capacity_l: 1000.0 }, PlumbingCircuit { island: 0 }));
        sys.tick(&mut world, 600.0, &data); // way more than enough to overfill
        assert!((world.get::<&WaterTank>(full).unwrap().liters - 1000.0).abs() < 0.01, "clamped to capacity");

        // A heavy draw on an island with no production drains to exactly 0, not negative.
        let mut world2 = hecs::World::new();
        world2.spawn((WaterConsumer { lpm: 1000.0, needs_power: false }, PlumbingCircuit { island: 0 }));
        let empty = world2.spawn((WaterTank { liters: 10.0, capacity_l: 1000.0 }, PlumbingCircuit { island: 0 }));
        sys.tick(&mut world2, 600.0, &data);
        assert!(world2.get::<&WaterTank>(empty).unwrap().liters.abs() < 0.01, "drained to zero, not negative");
    }
}

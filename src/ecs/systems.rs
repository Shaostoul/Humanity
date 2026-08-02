//! System runner — executes registered systems each frame.
//!
//! Systems implement the `System` trait and are ticked in registration order.
//! Each system receives the ECS world, delta time, and the data store
//! containing loaded game data (items, plants, recipes, etc.).
//!
//! MEASUREMENT (resource budgets increment 1): every system's tick is timed and
//! deposited in `renderer::frame_costs` as `cpu.system.<slug>`, plus the whole
//! loop as `cpu.systems`. That is what puts "Game systems" on the CPU pie of the
//! Performance page with a real number behind it, and what increment 3's
//! governor will read to notice a system running over its share.
//!
//! Cost of measuring: the id is interned ONCE at registration (no per-frame
//! allocation and no hash lookup), and the loop reads the clock once per system
//! rather than twice, so ~24 registered systems pay roughly one microsecond a
//! frame in total.

use std::time::Instant;

use crate::hot_reload::data_store::DataStore;
use crate::renderer::frame_costs;

/// A game system that runs each frame.
pub trait System: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

    /// Called once per frame with the ECS world, delta time, and game data.
    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore);
}

/// A registered system plus the measurement id its tick time is reported under.
struct Registered {
    system: Box<dyn System>,
    /// `cpu.system.<slug>`, interned at registration (see `frame_costs::intern_id`).
    cost_id: &'static str,
}

/// Runs registered systems in order each frame.
pub struct SystemRunner {
    systems: Vec<Registered>,
}

impl SystemRunner {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    /// Register a system. Systems run in the order they are registered.
    pub fn register<S: System + 'static>(&mut self, system: S) {
        let cost_id = frame_costs::intern_id("cpu.system.", system.name());
        log::info!("Registered system: {} (cost id {cost_id})", system.name());
        self.systems.push(Registered {
            system: Box::new(system),
            cost_id,
        });
    }

    /// Tick all registered systems for one frame, timing each one.
    ///
    /// Deliberately does NOT record the loop TOTAL: the frame loop already
    /// wraps this call in `frame_costs::stage("cpu.systems")`, and CPU stages
    /// accumulate per frame, so recording it here too would count the ECS tick
    /// twice on the Performance page's CPU pie.
    pub fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        let mut prev = Instant::now();
        for reg in &mut self.systems {
            reg.system.tick(world, dt, data);
            // One clock read per system: the end of one tick is the start of
            // the next, so N systems cost N+1 reads rather than 2N.
            let now = Instant::now();
            frame_costs::record_cpu(reg.cost_id, now.duration_since(prev));
            prev = now;
        }
    }

    /// Number of registered systems.
    pub fn count(&self) -> usize {
        self.systems.len()
    }

    /// The measurement ids this runner reports, in tick order. Dev tooling: it
    /// is how a caller (or a test) can enumerate what the CPU pie's per-system
    /// rows may name without hardcoding a list of systems.
    pub fn cost_ids(&self) -> Vec<&'static str> {
        self.systems.iter().map(|r| r.cost_id).collect()
    }
}

impl Default for SystemRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A system that burns a known amount of wall time, so the runner's timing
    /// can be checked against something real rather than against zero.
    struct Burner {
        name: &'static str,
        micros: u64,
    }

    impl System for Burner {
        fn name(&self) -> &str {
            self.name
        }
        fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
            let t0 = Instant::now();
            while t0.elapsed().as_micros() < self.micros as u128 {
                std::hint::spin_loop();
            }
        }
    }

    /// Registration must derive a stable, readable id per system: this is the
    /// join between the ECS and `data/performance/budget_systems.ron`.
    #[test]
    fn system_names_become_stable_measurement_ids() {
        let mut runner = SystemRunner::new();
        runner.register(Burner { name: "FarmingSystem", micros: 0 });
        runner.register(Burner { name: "WeatherSystem", micros: 0 });
        runner.register(Burner { name: "AISystem", micros: 0 });
        runner.register(Burner { name: "Interaction", micros: 0 });
        assert_eq!(
            runner.cost_ids(),
            vec![
                "cpu.system.farming",
                "cpu.system.weather",
                "cpu.system.ai",
                "cpu.system.interaction",
            ]
        );
        assert_eq!(runner.count(), 4);
    }

    /// The per-system times must actually reach the frame-cost store, and the
    /// expensive system must be the one that shows up as expensive.
    #[test]
    fn each_system_tick_is_measured_separately() {
        // The frame-cost store is process-global; share its serialising lock.
        let _g = frame_costs::test_lock();
        frame_costs::reset_for_test();
        let mut runner = SystemRunner::new();
        runner.register(Burner { name: "SlowSystem", micros: 4000 });
        runner.register(Burner { name: "QuickSystem", micros: 0 });
        let mut world = hecs::World::new();
        let data = DataStore::new();

        // Two frames: the first publishes into the EMA, the second confirms the
        // values are being maintained frame over frame.
        for _ in 0..2 {
            frame_costs::begin_frame();
            runner.tick(&mut world, 0.016, &data);
        }
        frame_costs::begin_frame();

        let snap = frame_costs::snapshot();
        let slow = snap.value("cpu.system.slow");
        let quick = snap.value("cpu.system.quick");
        println!("slow={slow:.3} ms quick={quick:.3} ms");
        assert!(slow > 0.0, "the 4 ms system was not measured at all");
        assert!(
            slow > quick,
            "per-system timing is not discriminating: slow={slow} quick={quick}"
        );
        // The frame loop owns the aggregate (`stage("cpu.systems")` around this
        // call). If the runner recorded it too, the CPU pie's "Game systems"
        // slice would be double the truth.
        assert_eq!(
            snap.value("cpu.systems"),
            0.0,
            "the runner must not record the ECS total; the frame loop's stage timer does"
        );
        frame_costs::reset_for_test();
    }
}

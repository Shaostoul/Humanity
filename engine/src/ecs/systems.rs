//! System runner — executes registered systems each frame.
//!
//! Systems implement the `System` trait and are ticked in registration order.
//! Each system receives the ECS world, delta time, and the data store
//! containing loaded game data (items, plants, recipes, etc.).

use crate::hot_reload::data_store::DataStore;

/// A game system that runs each frame.
pub trait System: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

    /// Called once per frame with the ECS world, delta time, and game data.
    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore);
}

/// Runs registered systems in order each frame.
pub struct SystemRunner {
    systems: Vec<Box<dyn System>>,
}

impl SystemRunner {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    /// Register a system. Systems run in the order they are registered.
    pub fn register<S: System + 'static>(&mut self, system: S) {
        log::info!("Registered system: {}", system.name());
        self.systems.push(Box::new(system));
    }

    /// Tick all registered systems for one frame.
    pub fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        for system in &mut self.systems {
            system.tick(world, dt, data);
        }
    }

    /// Number of registered systems.
    pub fn count(&self) -> usize {
        self.systems.len()
    }
}

//! System runner — executes registered systems each frame.
//!
//! Systems are plain functions operating on hecs::World queries.
//! Hot-reload hooks allow re-registering systems at runtime.

/// Runs registered systems in order each frame.
pub struct SystemRunner {
    // TODO: Vec<Box<dyn System>>
}

impl SystemRunner {
    pub fn new() -> Self {
        Self {}
    }

    /// Tick all registered systems for one frame.
    pub fn tick(&mut self, _world: &mut hecs::World, _dt: f32) {
        // TODO: iterate systems
    }
}

//! Multi-scale navigation — galaxy, solar system, orbital, surface.

pub mod galaxy;
pub mod system;
pub mod orbital;
pub mod surface;

/// Navigation system coordinator.
pub struct NavigationSystem {
    // TODO: current view level, camera target
}

impl NavigationSystem {
    pub fn new() -> Self {
        Self {}
    }
}

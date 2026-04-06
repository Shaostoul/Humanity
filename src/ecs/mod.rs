//! ECS — thin wrapper around hecs::World with system scheduling.

pub mod systems;
pub mod components;

/// Game world wrapping hecs::World.
pub struct GameWorld {
    pub world: hecs::World,
}

impl GameWorld {
    pub fn new() -> Self {
        Self {
            world: hecs::World::new(),
        }
    }
}

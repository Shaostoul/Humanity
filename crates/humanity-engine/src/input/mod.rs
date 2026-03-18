//! Input manager — processes keyboard, mouse, and gamepad input.
//!
//! Key bindings loaded from `config/bindings.toml`.

pub mod bindings;

/// Processes raw input events into game actions.
pub struct InputManager {
    // TODO: action map, pressed/released state
}

impl InputManager {
    pub fn new() -> Self {
        Self {}
    }
}

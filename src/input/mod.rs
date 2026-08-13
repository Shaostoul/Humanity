//! Input types shared across the engine.
//!
//! `bindings` is the rebindable game keymap (Settings > Controls): action
//! enum, defaults, conflict rules, and the persisted override form. lib.rs's
//! raw winit handler resolves key events through it; `AppConfig` stores the
//! user's overrides.

pub mod bindings;

/// Current frame's input state, stored in DataStore as "input_state".
/// Updated each frame by the platform layer (native winit or WASM event listeners).
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Movement keys (WASD or equivalent).
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    /// Jump (Space).
    pub jump: bool,
    /// Interact key (E or left click).
    pub interact: bool,
    /// Mouse delta this frame (pixels of motion).
    pub mouse_dx: f32,
    pub mouse_dy: f32,
}

/// Processes raw input events into game actions.
pub struct InputManager {
    // TODO: action map, pressed/released state
}

impl InputManager {
    pub fn new() -> Self {
        Self {}
    }
}

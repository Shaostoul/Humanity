//! Multi-scale rendering — AU, planet, habitat scale transitions.
//!
//! Scale thresholds configured in `config/multi_scale.toml`.

/// Active rendering scale level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleLevel {
    /// Galaxy / interstellar (distances in AU or light-years)
    Stellar,
    /// Solar system / orbital (distances in km)
    Orbital,
    /// Planetary surface (distances in meters)
    Surface,
    /// Habitat interior (distances in meters, high detail)
    Habitat,
}

/// Manages transitions between rendering scale levels.
pub struct MultiScaleManager {
    pub current_scale: ScaleLevel,
}

impl MultiScaleManager {
    pub fn new() -> Self {
        Self {
            current_scale: ScaleLevel::Surface,
        }
    }
}

//! Spaceship systems — power, life support, shields, weapons.
//!
//! Ship class definitions loaded from `data/ship_classes.csv`.

use serde::{Deserialize, Serialize};

/// A spaceship's system status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipSystems {
    pub hull_integrity: f32,
    pub power_output_kw: f32,
    pub shield_strength: f32,
    pub fuel_remaining: f32,
}

impl Default for ShipSystems {
    fn default() -> Self {
        Self {
            hull_integrity: 1.0,
            power_output_kw: 100.0,
            shield_strength: 1.0,
            fuel_remaining: 1.0,
        }
    }
}

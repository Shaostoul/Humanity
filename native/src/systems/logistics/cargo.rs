//! Volumetric cargo containers for bulk transport.

use serde::{Deserialize, Serialize};

/// A cargo container with volumetric and mass limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoContainer {
    pub capacity_m3: f32,
    pub max_mass_kg: f32,
    pub used_m3: f32,
    pub used_mass_kg: f32,
}

impl CargoContainer {
    pub fn new(capacity_m3: f32, max_mass_kg: f32) -> Self {
        Self {
            capacity_m3,
            max_mass_kg,
            used_m3: 0.0,
            used_mass_kg: 0.0,
        }
    }
}

//! Engine types — ion, nuclear thermal, fusion, chemical.
//!
//! Propulsion specs loaded from `data/propulsion.csv`.

use serde::{Deserialize, Serialize};

/// Propulsion type with performance characteristics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropulsionDef {
    pub id: String,
    pub name: String,
    /// Specific impulse in seconds.
    pub isp_seconds: f32,
    /// Maximum thrust in kilonewtons.
    pub max_thrust_kn: f32,
    /// Fuel consumption rate in kg/s at max thrust.
    pub fuel_rate_kg_s: f32,
}

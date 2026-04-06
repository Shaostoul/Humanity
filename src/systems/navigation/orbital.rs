//! Keplerian orbital mechanics — ported from ProjectUniverse.
//!
//! Orbital parameters for bodies loaded from `data/systems.ron`.

use serde::{Deserialize, Serialize};

/// Classical Keplerian orbital elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitalElements {
    /// Semi-major axis in AU.
    pub semi_major_axis_au: f64,
    /// Eccentricity (0 = circle, <1 = ellipse).
    pub eccentricity: f64,
    /// Inclination in radians.
    pub inclination_rad: f64,
    /// Longitude of ascending node in radians.
    pub longitude_ascending_node_rad: f64,
    /// Argument of periapsis in radians.
    pub argument_periapsis_rad: f64,
    /// Mean anomaly at epoch in radians.
    pub mean_anomaly_rad: f64,
}

impl OrbitalElements {
    /// Compute position at a given time (stub — needs Kepler equation solver).
    pub fn position_at(&self, _time: f64) -> (f64, f64, f64) {
        // TODO: solve Kepler's equation, convert to Cartesian
        (0.0, 0.0, 0.0)
    }
}

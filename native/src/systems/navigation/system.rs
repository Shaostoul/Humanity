//! Solar system view — planets, moons, asteroid belts.
//!
//! System definitions loaded from `data/systems.ron`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// A celestial body in a solar system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CelestialBody {
    pub name: String,
    pub body_type: BodyType,
    pub position: Vec3,
    pub radius_km: f64,
    pub mass_kg: f64,
}

/// Type of celestial body.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BodyType {
    Star,
    Planet,
    Moon,
    Asteroid,
    Station,
}

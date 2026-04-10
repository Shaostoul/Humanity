//! Planet registry: central source of truth for all celestial bodies.
//!
//! Both the holographic map (renderer/hologram.rs) and the terrain system
//! (terrain/planet.rs) use this registry. When the player orbits Earth,
//! the same PlanetDef that defines the hologram's Earth also generates
//! the terrain they walk on.
//!
//! Data flows:
//!   data/solar_system/bodies.json → PlanetRegistry (loaded at startup)
//!   data/planets/earth.ron → PlanetDef details (terrain seeds, biomes)
//!   PlanetRegistry → HologramRenderer (for the solar system room view)
//!   PlanetRegistry → PlanetRenderer (for terrain LOD when approaching)
//!   PlanetRegistry → Maps page (for the 2D orbit visualization)

use glam::DVec3;
use std::collections::HashMap;
use super::planet::PlanetDef;

/// A celestial body in the registry (planet, moon, asteroid, star, station).
pub struct CelestialBody {
    pub id: String,
    pub name: String,
    pub body_type: BodyType,
    pub parent_id: Option<String>,
    /// Orbital radius from parent (meters).
    pub orbital_radius_m: f64,
    /// Orbital period (seconds).
    pub orbital_period_s: f64,
    /// Radius of the body itself (meters).
    pub radius_m: f64,
    /// Mass (kg).
    pub mass_kg: f64,
    /// Surface gravity (m/s^2).
    pub surface_gravity: f32,
    /// Current world-space position (updated per tick by orbital mechanics).
    pub world_position: DVec3,
    /// Detailed planet definition (loaded from RON, None for bodies without terrain).
    pub planet_def: Option<PlanetDef>,
    /// Display color for maps/hologram (fallback when no custom shader).
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    Star,
    Planet,
    DwarfPlanet,
    Moon,
    Asteroid,
    Comet,
    Station,
}

/// Central registry of all celestial bodies in the game.
pub struct PlanetRegistry {
    pub bodies: Vec<CelestialBody>,
    /// Quick lookup by ID.
    id_index: HashMap<String, usize>,
}

impl PlanetRegistry {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            id_index: HashMap::new(),
        }
    }

    /// Add a body to the registry.
    pub fn add(&mut self, body: CelestialBody) {
        let idx = self.bodies.len();
        self.id_index.insert(body.id.clone(), idx);
        self.bodies.push(body);
    }

    /// Find a body by ID.
    pub fn get(&self, id: &str) -> Option<&CelestialBody> {
        self.id_index.get(id).map(|&i| &self.bodies[i])
    }

    /// Get mutable reference by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut CelestialBody> {
        self.id_index.get(id).copied().map(move |i| &mut self.bodies[i])
    }

    /// Update orbital positions for all bodies based on elapsed time.
    /// Uses simple circular orbits (Keplerian approximation).
    pub fn update_orbits(&mut self, time_s: f64) {
        // First pass: compute positions for all bodies that orbit the sun (parent = star)
        let sun_pos = DVec3::ZERO;
        let count = self.bodies.len();

        // Collect parent positions first to avoid borrow issues
        let mut positions: Vec<DVec3> = vec![DVec3::ZERO; count];

        for i in 0..count {
            let body = &self.bodies[i];
            if body.parent_id.is_none() || body.body_type == BodyType::Star {
                // Star at origin
                positions[i] = sun_pos;
                continue;
            }

            let parent_pos = match &body.parent_id {
                Some(pid) => self.id_index.get(pid.as_str())
                    .map(|&pi| positions[pi])
                    .unwrap_or(sun_pos),
                None => sun_pos,
            };

            let period = body.orbital_period_s;
            if period > 0.0 {
                let angle = (time_s / period) * std::f64::consts::TAU;
                let r = body.orbital_radius_m;
                let pos = parent_pos + DVec3::new(
                    r * angle.cos(),
                    0.0,
                    r * angle.sin(),
                );
                positions[i] = pos;
            } else {
                positions[i] = parent_pos;
            }
        }

        // Write positions back
        for (i, pos) in positions.into_iter().enumerate() {
            self.bodies[i].world_position = pos;
        }
    }

    /// Find the nearest body to a world position.
    pub fn nearest(&self, pos: DVec3) -> Option<&CelestialBody> {
        self.bodies.iter()
            .min_by(|a, b| {
                let da = (a.world_position - pos).length();
                let db = (b.world_position - pos).length();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

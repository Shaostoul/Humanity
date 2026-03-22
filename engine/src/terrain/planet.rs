//! Planet renderer with icosphere-based LOD.
//!
//! Each planet is defined by a small data file (seed, radius, atmosphere, biomes).
//! The engine generates terrain procedurally from those seeds at runtime.
//! LOD is continuous: billboard at >100,000km, icosahedron at 10,000km,
//! progressively subdivided icosphere as the camera approaches, down to
//! 0.5m resolution heightmap at surface level.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::icosphere::Icosphere;

/// Planet definition loaded from a data file (e.g., planets/earth.ron).
/// A planet is ~200 bytes of seed data. The engine generates everything from this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetDef {
    /// Display name.
    pub name: String,
    /// Radius in meters (Earth = 6_371_000.0).
    pub radius: f64,
    /// Surface gravity in m/s^2 (Earth = 9.81).
    pub gravity: f32,
    /// Seed for procedural terrain generation.
    pub terrain_seed: u64,
    /// Seed for ore/resource distribution.
    pub ore_seed: u64,
    /// Atmosphere color (RGBA, None = no atmosphere).
    #[serde(default)]
    pub atmosphere_color: Option<[f32; 4]>,
    /// Atmosphere thickness as fraction of radius (Earth ~0.015).
    #[serde(default)]
    pub atmosphere_scale: f32,
    /// Has liquid water on surface.
    #[serde(default)]
    pub has_water: bool,
    /// Sea level as fraction of max elevation (0.0-1.0).
    #[serde(default)]
    pub sea_level: f32,
    /// Base surface color (land).
    #[serde(default = "default_land_color")]
    pub land_color: [f32; 4],
    /// Water/ocean color.
    #[serde(default = "default_water_color")]
    pub water_color: [f32; 4],
    /// Orbital position (meters from star, for rendering in solar system view).
    #[serde(default)]
    pub orbital_radius: f64,
    /// Orbital period in seconds.
    #[serde(default)]
    pub orbital_period: f64,
    /// Rotation period in seconds (day length).
    #[serde(default = "default_rotation_period")]
    pub rotation_period: f64,
    /// Axial tilt in radians.
    #[serde(default)]
    pub axial_tilt: f32,
}

fn default_land_color() -> [f32; 4] { [0.3, 0.5, 0.2, 1.0] }
fn default_water_color() -> [f32; 4] { [0.1, 0.3, 0.6, 1.0] }
fn default_rotation_period() -> f64 { 86400.0 } // 24 hours

/// LOD level for rendering a planet based on camera distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetLod {
    /// >100,000 km: flat billboard sprite with glow.
    Billboard,
    /// 10,000-100,000 km: low-poly sphere (subdivision 1, 80 faces).
    LowPoly,
    /// 1,000-10,000 km: mid-detail (subdivision 2-3).
    MidDetail,
    /// 100-1,000 km: high-detail (subdivision 4).
    HighDetail,
    /// 10-100 km: surface approach (subdivision 5+).
    SurfaceApproach,
    /// <10 km: walkable surface with heightmap detail.
    Surface,
}

impl PlanetLod {
    /// Determine LOD from camera distance to planet center (in meters).
    pub fn from_distance(distance_m: f64, radius_m: f64) -> Self {
        let altitude = distance_m - radius_m;
        if altitude > 100_000_000.0 {
            PlanetLod::Billboard
        } else if altitude > 10_000_000.0 {
            PlanetLod::LowPoly
        } else if altitude > 1_000_000.0 {
            PlanetLod::MidDetail
        } else if altitude > 100_000.0 {
            PlanetLod::HighDetail
        } else if altitude > 10_000.0 {
            PlanetLod::SurfaceApproach
        } else {
            PlanetLod::Surface
        }
    }

    /// Icosphere subdivision level for this LOD.
    pub fn subdivision_level(&self) -> u32 {
        match self {
            PlanetLod::Billboard => 0,
            PlanetLod::LowPoly => 1,
            PlanetLod::MidDetail => 3,
            PlanetLod::HighDetail => 4,
            PlanetLod::SurfaceApproach => 5,
            PlanetLod::Surface => 6, // further detail via heightmap within faces
        }
    }
}

/// Manages the rendering state for a single planet.
pub struct PlanetRenderer {
    pub def: PlanetDef,
    pub position: Vec3, // world-space position of the planet center
    current_lod: PlanetLod,
    /// Cached icosphere at the current subdivision level.
    icosphere: Icosphere,
    icosphere_level: u32,
}

impl PlanetRenderer {
    /// Create a renderer for a planet at the given world position.
    pub fn new(def: PlanetDef, position: Vec3) -> Self {
        let icosphere = Icosphere::new();
        Self {
            def,
            position,
            current_lod: PlanetLod::Billboard,
            icosphere,
            icosphere_level: 0,
        }
    }

    /// Update LOD based on camera distance. Returns true if LOD changed.
    pub fn update_lod(&mut self, camera_pos: Vec3) -> bool {
        let distance = (camera_pos - self.position).length() as f64;
        let new_lod = PlanetLod::from_distance(distance, self.def.radius);

        if new_lod != self.current_lod {
            let new_level = new_lod.subdivision_level();

            // Only regenerate icosphere if subdivision level changed
            if new_level != self.icosphere_level {
                log::info!(
                    "Planet '{}': LOD {:?} -> {:?} (subdivision {})",
                    self.def.name, self.current_lod, new_lod, new_level
                );
                self.icosphere = Icosphere::new();
                if new_level > 0 {
                    self.icosphere.subdivide_n(new_level);
                }
                self.icosphere_level = new_level;
            }

            self.current_lod = new_lod;
            return true;
        }
        false
    }

    /// Get the current LOD.
    pub fn lod(&self) -> PlanetLod {
        self.current_lod
    }

    /// Get the icosphere vertices scaled to planet radius (in meters).
    /// For rendering, you'll want to scale these relative to the camera.
    pub fn vertices(&self) -> Vec<Vec3> {
        self.icosphere.scaled_vertices(self.def.radius as f32)
    }

    /// Get the current icosphere faces.
    pub fn faces(&self) -> &[super::icosphere::Face] {
        &self.icosphere.faces
    }

    /// Get raw icosphere reference for mesh generation.
    pub fn icosphere(&self) -> &Icosphere {
        &self.icosphere
    }

    /// Get vertex count.
    pub fn vertex_count(&self) -> usize {
        self.icosphere.vertices.len()
    }

    /// Get face count.
    pub fn face_count(&self) -> usize {
        self.icosphere.faces.len()
    }
}

//! Planet definitions + icosphere LOD selection.
//!
//! Each planet is defined by a small data file (data/planets/<id>.ron: seed,
//! radius, atmosphere, palette). The engine generates everything procedurally
//! from those seeds at runtime (see `terrain::planet_surface` for the fractal
//! surface mesh).
//!
//! Two LOD choosers live here:
//! - SCREEN-SIZE LOD (v0.763, `projected_pixel_diameter` +
//!   `lod_level_for_pixels`): drives the sky planets seen from the homestead.
//!   The subdivision level follows the body's projected pixel size, so a
//!   speck stays a 20-face icosahedron and a looming planet subdivides up to
//!   the settings cap. Both knobs are live in Settings -> Graphics -> Planets.
//! - DISTANCE LOD (`PlanetLod` + `PlanetRenderer`): the original
//!   altitude-banded ladder, kept as the seed of the future landing arc
//!   (surface approach -> chunked near-surface subdivision -> walkable
//!   0.5 m heightmap detail). Not currently wired into the sky path.

use glam::{DVec3, Vec3};
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

    // ── Procedural surface parameters (v0.763) ──
    // Consumed by terrain::planet_surface to build the fractal sky-planet
    // mesh. All serde-defaulted so pre-existing RON files keep loading.

    /// Max outward displacement of land as a fraction of the radius.
    /// Small values keep the silhouette round-ish (Earth ~0.02).
    #[serde(default = "default_surface_relief")]
    pub surface_relief: f32,
    /// Base frequency of the elevation noise on the unit sphere. Higher =
    /// more, smaller continents/features.
    #[serde(default = "default_noise_frequency")]
    pub noise_frequency: f32,
    /// Octave count for the fine-detail noise layer (1-8). More octaves =
    /// rougher small-scale texture.
    #[serde(default = "default_noise_octaves")]
    pub noise_octaves: u32,
    /// Shoreline band color, just above sea level (water worlds only).
    #[serde(default = "default_shore_color")]
    pub shore_color: [f32; 4],
    /// Mid-elevation land color (between lowland `land_color` and mountains).
    #[serde(default = "default_highland_color")]
    pub highland_color: [f32; 4],
    /// High-elevation rock color.
    #[serde(default = "default_mountain_color")]
    pub mountain_color: [f32; 4],
    /// Snow/ice cap color: polar caps + the very highest peaks.
    #[serde(default = "default_cap_color")]
    pub cap_color: [f32; 4],
    /// Low-basin color for waterless worlds (lunar maria, Martian basins).
    /// None derives a darkened `land_color`.
    #[serde(default)]
    pub basin_color: Option<[f32; 4]>,
    /// Polar cap threshold on |sin(latitude)| (0.9 = caps above ~64 deg).
    /// Values above 1.0 disable polar caps entirely (e.g. the Moon).
    #[serde(default = "default_polar_cap_latitude")]
    pub polar_cap_latitude: f32,
    /// Optional REAL elevation grid, path relative to data/ (Earth ships
    /// "planets/earth_heightmap.bin", built from NOAA ETOPO1 by
    /// scripts/build-earth-heightmap.js). When present, vertex elevations
    /// come from this grid instead of the seeded noise, and `sea_level` is
    /// OVERRIDDEN at load time with the grid's true 0 m position so the
    /// real coastline is exact (see `PlanetHeightmap::sea_level_normalized`).
    /// None (every other planet) keeps the procedural noise path.
    #[serde(default)]
    pub heightmap: Option<String>,
}

fn default_land_color() -> [f32; 4] { [0.3, 0.5, 0.2, 1.0] }
fn default_water_color() -> [f32; 4] { [0.1, 0.3, 0.6, 1.0] }
fn default_rotation_period() -> f64 { 86400.0 } // 24 hours
fn default_surface_relief() -> f32 { 0.02 }
fn default_noise_frequency() -> f32 { 2.2 }
fn default_noise_octaves() -> u32 { 5 }
fn default_shore_color() -> [f32; 4] { [0.76, 0.70, 0.50, 1.0] }
fn default_highland_color() -> [f32; 4] { [0.45, 0.38, 0.24, 1.0] }
fn default_mountain_color() -> [f32; 4] { [0.50, 0.48, 0.46, 1.0] }
fn default_cap_color() -> [f32; 4] { [0.93, 0.95, 0.97, 1.0] }
fn default_polar_cap_latitude() -> f32 { 0.90 }

/// Hard ceiling for the sky-planet subdivision slider.
///
/// Raised 7 -> 9 (2026-07-11) for the FTL-travel era: up close, level 7's
/// 327,680 faces read as visibly low detail on a screen-filling Earth.
/// Face counts and approximate FLAT-SHADED mesh memory (3 unique verts per
/// face, 32-byte renderer vertex + 4-byte index = 108 B/face):
///   level 7:   327,680 faces  (~35 MB GPU)
///   level 8: 1,310,720 faces  (~142 MB GPU, ~1-2 s CPU build hitch)
///   level 9: 5,242,880 faces  (~566 MB GPU, several-second build hitch)
/// This stays affordable ONLY because the LOD ladder gates it: with the
/// default 10 px-per-level threshold, level 8 needs a >1280 px disc and
/// level 9 a >2560 px disc -- i.e. one planet filling (or overflowing) the
/// screen, which at most one body does at a time. The per-(body, level)
/// mesh cache holds every level built this session (no eviction yet -- a
/// noted follow-up in lib.rs::reload_planet_defs), so a full approach that
/// walks levels 0..9 parks ~750 MB of meshes for that body until the caches
/// clear. The Settings slider default stays well below the ceiling; the
/// ceiling exists so players with headroom CAN turn it up.
/// (Near-surface walking detail remains the future landing arc's
/// chunked-subdivision problem, not this whole-sphere path's.)
pub const MAX_SKY_SUBDIVISION: u32 = 9;

/// Projected on-screen diameter of a sphere, in pixels.
///
/// `radius` and `distance` share any unit (meters, render units) as long as
/// they agree. Uses the true angular diameter (atan) so it stays finite when
/// the camera is close, then maps angle to pixels through the vertical FOV.
pub fn projected_pixel_diameter(
    radius: f64,
    distance: f64,
    viewport_h_px: f32,
    fov_y_deg: f32,
) -> f32 {
    if radius <= 0.0 || distance <= 0.0 {
        return 0.0;
    }
    let angular_diameter = 2.0 * (radius / distance).atan();
    let fov_y_rad = (fov_y_deg.max(1.0) as f64).to_radians();
    ((angular_diameter / fov_y_rad) * viewport_h_px.max(1.0) as f64) as f32
}

/// Choose an icosphere subdivision level from a body's projected pixel size.
///
/// Doubling ladder: level 0 below `px_per_level` pixels, then each further
/// doubling of on-screen size adds one subdivision level (each subdivision
/// doubles linear mesh resolution, so faces keep a roughly constant pixel
/// size). With the default `px_per_level = 10`:
///   < 10 px  -> level 0 (20 faces)
///   10-20 px -> level 1 (80 faces)
///   20-40 px -> level 2 (320 faces)
///   40-80 px -> level 3 (1,280 faces)  ... capped at `max_level`.
/// Both knobs are runtime-adjustable from Settings -> Graphics -> Planets.
pub fn lod_level_for_pixels(px: f32, px_per_level: f32, max_level: u32) -> u32 {
    let base = px_per_level.max(1.0);
    if !(px > base) {
        // Also catches NaN: any comparison with NaN is false -> coarsest level.
        return 0;
    }
    let level = (px / base).log2().floor() as i64 + 1;
    (level.max(0) as u32).min(max_level.min(MAX_SKY_SUBDIVISION))
}

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
    ///
    /// Raised from the original (1/3/4/5/6) because at GEO altitude the
    /// LowPoly level of 80 faces reads as a visibly faceted blob rather
    /// than a planet. 3→5 subdivision levels (1,280 → 20,480 faces) give
    /// a visually round sphere at the distances players will encounter
    /// most often. Face counts:
    ///   level 0: 20 faces (base icosahedron)
    ///   level 3: 1,280 faces
    ///   level 5: 20,480 faces
    ///   level 6: 81,920 faces
    ///   level 7: 327,680 faces
    pub fn subdivision_level(&self) -> u32 {
        match self {
            PlanetLod::Billboard => 0,
            PlanetLod::LowPoly => 3,
            PlanetLod::MidDetail => 4,
            PlanetLod::HighDetail => 5,
            PlanetLod::SurfaceApproach => 6,
            PlanetLod::Surface => 7,
        }
    }
}

/// Manages the rendering state for a single planet.
pub struct PlanetRenderer {
    pub def: PlanetDef,
    /// World-space position of the planet center (f64 meters).
    pub world_position: DVec3,
    current_lod: PlanetLod,
    /// Cached icosphere at the current subdivision level.
    icosphere: Icosphere,
    icosphere_level: u32,
}

impl PlanetRenderer {
    /// Create a renderer for a planet at the given world position.
    pub fn new(def: PlanetDef, world_position: DVec3) -> Self {
        let icosphere = Icosphere::new();
        Self {
            def,
            world_position,
            current_lod: PlanetLod::Billboard,
            icosphere,
            icosphere_level: 0,
        }
    }

    /// Update LOD based on camera distance (f64). Returns true if LOD changed.
    pub fn update_lod(&mut self, camera_pos: DVec3) -> bool {
        let distance = (camera_pos - self.world_position).length();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_level_zero_below_base_threshold() {
        assert_eq!(lod_level_for_pixels(0.0, 10.0, 7), 0);
        assert_eq!(lod_level_for_pixels(5.0, 10.0, 7), 0);
        assert_eq!(lod_level_for_pixels(9.9, 10.0, 7), 0);
        assert_eq!(lod_level_for_pixels(10.0, 10.0, 7), 0); // boundary stays coarse
        assert_eq!(lod_level_for_pixels(f32::NAN, 10.0, 7), 0);
    }

    #[test]
    fn lod_level_ladder_doubles_per_level() {
        // base 10: level n starts at 10 * 2^(n-1) px.
        assert_eq!(lod_level_for_pixels(11.0, 10.0, 7), 1);
        assert_eq!(lod_level_for_pixels(19.9, 10.0, 7), 1);
        assert_eq!(lod_level_for_pixels(21.0, 10.0, 7), 2);
        assert_eq!(lod_level_for_pixels(39.0, 10.0, 7), 2);
        assert_eq!(lod_level_for_pixels(41.0, 10.0, 7), 3);
        assert_eq!(lod_level_for_pixels(81.0, 10.0, 7), 4);
        assert_eq!(lod_level_for_pixels(161.0, 10.0, 7), 5);
        assert_eq!(lod_level_for_pixels(321.0, 10.0, 7), 6);
    }

    #[test]
    fn lod_level_respects_max_cap() {
        assert_eq!(lod_level_for_pixels(100_000.0, 10.0, 3), 3);
        assert_eq!(lod_level_for_pixels(100_000.0, 10.0, 0), 0);
        // The hard ceiling wins even when the caller passes a bigger cap.
        assert_eq!(
            lod_level_for_pixels(1.0e12, 10.0, 99),
            MAX_SKY_SUBDIVISION
        );
    }

    #[test]
    fn projected_pixels_halve_with_double_distance() {
        let near = projected_pixel_diameter(6_371_000.0, 42_164_000.0, 1080.0, 60.0);
        let far = projected_pixel_diameter(6_371_000.0, 84_328_000.0, 1080.0, 60.0);
        assert!(near > 0.0);
        // Small-angle regime: doubling distance should ~halve the pixel size.
        let ratio = near / far;
        assert!((ratio - 2.0).abs() < 0.1, "ratio {ratio} not ~2");
        // Degenerate inputs are safe.
        assert_eq!(projected_pixel_diameter(0.0, 1.0, 1080.0, 60.0), 0.0);
        assert_eq!(projected_pixel_diameter(1.0, 0.0, 1080.0, 60.0), 0.0);
    }

    #[test]
    fn planet_defs_parse_from_ron_data_files() {
        // The shipped planet defs must keep deserializing as PlanetDef after
        // any schema change (all new fields are serde-defaulted).
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("planets");
        let mut parsed = 0;
        for entry in std::fs::read_dir(&dir).expect("data/planets missing") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read planet ron");
            let def: PlanetDef = ron::from_str(&text)
                .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
            assert!(def.radius > 0.0, "{} has non-positive radius", path.display());
            assert!(
                def.surface_relief >= 0.0 && def.surface_relief < 0.5,
                "{} surface_relief out of sane range",
                path.display()
            );
            parsed += 1;
        }
        assert!(parsed >= 3, "expected at least earth/mars/moon defs, got {parsed}");
    }
}

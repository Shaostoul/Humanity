//! Solar system hologram — miniature display for room-scale rendering.
//!
//! Loads celestial body data from `data/world/solar_system.ron` and generates
//! meshes for a tabletop-sized solar system with logarithmically scaled orbits.
//! Planet sizes are exaggerated for visibility. Fits within a 5x5 meter room.

use glam::Vec3;
use serde::Deserialize;
use std::f32::consts::PI;
use std::path::Path;

use super::mesh::{Mesh, Vertex};

// ── RON data structures ─────────────────────────────────────

/// Body type enum matching the RON file format.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum BodyType {
    Star,
    Planet,
    DwarfPlanet,
    Moon,
    AsteroidBelt,
}

/// A single celestial body as defined in solar_system.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct CelestialBodyDef {
    pub name: String,
    pub body_type: BodyType,
    pub parent: Option<String>,
    pub radius_m: f64,
    pub mass_kg: f64,
    pub orbital_radius_m: f64,
    pub orbital_period_s: f64,
    pub orbital_eccentricity: f64,
    pub orbital_inclination_deg: f64,
    pub rotation_period_s: f64,
    pub axial_tilt_deg: f64,
    pub color: (f32, f32, f32, f32),
    pub has_atmosphere: bool,
    pub has_rings: bool,
}

/// Top-level RON file structure.
#[derive(Debug, Deserialize)]
pub struct SolarSystemData {
    pub bodies: Vec<CelestialBodyDef>,
}

// ── Hologram output ─────────────────────────────────────────

/// One celestial body in the hologram display.
pub struct HologramBody {
    /// Human-readable name (e.g. "Earth").
    pub name: String,
    /// Body type from data.
    pub body_type: BodyType,
    /// Parent body name (None for Sun).
    pub parent: Option<String>,
    /// Position relative to hologram center, in meters.
    pub local_position: Vec3,
    /// Visual sphere radius in meters (exaggerated for visibility).
    pub radius: f32,
    /// RGBA base color.
    pub color: [f32; 4],
    /// Distance from parent center in hologram space (meters).
    pub orbit_radius: f32,
    /// Has rings (Saturn, Uranus, Neptune).
    pub has_rings: bool,
    /// Has atmosphere.
    pub has_atmosphere: bool,
}

/// Complete hologram data: bodies and their orbital parameters.
pub struct SolarSystemHologram {
    /// All bodies with position, size, and color.
    pub bodies: Vec<HologramBody>,
}

// ── Constants ───────────────────────────────────────────────

/// Maximum orbit radius in hologram space (meters). Sized to fit a 5x5 room
/// with some margin from walls.
const MAX_HOLOGRAM_RADIUS: f32 = 2.2;

/// Minimum orbit radius for innermost planet (meters).
const MIN_HOLOGRAM_RADIUS: f32 = 0.30;

/// Minimum visual radius for any body (so tiny moons are still visible).
const MIN_VISUAL_RADIUS: f32 = 0.012;

/// Maximum visual radius for the Sun (so it doesn't dominate the hologram).
const MAX_SUN_RADIUS: f32 = 0.08;

/// Maximum visual radius for planets.
const MAX_PLANET_RADIUS: f32 = 0.055;

/// Maximum visual radius for moons.
const MAX_MOON_RADIUS: f32 = 0.02;

// ── Loading ─────────────────────────────────────────────────

/// Load solar system data from the RON file on disk, falling back to embedded data.
pub fn load_solar_system(data_dir: &Path) -> Option<SolarSystemData> {
    // Try disk first (modding support)
    let path = data_dir.join("world").join("solar_system.ron");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => {
            log::info!("Loaded solar_system.ron from disk: {}", path.display());
            t
        }
        Err(_) => {
            // Fall back to embedded data
            match crate::embedded_data::get_embedded("world/solar_system.ron") {
                Some(embedded) => {
                    log::info!("Loaded solar_system.ron from embedded data");
                    embedded.to_string()
                }
                None => {
                    log::warn!("solar_system.ron not found on disk or embedded");
                    return None;
                }
            }
        }
    };

    match ron::from_str::<SolarSystemData>(&text) {
        Ok(data) => {
            log::info!("Parsed {} celestial bodies from solar_system.ron", data.bodies.len());
            Some(data)
        }
        Err(e) => {
            log::warn!("Failed to parse solar_system.ron: {e}");
            None
        }
    }
}

// ── Orbit scaling ───────────────────────────────────────────

/// Map an orbital radius in meters to hologram-space meters using log scaling.
/// Handles the massive range from Mercury (58M km) to Neptune (4.5B km).
fn orbit_to_hologram(orbit_m: f64, min_orbit: f64, max_orbit: f64) -> f32 {
    if orbit_m <= 0.0 || min_orbit <= 0.0 || max_orbit <= min_orbit {
        return 0.0;
    }
    let log_min = min_orbit.ln();
    let log_max = max_orbit.ln();
    let t = ((orbit_m.ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0) as f32;
    MIN_HOLOGRAM_RADIUS + t * (MAX_HOLOGRAM_RADIUS - MIN_HOLOGRAM_RADIUS)
}

/// Compute an exaggerated visual radius from real radius in meters.
/// Uses log scaling so gas giants are bigger than rocky planets but not overwhelming.
fn radius_to_visual(radius_m: f64, body_type: &BodyType) -> f32 {
    if radius_m <= 0.0 {
        return 0.0;
    }
    match body_type {
        BodyType::Star => MAX_SUN_RADIUS,
        BodyType::AsteroidBelt => 0.0,
        BodyType::Moon => {
            // Moons are smaller, capped to avoid clutter around parent
            let log_min = 6000.0_f64.ln();
            let log_max = 2_700_000.0_f64.ln(); // Ganymede (largest moon)
            let t = ((radius_m.ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0) as f32;
            MIN_VISUAL_RADIUS + t * (MAX_MOON_RADIUS - MIN_VISUAL_RADIUS)
        }
        _ => {
            // Planets and dwarf planets
            let log_min = 400_000.0_f64.ln();  // Ceres (~400km)
            let log_max = 70_000_000.0_f64.ln(); // Jupiter
            let t = ((radius_m.ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0) as f32;
            MIN_VISUAL_RADIUS + t * (MAX_PLANET_RADIUS - MIN_VISUAL_RADIUS)
        }
    }
}

// ── Generation ──────────────────────────────────────────────

/// Build the hologram from loaded RON data.
///
/// Bodies orbiting the Sun are placed in the XZ plane at log-scaled distances.
/// Moons are placed near their parent planet at a smaller scale.
/// All positions are relative to the hologram center (Sun at origin).
pub fn generate_hologram_from_data(data: &SolarSystemData) -> SolarSystemHologram {
    // Collect Sun-orbiting bodies to find orbit range
    let sun_orbiters: Vec<&CelestialBodyDef> = data.bodies.iter()
        .filter(|b| {
            b.parent.as_deref() == Some("Sun") &&
            b.body_type != BodyType::AsteroidBelt &&
            b.orbital_radius_m > 0.0
        })
        .collect();

    let min_orbit = sun_orbiters.iter()
        .map(|b| b.orbital_radius_m)
        .fold(f64::INFINITY, f64::min);
    let max_orbit = sun_orbiters.iter()
        .map(|b| b.orbital_radius_m)
        .fold(0.0_f64, f64::max);

    let mut bodies = Vec::with_capacity(data.bodies.len());

    // First pass: Sun + Sun-orbiting bodies
    let mut body_positions: std::collections::HashMap<String, Vec3> = std::collections::HashMap::new();

    for (i, def) in data.bodies.iter().enumerate() {
        match def.body_type {
            BodyType::AsteroidBelt => continue,
            _ => {}
        }

        if def.parent.is_none() || def.parent.as_deref() == Some("Sun") || def.parent.as_deref().is_none() {
            let orbit_radius = if def.orbital_radius_m <= 0.0 {
                0.0
            } else {
                orbit_to_hologram(def.orbital_radius_m, min_orbit, max_orbit)
            };

            // Golden angle spacing for Sun-orbiting bodies
            let angle = if def.body_type == BodyType::Star {
                0.0
            } else {
                (i as f32) * 2.399_655
            };

            let x = orbit_radius * angle.cos();
            let z = orbit_radius * angle.sin();
            let pos = Vec3::new(x, 0.0, z);

            body_positions.insert(def.name.clone(), pos);

            let visual_radius = radius_to_visual(def.radius_m, &def.body_type);

            bodies.push(HologramBody {
                name: def.name.clone(),
                body_type: def.body_type.clone(),
                parent: def.parent.clone(),
                local_position: pos,
                radius: visual_radius,
                color: [def.color.0, def.color.1, def.color.2, def.color.3],
                orbit_radius,
                has_rings: def.has_rings,
                has_atmosphere: def.has_atmosphere,
            });
        }
    }

    // Second pass: moons (orbit around their parent planet)
    for (i, def) in data.bodies.iter().enumerate() {
        if def.body_type != BodyType::Moon {
            continue;
        }

        let parent_name = match &def.parent {
            Some(p) => p.as_str(),
            None => continue,
        };

        let parent_pos = match body_positions.get(parent_name) {
            Some(pos) => *pos,
            None => continue,
        };

        // Scale moon orbits relative to parent: small offset
        // Log-scale from closest (~9,400km Phobos) to farthest (~3,560,000km Iapetus)
        let moon_min = 9_000_000.0_f64;   // 9,000 km
        let moon_max = 4_000_000_000.0_f64; // 4,000,000 km
        let orbit_r = {
            let log_min = moon_min.ln();
            let log_max = moon_max.ln();
            let t = ((def.orbital_radius_m.max(moon_min).ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0) as f32;
            0.06 + t * 0.10  // 6cm to 16cm from parent
        };
        let angle = (i as f32) * 2.399_655;
        let offset = Vec3::new(
            orbit_r * angle.cos(),
            0.0,
            orbit_r * angle.sin(),
        );
        let pos = parent_pos + offset;

        let visual_radius = radius_to_visual(def.radius_m, &def.body_type);

        bodies.push(HologramBody {
            name: def.name.clone(),
            body_type: def.body_type.clone(),
            parent: def.parent.clone(),
            local_position: pos,
            radius: visual_radius,
            color: [def.color.0, def.color.1, def.color.2, def.color.3],
            orbit_radius: orbit_r,
            has_rings: def.has_rings,
            has_atmosphere: def.has_atmosphere,
        });
    }

    SolarSystemHologram { bodies }
}

/// Fallback: generate hardcoded hologram if RON loading fails.
pub fn generate_hologram_fallback() -> SolarSystemHologram {
    struct PlanetDef {
        name: &'static str,
        orbit_au: f32,
        visual_radius: f32,
        color: [f32; 4],
    }

    let planets = [
        PlanetDef { name: "Sun",     orbit_au: 0.0,   visual_radius: 0.12, color: [1.0, 0.95, 0.8, 1.0] },
        PlanetDef { name: "Mercury", orbit_au: 0.39,  visual_radius: 0.015, color: [0.6, 0.55, 0.5, 1.0] },
        PlanetDef { name: "Venus",   orbit_au: 0.72,  visual_radius: 0.025, color: [0.9, 0.8, 0.5, 1.0] },
        PlanetDef { name: "Earth",   orbit_au: 1.0,   visual_radius: 0.03, color: [0.2, 0.4, 0.8, 1.0] },
        PlanetDef { name: "Mars",    orbit_au: 1.52,  visual_radius: 0.02, color: [0.8, 0.4, 0.2, 1.0] },
        PlanetDef { name: "Jupiter", orbit_au: 5.20,  visual_radius: 0.06, color: [0.8, 0.7, 0.5, 1.0] },
        PlanetDef { name: "Saturn",  orbit_au: 9.54,  visual_radius: 0.055, color: [0.9, 0.8, 0.55, 1.0] },
        PlanetDef { name: "Uranus",  orbit_au: 19.19, visual_radius: 0.04, color: [0.6, 0.8, 0.9, 1.0] },
        PlanetDef { name: "Neptune", orbit_au: 30.07, visual_radius: 0.035, color: [0.3, 0.4, 0.9, 1.0] },
    ];

    let log_min = (0.39_f32).ln();
    let log_max = (30.0_f32).ln();

    let mut bodies = Vec::with_capacity(planets.len());

    for (i, p) in planets.iter().enumerate() {
        let orbit_radius = if p.orbit_au == 0.0 {
            0.0
        } else {
            let t = (p.orbit_au.ln() - log_min) / (log_max - log_min);
            MIN_HOLOGRAM_RADIUS + t * (MAX_HOLOGRAM_RADIUS - MIN_HOLOGRAM_RADIUS)
        };

        let angle = if i == 0 { 0.0 } else { (i as f32) * 2.399_655 };
        let x = orbit_radius * angle.cos();
        let z = orbit_radius * angle.sin();

        bodies.push(HologramBody {
            name: p.name.to_string(),
            body_type: if i == 0 { BodyType::Star } else { BodyType::Planet },
            parent: if i == 0 { None } else { Some("Sun".to_string()) },
            local_position: Vec3::new(x, 0.0, z),
            radius: p.visual_radius,
            color: p.color,
            orbit_radius,
            has_rings: matches!(p.name, "Saturn" | "Uranus" | "Neptune"),
            has_atmosphere: !matches!(p.name, "Sun" | "Mercury"),
        });
    }

    SolarSystemHologram { bodies }
}

// ── Mesh generation ─────────────────────────────────────────

/// Generate a UV sphere mesh with the given radius and resolution.
pub fn sphere_mesh(device: &wgpu::Device, radius: f32, stacks: u32, slices: u32) -> Mesh {
    let mut vertices = Vec::with_capacity(((stacks + 1) * (slices + 1)) as usize);
    let mut indices = Vec::new();

    for stack in 0..=stacks {
        let phi = PI * stack as f32 / stacks as f32;
        let y = radius * phi.cos();
        let ring_r = radius * phi.sin();
        let v_coord = stack as f32 / stacks as f32;

        for slice in 0..=slices {
            let theta = 2.0 * PI * slice as f32 / slices as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();
            let u = slice as f32 / slices as f32;

            let nx = phi.sin() * theta.cos();
            let ny = phi.cos();
            let nz = phi.sin() * theta.sin();

            vertices.push(Vertex {
                position: [x, y, z],
                normal: [nx, ny, nz],
                uv: [u, v_coord],
            });
        }
    }

    for stack in 0..stacks {
        for slice in 0..slices {
            let first = stack * (slices + 1) + slice;
            let second = first + slices + 1;
            indices.push(first);
            indices.push(second);
            indices.push(first + 1);
            indices.push(second);
            indices.push(second + 1);
            indices.push(first + 1);
        }
    }

    Mesh::from_vertices(device, &vertices, &indices)
}

/// Generate a pin marker mesh: sphere head on a cone stem.
/// Origin at tip (bottom of pin), total height = stem_height + head_radius * 2.
pub fn pin_marker_mesh(device: &wgpu::Device, head_radius: f32, stem_height: f32) -> Mesh {
    let stem_radius = head_radius * 0.15;
    let stem_segments: u32 = 8;
    let sphere_stacks: u32 = 6;
    let sphere_slices: u32 = 8;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Stem cone: tip at Y=0, ring at Y=stem_height
    vertices.push(Vertex {
        position: [0.0, 0.0, 0.0],
        normal: [0.0, -1.0, 0.0],
        uv: [0.5, 0.0],
    });

    for seg in 0..=stem_segments {
        let theta = 2.0 * PI * seg as f32 / stem_segments as f32;
        let x = stem_radius * theta.cos();
        let z = stem_radius * theta.sin();
        vertices.push(Vertex {
            position: [x, stem_height, z],
            normal: [theta.cos(), 0.3, theta.sin()],
            uv: [seg as f32 / stem_segments as f32, 1.0],
        });
    }

    for seg in 0..stem_segments {
        indices.push(0);
        indices.push(1 + seg + 1);
        indices.push(1 + seg);
    }

    // Sphere head
    let base_idx = vertices.len() as u32;
    let sphere_center_y = stem_height + head_radius;

    for stack in 0..=sphere_stacks {
        let phi = PI * stack as f32 / sphere_stacks as f32;
        let y = head_radius * phi.cos() + sphere_center_y;
        let ring_r = head_radius * phi.sin();

        for slice in 0..=sphere_slices {
            let theta = 2.0 * PI * slice as f32 / sphere_slices as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            vertices.push(Vertex {
                position: [x, y, z],
                normal: [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()],
                uv: [slice as f32 / sphere_slices as f32, stack as f32 / sphere_stacks as f32],
            });
        }
    }

    for stack in 0..sphere_stacks {
        for slice in 0..sphere_slices {
            let first = base_idx + stack * (sphere_slices + 1) + slice;
            let second = first + sphere_slices + 1;
            indices.push(first);
            indices.push(second);
            indices.push(first + 1);
            indices.push(second);
            indices.push(second + 1);
            indices.push(first + 1);
        }
    }

    Mesh::from_vertices(device, &vertices, &indices)
}

/// Generate an orbit ring mesh as a tube torus in the XZ plane.
pub fn orbit_ring_mesh(device: &wgpu::Device, radius: f32, segments: u32) -> Mesh {
    let tube_r = 0.006; // 6mm radius tube for visibility
    let tube_sides: u32 = 6;

    let ring_verts = segments;
    let mut vertices = Vec::with_capacity(((ring_verts + 1) * (tube_sides + 1)) as usize);
    let mut indices = Vec::new();

    for seg in 0..=ring_verts {
        let theta = 2.0 * PI * seg as f32 / ring_verts as f32;
        let u = seg as f32 / ring_verts as f32;

        let cx = radius * theta.cos();
        let cz = radius * theta.sin();

        for side in 0..=tube_sides {
            let phi = 2.0 * PI * side as f32 / tube_sides as f32;

            let outward_x = theta.cos();
            let outward_z = theta.sin();

            let nx = phi.cos() * outward_x;
            let ny = phi.sin();
            let nz = phi.cos() * outward_z;

            let px = cx + tube_r * nx;
            let py = tube_r * ny;
            let pz = cz + tube_r * nz;

            vertices.push(Vertex {
                position: [px, py, pz],
                normal: [nx, ny, nz],
                uv: [u, side as f32 / tube_sides as f32],
            });
        }
    }

    for seg in 0..ring_verts {
        for side in 0..tube_sides {
            let a = seg * (tube_sides + 1) + side;
            let b = a + tube_sides + 1;
            indices.push(a);
            indices.push(b);
            indices.push(a + 1);
            indices.push(b);
            indices.push(b + 1);
            indices.push(a + 1);
        }
    }

    Mesh::from_vertices(device, &vertices, &indices)
}

/// Generate a flat ring mesh for Saturn-like rings.
/// Inner and outer radius define the ring band, rendered as a flat disc in XZ.
pub fn ring_disc_mesh(device: &wgpu::Device, inner_radius: f32, outer_radius: f32, segments: u32) -> Mesh {
    let mut vertices = Vec::with_capacity((2 * (segments + 1)) as usize);
    let mut indices = Vec::new();

    for seg in 0..=segments {
        let theta = 2.0 * PI * seg as f32 / segments as f32;
        let u = seg as f32 / segments as f32;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        // Inner vertex
        vertices.push(Vertex {
            position: [inner_radius * cos_t, 0.0, inner_radius * sin_t],
            normal: [0.0, 1.0, 0.0],
            uv: [u, 0.0],
        });
        // Outer vertex
        vertices.push(Vertex {
            position: [outer_radius * cos_t, 0.0, outer_radius * sin_t],
            normal: [0.0, 1.0, 0.0],
            uv: [u, 1.0],
        });
    }

    for seg in 0..segments {
        let i = seg * 2;
        // Top face (CCW from above)
        indices.push(i);
        indices.push(i + 1);
        indices.push(i + 2);
        indices.push(i + 1);
        indices.push(i + 3);
        indices.push(i + 2);
        // Bottom face (reverse winding)
        indices.push(i);
        indices.push(i + 2);
        indices.push(i + 1);
        indices.push(i + 1);
        indices.push(i + 2);
        indices.push(i + 3);
    }

    Mesh::from_vertices(device, &vertices, &indices)
}

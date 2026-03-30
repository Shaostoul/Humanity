//! Solar system hologram — miniature display for room-scale rendering.
//!
//! Generates mesh data for a tabletop-sized solar system with logarithmically
//! scaled orbits so both inner and outer planets remain visible within a ~4m
//! diameter hologram. Planet sizes are exaggerated for visibility.

use glam::Vec3;
use std::f32::consts::PI;

use super::mesh::{Mesh, Vertex};

/// One celestial body in the hologram display.
pub struct HologramBody {
    /// Human-readable name (e.g. "Earth").
    pub name: String,
    /// Position relative to hologram center, in meters.
    pub local_position: Vec3,
    /// Visual sphere radius in meters (exaggerated for visibility).
    pub radius: f32,
    /// RGBA base color.
    pub color: [f32; 4],
    /// Distance from center in hologram space (meters).
    pub orbit_radius: f32,
}

/// Complete hologram data: bodies and their orbital parameters.
pub struct SolarSystemHologram {
    /// Sun + 8 planets, each with position, size, and color.
    pub bodies: Vec<HologramBody>,
}

/// Planet definition used during generation.
struct PlanetDef {
    name: &'static str,
    /// Semi-major axis in AU (not used for the Sun).
    orbit_au: f32,
    /// Exaggerated visual radius in meters.
    visual_radius: f32,
    /// RGBA color.
    color: [f32; 4],
}

/// Map an orbital radius in AU to hologram-space meters using log scaling.
/// Mercury (0.39 AU) maps to ~0.3m, Neptune (30 AU) maps to ~2.0m.
fn au_to_hologram(au: f32) -> f32 {
    let log_min = (0.39_f32).ln();
    let log_max = (30.0_f32).ln();
    let t = (au.ln() - log_min) / (log_max - log_min);
    0.3 + t * (2.0 - 0.3)
}

/// Build the complete solar system hologram data.
///
/// All positions are relative to the hologram center (the Sun sits at origin).
/// The caller is responsible for placing the hologram in world space.
pub fn generate_hologram() -> SolarSystemHologram {
    let planets = [
        PlanetDef { name: "Sun",     orbit_au: 0.0,   visual_radius: 0.15, color: [1.0, 0.9, 0.3, 1.0] },
        PlanetDef { name: "Mercury", orbit_au: 0.39,  visual_radius: 0.02, color: [0.6, 0.6, 0.6, 1.0] },
        PlanetDef { name: "Venus",   orbit_au: 0.72,  visual_radius: 0.03, color: [0.9, 0.8, 0.5, 1.0] },
        PlanetDef { name: "Earth",   orbit_au: 1.0,   visual_radius: 0.035, color: [0.2, 0.4, 0.8, 1.0] },
        PlanetDef { name: "Mars",    orbit_au: 1.52,  visual_radius: 0.025, color: [0.8, 0.3, 0.2, 1.0] },
        PlanetDef { name: "Jupiter", orbit_au: 5.20,  visual_radius: 0.08, color: [0.8, 0.7, 0.5, 1.0] },
        PlanetDef { name: "Saturn",  orbit_au: 9.54,  visual_radius: 0.07, color: [0.9, 0.8, 0.5, 1.0] },
        PlanetDef { name: "Uranus",  orbit_au: 19.19, visual_radius: 0.05, color: [0.5, 0.8, 0.9, 1.0] },
        PlanetDef { name: "Neptune", orbit_au: 30.07, visual_radius: 0.04, color: [0.2, 0.3, 0.8, 1.0] },
    ];

    let mut bodies = Vec::with_capacity(planets.len());

    for (i, p) in planets.iter().enumerate() {
        let orbit_radius = if p.orbit_au == 0.0 {
            0.0
        } else {
            au_to_hologram(p.orbit_au)
        };

        // Spread planets around the circle so the initial view looks nice.
        // Golden-angle spacing avoids clustering.
        let angle = if i == 0 {
            0.0
        } else {
            (i as f32) * 2.399_655 // golden angle in radians
        };

        let x = orbit_radius * angle.cos();
        let z = orbit_radius * angle.sin();

        bodies.push(HologramBody {
            name: p.name.to_string(),
            local_position: Vec3::new(x, 0.0, z),
            radius: p.visual_radius,
            color: p.color,
            orbit_radius,
        });
    }

    SolarSystemHologram { bodies }
}

/// Generate a UV sphere mesh with the given radius and resolution.
///
/// Suitable for planet bodies in the hologram. Uses the engine's Vertex format.
pub fn sphere_mesh(device: &wgpu::Device, radius: f32, stacks: u32, slices: u32) -> Mesh {
    let mut vertices = Vec::with_capacity(((stacks + 1) * (slices + 1)) as usize);
    let mut indices = Vec::new();

    for stack in 0..=stacks {
        let phi = PI * stack as f32 / stacks as f32; // 0 at top pole, PI at bottom
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

    // Triangle strip indices
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

/// Generate an orbit ring mesh as a thin tube in the XZ plane.
///
/// Since wgpu does not guarantee line-primitive support across all backends,
/// this builds a tube with a circular cross-section around the orbit path.
/// The ring lies flat at Y=0 relative to the hologram center.
pub fn orbit_ring_mesh(device: &wgpu::Device, radius: f32, segments: u32) -> Mesh {
    // Tube cross-section radius (thin enough to read as a line).
    let tube_r = 0.003;
    // Cross-section resolution (6 sides is plenty for a 3mm tube).
    let tube_sides: u32 = 6;

    let ring_verts = segments;
    let total_verts = (ring_verts + 1) * (tube_sides + 1);
    let mut vertices = Vec::with_capacity(total_verts as usize);
    let mut indices = Vec::new();

    for seg in 0..=ring_verts {
        let theta = 2.0 * PI * seg as f32 / ring_verts as f32;
        let u = seg as f32 / ring_verts as f32;

        // Center of the tube cross-section on the ring
        let cx = radius * theta.cos();
        let cz = radius * theta.sin();

        // Tangent along the ring (derivative of center position w.r.t. theta)
        let tx = -theta.sin();
        let tz = theta.cos();

        // Bitangent is simply Y-up for a flat ring
        // Normal of cross-section rotates around the tangent
        for side in 0..=tube_sides {
            let phi = 2.0 * PI * side as f32 / tube_sides as f32;

            // Cross-section offset: radial direction in the plane perpendicular to tangent
            // radial = cos(phi) * outward + sin(phi) * up
            let outward_x = theta.cos(); // points away from ring center
            let outward_z = theta.sin();

            let nx = phi.cos() * outward_x;
            let ny = phi.sin();
            let nz = phi.cos() * outward_z;

            let px = cx + tube_r * nx;
            let py = tube_r * ny;
            let pz = cz + tube_r * nz;

            let v_coord = side as f32 / tube_sides as f32;

            vertices.push(Vertex {
                position: [px, py, pz],
                normal: [nx, ny, nz],
                uv: [u, v_coord],
            });
        }
    }

    // Build triangle indices connecting adjacent cross-sections
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

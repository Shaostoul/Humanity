//! Procedural fractal planet surfaces for the sky renderer (v0.763).
//!
//! Turns a `PlanetDef` (data/planets/<id>.ron) + an icosphere subdivision
//! level into CPU-side mesh data: per-vertex elevation from seeded 3D Perlin
//! FBM, outward displacement for land (oceans stay smooth at the sphere
//! radius, so seas read as flat blue and noise basins below sea level become
//! lakes for free), and a per-face color classified from elevation +
//! latitude (ocean / shore / lowland / highland / mountain / polar cap).
//!
//! Faces are FLAT-SHADED: each triangle gets 3 unique vertices sharing one
//! color and one normal. That is both the intended low-poly-planet look and
//! a hard requirement of the color transport: the renderer packs the RGB
//! color into the 2-float UV channel (`pack_color_to_uv`), which only
//! survives rasterizer interpolation when all three corners carry identical
//! values.
//!
//! Everything in this module is pure math (no GPU), so it is fully unit
//! tested headless. The GPU hop lives in `renderer::mesh::Mesh::
//! from_planet_surface`.
//!
//! Deliberately out of scope (documented follow-ups, not omissions):
//! - Rivers / erosion: needs flow simulation over the elevation field.
//! - Chunked near-surface subdivision for the 1 m walking-resolution
//!   landing arc (this module subdivides the WHOLE sphere, which is the
//!   right shape for sky bodies but not for standing on one).
//! - Hooking the orphaned `data/biomes.ron` palette in as a richer
//!   classification source than the per-planet band palette.

use glam::Vec3;
use noise::{NoiseFn, Perlin};

use super::icosphere::Icosphere;
use super::planet::PlanetDef;

/// One flat-shaded vertex of a generated planet surface, unit-sphere scale
/// (radius 1.0 = the planet's nominal radius; the renderer scales at draw
/// time so one cached mesh serves any display size).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceVertexData {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// CPU-side planet surface mesh: flat-shaded triangles, sequential indices.
pub struct SurfaceMeshData {
    pub vertices: Vec<SurfaceVertexData>,
    pub indices: Vec<u32>,
}

/// Seeded elevation sampler. Same seed + params -> identical values forever
/// (tests rely on this; so does multiplayer, where every client re-derives
/// the same planet instead of syncing meshes).
pub struct SurfaceSampler {
    continental: Perlin,
    mountain: Perlin,
    detail: Perlin,
    frequency: f64,
    detail_octaves: u32,
}

impl SurfaceSampler {
    pub fn new(def: &PlanetDef) -> Self {
        let s = def.terrain_seed as u32;
        Self {
            continental: Perlin::new(s),
            mountain: Perlin::new(s.wrapping_add(1)),
            detail: Perlin::new(s.wrapping_add(2)),
            frequency: def.noise_frequency.max(0.01) as f64,
            detail_octaves: def.noise_octaves.clamp(1, 8),
        }
    }

    /// Elevation in [0, 1] at a unit-sphere direction. Sampled in 3D so
    /// there is no polar pinching (lat/lon-space noise stretches at poles).
    pub fn elevation_at(&self, unit: Vec3) -> f32 {
        let p = [
            unit.x as f64 * self.frequency,
            unit.y as f64 * self.frequency,
            unit.z as f64 * self.frequency,
        ];
        // FBM helper: `octaves` samples at doubling frequency, halving
        // amplitude, normalized back to roughly [-1, 1].
        let fbm = |n: &Perlin, base: f64, octaves: u32| -> f32 {
            let mut amp = 1.0_f64;
            let mut freq = base;
            let mut sum = 0.0_f64;
            let mut norm = 0.0_f64;
            for _ in 0..octaves {
                sum += amp * n.get([p[0] * freq, p[1] * freq, p[2] * freq]);
                norm += amp;
                amp *= 0.5;
                freq *= 2.0;
            }
            (sum / norm.max(1e-9)) as f32
        };

        // Layer weights mirror heightmap::TerrainGenerator (the walkable
        // terrain sibling): continents dominate, ridged mountains add peaks,
        // the detail layer adds grit.
        let continental = fbm(&self.continental, 1.0, 2) * 0.5 + 0.5;
        let m = fbm(&self.mountain, 4.0, 3);
        let mountain = if m > 0.0 { m * m } else { 0.0 };
        let detail = fbm(&self.detail, 8.0, self.detail_octaves) * 0.5 + 0.5;

        (continental * 0.55 + mountain * 0.30 + detail * 0.15).clamp(0.0, 1.0)
    }
}

fn rgb(c: [f32; 4]) -> [f32; 3] {
    [c[0], c[1], c[2]]
}

/// Radius multiplier for a given elevation (1.0 = the nominal sphere).
///
/// Water worlds never displace below sea level: the ocean is a smooth
/// sphere, and land rises out of it up to `surface_relief`. Waterless
/// worlds displace both ways, so basins are real depressions.
pub fn displaced_radius(def: &PlanetDef, elevation: f32) -> f32 {
    let sea = def.sea_level.clamp(0.0, 1.0);
    let relief = def.surface_relief.max(0.0);
    let e = if def.has_water {
        (elevation - sea).max(0.0)
    } else {
        elevation - sea
    };
    1.0 + e * relief
}

/// Classify a surface color from elevation and |sin(latitude)|.
///
/// Bands, in priority order: polar cap (threshold slightly relaxed at high
/// elevation so mountains snow first; > 1.0 disables caps), ocean with depth
/// shading (or dark basins on waterless worlds), shoreline, lowland,
/// highland, mountain, and cap again for the very highest peaks.
pub fn classify_color(def: &PlanetDef, elevation: f32, abs_sin_lat: f32) -> [f32; 3] {
    let sea = def.sea_level.clamp(0.0, 1.0);
    let land_t = if sea < 1.0 {
        ((elevation - sea) / (1.0 - sea)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Polar caps: also cover polar ocean (frozen sea ice), and reach a bit
    // further from the pole at high elevation.
    if def.polar_cap_latitude <= 1.0 {
        let relax = if elevation >= sea { land_t * 0.08 } else { 0.0 };
        if abs_sin_lat > def.polar_cap_latitude - relax {
            return rgb(def.cap_color);
        }
    }

    if elevation < sea {
        if def.has_water {
            // Deeper water is darker; shallows keep the base ocean color.
            let depth = if sea > 0.0 {
                ((sea - elevation) / sea).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let w = rgb(def.water_color);
            let k = 1.0 - depth * 0.45;
            return [w[0] * k, w[1] * k, w[2] * k];
        }
        // Dry world: dark low basins (lunar maria, Martian lowlands).
        return def
            .basin_color
            .map(rgb)
            .unwrap_or_else(|| {
                let l = rgb(def.land_color);
                [l[0] * 0.6, l[1] * 0.6, l[2] * 0.6]
            });
    }

    if def.has_water && land_t < 0.03 {
        return rgb(def.shore_color);
    }
    if land_t < 0.45 {
        return rgb(def.land_color);
    }
    if land_t < 0.72 {
        return rgb(def.highland_color);
    }
    if land_t < 0.92 {
        return rgb(def.mountain_color);
    }
    // Highest peaks cap regardless of latitude.
    rgb(def.cap_color)
}

/// Pack an RGB color into the 2-float UV channel so the standard PBR vertex
/// layout can carry per-face colors without a new pipeline.
///
/// `uv.x = round(r*255)*256 + round(g*255)` -- an exact integer (<= 65535,
/// well inside f32's 2^24 exact-integer range). `uv.y = b` as a plain float.
/// Because every corner of a flat-shaded face stores the SAME value, linear
/// interpolation across the triangle is a constant and the packed integer
/// survives to the fragment shader (material type 12 decodes it).
pub fn pack_color_to_uv(c: [f32; 3]) -> [f32; 2] {
    let r = (c[0].clamp(0.0, 1.0) * 255.0).round();
    let g = (c[1].clamp(0.0, 1.0) * 255.0).round();
    [r * 256.0 + g, c[2].clamp(0.0, 1.0)]
}

/// Rust mirror of the WGSL decode in pbr_simple.wgsl (material type 12).
/// Exists so a unit test locks the round-trip; keep both in sync.
pub fn unpack_uv_to_color(uv: [f32; 2]) -> [f32; 3] {
    let packed = uv[0].round().max(0.0) as u32;
    [
        ((packed >> 8) & 255) as f32 / 255.0,
        (packed & 255) as f32 / 255.0,
        uv[1],
    ]
}

/// Build the flat-shaded procedural surface mesh for a planet at the given
/// icosphere subdivision level, at unit radius (scale at draw time).
pub fn build_surface_mesh(def: &PlanetDef, level: u32) -> SurfaceMeshData {
    let mut ico = Icosphere::new();
    ico.subdivide_n(level);

    let sampler = SurfaceSampler::new(def);
    let sea = def.sea_level.clamp(0.0, 1.0);

    // Per-icosphere-vertex elevation + displaced position, computed once and
    // shared by every face touching the vertex (positions must agree across
    // faces or the surface cracks).
    let elev: Vec<f32> = ico.vertices.iter().map(|v| sampler.elevation_at(*v)).collect();
    let pos: Vec<Vec3> = ico
        .vertices
        .iter()
        .zip(&elev)
        .map(|(v, e)| *v * displaced_radius(def, *e))
        .collect();

    let mut vertices: Vec<SurfaceVertexData> = Vec::with_capacity(ico.faces.len() * 3);
    let mut indices: Vec<u32> = Vec::with_capacity(ico.faces.len() * 3);

    for face in &ico.faces {
        let (i0, i1, i2) = (face.v0 as usize, face.v1 as usize, face.v2 as usize);
        let (p0, p1, p2) = (pos[i0], pos[i1], pos[i2]);
        let mean_e = (elev[i0] + elev[i1] + elev[i2]) / 3.0;
        let centroid_dir =
            (ico.vertices[i0] + ico.vertices[i1] + ico.vertices[i2]).normalize_or_zero();
        // On a unit sphere, y IS sin(latitude).
        let color = classify_color(def, mean_e, centroid_dir.y.abs());

        let underwater = def.has_water && mean_e < sea;
        if underwater {
            // Smooth ocean: per-corner spherical normals, undisplaced radius.
            for &i in &[i0, i1, i2] {
                let n = ico.vertices[i].normalize_or_zero();
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: (pos[i]).to_array(),
                    normal: n.to_array(),
                    color,
                });
            }
        } else {
            // Land: flat geometric normal so slopes catch the sun.
            let mut n = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            if n.length_squared() < 1e-9 || n.dot(centroid_dir) < 0.0 {
                // Degenerate or inward-wound face: fall back to the outward
                // spherical direction (never render an inside-out face).
                n = centroid_dir;
            }
            for &p in &[p0, p1, p2] {
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: p.to_array(),
                    normal: n.to_array(),
                    color,
                });
            }
        }
    }

    SurfaceMeshData { vertices, indices }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A watery Earth-like test def with fixed parameters.
    fn water_world(seed: u64) -> PlanetDef {
        let mut def: PlanetDef = ron::from_str(
            r#"(
                name: "Testworld",
                radius: 6371000.0,
                gravity: 9.81,
                terrain_seed: 0,
                ore_seed: 1,
                has_water: true,
                sea_level: 0.55,
            )"#,
        )
        .expect("test def parses");
        def.terrain_seed = seed;
        def
    }

    fn dry_world(seed: u64) -> PlanetDef {
        let mut def = water_world(seed);
        def.has_water = false;
        def.sea_level = 0.45;
        def.basin_color = Some([0.2, 0.2, 0.22, 1.0]);
        def.polar_cap_latitude = 2.0; // caps disabled
        def
    }

    #[test]
    fn icosphere_face_counts_match_formula() {
        for level in 0..4u32 {
            let mut ico = Icosphere::new();
            ico.subdivide_n(level);
            let expected = Icosphere::face_count_at_level(level) as usize;
            assert_eq!(ico.faces.len(), expected, "level {level}");
        }
        assert_eq!(Icosphere::face_count_at_level(0), 20);
        assert_eq!(Icosphere::face_count_at_level(1), 80);
        assert_eq!(Icosphere::face_count_at_level(2), 320);
        assert_eq!(Icosphere::face_count_at_level(5), 20_480);
        assert_eq!(Icosphere::face_count_at_level(7), 327_680);
    }

    #[test]
    fn surface_mesh_is_flat_shaded_with_expected_counts() {
        let def = water_world(42);
        let data = build_surface_mesh(&def, 2);
        let faces = Icosphere::face_count_at_level(2) as usize;
        assert_eq!(data.vertices.len(), faces * 3);
        assert_eq!(data.indices.len(), faces * 3);
        // Sequential indices (flat shading, no vertex sharing).
        assert!(data.indices.iter().enumerate().all(|(i, &v)| v as usize == i));
    }

    #[test]
    fn elevation_deterministic_same_seed() {
        let def = water_world(1234);
        let a = SurfaceSampler::new(&def);
        let b = SurfaceSampler::new(&def);
        let dirs = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.577, 0.577, 0.577),
            Vec3::new(-0.267, 0.535, -0.802),
        ];
        for d in dirs {
            let ea = a.elevation_at(d);
            let eb = b.elevation_at(d);
            assert_eq!(ea, eb, "same seed must be bit-identical at {d:?}");
            assert!((0.0..=1.0).contains(&ea), "elevation out of range: {ea}");
        }
        // And the full mesh is byte-identical across two builds.
        let m1 = build_surface_mesh(&def, 1);
        let m2 = build_surface_mesh(&def, 1);
        assert_eq!(m1.vertices, m2.vertices);
        assert_eq!(m1.indices, m2.indices);
    }

    #[test]
    fn elevation_differs_across_seeds() {
        let a = SurfaceSampler::new(&water_world(1));
        let b = SurfaceSampler::new(&water_world(2));
        // At least one probe direction must differ between seeds.
        let dirs = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.577, 0.577, 0.577),
        ];
        assert!(
            dirs.iter().any(|d| a.elevation_at(*d) != b.elevation_at(*d)),
            "different seeds produced identical elevations at every probe"
        );
    }

    #[test]
    fn ocean_stays_at_sphere_radius_land_within_relief() {
        let def = water_world(42);
        let data = build_surface_mesh(&def, 3);
        let max_r = 1.0 + def.surface_relief + 1e-4;
        for v in &data.vertices {
            let r = Vec3::from_array(v.position).length();
            // Water worlds never dip below the sphere...
            assert!(r >= 1.0 - 1e-4, "vertex below ocean radius: {r}");
            // ...and land never exceeds the relief budget.
            assert!(r <= max_r, "vertex beyond relief budget: {r}");
        }
        // The ocean must actually exist at this sea level: some vertices sit
        // exactly on the sphere.
        let on_sphere = data
            .vertices
            .iter()
            .filter(|v| (Vec3::from_array(v.position).length() - 1.0).abs() < 1e-5)
            .count();
        assert!(on_sphere > 0, "expected ocean vertices at radius 1.0");
    }

    #[test]
    fn dry_world_has_real_depressions() {
        let def = dry_world(7);
        let data = build_surface_mesh(&def, 3);
        let below = data
            .vertices
            .iter()
            .any(|v| Vec3::from_array(v.position).length() < 1.0 - 1e-5);
        assert!(below, "waterless world should displace basins inward");
    }

    #[test]
    fn classify_below_sea_is_water_and_deep_is_darker() {
        let def = water_world(42);
        let shallow = classify_color(&def, def.sea_level - 0.01, 0.0);
        let deep = classify_color(&def, 0.0, 0.0);
        let w = [def.water_color[0], def.water_color[1], def.water_color[2]];
        // Shallow water is close to the base ocean color.
        for i in 0..3 {
            assert!((shallow[i] - w[i]).abs() < 0.05, "shallow ch{i} far from ocean");
        }
        // Deep water is strictly darker on every channel that has any color.
        for i in 0..3 {
            assert!(deep[i] <= shallow[i] + 1e-6, "deep ch{i} not darker");
        }
        assert!(deep.iter().sum::<f32>() < shallow.iter().sum::<f32>());
    }

    #[test]
    fn classify_bands_progress_with_elevation() {
        let def = water_world(42);
        let sea = def.sea_level;
        let t = |land_t: f32| sea + land_t * (1.0 - sea);
        assert_eq!(classify_color(&def, t(0.01), 0.0), rgb(def.shore_color));
        assert_eq!(classify_color(&def, t(0.20), 0.0), rgb(def.land_color));
        assert_eq!(classify_color(&def, t(0.60), 0.0), rgb(def.highland_color));
        assert_eq!(classify_color(&def, t(0.85), 0.0), rgb(def.mountain_color));
        // Highest peaks snow-cap even at the equator.
        assert_eq!(classify_color(&def, t(0.99), 0.0), rgb(def.cap_color));
    }

    #[test]
    fn classify_polar_is_cap_even_over_ocean() {
        let def = water_world(42);
        // Above the cap latitude both land and ocean freeze.
        assert_eq!(classify_color(&def, 0.1, 0.99), rgb(def.cap_color));
        assert_eq!(
            classify_color(&def, def.sea_level + 0.05, 0.99),
            rgb(def.cap_color)
        );
    }

    #[test]
    fn dry_world_below_sea_is_basin_not_water_and_no_caps() {
        let def = dry_world(7);
        let basin = classify_color(&def, 0.1, 0.0);
        assert_eq!(basin, [0.2, 0.2, 0.22]);
        // polar_cap_latitude > 1.0 disables caps: poles stay basin/land.
        let polar = classify_color(&def, 0.1, 1.0);
        assert_eq!(polar, basin);
    }

    #[test]
    fn pack_unpack_color_roundtrip() {
        let samples = [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.25, 0.5, 0.75],
            [0.93, 0.95, 0.97],
            [0.1, 0.3, 0.6],
        ];
        for c in samples {
            let uv = pack_color_to_uv(c);
            let back = unpack_uv_to_color(uv);
            for i in 0..3 {
                assert!(
                    (back[i] - c[i]).abs() <= 1.0 / 255.0 + 1e-6,
                    "channel {i} of {c:?} round-tripped to {back:?}"
                );
            }
            // The packed x component must be an exact integer (interpolation
            // safety depends on it).
            assert_eq!(uv[0].fract(), 0.0);
            assert!(uv[0] <= 65535.0);
        }
    }
}

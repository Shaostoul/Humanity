//! Procedural fractal planet surfaces for the sky renderer (v0.763).
//!
//! Turns a `PlanetDef` (data/planets/<id>.ron) + an icosphere subdivision
//! level into CPU-side mesh data: per-vertex elevation from seeded 3D Perlin
//! FBM -- or, when the def ships a real elevation grid (Earth: NOAA ETOPO1
//! via `terrain::planet_heightmap`), from bilinear samples of that grid, so
//! the sky planet shows REAL continents -- outward displacement for land
//! (oceans stay smooth at the sphere radius, so seas read as flat blue and
//! basins below sea level become lakes for free), and a per-face color from
//! `surface_color`: bilinear samples of a REAL albedo grid when the def
//! ships one (Earth: NASA Blue Marble via `terrain::planet_albedo`, so the
//! planet wears its actual photographed surface), otherwise the elevation +
//! latitude band classifier (ocean / shore / lowland / highland / mountain /
//! polar cap). Land face colors are additionally darkened by slope
//! (`slope_shade`) so mountain relief reads even under noon lighting.
//!
//! Faces are FLAT-SHADED: each triangle gets 3 unique vertices sharing one
//! color and one normal. That is both the intended low-poly-planet look and
//! a hard requirement of the color transport: the renderer packs the RGB
//! color (plus a water flag that drives the shader's ocean sun glint) into
//! the 2-float UV channel (`pack_color_to_uv`), which only survives
//! rasterizer interpolation when all three corners carry identical values.
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
use super::planet_albedo::PlanetAlbedo;
use super::planet_heightmap::PlanetHeightmap;

/// One flat-shaded vertex of a generated planet surface, unit-sphere scale
/// (radius 1.0 = the planet's nominal radius; the renderer scales at draw
/// time so one cached mesh serves any display size).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceVertexData {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    /// True on ocean faces (below-sea-level faces of `has_water` planets --
    /// exactly the faces the geometry keeps smooth at the sphere radius).
    /// Rides to the GPU as bit 16 of the packed UV so the shader can put a
    /// sun glint on water and nowhere else. Per FACE (all three corners
    /// carry the same value; flat-shading transport requires it).
    pub water: bool,
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

/// f64 twin of `displaced_radius` for the chunked-LOD patch path
/// (terrain::planet_chunks), where positions are composed in f64 so that
/// planet-radius magnitudes keep sub-meter precision (an f32 multiply at
/// 6.4e6 m already rounds by ~0.5 m). Keep the FORMULA in lockstep with
/// the f32 version above; a unit test asserts they agree.
pub fn displaced_radius_f64(def: &PlanetDef, elevation: f64) -> f64 {
    let sea = def.sea_level.clamp(0.0, 1.0) as f64;
    let relief = def.surface_relief.max(0.0) as f64;
    let e = if def.has_water {
        (elevation - sea).max(0.0)
    } else {
        elevation - sea
    };
    1.0 + e * relief
}

/// TRUE (bathymetric) displacement: the waterless formula applied even on
/// water worlds, used by the chunked path when a connected-ocean mask is
/// present (v0.876 real-water Stage 1) -- the seafloor and dry basins are
/// real depressions and the separate ocean shell draws the water. Above
/// sea level this agrees with displaced_radius_f64 exactly.
pub fn displaced_radius_f64_true(def: &PlanetDef, elevation: f64) -> f64 {
    let sea = def.sea_level.clamp(0.0, 1.0) as f64;
    let relief = def.surface_relief.max(0.0) as f64;
    1.0 + (elevation - sea) * relief
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

/// Orbital-look ocean floor (linear RGB): the Blue Marble topo-bathy source
/// paints deep ocean almost black (sRGB ~rgb(1,5,20) -> linear ~0.006 blue),
/// which is technically what the water itself reflects -- real orbital
/// photos read blue because sunlight scattered by the air IN FRONT of the
/// disc adds blue radiance. Increment 1 of the atmosphere does not yet add
/// enough in-disc scattering to supply that, so the compensation lives here:
/// each channel of a water face is floored (component-wise max) to this
/// color. Bright turquoise shelf shallows in the imagery punch through the
/// floor untouched; the abyssal plains lift to a uniform photo blue.
/// Delete this (return to the raw imagery) when the atmosphere gains real
/// in-disc radiance.
pub const OCEAN_ORBITAL_FLOOR: [f32; 3] = [0.010, 0.055, 0.22];

/// Land brightness gain on the albedo path. The linear-decoded Blue Marble
/// land (Amazon green ~0.02 linear) is physically plausible albedo but
/// reads near-black through this pipeline's sun intensity + ACES tonemap,
/// where the classifier palette it replaced was authored ~3x brighter.
/// A flat gain keeps the imagery's hue and relative contrast.
pub const LAND_ALBEDO_GAIN: f32 = 1.6;
/// Dark-land shadow lift (v0.908): linear-luma knee below which land pixels
/// ride a hue-preserving power curve toward daylight brightness. Blue
/// Marble vegetation measures ~20x darker than desert in linear light;
/// this closes most of that gap without touching bright terrain.
pub const LAND_SHADOW_KNEE: f32 = 0.15;
pub const LAND_SHADOW_EXP: f32 = 0.5;

/// Width of the sea-ice blend band on |sin(latitude)|: the albedo path's
/// cap layer fades in from `polar_cap_latitude` to `polar_cap_latitude +
/// this` instead of switching hard, so the ice edge reads as pack ice
/// thinning into open water rather than a jagged color cliff (the classify
/// path keeps its hard threshold -- its whole look is banded).
pub const SEA_ICE_BLEND_BAND: f32 = 0.03;

/// Face color for a surface point: REAL imagery when the def ships an
/// albedo grid, elevation-band classifier otherwise.
///
/// Albedo path layering decision (2026-07-11, verified by eye against the
/// shipped Blue Marble grid): the August imagery already contains all LAND
/// snow (Greenland, Antarctica, the Himalaya) -- re-layering the classify
/// cap over polar land would white out coastal Greenland tundra the photo
/// renders correctly. What the imagery does NOT contain is SEA ICE: its
/// oceans stay liquid dark blue right up to both poles, which reads wrong
/// from orbit. So the cap color is layered over BELOW-SEA faces only,
/// fading in across `SEA_ICE_BLEND_BAND` above the def's
/// `polar_cap_latitude`. Ocean depth shading likewise comes from the
/// imagery (the "topo.bathy" Blue Marble bakes bathymetry), not from the
/// classifier's depth-darkening.
pub fn surface_color(
    def: &PlanetDef,
    albedo: Option<&PlanetAlbedo>,
    unit_dir: Vec3,
    elevation: f32,
) -> [f32; 3] {
    // On a unit sphere, y IS sin(latitude).
    let abs_sin_lat = unit_dir.y.abs();
    let Some(al) = albedo else {
        return classify_color(def, elevation, abs_sin_lat);
    };
    grade_albedo(def, al.sample_linear(unit_dir), elevation, abs_sin_lat)
}

/// Orbital-look grading of one LINEAR imagery sample: ocean floor / land
/// gain / sea-ice cap blend (see the three constants + the layering decision
/// above). THE single source of truth shared by the per-face color path
/// (`surface_color`, both mesh builders) and the per-pixel texture bake
/// (`bake_albedo_rgba`), so the two can never drift apart -- the LOD handoff
/// between the packed-color fallback and the sampled texture depends on
/// them agreeing.
/// Total gain applied to an above-sea land texel (v0.908), hue-preserving:
/// the calibrated orbital LAND_ALBEDO_GAIN x a dark-land shadow lift x a
/// vegetation nudge. The shadow lift is the Europe-noon-darkness fix: Blue
/// Marble's vegetated/forested land is radiometrically FAR darker than it
/// reads to the eye (measured linear luma at 47N France 0.021 vs Sahara
/// sand 0.40 -- a 20x gap), so mid-latitude noon rendered night-dark while
/// deserts glowed (probe A/B 2026-07-20). Pixels below the LAND_SHADOW_KNEE
/// luma ride a power curve (lifted = knee * (luma/knee)^exp), continuous at
/// the knee; bright terrain passes through untouched. Public so the bake
/// tests compute the same expectation.
pub fn land_gain(raw: [f32; 3]) -> f32 {
    let luma = 0.299 * raw[0] + 0.587 * raw[1] + 0.114 * raw[2];
    let shadow_lift = if luma > 0.0005 && luma < LAND_SHADOW_KNEE {
        LAND_SHADOW_KNEE * (luma / LAND_SHADOW_KNEE).powf(LAND_SHADOW_EXP) / luma
    } else {
        1.0
    };
    // Green-dominant pixels get a further nudge (living cover reflects
    // more than shadowed rock): plain linear ratio, up to +50%.
    let greenness = ((raw[1] - raw[0].max(raw[2])) / raw[1].max(0.001)).clamp(0.0, 1.0);
    let t = ((greenness - 0.1) / 0.5).clamp(0.0, 1.0);
    let veg_lift = 1.0 + 0.5 * (t * t * (3.0 - 2.0 * t));
    LAND_ALBEDO_GAIN * shadow_lift * veg_lift
}

pub fn grade_albedo(
    def: &PlanetDef,
    raw: [f32; 3],
    elevation: f32,
    abs_sin_lat: f32,
) -> [f32; 3] {
    let sea = def.sea_level.clamp(0.0, 1.0);
    // v0.905 (multi-planet imagery): the ocean floor / sea-ice grading only
    // applies to WATER worlds. On a dry Moon/Mars/Pluto, "below sea level"
    // is ordinary low ground - the old unconditional path tinted lunar
    // maria photo-blue and ice-capped Mars' poles twice (survey gotcha G1).
    // Dry worlds pass imagery through with a NEUTRAL gain (the 1.6x land
    // gain was calibrated to Earth's dark Blue Marble bake; NASA/USGS moon
    // and mars mosaics are already exposure-balanced).
    if !def.has_water {
        return raw;
    }
    // Orbital-look grading (2026-07-11 field report: "black planet surface
    // under the clouds"): floor water to photo blue, gain land toward the
    // brightness the pipeline is calibrated for. See the two constants above.
    let base = if elevation < sea {
        [
            raw[0].max(OCEAN_ORBITAL_FLOOR[0]),
            raw[1].max(OCEAN_ORBITAL_FLOOR[1]),
            raw[2].max(OCEAN_ORBITAL_FLOOR[2]),
        ]
    } else {
        let k = land_gain(raw);
        [
            (raw[0] * k).min(1.0),
            (raw[1] * k).min(1.0),
            (raw[2] * k).min(1.0),
        ]
    };
    // Sea-ice layer: polar ocean only (see the layering decision above).
    // polar_cap_latitude > 1.0 disables caps entirely, same as classify.
    if def.polar_cap_latitude <= 1.0 && elevation < sea {
        let t = ((abs_sin_lat - def.polar_cap_latitude) / SEA_ICE_BLEND_BAND).clamp(0.0, 1.0);
        if t > 0.0 {
            let cap = rgb(def.cap_color);
            return [
                base[0] + (cap[0] - base[0]) * t,
                base[1] + (cap[1] - base[1]) * t,
                base[2] + (cap[2] - base[2]) * t,
            ];
        }
    }
    base
}

/// Bake a planet's finished per-pixel surface texture (v0.811): the raw
/// albedo imagery with the orbital-look grading applied per TEXEL, returned
/// as sRGB RGBA8 bytes (row-major, row 0 north -- the same orientation the
/// file stores) ready to upload as an `Rgba8UnormSrgb` GPU texture. The
/// fragment shader (pbr_simple.wgsl material type 12, params.w flag) then
/// samples a finished color per pixel, replacing the one-color-per-triangle
/// mosaic the packed-UV transport produced.
///
/// Water vs land per texel comes from the ELEVATION grid (below the def's
/// sea level = water), which is why this bake requires both grids: without
/// elevation there is no per-texel water mask for the floor/gain/ice split.
/// Colors are decoded to linear, graded via `grade_albedo` (the same
/// function the per-face fallback path uses), and re-encoded -- the GPU
/// decodes back to linear on sample, so the shader sees exactly what
/// `surface_color` would compute, just at texel resolution instead of face
/// resolution. Slope shading stays a per-face concern (it needs the mesh
/// normal); the imagery already carries its own relief shading.
pub fn bake_albedo_rgba(
    def: &PlanetDef,
    hm: &super::planet_heightmap::PlanetHeightmap,
    al: &PlanetAlbedo,
) -> Vec<u8> {
    use super::planet_albedo::linear_to_srgb_byte;
    let (w, h) = (al.width(), al.height());
    let range_m = hm.max_meters() - hm.min_meters();
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        // Texel-center geography, identical to the grids' cell-centered
        // registration (planet_heightmap module doc): row 0 is the
        // northernmost row, column 0 the westernmost column.
        let lat = 90.0 - (y as f32 + 0.5) * 180.0 / h as f32;
        let abs_sin_lat = lat.to_radians().sin().abs();
        for x in 0..w {
            let lon = -180.0 + (x as f32 + 0.5) * 360.0 / w as f32;
            let raw = al.texel_linear(x, y);
            // Normalized elevation in the same 0..1 domain the def's
            // sea_level lives in (the loader overrides sea_level with the
            // grid's true 0 m position, so this coastline is exact).
            let elevation = ((hm.sample_meters_latlon(lat, lon) - hm.min_meters()) / range_m)
                .clamp(0.0, 1.0);
            let graded = grade_albedo(def, raw, elevation, abs_sin_lat);
            out.push(linear_to_srgb_byte(graded[0]));
            out.push(linear_to_srgb_byte(graded[1]));
            out.push(linear_to_srgb_byte(graded[2]));
            out.push(255);
        }
    }
    out
}

/// Floor of the slope-shading multiplier: a perfectly vertical cliff face
/// keeps this fraction of its color; flat ground keeps 100%. Kept gentle --
/// this is relief legibility (a cheap baked ambient-occlusion stand-in for
/// noon lighting, when sun-facing and steep faces would otherwise read
/// identically), not dramatic terrain shadowing.
pub const SLOPE_SHADE_FLOOR: f32 = 0.65;

/// Slope-shading multiplier for a land face: 1.0 where the face normal is
/// radial (flat ground), falling linearly to `SLOPE_SHADE_FLOOR` as the
/// face tips vertical. Applied at BUILD time to the face color (both mesh
/// paths), so it costs nothing per frame. Water faces have radial normals,
/// so this is an identity for them by construction.
pub fn slope_shade(normal: Vec3, radial: Vec3) -> f32 {
    let d = normal.dot(radial).clamp(0.0, 1.0);
    SLOPE_SHADE_FLOOR + (1.0 - SLOPE_SHADE_FLOOR) * d
}

/// Pack an RGB color + the water flag into the 2-float UV channel so the
/// standard PBR vertex layout can carry per-face colors without a new
/// pipeline.
///
/// `uv.x = water*65536 + round(r*255)*256 + round(g*255)` -- an exact
/// integer (<= 131071, well inside f32's 2^24 exact-integer range).
/// `uv.y = b` as a plain float. Because every corner of a flat-shaded face
/// stores the SAME value, linear interpolation across the triangle is a
/// constant and the packed integer survives to the fragment shader
/// (material type 12 decodes it; the water bit gates the ocean sun glint).
pub fn pack_color_to_uv(c: [f32; 3], water: bool) -> [f32; 2] {
    let r = (c[0].clamp(0.0, 1.0) * 255.0).round();
    let g = (c[1].clamp(0.0, 1.0) * 255.0).round();
    let w = if water { 65536.0 } else { 0.0 };
    [w + r * 256.0 + g, c[2].clamp(0.0, 1.0)]
}

/// Rust mirror of the WGSL decode in pbr_simple.wgsl (material type 12).
/// Exists so a unit test locks the round-trip; keep both in sync.
pub fn unpack_uv_to_color(uv: [f32; 2]) -> ([f32; 3], bool) {
    let packed = uv[0].round().max(0.0) as u32;
    (
        [
            ((packed >> 8) & 255) as f32 / 255.0,
            (packed & 255) as f32 / 255.0,
            uv[1],
        ],
        (packed & 0x1_0000) != 0,
    )
}

/// Build the flat-shaded procedural surface mesh for a planet at the given
/// icosphere subdivision level, at unit radius (scale at draw time).
///
/// `heightmap`: pass the planet's loaded real-elevation grid to sample
/// vertex elevations from data instead of noise (the caller pairs it with a
/// def whose `sea_level` was overridden to the grid's true 0 m position --
/// see `lib.rs::reload_planet_defs`). None keeps the seeded-noise path.
///
/// `albedo`: pass the planet's loaded real-color grid to sample face colors
/// from imagery instead of the elevation-band classifier (see
/// `surface_color`). None keeps the classifier path.
pub fn build_surface_mesh(
    def: &PlanetDef,
    heightmap: Option<&PlanetHeightmap>,
    albedo: Option<&PlanetAlbedo>,
    level: u32,
) -> SurfaceMeshData {
    let mut ico = Icosphere::new();
    ico.subdivide_n(level);

    let sea = def.sea_level.clamp(0.0, 1.0);

    // Per-icosphere-vertex elevation + displaced position, computed once and
    // shared by every face touching the vertex (positions must agree across
    // faces or the surface cracks). Both sources land in the same 0..1
    // domain, so everything downstream (displacement, sea level, colors) is
    // source-agnostic.
    let elev: Vec<f32> = match heightmap {
        Some(hm) => ico.vertices.iter().map(|v| hm.normalized_at(*v)).collect(),
        None => {
            let sampler = SurfaceSampler::new(def);
            ico.vertices.iter().map(|v| sampler.elevation_at(*v)).collect()
        }
    };
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
        // Real imagery when the def ships an albedo grid (Earth), the
        // elevation-band classifier otherwise (every other planet).
        let color = surface_color(def, albedo, centroid_dir, mean_e);

        let underwater = def.has_water && mean_e < sea;
        if underwater {
            // Smooth ocean: per-corner spherical normals, undisplaced radius.
            // water: true is what turns on the shader's sun glint.
            for &i in &[i0, i1, i2] {
                let n = ico.vertices[i].normalize_or_zero();
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: (pos[i]).to_array(),
                    normal: n.to_array(),
                    color,
                    water: true,
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
            // Slope shading: steeper faces darken slightly so relief stays
            // readable even when the sun is overhead (see slope_shade).
            let shade = slope_shade(n, centroid_dir);
            let color = [color[0] * shade, color[1] * shade, color[2] * shade];
            for &p in &[p0, p1, p2] {
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: p.to_array(),
                    normal: n.to_array(),
                    color,
                    water: false,
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
        let data = build_surface_mesh(&def, None, None, 2);
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
        let m1 = build_surface_mesh(&def, None, None, 1);
        let m2 = build_surface_mesh(&def, None, None, 1);
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
        let data = build_surface_mesh(&def, None, None, 3);
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
        let data = build_surface_mesh(&def, None, None, 3);
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

    /// Build an in-memory PlanetHeightmap through its public byte format
    /// (same layout scripts/build-earth-heightmap.js writes).
    fn synth_heightmap(
        width: u32,
        height: u32,
        min_m: f32,
        max_m: f32,
        meters: &[f32],
    ) -> PlanetHeightmap {
        use crate::terrain::planet_heightmap::{quantize_meters, HEIGHTMAP_MAGIC};
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HEIGHTMAP_MAGIC);
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&min_m.to_le_bytes());
        bytes.extend_from_slice(&max_m.to_le_bytes());
        for &m in meters {
            bytes.extend_from_slice(&quantize_meters(m, min_m, max_m).to_le_bytes());
        }
        PlanetHeightmap::from_bytes(&bytes).expect("synthetic heightmap parses")
    }

    #[test]
    fn heightmap_drives_vertex_elevation_instead_of_noise() {
        let mut def = water_world(42);
        // Match the loader behavior: with a heightmap, sea_level is the
        // grid's 0 m position -- for a -1000..+1000 m window that is 0.5.
        def.sea_level = 0.5;

        // A uniform all-land grid at +1000 m (= the max, normalized 1.0):
        // EVERY vertex must sit at the same displaced radius, something the
        // fractal noise could never produce.
        let flat = synth_heightmap(4, 2, -1000.0, 1000.0, &[1000.0; 8]);
        let mesh = build_surface_mesh(&def, Some(&flat), None, 2);
        let expected = displaced_radius(&def, 1.0);
        assert!(expected > 1.0, "all-land grid must displace outward");
        for v in &mesh.vertices {
            let r = Vec3::from_array(v.position).length();
            assert!(
                (r - expected).abs() < 2e-4,
                "vertex radius {r} != uniform grid radius {expected}"
            );
        }

        // And the data path must actually differ from the noise path.
        let noise_mesh = build_surface_mesh(&def, None, None, 2);
        assert_ne!(mesh.vertices, noise_mesh.vertices);

        // Determinism holds for the heightmap path too (mesh cache +
        // multiplayer re-derivation both rely on it).
        let again = build_surface_mesh(&def, Some(&flat), None, 2);
        assert_eq!(mesh.vertices, again.vertices);
        assert_eq!(mesh.indices, again.indices);
    }

    #[test]
    fn heightmap_all_ocean_grid_stays_smooth_sphere() {
        let mut def = water_world(42);
        def.sea_level = 0.5;
        // Everything 500 m BELOW sea level: a pure water world -- every
        // vertex on the unit sphere, every face ocean-colored (no land
        // bands can appear from interpolation).
        let ocean = synth_heightmap(4, 2, -1000.0, 1000.0, &[-500.0; 8]);
        let mesh = build_surface_mesh(&def, Some(&ocean), None, 2);
        for v in &mesh.vertices {
            let r = Vec3::from_array(v.position).length();
            assert!((r - 1.0).abs() < 1e-4, "ocean vertex off the sphere: {r}");
        }
    }

    #[test]
    fn displaced_radius_f64_matches_f32() {
        // The chunked-LOD patch path composes positions through the f64
        // twin; both formulas must stay in lockstep or the uniform-sphere
        // and patch surfaces would disagree at the LOD transition.
        for def in [water_world(42), dry_world(7)] {
            for e in [0.0_f32, 0.1, 0.45, 0.55, 0.72, 0.9, 1.0] {
                let a = displaced_radius(&def, e) as f64;
                let b = displaced_radius_f64(&def, e as f64);
                assert!(
                    (a - b).abs() < 1e-6,
                    "f32 {a} vs f64 {b} diverged at elevation {e}"
                );
            }
        }
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
        for water in [false, true] {
            for c in samples {
                let uv = pack_color_to_uv(c, water);
                let (back, back_water) = unpack_uv_to_color(uv);
                for i in 0..3 {
                    assert!(
                        (back[i] - c[i]).abs() <= 1.0 / 255.0 + 1e-6,
                        "channel {i} of {c:?} round-tripped to {back:?}"
                    );
                }
                // The water flag must survive the packing exactly.
                assert_eq!(back_water, water, "water flag lost for {c:?}");
                // The packed x component must be an exact integer
                // (interpolation safety depends on it) and stay well inside
                // f32's 2^24 exact-integer range.
                assert_eq!(uv[0].fract(), 0.0);
                assert!(uv[0] <= 131071.0);
            }
        }
    }

    /// The mesh builder's water flag must match the geometry rule exactly:
    /// water = has_water AND face below sea level. This is the contract the
    /// shader's ocean sun glint relies on.
    #[test]
    fn water_flag_matches_below_sea_rule() {
        let mut def = water_world(42);
        def.sea_level = 0.5;
        // All-ocean grid: every vertex flagged water.
        let ocean = synth_heightmap(4, 2, -1000.0, 1000.0, &[-500.0; 8]);
        let mesh = build_surface_mesh(&def, Some(&ocean), None, 2);
        assert!(mesh.vertices.iter().all(|v| v.water), "ocean world must flag all water");
        // All-land grid: none flagged.
        let land = synth_heightmap(4, 2, -1000.0, 1000.0, &[900.0; 8]);
        let mesh = build_surface_mesh(&def, Some(&land), None, 2);
        assert!(mesh.vertices.iter().all(|v| !v.water), "land world must flag no water");
        // Dry world: below-sea basins are NOT water (no glint on the Moon).
        let dry = dry_world(7);
        let mesh = build_surface_mesh(&dry, None, None, 2);
        assert!(mesh.vertices.iter().all(|v| !v.water), "waterless world must flag no water");
    }

    #[test]
    fn slope_shade_flat_full_steep_darker() {
        let radial = Vec3::Y;
        // Flat ground (normal == radial): identity.
        assert!((slope_shade(radial, radial) - 1.0).abs() < 1e-6);
        // Vertical cliff (normal perpendicular to radial): the floor.
        assert!((slope_shade(Vec3::X, radial) - SLOPE_SHADE_FLOOR).abs() < 1e-6);
        // Monotonic in between, and never below the floor / above 1.
        let mid = slope_shade(Vec3::new(0.707, 0.707, 0.0), radial);
        assert!(mid > SLOPE_SHADE_FLOOR && mid < 1.0);
        // Degenerate inputs (inward normal) clamp instead of exploding.
        let inward = slope_shade(-radial, radial);
        assert!((inward - SLOPE_SHADE_FLOOR).abs() < 1e-6);
    }

    /// Mesh-level color contract: every LAND face's stored color must be
    /// exactly surface_color(...) * slope_shade(...), recomputed here from
    /// the same icosphere the builder walks. Locks the whole color pipeline
    /// (classifier/albedo choice + slope shading) in one place, and proves
    /// slope shading actually engages (some face must be visibly shaded).
    #[test]
    fn land_face_colors_are_surface_color_times_slope_shade() {
        let mut def = water_world(42);
        def.sea_level = 0.2;
        def.surface_relief = 0.3; // steep enough that shading is exercised
        // A tall thin ridge on an otherwise flat land world.
        let mut meters = vec![0.0f32; 64 * 32];
        for (i, m) in meters.iter_mut().enumerate() {
            if i % 64 == 32 {
                *m = 1000.0;
            }
        }
        let hm = synth_heightmap(64, 32, -1000.0, 1000.0, &meters);
        let level = 4;
        let mesh = build_surface_mesh(&def, Some(&hm), None, level);

        // Mirror the builder's per-face derivation.
        let mut ico = Icosphere::new();
        ico.subdivide_n(level);
        let elev: Vec<f32> = ico.vertices.iter().map(|v| hm.normalized_at(*v)).collect();
        let mut any_shaded = false;
        for (fi, face) in ico.faces.iter().enumerate() {
            let (i0, i1, i2) = (face.v0 as usize, face.v1 as usize, face.v2 as usize);
            let mean_e = (elev[i0] + elev[i1] + elev[i2]) / 3.0;
            if def.has_water && mean_e < def.sea_level {
                continue; // ocean faces: unshaded, covered by other tests
            }
            let centroid_dir =
                (ico.vertices[i0] + ico.vertices[i1] + ico.vertices[i2]).normalize_or_zero();
            let stored = &mesh.vertices[fi * 3];
            let n = Vec3::from_array(stored.normal);
            let shade = slope_shade(n, centroid_dir);
            let base = surface_color(&def, None, centroid_dir, mean_e);
            for ch in 0..3 {
                assert!(
                    (stored.color[ch] - base[ch] * shade).abs() < 1e-6,
                    "face {fi} channel {ch}: stored {} != base {} * shade {shade}",
                    stored.color[ch],
                    base[ch]
                );
            }
            if shade < 0.99 {
                any_shaded = true;
            }
        }
        assert!(any_shaded, "no face was meaningfully slope-shaded; ridge too gentle?");
    }

    /// Build an in-memory PlanetAlbedo through its public byte format
    /// (same layout scripts/build-earth-albedo.js writes): a uniform-color
    /// grid so face colors are exactly predictable.
    fn synth_albedo_uniform(rgb_bytes: [u8; 3]) -> crate::terrain::planet_albedo::PlanetAlbedo {
        use crate::terrain::planet_albedo::{PlanetAlbedo, ALBEDO_MAGIC};
        let (w, h) = (4u32, 2u32);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(ALBEDO_MAGIC);
        bytes.extend_from_slice(&w.to_le_bytes());
        bytes.extend_from_slice(&h.to_le_bytes());
        for _ in 0..(w * h) {
            bytes.extend_from_slice(&rgb_bytes);
        }
        PlanetAlbedo::from_bytes(&bytes).expect("synthetic albedo parses")
    }

    #[test]
    fn surface_color_uses_albedo_when_present() {
        use crate::terrain::planet_albedo::srgb_byte_to_linear;
        let def = water_world(42);
        let al = synth_albedo_uniform([200, 100, 50]);
        // Equatorial land face: imagery hue preserved, scaled by the flat
        // orbital-look gain (clamped at 1).
        let expect = [
            (srgb_byte_to_linear(200) * LAND_ALBEDO_GAIN).min(1.0),
            (srgb_byte_to_linear(100) * LAND_ALBEDO_GAIN).min(1.0),
            (srgb_byte_to_linear(50) * LAND_ALBEDO_GAIN).min(1.0),
        ];
        let c = surface_color(&def, Some(&al), Vec3::new(1.0, 0.0, 0.0), def.sea_level + 0.1);
        for i in 0..3 {
            assert!((c[i] - expect[i]).abs() < 1e-5, "albedo not passed through: {c:?}");
        }
        // Equatorial OCEAN face: imagery floored to the orbital blue
        // (component-wise max -- this synthetic orange is BRIGHTER than the
        // floor in r/g, so those channels pass through; b lifts to floor).
        let expect_ocean = [
            srgb_byte_to_linear(200).max(OCEAN_ORBITAL_FLOOR[0]),
            srgb_byte_to_linear(100).max(OCEAN_ORBITAL_FLOOR[1]),
            srgb_byte_to_linear(50).max(OCEAN_ORBITAL_FLOOR[2]),
        ];
        let c = surface_color(&def, Some(&al), Vec3::new(1.0, 0.0, 0.0), 0.1);
        for i in 0..3 {
            assert!(
                (c[i] - expect_ocean[i]).abs() < 1e-5,
                "ocean albedo modified beyond the floor: {c:?}"
            );
        }
        // None falls back to the classifier exactly.
        let c = surface_color(&def, None, Vec3::new(1.0, 0.0, 0.0), def.sea_level + 0.1);
        assert_eq!(c, classify_color(&def, def.sea_level + 0.1, 0.0));
    }

    /// The per-pixel texture bake must agree TEXEL-FOR-TEXEL with the
    /// shared grading helper: decode the stored byte, grade with the texel's
    /// own elevation + latitude, re-encode. Uses a half-ocean/half-land
    /// heightmap so the water/land split, the land gain, the ocean floor
    /// AND the polar sea-ice blend are all exercised in one grid.
    #[test]
    fn bake_albedo_matches_grade_albedo_per_texel() {
        use crate::terrain::planet_albedo::{linear_to_srgb_byte, srgb_byte_to_linear};
        let mut def = water_world(42);
        def.sea_level = 0.5; // grid 0 m position for a -1000..1000 window
        def.polar_cap_latitude = 0.90; // polar rows of an 8-row grid ice over
        // 4x8 heightmap: west half deep ocean, east half high land.
        let (w, h) = (4u32, 8u32);
        let mut meters = Vec::new();
        for _row in 0..h {
            for col in 0..w {
                meters.push(if col < 2 { -500.0f32 } else { 800.0 });
            }
        }
        let hm = synth_heightmap(w, h, -1000.0, 1000.0, &meters);
        // 4x8 albedo with a per-texel color ramp (distinct every texel).
        use crate::terrain::planet_albedo::{PlanetAlbedo, ALBEDO_MAGIC};
        let mut bytes = Vec::new();
        bytes.extend_from_slice(ALBEDO_MAGIC);
        bytes.extend_from_slice(&w.to_le_bytes());
        bytes.extend_from_slice(&h.to_le_bytes());
        for y in 0..h {
            for x in 0..w {
                bytes.extend_from_slice(&[(x * 60) as u8, (y * 30) as u8, 90]);
            }
        }
        let al = PlanetAlbedo::from_bytes(&bytes).expect("synthetic albedo parses");

        let baked = bake_albedo_rgba(&def, &hm, &al);
        assert_eq!(baked.len(), (w * h * 4) as usize);
        let range = 2000.0f32;
        let mut saw_ocean_floor = false;
        let mut saw_land_gain = false;
        let mut saw_sea_ice = false;
        for y in 0..h {
            let lat = 90.0 - (y as f32 + 0.5) * 180.0 / h as f32;
            let abs_sin_lat = lat.to_radians().sin().abs();
            for x in 0..w {
                let lon = -180.0 + (x as f32 + 0.5) * 360.0 / w as f32;
                let raw = [
                    srgb_byte_to_linear((x * 60) as u8),
                    srgb_byte_to_linear((y * 30) as u8),
                    srgb_byte_to_linear(90),
                ];
                let elevation =
                    ((hm.sample_meters_latlon(lat, lon) - hm.min_meters()) / range).clamp(0.0, 1.0);
                let expect = grade_albedo(&def, raw, elevation, abs_sin_lat);
                let o = ((y * w + x) * 4) as usize;
                for ch in 0..3 {
                    assert_eq!(
                        baked[o + ch],
                        linear_to_srgb_byte(expect[ch]),
                        "texel ({x},{y}) channel {ch} disagrees with grade_albedo"
                    );
                }
                assert_eq!(baked[o + 3], 255, "alpha must be opaque");
                if elevation < def.sea_level && abs_sin_lat > def.polar_cap_latitude {
                    saw_sea_ice = true;
                } else if elevation < def.sea_level {
                    saw_ocean_floor = true;
                } else {
                    saw_land_gain = true;
                }
            }
        }
        // The grid must actually have exercised all three grading regimes,
        // or this test silently degrades to checking one code path.
        assert!(saw_ocean_floor && saw_land_gain && saw_sea_ice);
    }

    /// Interior texels of a bilinear-sampled cell-centered grid: the bake's
    /// per-texel elevation lookup at the texel's own center must return the
    /// stored cell value exactly (tap lands on a cell center), so the
    /// water/land mask in the texture matches the file data 1:1 when the
    /// two grids share dimensions.
    #[test]
    fn bake_water_mask_tracks_heightmap_cells_exactly() {
        let mut def = water_world(7);
        def.sea_level = 0.5;
        def.polar_cap_latitude = 2.0; // ice off: isolate the water mask
        let (w, h) = (8u32, 4u32);
        // Checkerboard ocean/land.
        let mut meters = Vec::new();
        for row in 0..h {
            for col in 0..w {
                meters.push(if (row + col) % 2 == 0 { -700.0f32 } else { 700.0 });
            }
        }
        let hm = synth_heightmap(w, h, -1000.0, 1000.0, &meters);
        let al = {
            use crate::terrain::planet_albedo::{PlanetAlbedo, ALBEDO_MAGIC};
            let mut bytes = Vec::new();
            bytes.extend_from_slice(ALBEDO_MAGIC);
            bytes.extend_from_slice(&w.to_le_bytes());
            bytes.extend_from_slice(&h.to_le_bytes());
            for _ in 0..(w * h) {
                // Mid-gray: darker than the land gain can clamp, brighter
                // than the ocean floor's b channel, so water vs land texels
                // get visibly different encodings.
                bytes.extend_from_slice(&[100, 100, 100]);
            }
            PlanetAlbedo::from_bytes(&bytes).expect("parses")
        };
        let baked = bake_albedo_rgba(&def, &hm, &al);
        use crate::terrain::planet_albedo::{linear_to_srgb_byte, srgb_byte_to_linear};
        let gray = srgb_byte_to_linear(100);
        let land_r = linear_to_srgb_byte((gray * land_gain([gray, gray, gray])).min(1.0));
        let water_r = linear_to_srgb_byte(gray.max(OCEAN_ORBITAL_FLOOR[0]));
        for row in 0..h {
            for col in 0..w {
                let o = ((row * w + col) * 4) as usize;
                let expect = if (row + col) % 2 == 0 { water_r } else { land_r };
                assert_eq!(
                    baked[o], expect,
                    "texel ({col},{row}) water/land classification drifted from the grid"
                );
            }
        }
    }

    #[test]
    fn surface_color_layers_sea_ice_over_polar_ocean_only() {
        let mut def = water_world(42);
        def.polar_cap_latitude = 0.88;
        let al = synth_albedo_uniform([10, 30, 80]); // dark ocean blue
        let cap = rgb(def.cap_color);
        // Deep polar ocean, well inside the cap band: full cap color.
        let polar_dir = Vec3::new(0.0, 0.99, 0.14).normalize();
        let c = surface_color(&def, Some(&al), polar_dir, 0.1);
        for i in 0..3 {
            assert!((c[i] - cap[i]).abs() < 1e-5, "polar ocean not iced: {c:?}");
        }
        // Polar LAND keeps the imagery (Blue Marble already paints land
        // snow; re-capping would white out what the photo gets right).
        // The land gain applies, but no cap color.
        let raw_land = [
            crate::terrain::planet_albedo::srgb_byte_to_linear(10),
            crate::terrain::planet_albedo::srgb_byte_to_linear(30),
            crate::terrain::planet_albedo::srgb_byte_to_linear(80),
        ];
        let expect_land_b = (raw_land[2] * land_gain(raw_land)).min(1.0);
        let c_land = surface_color(&def, Some(&al), polar_dir, def.sea_level + 0.1);
        assert!(
            (c_land[2] - expect_land_b).abs() < 1e-5,
            "polar land was capped over imagery: {c_land:?}"
        );
        // Mid-latitude ocean: no ice, just the orbital floor over the imagery.
        let mid_dir = Vec3::new(1.0, 0.5, 0.0).normalize();
        let c_mid = surface_color(&def, Some(&al), mid_dir, 0.1);
        let expect_mid_b =
            crate::terrain::planet_albedo::srgb_byte_to_linear(80).max(OCEAN_ORBITAL_FLOOR[2]);
        assert!(
            (c_mid[2] - expect_mid_b).abs() < 1e-5,
            "mid-latitude ocean was iced: {c_mid:?}"
        );
        // Inside the blend band: strictly between imagery and cap.
        let sin_edge = def.polar_cap_latitude + SEA_ICE_BLEND_BAND * 0.5;
        let edge_dir = Vec3::new((1.0 - sin_edge * sin_edge).sqrt(), sin_edge, 0.0);
        let c_edge = surface_color(&def, Some(&al), edge_dir, 0.1);
        assert!(c_edge[0] > c_mid[0] && c_edge[0] < cap[0], "no smooth ice edge: {c_edge:?}");
        // polar_cap_latitude > 1.0 disables the layer entirely.
        def.polar_cap_latitude = 2.0;
        let c_off = surface_color(&def, Some(&al), polar_dir, 0.1);
        assert!((c_off[0] - c_mid[0]).abs() < 1e-5, "disabled caps still iced: {c_off:?}");
    }
}

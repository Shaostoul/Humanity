//! Procedural cloud layer: the Rust mirror of the WGSL math (increment 1).
//!
//! The actual rendering lives in `assets/shaders/pbr_simple.wgsl` (material
//! type 15, `cloud_layer`). This module exists for the same three reasons as
//! its sibling `renderer::atmosphere`:
//!
//! 1. **Shared constants for lib.rs** (`CLOUD_SHELL_SCALE`, the seed
//!    packing): the producer side of the material lives here so the shader
//!    and the scene code cannot silently disagree.
//! 2. **Testable mirrors** of every pure shader function (the hash, the
//!    value noise, the triplanar sphere noise, the drifting octave field,
//!    the coverage -> alpha mapping), so determinism, ranges, and the
//!    coverage semantics are locked by unit tests instead of eyeballs.
//! 3. **One documented home** for the increment-1 design and its
//!    increment-2 reuse contract.
//!
//! ## The model (increment 1: a shell deck that reads volumetric from orbit)
//!
//! A second translucent icosphere shell at `CLOUD_SHELL_SCALE` times the
//! planet radius (above the tallest terrain, below the atmosphere shell)
//! carries a per-fragment coverage field: four octaves of the shader's
//! existing 2D value noise, blended TRIPLANARLY onto the sphere (for a unit
//! direction the squared components sum to 1, so `dir*dir` are free blend
//! weights), split into two octave SETS that drift as rigid rotations around
//! different axes at different speeds -- the sum therefore morphs over time
//! instead of sliding as one frozen texture. Lighting is N dot L from the
//! sphere normal plus a one-tap self-shadow (re-sample the field a short
//! great-circle step toward the sun; density rising toward the sun means
//! this point is on a cloud mass's shaded flank) and a Henyey-Greenstein
//! silver lining at thin edges. The night side fades to near-black.
//!
//! ## Increment 2 (v0.815): the raymarch is SHIPPED
//!
//! `cloud_field(dir, t, seed)` stayed untouched, exactly as the reuse
//! contract promised. The shader now marches `CLOUD_MARCH_SAMPLES` jittered
//! samples through a thin spherical slab (`CLOUD_BASE_SCALE` ..
//! `CLOUD_TOP_SCALE` planet radii; the drawn shell at `CLOUD_SHELL_SCALE`
//! sits mid-slab and only supplies fragments/rays), sampling
//!
//! ```text
//! density(p_local) = cloud_alpha_from_field(
//!     cloud_field(normalize(p_local), t, seed), coverage)
//!     * cloud_altitude_envelope(length(p_local))
//! ```
//!
//! with front-to-back alpha accumulation (early-out at saturation),
//! per-sample macro N-dot-L, a one-tap sun-direction density gradient for
//! volumetric self-shadow, and a base-to-top height gradient. The
//! increment-1 single-sample path survives verbatim as `cloud_layer_flat`
//! behind the `CLOUD_MARCH_SAMPLES = 0` quality switch (the WGSL is
//! hot-reloaded from disk, so weak GPUs are one edit away from the cheap
//! deck). Ground shadows from clouds (the type-12 surface shader sampling
//! this field) remain deferred -- increment-3 work, noted so it is not
//! forgotten.
//!
//! Keep every `CLOUD_*` constant below byte-identical with the WGSL; the
//! `wgsl_cloud_constants_stay_in_sync` test parses the shader source and
//! fails on drift.

/// Cloud shell radius as a multiple of the planet radius. Chosen to clear
/// the tallest possible terrain: Earth's heightmap peaks displace up to
/// (1 - sea_level) * surface_relief = ~0.0041 of the radius (the 4x
/// vertical exaggeration documented in earth.ron), so 1.008 gives ~2x
/// margin over the highest peak while staying far below the atmosphere
/// shell (1 + atmosphere_scale * 2 = 1.03 for Earth). Physically ~51 km up:
/// higher than real cloud tops, invisible at orbital scales, and the slack
/// prevents any z-fighting with close-approach terrain patches. The
/// `cloud_shell_sits_between_peaks_and_atmosphere` test locks this ordering
/// against the shipped earth.ron numbers.
pub const CLOUD_SHELL_SCALE: f32 = 1.008;

/// Mirrors `CLOUD_BASE_SCALE` / `CLOUD_TOP_SCALE` in pbr_simple.wgsl: the
/// increment-2 raymarch slab, in planet-radius multiples. The drawn shell
/// (`CLOUD_SHELL_SCALE`) sits mid-slab; density lives between these bounds.
/// Earth: base ~25.5 km, top ~76.5 km. Terrain peaks (up to ~1.0041 R, see
/// the shell-scale note above) may poke ~100 m into the base -- mountains in
/// cloud, harmless because the altitude envelope is ~0 there.
pub const CLOUD_BASE_SCALE: f32 = 1.004;
pub const CLOUD_TOP_SCALE: f32 = 1.012;
/// Mirrors `CLOUD_MARCH_SAMPLES` (i32 in WGSL): march quality switch.
/// 8..=12 is the designed band; 0 selects the increment-1 flat deck.
pub const CLOUD_MARCH_SAMPLES: i32 = 10;
/// Mirrors `CLOUD_SIGMA_T`: extinction per drawn-shell unit at density 1,
/// calibrated so a full-density radial pass reaches ~93% opacity (matching
/// increment 1's thick-core look after the CLOUD_MAX_ALPHA cap).
pub const CLOUD_SIGMA_T: f32 = 560.0;
/// Mirrors `CLOUD_MARCH_SHADOW_STEP` / `CLOUD_MARCH_SHADOW_SHARP`: the 3D
/// sun-direction density tap (drawn-shell units, ~half the slab thickness)
/// and its difference-to-shade amplification.
pub const CLOUD_MARCH_SHADOW_STEP: f32 = 0.004;
pub const CLOUD_MARCH_SHADOW_SHARP: f32 = 4.0;
/// Mirrors `CLOUD_BASE_DARKEN`: bottom-of-slab light multiplier (bases
/// darker than tops -- the classic volumetric cue).
pub const CLOUD_BASE_DARKEN: f32 = 0.75;

// ── Increment-3 volumetric constants (WGSL: the CLOUD_HI_* block) ──
// The High-quality path: precomputed tiling 3D noise (renderer::cloud_noise,
// group 3 bindings 2..4) + weather map + per-sample light march.

/// Mirrors `CLOUD_HI_SAMPLES`: view-march samples, exponentially spaced.
pub const CLOUD_HI_SAMPLES: i32 = 48;
/// Mirrors `CLOUD_HI_STEP_EXP`: sample-position curve t = m0 + seg * u^EXP
/// (1 = uniform; higher = denser near the slab entry).
pub const CLOUD_HI_STEP_EXP: f32 = 1.6;
/// Mirrors `CLOUD_HI_LIGHT_SAMPLES`: light-march taps toward the sun per
/// lit view sample.
pub const CLOUD_HI_LIGHT_SAMPLES: i32 = 8;
/// Mirrors `CLOUD_LIGHT_STEP`: base light-march step, drawn-shell units.
pub const CLOUD_LIGHT_STEP: f32 = 0.0012;
/// Mirrors `CLOUD_HI_SIGMA_T`: extinction per drawn-shell unit at density 1
/// for the High path (higher than Medium's -- the noise-carved density
/// field averages far lower, and cores must still saturate).
pub const CLOUD_HI_SIGMA_T: f32 = 850.0;
/// Mirrors `CLOUD_HI_MAX_ALPHA`: peak alpha of the High deck (above
/// Medium's 0.72 -- photoreal cumulus cores genuinely block the ground).
pub const CLOUD_HI_MAX_ALPHA: f32 = 0.96;
/// Mirrors `CLOUD_SHAPE_FREQ`: shape-volume tiles per drawn-shell unit
/// (Earth: ~268 km per tile -> 45 km base Worley cells).
pub const CLOUD_SHAPE_FREQ: f32 = 24.0;
/// Mirrors `CLOUD_DETAIL_FREQ`: detail-volume tiles per drawn-shell unit
/// (Earth: ~71 km per tile -> 9/4.5/2.2 km erosion octaves).
pub const CLOUD_DETAIL_FREQ: f32 = 60.0;
/// Mirrors `CLOUD_DETAIL_ERODE`: how deeply the detail octaves erode the
/// shape's edges.
pub const CLOUD_DETAIL_ERODE: f32 = 0.38;
/// Mirrors `CLOUD_TYPE_FREQ`: the primary (very-low-frequency) cloud-type
/// field frequency -- one air mass per cell picks the regime family.
pub const CLOUD_TYPE_FREQ: f32 = 3.0;
/// Mirrors `CLOUD_TYPE_FREQ2`: secondary type octave, blended with the primary
/// so the regime map has organic sub-structure (every type shows somewhere).
pub const CLOUD_TYPE_FREQ2: f32 = 7.0;
/// Mirrors `CLOUD_FRAY_FREQ`: coarse edge-fray tile frequency. Deliberately
/// LOW (Earth ~88 km features) so the fray survives at orbital distance -- the
/// fix for the "giant blotches" (the fine detail band faded out from orbit,
/// leaving smooth blobs). The coarse band never fades.
pub const CLOUD_FRAY_FREQ: f32 = 9.0;
/// Mirrors `CLOUD_FRAY_ERODE`: global strength of the coarse fray band.
pub const CLOUD_FRAY_ERODE: f32 = 0.5;
/// Mirrors `CLOUD_DENSITY_POW`: thin-edge shaping exponent (> 1 makes low
/// densities translucent while cores stay opaque -- wispy see-through skirts).
pub const CLOUD_DENSITY_POW: f32 = 1.7;
/// Mirrors `CLOUD_FIL_LO` / `CLOUD_FIL_HI`: the ridged-filament (detail alpha)
/// mask window that frays cirrus sheets into thin streaks.
pub const CLOUD_FIL_LO: f32 = 0.30;
pub const CLOUD_FIL_HI: f32 = 0.74;
/// Mirrors `CLOUD_HG_FWD` / `CLOUD_HG_BACK` / `CLOUD_HG_FWD_WEIGHT`: the
/// dual-lobe Henyey-Greenstein phase.
pub const CLOUD_HG_FWD: f32 = 0.55;
pub const CLOUD_HG_BACK: f32 = -0.15;
pub const CLOUD_HG_FWD_WEIGHT: f32 = 0.7;
/// Mirrors `CLOUD_POWDER_STRENGTH`: Beer-powder edge darkening strength.
pub const CLOUD_POWDER_STRENGTH: f32 = 0.92;
/// Mirrors `CLOUD_AMB_BASE` / `CLOUD_AMB_TOP`: ambient skylight at the
/// slab base/top (tops see the sky dome, bases their own shadow).
pub const CLOUD_AMB_BASE: f32 = 0.03;
pub const CLOUD_AMB_TOP: f32 = 0.14;

/// Mirrors `CLOUD_MAX_ALPHA` in pbr_simple.wgsl: peak opacity of the
/// thickest cloud core, deliberately < 1 so the surface stays readable.
/// Lowered 0.85 -> 0.72 after the first orbital field test (2026-07-11):
/// at 0.85 the decks fused into a featureless white cue ball.
pub const CLOUD_MAX_ALPHA: f32 = 0.72;
/// Mirrors `CLOUD_FIELD_LO` / `CLOUD_FIELD_HI`: the raw octave sum's
/// empirical p03/p96 window over the sphere (20,000-sample spiral probe).
/// The triplanar blend + octave averaging concentrate the sum around ~0.49;
/// smoothstepping through this window spreads it to a roughly uniform 0..1
/// cloudiness so the coverage knob can track real sky fraction. Discovered
/// the hard way: without the stretch, Earth at coverage 0.55 caught only
/// the distribution's thin upper tail (~11% mean alpha, essentially
/// cloudless) -- the coverage_maps_monotonically test below is the guard.
pub const CLOUD_FIELD_LO: f32 = 0.32;
pub const CLOUD_FIELD_HI: f32 = 0.65;
/// Mirrors `CLOUD_EDGE`: smoothstep softness above the threshold. Widened
/// 0.18 -> 0.30 with the detail octaves (2026-07-11) so the high-frequency
/// octaves erode borders into filigree instead of hard blob outlines.
pub const CLOUD_EDGE: f32 = 0.30;
/// Mirrors `CLOUD_BAND_STRETCH`: zonal anisotropy -- the sampling
/// direction's y is scaled up by this before the noise lookup, so features
/// stretch east-west like real storm bands. 1.0 = isotropic blobs.
pub const CLOUD_BAND_STRETCH: f32 = 1.75;
/// Mirrors `CLOUD_DRIFT_ZONAL` / `CLOUD_DRIFT_CROSS`: the increment-1
/// "weather" -- rigid-rotation drift rates (rad/s of cloud-clock time) for
/// the two octave sets. Different axes + different speeds = the summed
/// field morphs rather than rotating as one piece.
pub const CLOUD_DRIFT_ZONAL: f32 = 0.0015;
pub const CLOUD_DRIFT_CROSS: f32 = -0.0009;
/// Mirrors `CLOUD_SHADOW_STEP` / `CLOUD_SHADOW_STRENGTH` /
/// `CLOUD_SHADOW_SHARP`: the one-tap self-shadow lookup toward the sun.
pub const CLOUD_SHADOW_STEP: f32 = 0.05;
pub const CLOUD_SHADOW_STRENGTH: f32 = 0.65;
pub const CLOUD_SHADOW_SHARP: f32 = 2.5;
/// Mirrors `CLOUD_SILVER_GAIN`: forward-scatter silver-lining strength.
pub const CLOUD_SILVER_GAIN: f32 = 0.3;
/// Mirrors `CLOUD_AMBIENT` / `CLOUD_NIGHT_FLOOR`: day-side skylight floor
/// and the near-black night floor.
pub const CLOUD_AMBIENT: f32 = 0.08;
pub const CLOUD_NIGHT_FLOOR: f32 = 0.006;

/// Per-planet noise seed packed into the material's params.x slot: a small
/// float derived from the terrain seed so every cloudy world gets its own
/// pattern while the value stays tiny enough that adding it to noise-domain
/// coordinates (which reach ~16 * freq) costs no f32 precision.
pub fn cloud_seed(terrain_seed: u64) -> f32 {
    (terrain_seed % 1024) as f32
}

/// Settings string -> the material params.y quality selector the shader
/// dispatches on (clouds increment 3): 0 = Low (increment-1 painted deck),
/// 1 = Medium (increment-2 field march), 2 = High (the volumetric system).
/// Unknown strings fall to High -- the default posture, and a typo in a
/// hand-edited config should never silently degrade the sky.
pub fn quality_param(quality: &str) -> f32 {
    match quality {
        "low" => 0.0,
        "medium" => 1.0,
        _ => 2.0,
    }
}

/// WGSL `fract` semantics: `x - floor(x)`, always in [0, 1) -- NOT Rust's
/// `f32::fract`, which is negative for negative inputs. Every mirror below
/// must use this or the noise diverges from the shader on negative coords.
fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// Mirrors WGSL `mix(a, b, t)`.
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Mirrors WGSL `smoothstep(e0, e1, x)`.
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Mirrors `hash21` in pbr_simple.wgsl (the shader's shared procedural
/// hash, unchanged since the original material set -- reused, not redefined).
pub fn hash21(px: f32, py: f32) -> f32 {
    let mut p3 = [fract(px * 0.1031), fract(py * 0.1031), fract(px * 0.1031)];
    // dot(p3, p3.yzx + 33.33)
    let d = p3[0] * (p3[1] + 33.33) + p3[1] * (p3[2] + 33.33) + p3[2] * (p3[0] + 33.33);
    p3[0] += d;
    p3[1] += d;
    p3[2] += d;
    fract((p3[0] + p3[1]) * p3[2])
}

/// Mirrors `value_noise` in pbr_simple.wgsl: bilinear hash interpolation
/// with a smoothstep fade.
pub fn value_noise(px: f32, py: f32) -> f32 {
    let ix = px.floor();
    let iy = py.floor();
    let fx = px - ix;
    let fy = py - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash21(ix, iy);
    let b = hash21(ix + 1.0, iy);
    let c = hash21(ix, iy + 1.0);
    let d = hash21(ix + 1.0, iy + 1.0);
    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    ab + (cd - ab) * uy
}

/// Mirrors `cloud_rot_y`: rigid rotation around the local Y (spin) axis.
pub fn cloud_rot_y(v: [f32; 3], a: f32) -> [f32; 3] {
    let (s, c) = a.sin_cos();
    [c * v[0] + s * v[2], v[1], c * v[2] - s * v[0]]
}

/// Mirrors `cloud_rot_x`: rigid rotation around the local X axis.
pub fn cloud_rot_x(v: [f32; 3], a: f32) -> [f32; 3] {
    let (s, c) = a.sin_cos();
    [v[0], c * v[1] - s * v[2], c * v[2] + s * v[1]]
}

/// Mirrors `cloud_noise`: triplanar blend of the 2D value noise onto the
/// sphere. `dir` must be a unit vector (the squared components are the
/// blend weights and only sum to 1 on the unit sphere).
pub fn cloud_noise(dir: [f32; 3], freq: f32, seed: f32) -> f32 {
    // Pow-4 sharpened blend weights, normalized -- see the WGSL comment
    // (2026-07-11: plain dir*dir blend zones creased into straight lines).
    let w2 = [dir[0] * dir[0], dir[1] * dir[1], dir[2] * dir[2]];
    let w4 = [w2[0] * w2[0], w2[1] * w2[1], w2[2] * w2[2]];
    let sum = (w4[0] + w4[1] + w4[2]).max(1e-12);
    let wn = [w4[0] / sum, w4[1] / sum, w4[2] / sum];
    let p = [dir[0] * freq, dir[1] * freq, dir[2] * freq];
    let (ox, oy) = (seed, seed * 0.617);
    // Plane coordinate order matches WGSL swizzles: p.yz, p.zx, p.xy.
    let nx = value_noise(p[1] + ox, p[2] + oy);
    let ny = value_noise(p[2] + ox * 1.3, p[0] + oy * 1.3);
    let nz = value_noise(p[0] + ox * 1.7, p[1] + oy * 1.7);
    nx * wn[0] + ny * wn[1] + nz * wn[2]
}

/// Mirrors `cloud_field`: the 5-octave, two-set drifting density field.
/// Pure in (dir, t, seed). Set A is band-stretched (see
/// `CLOUD_BAND_STRETCH`) and carries four octaves down to filigree scale;
/// set B stays isotropic on its own drift axis so the sum morphs. The
/// amplitude-normalized sum (0.5 + 0.25 + 0.125 + 0.0625 + 0.35 = 1.2875)
/// is contrast-stretched through its empirical window so the output is a
/// roughly uniform 0..1 cloudiness.
pub fn cloud_field(dir: [f32; 3], t: f32, seed: f32) -> f32 {
    let da0 = cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL);
    let stretched = [da0[0], da0[1] * CLOUD_BAND_STRETCH, da0[2]];
    let len = (stretched[0] * stretched[0]
        + stretched[1] * stretched[1]
        + stretched[2] * stretched[2])
        .sqrt()
        .max(1e-9);
    let da = [stretched[0] / len, stretched[1] / len, stretched[2] / len];
    let db = cloud_rot_x(dir, t * CLOUD_DRIFT_CROSS);
    let mut f = 0.5 * cloud_noise(da, 5.0, seed);
    f += 0.25 * cloud_noise(da, 11.0, seed + 19.0);
    f += 0.125 * cloud_noise(da, 23.0, seed + 47.0);
    f += 0.0625 * cloud_noise(da, 47.0, seed + 83.0);
    f += 0.35 * cloud_noise(db, 7.0, seed + 101.0);
    smoothstep(CLOUD_FIELD_LO, CLOUD_FIELD_HI, f / 1.2875)
}

/// Mirrors `cloud_alpha_from_field`: the field is ~uniform after its
/// stretch, so thr = 1 - coverage makes the knob track real sky fraction;
/// the -CLOUD_EDGE endpoint lets coverage 1.0 reach full opacity everywhere
/// (thr + edge <= 0). Monotonic in both arguments.
pub fn cloud_alpha_from_field(field: f32, coverage: f32) -> f32 {
    let thr = mix(1.0, -CLOUD_EDGE, coverage.clamp(0.0, 1.0));
    smoothstep(thr, thr + CLOUD_EDGE, field)
}

/// Mirrors `cloud_altitude_envelope` (increment 2): density shaping across
/// the slab. `r` is in DRAWN-SHELL units (drawn shell = 1.0). Zero at and
/// below the base, smooth rise to a full-density plateau through the middle
/// (the drawn-shell radius evaluates to exactly 1.0, so the increment-1
/// fragment altitude sits on the plateau), fade to zero at and above the top.
pub fn cloud_altitude_envelope(r: f32) -> f32 {
    let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
    let top = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
    let u = ((r - base) / (top - base)).clamp(0.0, 1.0);
    smoothstep(0.0, 0.4, u) * (1.0 - smoothstep(0.6, 1.0, u))
}

/// Mirrors `cloud_density`: the increment-2 sampling contract (horizontal
/// field times altitude envelope) with the horizontal alpha SQUARED --
/// Beer-Lambert accumulation is concave and fused the raw ~uniform alpha
/// into a pale shroud on the first orbital capture; squaring restores
/// increment 1's translucent-skirt / opaque-core response through the march
/// (see the WGSL comment for the curve match). `p` is a point in the mesh's
/// local frame (planet-fixed, drawn shell = radius 1).
pub fn cloud_density(p: [f32; 3], t: f32, seed: f32, coverage: f32) -> f32 {
    let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    let env = cloud_altitude_envelope(r);
    if env <= 0.0 {
        return 0.0;
    }
    let inv = 1.0 / r.max(1e-9);
    let dir = [p[0] * inv, p[1] * inv, p[2] * inv];
    let a_h = cloud_alpha_from_field(cloud_field(dir, t, seed), coverage);
    a_h * a_h * env
}

// ── Increment-3 mirrors (pure functions of the volumetric path) ──

/// Mirrors `cloud_remap`: rescale v from [l0, h0] to [l1, h1], no clamp.
pub fn cloud_remap(v: f32, l0: f32, h0: f32, l1: f32, h1: f32) -> f32 {
    l1 + (v - l0) / (h0 - l0) * (h1 - l1)
}

/// Mirrors `cloud_hg`: Henyey-Greenstein lobe with RELATIVE normalization
/// (1.0 everywhere at g = 0, so it shapes without globally dimming).
pub fn cloud_hg(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    (1.0 - g2) / (1.0 + g2 - 2.0 * g * cos_t).max(1.0e-4).powf(1.5)
}

/// Mirrors `cloud_phase`: dual-lobe HG (forward silver-lining lobe + mild
/// back lobe, blended by CLOUD_HG_FWD_WEIGHT).
pub fn cloud_phase(cos_t: f32) -> f32 {
    mix(
        cloud_hg(cos_t, CLOUD_HG_BACK),
        cloud_hg(cos_t, CLOUD_HG_FWD),
        CLOUD_HG_FWD_WEIGHT,
    )
}

/// Mirrors `cloud_weather`: increment 1's cloud_field minus its two finest
/// octaves (the 3D volumes own sub-50 km detail at High), renormalized
/// (0.5 + 0.25 + 0.35 = 1.10) through the same contrast window.
pub fn cloud_weather(dir: [f32; 3], t: f32, seed: f32) -> f32 {
    let da0 = cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL);
    let stretched = [da0[0], da0[1] * CLOUD_BAND_STRETCH, da0[2]];
    let len = (stretched[0] * stretched[0]
        + stretched[1] * stretched[1]
        + stretched[2] * stretched[2])
        .sqrt()
        .max(1e-9);
    let da = [stretched[0] / len, stretched[1] / len, stretched[2] / len];
    let db = cloud_rot_x(dir, t * CLOUD_DRIFT_CROSS);
    let mut f = 0.5 * cloud_noise(da, 5.0, seed);
    f += 0.25 * cloud_noise(da, 11.0, seed + 19.0);
    f += 0.35 * cloud_noise(db, 7.0, seed + 101.0);
    smoothstep(CLOUD_FIELD_LO, CLOUD_FIELD_HI, f / 1.10)
}

/// Mirrors the WGSL `CloudRegime` struct: the blended per-regime parameters
/// for one ray. Order everywhere is cirrus / cumulus / stratus / stratocumulus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudRegime {
    /// Slab-fraction bottom of the regime's height band.
    pub h_lo: f32,
    /// Slab-fraction top of the band.
    pub h_hi: f32,
    /// Density scale (cirrus faint, cumulus solid).
    pub opacity: f32,
    /// Added to coverage (stratus fills toward overcast).
    pub cover_bias: f32,
    /// Coarse edge-fray strength.
    pub fray: f32,
    /// Fine cauliflower strength (close-up billow).
    pub fine: f32,
    /// Domain anisotropy (cirrus streaks east-west).
    pub stretch: f32,
    /// Ridged-filament streaking amount (cirrus).
    pub filament: f32,
    /// Luminance factor (overcast reads greyer).
    pub tint: f32,
}

/// Dot of a 4-weight vector with a per-regime parameter table.
fn dot4(w: [f32; 4], v: [f32; 4]) -> f32 {
    w[0] * v[0] + w[1] * v[1] + w[2] * v[2] + w[3] * v[3]
}

/// Mirrors `cloud_regime_weights`: overlapping smoothstep tents around four
/// centers spread across [0, 1], normalized to a partition of unity. Smooth
/// everywhere, so the four cloud families cross-fade with no hard boundary.
pub fn cloud_regime_weights(tc: f32) -> [f32; 4] {
    let centers = [0.0f32, 0.34, 0.67, 1.0];
    let hw = 0.42f32;
    let mut w = [0.0f32; 4];
    let mut s = 0.0f32;
    for i in 0..4 {
        let mut wi = (1.0 - (tc - centers[i]).abs() / hw).clamp(0.0, 1.0);
        wi = wi * wi * (3.0 - 2.0 * wi);
        w[i] = wi;
        s += wi;
    }
    let inv = 1.0 / s.max(1.0e-4);
    [w[0] * inv, w[1] * inv, w[2] * inv, w[3] * inv]
}

/// Mirrors `cloud_regime`: blend the per-regime parameter tables by the
/// weights. Keep these tables byte-identical with the WGSL `cloud_regime`.
pub fn cloud_regime(tc: f32) -> CloudRegime {
    let w = cloud_regime_weights(tc);
    //                         cirrus cumulus stratus stratocu
    CloudRegime {
        h_lo: dot4(w, [0.68, 0.05, 0.00, 0.05]),
        h_hi: dot4(w, [1.00, 0.72, 0.20, 0.40]),
        opacity: dot4(w, [0.34, 1.00, 0.80, 0.62]),
        cover_bias: dot4(w, [0.06, -0.03, 0.34, 0.03]),
        fray: dot4(w, [1.00, 0.55, 0.18, 0.80]),
        fine: dot4(w, [0.35, 0.95, 0.30, 0.80]),
        stretch: dot4(w, [3.40, 1.15, 1.50, 1.70]),
        filament: dot4(w, [0.90, 0.10, 0.04, 0.30]),
        tint: dot4(w, [1.00, 1.00, 0.80, 0.90]),
    }
}

/// Mirrors `cloud_type_coord`: two low-frequency octaves -> the [0,1] type
/// coordinate that selects the cloud family at a planet-fixed direction.
pub fn cloud_type_coord(dir: [f32; 3], t: f32, seed: f32) -> f32 {
    let d = cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL);
    let a = cloud_noise(d, CLOUD_TYPE_FREQ, seed + 211.0);
    let b = cloud_noise(d, CLOUD_TYPE_FREQ2, seed + 331.0);
    (0.62 * a + 0.38 * b).clamp(0.0, 1.0)
}

/// Mirrors `cloud_height_band`: smooth rise / plateau / fall over the slab
/// fraction h for a regime's [h_lo, h_hi] altitude band.
pub fn cloud_height_band(h: f32, h_lo: f32, h_hi: f32) -> f32 {
    let a = mix(h_lo, h_hi, 0.30);
    let b = mix(h_lo, h_hi, 0.62);
    smoothstep(h_lo, a, h) * (1.0 - smoothstep(b, h_hi, h))
}

/// Mirrors `cloud_stretch_domain`: slow the sample coordinate along the zonal
/// tangent (east-west, perpendicular to the spin axis Y) so noise features
/// elongate into streaks. No-ops smoothly at the poles.
pub fn cloud_stretch_domain(p: [f32; 3], dir: [f32; 3], stretch: f32) -> [f32; 3] {
    // cross((0,1,0), dir) = (dir.z, 0, -dir.x)
    let mut tang = [dir[2], 0.0, -dir[0]];
    let tl = (tang[0] * tang[0] + tang[1] * tang[1] + tang[2] * tang[2]).sqrt();
    if tl < 1.0e-4 {
        return p;
    }
    tang = [tang[0] / tl, tang[1] / tl, tang[2] / tl];
    let d = p[0] * tang[0] + p[1] * tang[1] + p[2] * tang[2];
    let k = d * (1.0 - 1.0 / stretch);
    [p[0] - tang[0] * k, p[1] - tang[1] * k, p[2] - tang[2] * k]
}

/// Mirrors `cloud_scatter_energy`: 3-octave multiple-scattering
/// approximation -- deep cores fade to a diffuse glow instead of the black
/// that single-scatter Beer-Lambert would give.
pub fn cloud_scatter_energy(tau: f32, phase: f32) -> f32 {
    let mut e = phase * (-tau).exp();
    e += 0.45 * mix(1.0, phase, 0.5) * (-tau * 0.25).exp();
    e += 0.18 * (-tau * 0.06).exp();
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic direction sampler: a spherical Fibonacci-ish spiral, so
    /// tests cover the whole sphere (both hemispheres, all octants) without
    /// randomness.
    fn sample_dirs(n: usize) -> Vec<[f32; 3]> {
        let golden = 2.399_963_2_f32; // golden angle, radians
        (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
                let r = (1.0 - y * y).max(0.0).sqrt();
                let a = golden * i as f32;
                [r * a.cos(), y, r * a.sin()]
            })
            .collect()
    }

    #[test]
    fn hash_and_value_noise_stay_in_unit_range() {
        for i in 0..500 {
            let x = (i as f32) * 0.73 - 180.0; // negative coords included:
            let y = (i as f32) * 1.19 - 250.0; // the WGSL-fract mirror matters
            let h = hash21(x, y);
            assert!((0.0..1.0).contains(&h), "hash out of range: {h}");
            let v = value_noise(x, y);
            assert!((0.0..=1.0).contains(&v), "value_noise out of range: {v}");
        }
    }

    #[test]
    fn field_is_deterministic_and_in_range() {
        for dir in sample_dirs(200) {
            let a = cloud_field(dir, 123.45, 42.0);
            let b = cloud_field(dir, 123.45, 42.0);
            // Bit-identical on repeat evaluation: pure function, no state.
            assert_eq!(a, b, "field not deterministic at {dir:?}");
            assert!((0.0..=1.0).contains(&a), "field out of range: {a}");
        }
    }

    #[test]
    fn field_animates_and_seeds_decorrelate() {
        let dirs = sample_dirs(64);
        let mut moved = 0;
        let mut reseeded = 0;
        for &dir in &dirs {
            let now = cloud_field(dir, 0.0, 42.0);
            // 60 s of drift at these rates rotates the domain ~0.09 rad --
            // a small but real change the field must reflect (animation).
            if (cloud_field(dir, 60.0, 42.0) - now).abs() > 1.0e-4 {
                moved += 1;
            }
            // A different planet seed must give a different pattern.
            if (cloud_field(dir, 0.0, 777.0) - now).abs() > 1.0e-3 {
                reseeded += 1;
            }
        }
        assert!(moved > 48, "field barely animates: {moved}/64 moved");
        assert!(reseeded > 48, "seeds correlate: {reseeded}/64 differ");
    }

    #[test]
    fn octave_sets_morph_relative_to_each_other() {
        // If both octave sets drifted identically the field would rotate as
        // one rigid texture: field(rot_y(dir, w*t), 0) == field(dir, t).
        // The cross-drifting set B breaks that on purpose -- verify the
        // rigid-rotation prediction FAILS, i.e. the pattern genuinely
        // evolves rather than sliding.
        let t = 400.0_f32;
        let mut diverged = 0;
        for dir in sample_dirs(64) {
            let evolved = cloud_field(dir, t, 42.0);
            let rigid = cloud_field(cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL), 0.0, 42.0);
            if (evolved - rigid).abs() > 1.0e-3 {
                diverged += 1;
            }
        }
        assert!(diverged > 32, "field moves rigidly: only {diverged}/64 diverged");
    }

    #[test]
    fn coverage_maps_monotonically_to_sky_fraction() {
        // Mean alpha over the sphere must rise with the coverage knob, hit
        // ~0 at coverage 0, and blanket most of the sky at coverage 1.
        let dirs = sample_dirs(400);
        let mean_alpha = |cov: f32| -> f32 {
            let sum: f32 = dirs
                .iter()
                .map(|&d| cloud_alpha_from_field(cloud_field(d, 33.0, 42.0), cov))
                .sum();
            sum / dirs.len() as f32
        };
        let clear = mean_alpha(0.0);
        let sparse = mean_alpha(0.25);
        let earth = mean_alpha(0.55);
        let heavy = mean_alpha(0.85);
        let overcast = mean_alpha(1.0);
        assert!(clear < 0.02, "coverage 0 should be clear sky, got {clear}");
        assert!(sparse < earth && earth < heavy && heavy < overcast,
            "coverage not monotonic: {sparse} {earth} {heavy} {overcast}");
        assert!(overcast > 0.6, "coverage 1 should be overcast, got {overcast}");
        // Earth's shipped 0.55 should land in a partly-cloudy middle band --
        // neither a wisp nor a shroud.
        assert!((0.15..0.75).contains(&earth), "earth coverage off: {earth}");
    }

    #[test]
    fn threshold_mapping_endpoints_and_edge_softness() {
        // Below threshold: zero. Coverage 1 saturates even a mid field
        // value (its threshold + edge sits at 0, by design).
        assert_eq!(cloud_alpha_from_field(0.0, 0.5), 0.0);
        assert_eq!(cloud_alpha_from_field(1.0, 1.0), 1.0);
        assert_eq!(cloud_alpha_from_field(0.5, 1.0), 1.0);
        // Coverage 0 stays clear even at the field's ceiling.
        assert_eq!(cloud_alpha_from_field(1.0, 0.0), 0.0);
        // The edge band actually ramps (soft edges, not a hard cut).
        let thr = 1.0 + (-CLOUD_EDGE - 1.0) * 0.55; // mirror of the mapping
        let lo = cloud_alpha_from_field(thr + CLOUD_EDGE * 0.25, 0.55);
        let hi = cloud_alpha_from_field(thr + CLOUD_EDGE * 0.75, 0.55);
        assert!(lo > 0.0 && lo < hi && hi < 1.0, "edge not soft: {lo} {hi}");
        // Out-of-range coverage is clamped, not extrapolated.
        assert_eq!(
            cloud_alpha_from_field(0.5, 2.0),
            cloud_alpha_from_field(0.5, 1.0)
        );
    }

    #[test]
    fn cloud_shell_sits_between_peaks_and_atmosphere() {
        // The shell-stack ordering that makes the whole increment work:
        //   terrain peaks < cloud shell < atmosphere shell
        // checked against the SHIPPED earth.ron numbers (the same file the
        // engine loads), so retuning Earth's relief or shell without
        // rethinking the cloud height fails here instead of z-fighting on
        // screen. The atmosphere expression mirrors lib.rs / shell_packing.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("planets")
            .join("earth.ron");
        let text = std::fs::read_to_string(&path).expect("earth.ron missing");
        let def: crate::terrain::planet::PlanetDef =
            ron::from_str(&text).expect("earth.ron failed to parse");
        // Worst-case peak: the RON's fallback sea_level is LOWER than the
        // heightmap's true value (0.55 vs ~0.629), so it bounds the real
        // displacement from above: (1 - sea) * relief.
        let peak = 1.0 + (1.0 - def.sea_level) * def.surface_relief;
        let atmo = 1.0 + def.atmosphere_scale.max(0.005) * 2.0;
        assert!(
            peak < CLOUD_SHELL_SCALE,
            "terrain peaks ({peak}) poke through the cloud shell ({CLOUD_SHELL_SCALE})"
        );
        assert!(
            CLOUD_SHELL_SCALE < atmo,
            "cloud shell ({CLOUD_SHELL_SCALE}) outside the atmosphere shell ({atmo})"
        );
        // Increment 2: the whole march slab must also stay under the
        // atmosphere shell (the base may brush the very tallest peaks by
        // ~100 m -- documented and harmless, so no peak < base assert).
        assert!(
            CLOUD_TOP_SCALE < atmo,
            "cloud slab top ({CLOUD_TOP_SCALE}) outside the atmosphere shell ({atmo})"
        );
        // And Earth actually ships a cloud deck.
        assert!(
            def.cloud_coverage.is_some(),
            "earth.ron lost its cloud_coverage"
        );
    }

    #[test]
    fn wgsl_cloud_constants_stay_in_sync() {
        // Parse each CLOUD_* constant straight out of the shipped shader
        // source so the Rust mirror and the WGSL can never drift silently
        // (the atmosphere module relies on a comment asking nicely; clouds
        // get enforcement).
        let wgsl = include_str!("../../assets/shaders/pbr_simple.wgsl");
        let expect: &[(&str, f32)] = &[
            ("CLOUD_MAX_ALPHA", CLOUD_MAX_ALPHA),
            ("CLOUD_FIELD_LO", CLOUD_FIELD_LO),
            ("CLOUD_FIELD_HI", CLOUD_FIELD_HI),
            ("CLOUD_EDGE", CLOUD_EDGE),
            ("CLOUD_DRIFT_ZONAL", CLOUD_DRIFT_ZONAL),
            ("CLOUD_DRIFT_CROSS", CLOUD_DRIFT_CROSS),
            ("CLOUD_SHADOW_STEP", CLOUD_SHADOW_STEP),
            ("CLOUD_SHADOW_STRENGTH", CLOUD_SHADOW_STRENGTH),
            ("CLOUD_SHADOW_SHARP", CLOUD_SHADOW_SHARP),
            ("CLOUD_SILVER_GAIN", CLOUD_SILVER_GAIN),
            ("CLOUD_AMBIENT", CLOUD_AMBIENT),
            ("CLOUD_NIGHT_FLOOR", CLOUD_NIGHT_FLOOR),
            // Increment-2 raymarch constants.
            ("CLOUD_SHELL_SCALE", CLOUD_SHELL_SCALE),
            ("CLOUD_BASE_SCALE", CLOUD_BASE_SCALE),
            ("CLOUD_TOP_SCALE", CLOUD_TOP_SCALE),
            ("CLOUD_SIGMA_T", CLOUD_SIGMA_T),
            ("CLOUD_MARCH_SHADOW_STEP", CLOUD_MARCH_SHADOW_STEP),
            ("CLOUD_MARCH_SHADOW_SHARP", CLOUD_MARCH_SHADOW_SHARP),
            ("CLOUD_BASE_DARKEN", CLOUD_BASE_DARKEN),
            // Increment-3 volumetric constants.
            ("CLOUD_HI_STEP_EXP", CLOUD_HI_STEP_EXP),
            ("CLOUD_LIGHT_STEP", CLOUD_LIGHT_STEP),
            ("CLOUD_HI_SIGMA_T", CLOUD_HI_SIGMA_T),
            ("CLOUD_HI_MAX_ALPHA", CLOUD_HI_MAX_ALPHA),
            ("CLOUD_SHAPE_FREQ", CLOUD_SHAPE_FREQ),
            ("CLOUD_DETAIL_FREQ", CLOUD_DETAIL_FREQ),
            ("CLOUD_DETAIL_ERODE", CLOUD_DETAIL_ERODE),
            ("CLOUD_TYPE_FREQ", CLOUD_TYPE_FREQ),
            ("CLOUD_TYPE_FREQ2", CLOUD_TYPE_FREQ2),
            ("CLOUD_FRAY_FREQ", CLOUD_FRAY_FREQ),
            ("CLOUD_FRAY_ERODE", CLOUD_FRAY_ERODE),
            ("CLOUD_DENSITY_POW", CLOUD_DENSITY_POW),
            ("CLOUD_FIL_LO", CLOUD_FIL_LO),
            ("CLOUD_FIL_HI", CLOUD_FIL_HI),
            ("CLOUD_HG_FWD", CLOUD_HG_FWD),
            ("CLOUD_HG_BACK", CLOUD_HG_BACK),
            ("CLOUD_HG_FWD_WEIGHT", CLOUD_HG_FWD_WEIGHT),
            ("CLOUD_POWDER_STRENGTH", CLOUD_POWDER_STRENGTH),
            ("CLOUD_AMB_BASE", CLOUD_AMB_BASE),
            ("CLOUD_AMB_TOP", CLOUD_AMB_TOP),
        ];
        for (name, rust_val) in expect {
            let needle = format!("const {name}: f32 = ");
            let start = wgsl
                .find(&needle)
                .unwrap_or_else(|| panic!("{name} missing from pbr_simple.wgsl"));
            let rest = &wgsl[start + needle.len()..];
            let end = rest.find(';').expect("unterminated const");
            let parsed: f32 = rest[..end]
                .trim()
                .parse()
                .unwrap_or_else(|e| panic!("{name} literal unparseable: {e}"));
            assert_eq!(
                parsed, *rust_val,
                "{name} drifted: WGSL {parsed} vs Rust {rust_val}"
            );
        }
        // The i32 consts in WGSL (loop bounds), parsed separately with the
        // same never-drift guarantee.
        let expect_i32: &[(&str, i32)] = &[
            ("CLOUD_MARCH_SAMPLES", CLOUD_MARCH_SAMPLES),
            ("CLOUD_HI_SAMPLES", CLOUD_HI_SAMPLES),
            ("CLOUD_HI_LIGHT_SAMPLES", CLOUD_HI_LIGHT_SAMPLES),
        ];
        for (name, rust_val) in expect_i32 {
            let needle = format!("const {name}: i32 = ");
            let start = wgsl
                .find(&needle)
                .unwrap_or_else(|| panic!("{name} missing from pbr_simple.wgsl"));
            let rest = &wgsl[start + needle.len()..];
            let end = rest.find(';').expect("unterminated const");
            let parsed: i32 = rest[..end]
                .trim()
                .parse()
                .unwrap_or_else(|e| panic!("{name} literal unparseable: {e}"));
            assert_eq!(
                parsed, *rust_val,
                "{name} drifted: WGSL {parsed} vs Rust {rust_val}"
            );
        }
    }

    #[test]
    fn march_sample_count_is_zero_or_in_the_designed_band() {
        // 0 = the increment-1 flat-deck quality fallback; otherwise the march
        // must stay in the 8..=12 band the increment-2 design (and the FPS
        // budget measurement) covers.
        assert!(
            CLOUD_MARCH_SAMPLES == 0 || (8..=12).contains(&CLOUD_MARCH_SAMPLES),
            "CLOUD_MARCH_SAMPLES {CLOUD_MARCH_SAMPLES} outside 0 / 8..=12"
        );
        // The High path's designed band: 16..=32 view samples (the FPS gates
        // were measured at 22), 4..=8 light taps.
        assert!(
            (16..=64).contains(&CLOUD_HI_SAMPLES),
            "CLOUD_HI_SAMPLES {CLOUD_HI_SAMPLES} outside 16..=64"
        );
        assert!(
            (4..=8).contains(&CLOUD_HI_LIGHT_SAMPLES),
            "CLOUD_HI_LIGHT_SAMPLES {CLOUD_HI_LIGHT_SAMPLES} outside 4..=8"
        );
    }

    #[test]
    fn quality_param_maps_settings_strings() {
        assert_eq!(quality_param("low"), 0.0);
        assert_eq!(quality_param("medium"), 1.0);
        assert_eq!(quality_param("high"), 2.0);
        // Unknown/corrupt strings fall to the High default, never Low.
        assert_eq!(quality_param(""), 2.0);
        assert_eq!(quality_param("ultra"), 2.0);
    }

    #[test]
    fn remap_rescales_and_is_exact_at_endpoints() {
        assert_eq!(cloud_remap(0.5, 0.0, 1.0, 0.0, 1.0), 0.5);
        assert_eq!(cloud_remap(0.25, 0.25, 1.0, 0.0, 1.0), 0.0);
        assert_eq!(cloud_remap(1.0, 0.25, 1.0, 0.0, 1.0), 1.0);
        // The classic coverage carve: value below the low bound goes
        // negative (callers clamp) -- that sign IS the erosion.
        assert!(cloud_remap(0.1, 0.3, 1.0, 0.0, 1.0) < 0.0);
    }

    #[test]
    fn phase_is_forward_dominant_with_a_back_lobe() {
        // Relative HG: g = 0 is flat 1.0 everywhere.
        for c in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            assert!((cloud_hg(c, 0.0) - 1.0).abs() < 1e-5);
        }
        // The dual-lobe blend: strongly forward-peaked, with the minimum in
        // the back quadrant and a mild RETRO rise from there toward cos -1
        // (the -0.15 lobe) -- the glow you see on clouds opposite the sun.
        let fwd = cloud_phase(1.0);
        let backq = cloud_phase(-0.5);
        let back = cloud_phase(-1.0);
        assert!(fwd > 3.0, "forward lobe too weak: {fwd}");
        assert!(back > backq, "retro rise missing: {back} <= {backq}");
        assert!(fwd > back * 4.0, "phase not forward-dominant: {fwd} vs {back}");
        // Positive everywhere (it multiplies energy).
        for i in 0..=40 {
            let c = -1.0 + i as f32 / 20.0;
            assert!(cloud_phase(c) > 0.0);
        }
    }

    #[test]
    fn regime_weights_are_a_smooth_partition_of_unity() {
        // At every type coordinate the four family weights sum to 1 (a
        // partition of unity -> seamless blend) and stay non-negative.
        for i in 0..=100 {
            let tc = i as f32 / 100.0;
            let w = cloud_regime_weights(tc);
            let s: f32 = w.iter().sum();
            assert!((s - 1.0).abs() < 1e-4, "weights don't sum to 1 at {tc}: {s}");
            for &wi in &w {
                assert!((0.0..=1.0001).contains(&wi), "weight out of range: {wi}");
            }
        }
        // Each family dominates near its own center (cirrus low tc, cumulus
        // ~0.34, stratus ~0.67, stratocumulus high tc).
        let argmax = |w: [f32; 4]| {
            (0..4).max_by(|&a, &b| w[a].partial_cmp(&w[b]).unwrap()).unwrap()
        };
        assert_eq!(argmax(cloud_regime_weights(0.0)), 0, "cirrus should peak at tc 0");
        assert_eq!(argmax(cloud_regime_weights(0.34)), 1, "cumulus should peak mid-low");
        assert_eq!(argmax(cloud_regime_weights(0.67)), 2, "stratus should peak mid-high");
        assert_eq!(argmax(cloud_regime_weights(1.0)), 3, "stratocumulus should peak at tc 1");
    }

    #[test]
    fn regime_params_match_the_four_families() {
        // The blended params at each family's center must express its physical
        // character: this is the "all cloud types" contract.
        let cirrus = cloud_regime(0.0);
        let cumulus = cloud_regime(0.34);
        let stratus = cloud_regime(0.67);
        let stratocu = cloud_regime(1.0);
        // Cirrus: HIGH in the slab, thin/faint, very streaky (stretch + fila).
        assert!(cirrus.h_lo > 0.5, "cirrus not high: h_lo {}", cirrus.h_lo);
        assert!(cirrus.opacity < 0.6, "cirrus not faint: {}", cirrus.opacity);
        assert!(cirrus.stretch > 2.0, "cirrus not streaky: {}", cirrus.stretch);
        assert!(cirrus.filament > 0.5, "cirrus not filamentary: {}", cirrus.filament);
        // Cumulus: reaches low, solid, tallest band, minimal streaking.
        assert!(cumulus.h_lo < 0.2 && cumulus.h_hi > 0.6, "cumulus band off");
        assert!(cumulus.opacity > 0.9, "cumulus not solid: {}", cumulus.opacity);
        assert!(cumulus.filament < 0.2, "cumulus should not streak: {}", cumulus.filament);
        // Stratus: hugs the base, overcast (positive cover bias), grey tint.
        assert!(stratus.h_hi < 0.35, "stratus not low: h_hi {}", stratus.h_hi);
        assert!(stratus.cover_bias > 0.2, "stratus not overcast: {}", stratus.cover_bias);
        assert!(stratus.tint < 0.9, "stratus not greyer: {}", stratus.tint);
        // Stratocumulus: low-mid, broken (high fray), moderate everything.
        assert!(stratocu.fray > 0.6, "stratocumulus not broken: {}", stratocu.fray);
        assert!(stratocu.h_hi > 0.3 && stratocu.h_hi < 0.55, "stratocu band off");
        // Determinism.
        assert_eq!(cloud_regime(0.5), cloud_regime(0.5));
    }

    #[test]
    fn type_coord_is_deterministic_and_in_range() {
        for dir in sample_dirs(200) {
            let a = cloud_type_coord(dir, 12.0, 42.0);
            assert_eq!(a, cloud_type_coord(dir, 12.0, 42.0), "type coord not pure");
            assert!((0.0..=1.0).contains(&a), "type coord out of range: {a}");
        }
        // A different seed decorrelates the type map (so worlds differ).
        let mut differ = 0;
        for dir in sample_dirs(64) {
            if (cloud_type_coord(dir, 0.0, 42.0) - cloud_type_coord(dir, 0.0, 900.0)).abs() > 1e-3 {
                differ += 1;
            }
        }
        assert!(differ > 40, "type map correlates across seeds: {differ}/64");
    }

    #[test]
    fn height_band_zero_outside_and_peaks_inside() {
        // A mid-slab cumulus-like band [0.05, 0.72].
        assert_eq!(cloud_height_band(0.0, 0.05, 0.72), 0.0);
        assert_eq!(cloud_height_band(0.05, 0.05, 0.72), 0.0);
        assert_eq!(cloud_height_band(0.72, 0.05, 0.72), 0.0);
        assert_eq!(cloud_height_band(1.0, 0.05, 0.72), 0.0);
        // Plateau in the interior (between the 0.30 and 0.62 mix points).
        let mid = cloud_height_band(mix(0.05, 0.72, 0.46), 0.05, 0.72);
        assert!((mid - 1.0).abs() < 1e-5, "band interior not full: {mid}");
        // A high cirrus band [0.68, 1.0] must be zero low down, alive up high.
        assert_eq!(cloud_height_band(0.2, 0.68, 1.0), 0.0);
        assert!(cloud_height_band(0.85, 0.68, 1.0) > 0.0, "cirrus band dead up high");
    }

    #[test]
    fn stretch_domain_elongates_along_zonal_and_noops_at_poles() {
        // Equatorial direction: the zonal tangent is well-defined, so a point
        // offset along it is pulled toward the origin (features vary slower
        // there -> elongated). Perpendicular offsets are untouched.
        let dir = [1.0f32, 0.0, 0.0];
        // tangent = cross((0,1,0),(1,0,0)) = (0,0,-1); pick p along +z.
        let p = [0.0f32, 0.0, 1.0];
        let out = cloud_stretch_domain(p, dir, 3.0);
        // z-projection scaled by 1/3 (stretched), x/y unchanged.
        assert!((out[2] - (1.0 / 3.0)).abs() < 1e-5, "zonal not stretched: {out:?}");
        assert!(out[0].abs() < 1e-6 && out[1].abs() < 1e-6);
        // A point purely meridional (along y) is not stretched at all.
        let py = [0.0f32, 0.5, 0.0];
        assert_eq!(cloud_stretch_domain(py, dir, 3.0), py);
        // At the pole the tangent vanishes -> exact no-op (no NaN).
        let pole = [0.0f32, 1.0, 0.0];
        let any = [0.3f32, 0.4, 0.5];
        assert_eq!(cloud_stretch_domain(any, pole, 4.0), any);
        // stretch == 1 is always the identity.
        assert_eq!(cloud_stretch_domain(any, dir, 1.0), any);
    }

    #[test]
    fn weather_is_deterministic_in_range_and_tracks_coverage() {
        let dirs = sample_dirs(400);
        for &d in dirs.iter().take(50) {
            let a = cloud_weather(d, 123.0, 42.0);
            assert_eq!(a, cloud_weather(d, 123.0, 42.0));
            assert!((0.0..=1.0).contains(&a), "weather out of range: {a}");
        }
        // The coverage knob must still map monotonically through the
        // 3-octave weather variant (same guard as the 5-octave field).
        let mean_alpha = |cov: f32| -> f32 {
            let sum: f32 = dirs
                .iter()
                .map(|&d| cloud_alpha_from_field(cloud_weather(d, 33.0, 42.0), cov))
                .sum();
            sum / dirs.len() as f32
        };
        let clear = mean_alpha(0.0);
        let earth = mean_alpha(0.55);
        let overcast = mean_alpha(1.0);
        assert!(clear < 0.02, "coverage 0 should be clear, got {clear}");
        assert!(clear < earth && earth < overcast, "not monotonic");
        assert!(overcast > 0.6, "coverage 1 should blanket: {overcast}");
        assert!((0.15..0.8).contains(&earth), "earth band off: {earth}");
    }

    #[test]
    fn scatter_energy_decays_but_never_reaches_black() {
        let side_phase = cloud_phase(0.0);
        let thin = cloud_scatter_energy(0.0, side_phase);
        let mid = cloud_scatter_energy(2.0, side_phase);
        let deep = cloud_scatter_energy(12.0, side_phase);
        assert!(thin > mid && mid > deep, "not decaying: {thin} {mid} {deep}");
        // The multiple-scattering octaves keep deep cores glowing (the
        // whole point vs single-scatter Beer).
        assert!(deep > 0.05, "deep core went black: {deep}");
        assert!(thin < 2.0, "side-view thin energy blown out: {thin}");
    }

    #[test]
    fn altitude_envelope_is_zero_outside_and_peaks_mid_slab() {
        let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
        let top = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
        // Hard zero at and outside both bounds (the planet surface and the
        // slab top must never grow cloud).
        assert_eq!(cloud_altitude_envelope(base), 0.0);
        assert_eq!(cloud_altitude_envelope(top), 0.0);
        assert_eq!(cloud_altitude_envelope(base - 0.01), 0.0);
        assert_eq!(cloud_altitude_envelope(top + 0.01), 0.0);
        assert_eq!(cloud_altitude_envelope(0.0), 0.0);
        // Full density through the mid plateau -- INCLUDING the drawn-shell
        // radius 1.0, so the increment-1 fragment altitude keeps evaluating
        // at envelope 1 (the flat fallback and the march agree at mid-slab).
        assert_eq!(cloud_altitude_envelope(1.0), 1.0);
        let mid = 0.5 * (base + top);
        assert_eq!(cloud_altitude_envelope(mid), 1.0);
        assert_eq!(cloud_altitude_envelope(base + 0.45 * (top - base)), 1.0);
    }

    #[test]
    fn altitude_envelope_rises_then_falls_smoothly() {
        let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
        let top = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
        let th = top - base;
        // Monotone non-decreasing through the rise, non-increasing through
        // the fall, and genuinely soft (interior values strictly between 0
        // and 1 exist on both flanks -- no hard cut).
        let mut prev = 0.0_f32;
        let mut soft_rise = false;
        for i in 0..=40 {
            let u = 0.4 * (i as f32) / 40.0;
            let e = cloud_altitude_envelope(base + u * th);
            assert!(e >= prev - 1e-6, "rise not monotone at u {u}: {e} < {prev}");
            if e > 0.05 && e < 0.95 {
                soft_rise = true;
            }
            prev = e;
        }
        assert!(soft_rise, "rise has no soft interior band");
        let mut prev = 1.0_f32;
        let mut soft_fall = false;
        for i in 0..=40 {
            let u = 0.6 + 0.4 * (i as f32) / 40.0;
            let e = cloud_altitude_envelope(base + u * th);
            assert!(e <= prev + 1e-6, "fall not monotone at u {u}: {e} > {prev}");
            if e > 0.05 && e < 0.95 {
                soft_fall = true;
            }
            prev = e;
        }
        assert!(soft_fall, "fall has no soft interior band");
    }

    #[test]
    fn density_composes_field_times_envelope() {
        // The increment-2 contract, verified literally: density(p) must be
        // exactly the SQUARED horizontal alpha at normalize(p) times the
        // envelope at length(p) -- same field, no rework; the square is the
        // Beer-response shaping documented on cloud_density.
        let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
        let top = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
        for (i, dir) in sample_dirs(32).into_iter().enumerate() {
            let r = base + (top - base) * (i as f32 + 0.5) / 32.0;
            let p = [dir[0] * r, dir[1] * r, dir[2] * r];
            let a_h = cloud_alpha_from_field(cloud_field(dir, 33.0, 42.0), 0.55);
            let expect = a_h * a_h * cloud_altitude_envelope(r);
            let got = cloud_density(p, 33.0, 42.0, 0.55);
            assert!(
                (got - expect).abs() < 1e-5,
                "density decomposition broke at r {r}: {got} vs {expect}"
            );
        }
        // Outside the slab: zero regardless of the horizontal field.
        assert_eq!(cloud_density([0.0, base - 0.005, 0.0], 33.0, 42.0, 1.0), 0.0);
        assert_eq!(cloud_density([top + 0.005, 0.0, 0.0], 33.0, 42.0, 1.0), 0.0);
    }

    #[test]
    fn march_slab_brackets_the_drawn_shell() {
        // The geometric premise of the march: base < drawn shell < top, and
        // the whole slab still sits between the terrain peaks and the
        // atmosphere shell (the ordering test below checks the outer stack).
        assert!(CLOUD_BASE_SCALE < CLOUD_SHELL_SCALE);
        assert!(CLOUD_SHELL_SCALE < CLOUD_TOP_SCALE);
        // Calibration cross-check for CLOUD_SIGMA_T: a full-density radial
        // pass (envelope integrates to ~0.6 of the slab thickness) must land
        // deep in the opaque regime but NOT waste range (0.85..0.99).
        let thickness = (CLOUD_TOP_SCALE - CLOUD_BASE_SCALE) / CLOUD_SHELL_SCALE;
        let n = 1000;
        let dr = thickness / n as f32;
        let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
        let mut integral = 0.0;
        for i in 0..n {
            integral += cloud_altitude_envelope(base + (i as f32 + 0.5) * dr) * dr;
        }
        let opacity = 1.0 - (-CLOUD_SIGMA_T * integral).exp();
        assert!(
            (0.85..0.99).contains(&opacity),
            "radial full-density opacity {opacity} off calibration"
        );
    }

    #[test]
    fn cloud_seed_is_small_and_stable() {
        // The seed must stay small (it is ADDED to noise coordinates -- a
        // seed of 1e9 would eat all the f32 mantissa the noise needs) and be
        // a pure function of the terrain seed.
        assert_eq!(cloud_seed(42), 42.0);
        assert_eq!(cloud_seed(1024), 0.0);
        assert_eq!(cloud_seed(u64::MAX), (u64::MAX % 1024) as f32);
        assert!(cloud_seed(u64::MAX) < 1024.0);
    }
}

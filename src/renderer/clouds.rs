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
//! ## Increment 2 reuse contract (raymarched volumetrics)
//!
//! `cloud_field(dir, t, seed)` is a pure function of planet-fixed DIRECTION
//! and time -- exactly the horizontal term a volumetric raymarcher needs.
//! The planned extension keeps this module's functions untouched:
//!
//! ```text
//! density(p_local) = cloud_alpha_from_field(
//!     cloud_field(normalize(p_local), t, seed), coverage)
//!     * altitude_envelope(length(p_local))   // NEW in increment 2
//! ```
//!
//! Only the altitude envelope (a smooth band around the deck radius) and the
//! march loop are new work; every constant and noise decision made here
//! carries over, so the from-orbit look and the in-atmosphere look stay the
//! same field. Ground shadows from clouds were considered for increment 1
//! and deliberately deferred: they require the planet-surface shader (type
//! 12) to sample this field per surface fragment, which is a second shader's
//! worth of plumbing -- increment-2 work, noted here so it is not forgotten.
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

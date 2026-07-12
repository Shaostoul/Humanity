//! Planet ocean waves + land close-range detail: the Rust mirror of the
//! WGSL math (v0.816).
//!
//! The actual rendering lives in `assets/shaders/pbr_simple.wgsl` (material
//! type 12, textured path, params.w bit 1 = the Settings "Surface detail"
//! toggle). This module exists for the same reasons as its siblings
//! `renderer::clouds` / `renderer::atmosphere`:
//!
//! 1. **Testable mirrors** of every pure shader function (the anti-alias
//!    octave fade, the directional wave-gradient sum, the wave-presence
//!    blend, the land detail factor), so determinism, ranges, tangency and
//!    the far-field convergence guarantee are locked by unit tests instead
//!    of eyeballs.
//! 2. **One documented home** for the wave-octave table and the design.
//! 3. **Constant sync enforcement**: `wgsl_water_constants_stay_in_sync`
//!    parses the shader source and fails on any drift.
//!
//! ## The model
//!
//! Water: six directional gravity-wave trains (2 km down to 50 m
//! wavelength) evaluated in the planet-local tangent frame. Each train is a
//! moving sine whose SLOPE gradient perturbs the smooth sphere normal; the
//! perturbed normal then drives Schlick Fresnel against a cheap analytic
//! sky, the graded bathymetry body term, and a tight sun-only Blinn sparkle
//! lobe (the shading itself lives in WGSL `water_shade`; it is a direct
//! function of camera/sun uniforms, so the pure-math mirror stops at the
//! gradient).
//!
//! Land: 2-3 octaves of triplanar value noise (10 km / 1 km / 150 m)
//! multiply the sampled photo albedo by +-10-15 percent luminance. No biome
//! recoloring -- detail synthesis under the imagery.
//!
//! ## The anti-aliasing rule (load-bearing)
//!
//! Every octave fades with `detail_octave_fade`: the number of projected
//! pixels one wavelength spans (wavelength / footprint, footprint =
//! fragment distance * `PLANET_PIXEL_ANGLE`), smoothstepped through
//! [`DETAIL_FADE_LO`, `DETAIL_FADE_HI`]. Exactly 0.0 when the octave would
//! alias, so the orbit view is BIT-IDENTICAL to the pre-v0.816 look (the
//! `orbit_footprint_kills_every_octave` test is the regression gate), and
//! distant ocean converges to the smooth normal instead of shimmering.

/// Mirrors `PLANET_PIXEL_ANGLE` in pbr_simple.wgsl: the analytic estimate of
/// one pixel's view angle in radians (~90 deg vertical FOV over a ~1400 px
/// viewport, rounded down so octaves fade EARLIER on small windows --
/// conservative against shimmer). footprint_m = distance_m * this.
pub const PLANET_PIXEL_ANGLE: f32 = 0.0008;
/// Mirrors `DETAIL_FADE_LO` / `DETAIL_FADE_HI`: the octave visibility band
/// in projected pixels per wavelength. Zero at or below LO, fully on at or
/// above HI; both sit comfortably above the 2 px Nyquist floor.
pub const DETAIL_FADE_LO: f32 = 4.0;
pub const DETAIL_FADE_HI: f32 = 12.0;
/// Mirrors `WATER_F0`: Fresnel reflectance of water at normal incidence.
pub const WATER_F0: f32 = 0.02;
/// Mirrors `WATER_SPEC_POWER` / `WATER_SPEC_GAIN`: the tight Blinn sparkle
/// lobe on the wave-perturbed normal. Sun-only (the fill light would paint
/// a bogus second hotspot -- same reasoning as the v0.810 glint).
pub const WATER_SPEC_POWER: f32 = 900.0;
pub const WATER_SPEC_GAIN: f32 = 1.1;
/// Mirrors `WATER_SKY_GAIN`: analytic reflected-sky brightness as a
/// fraction of sun intensity.
pub const WATER_SKY_GAIN: f32 = 0.5;
/// Mirrors `WATER_ICE_LUM_LO` / `WATER_ICE_LUM_HI`: sea-ice guard. Polar
/// below-sea faces carry the water flag but grade toward cap white; wave
/// presence fades out across this max-channel-luminance band so pack ice
/// never shades like open ocean.
pub const WATER_ICE_LUM_LO: f32 = 0.35;
pub const WATER_ICE_LUM_HI: f32 = 0.6;

/// One directional wave train (mirrors the WAVE{N}_* constants in WGSL).
#[derive(Debug, Clone, Copy)]
pub struct WaveOctave {
    /// Wavelength in metres.
    pub lambda_m: f32,
    /// Temporal frequency in cycles per second of cloud-clock time (near
    /// the deep-water dispersion rate sqrt(g / (2 pi lambda))).
    pub cps: f32,
    /// Slope amplitude (dimensionless steepness A*k -- what the normal
    /// perturbation actually consumes, scale-free).
    pub slope: f32,
    /// Fixed planet-local propagation direction (unit vector; projected
    /// onto the local tangent plane per fragment).
    pub dir: [f32; 3],
}

/// The wave-octave table, largest first (the largest octave's fade doubles
/// as the master `wave_presence` blend). Six trains spanning 2 km to 50 m,
/// each with its own direction and speed so the sum genuinely moves and
/// sparkles instead of sliding as one frozen pattern.
pub const WAVE_OCTAVES: [WaveOctave; 6] = [
    WaveOctave { lambda_m: 2000.0, cps: 0.028, slope: 0.06, dir: [0.707_106_8, 0.0, 0.707_106_8] },
    WaveOctave { lambda_m: 850.0, cps: 0.045, slope: 0.07, dir: [0.962_250_4, 0.192_450_1, 0.192_450_1] },
    WaveOctave { lambda_m: 360.0, cps: 0.07, slope: 0.09, dir: [0.267_261_2, 0.534_522_5, 0.801_783_7] },
    WaveOctave { lambda_m: 150.0, cps: 0.105, slope: 0.1, dir: [-0.577_350_3, 0.577_350_3, 0.577_350_3] },
    WaveOctave { lambda_m: 80.0, cps: 0.145, slope: 0.11, dir: [0.408_248_3, -0.816_496_6, 0.408_248_3] },
    WaveOctave { lambda_m: 50.0, cps: 0.18, slope: 0.12, dir: [-0.666_666_7, 0.333_333_3, -0.666_666_7] },
];

/// One land detail octave (mirrors the LAND{N}_* constants in WGSL):
/// (wavelength metres, luminance amplitude, noise seed).
pub const LAND_OCTAVES: [(f32, f32, f32); 3] = [
    (10_000.0, 0.1, 3.7),
    (1000.0, 0.08, 17.3),
    (150.0, 0.06, 31.9),
];

const TAU: f32 = 6.283_185_5;

/// Mirrors WGSL `smoothstep(e0, e1, x)`.
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Mirrors WGSL `fract` (always in [0, 1), unlike Rust's `f32::fract`).
fn fract(x: f32) -> f32 {
    x - x.floor()
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len3(a: [f32; 3]) -> f32 {
    dot3(a, a).sqrt()
}

/// Mirrors `detail_octave_fade`: the per-octave anti-alias fade. Exactly
/// 0.0 when one wavelength spans <= DETAIL_FADE_LO projected pixels,
/// exactly 1.0 at >= DETAIL_FADE_HI, smooth in between.
pub fn detail_octave_fade(lambda_m: f32, footprint_m: f32) -> f32 {
    smoothstep(DETAIL_FADE_LO, DETAIL_FADE_HI, lambda_m / footprint_m)
}

/// Mirrors `wave_presence`: the master water-shading blend -- the fade of
/// the LONGEST wave octave. 0 from orbit (the v0.810 path is untouched),
/// 1 once 2 km swells span DETAIL_FADE_HI pixels (~200 km altitude).
pub fn wave_presence(footprint_m: f32) -> f32 {
    detail_octave_fade(WAVE_OCTAVES[0].lambda_m, footprint_m)
}

/// Mirrors `wave_octave`: one train's contribution to the tangent-plane
/// slope gradient at planet-local point `p_m` (metres) with sphere normal
/// `n`. The fixed 3D direction projects onto the local tangent plane; the
/// phase wraps through fract() BEFORE the sin so the argument stays inside
/// one period at planet-radius coordinate magnitudes.
pub fn wave_octave(
    p_m: [f32; 3],
    n: [f32; 3],
    oct: &WaveOctave,
    t: f32,
    footprint_m: f32,
) -> [f32; 3] {
    let fade = detail_octave_fade(oct.lambda_m, footprint_m);
    if fade <= 0.001 {
        return [0.0; 3];
    }
    let dn = dot3(oct.dir, n);
    let mut tp = [
        oct.dir[0] - n[0] * dn,
        oct.dir[1] - n[1] * dn,
        oct.dir[2] - n[2] * dn,
    ];
    let l = len3(tp);
    if l < 1e-4 {
        return [0.0; 3];
    }
    tp = [tp[0] / l, tp[1] / l, tp[2] / l];
    let cycles = dot3(p_m, tp) / oct.lambda_m + t * oct.cps;
    let ph = fract(cycles) * TAU;
    let s = oct.slope * fade * ph.cos();
    [tp[0] * s, tp[1] * s, tp[2] * s]
}

/// Mirrors `water_wave_gradient`: the summed slope gradient of all six
/// trains. The perturbed water normal is normalize(n - this).
pub fn water_wave_gradient(p_m: [f32; 3], n: [f32; 3], t: f32, footprint_m: f32) -> [f32; 3] {
    let mut g = [0.0f32; 3];
    for oct in &WAVE_OCTAVES {
        let o = wave_octave(p_m, n, oct, t, footprint_m);
        g[0] += o[0];
        g[1] += o[1];
        g[2] += o[2];
    }
    g
}

/// Mirrors `surface_detail_noise`: triplanar value noise on the sphere for
/// the land octaves -- the same pow-4-weight construction as the cloud
/// field's sphere noise but with its own seed offsets, kept INDEPENDENT of
/// the cloud functions (which have their own rework cadence). Reuses the
/// one Rust mirror of the shader's shared `value_noise` primitive.
pub fn surface_detail_noise(dir: [f32; 3], freq: f32, seed: f32) -> f32 {
    use super::clouds::value_noise;
    let w2 = [dir[0] * dir[0], dir[1] * dir[1], dir[2] * dir[2]];
    let w4 = [w2[0] * w2[0], w2[1] * w2[1], w2[2] * w2[2]];
    let sum = (w4[0] + w4[1] + w4[2]).max(1e-12);
    let wn = [w4[0] / sum, w4[1] / sum, w4[2] / sum];
    let p = [dir[0] * freq, dir[1] * freq, dir[2] * freq];
    let (ox, oy) = (seed, seed * 0.713);
    let nx = value_noise(p[1] + ox, p[2] + oy);
    let ny = value_noise(p[2] + ox * 1.31, p[0] + oy * 1.31);
    let nz = value_noise(p[0] + ox * 1.73, p[1] + oy * 1.73);
    nx * wn[0] + ny * wn[1] + nz * wn[2]
}

/// Mirrors `land_detail_factor`: the multiplicative albedo factor from the
/// 3 land octaves, each anti-alias faded, clamped to [0.7, 1.3]. Returns
/// exactly 1.0 when every octave is faded out (the orbit regression gate).
pub fn land_detail_factor(dir: [f32; 3], r_m: f32, footprint_m: f32) -> f32 {
    let mut f = 0.0f32;
    for (lambda, amp, seed) in LAND_OCTAVES {
        f += amp
            * detail_octave_fade(lambda, footprint_m)
            * (2.0 * surface_detail_noise(dir, r_m / lambda, seed) - 1.0);
    }
    (1.0 + f).clamp(0.7, 1.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic whole-sphere direction sampler (same spiral as the
    /// clouds tests).
    fn sample_dirs(n: usize) -> Vec<[f32; 3]> {
        let golden = 2.399_963_2_f32;
        (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
                let r = (1.0 - y * y).max(0.0).sqrt();
                let a = golden * i as f32;
                [r * a.cos(), y, r * a.sin()]
            })
            .collect()
    }

    const EARTH_R: f32 = 6.371e6;

    /// Footprint helper: metres per pixel at a given altitude (km).
    fn footprint_at_alt_km(alt_km: f32) -> f32 {
        alt_km * 1000.0 * PLANET_PIXEL_ANGLE
    }

    #[test]
    fn wave_directions_are_unit_vectors() {
        for (i, oct) in WAVE_OCTAVES.iter().enumerate() {
            let l = len3(oct.dir);
            assert!(
                (l - 1.0).abs() < 1e-4,
                "wave octave {i} direction not unit: |d| = {l}"
            );
        }
    }

    #[test]
    fn octave_table_is_ordered_and_in_the_designed_bands() {
        // Largest-first ordering (wave_presence keys off index 0), the
        // designed 2 km..50 m span, and sane slope/speed magnitudes.
        let mut prev = f32::INFINITY;
        for oct in &WAVE_OCTAVES {
            assert!(oct.lambda_m < prev, "octaves not descending");
            prev = oct.lambda_m;
            assert!((0.01..=0.3).contains(&oct.cps), "cps out of band: {}", oct.cps);
            assert!((0.02..=0.2).contains(&oct.slope), "slope out of band: {}", oct.slope);
        }
        assert_eq!(WAVE_OCTAVES[0].lambda_m, 2000.0);
        assert_eq!(WAVE_OCTAVES[5].lambda_m, 50.0);
        // Total worst-case slope stays below 1 (the perturbed normal can
        // never flip through the tangent plane).
        let total: f32 = WAVE_OCTAVES.iter().map(|o| o.slope).sum();
        assert!(total < 1.0, "summed slope {total} too steep");
    }

    #[test]
    fn octave_fade_endpoints_and_monotonicity() {
        let lambda = 500.0;
        // Aliasing regime (wavelength <= LO pixels): exact zero.
        assert_eq!(detail_octave_fade(lambda, lambda / DETAIL_FADE_LO), 0.0);
        assert_eq!(detail_octave_fade(lambda, lambda), 0.0);
        // Resolved regime (>= HI pixels): exact one.
        assert_eq!(detail_octave_fade(lambda, lambda / DETAIL_FADE_HI), 1.0);
        assert_eq!(detail_octave_fade(lambda, lambda / 100.0), 1.0);
        // Monotone non-decreasing as the footprint shrinks.
        let mut prev = -1.0f32;
        for i in 0..=60 {
            let px = DETAIL_FADE_LO + (DETAIL_FADE_HI - DETAIL_FADE_LO) * (i as f32) / 60.0;
            let f = detail_octave_fade(lambda, lambda / px);
            assert!(f >= prev - 1e-6, "fade not monotone at {px} px: {f} < {prev}");
            assert!((0.0..=1.0).contains(&f));
            prev = f;
        }
    }

    /// THE orbit regression gate: at the 12,000 km capture altitude every
    /// wave and land octave must fade to EXACTLY zero, so the far field is
    /// bit-identical to the pre-v0.816 look.
    #[test]
    fn orbit_footprint_kills_every_octave() {
        let fp = footprint_at_alt_km(12_000.0); // 9600 m/px
        assert_eq!(wave_presence(fp), 0.0, "wave presence must be 0 from orbit");
        for oct in &WAVE_OCTAVES {
            assert_eq!(
                detail_octave_fade(oct.lambda_m, fp),
                0.0,
                "wave octave {} m visible from orbit",
                oct.lambda_m
            );
        }
        for dir in sample_dirs(64) {
            assert_eq!(
                land_detail_factor(dir, EARTH_R, fp),
                1.0,
                "land factor must be exactly 1.0 from orbit at {dir:?}"
            );
            let g = water_wave_gradient([dir[0] * EARTH_R, dir[1] * EARTH_R, dir[2] * EARTH_R], dir, 123.0, fp);
            assert_eq!(g, [0.0; 3], "wave gradient must vanish from orbit");
        }
    }

    #[test]
    fn presence_ramps_in_through_the_descent() {
        // 400 km: the broad-glint transition band (partial presence).
        let p400 = wave_presence(footprint_at_alt_km(400.0));
        assert!(
            p400 > 0.05 && p400 < 0.6,
            "400 km presence should be partial, got {p400}"
        );
        // 200 km and below: full water shading.
        assert_eq!(wave_presence(footprint_at_alt_km(200.0)), 1.0);
        assert_eq!(wave_presence(footprint_at_alt_km(10.0)), 1.0);
        // Monotone in altitude.
        let p50 = wave_presence(footprint_at_alt_km(50.0));
        assert!(p400 < p50 && p50 <= 1.0);
    }

    #[test]
    fn gradient_is_tangent_bounded_and_deterministic() {
        let fp = footprint_at_alt_km(10.0); // everything resolved
        let max_slope: f32 = WAVE_OCTAVES.iter().map(|o| o.slope).sum();
        for dir in sample_dirs(128) {
            let p = [dir[0] * EARTH_R, dir[1] * EARTH_R, dir[2] * EARTH_R];
            let g = water_wave_gradient(p, dir, 77.7, fp);
            let g2 = water_wave_gradient(p, dir, 77.7, fp);
            assert_eq!(g, g2, "gradient not deterministic at {dir:?}");
            // Tangent to the sphere: no radial component beyond float noise.
            let radial = dot3(g, dir);
            assert!(
                radial.abs() < 1e-4,
                "gradient has radial leak {radial} at {dir:?}"
            );
            // Bounded by the summed slope amplitudes.
            assert!(
                len3(g) <= max_slope + 1e-4,
                "gradient magnitude {} exceeds slope budget {max_slope}",
                len3(g)
            );
        }
    }

    /// The "moves around" requirement, in math: 40 s of cloud-clock time
    /// must visibly decorrelate the wave field (the two-capture proof in
    /// the live verification mirrors this).
    #[test]
    fn wave_field_moves_over_40_seconds() {
        let fp = footprint_at_alt_km(10.0);
        let mut moved = 0;
        let dirs = sample_dirs(64);
        for &dir in &dirs {
            let p = [dir[0] * EARTH_R, dir[1] * EARTH_R, dir[2] * EARTH_R];
            let a = water_wave_gradient(p, dir, 100.0, fp);
            let b = water_wave_gradient(p, dir, 140.0, fp);
            let d = [(a[0] - b[0]), (a[1] - b[1]), (a[2] - b[2])];
            if len3(d) > 0.02 {
                moved += 1;
            }
        }
        assert!(moved > 48, "wave field barely moves: {moved}/64 points changed");
    }

    #[test]
    fn land_factor_stays_in_band_and_varies_spatially() {
        let fp = footprint_at_alt_km(10.0);
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for dir in sample_dirs(400) {
            let f = land_detail_factor(dir, EARTH_R, fp);
            assert!((0.7..=1.3).contains(&f), "land factor out of band: {f}");
            let f2 = land_detail_factor(dir, EARTH_R, fp);
            assert_eq!(f, f2, "land factor not deterministic");
            lo = lo.min(f);
            hi = hi.max(f);
        }
        // The variation must actually exist (not a constant field) and be
        // the designed subtle band, not a repaint.
        assert!(hi - lo > 0.05, "land detail flat: range {lo}..{hi}");
        assert!(hi - lo < 0.61, "land detail too loud: range {lo}..{hi}");
    }

    #[test]
    fn land_octaves_fade_in_by_wavelength_at_400km() {
        // At 400 km only the 10 km octave is resolved (footprint 320 m/px:
        // 10 km = 31 px on, 1 km = 3.1 px off, 150 m off) -- the "first
        // detail octave" capture expectation, locked here.
        let fp = footprint_at_alt_km(400.0);
        assert_eq!(detail_octave_fade(LAND_OCTAVES[0].0, fp), 1.0);
        assert_eq!(detail_octave_fade(LAND_OCTAVES[1].0, fp), 0.0);
        assert_eq!(detail_octave_fade(LAND_OCTAVES[2].0, fp), 0.0);
        // At 10 km all three are fully resolved.
        let fp10 = footprint_at_alt_km(10.0);
        for (lambda, _, _) in LAND_OCTAVES {
            assert_eq!(detail_octave_fade(lambda, fp10), 1.0);
        }
    }

    #[test]
    fn surface_detail_noise_in_range_and_seeded() {
        let mut differs = 0;
        for dir in sample_dirs(100) {
            let n = surface_detail_noise(dir, 637.0, 3.7);
            assert!((0.0..=1.0).contains(&n), "noise out of range: {n}");
            if (surface_detail_noise(dir, 637.0, 17.3) - n).abs() > 1e-3 {
                differs += 1;
            }
        }
        assert!(differs > 75, "seeds correlate: {differs}/100 differ");
    }

    #[test]
    fn wgsl_water_constants_stay_in_sync() {
        // Parse every constant straight out of the shipped shader source so
        // the Rust mirror and the WGSL can never drift silently (same
        // enforcement pattern as renderer::clouds).
        let wgsl = include_str!("../../assets/shaders/pbr_simple.wgsl");
        let parse_f32 = |name: &str| -> f32 {
            let needle = format!("const {name}: f32 = ");
            let start = wgsl
                .find(&needle)
                .unwrap_or_else(|| panic!("{name} missing from pbr_simple.wgsl"));
            let rest = &wgsl[start + needle.len()..];
            let end = rest.find(';').expect("unterminated const");
            rest[..end]
                .trim()
                .parse()
                .unwrap_or_else(|e| panic!("{name} literal unparseable: {e}"))
        };
        let parse_vec3 = |name: &str| -> [f32; 3] {
            let needle = format!("const {name}: vec3<f32> = vec3<f32>(");
            let start = wgsl
                .find(&needle)
                .unwrap_or_else(|| panic!("{name} missing from pbr_simple.wgsl"));
            let rest = &wgsl[start + needle.len()..];
            let end = rest.find(')').expect("unterminated vec3 const");
            let parts: Vec<f32> = rest[..end]
                .split(',')
                .map(|s| s.trim().parse().unwrap_or_else(|e| panic!("{name}: {e}")))
                .collect();
            assert_eq!(parts.len(), 3, "{name} is not a 3-component literal");
            [parts[0], parts[1], parts[2]]
        };

        let scalars: &[(&str, f32)] = &[
            ("PLANET_PIXEL_ANGLE", PLANET_PIXEL_ANGLE),
            ("DETAIL_FADE_LO", DETAIL_FADE_LO),
            ("DETAIL_FADE_HI", DETAIL_FADE_HI),
            ("WATER_F0", WATER_F0),
            ("WATER_SPEC_POWER", WATER_SPEC_POWER),
            ("WATER_SPEC_GAIN", WATER_SPEC_GAIN),
            ("WATER_SKY_GAIN", WATER_SKY_GAIN),
            ("WATER_ICE_LUM_LO", WATER_ICE_LUM_LO),
            ("WATER_ICE_LUM_HI", WATER_ICE_LUM_HI),
        ];
        for (name, rust_val) in scalars {
            let parsed = parse_f32(name);
            assert_eq!(parsed, *rust_val, "{name} drifted: WGSL {parsed} vs Rust {rust_val}");
        }
        for (i, oct) in WAVE_OCTAVES.iter().enumerate() {
            let n = i + 1;
            assert_eq!(parse_f32(&format!("WAVE{n}_LAMBDA")), oct.lambda_m, "WAVE{n}_LAMBDA drifted");
            assert_eq!(parse_f32(&format!("WAVE{n}_CPS")), oct.cps, "WAVE{n}_CPS drifted");
            assert_eq!(parse_f32(&format!("WAVE{n}_SLOPE")), oct.slope, "WAVE{n}_SLOPE drifted");
            let d = parse_vec3(&format!("WAVE{n}_DIR"));
            for c in 0..3 {
                assert!(
                    (d[c] - oct.dir[c]).abs() < 1e-6,
                    "WAVE{n}_DIR component {c} drifted: WGSL {} vs Rust {}",
                    d[c],
                    oct.dir[c]
                );
            }
        }
        for (i, (lambda, amp, seed)) in LAND_OCTAVES.iter().enumerate() {
            let n = i + 1;
            assert_eq!(parse_f32(&format!("LAND{n}_LAMBDA")), *lambda, "LAND{n}_LAMBDA drifted");
            assert_eq!(parse_f32(&format!("LAND{n}_AMP")), *amp, "LAND{n}_AMP drifted");
            assert_eq!(parse_f32(&format!("LAND{n}_SEED")), *seed, "LAND{n}_SEED drifted");
        }
    }
}

//! CPU twin of the ocean-shell wave height (pbr_simple.wgsl, material
//! type 16). The golden rule from docs/design/ocean.md: whatever the water
//! DRAWS must be exactly what physics SAMPLES -- so this module reproduces
//! `ocean_wave_height` analytically (same four directional cosine trains,
//! same constants, same fract-before-cos phase wrap) for buoyancy, the
//! ground/float clamp, and eventually ship physics.
//!
//! No domain warp here OR in the shader's height path (the warp is
//! shading-only, in the normal perturbation), precisely so this twin stays
//! a four-line sum. A guard test parses the WGSL for every OCEAN_W*
//! constant and the wave direction vectors, so a shader-side retune that
//! forgets this file fails the build immediately.

/// One wave train: fixed planet-local unit direction, wavelength (m),
/// temporal frequency (cycles per cloud-clock second), height amplitude (m).
#[derive(Clone, Copy, Debug)]
pub struct WaveTrain {
    pub dir: [f32; 3],
    pub lambda_m: f32,
    pub cps: f32,
    pub height_m: f32,
}

/// The six geometric wave trains, in shader order. Trains 1-4 reuse the
/// shading octaves' fixed vectors (WAVE1/3/4/6_DIR in the WGSL).
/// Trains 5-6 (v0.912) are the NEAR chop - resolution-faded by a few
/// hundred metres; this CPU twin runs at the player where that fade is
/// ~1, so the float height includes them at full strength.
///
/// v0.1017 (operator: "all the verts are kinda bouncing up and down
/// rapidly"): the shader used to phase the chop off planet-radius f32
/// coordinates, whose ~0.5 m quantization SHIFTS as the camera moves -
/// on a 6 m wavelength that scrambled the whole near field into
/// camera-coupled vertex jitter (the f32-at-scale defect class, GPU
/// edition). The chop now phases in the camera-ANCHORED small domain
/// with AXIS-ALIGNED directions and wavelengths that DIVIDE the 64 m
/// anchor modulus, so anchor re-snaps shift phase by exact integers
/// (invisible) and this twin's f64 planet-coordinate phase stays
/// mathematically identical mod 1 (64k/16 and 64k/6.4 are integers).
pub const TRAINS: [WaveTrain; 6] = [
    WaveTrain { dir: [0.7071068, 0.0, 0.7071068], lambda_m: 2000.0, cps: 0.028, height_m: 1.1 },
    WaveTrain { dir: [0.2672612, 0.5345225, 0.8017837], lambda_m: 360.0, cps: 0.07, height_m: 0.7 },
    WaveTrain { dir: [-0.5773503, 0.5773503, 0.5773503], lambda_m: 150.0, cps: 0.105, height_m: 0.45 },
    WaveTrain { dir: [0.0, 1.0, 0.0], lambda_m: 32.0, cps: 0.221, height_m: 0.4 },
    WaveTrain { dir: [1.0, 0.0, 0.0], lambda_m: 16.0, cps: 0.31, height_m: 0.35 },
    WaveTrain { dir: [0.0, 0.0, 1.0], lambda_m: 6.4, cps: 0.49, height_m: 0.1 },
];

/// The water surface floats this far ABOVE the nominal sea sphere
/// (v0.882): beach-line terrain sits within centimeters of sea level, and
/// coincident surfaces z-shimmered as patch anchors requantized every
/// frame ("water flickering various shades of blue... worst around land
/// mass above water"). 1.2 m of freeboard separates them cleanly; the
/// float clamp and the mesh builder share this constant (drawn == sampled).
pub const SURFACE_LIFT_M: f32 = 1.2;

/// Worst-case |wave height|: the sum of every train's amplitude. Useful for
/// conservative bounds (patch radial bands, "am I possibly submerged").
/// v0.957 (field report "no actual wave height"): chop trains 4-6 lifted to
/// Beaufort-4-ish amplitudes now that the water mesh is fine enough to
/// actually draw them (WATER_MAX_PATCH_DEPTH 17).
pub const MAX_WAVE_HEIGHT_M: f32 = 1.1 + 0.7 + 0.45 + 0.4 + 0.35 + 0.1;

/// Shoal damping for the GEOMETRIC wave height: full amplitude in open
/// water, fading to zero as the sea shallows so bigger waves never stab
/// through beach terrain (the drawn waterline stays clean). Mirrors the
/// vertex shader's smoothstep(0.4, 7.0, depth_m) exactly (drawn == sampled).
pub fn shoal_factor(depth_m: f32) -> f32 {
    let t = ((depth_m - 0.4) / (7.0 - 0.4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Wave height with shoal damping applied, for callers that know the local
/// water depth (the player float clamp). `wave_height_m` stays the
/// open-deep form for callers without bathymetry.
pub fn wave_height_shoaled_m(p_m: glam::DVec3, t: f32, depth_m: f32) -> f32 {
    wave_height_m(p_m, t) * shoal_factor(depth_m)
}

const TAU: f32 = std::f32::consts::TAU;

/// WGSL fract(): x - floor(x), always in [0, 1). Rust's f32::fract() is
/// sign-preserving (x - trunc(x)) and would diverge for negative phases.
#[inline]
fn wgsl_fract(x: f32) -> f32 {
    x - x.floor()
}

/// Signed wave height in meters at planet-local position `p_m` (meters)
/// and cloud-clock time `t` (camera.sun_color.w). The phase runs in f64
/// (v0.1017): at planet-radius magnitudes an f32 dot product quantizes at
/// ~0.5 m, which on the short chop trains was most of a wavelength of
/// pure noise - the physics float must be smooth even where the shader's
/// anchored-domain trick is unavailable.
pub fn wave_height_m(p_m: glam::DVec3, t: f32) -> f32 {
    trains_height(&TRAINS, p_m, t)
}

/// The three long-swell trains only (lambda > 64 m): the part of the sea
/// that stays analytic in FFT-ocean mode, because the 64 m FFT tile
/// cannot hold wavelengths longer than itself (see ocean_fft.rs).
pub fn swell_height_m(p_m: glam::DVec3, t: f32) -> f32 {
    trains_height(&TRAINS[..3], p_m, t)
}

fn trains_height(trains: &[WaveTrain], p_m: glam::DVec3, t: f32) -> f32 {
    let mut h = 0.0f32;
    for tr in trains {
        let d = glam::DVec3::new(tr.dir[0] as f64, tr.dir[1] as f64, tr.dir[2] as f64);
        let phase = wgsl_fract64(p_m.dot(d) / tr.lambda_m as f64 - (t * tr.cps) as f64);
        h += tr.height_m * (phase as f32 * TAU).cos();
    }
    h
}

/// WGSL fract() in f64: x - floor(x), always in [0, 1).
#[inline]
fn wgsl_fract64(x: f64) -> f64 {
    x - x.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_bounded_and_animates() {
        let p = glam::DVec3::new(4_000_000.0, 2_000_000.0, -4_500_000.0);
        let mut moved = false;
        let mut prev = None;
        for i in 0..200 {
            let t = i as f32 * 0.37;
            let h = wave_height_m(p, t);
            assert!(
                h.abs() <= MAX_WAVE_HEIGHT_M + 1e-3,
                "height {h} exceeds the amplitude sum"
            );
            if let Some(p) = prev {
                if (h - p as f32).abs() > 1e-4 {
                    moved = true;
                }
            }
            prev = Some(h);
        }
        assert!(moved, "wave height never animated");
    }

    #[test]
    fn wgsl_fract_matches_spec_for_negatives() {
        assert_eq!(wgsl_fract(1.25), 0.25);
        assert_eq!(wgsl_fract(-0.25), 0.75);
        assert_eq!(wgsl_fract(-3.0), 0.0);
    }

    /// The lockstep guard: parse pbr_simple.wgsl and verify every OCEAN_W*
    /// constant and the four wave-direction vectors match TRAINS exactly.
    /// A shader retune that forgets this module fails here, not in-game.
    #[test]
    fn shader_constants_match_cpu_twin() {
        // v0.973 source split: scan the CONCATENATION of the megashader's
        // parts, so a constant moved between parts can never dodge this
        // lockstep check.
        let parts_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/shaders/pbr");
        let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&parts_dir)
            .expect("shader parts dir readable")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        names.sort();
        let wgsl: String = names
            .iter()
            .map(|p| std::fs::read_to_string(p).expect("shader part readable"))
            .collect();
        let grab = |name: &str| -> f32 {
            let pat = format!("const {name}: f32 = ");
            let at = wgsl.find(&pat).unwrap_or_else(|| panic!("{name} missing in WGSL"));
            let rest = &wgsl[at + pat.len()..];
            let end = rest.find(';').expect("terminated");
            rest[..end].trim().parse().unwrap_or_else(|_| panic!("{name} unparsable"))
        };
        let grab_dir = |name: &str| -> [f32; 3] {
            let pat = format!("const {name}: vec3<f32> = vec3<f32>(");
            let at = wgsl.find(&pat).unwrap_or_else(|| panic!("{name} missing in WGSL"));
            let rest = &wgsl[at + pat.len()..];
            let end = rest.find(')').expect("terminated");
            let parts: Vec<f32> = rest[..end]
                .split(',')
                .map(|p| p.trim().parse().expect("component"))
                .collect();
            [parts[0], parts[1], parts[2]]
        };
        let shader_dirs = [
            "WAVE1_DIR",
            "WAVE3_DIR",
            "WAVE4_DIR",
            "OCEAN_W4_DIR",
            "OCEAN_W5_DIR",
            "OCEAN_W6_DIR",
        ];
        for (i, tr) in TRAINS.iter().enumerate() {
            let n = i + 1;
            assert_eq!(grab(&format!("OCEAN_W{n}_LAMBDA")), tr.lambda_m, "W{n} lambda");
            assert_eq!(grab(&format!("OCEAN_W{n}_CPS")), tr.cps, "W{n} cps");
            assert_eq!(grab(&format!("OCEAN_W{n}_HEIGHT")), tr.height_m, "W{n} height");
            assert_eq!(grab_dir(shader_dirs[i]), tr.dir, "W{n} direction");
        }
        // The shader's ocean_wave_height must pair train i with the same
        // direction constant this table does (order is load-bearing).
        let body_at = wgsl.find("fn ocean_wave_height").expect("fn present");
        let body = &wgsl[body_at..body_at + 1400];
        for (i, d) in shader_dirs.iter().enumerate() {
            assert!(
                body.contains(&format!("{d}, OCEAN_W{}_LAMBDA", i + 1)),
                "train {} not paired with {d} in the shader sum",
                i + 1
            );
        }
    }
}

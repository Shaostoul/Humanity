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

/// The four geometric wave trains, in shader order. Directions reuse the
/// shading octaves' fixed vectors (WAVE1/3/4/6_DIR in the WGSL).
pub const TRAINS: [WaveTrain; 4] = [
    WaveTrain { dir: [0.7071068, 0.0, 0.7071068], lambda_m: 2000.0, cps: 0.028, height_m: 1.1 },
    WaveTrain { dir: [0.2672612, 0.5345225, 0.8017837], lambda_m: 360.0, cps: 0.07, height_m: 0.7 },
    WaveTrain { dir: [-0.5773503, 0.5773503, 0.5773503], lambda_m: 150.0, cps: 0.105, height_m: 0.45 },
    WaveTrain { dir: [-0.6666667, 0.3333333, -0.6666667], lambda_m: 50.0, cps: 0.18, height_m: 0.22 },
];

/// Worst-case |wave height|: the sum of every train's amplitude. Useful for
/// conservative bounds (patch radial bands, "am I possibly submerged").
pub const MAX_WAVE_HEIGHT_M: f32 = 1.1 + 0.7 + 0.45 + 0.22;

const TAU: f32 = std::f32::consts::TAU;

/// WGSL fract(): x - floor(x), always in [0, 1). Rust's f32::fract() is
/// sign-preserving (x - trunc(x)) and would diverge for negative phases.
#[inline]
fn wgsl_fract(x: f32) -> f32 {
    x - x.floor()
}

/// Signed wave height in meters at planet-local position `p_m` (meters,
/// f32 like the shader) and cloud-clock time `t` (camera.sun_color.w).
pub fn wave_height_m(p_m: glam::Vec3, t: f32) -> f32 {
    let mut h = 0.0f32;
    for tr in TRAINS {
        let d = glam::Vec3::from_array(tr.dir);
        let phase = wgsl_fract(p_m.dot(d) / tr.lambda_m - t * tr.cps);
        h += tr.height_m * (phase * TAU).cos();
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_bounded_and_animates() {
        let p = glam::Vec3::new(4_000_000.0, 2_000_000.0, -4_500_000.0);
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
        let wgsl = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/shaders/pbr_simple.wgsl"),
        )
        .expect("shader readable");
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
        let shader_dirs = ["WAVE1_DIR", "WAVE3_DIR", "WAVE4_DIR", "WAVE6_DIR"];
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
        let body = &wgsl[body_at..body_at + 700];
        for (i, d) in shader_dirs.iter().enumerate() {
            assert!(
                body.contains(&format!("{d}, OCEAN_W{}_LAMBDA", i + 1)),
                "train {} not paired with {d} in the shader sum",
                i + 1
            );
        }
    }
}

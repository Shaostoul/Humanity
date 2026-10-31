//! The celestial pass's projection: the one reverse-Z matrix, its near and far planes and the
//! sky threshold that every planet-scale view is built from.
//!
//! WHY IT LIVES IN TERRAIN (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was the top of `renderer::camera`, and the patch selector's
//! culling frustum (`terrain::planet_chunks::FrustumPlanes`, built from it in f64 by the frame
//! loop and by terrain's and the simulation's own tests) reached up into the renderer for it.
//! It is pure glam: the camera, the cloud, god-ray, emission and sun passes read it from here,
//! beneath the renderer and the simulation both.

use glam::Mat4;

// ── The celestial pass's projection ──────────────────────────
//
// The celestial pass draws everything that is not the home: the planets, their
// terrain, trees and buildings, and the pieces a player builds on a planet's
// ground (engine/planet_build.rs). Its projection is reverse-Z (near and far
// swapped) into a Depth32Float buffer with the far plane at 1e13 m, so the
// solar system fits.
//
// THE NEAR PLANE is 5 cm (2026-09-27, the planet-build review). It was 1 m,
// and the corner of a 1 m near plane reaches 2.27 m from the eye at the
// widest view (90 degrees vertical at 16:9: sqrt(1 + 1 + (16/9)^2) = 2.27),
// so a wall you stood beside on a planet was cut open and showed the outside
// through it. The first answer drew a second copy of every nearby planet
// piece in the scene pass, which clears depth, so the copy painted over
// anything in front of it (a chest inside a hut vanished behind its own
// wall). At 5 cm the corner is 11 cm out, closer than a body can stand to a
// wall, and the copy is gone.
//
// WHY IT COSTS NOTHING. With the far plane at 1e13 m the reverse-Z depth of a
// point d metres away is near / d to within d / 1e13 (see the tests). A
// smaller near scales every depth by the same factor, and a float's relative
// precision does not depend on its size, so two surfaces are resolved at
// exactly the same distance ratio as before: depth precision is the same at
// every range, orbit included. What does change is every depth's absolute
// VALUE, so a shader that tested depth against a fixed number (the sky test
// in the god rays and the SSAO, "depth <= 1e-7") now takes that number from
// here ([`celestial_depth_at`]) instead. The passes that turn depth back into
// metres (the cloud composite, the emission pass, the SSAO) use this matrix's
// m22 and m32, so they follow on their own.
//
// EVERY SITE that builds this projection uses these (the frame loop's f64
// culling frustum, the god rays, the SSAO, the cloud composite, the emission
// pass, the surface tests): a copy that drifted would cull, clip or
// linearise at the wrong distance with no error anywhere.

/// The celestial pass's near plane, metres. See the section above.
pub const CELESTIAL_NEAR_M: f32 = 0.05;
/// The celestial pass's far plane, metres: past the outer planets.
pub const CELESTIAL_FAR_M: f32 = 1.0e13;
/// A celestial depth sample farther than this counts as SKY for the passes
/// that only ask "is anything close drawn here?" (the god rays let light
/// through it, the SSAO skips it): 10,000 km, the distance the old fixed
/// threshold of 1e-7 meant with a 1 m near plane.
pub const CELESTIAL_SKY_M: f32 = 1.0e7;

/// The celestial pass's projection: reverse-Z, [`CELESTIAL_NEAR_M`] to
/// [`CELESTIAL_FAR_M`]. The one definition every consumer reads.
pub fn celestial_projection(fov_degrees: f32, aspect: f32) -> Mat4 {
    Mat4::perspective_rh(fov_degrees.to_radians(), aspect, CELESTIAL_FAR_M, CELESTIAL_NEAR_M)
}

/// The same projection in f64, for the frame loop's culling frustum (built
/// in f64 at planet scale) and the tests that mirror it.
pub fn celestial_projection_f64(fov_degrees: f64, aspect: f64) -> glam::DMat4 {
    glam::DMat4::perspective_rh(
        fov_degrees.to_radians(),
        aspect,
        f64::from(CELESTIAL_FAR_M),
        f64::from(CELESTIAL_NEAR_M),
    )
}

/// The depth the celestial pass writes for a point `distance_m` in front of
/// the eye (reverse-Z: 1 at the near plane, 0 at the far plane). For fixed
/// thresholds, such as [`CELESTIAL_SKY_M`]'s.
pub fn celestial_depth_at(distance_m: f32) -> f32 {
    let m = celestial_projection(60.0, 1.0).to_cols_array_2d();
    (m[3][2] - m[2][2] * distance_m) / distance_m
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// The celestial pass's depth for a point `d` metres ahead, computed the
    /// way the GPU does it: the f32 projection times an f32 view-space point,
    /// then the divide. `near` lets a test compare against the old 1 m plane.
    fn depth_f32(near: f32, d: f32) -> f32 {
        let p = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, CELESTIAL_FAR_M, near);
        let clip = p * glam::Vec4::new(0.0, 0.0, -d, 1.0);
        clip.z / clip.w
    }

    /// A wall a hand's width from the eye is not cut. The near plane's
    /// corner at the widest view the game offers (90 degrees vertical at
    /// 16:9) is 2.27 near-plane distances from the eye: 2.27 m with the old
    /// 1 m plane, which is why a planet wall beside the player showed the
    /// outside through it, and 11 cm now. A point 15 cm from the eye in the
    /// corner direction lies inside the frustum (depth at most 1). Red
    /// check, run: CELESTIAL_NEAR_M back at 1.0 puts the corner 2.27 m out
    /// and the first assertion fails (that point would sit at depth 15, far
    /// outside the frustum).
    #[test]
    fn a_wall_beside_the_eye_is_inside_the_celestial_near_plane() {
        let (fov, aspect) = (90f32, 16.0 / 9.0);
        let half_h = (fov.to_radians() * 0.5).tan();
        let corner = Vec3::new(half_h * aspect, half_h, -1.0);
        let reach = CELESTIAL_NEAR_M * corner.length();
        assert!(reach < 0.12, "the near plane's corner is {reach} m out");
        let p = celestial_projection(fov, aspect) * (corner.normalize() * 0.15).extend(1.0);
        let depth = p.z / p.w;
        assert!(depth > 0.0 && depth <= 1.0, "a point 15 cm out in the corner: depth {depth}");
        // The old plane's corner, the figure the docs had wrong as 1.8 m.
        assert!((corner.length() - 2.27).abs() < 0.005);
    }

    /// NO Z-FIGHTING IS BOUGHT BY THE SMALLER NEAR PLANE, AT ANY RANGE. At
    /// each range from arm's length to the Sun, 4,000 points spaced one part
    /// in a million apart are pushed through the f32 matrix as the GPU does:
    /// the new plane resolves at least as many distinct depths as the old
    /// 1 m one did (orbit to the surface is 1.2e7 m, the limb from 400 km
    /// is 2.3e6 m, coastlines under them), every depth is a normal float
    /// (no denormal flush at the far end), and the order is kept. This is the
    /// reverse-Z float property the constant's comment relies on: the depth
    /// is near / d, and a float's relative precision does not care what
    /// near is.
    #[test]
    fn the_smaller_near_plane_resolves_every_range_as_finely_as_the_old_one() {
        for range in [0.5_f32, 30.0, 2.0e3, 4.0e5, 2.3e6, 1.2e7, 3.8e8, 1.5e11] {
            let (mut old, mut new) = (Vec::new(), Vec::new());
            for k in 0..4_000 {
                let d = range * (1.0 + k as f32 * 1.0e-6);
                old.push(depth_f32(1.0, d));
                new.push(depth_f32(CELESTIAL_NEAR_M, d));
            }
            let distinct = |v: &[f32]| {
                let mut s: Vec<u32> = v.iter().map(|x| x.to_bits()).collect();
                s.dedup();
                s.len()
            };
            assert!(new.windows(2).all(|w| w[1] <= w[0]), "depth falls with distance at {range} m");
            assert!(new.iter().all(|x| x.is_normal() && *x > 0.0), "normal floats at {range} m");
            if range >= 1.0 {
                assert!(
                    distinct(&new) >= distinct(&old),
                    "{range} m: {} distinct depths now, {} with the 1 m plane",
                    distinct(&new),
                    distinct(&old)
                );
            }
        }
    }

    /// The fullscreen passes that turn depth back into metres (the cloud
    /// composite, the emission pass, the SSAO) use m32 / (depth + m22) from
    /// the same matrix, so they read true distances at every range with the
    /// new plane, and an empty sky (depth 0) reads as beyond the far plane
    /// region they treat as open. The SKY threshold the god rays and the SSAO
    /// test against sits at 10,000 km as it did: a point at 9,000 km is
    /// drawn geometry, one at 11,000 km is sky.
    #[test]
    fn depth_linearises_to_true_distance_and_the_sky_threshold_keeps_its_meaning() {
        let m = celestial_projection(60.0, 16.0 / 9.0).to_cols_array_2d();
        for d in [0.06_f32, 1.0, 250.0, 4.0e5, 1.2e7, 3.8e8] {
            let back = m[3][2] / (depth_f32(CELESTIAL_NEAR_M, d) + m[2][2]);
            assert!((back / d - 1.0).abs() < 2.0e-6, "{d} m reads back as {back}");
        }
        assert!(m[3][2] / m[2][2] > 0.99e13, "the sky reads as the far plane");
        let sky = celestial_depth_at(CELESTIAL_SKY_M);
        assert!(sky.is_normal() && sky > 0.0);
        assert!(depth_f32(CELESTIAL_NEAR_M, 9.0e6) > sky && depth_f32(CELESTIAL_NEAR_M, 1.1e7) < sky);
        // The same line the old fixed 1e-7 drew with the 1 m plane.
        assert!(depth_f32(1.0, 9.0e6) > 1.0e-7 && depth_f32(1.0, 1.1e7) < 1.0e-7);
    }
}

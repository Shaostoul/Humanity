//! Which way a camera looks: the tangent frame on a planet's surface (up is the local radial,
//! so the horizon is level wherever you stand), the forward a yaw and pitch give in it, and the
//! yaw and pitch back from a direction, in that frame and in the world-Y one.
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was the middle of `surface_walk`, the simulation's walk, and
//! the renderer's camera aims with it (`Camera::forward`, `aim_along`), so the renderer reached
//! into the simulation for four functions of glam. It names no planet and no walk: here the
//! camera, the walk, the station and the main loop all read the one definition.
//!
//! Pure glam, ungated: its tests run under every feature set.

use glam::Vec3;

/// Build the orthonormal tangent basis `(east, north)` for a given radial
/// `up`. Pole-safe: near the poles `up` is nearly parallel to world-Y, so the
/// world reference axis switches to world-X to keep the cross product well
/// conditioned. Returns unit vectors with `east x north = up` (a right-handed
/// frame in the order east, north, up).
pub fn tangent_basis(up: Vec3) -> (Vec3, Vec3) {
    let up = up.normalize_or_zero();
    if up.length_squared() < 0.5 {
        // Degenerate up: fall back to the world axes so callers never NaN.
        return (Vec3::X, Vec3::Z);
    }
    // Reference axis: world-Y, unless up hugs the pole (then world-X).
    let world_ref = if up.dot(Vec3::Y).abs() > 0.999 { Vec3::X } else { Vec3::Y };
    let east = world_ref.cross(up).normalize_or_zero();
    let north = up.cross(east); // unit already (up perp east, both unit)
    (east, north)
}

/// Forward direction of a surface-oriented camera: `up` is the local radial,
/// `yaw` spins around it (in the tangent plane) and `pitch` tilts toward the
/// zenith (positive) or the ground (negative). At pitch 0 the forward lies in
/// the tangent plane, so the horizon is level; at negative pitch the forward
/// gains a `-up` component, i.e. it points below the horizon toward the body
/// centre.
pub fn surface_forward(up: Vec3, yaw: f32, pitch: f32) -> Vec3 {
    let up = up.normalize_or_zero();
    let (east, north) = tangent_basis(up);
    let horizontal = north * yaw.cos() + east * yaw.sin();
    (horizontal * pitch.cos() + up * pitch.sin()).normalize_or_zero()
}

/// Inverse of `surface_forward`: the (yaw, pitch) that aim a surface-oriented
/// camera with radial `up` along `dir`. Used so a scenic capture or a mode
/// transition can preserve a desired look direction across the world-Y ->
/// radial basis change. Pitch is clamped just inside +-90 degrees (the same
/// limit mouse-look enforces).
pub fn surface_look_angles(up: Vec3, dir: Vec3) -> (f32, f32) {
    let up = up.normalize_or_zero();
    let d = dir.normalize_or_zero();
    if up.length_squared() < 0.5 || d.length_squared() < 0.5 {
        return (0.0, 0.0);
    }
    let (east, north) = tangent_basis(up);
    let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
    let pitch = d.dot(up).clamp(-1.0, 1.0).asin().clamp(-max_pitch, max_pitch);
    // horizontal = cos(yaw)*north + sin(yaw)*east  =>  yaw = atan2(e, n).
    let yaw = d.dot(east).atan2(d.dot(north));
    (yaw, pitch)
}

/// Inverse of the WORLD-Y camera `forward()` (yaw about world-Y, pitch about
/// world-X): `forward = (sin yaw cos pitch, sin pitch, -cos yaw cos pitch)`,
/// so yaw = atan2(x, -z) and pitch = asin(y). Used when LEAVING surface mode
/// to preserve the look direction back into the default basis. Mirrors
/// `dev_travel::look_angles` but for a Vec3.
pub fn world_look_angles(dir: Vec3) -> (f32, f32) {
    let d = dir.normalize_or_zero();
    if d.length_squared() < 0.5 {
        return (0.0, 0.0);
    }
    let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
    let pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-max_pitch, max_pitch);
    let yaw = d.x.atan2(-d.z);
    (yaw, pitch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vec3, b: Vec3, eps: f32) -> bool {
        (a - b).length() < eps
    }

    #[test]
    fn tangent_basis_is_orthonormal_and_right_handed() {
        for up in [
            Vec3::Y,
            Vec3::X,
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-3.0, 2.0, 5.0),
            Vec3::new(0.2, -0.9, 0.1),
        ] {
            let up_n = up.normalize();
            let (east, north) = tangent_basis(up);
            assert!((east.length() - 1.0).abs() < 1e-5, "east not unit for {up}");
            assert!((north.length() - 1.0).abs() < 1e-5, "north not unit for {up}");
            assert!(east.dot(up_n).abs() < 1e-5, "east not perp up for {up}");
            assert!(north.dot(up_n).abs() < 1e-5, "north not perp up for {up}");
            assert!(east.dot(north).abs() < 1e-5, "east not perp north for {up}");
            // Right-handed in the order (east, north, up).
            assert!(approx(east.cross(north), up_n, 1e-4), "handedness wrong for {up}");
        }
    }

    #[test]
    fn tangent_basis_is_pole_safe() {
        // Almost straight up (and its mirror): must not collapse to zero.
        for up in [Vec3::new(0.0, 1.0, 1e-6), Vec3::new(1e-7, -1.0, 0.0)] {
            let (east, north) = tangent_basis(up);
            assert!(east.is_finite() && north.is_finite());
            assert!((east.length() - 1.0).abs() < 1e-4, "east collapsed at the pole");
            assert!((north.length() - 1.0).abs() < 1e-4, "north collapsed at the pole");
        }
    }

    #[test]
    fn horizon_is_level_at_pitch_zero() {
        // At pitch 0 the forward has zero radial component for ANY up, so the
        // view lies in the tangent plane: the horizon reads level.
        for up in [Vec3::Y, Vec3::new(1.0, 0.3, -0.5), Vec3::new(-2.0, 5.0, 1.0)] {
            let up_n = up.normalize();
            for yaw in [-2.0f32, -0.5, 0.0, 1.0, 3.0] {
                let fwd = surface_forward(up, yaw, 0.0);
                assert!(
                    fwd.dot(up_n).abs() < 1e-5,
                    "pitch-0 forward not tangent: up={up} yaw={yaw} dot={}",
                    fwd.dot(up_n)
                );
            }
        }
    }

    #[test]
    fn looking_down_points_below_the_horizon_toward_center() {
        // Negative pitch => forward gains a -up component => points at the
        // ground/body centre. Positive pitch => toward the zenith.
        for up in [Vec3::Y, Vec3::new(3.0, 1.0, -2.0), Vec3::new(0.0, -1.0, 0.5)] {
            let up_n = up.normalize();
            let down = surface_forward(up, 0.7, -0.6);
            assert!(down.dot(up_n) < -0.4, "negative pitch did not look down: {}", down.dot(up_n));
            let upward = surface_forward(up, 0.7, 0.6);
            assert!(upward.dot(up_n) > 0.4, "positive pitch did not look up: {}", upward.dot(up_n));
        }
    }

    #[test]
    fn look_angles_reproduce_the_direction() {
        // surface_look_angles must invert surface_forward for the given up.
        let ups = [Vec3::Y, Vec3::new(1.0, 2.0, -3.0), Vec3::new(-4.0, 1.0, 0.5)];
        for up in ups {
            for (yaw, pitch) in [(0.0f32, 0.0f32), (1.2, -0.3), (-2.5, 0.8), (2.0, -1.4)] {
                let dir = surface_forward(up, yaw, pitch);
                let (ry, rp) = surface_look_angles(up, dir);
                let rebuilt = surface_forward(up, ry, rp);
                assert!(
                    approx(rebuilt, dir, 1e-4),
                    "look_angles missed: up={up} yaw={yaw} pitch={pitch} rebuilt={rebuilt} dir={dir}"
                );
            }
        }
    }

    #[test]
    fn world_look_angles_matches_world_forward() {
        // world_look_angles must invert the default camera forward formula.
        let world_forward = |yaw: f32, pitch: f32| {
            Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()).normalize()
        };
        for (yaw, pitch) in [(0.3f32, 0.2f32), (-1.7, -0.5), (2.9, 0.9)] {
            let dir = world_forward(yaw, pitch);
            let (ry, rp) = world_look_angles(dir);
            assert!(approx(world_forward(ry, rp), dir, 1e-4), "world inverse missed");
        }
    }
}

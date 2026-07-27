//! Planetary surface FPS math (task #76 increment 1) - pure helpers for the
//! surface-oriented camera and the stand/walk ground clamp.
//!
//! WHY: away from the ship the camera's "up" is world-Y, so at a tilted point
//! on the globe the horizon tilts and the player floats. On a planet surface
//! "down" must point at the body's CENTER (the gravity well). These helpers
//! build a TANGENT-frame camera basis whose up is the local radial (the
//! direction from the body centre to the camera): yaw then spins around the
//! radial and pitch tilts toward/away from the ground, so looking down points
//! at the planet centre and the horizon is level regardless of where on the
//! sphere you stand.
//!
//! Everything here is pure glam (no GPU, no winit, no cfg gate), so it
//! compiles in every feature set and is fully unit-tested headless. The main
//! loop (native `lib.rs`) and the camera (`renderer::camera`) call in; they
//! own the state (which body, current spin, ship_world_pos) while this module
//! owns the geometry.

use glam::Vec3;

/// Standing eye height above the ground (metres): the radial distance the
/// stand/walk clamp rests the camera at above the sampled ground radius.
pub const EYE_HEIGHT_M: f64 = 1.7;

/// Extra clearance between the eye and the MODELED ground (v0.889): the
/// clamp samples the full-detail elevation, but the patch actually DRAWN
/// under the player may be several LOD levels coarser, and its linear
/// interpolation can bulge above the fine model on ridges - the "seeing
/// through the Earth while standing on it" clip. This slop covers the
/// worst-case coarse-over-fine bulge at walking depths.
pub const LOD_CLEARANCE_M: f64 = 2.5;

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

/// Radial distance the camera rests at when standing: the sampled ground
/// radius plus one eye height.
/// NOTE v0.889: rest_radius also carries LOD_CLEARANCE_M via the shared
/// floor in clamp_above_ground; keep the two consistent.
pub fn rest_radius(ground_r: f64, eye_height: f64) -> f64 {
    ground_r + eye_height + LOD_CLEARANCE_M
}

/// Never sink below standing height: clamp a radial distance so the eye stays
/// at least `eye_height` above the ground radius.
pub fn clamp_above_ground(r: f64, ground_r: f64, eye_height: f64) -> f64 {
    r.max(ground_r + eye_height + LOD_CLEARANCE_M)
}

/// Ease a radial distance toward its rest height (gravity): exponential decay
/// at `rate` per second, clamped so the result is never below `rest` (you
/// settle onto the ground, you do not tunnel through it). If already at or
/// below rest, snap up to rest.
/// NOTE v0.1005: this is now only the GROUNDED fine-tune (breathing terrain
/// under a standing player). Airborne motion is real ballistics via
/// `vertical_step` below - the operator's "falls extremely fast... not
/// remotely like gravity" was this exponential snap being applied to
/// kilometre falls.
pub fn settle_radius(current: f64, rest: f64, dt: f64, rate: f64) -> f64 {
    if current <= rest {
        return rest;
    }
    let eased = rest + (current - rest) * (-rate * dt.max(0.0)).exp();
    eased.max(rest)
}

/// Human terminal velocity in a thick atmosphere (m/s, belly-to-earth).
/// The free-fall clamp `vertical_step` applies; powered descent (thrust
/// down) may exceed it deliberately.
pub const TERMINAL_FALL_MPS: f64 = 55.0;

/// How hard vertical thrust pulls the velocity toward its target, as a
/// multiple of the local gravity (with a floor so tiny bodies still feel
/// responsive). ~3g reaches a 50 m/s climb in under 2 s while still
/// reading as a spool-up rather than a teleport.
pub const THRUST_ACCEL_G_MULT: f64 = 3.0;
pub const THRUST_ACCEL_MIN: f64 = 20.0;

/// One step of real vertical ballistics (v0.1005, operator: "It's not
/// remotely like gravity. The up speed is super slow while the down speed
/// is always very fast").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalStep {
    pub r: f64,
    pub v_r: f64,
    /// Landed on (or resting at) standing height this step.
    pub grounded: bool,
}

/// Integrate the walk band's radial motion for one frame.
///
/// - `thrust_mps`: target vertical rate from input (+ = climb from Space,
///   - = powered descent from Shift, 0 = no vertical input). The velocity
///   RAMPS toward it at ~3g (a jetpack spool, not a teleport), and the
///   same ramp brakes a fall when you thrust against it.
/// - No input: gravity `g` accelerates the fall, clamped at `terminal`
///   (real free-fall: ~4.5 s and 55 m/s from a 100 m drop, minutes from
///   the band ceiling - not the old fixed-rate snap).
/// - Landing at `rest` zeroes the velocity; standing there stays put.
pub fn vertical_step(
    r: f64,
    v_r: f64,
    rest: f64,
    g: f64,
    dt: f64,
    thrust_mps: f64,
    terminal: f64,
) -> VerticalStep {
    let dt = dt.max(0.0);
    let g = g.max(0.0);
    let mut v = v_r;
    if thrust_mps.abs() > 1e-9 {
        // Powered: ramp toward the commanded rate from EITHER side (also
        // how an upward burn arrests a fall).
        let accel = (g * THRUST_ACCEL_G_MULT).max(THRUST_ACCEL_MIN);
        let dv = accel * dt;
        if v < thrust_mps {
            v = (v + dv).min(thrust_mps);
        } else {
            v = (v - dv).max(thrust_mps);
        }
    } else {
        // Free fall, terminal-velocity clamped. (An upward coast also
        // decays through zero into a fall - the ballistic arc.)
        v = (v - g * dt).max(-terminal.abs());
    }
    let mut r_new = r + v * dt;
    let mut grounded = false;
    if r_new <= rest {
        r_new = rest;
        // Ground stops downward motion; an upward thrust may lift off the
        // same frame it starts, so never clamp a positive velocity.
        if v < 0.0 {
            v = 0.0;
        }
        grounded = v <= 0.0;
    }
    VerticalStep { r: r_new, v_r: v, grounded }
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

    #[test]
    fn ground_clamp_never_sinks_below_standing_height() {
        let ground = 6.371e6;
        let eye = EYE_HEIGHT_M;
        let floor = ground + eye + LOD_CLEARANCE_M;
        // Below ground snaps up to standing height (+ LOD slop, v0.889).
        assert_eq!(clamp_above_ground(ground - 100.0, ground, eye), floor);
        // Above the floor is left alone.
        assert_eq!(clamp_above_ground(ground + 50.0, ground, eye), ground + 50.0);
        // Rest and clamp share the same floor so settle never fights the clamp.
        assert_eq!(rest_radius(ground, eye), floor);
    }

    #[test]
    fn free_fall_accelerates_at_g_and_caps_at_terminal() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let mut r = rest + 10_000.0;
        let mut v = 0.0;
        let dt = 1.0 / 60.0;
        // After 2 s of free fall: v ~ -g*t (well under terminal).
        for _ in 0..120 {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
        }
        assert!((v + 9.81 * 2.0).abs() < 0.5, "2 s fall should be ~ -19.6 m/s, got {v}");
        // Long fall: clamps at terminal, never faster.
        for _ in 0..1200 {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            assert!(v >= -TERMINAL_FALL_MPS - 1e-9, "fell past terminal: {v}");
        }
        assert!((v + TERMINAL_FALL_MPS).abs() < 1e-6, "did not reach terminal: {v}");
    }

    #[test]
    fn hundred_metre_drop_takes_real_seconds_not_a_snap() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let mut r = rest + 100.0;
        let mut v = 0.0;
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while r > rest {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            t += dt;
            assert!(t < 30.0, "fall never landed");
        }
        // sqrt(2h/g) = 4.52 s; discrete integration lands within half a second.
        assert!((4.0..5.0).contains(&t), "100 m drop took {t:.2} s, expected ~4.5 s");
    }

    #[test]
    fn thrust_ramps_to_target_and_release_goes_ballistic() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let mut r = rest;
        let mut v = 0.0;
        let dt = 1.0 / 60.0;
        // Hold Space (target 50 m/s): the ramp reaches the target in ~1.7 s
        // at 3g and NEVER exceeds it.
        let mut t = 0.0;
        while v < 50.0 - 1e-6 {
            let s = vertical_step(r, v, rest, 9.81, dt, 50.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            t += dt;
            assert!(v <= 50.0 + 1e-9, "overshot the commanded climb: {v}");
            assert!(t < 5.0, "ramp never reached the target");
        }
        assert!((1.0..3.0).contains(&t), "3g spool to 50 m/s took {t:.2} s");
        // Release: the coast decays through zero into a fall (ballistic arc),
        // and the peak sits above the release height.
        let release_r = r;
        let mut peak = r;
        for _ in 0..1200 {
            let s = vertical_step(r, v, rest, 9.81, dt, 0.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            peak = peak.max(r);
            if s.grounded {
                break;
            }
        }
        assert!(peak > release_r + 50.0, "no ballistic coast above release point");
        assert!((r - rest).abs() < 1e-6, "arc never landed");
        assert_eq!(v, 0.0, "landing must zero the velocity");
    }

    #[test]
    fn upward_burn_arrests_a_fall() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        // Falling at terminal from 2 km up: thrust up must brake the fall
        // long before the ground.
        let mut r = rest + 2_000.0;
        let mut v = -TERMINAL_FALL_MPS;
        let dt = 1.0 / 60.0;
        for _ in 0..600 {
            let s = vertical_step(r, v, rest, 9.81, dt, 10.0, TERMINAL_FALL_MPS);
            r = s.r;
            v = s.v_r;
            if v >= 10.0 - 1e-6 {
                break;
            }
        }
        assert!(v >= 10.0 - 1e-6, "burn never arrested the fall: v={v}");
        assert!(r > rest + 500.0, "braked too late: {r}");
    }

    #[test]
    fn standing_on_ground_stays_grounded_and_still() {
        let rest = 6.371e6 + EYE_HEIGHT_M + LOD_CLEARANCE_M;
        let s = vertical_step(rest, 0.0, rest, 9.81, 1.0 / 60.0, 0.0, TERMINAL_FALL_MPS);
        assert_eq!(s.r, rest);
        assert_eq!(s.v_r, 0.0);
        assert!(s.grounded);
        // Lifting off from the ground works the same frame thrust starts.
        let s2 = vertical_step(rest, 0.0, rest, 9.81, 1.0, 5.0, TERMINAL_FALL_MPS);
        assert!(s2.r > rest && s2.v_r > 0.0 && !s2.grounded);
    }

    #[test]
    fn settle_eases_down_and_clamps_to_rest() {
        let rest = 6.371e6 + EYE_HEIGHT_M;
        // From 50 m up, one step eases toward rest but never past it.
        let mut r = rest + 50.0;
        for _ in 0..600 {
            r = settle_radius(r, rest, 1.0 / 60.0, 4.0);
            assert!(r >= rest, "settle tunneled below the ground: {r} < {rest}");
        }
        // After ~10 s of easing at rate 4 it is essentially resting.
        assert!((r - rest) < 1e-3, "did not settle: {r} vs {rest}");
        // A camera already below ground snaps up to rest immediately.
        assert_eq!(settle_radius(rest - 500.0, rest, 0.016, 4.0), rest);
    }
}

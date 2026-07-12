//! Dev travel math (v0.791.x, Platform > Dev > Travel) - pure helpers for the
//! teleport tool that lets the operator inspect any solar body up close.
//!
//! Coordinate model recap (see the celestial pass in lib.rs): the local scene
//! (homestead, camera, creatures) lives in small f32 coordinates around the
//! world origin; `ship_world_pos` (f64, Earth-centred metres) says where that
//! local origin sits in the solar system, and every celestial body renders at
//! `rel_earth_m - ship_world_pos`. Planets are 5e10..4.5e12 m away - far beyond
//! what f32 camera coordinates can hold without catastrophic jitter (f32 ulp at
//! 1.5e11 is ~16 km) - so teleporting moves `ship_world_pos` (the mothership
//! jumps, carrying the player) while the camera stays at metre-scale local
//! coordinates. These helpers compute WHERE to park the ship and HOW to aim the
//! camera; they are pure math (glam only, no cfg gate) so the unit tests below
//! run in every feature set.

use glam::DVec3;

/// How many body radii away the teleport viewpoint parks. At 4 radii the disc
/// subtends ~2*asin(1/4) = 29 degrees - a big, unmistakable planet. Note the
/// celestial pass's min-angular-size clamp (`radius_m.max(dist * min_ang)`,
/// min_ang <= 0.045) is INACTIVE at this range: 4R * 0.045 < R, so the rendered
/// radius equals the true radius and parking at 4x the TRUE radius is exactly
/// 4x the VISUAL radius on arrival.
pub const VIEW_DISTANCE_RADII: f64 = 4.0;

/// Where to park the viewpoint (the new `ship_world_pos`) when teleporting to a
/// body. All positions are Earth-relative metres - the same frame the celestial
/// pass works in (`(helio - earth_helio) * M_PER_AU`).
///
/// The viewpoint sits `VIEW_DISTANCE_RADII * radius_m` from the body's centre,
/// offset TOWARD the Sun so the face you arrive looking at is the sunlit one
/// (arriving on the night side reads as "empty space with a black hole in the
/// stars"). For the Sun itself there is no lit side; approach from Earth's side
/// (Earth is the origin of this frame) so "toward home" is behind you.
pub fn teleport_viewpoint(
    body_rel_earth_m: DVec3,
    sun_rel_earth_m: DVec3,
    radius_m: f64,
    is_sun: bool,
) -> DVec3 {
    let dir = if is_sun {
        // From the Sun toward Earth (frame origin).
        (-sun_rel_earth_m).normalize_or_zero()
    } else {
        (sun_rel_earth_m - body_rel_earth_m).normalize_or_zero()
    };
    // Degenerate fallback (body exactly at the Sun, or zero vectors in a test
    // rig): park above the ecliptic instead of dividing by zero.
    let dir = if dir.length_squared() < 0.5 { DVec3::Y } else { dir };
    body_rel_earth_m + dir * (radius_m.max(1.0) * VIEW_DISTANCE_RADII)
}

/// Yaw/pitch that aim the engine camera along `dir` (any nonzero vector).
/// Matches `Camera::forward()`:
///   forward = (sin(yaw)*cos(pitch), sin(pitch), -cos(yaw)*cos(pitch))
/// so yaw = atan2(x, -z) and pitch = asin(y). Pitch is clamped just inside
/// +-90 degrees, the same limit mouse-look enforces.
pub fn look_angles(dir: DVec3) -> (f32, f32) {
    let d = dir.normalize_or_zero();
    if d.length_squared() < 0.5 {
        return (0.0, 0.0);
    }
    let max_pitch = std::f64::consts::FRAC_PI_2 - 0.01;
    let pitch = d.y.asin().clamp(-max_pitch, max_pitch);
    let yaw = d.x.atan2(-d.z);
    (yaw as f32, pitch as f32)
}

/// Planet visible spin rate (rad/s), matching the celestial pass's
/// `Quat::from_rotation_y(elapsed * 0.01)`. A full turn every ~10.5 min reads
/// as a lively day from orbit, but near the surface it is ~64 km/s of ground
/// motion at Earth's radius - hopeless to track by hand. `frame_lock_*` below
/// exists so the operator can sit in the body's rotating frame instead.
pub const PLANET_SPIN_RATE: f64 = 0.01;

/// Frame-lock capture (v0.819): express the camera's current Earth-relative
/// world position back in the body's UNROTATED local frame, so a later
/// `frame_lock_ship_pos` can re-place it as the body spins/orbits. Inverse of
/// `frame_lock_ship_pos`. `body_center_m` is the body's current Earth-relative
/// centre (DVec3::ZERO for Earth, the frame origin); `spin` is the body's
/// current rotation angle in radians (`elapsed * PLANET_SPIN_RATE`).
pub fn frame_lock_capture(body_center_m: DVec3, spin: f64, cam_world_m: DVec3) -> DVec3 {
    let rot = glam::DQuat::from_rotation_y(spin);
    rot.inverse() * (cam_world_m - body_center_m)
}

/// Frame-lock apply (v0.819): the `ship_world_pos` that keeps the camera parked
/// at the co-rotating anchor point, i.e. `body_center + Ry(spin) * anchor_local`
/// minus the camera's small local offset. Feeding successive `spin` values each
/// frame carries the local scene along with the planet's rotation AND orbital
/// motion, so the surface sits still relative to the viewer (KSP-style surface
/// reference frame). The matching view co-rotation is a yaw delta of
/// `spin - last_spin` applied by the caller (planet spins about Y, camera yaw is
/// about world Y, so they compose exactly in this Y-axis-spin model).
pub fn frame_lock_ship_pos(
    body_center_m: DVec3,
    spin: f64,
    anchor_local: DVec3,
    cam_local: DVec3,
) -> DVec3 {
    let rot = glam::DQuat::from_rotation_y(spin);
    body_center_m + rot * anchor_local - cam_local
}

/// Compact human label for the FTL speed multiplier ("x1", "x50", "x2k",
/// "x300M", "x1G"). Shown on the HUD and the Dev page while flying.
pub fn format_multiplier(mult: f32) -> String {
    let m = mult.max(1.0);
    if m < 1.0e3 {
        format!("x{m:.0}")
    } else if m < 1.0e6 {
        format!("x{:.0}k", m / 1.0e3)
    } else if m < 1.0e9 {
        format!("x{:.0}M", m / 1.0e6)
    } else {
        format!("x{:.0}G", m / 1.0e9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rebuild Camera::forward() from (yaw, pitch) in f64 to verify look_angles.
    fn forward_of(yaw: f32, pitch: f32) -> DVec3 {
        let (yaw, pitch) = (yaw as f64, pitch as f64);
        DVec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        )
    }

    #[test]
    fn viewpoint_at_mercury_scale_is_four_radii_out_on_the_sunlit_side() {
        // Mercury-like numbers, Earth-relative frame: Sun ~1.5e11 m away,
        // Mercury between us and the Sun (~1e11 m out), radius 2.44e6 m.
        let sun = DVec3::new(-1.496e11, 0.0, 0.0);
        let body = DVec3::new(-1.0e11, 2.0e9, 5.0e9);
        let r = 2.44e6;
        let vp = teleport_viewpoint(body, sun, r, false);

        // Exactly VIEW_DISTANCE_RADII radii from the centre (f64 keeps this
        // tight even at 1e11-scale coordinates).
        let dist = (vp - body).length();
        assert!(
            (dist - VIEW_DISTANCE_RADII * r).abs() < 1.0,
            "distance {dist} != {}",
            VIEW_DISTANCE_RADII * r
        );
        // On the sunlit side: the offset points toward the Sun.
        let to_sun = (sun - body).normalize();
        let off = (vp - body).normalize();
        assert!(off.dot(to_sun) > 0.999, "viewpoint not sunward: {}", off.dot(to_sun));
        // And safely outside the body.
        assert!(dist > r * 2.0);
    }

    #[test]
    fn viewpoint_at_neptune_scale_keeps_f64_precision() {
        // Neptune-like: 4.3e12 m out on a diagonal, radius 2.4622e7 m. The
        // 4-radii offset (~1e8 m) is 5 orders below the position magnitude -
        // exactly the regime that f32 would destroy (ulp at 4.3e12 is ~0.5e6 m)
        // and f64 must preserve (ulp ~1e-4 m).
        let sun = DVec3::new(-9.0e10, 1.0e9, -1.2e11);
        let body = DVec3::new(3.0e12, -1.0e11, -3.1e12);
        let r = 2.4622e7;
        let vp = teleport_viewpoint(body, sun, r, false);

        let dist = (vp - body).length();
        let want = VIEW_DISTANCE_RADII * r;
        assert!(
            (dist - want).abs() / want < 1.0e-9,
            "relative error too large at Neptune scale: dist={dist} want={want}"
        );
        let to_sun = (sun - body).normalize();
        assert!((vp - body).normalize().dot(to_sun) > 0.999);
    }

    #[test]
    fn sun_viewpoint_approaches_from_the_earth_side() {
        let sun = DVec3::new(-1.496e11, 0.0, 2.0e10);
        let r = 6.96e8;
        let vp = teleport_viewpoint(sun, sun, r, true);
        // Offset points from the Sun back toward Earth (the frame origin).
        let toward_earth = (-sun).normalize();
        let off = (vp - sun).normalize();
        assert!(off.dot(toward_earth) > 0.999);
        assert!(((vp - sun).length() - VIEW_DISTANCE_RADII * r).abs() < 1.0);
    }

    #[test]
    fn degenerate_direction_falls_back_instead_of_nan() {
        let vp = teleport_viewpoint(DVec3::ZERO, DVec3::ZERO, 1.0e6, false);
        assert!(vp.is_finite());
        assert!((vp.length() - VIEW_DISTANCE_RADII * 1.0e6).abs() < 1.0);
    }

    #[test]
    fn look_angles_reproduce_the_direction() {
        let dirs = [
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, -1.0),
            DVec3::new(-3.0, 2.0, 5.0),
            DVec3::new(0.2, -0.9, 0.1),
            DVec3::new(-1.0e12, 3.0e11, 2.0e12), // planetary-scale magnitudes
        ];
        for d in dirs {
            let (yaw, pitch) = look_angles(d);
            let rebuilt = forward_of(yaw, pitch);
            let want = d.normalize();
            assert!(
                rebuilt.dot(want) > 0.9999,
                "look_angles missed: dir={d:?} yaw={yaw} pitch={pitch} dot={}",
                rebuilt.dot(want)
            );
        }
    }

    #[test]
    fn straight_up_pitch_is_clamped_inside_the_mouse_look_limit() {
        let (_, pitch) = look_angles(DVec3::Y);
        assert!(pitch < std::f32::consts::FRAC_PI_2);
        assert!(pitch > 1.5);
    }

    #[test]
    fn frame_lock_capture_and_apply_round_trip() {
        // Capture a camera position in the body's frame, then re-apply at the
        // SAME spin: ship_world_pos + cam_local must reproduce the camera world
        // position exactly (the lock is an identity when time has not advanced).
        let body_center = DVec3::new(0.0, 0.0, 0.0); // Earth at origin
        let spin = 1.234;
        let cam_local = DVec3::new(5.0, 27.0, -3.0);
        let cam_world = DVec3::new(6.371e6, 1.2e5, -8.0e5);
        let anchor = frame_lock_capture(body_center, spin, cam_world);
        let ship = frame_lock_ship_pos(body_center, spin, anchor, cam_local);
        let reproduced = ship + cam_local;
        assert!((reproduced - cam_world).length() < 1e-3, "round trip drifted: {reproduced} vs {cam_world}");
    }

    #[test]
    fn frame_lock_holds_surface_still_as_the_planet_spins() {
        // A camera hovering over a fixed surface point: as spin advances, the
        // ship_world_pos must follow the point's rotation so the camera stays
        // above the SAME spot (its Earth-relative world position rotates with
        // the planet rather than the ground sliding out from under it).
        let body_center = DVec3::ZERO;
        let spin0 = 0.0;
        let cam_local = DVec3::ZERO; // camera == frame origin for this test
        // Camera hovering 100 km above a point on the equator (+X face).
        let cam_world0 = DVec3::new(6.471e6, 0.0, 0.0);
        let anchor = frame_lock_capture(body_center, spin0, cam_world0);
        // Advance a quarter turn.
        let spin1 = std::f64::consts::FRAC_PI_2;
        let ship1 = frame_lock_ship_pos(body_center, spin1, anchor, cam_local);
        let cam_world1 = ship1 + cam_local;
        // The camera should now sit above the same surface point after it
        // rotated 90 degrees about Y: +X maps to -Z (Ry convention).
        let expected = DVec3::new(0.0, 0.0, -6.471e6);
        assert!((cam_world1 - expected).length() < 1.0, "did not co-rotate: {cam_world1} vs {expected}");
        // Distance from centre preserved (still hovering, not falling in/out).
        assert!(((cam_world1 - body_center).length() - 6.471e6).abs() < 1.0);
    }

    #[test]
    fn frame_lock_view_yaw_keeps_pointing_at_the_body() {
        // Camera on +X looking at the origin: forward = -X => yaw = -pi/2.
        let c0 = DVec3::new(8.37e6, 0.0, 0.0);
        let yaw0 = -std::f32::consts::FRAC_PI_2;
        // Advance a quarter turn; the position co-rotates by Ry(+theta)...
        let theta = std::f64::consts::FRAC_PI_2;
        let c1 = glam::DQuat::from_rotation_y(theta) * c0;
        // ...and the tracking yaw SUBTRACTS the delta (the +/- sign that,
        // gotten wrong, made the planet slide across the frame).
        let yaw1 = yaw0 - theta as f32;
        let fwd = forward_of(yaw1, 0.0);
        let to_origin = (-c1).normalize();
        assert!(
            fwd.dot(to_origin) > 0.999,
            "view stopped tracking the body: fwd={fwd} to_origin={to_origin} dot={}",
            fwd.dot(to_origin)
        );
    }

    #[test]
    fn frame_lock_carries_the_frame_with_an_orbiting_body() {
        // For a NON-Earth body the centre itself moves; the lock must add that
        // translation too (so Mars does not drift away while you watch it).
        let center_a = DVec3::new(1.0e11, 2.0e9, -3.0e10);
        let spin = 0.5;
        let cam_local = DVec3::new(0.0, 20.0, 0.0);
        let cam_world = center_a + DVec3::new(0.0, 0.0, 1.0e7); // 10,000 km out
        let anchor = frame_lock_capture(center_a, spin, cam_world);
        // Body has drifted 500,000 km along its orbit; same spin.
        let center_b = center_a + DVec3::new(5.0e8, 0.0, 0.0);
        let ship = frame_lock_ship_pos(center_b, spin, anchor, cam_local);
        let cam_world_b = ship + cam_local;
        // Camera keeps the SAME offset relative to the (moved) body centre.
        let off_a = cam_world - center_a;
        let off_b = cam_world_b - center_b;
        assert!((off_a - off_b).length() < 1e-2, "offset not preserved across orbit: {off_a} vs {off_b}");
    }

    #[test]
    fn multiplier_labels_are_compact() {
        assert_eq!(format_multiplier(1.0), "x1");
        assert_eq!(format_multiplier(10.0), "x10");
        assert_eq!(format_multiplier(1.0e3), "x1k");
        assert_eq!(format_multiplier(2.5e4), "x25k");
        assert_eq!(format_multiplier(1.0e6), "x1M");
        assert_eq!(format_multiplier(1.0e9), "x1G");
        assert_eq!(format_multiplier(0.5), "x1"); // floor at 1x
    }
}

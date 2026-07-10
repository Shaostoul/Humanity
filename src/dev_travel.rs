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

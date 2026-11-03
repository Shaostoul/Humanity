//! From surface_move.rs's tests: the movement model's wish direction, from the renderer's camera
//! and its controller.

use humanity_systems::surface_move::*;
use crate::renderer::camera::Camera;
use crate::renderer::camera::CameraController;
use humanity_systems::surface_move::fixtures::DT;
use humanity_systems::surface_move::fixtures::G;
use humanity_systems::surface_move::fixtures::SETTLE;
use glam::DVec3;
use glam::Vec3;

/// A surface camera standing at `radial`, looking `pitch_deg` above the
/// horizon, with the given keys held. Returns the engine's own wish: the
/// SAME `surface_wish_dir` / `fly_wish_dir_up` the frame-lock calls, so
/// these gates test the shipped input path and not a restatement of it.
fn wish_for(radial: Vec3, pitch_deg: f32, keys: &[&str], fly: bool) -> DVec3 {
    let mut cam = Camera::new();
    cam.set_surface_up(radial);
    cam.yaw = 0.7;
    cam.pitch = pitch_deg.to_radians();
    let mut ctl = CameraController::new(5.0, 1.0);
    ctl.fly_mode = fly;
    for k in keys {
        ctl.process_key(k, true);
    }
    let w = if fly {
        ctl.fly_wish_dir_up(&cam, radial)
    } else {
        ctl.surface_wish_dir(&cam)
    };
    DVec3::new(w.x as f64, w.y as f64, w.z as f64)
}

/// THE GATE (operator: "While I press W I immediately start flying").
///
/// Pure lateral input must produce ZERO radial velocity change - not a
/// small one, zero - at every pitch, including the two that used to leak
/// hardest: nose down at your feet and nose up at the sky. Runs the real
/// keyboard -> wish -> split -> integrate chain for ten seconds and
/// demands the player never leaves the ground.
#[test]
fn walking_forward_never_lifts() {
    let radial = radial_at(35.0, 20.0);
    let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
    let rest = 6.371e6 + 1.75;
    for pitch in [-80.0_f32, -25.0, 0.0, 25.0, 80.0] {
        for keys in [&["KeyW"][..], &["KeyS"][..], &["KeyA"][..], &["KeyW", "KeyD"][..]] {
            let wish = wish_for(radial, pitch, keys, false);
            let (tangential, radial_wish) = split_wish(wish, up, MoveMode::Walk);
            assert_eq!(
                radial_wish, 0.0,
                "pitch {pitch} keys {keys:?}: lateral input produced a radial \
                 command of {radial_wish:e} - THIS is the lift, and an epsilon \
                 is not a fix for it"
            );
            assert!(
                tangential.length() > 0.5,
                "pitch {pitch} keys {keys:?}: the lateral part vanished ({:.3}); \
                 the fix must not cost you the ability to walk",
                tangential.length()
            );
            // Ten seconds of held input: never a positive velocity, never
            // a metre of altitude.
            let (mut r, mut v) = (rest, 0.0);
            for _ in 0..600 {
                let s = radial_step(
                    MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT,
                    radial_wish, 5.0 * DT,
                );
                r = s.r;
                v = s.v_r;
                assert!(v <= 0.0, "pitch {pitch} keys {keys:?}: gained climb speed {v}");
                assert!(
                    r <= rest + 1e-9,
                    "pitch {pitch} keys {keys:?}: rose {:.3} m off the ground",
                    r - rest
                );
                assert!(s.grounded, "a walker on flat ground must stay grounded");
            }
        }
    }
}

/// The other half of the same claim: releasing the key must not leave
/// MOMENTUM behind ("when I let go of W I maintain momentum briefly then
/// fall"). Ten seconds of W then ten seconds of nothing: the velocity is
/// zero at every single step, so there is nothing to coast on.
#[test]
fn releasing_forward_leaves_no_vertical_momentum() {
    let radial = radial_at(-35.0, 140.0);
    let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
    let rest = 6.371e6 + 1.75;
    let held = split_wish(wish_for(radial, 20.0, &["KeyW"], false), up, MoveMode::Walk).1;
    let released = split_wish(wish_for(radial, 20.0, &[], false), up, MoveMode::Walk).1;
    let (mut r, mut v) = (rest, 0.0);
    for frame in 0..1200 {
        let wish = if frame < 600 { held } else { released };
        let s = radial_step(
            MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT, wish, 5.0 * DT,
        );
        r = s.r;
        v = s.v_r;
        assert_eq!(v, 0.0, "frame {frame}: carried vertical momentum {v} m/s");
    }
    assert_eq!(r, rest, "the walker drifted off its rest height");
}

/// Do not over-fix: the jetpack must survive. Space is a DELIBERATE radial
/// command and arrives as exactly +1 (Shift as exactly -1); Space+Shift
/// cancel to exactly 0.
#[test]
fn deliberate_vertical_keys_still_command_exactly_one() {
    let radial = radial_at(0.0, 0.0);
    let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
    for (keys, want) in [
        (&["Space"][..], 1.0_f64),
        (&["ShiftLeft"][..], -1.0),
        (&["Space", "ShiftLeft"][..], 0.0),
        (&["KeyW", "Space"][..], 1.0),
    ] {
        let wish = wish_for(radial, -30.0, keys, false);
        let (_, radial_wish) = split_wish(wish, up, MoveMode::Walk);
        assert_eq!(radial_wish, want, "keys {keys:?} must command {want}");
    }
    // And it actually lifts: one second of Space at 5 m/s leaves the floor.
    let wish = split_wish(wish_for(radial, 0.0, &["Space"], false), up, MoveMode::Walk).1;
    let rest = 6.371e6 + 1.75;
    let (mut r, mut v) = (rest, 0.0);
    for _ in 0..60 {
        let s = radial_step(
            MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT, wish, 5.0 * DT,
        );
        r = s.r;
        v = s.v_r;
    }
    assert!(r > rest + 1.0, "Space no longer jumps (only {:.2} m up)", r - rest);
}

/// DEV FLIGHT HOLDS ITS ALTITUDE. Not "approximately", not "settles to":
/// the operator wants to park the camera 5,000 ft up and photograph
/// scenery, so a still stick must be a bit-identical radius forever.
/// 36,000 frames is ten minutes of hovering.
#[test]
fn dev_flight_holds_altitude_with_no_input() {
    let radial = radial_at(46.0, 8.0);
    let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
    let rest = 6.371e6 + 1.75;
    let start = rest + 1524.0; // 5,000 ft
    // The engine's own idle wish, through the real controller.
    let (_, radial_wish) = split_wish(wish_for(radial, -12.0, &[], true), up, MoveMode::DevFlight);
    assert_eq!(radial_wish, 0.0, "an untouched stick must be an exact zero");
    let mut r = start;
    let mut v = 0.0;
    for frame in 0..36_000 {
        let s = radial_step(
            MoveMode::DevFlight, true, false, r, v, rest, G, SETTLE, DT,
            radial_wish, 50.0,
        );
        r = s.r;
        v = s.v_r;
        assert_eq!(r, start, "frame {frame}: hover drifted to {r} (want {start})");
        assert_eq!(v, 0.0, "frame {frame}: hover picked up vertical speed");
        assert!(!s.grounded, "a hovering camera must not report footsteps");
    }
}

/// Dev flight keeps the look-direction model that makes it useful: nose up
/// 30 degrees and hold W to climb, nose down to descend. The gate is that
/// this is now confined to dev flight instead of being the walk model.
#[test]
fn dev_flight_flies_where_you_look() {
    let radial = radial_at(10.0, -75.0);
    let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
    let rest = 6.371e6 + 1.75;
    for (pitch, want_sign) in [(30.0_f32, 1.0_f64), (-30.0, -1.0)] {
        let (_, radial_wish) =
            split_wish(wish_for(radial, pitch, &["KeyW"], true), up, MoveMode::DevFlight);
        assert!(
            (radial_wish - want_sign * 0.5).abs() < 0.01,
            "pitch {pitch}: expected a {want_sign} half-rate climb, got {radial_wish}"
        );
        let s = radial_step(
            MoveMode::DevFlight, true, false, rest + 1000.0, 0.0, rest, G, SETTLE, DT,
            radial_wish, 10.0 * DT,
        );
        assert!(
            (s.r - (rest + 1000.0)) * want_sign > 0.0,
            "pitch {pitch}: W did not move the camera the way it was looking"
        );
    }
}

/// A real walking site: 35 N, 20 E on a 6,371 km sphere.
pub(crate) fn radial_at(lat_deg: f32, lon_deg: f32) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin()).normalize()
}

//! The Maps camera's tests (Maps increment 1, docs/design/maps-zoom-to-anything.md,
//! increment 1's Proof). Each was seen red first; what broke it and what it
//! said is in its comment.

use super::*;
use crate::station::orbit;

/// A feed for the shipped home station at game second `t` (the default
/// 24-hour day), placed exactly the way the frame loop places it
/// (`orbit::propagate` of `over_its_longitude`, `orbit::attitude`), shifted
/// by `origin` (to put the whole thing 1 AU out).
pub(crate) fn feed_at(t: f64, origin: DVec3) -> MapSelf {
    let def = crate::station::default_home();
    let day = crate::systems::time::EARTH_DAY_S;
    let (pos, vel) = orbit::propagate(
        &orbit::over_its_longitude(&def.orbit, 1.0),
        orbit::MU_EARTH,
        orbit::REAL_SECONDS_PER_DAY,
        orbit::sim_seconds(t, day),
        day,
    );
    let rot = orbit::attitude(&def.attitude, pos, vel);
    MapSelf {
        ship_id: "home".into(),
        ship_name: "UHS Mothership".into(),
        parent: "earth".into(),
        parent_mu: orbit::MU_EARTH,
        ship_pos_m: origin + pos,
        ship_vel_mps: vel,
        ship_rot: rot,
        ship_box_m: Some((DVec3::new(-60.0, -10.0, -40.0), DVec3::new(940.0, 90.0, 560.0))),
        earth_spin_rad: 0.0,
        sun_pos_m: DVec3::new(1.496e11, 0.0, 0.0),
        aboard: true,
        you_ship_m: DVec3::new(10.0, 1.7, 10.0),
        you_world_m: origin + pos + rot * DVec3::new(10.0, 1.7, 10.0),
        game_seconds: t,
        game_day_s: day,
        sky_seconds: 843_782_400.0,
        level: None,
        you_room: None,
        you_facing_ship: DVec3::NEG_Z,
        world_loaded: true,
    }
}

fn canvas() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(260.0, 60.0), egui::vec2(1400.0, 900.0))
}

/// How far the ship's centre is from the canvas centre, px; infinite when it
/// is behind the camera (it has left the view as surely as off the edge).
fn ship_offset_px(cam: &MapCamera, me: &MapSelf) -> f32 {
    let scene = MapScene::new(Some(me), me.sky_seconds, true);
    let proj = MapProjector::new(cam.pose(&scene), canvas());
    proj.project(me.ship_centre_m()).map_or(f32::INFINITY, |(p, _)| (p - canvas().center()).length())
}

/// Follow keeps the ship on the canvas centre within half a pixel across a
/// whole day of clock steps, 10 m from it, both at its real orbit and with
/// everything 1 AU out (the f64 stability the anchor exists for).
///
/// Seen red 2026-10-21 with `project` narrowing the point and the eye to f32
/// before subtracting them: "at its orbit, hour 0: the ship is 62.6 px off
/// centre" (f32 fails already 42,000 km out, never mind 1 AU).
#[test]
fn follow_keeps_the_ship_centred_for_a_day_at_10_m_even_1_au_out() {
    for (label, origin) in [("its orbit", DVec3::ZERO), ("1 AU + 10 m", DVec3::new(M_PER_AU, 0.0, 0.0))] {
        let mut cam = MapCamera::default();
        cam.open_on_me(Some(&feed_at(0.0, origin)));
        cam.set_distance(10.0);
        assert!(cam.follows_me(), "opening on you follows you");
        for hour in 0..=24 {
            let me = feed_at(hour as f64 * 3600.0, origin);
            let off = ship_offset_px(&cam, &me);
            assert!(off < 0.5, "at {label}, hour {hour}: the ship is {off:.1} px off centre");
        }
    }
}

/// The zoom reaches exactly 10 m and exactly 1e14 m, and no further.
///
/// Seen red 2026-10-21 with the floor put back to the old page's 0.001 AU
/// (149,600 km): "zoomed all the way in, the camera is 149597870.70000017 m
/// out". (A first version compared against `MIN_DISTANCE_M` and stayed green
/// with the floor moved: it moved with it. Hence the literals.)
#[test]
fn the_zoom_reaches_10_m_and_1e14_m() {
    let me = feed_at(0.0, DVec3::ZERO);
    let scene = MapScene::new(Some(&me), me.sky_seconds, true);
    let mut cam = MapCamera::default();
    cam.open_on_me(Some(&me));
    for _ in 0..200 {
        cam.zoom_by(0.5, &scene);
    }
    let d = cam.distance();
    // The literals, not the constants: a test against MIN_DISTANCE_M moves with it.
    assert!((d - 10.0).abs() < 1e-6, "zoomed all the way in, the camera is {d} m out");
    for _ in 0..400 {
        cam.zoom_by(2.0, &scene);
    }
    let d = cam.distance();
    assert!((d / 1.0e14 - 1.0).abs() < 1e-9, "zoomed all the way out, the camera is {d:e} m out");
    assert!(cam.follows_me(), "zooming never ends following");
}

/// A pan ends following (the ship then leaves the centre as the clock runs),
/// and "Back to me" flies back and follows again (the ship centred however
/// far the clock has run).
///
/// Seen red 2026-10-21 with `pan_by` not letting go of you first: "after a
/// pan the camera still follows you".
#[test]
fn a_pan_ends_follow_and_back_to_me_resumes_it() {
    let me0 = feed_at(0.0, DVec3::ZERO);
    let scene0 = MapScene::new(Some(&me0), me0.sky_seconds, true);
    let mut cam = MapCamera::default();
    cam.open_on_me(Some(&me0));
    cam.rotate_by(0.3, 0.1, &scene0);
    assert!(cam.follows_me(), "turning around the ship keeps following");
    cam.pan_by(40.0, 0.0, 900.0, &scene0);
    assert!(!cam.follows_me(), "after a pan the camera still follows you");
    // Two game hours on, the ship has gone 30 degrees round its orbit.
    let me2 = feed_at(7200.0, DVec3::ZERO);
    assert!(ship_offset_px(&cam, &me2) > 2000.0, "not following, the ship leaves the view as it moves");
    let scene2 = MapScene::new(Some(&me2), me2.sky_seconds, true);
    cam.back_to_me(&scene2);
    for _ in 0..400 {
        cam.tick(1.0 / 60.0);
    }
    assert!(cam.fly.is_none(), "the flight back lands");
    assert!(cam.follows_me(), "back to me follows you again");
    let off = ship_offset_px(&cam, &me2);
    assert!(off < 0.5, "back to me puts the ship on the centre: {off:.2} px off");
}

/// Stopping following and grabbing a flight both leave the view exactly
/// where it was (nothing on screen jumps when you take hold).
///
/// Seen red 2026-10-21 with `unfollow` resetting the frame to `Auto`: "the
/// ship moved 308.7 px when following stopped" (the view turned from the
/// ship's frame to the stars').
#[test]
fn letting_go_or_grabbing_moves_nothing_on_screen() {
    let me = feed_at(5400.0, DVec3::ZERO);
    let scene = MapScene::new(Some(&me), me.sky_seconds, true);
    let probe = me.ship_to_world(DVec3::new(900.0, 0.0, 500.0));
    let at = |cam: &MapCamera| MapProjector::new(cam.pose(&scene), canvas()).project(probe).unwrap().0;

    let mut cam = MapCamera::default();
    cam.open_on_me(Some(&me));
    let before = at(&cam);
    cam.unfollow(&scene);
    let moved = (at(&cam) - before).length();
    assert!(moved < 0.01, "the ship moved {moved:.1} px when following stopped");

    let mut cam = MapCamera::default();
    cam.open_on_me(Some(&me));
    cam.focus_body("moon", 2.0e7, &scene);
    for _ in 0..30 {
        cam.tick(1.0 / 60.0);
    }
    let mid = cam.pose(&scene);
    cam.grab(&scene);
    let after = cam.pose(&scene);
    let d = (after.eye - mid.eye).length() / mid.distance;
    let turn = after.forward.dot(mid.forward).clamp(-1.0, 1.0).acos();
    assert!(d < 1e-9 && turn < 1e-9, "grabbing a flight moved the eye {d:e} of the distance and turned it {turn:e} rad");
}

/// The fly-to path starts where the camera is and ends where it goes, and a
/// long flight zooms out on the way (the "zoom out, travel, zoom in" shape).
///
/// Seen red 2026-10-21 with the paper's `ln(-b + sqrt(b^2 + 1))` in place of
/// `-asinh(b)`: "starts at (NaN, NaN) not (0, 1000)" (the cancellation the
/// module doc names: for a 1e12 m flight from a 1 km view the logarithm's
/// argument rounds to zero).
#[test]
fn the_flight_path_starts_and_ends_in_the_right_places() {
    for (w0, w1, u1) in [(1_000.0, 1_000.0, 1.0e12), (1.0e6, 2.0e3, 4.2e7), (5.0e3, 5.0e9, 0.0), (2.0e3, 2.0e3, 3.0e4)] {
        let p = fly_path(w0, w1, u1);
        let (u_start, w_start) = p.at(0.0);
        let (u_end, w_end) = p.at(p.length);
        assert!(u_start.abs() < 1e-9 && (w_start / w0 - 1.0).abs() < 1e-9, "starts at ({u_start}, {w_start}) not (0, {w0})");
        assert!((u_end - 1.0).abs() < 1e-6 && (w_end / w1 - 1.0).abs() < 1e-6, "a {u1:e} m flight from a {w0} m view ends {u_end:.3} of the way across, {w_end:e} wide");
        if u1 > 1e3 * w0.max(w1) {
            let widest = (0..=100).map(|i| p.at(p.length * i as f64 / 100.0).1).fold(0.0, f64::max);
            assert!(widest > 10.0 * w0.max(w1), "a long flight zooms out on the way: widest {widest:e}");
        }
    }
}

/// At ship scale the camera turns with the ship, so Earth stays below the
/// ship on screen wherever the ship is in its orbit (decision 3); far out it
/// holds the stars, so its up is the world's.
///
/// Seen red 2026-10-21 with `ship_frame_weight` returning 0 (always the
/// stars' frame): "Earth is in view behind the ship" (the `expect` below:
/// turned to the stars, the opening view looks away from Earth altogether).
#[test]
fn the_ship_frame_at_ship_scale_the_stars_beyond() {
    for hour in [0.0, 6.0, 12.0, 18.0] {
        let me = feed_at(hour * 3600.0, DVec3::ZERO);
        let scene = MapScene::new(Some(&me), me.sky_seconds, true);
        let mut cam = MapCamera::default();
        cam.open_on_me(Some(&me));
        let proj = MapProjector::new(cam.pose(&scene), canvas());
        // Earth's centre is behind the ship; project its direction from the eye.
        let earth_dir = (DVec3::ZERO - cam.pose(&scene).eye).normalize() * 1.0e5 + cam.pose(&scene).eye;
        let (p, _) = proj.project(earth_dir).expect("Earth is in view behind the ship");
        let below = p.y - canvas().center().y;
        assert!(below > 0.0, "at hour {hour} Earth's centre is {:.1} px ABOVE the canvas centre", -below);
        let up = cam.pose(&scene).up;
        assert!(up.dot(me.ship_rot * DVec3::Y) > 0.3, "at ship scale the view's up leans to the ship's");
    }
    let me = feed_at(6.0 * 3600.0, DVec3::ZERO);
    let scene = MapScene::new(Some(&me), me.sky_seconds, true);
    let mut cam = MapCamera::default();
    cam.open_on_me(Some(&me));
    cam.set_distance(1.0e9);
    let fwd_world = cam.pose(&scene).forward;
    let expect = yaw_pitch_rot(cam.yaw, cam.pitch) * DVec3::NEG_Z;
    assert!(fwd_world.dot(expect) > 1.0 - 1e-12, "far out the yaw and pitch are the stars' own");
}

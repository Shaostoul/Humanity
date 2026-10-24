//! The Maps feed's tests (Maps increments 1 and 3; moved out of map_feed.rs in increment 3 to keep
//! its line budget). Each was seen red first; what broke it and what it said is in its comment.

use super::*;
use crate::gui::MapVerb;
use crate::station::orbit;

fn inputs(pos: DVec3, vel: DVec3, rot: DQuat) -> FeedInputs<'static> {
    FeedInputs {
        station_id: "home",
        station_name: "UHS Mothership",
        parent: "earth",
        station_pos: pos,
        station_vel: vel,
        station_rot: rot,
        aboard_bounds: Some((Vec3::new(-50.0, -10.0, -50.0), Vec3::new(950.0, 90.0, 550.0))),
        spin: 1.25,
        sun: DVec3::new(1.4e11, 0.0, -5.0e10),
        riding: true,
        cam_local: Vec3::new(12.0, 1.7, -3.0),
        cam_forward: Vec3::new(0.0, 0.0, -1.0),
        ship_world_pos: pos,
        game_seconds: 3600.0,
        game_day_s: 86_400.0,
        sky_seconds: 843_782_400.0,
        world_loaded: true,
        level: Some(1),
        room: Some(("room-bedroom".into(), "Bedroom".into())),
    }
}

/// The feed's ship pose EQUALS the pose the frame computed, exactly,
/// and is a copy: handed a pose no propagation of the station's orbit
/// would give (moved 1 km and turned 3 degrees off it), the feed reports
/// that pose, not the propagated one. A feed that recomputed the orbit
/// itself would pass the first check and fail the second.
///
/// Seen red 2026-10-21 with `build` propagating the shipped orbit at the
/// game clock instead of copying `station_pos`: "the feed recomputed the
/// ship: it is 1000.000 m from the pose the frame computed".
#[test]
fn the_feed_copies_the_frames_pose_and_never_recomputes_it() {
    let def = crate::station::default_home();
    let (pos, vel) = orbit::propagate(
        &orbit::over_its_longitude(&def.orbit, 2.0),
        orbit::MU_EARTH,
        orbit::REAL_SECONDS_PER_DAY,
        3600.0,
        86_400.0,
    );
    let rot = orbit::attitude(&def.attitude, pos, vel);
    let me = build(&inputs(pos, vel, rot));
    assert_eq!(me.ship_pos_m, pos, "the ship's position is the frame's, bit for bit");
    assert_eq!(me.ship_vel_mps, vel);
    assert_eq!(me.ship_rot, rot, "the ship's attitude is the frame's, bit for bit");
    assert_eq!((me.earth_spin_rad, me.sun_pos_m), (1.25, DVec3::new(1.4e11, 0.0, -5.0e10)), "Earth's spin and the Sun are the engine's");

    let off_pos = pos + DVec3::new(1000.0, 0.0, 0.0);
    let off_rot = rot * DQuat::from_rotation_y(3.0_f64.to_radians());
    let me = build(&inputs(off_pos, vel, off_rot));
    let miss = me.ship_pos_m.distance(off_pos);
    assert!(miss == 0.0, "the feed recomputed the ship: it is {miss:.3} m from the pose the frame computed");
    assert_eq!(me.ship_rot, off_rot);
}

/// You are where the camera is: aboard, the camera's home-frame position
/// IS your place in the ship's own frame; in the world it is that turned
/// by the ship's attitude.
///
/// Seen red 2026-10-21 with you left in the render frame's axes (no
/// `render_to_world_rot`): "aboard, you are at the camera's home-frame
/// spot: DVec3(7.987384126781649, -3.1072163739792904, 9.079476933096764)".
#[test]
fn you_are_where_the_camera_is() {
    let rot = DQuat::from_rotation_z(0.4) * DQuat::from_rotation_y(1.1);
    let pos = DVec3::new(3.0e7, 0.0, -2.9e7);
    let me = build(&inputs(pos, DVec3::new(2000.0, 0.0, 2100.0), rot));
    assert!((me.you_ship_m - DVec3::new(12.0, 1.7, -3.0)).length() < 1e-6, "aboard, you are at the camera's home-frame spot: {:?}", me.you_ship_m);
    let off = (me.you_world_m - (pos + rot * DVec3::new(12.0, 1.7, -3.0))).length();
    assert!(off < 1e-6, "in the world you are {off:.1} m from where the ship turns your spot aboard");
}

/// You face where the camera looks, in the ship's own frame (Maps increment 3: the map draws
/// your facing): aboard the camera's look is already in the ship's axes; off the ship it is in
/// the world's, and the feed turns it into the ship's. Your level and your room are copied.
///
/// Seen red 2026-10-22 with the look copied without the turn into the ship's axes: "off the
/// ship you face DVec3(0.0, 0.0, -1.0) in the ship's frame, want
/// DVec3(0.7173560908995228, 0.0, -0.6967067093471655)".
#[test]
fn you_face_where_the_camera_looks() {
    let rot = DQuat::from_rotation_y(0.8);
    let pos = DVec3::new(3.0e7, 0.0, -2.9e7);
    let me = build(&inputs(pos, DVec3::ZERO, rot));
    assert!((me.you_facing_ship - DVec3::NEG_Z).length() < 1e-6, "aboard you face {:?} in the ship's frame, want -Z", me.you_facing_ship);
    assert_eq!((me.level, me.you_room.as_ref().map(|r| r.1.as_str())), (Some(1), Some("Bedroom")));
    let mut off = inputs(pos, DVec3::ZERO, rot);
    off.riding = false;
    let me = build(&off);
    let want = rot.inverse() * DVec3::NEG_Z;
    assert!((me.you_facing_ship - want).length() < 1e-6, "off the ship you face {:?} in the ship's frame, want {want:?}", me.you_facing_ship);
}

/// Opening the page from the game frames your ship and follows it, on the
/// live clock; with "Open where I left it" it stays where it was. A rig
/// request that opens the page and stops following in one go is applied
/// AFTER the opening framing, so it is not undone.
///
/// Seen red 2026-10-21 with the verbs applied before the page-open
/// handling: "the request to stop following was undone by the opening".
#[test]
fn opening_the_page_frames_you_and_requests_come_after() {
    let def = crate::station::default_home();
    let (pos, vel) = orbit::propagate(&def.orbit, orbit::MU_EARTH, orbit::REAL_SECONDS_PER_DAY, 0.0, 86_400.0);
    let rot = orbit::attitude(&def.attitude, pos, vel);
    let mut maps = MapsState::default();
    maps.live = false;
    frame_update(&mut maps, build(&inputs(pos, vel, rot)), false, false);
    assert!(!maps.camera.follows_me(), "closed, nothing changes");
    frame_update(&mut maps, build(&inputs(pos, vel, rot)), true, false);
    assert!(maps.camera.follows_me() && maps.live, "opening the page follows you on the live clock");
    assert_eq!(maps.sim_time_seconds, 843_782_400.0, "live, the page's clock is the engine's sky clock");

    let mut left = MapsState::default();
    frame_update(&mut left, build(&inputs(pos, vel, rot)), true, true);
    assert!(!left.camera.follows_me(), "open where I left it keeps the camera where it was");

    let mut maps = MapsState::default();
    maps.pending.push(MapVerb { home: true, follow: Some(false), ..Default::default() });
    frame_update(&mut maps, build(&inputs(pos, vel, rot)), true, false);
    assert!(!maps.camera.follows_me(), "the request to stop following was undone by the opening");
    assert!(maps.pending.is_empty());
}

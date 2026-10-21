//! The engine tells the Maps page where you are (Maps increment 1,
//! docs/design/maps-zoom-to-anything.md, "The engine says where you are").
//!
//! The page receives `GuiState` only, never `EngineState`, so until this
//! module it could not see where the ship or the player was: it drew "You ·
//! Home" at Earth's centre. Now, once a frame, right after the station block
//! in lib.rs has placed the ship, `publish` copies what that block and the
//! frame before it already computed into `GuiState::maps.me` (`MapSelf`): the
//! ship's pose, its velocity, Earth's spin, the Sun, whether you are aboard
//! and where, and the clocks.
//!
//! ONE SOURCE. The feed copies; it never calls `orbit::propagate` itself.
//! Two callers of the orbit maths drift apart (the v0.881 and BUG-090 history
//! in lib.rs's station block is the warning), and a map whose ship sat a few
//! degrees from the game's would show it hanging over the wrong place. The
//! test `the_feed_copies_the_frames_pose_and_never_recomputes_it` holds this
//! by handing the feed a pose no propagation would give.
//!
//! The same call also does what must happen before the page draws and before
//! the engine renders the page's world layer (`engine/map_view.rs`), so both
//! see one camera: the page-open detector (opening the page from the game
//! frames your ship and follows it, decision 2), the Live clock (the page's
//! clock is the engine's own sky clock while live), and the rig's queued
//! requests (`MapVerb`, applied after the opening framing so a request that
//! opens the page and stops following is not undone by it).
//!
//! Cost: a copy of values already computed, under 0.01 ms (`cpu.maps_feed`).

use glam::{DQuat, DVec3, Vec3};

use crate::engine::state::EngineState;
use crate::gui::pages::maps::camera::{MapScene, MAX_DISTANCE_M, MIN_DISTANCE_M};
use crate::gui::pages::maps::MapView;
use crate::gui::{MapSelf, MapVerb, MapsState};
use crate::gui::GuiPage;

/// What the frame already computed, as the feed reads it. A plain struct so
/// the copy is tested without an engine.
pub(crate) struct FeedInputs<'a> {
    pub station_id: &'a str,
    pub station_name: &'a str,
    pub parent: &'a str,
    /// `station_world_pos`, the velocity the station block propagated with
    /// it, and `station_world_rot`.
    pub station_pos: DVec3,
    pub station_vel: DVec3,
    pub station_rot: DQuat,
    pub aboard_bounds: Option<(Vec3, Vec3)>,
    pub spin: f64,
    pub sun: DVec3,
    pub riding: bool,
    /// `camera.position` (render frame) and `ship_world_pos` (its origin).
    pub cam_local: Vec3,
    pub ship_world_pos: DVec3,
    pub game_seconds: f64,
    pub game_day_s: f64,
    pub sky_seconds: f64,
    pub world_loaded: bool,
}

/// Build the feed from what the frame computed. Positions are copied, never
/// recomputed; "you" is the camera's render-frame position taken back to the
/// world (`station::render_to_world_rot`) and into the ship's own axes.
pub(crate) fn build(i: &FeedInputs) -> MapSelf {
    let to_world = crate::station::render_to_world_rot(i.riding, i.station_rot);
    let cam = DVec3::new(i.cam_local.x as f64, i.cam_local.y as f64, i.cam_local.z as f64);
    let you_world = i.ship_world_pos + to_world * cam;
    MapSelf {
        ship_id: i.station_id.to_string(),
        ship_name: i.station_name.to_string(),
        parent: i.parent.to_string(),
        parent_mu: crate::gui::pages::maps::parent_constants(i.parent).0,
        ship_pos_m: i.station_pos,
        ship_vel_mps: i.station_vel,
        ship_rot: i.station_rot,
        ship_box_m: i.aboard_bounds.map(|(lo, hi)| (lo.as_dvec3(), hi.as_dvec3())),
        earth_spin_rad: i.spin,
        sun_pos_m: i.sun,
        aboard: i.riding,
        you_ship_m: i.station_rot.inverse() * (you_world - i.station_pos),
        you_world_m: you_world,
        game_seconds: i.game_seconds,
        game_day_s: i.game_day_s,
        sky_seconds: i.sky_seconds,
        level: None,
        world_loaded: i.world_loaded,
    }
}

/// Once a frame, from lib.rs's station block, after it has placed the ship
/// (`vel` is the velocity it propagated with the pose).
pub(crate) fn publish(state: &mut EngineState, vel: DVec3) {
    let _cost = crate::renderer::frame_costs::stage("cpu.maps_feed");
    let (game_seconds, game_day_s) = state
        .data_store
        .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|g| (g.elapsed_seconds, g.seconds_per_day())))
        .unwrap_or((0.0, crate::systems::time::EARTH_DAY_S));
    let me = build(&FeedInputs {
        station_id: &state.station_def.id,
        station_name: &state.station_def.name,
        parent: &state.station_def.parent,
        station_pos: state.station_world_pos,
        station_vel: vel,
        station_rot: state.station_world_rot,
        aboard_bounds: state.aboard_bounds,
        spin: state.current_spin,
        sun: state.sun_world_pos,
        riding: state.station_ride,
        cam_local: state.camera.position,
        ship_world_pos: state.ship_world_pos,
        game_seconds,
        game_day_s,
        sky_seconds: crate::gui::pages::maps::current_real_time_seconds_since_j2000(),
        world_loaded: state.world_loaded,
    });
    let open = state.gui_state.active_page == GuiPage::Maps;
    let where_left = state.gui_state.settings.maps_open_where_left;
    frame_update(&mut state.gui_state.maps, me, open, where_left);
}

/// The page-side half of `publish`, pure over `MapsState`: count the frame,
/// open on you, keep Live on the engine's clock, apply the rig's requests.
pub(crate) fn frame_update(maps: &mut MapsState, me: MapSelf, open: bool, open_where_left: bool) {
    maps.frame_seq += 1;
    let opened = open && !maps.was_open;
    maps.was_open = open;
    if opened && !open_where_left {
        maps.view = MapView::System;
        maps.camera.open_on_me(Some(&me));
        maps.live = true;
        maps.sim_speed = 0.0;
        maps.tracked_body = None;
        // The rig's dev switches never outlive an opening of the page.
        maps.turn_dps = 0.0;
        maps.draw_ship = true;
    }
    if maps.live {
        maps.sim_time_seconds = me.sky_seconds;
        maps.sim_time_initialized = true;
    }
    maps.me = Some(me);
    for verb in std::mem::take(&mut maps.pending) {
        apply_verb(maps, &verb);
    }
}

/// One rig request (`MapVerb`), applied to the page's state.
pub(crate) fn apply_verb(maps: &mut MapsState, verb: &MapVerb) {
    let scene_me = maps.me.clone();
    let scene = MapScene::new(scene_me.as_ref(), maps.sim_time_seconds, maps.live);
    if verb.home {
        // A fresh start on you, the dev switches included: the page can stay
        // open from an earlier request (a rig vantage), so the reset on
        // opening never ran (the turning vantage's 20 degrees a second turned
        // the next vantage's camera between its two captures, 2026-10-21).
        maps.view = MapView::System;
        maps.live = true;
        maps.turn_dps = 0.0;
        maps.draw_ship = true;
        maps.camera.open_on_me(scene_me.as_ref());
    }
    match verb.follow {
        Some(true) if !maps.camera.follows_me() => {
            let (d, yaw, pitch) = (maps.camera.distance(), maps.camera.yaw, maps.camera.pitch);
            maps.camera.open_on_me(scene_me.as_ref());
            maps.camera.set_distance(d);
            maps.camera.yaw = yaw;
            maps.camera.pitch = pitch;
        }
        Some(false) => maps.camera.unfollow(&scene),
        _ => {}
    }
    if let Some(d) = verb.distance_m {
        maps.camera.set_distance(d.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M));
    }
    if let Some(ship) = verb.ship {
        maps.draw_ship = ship;
    }
    if verb.report {
        maps.report_after_seq = Some(maps.frame_seq);
    }
    if let Some(t) = verb.turn_dps {
        maps.turn_dps = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            ship_world_pos: pos,
            game_seconds: 3600.0,
            game_day_s: 86_400.0,
            sky_seconds: 843_782_400.0,
            world_loaded: true,
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
}

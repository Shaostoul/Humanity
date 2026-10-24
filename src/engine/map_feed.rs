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
//! Maps increment 3 adds, while the page is open and you are aboard, which
//! LEVEL of the ship you are on (ship/levels.rs `level_at` of your feet, the
//! rule the build editor finds your storey by), the room the HUD's room card
//! names (`hud::room_card_at`, the HUD's own pick), and which way you face, so
//! the map and the game never disagree about where you are aboard.
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
// (The rig's requests are applied by `map_rig::apply_verb`, Maps increment 2.)
use crate::gui::pages::maps::MapView;
use crate::gui::{MapSelf, MapsState};
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
    /// `camera.position` (render frame) and `ship_world_pos` (its origin),
    /// and the camera's look (render frame).
    pub cam_local: Vec3,
    pub cam_forward: Vec3,
    pub ship_world_pos: DVec3,
    pub game_seconds: f64,
    pub game_day_s: f64,
    pub sky_seconds: f64,
    pub world_loaded: bool,
    /// Maps increment 3: your level's ordinal and the HUD's room (id, name),
    /// worked out by `publish` only while the page is open.
    pub level: Option<i32>,
    pub room: Option<(String, String)>,
}

/// Build the feed from what the frame computed. Positions are copied, never
/// recomputed; "you" is the camera's render-frame position taken back to the
/// world (`station::render_to_world_rot`) and into the ship's own axes.
pub(crate) fn build(i: &FeedInputs) -> MapSelf {
    let to_world = crate::station::render_to_world_rot(i.riding, i.station_rot);
    let cam = DVec3::new(i.cam_local.x as f64, i.cam_local.y as f64, i.cam_local.z as f64);
    let you_world = i.ship_world_pos + to_world * cam;
    let facing = i.station_rot.inverse() * (to_world * i.cam_forward.as_dvec3());
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
        level: i.level,
        you_room: i.room.clone(),
        you_facing_ship: facing.normalize_or_zero(),
        world_loaded: i.world_loaded,
    }
}

/// Your level and the HUD's room while the page is open and you are aboard
/// (Maps increment 3): the level your FEET are on (`levels::level_at`, the
/// rule the build editor seeds its avatar's storey by, `avatar_seed`) and
/// the room `hud::room_card_at` names for your eye, the HUD's own pick.
fn where_aboard(state: &EngineState) -> (Option<i32>, Option<(String, String)>) {
    let g = &state.gui_state;
    let Some(ship) = g.ship_structure.as_ref().filter(|_| state.station_ride && g.active_page == GuiPage::Maps) else {
        return (None, None);
    };
    let eye = state.camera.position;
    let levels = crate::ship::levels::ship_levels(ship);
    let level = crate::ship::levels::level_at(&levels, eye.y - state.controller.eye_height()).map(|i| levels[i].ordinal);
    let room = crate::gui::pages::hud::room_card_at(&g.room_bounds, Some(ship), &g.keybinds, eye).map(|r| (r.id.clone(), r.display_name.clone()));
    (level, room)
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
    let (level, room) = where_aboard(state);
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
        cam_forward: state.camera.forward(),
        ship_world_pos: state.ship_world_pos,
        game_seconds,
        game_day_s,
        sky_seconds: crate::gui::pages::maps::current_real_time_seconds_since_j2000(),
        world_loaded: state.world_loaded,
        level,
        room,
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
        // The page opens on the whole ship (Maps increment 3): a level is picked.
        maps.level_pick = None;
        maps.plan = false;
    }
    if maps.live {
        maps.sim_time_seconds = me.sky_seconds;
        maps.sim_time_initialized = true;
    }
    maps.me = Some(me);
    for verb in std::mem::take(&mut maps.pending) {
        crate::engine::map_rig::apply_verb(maps, &verb);
    }
}

#[cfg(test)]
#[path = "map_feed_tests.rs"]
mod tests;

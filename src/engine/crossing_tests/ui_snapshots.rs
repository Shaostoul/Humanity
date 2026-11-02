//! From gui/ui_snapshots.rs: the snapshots and toasts that set a page up the way the engine does
//! (the guest notice, `home_plot::guest_arrival`; the Maps feed, `map_feed::frame_update`). They
//! render through the GUI's snapshot harness (`render_page_png`); `just snapshot <name>` finds them
//! here too (scripts/snapshot-name.js).

use crate::gui::theme::load_theme;
use crate::gui::GuiState;
use crate::gui::ui_snapshots::render_page_png;
use crate::gui::ui_snapshots::snapshot_ctx;

/// The guest notice as a Cabin player reads it on a ship whose cabins are taken and whose
/// homesteads are not (engine/home_plot.rs `guest_arrival`, ship homes increment 6).
fn guest_arrival_example() -> String {
    let welcome = serde_json::json!({ "home_full": "cabin", "home_free": { "homestead": 3 } });
    crate::engine::home_plot::guest_arrival(&welcome, crate::gui::pages::where_you_live::starts())
}

/// A long notice stays on the window (first-hour audit 2026-10-04). A toast's text never
/// wrapped, so a notice longer than the window ran off both edges and could not be read: the
/// guest notice, and before it the shared world's refusal sentences (engine/home_plot.rs
/// `refuse_shared_world`). Each toast now wraps inside the window and stands above the one
/// below by its real height. GPU-free: the toasts' laid-out rects, from egui's memory.
///
/// Seen red 2026-10-04 on 8e400d7ed (no wrap, a fixed 40 point step): "toast 0 runs off the
/// window: [[-520.2 269.0] - [1800.2 312.0]] on [[0.0 0.0] - [1280.0 360.0]]".
#[test]
fn a_long_notice_wraps_inside_the_window() {
    let ctx = snapshot_ctx();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = GuiState::default();
    state.pending_notices.push(guest_arrival_example());
    state.pending_notices.push(crate::gui::first_steps::controls_hint(&state.keybinds));
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 360.0));
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        // The frame's shapes are not what this test reads (its state and egui's memory are).
        let _ = ctx.run(input, |ctx| crate::gui::widgets::draw_toasts(ctx, &theme, &mut state));
    }
    let rects: Vec<egui::Rect> = (0..2)
        .map(|i| ctx.memory(|m| m.area_rect(egui::Id::new(("hos_toast", i)))).expect("the toast was laid out"))
        .collect();
    for (i, r) in rects.iter().enumerate() {
        assert!(screen.contains_rect(*r), "toast {i} runs off the window: {r:?} on {screen:?}");
    }
    assert!(!rects[0].intersects(rects[1]), "the toasts overlap: {rects:?}");
    assert!(rects[0].height() > rects[1].height() + 10.0, "the long notice is still one line: {rects:?}");
}

/// The two notices a new player's first session can show (first-hour audit 2026-10-04): the
/// controls hint on the first entry into the world (gui/first_steps.rs) and, on a shared
/// server with no plot of the asked kind free, the guest notice (engine/home_plot.rs
/// `guest_arrival`). Both ride the long-lived notice toast; this shows how they read on a 1280 px
/// window.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_first_steps_notices() {
    render_page_png("first_steps_notices", 1280, 360, |ctx, theme, state| {
        // First frame only, as `snapshot_toast` does: the closure runs once per frame.
        if state.toasts.is_empty() {
            let now = ctx.input(|i| i.time);
            state.notice(&guest_arrival_example(), now);
            state.notice(crate::gui::first_steps::controls_hint(&state.keybinds), now);
        }
        crate::gui::widgets::draw_toasts(ctx, theme, state);
    });
}

/// Maps increment 1: the page as opening it from the game leaves it, with a
/// fixed feed (the shipped home at game hour 1, 2026-09-27 12:00 UTC on the
/// sky clock) and no 3D world (a snapshot has no engine): following you, the
/// UHS Mothership's outline and icon at the centre on its synchronous ring,
/// labelled with its real height, the page saying why the ship is not drawn
/// for real, and no "400 km LEO" anywhere.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_maps_follow_home() {
    crate::gui::pages::maps::set_clock_for_snapshot(Some(843_782_400.0));
    render_page_png("maps_follow_home", 1280, 900, |ctx, theme, state| {
        if state.maps.me.is_none() {
            let mut me = crate::gui::pages::maps::camera::tests::feed_at(3600.0, glam::DVec3::ZERO);
            me.world_loaded = false;
            crate::engine::map_feed::frame_update(&mut state.maps, me, true, false);
        }
        crate::gui::pages::maps::draw(ctx, theme, state);
    });
    crate::gui::pages::maps::set_clock_for_snapshot(None);
}

/// Maps increment 2 in near space: the camera on Earth 1.1 million km out with
/// the ship's feed (as `maps_follow_home`), the reference orbits and the
/// Lagrange points switched on: Earth and the Moon as phase-lit discs of their
/// map colours, the Moon's orbit trail, the ship's synchronous ring and name,
/// GEO and the rest round Earth, Earth-Moon L1 to L5.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_maps_earth_moon() {
    crate::gui::pages::maps::set_clock_for_snapshot(Some(843_782_400.0));
    render_page_png("maps_earth_moon", 1280, 900, |ctx, theme, state| {
        if state.maps.me.is_none() {
            let mut me = crate::gui::pages::maps::camera::tests::feed_at(3600.0, glam::DVec3::ZERO);
            me.world_loaded = false;
            crate::engine::map_feed::frame_update(&mut state.maps, me, true, false);
            let mut cam = crate::gui::pages::maps::MapCamera::default();
            cam.anchor = crate::gui::pages::maps::camera::MapAnchor::Body("earth".into());
            cam.set_distance(1.1e9);
            cam.pitch = -50.0_f64.to_radians();
            cam.yaw = 0.6;
            state.maps.camera = cam;
            state.maps.layers_on.insert("reference_orbits".into(), true);
            state.maps.layers_on.insert("lagrange".into(), true);
        }
        crate::gui::pages::maps::draw(ctx, theme, state);
    });
    crate::gui::pages::maps::set_clock_for_snapshot(None);
}

/// Maps increment 3: you in the homestead's upstairs Bedroom on the shipped ship, the page
/// showing the level you are on (Storey 2) from 70 m, no engine (so the walls are drawn from the
/// data as lines, the world layer being the game's): the level picker with Storey 2 shown and
/// marked yours, the upstairs rooms filled in their room types' colours and named as the HUD
/// names them (the Bedroom among them), never the Dining Room under the Bedroom, and your dot
/// with the way you face.
fn ship_view_state(state: &mut crate::gui::GuiState, plan: bool) {
    use crate::gui::pages::maps::camera::{MapAnchor, MapFrame};
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship");
    state.room_bounds = crate::ship::room_types::RoomTypeRegistry::load(&data).room_bounds(&ship.generate_meshes().room_info);
    let mut me = crate::gui::pages::maps::camera::tests::feed_at(3600.0, glam::DVec3::ZERO);
    me.world_loaded = false;
    me.ship_box_m = ship.aboard_bounds().map(|(lo, hi)| (lo.as_dvec3(), hi.as_dvec3()));
    me.you_ship_m = glam::DVec3::new(38.0, 4.7, 34.0);
    me.you_world_m = me.ship_to_world(me.you_ship_m);
    me.you_facing_ship = glam::DVec3::new(0.6, 0.0, -0.8);
    me.level = Some(1);
    me.you_room = Some(("room-bedroom".into(), "Bedroom".into()));
    state.ship_structure = Some(ship);
    crate::engine::map_feed::frame_update(&mut state.maps, me, true, false);
    state.maps.level_pick = Some(1);
    state.maps.plan = plan;
    let cam = &mut state.maps.camera;
    cam.anchor = MapAnchor::You;
    cam.frame = MapFrame::Auto;
    cam.fly = None;
    cam.set_distance(if plan { 60.0 } else { 70.0 });
    cam.yaw = if plan { 0.0 } else { 0.35 };
    cam.pitch = if plan { -crate::gui::pages::maps::camera::PITCH_LIMIT_RAD } else { -55.0_f64.to_radians() };
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_maps_ship_upstairs() {
    crate::gui::pages::maps::set_clock_for_snapshot(Some(843_782_400.0));
    render_page_png("maps_ship_upstairs", 1280, 900, |ctx, theme, state| {
        if state.ship_structure.is_none() {
            ship_view_state(state, false);
        }
        crate::gui::pages::maps::draw(ctx, theme, state);
    });
    crate::gui::pages::maps::set_clock_for_snapshot(None);
}

/// Maps increment 3: the same, on the plan: straight down over you from 60 m, the ship's bow up
/// the screen, the upper floor's rooms filled and named, walls as lines (no engine, so no baked
/// plan tiles: in the game the tiles draw the floor under these symbols).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_maps_ship_plan() {
    crate::gui::pages::maps::set_clock_for_snapshot(Some(843_782_400.0));
    render_page_png("maps_ship_plan", 1280, 900, |ctx, theme, state| {
        if state.ship_structure.is_none() {
            ship_view_state(state, true);
        }
        crate::gui::pages::maps::draw(ctx, theme, state);
    });
    crate::gui::pages::maps::set_clock_for_snapshot(None);
}

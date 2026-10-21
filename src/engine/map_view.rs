//! The Maps page's WORLD layer (Maps increment 1, docs/design/maps-zoom-to-anything.md
//! 5.2 and 5.6): the real world rendered from the map's camera, so the ship
//! on the map is the real ship, every mesh and light, the hull wrap, the glass
//! roofs and the gardens under them, the player's own build, lit by the real
//! Sun with the real Earth behind it. The page draws the texture as its
//! canvas backdrop and its symbols over it (`gui/pages/maps/you_layer.rs`).
//!
//! How:
//! * The camera. The page's `MapCamera` gives a pose in the world frame, f64
//!   (`camera.rs`). `render_camera` turns it into the renderer's `Camera` in
//!   the RENDER frame: relative to the ship frame's origin and, while riding,
//!   turned into the hull's own axes (`station::to_hull`), the frame the draw
//!   lists are in. The test `a_point_on_the_hull_lands_where_the_symbols_say`
//!   holds the two projections to half a pixel.
//! * The draw lists. Inside the page branch of the frame (`lib.rs`, one call
//!   line) the lists are still in the HOME frame: lib.rs adds `station_off`
//!   only in the live branch. Riding, the station is the frame's origin and
//!   `station_off` is zero, so home frame and render frame agree; that is why
//!   the world layer is drawn only aboard (`you_layer::world_layer_want`).
//! * The render. `engine::ipc::render_view_onto_kept` with
//!   `ViewPasses::Everything` (the hi-res screenshot's passes: sky, planet and
//!   clouds, celestial lines, god rays, SSAO, scene, transparent, overlay),
//!   into a display-format texture registered with the window's egui
//!   renderer. Its view buffers are kept while the page is open (the same
//!   size again next render) and released when it closes.
//! * When. Only when something changed: the camera moved (in the render
//!   frame: following a ship that hangs over one place, it does not), the
//!   canvas resized, the ship was hidden or shown, or a second passed (the
//!   clock, the clouds, an edit to the ship). While the camera moves it
//!   renders at HALF resolution (a quarter of the pixels) and refines to full
//!   250 ms after it stops (Google Earth's progressive refinement).
//! * Cost. While a page is open the live 3D frame is not drawn at all, so this
//!   spends what the game would have spent at the same place, never more.
//!   Its passes are keyed as the live frame's (`gpu.scene`, `gpu.celestial`,
//!   ...) so the two compare directly, plus `cpu.map_view` for the whole
//!   render; `Renderer::mark_view_frame` makes a page frame that drew it count
//!   on the Performance page and in `debug/frame_costs.json`.
//!
//! Cloud history (the design's risk 1): the cloud passes keep a temporal map
//! indexed by direction from the camera. At ship scale the map's camera is a
//! kilometre or two from the player's and the clouds are 36,000 km off, so
//! the history fits both; the world layer is limited to that band
//! (`camera::WORLD_LAYER_MAX_M`).

use std::time::{Duration, Instant};

use glam::{DVec3, Vec3};

use crate::engine::ipc::{render_view_onto_kept, ViewPasses};
use crate::engine::state::{EngineState, SceneDrawLists};
use crate::gui::pages::maps::camera::{MapPose, MapScene};
use crate::gui::pages::maps::you_layer::{world_layer_want, WorldLayerWant};
use crate::gui::{MapSelf, MapVerb};
use crate::gui::GuiPage;
use crate::renderer::camera::Camera;
use crate::renderer::line::LineVertex;
use crate::renderer::RenderObject;

/// How long after the camera stops before the full-resolution render.
const REFINE_AFTER: Duration = Duration::from_millis(250);
/// The longest the layer goes unrendered at rest (the clock, the clouds,
/// an edit to the ship all show within this).
const REST_REFRESH: Duration = Duration::from_millis(1000);
/// Where the rig's report goes.
const REPORT_PATH: &str = "debug/map_done.json";

/// One texture the world is rendered into.
pub(crate) struct Target {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    id: egui::TextureId,
    size: (u32, u32),
}

/// The world layer's GPU side and its refresh bookkeeping
/// (`EngineState::map_view`).
#[derive(Default)]
pub(crate) struct MapViewGpu {
    full: Option<Target>,
    half: Option<Target>,
    last_key: Option<ViewKey>,
    last_render: Option<Instant>,
    last_move: Option<Instant>,
    last_half: bool,
    /// Buffers are parked in the renderer for the next render.
    buffers_kept: bool,
}

/// What a render depends on, in the render frame.
#[derive(Clone, Copy, PartialEq)]
struct ViewKey {
    eye: [u32; 3],
    fwd: [u32; 3],
    up: [u32; 3],
    size: (u32, u32),
    draw_ship: bool,
}

impl ViewKey {
    fn pose_part(&self) -> ([u32; 3], [u32; 3], [u32; 3]) {
        (self.eye, self.fwd, self.up)
    }
}

fn bits(v: Vec3) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

/// The renderer camera for a map pose: the same eye, look and up, in the
/// render frame (relative to the ship frame's origin, turned into the hull's
/// axes while riding), the same field of view, and the canvas's aspect.
pub(crate) fn render_camera(pose: &MapPose, me: &MapSelf, aspect: f32) -> Camera {
    let to_render = |v: DVec3| crate::station::to_hull(me.aboard, me.ship_rot, v);
    let eye = to_render(pose.eye - me.ship_pos_m).as_vec3();
    let fwd = to_render(pose.forward).normalize().as_vec3();
    let up = to_render(pose.up).normalize().as_vec3();
    let mut cam = Camera::new();
    cam.position = eye;
    // The surface basis takes any up, so the renderer's look_at gets the
    // map's own up and no roll is lost (`surface_walk::surface_look_angles`).
    cam.surface_mode = true;
    cam.surface_up = up;
    cam.up = up;
    let (yaw, pitch) = crate::surface_walk::surface_look_angles(up, fwd);
    cam.yaw = yaw;
    cam.pitch = pitch;
    cam.roll = 0.0;
    cam.fov_degrees = pose.fov_y.to_degrees() as f32;
    cam.aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 16.0 / 9.0 };
    cam.near = 0.1;
    cam
}

/// Once a frame, from lib.rs's page branch, before the egui pass: render the
/// world layer if the Maps page wants it and something changed, and write
/// the rig's report when one is due.
#[allow(clippy::too_many_arguments)]
pub(crate) fn frame(
    state: &mut EngineState,
    celestial: &[RenderObject],
    celestial_transparent: &[RenderObject],
    orbit_lines: &[LineVertex],
    opaque: &[RenderObject],
    transparent: &[RenderObject],
    overlay: &[RenderObject],
    ring_lines: &[LineVertex],
) {
    if state.gui_state.active_page != GuiPage::Maps {
        release(state);
        write_report_if_due(state, None);
        return;
    }
    let want = world_layer_want(&state.gui_state.maps);
    if want == WorldLayerWant::Draw && state.world_loaded {
        let lists = SceneDrawLists { celestial, celestial_transparent, orbit_lines, opaque, transparent, overlay, ring_lines };
        render_if_due(state, &lists);
    }
    write_report_if_due(state, Some(want));
}

fn render_if_due(state: &mut EngineState, lists: &SceneDrawLists) {
    let maps = &state.gui_state.maps;
    let (Some(me), Some([w_pt, h_pt])) = (maps.me.clone(), maps.canvas_points) else { return };
    let ppp = maps.pixels_per_point.max(0.25);
    let full = (((w_pt * ppp).round() as u32).max(16), ((h_pt * ppp).round() as u32).max(16));
    let scene = MapScene::new(Some(&me), maps.sim_time_seconds, maps.live);
    let pose = maps.camera.pose(&scene);
    let draw_ship = maps.draw_ship;
    let cam_full = render_camera(&pose, &me, full.0 as f32 / full.1 as f32);
    let key = ViewKey { eye: bits(cam_full.position), fwd: bits(cam_full.forward()), up: bits(cam_full.up), size: full, draw_ship };

    let now = Instant::now();
    let mv = &mut state.map_view;
    let moved = mv.last_key.map_or(true, |k| k.pose_part() != key.pose_part());
    if moved && mv.last_key.is_some() {
        mv.last_move = Some(now);
    }
    let half = mv.last_move.is_some_and(|t| now.duration_since(t) < REFINE_AFTER);
    let due = moved
        || mv.last_key.map_or(true, |k| k.size != key.size || k.draw_ship != key.draw_ship)
        || (mv.last_half && !half)
        || mv.last_render.map_or(true, |t| now.duration_since(t) >= REST_REFRESH);
    if !due {
        return;
    }
    let size = if half { ((full.0 / 2).max(8), (full.1 / 2).max(8)) } else { full };
    let target = ensure_target(state, half, size);
    let _cost = crate::renderer::frame_costs::stage("cpu.map_view");
    let empty_objects: [RenderObject; 0] = [];
    let empty_lines: [LineVertex; 0] = [];
    let ship_lists = SceneDrawLists {
        celestial: lists.celestial,
        celestial_transparent: lists.celestial_transparent,
        orbit_lines: lists.orbit_lines,
        opaque: if draw_ship { lists.opaque } else { &empty_objects },
        transparent: if draw_ship { lists.transparent } else { &empty_objects },
        overlay: if draw_ship { lists.overlay } else { &empty_objects },
        ring_lines: if draw_ship { lists.ring_lines } else { &empty_lines },
    };
    render_view_onto_kept(state, &cam_full, &target.0, size, &ship_lists, ViewPasses::Everything, true);
    state.renderer.mark_view_frame();
    let mv = &mut state.map_view;
    mv.buffers_kept = true;
    mv.last_key = Some(key);
    mv.last_render = Some(now);
    mv.last_half = half;
    let world = &mut state.gui_state.maps.world;
    world.texture = Some(target.1);
    world.size = [size.0, size.1];
    world.half = half;
    world.renders += 1;
    world.ship_drawn = draw_ship;
    world.rendered_seq = state.gui_state.maps.frame_seq;
}

/// The texture for this render (full or half), made or remade at `size`
/// and registered with the window's egui renderer. Returns its view and id.
fn ensure_target(state: &mut EngineState, half: bool, size: (u32, u32)) -> (wgpu::TextureView, egui::TextureId) {
    let slot_ok = {
        let slot = if half { &state.map_view.half } else { &state.map_view.full };
        slot.as_ref().is_some_and(|t| t.size == size)
    };
    if !slot_ok {
        let texture = state.renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(if half { "Maps World Layer (half)" } else { "Maps World Layer" }),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: state.renderer.surface_format(), // display-format: the present pass writes it and egui samples it, as the swapchain
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let old = if half { state.map_view.half.take() } else { state.map_view.full.take() };
        let id = match old {
            Some(old) => {
                state.egui_renderer.update_egui_texture_from_wgpu_texture(
                    &state.renderer.device,
                    &view,
                    wgpu::FilterMode::Linear,
                    old.id,
                );
                old.id
            }
            None => state.egui_renderer.register_native_texture(&state.renderer.device, &view, wgpu::FilterMode::Linear),
        };
        let t = Target { _texture: texture, view, id, size };
        if half {
            state.map_view.half = Some(t);
        } else {
            state.map_view.full = Some(t);
        }
    }
    let t = if half { state.map_view.half.as_ref() } else { state.map_view.full.as_ref() }.expect("made above");
    (t.view.clone(), t.id)
}

/// The page closed: give back the textures, their egui ids and the parked
/// view buffers, so a closed map holds no video memory.
fn release(state: &mut EngineState) {
    for t in [state.map_view.full.take(), state.map_view.half.take()].into_iter().flatten() {
        state.egui_renderer.free_texture(&t.id);
    }
    if state.map_view.buffers_kept {
        state.renderer.release_view_buffers();
    }
    state.map_view = MapViewGpu::default();
    let world = &mut state.gui_state.maps.world;
    world.texture = None;
    world.renders = 0;
}

/// The showcase verbs for the map (`engine/ipc.rs` `poll_showcase_request`):
/// `{"map":"home"|"follow"|"unfollow"|"report"}`, `{"map_distance":"<m>"}`,
/// `{"map_ship":"0"|"1"}`, `{"map_turn":"<deg/s>"}`. Queued for the feed, which applies them after its
/// page-open handling. Permanent dev tooling (the rig's map vantages).
pub(crate) fn showcase(state: &mut EngineState, text: &str) {
    match parse_map_verb(|k| crate::engine::ipc::showcase_value(text, k)) {
        Ok(Some(verb)) => {
            log::info!("Showcase: map -> {verb:?}");
            state.gui_state.maps.pending.push(verb);
        }
        Ok(None) => {}
        Err(why) => log::warn!("Showcase: {why}"),
    }
}

/// Read the map verbs out of a showcase request. None when it has none.
pub(crate) fn parse_map_verb(get: impl Fn(&str) -> Option<String>) -> Result<Option<MapVerb>, String> {
    let mut verb = MapVerb::default();
    let mut any = false;
    if let Some(m) = get("map") {
        any = true;
        match m.trim() {
            "home" => verb.home = true,
            "follow" => verb.follow = Some(true),
            "unfollow" => verb.follow = Some(false),
            "report" => verb.report = true,
            other => return Err(format!("map wants home, follow, unfollow or report, not {other:?}")),
        }
    }
    if let Some(d) = get("map_distance") {
        any = true;
        match d.trim().parse::<f64>() {
            Ok(m) if m.is_finite() && m > 0.0 => verb.distance_m = Some(m),
            _ => return Err(format!("map_distance wants metres above 0, not {d:?}")),
        }
    }
    if let Some(s) = get("map_ship") {
        any = true;
        verb.ship = Some(match s.trim() {
            "1" => true,
            "0" => false,
            other => return Err(format!("map_ship wants 0 or 1, not {other:?}")),
        });
    }
    if let Some(t) = get("map_turn") {
        any = true;
        match t.trim().parse::<f64>() {
            Ok(dps) if dps.is_finite() && dps.abs() <= 360.0 => verb.turn_dps = Some(dps),
            _ => return Err(format!("map_turn wants degrees a second, -360 to 360, not {t:?}")),
        }
    }
    // A report rides along with any other verb when asked for.
    if get("map_report").as_deref() == Some("1") {
        any = true;
        verb.report = true;
    }
    Ok(any.then_some(verb))
}

/// Write `debug/map_done.json` once the rig's report request has been
/// drawn: the page drew at or after the request, and with the world layer
/// wanted, it rendered at or after it too. `want` is None when the page is
/// not open (written at once, saying so).
fn write_report_if_due(state: &mut EngineState, want: Option<WorldLayerWant>) {
    let maps = &state.gui_state.maps;
    let Some(after) = maps.report_after_seq else { return };
    if let Some(w) = want {
        let drawn = maps.drawn_seq >= after;
        let rendered = w != WorldLayerWant::Draw || maps.world.rendered_seq >= after;
        if !(drawn && rendered) {
            return;
        }
    }
    let report = report_json(&state.gui_state.maps, want, state.gui_state.active_page == GuiPage::Maps);
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(REPORT_PATH, report.to_string());
    state.gui_state.maps.report_after_seq = None;
}

/// The report's body (pure, so its shape is pinned without an engine).
pub(crate) fn report_json(maps: &crate::gui::MapsState, want: Option<WorldLayerWant>, open: bool) -> serde_json::Value {
    use crate::gui::pages::maps::camera::MapAnchor;
    let anchor = match &maps.camera.anchor {
        MapAnchor::Me => "me".to_string(),
        MapAnchor::Body(id) => format!("body:{id}"),
        MapAnchor::Point { body, .. } => format!("point:{body}"),
    };
    serde_json::json!({
        "ok": true,
        "page_open": open,
        "follow_me": maps.camera.follows_me(),
        "anchor": anchor,
        "distance_m": maps.camera.distance(),
        "live": maps.live,
        "aboard": maps.me.as_ref().map(|m| m.aboard),
        "world": {
            "want": want.map(|w| format!("{w:?}")),
            "renders": maps.world.renders,
            "size": maps.world.size,
            "half": maps.world.half,
            "ship_drawn": maps.world.ship_drawn,
        },
        "canvas_px": maps.canvas_px,
        "ship_px": maps.ship_px,
        "frame_seq": maps.frame_seq,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::pages::maps::camera::{MapCamera, MapProjector};

    /// A point on the hull lands where the symbols say it is: the renderer
    /// camera built from the map's pose projects a ship-frame point (the
    /// render frame while riding) onto the same canvas pixel the page's f64
    /// projector puts its world position, within half a pixel, at the
    /// opening framing and turned and zoomed round, at four hours of the day.
    ///
    /// Seen red 2026-10-21 with `render_camera` leaving out `station::to_hull`
    /// (the world's axes, not the hull's): "hour 0, view 0: the hull corner is
    /// 24.2 px from where the symbols put it".
    #[test]
    fn a_point_on_the_hull_lands_where_the_symbols_say() {
        let rect = egui::Rect::from_min_size(egui::pos2(260.0, 60.0), egui::vec2(1400.0, 900.0));
        for hour in [0.0, 5.0, 11.0, 19.5] {
            let me = crate::gui::pages::maps::camera::tests::feed_at(hour * 3600.0, DVec3::ZERO);
            let scene = MapScene::new(Some(&me), me.sky_seconds, true);
            for view in 0..3 {
                let mut cam = MapCamera::default();
                cam.open_on_me(Some(&me));
                if view >= 1 {
                    cam.rotate_by(1.3, 0.4, &scene);
                }
                if view == 2 {
                    cam.set_distance(320.0);
                }
                let pose = cam.pose(&scene);
                let proj = MapProjector::new(pose, rect);
                let rcam = render_camera(&pose, &me, rect.width() / rect.height());
                let vp = rcam.view_projection_matrix();
                let (lo, hi) = me.ship_box_m.expect("the feed carries the ship's box");
                let corners = (0..8).map(|i| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }));
                let mut checked = 0;
                for local in corners.chain([me.you_ship_m]) {
                    // Only what is drawn: a point tens of thousands of pixels off the
                    // canvas (one a few metres in front of the eye at 320 m) carries
                    // f32 noise magnified by its angle, and nobody sees it.
                    let Some((want, _)) = proj.project(me.ship_to_world(local)) else { continue };
                    if !rect.expand(rect.height()).contains(want) {
                        continue;
                    }
                    checked += 1;
                    let clip = vp * local.as_vec3().extend(1.0);
                    let ndc = clip.truncate() / clip.w;
                    let got = egui::pos2(
                        rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(),
                        rect.top() + (1.0 - ndc.y) * 0.5 * rect.height(),
                    );
                    let off = (got - want).length();
                    assert!(off < 0.5, "hour {hour}, view {view}: the hull corner is {off:.1} px from where the symbols put it");
                }
                assert!(checked >= 3, "hour {hour}, view {view}: only {checked} hull points were on the canvas to compare");
            }
        }
    }

    /// The rig's verbs parse, and a typo is refused with the reason.
    ///
    /// Seen red 2026-10-21 with "unfollow" read as follow: left `follow:
    /// Some(true)`, right `follow: Some(false)`.
    #[test]
    fn the_map_verbs_parse() {
        let get = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string())
        };
        assert_eq!(parse_map_verb(get(&[])), Ok(None));
        assert_eq!(
            parse_map_verb(get(&[("map", "unfollow"), ("map_distance", "1500")])),
            Ok(Some(MapVerb { follow: Some(false), distance_m: Some(1500.0), ..Default::default() }))
        );
        assert_eq!(
            parse_map_verb(get(&[("map_ship", "0"), ("map_report", "1")])),
            Ok(Some(MapVerb { ship: Some(false), report: true, ..Default::default() }))
        );
        assert!(parse_map_verb(get(&[("map", "hoem")])).is_err());
        assert!(parse_map_verb(get(&[("map_distance", "-3")])).is_err());
    }
}

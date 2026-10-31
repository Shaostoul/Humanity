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
//!   the whole world is drawn only aboard (`maps/world_layer.rs`).
//! * The render. `engine::ipc::render_view_onto_kept` with
//!   `ViewPasses::Everything` (the hi-res screenshot's passes: sky, planet and
//!   clouds, celestial lines, god rays, SSAO, scene, transparent, overlay),
//!   into a display-format texture registered with the window's egui
//!   renderer. Its view buffers are kept while the page is open (the same
//!   size again next render) and released when it closes. Maps increment 2:
//!   the whole world reaches out to the edge of near space (2 million km, the
//!   ship's own lists only within 60 km), and wherever it cannot be drawn but
//!   the world is loaded (the solar-system band, the Diagram, away from the
//!   ship, off Live) the layer is the STAR SKY alone, `ViewPasses::SkyOnly`,
//!   exact from any distance because the star pass reads only the direction.
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
//! the history fits both. Increment 2 takes the whole world out to near space;
//! the rig's `maps-closed-after` vantage is the check that leaving the map
//! does not smear the player's clouds.

use std::time::{Duration, Instant};

use glam::{DVec3, Vec3};

use crate::engine::ipc::{render_view_onto_kept, ViewPasses};
use crate::engine::state::{EngineState, SceneDrawLists};
use crate::gui::pages::maps::camera::{MapPose, MapScene};
use crate::gui::pages::maps::you_layer::{world_layer_want, WorldLayerWant};
use crate::gui::MapSelf;
use crate::gui::GuiPage;
use crate::renderer::camera::Camera;
use crate::renderer::line::LineVertex;
use crate::renderer::RenderObject;

/// How long after the camera stops before the full-resolution render.
const REFINE_AFTER: Duration = Duration::from_millis(250);
/// The longest the layer goes unrendered at rest (the clock, the clouds,
/// an edit to the ship all show within this).
const REST_REFRESH: Duration = Duration::from_millis(1000);

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
    /// The plan tiles (Maps increment 3, engine/map_tiles.rs).
    pub(crate) tiles: crate::engine::map_tiles::MapTilesGpu,
}

/// What a render depends on, in the render frame.
#[derive(Clone, Copy, PartialEq)]
struct ViewKey {
    eye: [u32; 3],
    fwd: [u32; 3],
    up: [u32; 3],
    size: (u32, u32),
    draw_ship: bool,
    sky_only: bool,
    /// The level shown cut open, and the home's rebuilds (Maps increment 3): a pick or an edit
    /// shows at once, not at the next rest refresh.
    level: Option<i32>,
    edits: u64,
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
    // map's own up and no roll is lost (`camera_angles::surface_look_angles`).
    cam.surface_mode = true;
    cam.surface_up = up;
    cam.up = up;
    let (yaw, pitch) = crate::camera_angles::surface_look_angles(up, fwd);
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
        crate::engine::map_rig::write_report_if_due(state, None);
        return;
    }
    let want = world_layer_want(&state.gui_state.maps);
    crate::engine::map_tiles::frame(state, want == WorldLayerWant::Plan);
    if state.world_loaded && (want == WorldLayerWant::Draw || want.draws_sky()) {
        let lists = SceneDrawLists { celestial, celestial_transparent, orbit_lines, opaque, transparent, overlay, ring_lines };
        render_if_due(state, &lists, want != WorldLayerWant::Draw);
    }
    crate::engine::map_rig::write_report_if_due(state, Some(want));
}

/// Render the world layer if anything it depends on changed: the whole world
/// (`sky_only` false: aboard, live, within near space), or the star sky alone
/// (Maps increment 2: the solar-system band, the Diagram, away from the ship).
fn render_if_due(state: &mut EngineState, lists: &SceneDrawLists, sky_only: bool) {
    let maps = &state.gui_state.maps;
    let (Some(me), Some([w_pt, h_pt])) = (maps.me.clone(), maps.canvas_points) else { return };
    let ppp = maps.pixels_per_point.max(0.25);
    let full = (((w_pt * ppp).round() as u32).max(16), ((h_pt * ppp).round() as u32).max(16));
    let scene = MapScene::new(Some(&me), maps.sim_time_seconds, maps.live).with_diagram(maps.diagram);
    let pose = maps.camera.pose(&scene);
    // The ship's meshes only in the ship band: beyond it the ship is under a
    // pixel and its draw lists would cost their submission for nothing.
    let in_ship_band = (pose.eye - me.ship_centre_m()).length() <= crate::gui::pages::maps::camera::WORLD_LAYER_MAX_M;
    let draw_ship = maps.draw_ship && in_ship_band && !sky_only;
    let cam_full = render_camera(&pose, &me, full.0 as f32 / full.1 as f32);
    // The sky depends on the direction only: where the eye is does not count.
    let eye = if sky_only { Vec3::ZERO } else { cam_full.position };
    let level = crate::gui::pages::maps::ship_view::shown_level(maps);
    let key = ViewKey { eye: bits(eye), fwd: bits(cam_full.forward()), up: bits(cam_full.up), size: full, draw_ship, sky_only, level, edits: state.home_draw.edits };

    let now = Instant::now();
    let mv = &mut state.map_view;
    let moved = mv.last_key.map_or(true, |k| k.pose_part() != key.pose_part());
    if moved && mv.last_key.is_some() {
        mv.last_move = Some(now);
    }
    let half = mv.last_move.is_some_and(|t| now.duration_since(t) < REFINE_AFTER);
    let due = moved
        || mv.last_key.map_or(true, |k| (k.size, k.draw_ship, k.sky_only, k.level, k.edits) != (key.size, key.draw_ship, key.sky_only, key.level, key.edits))
        || (mv.last_half && !half)
        || mv.last_render.map_or(true, |t| now.duration_since(t) >= REST_REFRESH);
    if !due {
        return;
    }
    let size = if half { ((full.0 / 2).max(8), (full.1 / 2).max(8)) } else { full };
    let target = ensure_target(state, half, size);
    let _cost = crate::frame_costs::stage("cpu.map_view");
    let empty_objects: [RenderObject; 0] = [];
    let empty_lines: [LineVertex; 0] = [];
    // A level shown: no roof, no hull, nothing over the cut (engine/map_cut.rs).
    let inside = crate::engine::map_cut::inside_lists(state, lists.opaque, lists.transparent, lists.overlay);
    let [opaque, transparent, overlay] = inside.as_ref().map_or([lists.opaque, lists.transparent, lists.overlay], |[o, t, v]| [o.as_slice(), t.as_slice(), v.as_slice()]);
    let ship_lists = SceneDrawLists {
        celestial: lists.celestial,
        celestial_transparent: lists.celestial_transparent,
        orbit_lines: lists.orbit_lines,
        opaque: if draw_ship { opaque } else { &empty_objects },
        transparent: if draw_ship { transparent } else { &empty_objects },
        overlay: if draw_ship { overlay } else { &empty_objects },
        ring_lines: if draw_ship { lists.ring_lines } else { &empty_lines },
    };
    let passes = if sky_only { ViewPasses::SkyOnly } else { ViewPasses::Everything };
    render_view_onto_kept(state, &cam_full, &target.0, size, &ship_lists, passes, true);
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
    world.sky_only = sky_only;
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
    crate::engine::map_tiles::release(state);
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

#[cfg(test)]
#[path = "map_view_tests.rs"]
mod tests;

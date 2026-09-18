//! In-world screens (rungs 1 and 2 of the screens ladder,
//! docs/design/in-world-screens.md): the engine side of a native egui page
//! shown on a flat display placed in the 3D world.
//!
//! What lives here:
//!
//! * The GEOMETRY of a screen as a world quad (`QuadGeom`,
//!   `quad_from_placement`) and the ray test that turns a look ray into a
//!   (u, v) on it (`ray_hit_uv`). Pure functions with unit tests; the
//!   convention they pin is "(0, 0) is the top-left corner AS SEEN BY SOMEONE
//!   FACING THE SCREEN, u runs to their right, v runs down", which is egui's
//!   own y-down coordinate system, so text on a wall is never mirrored or
//!   upside down.
//! * `Screens` on `EngineState`: the surfaces, one per placed machine with a
//!   `screen` in its catalog def, their quads, the hover / focus state, and
//!   the per-frame `update` (look ray -> pointer events), `frame_surfaces`
//!   (draw the pages into their textures) and `push_render_objects` (the
//!   display quads for the scene pass).
//! * The event routing lib.rs calls from its winit handlers: `route_button`,
//!   `route_scroll`, `route_key`.
//!
//! ORDERING RULE, load-bearing: `frame_surfaces` runs a page under the
//! surface's own context with `&mut GuiState`, so it must run OUTSIDE the
//! main UI's `egui_ctx.run` closure (which holds the same borrow for the
//! whole main frame) and BEFORE the scene passes that sample the surface
//! texture, or the wall shows the previous frame's page. lib.rs calls
//! `update` + `frame_surfaces` right before the scene passes for exactly
//! this reason.

/// One provider per source kind, each in its own file (rung 5: video).
pub mod video;

use crate::engine::state::EngineState;
use crate::gui::screen_surface::{ScreenSurface, ScreenSource, ScreenProvider, ScreenWorld};
use crate::gui::GuiPage;
use crate::machines::PlacedMachine;
use crate::renderer::mesh::{Mesh, Vertex};
use crate::renderer::RenderObject;
use glam::{Quat, Vec3};

/// How far the player can reach a screen with the look ray or the cursor,
/// in metres. Beyond this a screen still renders but ignores input.
pub const REACH_M: f32 = 3.5;
/// Screens farther than this from the camera keep their last image instead
/// of re-running their page every frame.
pub const FRAME_RANGE_M: f32 = 40.0;
/// At most this many surfaces re-run their page in one frame (the nearest
/// ones); the rest keep their last image until they come closer.
pub const MAX_FRAMES_PER_TICK: usize = 4;
/// How far the display quad floats in front of the body face, so it never
/// z-fights with the box it sits on.
pub const PROUD_M: f32 = 0.002;

/// A screen's rectangle in space. `origin` is the TOP-LEFT corner as seen
/// by a viewer facing the screen; `u_axis` runs along the full width from
/// that corner to the top-right (the viewer's rightward), `v_axis` along the
/// full height to the bottom-left (the viewer's downward, egui y-down);
/// `normal` points OUT of the face, toward that viewer. So `origin + u*u_axis
/// + v*v_axis` is the world point of egui coordinate (u, v) in 0..1.
///
/// Handedness: `v_axis x u_axis == normal` (equivalently `u_axis x v_axis`
/// points INTO the screen). That is what a right-handed world gives for
/// "right, then down, as seen from the front"; the test
/// `axes_are_right_handed_for_a_front_viewer` pins it, and the corner test
/// proves it is the un-mirrored reading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadGeom {
    pub origin: Vec3,
    pub u_axis: Vec3,
    pub v_axis: Vec3,
    pub normal: Vec3,
}

/// A placed screen: its geometry in the HOME frame (the scene pass adds the
/// station offset), the surface it shows and its renderer slots.
#[derive(Debug, Clone)]
pub struct ScreenQuad {
    /// The placed machine instance id (`wall_screen_1`).
    pub id: String,
    pub geom: QuadGeom,
    /// Index into `Screens::surfaces`.
    pub surface: usize,
    /// Renderer mesh slot of the display quad (LOCAL vertices: the body's
    /// origin, unrotated; `position` + `yaw` place it like the body mesh).
    pub mesh: usize,
    /// Renderer material slot (type 24, albedo view = the surface texture).
    pub material: usize,
    /// The body's draw position and yaw, the same transform the machine
    /// mesh gets, so the quad follows the body exactly.
    pub position: Vec3,
    pub yaw_deg: f32,
}

/// Local-space basis of one box face: (outward normal, the viewer's right,
/// the viewer's down, extent along right, extent along down, face centre)
/// for a box `w` x `h` x `d` centred in x/z with its base at y = 0 (the
/// `Mesh::box_xyz` convention every machine body uses).
///
/// "front" is the -Z face (glTF forward, docs/game/model-pipeline.md). The
/// viewer's right is `forward x up` for a viewer looking at the face along
/// `-normal` with world +Y up: for the front face that is -X, which is why
/// u runs from +x to -x there. "left" and "right" are the viewer's left and
/// right when facing the front. "top" is read by a viewer standing at the
/// front looking down, so its top edge is the far (+Z) edge.
fn face_basis(face: &str, w: f32, h: f32, d: f32) -> Option<(Vec3, Vec3, Vec3, f32, f32, Vec3)> {
    let up = Vec3::Y;
    let hh = h * 0.5;
    Some(match face {
        "front" => (-Vec3::Z, -Vec3::X, -up, w, h, Vec3::new(0.0, hh, -d * 0.5)),
        "back" => (Vec3::Z, Vec3::X, -up, w, h, Vec3::new(0.0, hh, d * 0.5)),
        "left" => (Vec3::X, -Vec3::Z, -up, d, h, Vec3::new(w * 0.5, hh, 0.0)),
        "right" => (-Vec3::X, Vec3::Z, -up, d, h, Vec3::new(-w * 0.5, hh, 0.0)),
        "top" => (Vec3::Y, -Vec3::X, -Vec3::Z, w, d, Vec3::new(0.0, h, 0.0)),
        _ => return None,
    })
}

/// The screen rectangle of a box placed at `pos` (base centre, the machine
/// draw position) with size `(w, h, d)` and yaw `rotation_deg` about Y, on
/// `face`, inset by `bezel` metres on every side and `PROUD_M` in front of
/// the face. Unknown faces fall back to "front" (the caller logs it).
pub fn quad_from_placement(pos: Vec3, size: (f32, f32, f32), rotation_deg: f32, face: &str, bezel: f32) -> QuadGeom {
    let (w, h, d) = size;
    let (n, right, down, lu, lv, centre) =
        face_basis(face, w, h, d).unwrap_or_else(|| face_basis("front", w, h, d).expect("front exists"));
    // A bezel can never eat the whole face: keep at least 2 mm of display.
    let b = bezel.max(0.0).min((lu.min(lv) * 0.5 - 0.001).max(0.0));
    let origin_local = centre + n * PROUD_M - right * (lu * 0.5 - b) - down * (lv * 0.5 - b);
    let rot = Quat::from_rotation_y(rotation_deg.to_radians());
    QuadGeom {
        origin: pos + rot * origin_local,
        u_axis: rot * (right * (lu - 2.0 * b)),
        v_axis: rot * (down * (lv - 2.0 * b)),
        normal: rot * n,
    }
}

/// Intersect a ray with a screen. Returns `(u, v, distance)` with u, v in
/// 0..1 when the ray hits the FRONT of the quad; `None` when it misses, runs
/// parallel, starts past it, or comes from behind (a screen has one face).
pub fn ray_hit_uv(ray_origin: Vec3, ray_dir: Vec3, quad: &QuadGeom) -> Option<(f32, f32, f32)> {
    let denom = ray_dir.dot(quad.normal);
    // From the front the ray travels AGAINST the outward normal. A ray
    // travelling with it is behind the screen, looking at its back.
    if denom >= -1.0e-6 {
        return None;
    }
    let t = (quad.origin - ray_origin).dot(quad.normal) / denom;
    if t <= 0.0 {
        return None;
    }
    let p = ray_origin + ray_dir * t;
    let rel = p - quad.origin;
    let uu = quad.u_axis.length_squared();
    let vv = quad.v_axis.length_squared();
    if uu <= 0.0 || vv <= 0.0 {
        return None;
    }
    let u = rel.dot(quad.u_axis) / uu;
    let v = rel.dot(quad.v_axis) / vv;
    // A ray aimed at the exact edge lands a rounding error outside 0..1;
    // the edge is part of the screen, so accept a hair beyond it and clamp.
    const EDGE_EPS: f32 = 1.0e-4;
    if !(-EDGE_EPS..=1.0 + EDGE_EPS).contains(&u) || !(-EDGE_EPS..=1.0 + EDGE_EPS).contains(&v) {
        return None;
    }
    Some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0), t))
}

/// The display quad mesh in the body's LOCAL space (two triangles). UV (0,0)
/// at the top-left corner as seen from the front, (1,1) bottom-right, which
/// matches the egui page's own pixel layout in the surface texture. Wound
/// counter-clockwise as seen from the front so back-face culling keeps the
/// visible side: `(BL - TL) x (BR - TL)` points along the outward normal.
pub fn screen_quad_mesh(device: &wgpu::Device, local: &QuadGeom) -> Mesh {
    let (vertices, indices) = screen_quad_vertices(local);
    Mesh::from_vertices(device, &vertices, &indices)
}

/// The quad's four vertices and six indices, GPU-free so a test can check
/// the exact data the mesh is built from: vertex 0 is the top-left corner
/// at uv (0,0), 1 top-right (1,0), 2 bottom-right (1,1), 3 bottom-left
/// (0,1); the index order winds both triangles so their face normal is the
/// outward normal (counter-clockwise seen from the front).
pub fn screen_quad_vertices(local: &QuadGeom) -> ([Vertex; 4], [u32; 6]) {
    let tl = local.origin;
    let tr = local.origin + local.u_axis;
    let br = local.origin + local.u_axis + local.v_axis;
    let bl = local.origin + local.v_axis;
    let n = local.normal.to_array();
    let v = |p: Vec3, uv: [f32; 2]| Vertex { position: p.to_array(), normal: n, uv };
    let vertices = [v(tl, [0.0, 0.0]), v(tr, [1.0, 0.0]), v(br, [1.0, 1.0]), v(bl, [0.0, 1.0])];
    let indices = [0u32, 3, 2, 0, 2, 1];
    (vertices, indices)
}

/// The ONE registry of content providers: which `ScreenProvider` serves a
/// non-page source. A page source needs none (the surface draws it through
/// `gui::dispatch`); a kind with no arm yet draws the "not wired" notice.
/// Each rung of the screens ladder adds its arm here and its provider in
/// its own file under `src/engine/screens/`, so the live-feed, video and web
/// rungs never edit each other's code.
pub fn provider_for(source: &ScreenSource) -> Option<Box<dyn ScreenProvider>> {
    match source {
        ScreenSource::Page(_) | ScreenSource::Unknown(_) => None,
        // A WebM clip through the purpose-built player, looping, with its
        // sound placed at the screen (rung 5, `screens/video.rs`).
        ScreenSource::Video(path) => Some(Box::new(video::VideoProvider::new(path))),
        // Live streams, in-game cameras and the readable web: added by
        // their rungs (see docs/design/in-world-screens.md, the sources
        // table). Until then the surface's notice names the gap.
        ScreenSource::Live(_) | ScreenSource::Camera(_) | ScreenSource::Web(_) => None,
    }
}

/// The egui key for a winit key name: winit says "KeyA" and "Digit1" where
/// egui says "A" and "1"; everything else ("Enter", "ArrowLeft",
/// "Backspace") is spelled the same. `None` for keys egui has no name for.
pub fn egui_key_from_winit_name(key_name: &str) -> Option<egui::Key> {
    let name = key_name.strip_prefix("Key").or_else(|| key_name.strip_prefix("Digit")).unwrap_or(key_name);
    egui::Key::from_name(name)
}

/// A dev-IPC interaction in progress (see `engine::ipc::poll_screen_request`).
/// The engine holds it across frames because a click is a press on one frame
/// and a release on the next (re-interacted rows need them apart), and the
/// snapshot must be read AFTER the surface has drawn the frame the event
/// landed in.
#[derive(Debug, Clone)]
pub struct ScreenIpc {
    /// Index into `Screens::surfaces`.
    pub surface: usize,
    pub action: String,
    pub uv: (f32, f32),
    /// 0 = event queued this frame, 1 = a click's release still to queue,
    /// 2 = ready to complete after this frame's `frame_surfaces`.
    pub stage: u8,
    pub snapshot: bool,
}

/// Every in-world screen, on `EngineState`.
#[derive(Default)]
pub struct Screens {
    pub surfaces: Vec<ScreenSurface>,
    pub quads: Vec<ScreenQuad>,
    /// The surface under the look ray / cursor this frame, with the hit as
    /// a (u, v) pair stored in a `Pos2` (x = u, y = v, both 0..1).
    pub hover: Option<(usize, egui::Pos2)>,
    /// The surface that took the last click; it receives typed text while
    /// its page wants keyboard input. Escape or a click elsewhere clears it.
    pub focused: Option<usize>,
    /// A dev-IPC interaction in flight; while set, the look ray leaves that
    /// surface's pointer alone so the synthetic events are not overwritten.
    pub ipc: Option<ScreenIpc>,
    /// The request's one-shot payloads (wheel notches, typed text), read
    /// once when the event is queued.
    pub ipc_payload: Option<(f32, String)>,
    /// Monotonic per-session counter for `debug/screen_<id>_N.png`.
    pub snapshot_counter: u32,
}

impl Screens {
    /// Index of the surface for machine instance `id`.
    pub fn surface_index(&self, id: &str) -> Option<usize> {
        self.surfaces.iter().position(|s| s.core.id == id)
    }

    /// Whether a screen currently owns the keyboard: it holds focus AND its
    /// page has a focused text field. lib.rs skips gameplay key presses while
    /// this is true, the same way it does for an open in-world modal.
    pub fn keyboard_captured(&self) -> bool {
        self.focused.and_then(|i| self.surfaces.get(i)).map_or(false, |s| s.core.wants_keyboard())
    }

    /// Route a primary button press or release. Returns true when a screen
    /// took it (the caller then skips the game's own click action for a
    /// press). A press with no screen under the ray releases keyboard focus,
    /// so clicking away from a screen you were typing on hands the keys back
    /// to the game.
    pub fn route_button(&mut self, pressed: bool) -> bool {
        match self.hover {
            Some((si, uv)) => {
                if let Some(s) = self.surfaces.get_mut(si) {
                    // The surface's own entry point: the egui core AND the
                    // provider (a clip pauses on click) see the event.
                    s.button((uv.x, uv.y), pressed);
                    if pressed {
                        self.set_focus(Some(si));
                    }
                    return true;
                }
                false
            }
            None => {
                if pressed {
                    self.set_focus(None);
                }
                false
            }
        }
    }

    /// Route a wheel notch to the hovered screen. Returns true when taken.
    pub fn route_scroll(&mut self, lines: f32) -> bool {
        if let Some((si, uv)) = self.hover {
            if let Some(s) = self.surfaces.get_mut(si) {
                s.core.scroll((uv.x, uv.y), lines);
                return true;
            }
        }
        false
    }

    /// Route a key event (and the text it typed, if any) to the focused
    /// screen while its page wants keyboard input. Escape always releases
    /// focus instead. Returns true when the screen took the event, in which
    /// case the caller runs no gameplay handler for it.
    ///
    /// `key_name` is the winit `KeyCode` debug name ("KeyA", "Digit1",
    /// "Enter", "ArrowLeft"); egui's `Key::from_name` reads the un-prefixed
    /// forms, so the two prefixes are stripped here.
    pub fn route_key(&mut self, key_name: &str, pressed: bool, text: Option<&str>, modifiers: egui::Modifiers) -> bool {
        let Some(si) = self.focused else { return false };
        if key_name == "Escape" {
            if pressed {
                self.set_focus(None);
            }
            return true;
        }
        let Some(s) = self.surfaces.get_mut(si) else { return false };
        if !s.core.wants_keyboard() {
            return false;
        }
        if let Some(key) = egui_key_from_winit_name(key_name) {
            s.core.key(key, pressed, modifiers);
        }
        // Typed text only on the press, and never while Ctrl is held (a
        // Ctrl+A is a shortcut, not the letter a).
        if pressed && !modifiers.ctrl && !modifiers.command {
            if let Some(t) = text {
                if !t.chars().any(|c| c.is_control()) {
                    s.core.text(t);
                }
            }
        }
        true
    }

    fn set_focus(&mut self, next: Option<usize>) {
        if self.focused == next {
            return;
        }
        if let Some(prev) = self.focused.and_then(|i| self.surfaces.get_mut(i)) {
            prev.core.set_focus(false);
        }
        if let Some(s) = next.and_then(|i| self.surfaces.get_mut(i)) {
            s.core.set_focus(true);
        }
        self.focused = next;
    }

    /// The display quads for the scene pass, one `RenderObject` each, placed
    /// with the body's own position + yaw.
    pub fn push_render_objects(&self, out: &mut Vec<RenderObject>) {
        for q in &self.quads {
            out.push(RenderObject {
                fade: 0.0,
                position: q.position,
                rotation: Quat::from_rotation_y(q.yaw_deg.to_radians()),
                scale: Vec3::ONE,
                mesh: q.mesh,
                material: q.material,
            });
        }
    }

    /// Recompute the quads' world geometry for a count-unchanged move (an
    /// editor drag). No surface or renderer slot changes.
    pub fn update_poses(&mut self, placements: &[PlacedMachine]) {
        for q in &mut self.quads {
            let Some(p) = placements.iter().find(|p| p.id == q.id) else { continue };
            let Some(def) = p.screen.as_ref() else { continue };
            let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
            q.position = pos;
            q.yaw_deg = p.rotation;
            q.geom = quad_from_placement(pos, p.size, p.rotation, &def.face, def.bezel_m);
        }
    }
}

/// Rebuild the screen set from the current placements: create a surface and
/// a display quad for every placed machine whose def has a `screen`, REUSE
/// the surface of an instance that already had one (an editor move must not
/// recreate contexts, which would drop the page's scroll state), and drop
/// the surfaces of instances that are gone. Renderer mesh/material slots
/// follow the same prior-slot reuse the machine bodies use: a screen that
/// persists keeps its slots (mesh replaced in place), a new one gets fresh
/// slots, a removed one's slots are orphaned once (bounded).
pub(crate) fn sync_screens(state: &mut EngineState, placements: &[PlacedMachine]) {
    let mut old_surfaces = std::mem::take(&mut state.screens.surfaces);
    let old_quads = std::mem::take(&mut state.screens.quads);
    let mut surfaces: Vec<ScreenSurface> = Vec::new();
    let mut quads: Vec<ScreenQuad> = Vec::new();
    for p in placements {
        let Some(def) = p.screen.as_ref() else { continue };
        if face_basis(&def.face, 1.0, 1.0, 1.0).is_none() {
            log::warn!(
                "[Screens] {}: unknown face {:?} (front/back/left/right/top); using front",
                p.id,
                def.face
            );
        }
        let (w, h) = (def.px.0.max(1), def.px.1.max(1));
        // Reuse the existing surface when it still matches the def; a
        // changed source or pixel size means a new context and texture. A
        // provider may have resized its surface to its frame (see
        // `ScreenSurface::resize`), so the size check is against the def's
        // px only for a surface that has no provider.
        let reused = old_surfaces
            .iter()
            .position(|s| s.core.id == p.id)
            .map(|i| old_surfaces.remove(i))
            .filter(|s| s.core.source_id == def.source && (s.provider().is_some() || s.size() == (w, h)));
        let (surface, surface_is_new) = match reused {
            Some(s) => (s, false),
            None => {
                let mut s = ScreenSurface::new(&state.renderer.device, &p.id, &def.source, w, h, &state.theme);
                s.set_provider(provider_for(&s.core.source));
                (s, true)
            }
        };
        let si = surfaces.len();
        surfaces.push(surface);
        let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        let local = quad_from_placement(Vec3::ZERO, p.size, 0.0, &def.face, def.bezel_m);
        let mesh = screen_quad_mesh(&state.renderer.device, &local);
        // The material tints the page white (the page as drawn) at the def's
        // brightness; roughness 1 / metallic 0 are ignored by the emitter
        // path but keep the uniform sane if the type is ever bisected.
        let (mesh_slot, material_slot) = match old_quads.iter().find(|q| q.id == p.id) {
            Some(prior) => {
                state.renderer.replace_mesh(prior.mesh, mesh);
                state.renderer.update_material_full(
                    prior.material,
                    [1.0, 1.0, 1.0, 1.0],
                    0.0,
                    1.0,
                    crate::renderer::materials::MATERIAL_TYPE_SCREEN,
                    def.brightness,
                );
                if surface_is_new {
                    state.renderer.set_material_albedo_view(prior.material, surfaces[si].view());
                }
                (prior.mesh, prior.material)
            }
            None => {
                let mi = state.renderer.add_mesh(mesh);
                let ma = state.renderer.add_material_with_albedo_view(
                    [1.0, 1.0, 1.0, 1.0],
                    0.0,
                    1.0,
                    crate::renderer::materials::MATERIAL_TYPE_SCREEN,
                    def.brightness,
                    surfaces[si].view(),
                );
                (mi, ma)
            }
        };
        quads.push(ScreenQuad {
            id: p.id.clone(),
            geom: quad_from_placement(pos, p.size, p.rotation, &def.face, def.bezel_m),
            surface: si,
            mesh: mesh_slot,
            material: material_slot,
            position: pos,
            yaw_deg: p.rotation,
        });
    }
    if !old_surfaces.is_empty() {
        log::info!("[Screens] dropped {} surface(s) whose machine is gone", old_surfaces.len());
    }
    // Indices changed: hover is recomputed next frame, focus and any IPC
    // interaction in flight are simply released.
    state.screens.surfaces = surfaces;
    state.screens.quads = quads;
    state.screens.hover = None;
    state.screens.focused = None;
    state.screens.ipc = None;
    if !state.screens.quads.is_empty() {
        log::info!("[Screens] {} in-world screen(s) placed", state.screens.quads.len());
    }
}

/// Whether screens take input this frame: the world is up, no menu page is
/// covering it, and no editor / showroom / modal / death screen owns the
/// pointer instead.
fn interactive(state: &EngineState) -> bool {
    state.world_loaded
        && state.gui_state.active_page == GuiPage::None
        && !state.gui_state.construction_active
        && !state.gui_state.showroom_active
        && !state.gui_state.in_world_modal_open()
        && state.gui_state.player_death_cause.is_none()
}

/// The ray the player points with: the camera's own look ray while the
/// cursor is grabbed (mouse-look), the cursor's world ray while it is free
/// (Alt held, the F10 sidebar) unless the main UI has something interactive
/// under the cursor, in which case the world gets no ray at all.
fn pointing_ray(state: &EngineState) -> Option<(Vec3, Vec3)> {
    if state.cursor_free {
        if state.egui_ctx.wants_pointer_input() {
            return None;
        }
        let sz = state.window.inner_size();
        let (o, d) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        if d.length_squared() < 0.5 {
            return None;
        }
        Some((o, d))
    } else {
        Some((state.camera.effective_position(), state.camera.forward()))
    }
}

/// Per frame, BEFORE the scene passes: aim the ray, find the nearest screen
/// within reach, and move that screen's pointer (or take it away). Also
/// The click / wheel / key events themselves are routed at winit event time
/// against the hover computed here, one frame stale at most; `route_button`
/// returns true when a screen took the press and lib.rs withholds that
/// press from the game's own click action.
pub(crate) fn update(state: &mut EngineState) {
    if state.screens.surfaces.is_empty() {
        return;
    }
    let ray = if interactive(state) { pointing_ray(state) } else { None };
    let so = state.station_off;
    let mut best: Option<(usize, f32, f32, f32)> = None;
    if let Some((o, d)) = ray {
        for q in &state.screens.quads {
            let g = QuadGeom { origin: q.geom.origin + so, ..q.geom };
            if let Some((u, v, t)) = ray_hit_uv(o, d, &g) {
                if t <= REACH_M && best.map_or(true, |b| t < b.3) {
                    best = Some((q.surface, u, v, t));
                }
            }
        }
    }
    let held = state.screens.ipc.as_ref().map(|i| i.surface);
    let prev = state.screens.hover.map(|(si, _)| si);
    let next = best.map(|(si, _, _, _)| si);
    if let Some(p) = prev {
        if next != Some(p) && held != Some(p) {
            if let Some(s) = state.screens.surfaces.get_mut(p) {
                s.core.pointer_gone();
            }
        }
    }
    match best {
        Some((si, u, v, _)) => {
            if held != Some(si) {
                if let Some(s) = state.screens.surfaces.get_mut(si) {
                    s.core.pointer_moved((u, v));
                }
            }
            state.screens.hover = Some((si, egui::pos2(u, v)));
        }
        None => state.screens.hover = None,
    }
}

/// Draw the pages of the surfaces that matter this frame into their
/// textures: the nearest `MAX_FRAMES_PER_TICK` within `FRAME_RANGE_M` and in
/// front of the camera, plus the hovered one and any dev-IPC target. A
/// surface not framed keeps its last image (the texture persists), so a
/// distant wall still shows a page, just a frozen one.
///
/// Must run outside the main egui closure and before the scene passes (see
/// the module doc).
pub(crate) fn frame_surfaces(state: &mut EngineState) {
    if state.screens.surfaces.is_empty() {
        return;
    }
    let cam = state.camera.effective_position();
    let fwd = state.camera.forward();
    let right = state.camera.right();
    let so = state.station_off;
    let mut ranked: Vec<(usize, f32)> = Vec::new();
    for q in &state.screens.quads {
        let centre = q.geom.origin + (q.geom.u_axis + q.geom.v_axis) * 0.5 + so;
        let to = centre - cam;
        let dist = to.length();
        if dist <= FRAME_RANGE_M && to.dot(fwd) > 0.0 {
            ranked.push((q.surface, dist));
        }
    }
    // The world context for EVERY surface with a provider, framed or not: a
    // clip's sound keeps coming from its screen while the player faces away
    // or walks off, so its volume and pan follow the listener every frame,
    // independent of the framing budget below. A surface with no provider
    // is skipped inside `world_update` (nothing to place).
    //
    // This walks the QUADS, and relies on there being exactly one quad per
    // surface, which `sync_screens` guarantees by construction (one
    // placement makes one surface and one quad, `surface: si`). If a surface
    // ever gets a second quad (a two-sided display, a mirror), this loop
    // must dedupe by surface index, or that surface's provider would get two
    // world updates a frame and its mix would be sent twice (harmless for
    // the epsilon gate, wasteful for the audio thread). Not deduped today
    // because the invariant holds and a per-frame seen-set is a cost paid
    // for a case that does not exist.
    {
        let EngineState { screens, audio, .. } = state;
        let Screens { surfaces, quads, .. } = screens;
        for q in quads.iter() {
            let Some(s) = surfaces.get_mut(q.surface) else { continue };
            if s.provider().is_none() {
                continue;
            }
            let centre = q.geom.origin + (q.geom.u_axis + q.geom.v_axis) * 0.5 + so;
            let mut world = ScreenWorld {
                screen_centre: centre.to_array(),
                listener_pos: cam.to_array(),
                listener_right: right.to_array(),
                audio: audio.as_mut(),
            };
            s.world_update(&mut world);
        }
    }
    ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut chosen: Vec<usize> = ranked.iter().take(MAX_FRAMES_PER_TICK).map(|(si, _)| *si).collect();
    if let Some((si, _)) = state.screens.hover {
        if !chosen.contains(&si) {
            chosen.push(si);
        }
    }
    if let Some(si) = state.screens.ipc.as_ref().map(|i| i.surface) {
        if !chosen.contains(&si) {
            chosen.push(si);
        }
    }
    // Disjoint borrows of EngineState: the surfaces, the renderer's device
    // and queue, the theme and the shared GuiState.
    let EngineState { screens, renderer, theme, gui_state, .. } = state;
    let Screens { surfaces, quads, .. } = screens;
    for si in chosen {
        if let Some(s) = surfaces.get_mut(si) {
            s.frame(&renderer.device, &renderer.queue, theme, gui_state);
            // A provider that resized its surface to its frame left the
            // scene material pointing at the old texture; rebind it now,
            // before the scene pass samples it.
            if s.take_view_changed() {
                for q in quads.iter().filter(|q| q.surface == si) {
                    renderer.set_material_albedo_view(q.material, s.view());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1.0e-4
    }

    /// (a) THE FOUR CORNERS OF A YAW-ROTATED QUAD map to (0,0), (1,0), (0,1),
    /// (1,1) with (0,0) at the top-left AS SEEN FROM THE FRONT, and a ray
    /// from behind misses. The screen is a 1.2 x 0.7 wall screen turned 90
    /// degrees so its front (-Z at rest) faces -X; the viewer stands at -X
    /// looking +X, and for that viewer "right" is +Z and "up" is +Y.
    #[test]
    fn corners_map_to_unit_uv_as_seen_from_the_front() {
        let pos = Vec3::new(10.0, 0.0, 5.0);
        let size = (1.2, 0.7, 0.05);
        let q = quad_from_placement(pos, size, 90.0, "front", 0.0);
        // Rotated front normal: -Z turned by +90 about Y is -X.
        assert!(close(q.normal, -Vec3::X), "normal {:?}", q.normal);
        // The viewer at -X looking +X: their right is +Z, so u runs +Z.
        assert!(close(q.u_axis.normalize(), Vec3::Z), "u {:?}", q.u_axis);
        assert!(close(q.v_axis.normalize(), -Vec3::Y), "v {:?}", q.v_axis);
        // The quad sits 2 mm in front of the -X face (x = 10 - 0.025 - 0.002).
        assert!((q.origin.x - (10.0 - 0.025 - PROUD_M)).abs() < 1.0e-4, "origin {:?}", q.origin);

        // World corners for that viewer: top-left is the top edge at the
        // viewer's LEFT, which is -Z; bottom-right is the bottom at +Z.
        let x = q.origin.x;
        let top_left = Vec3::new(x, 0.7, 5.0 - 0.6);
        let top_right = Vec3::new(x, 0.7, 5.0 + 0.6);
        let bottom_left = Vec3::new(x, 0.0, 5.0 - 0.6);
        let bottom_right = Vec3::new(x, 0.0, 5.0 + 0.6);
        let eye = Vec3::new(8.0, 0.35, 5.0); // 2 m in front, facing +X
        for (corner, want) in [
            (top_left, (0.0, 0.0)),
            (top_right, (1.0, 0.0)),
            (bottom_left, (0.0, 1.0)),
            (bottom_right, (1.0, 1.0)),
        ] {
            let dir = (corner - eye).normalize();
            let (u, v, t) = ray_hit_uv(eye, dir, &q).unwrap_or_else(|| panic!("corner {corner:?} should hit"));
            assert!((u - want.0).abs() < 1.0e-3 && (v - want.1).abs() < 1.0e-3, "corner {corner:?} -> ({u}, {v}), want {want:?}");
            assert!(t > 1.9 && t < 2.3, "distance {t}");
        }
        // From BEHIND (at +X looking -X, straight at the centre) the ray misses.
        let behind = Vec3::new(12.0, 0.35, 5.0);
        assert!(ray_hit_uv(behind, -Vec3::X, &q).is_none(), "a ray from behind must miss");
        // And a ray that starts in front but points away misses too.
        assert!(ray_hit_uv(eye, -Vec3::X, &q).is_none());
        // Just outside the rectangle misses.
        let outside = Vec3::new(x, 0.7 + 0.01, 5.0);
        assert!(ray_hit_uv(eye, (outside - eye).normalize(), &q).is_none());
    }

    /// (b) HANDEDNESS. For a viewer facing the screen, right-then-down is a
    /// right-handed pair whose cross product points AWAY from them, into the
    /// screen: `u x v == -normal`, i.e. `v x u == normal`. A quad satisfying
    /// the opposite sign would be mirrored (u running to the viewer's left).
    /// Checked on every face and at three yaws, with the bezel on.
    #[test]
    fn axes_are_right_handed_for_a_front_viewer() {
        for face in ["front", "back", "left", "right", "top"] {
            for yaw in [0.0_f32, 37.0, 180.0] {
                let q = quad_from_placement(Vec3::new(1.0, 2.0, 3.0), (1.2, 0.7, 0.05), yaw, face, 0.03);
                let vxu = q.v_axis.cross(q.u_axis).normalize();
                assert!(close(vxu, q.normal), "{face} at yaw {yaw}: v x u = {vxu:?}, normal = {:?}", q.normal);
                let uxv = q.u_axis.cross(q.v_axis).normalize();
                assert!(close(uxv, -q.normal), "{face} at yaw {yaw}: u x v must point INTO the screen");
                // The axes are perpendicular to each other and to the normal.
                assert!(q.u_axis.dot(q.v_axis).abs() < 1.0e-4);
                assert!(q.u_axis.dot(q.normal).abs() < 1.0e-4);
                assert!(q.v_axis.dot(q.normal).abs() < 1.0e-4);
            }
        }
    }

    /// The bezel insets the display on every side, and the front face at
    /// rest is the -Z face with u running from +x to -x (the viewer at -Z
    /// looking +Z has +x on their LEFT).
    #[test]
    fn bezel_insets_the_display_and_front_is_minus_z() {
        let q = quad_from_placement(Vec3::ZERO, (1.2, 0.7, 0.05), 0.0, "front", 0.05);
        assert!(close(q.normal, -Vec3::Z));
        assert!(close(q.origin, Vec3::new(0.55, 0.65, -0.025 - PROUD_M)), "origin {:?}", q.origin);
        assert!(close(q.u_axis, Vec3::new(-1.1, 0.0, 0.0)), "u {:?}", q.u_axis);
        assert!(close(q.v_axis, Vec3::new(0.0, -0.6, 0.0)), "v {:?}", q.v_axis);
        // An absurd bezel cannot invert the axes.
        let q2 = quad_from_placement(Vec3::ZERO, (1.2, 0.7, 0.05), 0.0, "front", 5.0);
        assert!(q2.u_axis.dot(Vec3::new(-1.0, 0.0, 0.0)) > 0.0 && q2.u_axis.length() > 0.001);
        assert!(q2.v_axis.dot(-Vec3::Y) > 0.0 && q2.v_axis.length() > 0.001);
        // Unknown faces fall back to the front basis.
        let q3 = quad_from_placement(Vec3::ZERO, (1.2, 0.7, 0.05), 0.0, "sideways", 0.05);
        assert_eq!(q3, q);
    }

    /// The mesh data the quad is built from (read from `screen_quad_vertices`,
    /// the exact vertices and index order the GPU mesh gets, not a rebuilt
    /// copy) agrees with the ray test: each vertex's uv is what the ray
    /// reports at that vertex's position, and BOTH triangles, taken in the
    /// production index order, wind so their face normal is the outward
    /// normal. Reversing the index order fails the winding assertions.
    #[test]
    fn quad_mesh_corner_uvs_and_winding_match_the_ray_test() {
        let local = quad_from_placement(Vec3::ZERO, (1.2, 0.7, 0.05), 0.0, "front", 0.0);
        let (verts, idx) = screen_quad_vertices(&local);
        let eye = Vec3::new(0.0, 0.35, -2.0);
        for (i, v) in verts.iter().enumerate() {
            let p = Vec3::from_array(v.position);
            let (u, w, _) = ray_hit_uv(eye, (p - eye).normalize(), &local).unwrap_or_else(|| panic!("vertex {i} hits"));
            assert!((u - v.uv[0]).abs() < 1e-3 && (w - v.uv[1]).abs() < 1e-3, "vertex {i}: mesh uv {:?}, ray uv ({u}, {w})", v.uv);
            assert!(close(Vec3::from_array(v.normal), local.normal), "vertex {i} normal");
        }
        for tri in idx.chunks(3) {
            let a = Vec3::from_array(verts[tri[0] as usize].position);
            let b = Vec3::from_array(verts[tri[1] as usize].position);
            let c = Vec3::from_array(verts[tri[2] as usize].position);
            let face_n = (b - a).cross(c - a).normalize();
            assert!(close(face_n, local.normal), "triangle {tri:?} winds against the outward normal: {face_n:?}");
        }
        assert_eq!(verts[0].uv, [0.0, 0.0], "vertex 0 is the top-left origin");
        assert_eq!(verts[2].uv, [1.0, 1.0], "vertex 2 is the far corner");
    }

    /// Key names from winit reach egui un-prefixed: "KeyA" -> A, "Digit1" -> 1,
    /// the rest as-is. Goes through the PRODUCTION helper `route_key` uses,
    /// so a regression in the prefix strip fails here.
    #[test]
    fn winit_key_names_resolve_to_egui_keys() {
        for (name, want) in [
            ("KeyA", egui::Key::A),
            ("Digit1", egui::Key::Num1),
            ("Enter", egui::Key::Enter),
            ("ArrowLeft", egui::Key::ArrowLeft),
            ("Backspace", egui::Key::Backspace),
        ] {
            assert_eq!(egui_key_from_winit_name(name), Some(want), "{name}");
        }
        assert_eq!(egui_key_from_winit_name("NoSuchKey"), None);
    }
}

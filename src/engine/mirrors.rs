//! MIRRORS: the engine's side of the planar reflection (2026-11-21, docs/design/mirrors.md).
//!
//! The operator, 2026-11-21: "If a player runs in front of the mirror will it show the player's
//! model?" A machine def with a `mirror` (data: `humanity_ship::machines::MirrorDef`, any piece in
//! any catalog) is real glass, drawn in three steps a frame:
//!
//! 1. [`choose`], before anyone is posed (frame_lists_home.rs): which mirror reflects this frame,
//!    the NEAREST one in view ([`pick`]: the eye in front of its glass, the glass inside the view's
//!    frustum, within [`MAX_DISTANCE_M`], not hidden behind the home's walls and floors), and the
//!    size its picture is drawn at (`renderer::mirror::picture_size`: the glass's size on screen
//!    times the level's scale, Settings > Graphics > Mirror reflections). None while the level is
//!    Off, a page covers the world or no mirror is in view: then nothing more is drawn.
//! 2. The bodies (engine/animation.rs): while a mirror reflects, the player's whole body is queued
//!    for the glass ([`wants_my_body`]): in first person the eye's copy has its head folded away, so
//!    the reflection leaves that copy out and draws the unfolded twin; with "Show my body in first
//!    person" off it draws the body the eye never sees.
//! 3. [`render`], with the camera screens (frame_load_screens.rs, before the live scene pass, so the
//!    glass samples THIS frame's reflection): the reflection camera (`renderer::mirror::
//!    reflected_view`) draws the scene's opaque and see-through lists into the mirror's texture.
//!
//! The glass is a quad on the body's face ([`sync`]), drawn with the reflection's material while
//! its mirror reflects and with plain silvered glass otherwise ([`push_render_objects`]). Only one
//! mirror reflects at a time, the nearest: two mirrors facing each other show each other's glass
//! as plain silver, and a mirror never sees its own glass (left out of its own view).
//!
//! FRAMES: everything here is in the HOME frame, the frame the draw lists are in when the camera
//! screens and the reflection render (lib.rs adds `station_off` to them later in the frame); the
//! live camera is in the render frame, so its eye is moved by `-station_off` ([`eye_home`]).

use crate::engine::screens::{quad_from_placement, screen_quad_vertices, QuadGeom};
use crate::engine::state::{EngineState, SceneDrawLists};
use crate::renderer::mesh::Mesh;
use crate::renderer::mirror as rm;
use crate::renderer::RenderObject;
use glam::{Mat4, Quat, Vec3};
use humanity_ship::machines::{MirrorDef, PlacedMachine};

/// A mirror farther than this from the eye does not reflect: its picture would be a handful of
/// pixels, and a home's far rooms hold mirrors that the walls hide anyway.
pub const MAX_DISTANCE_M: f32 = 15.0;

/// How far from the reflected eye the reflection draws (its far plane, by which its lists are
/// culled too): longer than any room's sight line in a home, shorter than the whole homestead.
pub const REFLECTION_FAR_M: f32 = 80.0;

/// A placed mirror's glass, in the HOME frame.
#[derive(Debug, Clone)]
pub struct MirrorQuad {
    /// The placed machine instance id (`standing_mirror_1`).
    pub id: String,
    /// The glass's rectangle (`screens::QuadGeom`'s convention: top-left corner as seen from the
    /// front, u to the viewer's right, v down, the normal out of the glass).
    pub geom: QuadGeom,
    /// Renderer mesh slot of the glass quad (LOCAL vertices, its u running right to left: the
    /// reflection is the picture a camera behind the glass sees, the mirror image).
    pub mesh: usize,
    /// The material the glass is drawn with while it does not reflect: plain silvered glass.
    pub plain: usize,
    /// The body's draw position and yaw, which place the quad as they place the body.
    pub position: Vec3,
    pub yaw_deg: f32,
    pub def: MirrorDef,
}

/// The mirror reflecting this frame and the size its picture is drawn at (`choose`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Active {
    /// Index into `Mirrors::quads`.
    pub index: usize,
    pub size: (u32, u32),
}

/// Every placed mirror, on `EngineState`.
#[derive(Default)]
pub struct Mirrors {
    pub quads: Vec<MirrorQuad>,
    /// The material whose albedo is the reflection's texture (one: one mirror reflects at a time),
    /// made at the first reflection; and the tint it carries now (the reflecting mirror's).
    pub reflect_material: Option<usize>,
    pub reflect_tint: Option<(f32, f32, f32)>,
    /// This frame's reflecting mirror, chosen before the bodies are posed (`choose`).
    pub active: Option<Active>,
    /// The reflection was drawn this frame (`render`): what the glass's material shows.
    pub drawn: bool,
    /// The draws the last reflection sent against the scene's (opaque kept / opaque in the scene,
    /// see-through kept / see-through in the scene): what the frustum cull leaves, for the probe.
    pub draw_counts: [usize; 4],
    /// A mirror probe in flight (the showcase's `mirror_probe`).
    pub probe: Option<Probe>,
}

/// THE MIRROR PROBE (the showcase's `{"mirror_probe":"<secs>"}`, read by the mirror rig,
/// scripts/lib/mirror-check.js): for `secs`, count the frames and the reflection passes the renderer
/// actually drew (`MirrorView::passes`, counted in `render_scene_onto` itself: real passes, not a
/// flag), then write debug/mirror_probe.json with them, the mirror reflecting at the end, its
/// picture's size and its glass's box on the window (0..1 of the window each way).
#[derive(Debug, Clone, Copy)]
pub struct Probe {
    pub until: std::time::Instant,
    pub secs: f32,
    pub start_passes: u64,
    pub frames: u32,
}

/// Start a mirror probe of `secs`.
pub(crate) fn start_probe(state: &mut EngineState, secs: f32) {
    let start_passes = gpu_ref!(state).renderer.mirror.passes.get();
    state.mirrors.probe = Some(Probe { until: std::time::Instant::now() + std::time::Duration::from_secs_f32(secs), secs, start_passes, frames: 0 });
}

/// The glass's box on the window, (x0, y0, x1, y1) as shares of the window's width and height,
/// clamped to it; None when a corner is behind the eye. Pure.
pub fn glass_box(g: &QuadGeom, view_proj: Mat4) -> Option<[f32; 4]> {
    let corners = [g.origin, g.origin + g.u_axis, g.origin + g.u_axis + g.v_axis, g.origin + g.v_axis];
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for c in corners {
        let p = view_proj * c.extend(1.0);
        if p.w <= 1.0e-4 {
            return None;
        }
        let (x, y) = (p.x / p.w * 0.5 + 0.5, 0.5 - p.y / p.w * 0.5);
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    Some([b[0].clamp(0.0, 1.0), b[1].clamp(0.0, 1.0), b[2].clamp(0.0, 1.0), b[3].clamp(0.0, 1.0)])
}

/// Count this frame for a probe in flight and write its file when its time is up.
fn tick_probe(state: &mut EngineState) {
    let Some(mut p) = state.mirrors.probe else { return };
    p.frames += 1;
    if std::time::Instant::now() < p.until {
        state.mirrors.probe = Some(p);
        return;
    }
    state.mirrors.probe = None;
    let passes = gpu_ref!(state).renderer.mirror.passes.get().saturating_sub(p.start_passes);
    let view_proj = state.camera.view_projection_matrix() * Mat4::from_translation(state.station_off);
    let active = state.mirrors.active.and_then(|a| state.mirrors.quads.get(a.index).map(|q| (a, q)));
    let report = serde_json::json!({
        "secs": p.secs,
        "frames": p.frames,
        "passes": passes,
        "level": level_id(state),
        "mirrors_placed": state.mirrors.quads.len(),
        "active": active.map(|(a, q)| serde_json::json!({
            "id": q.id,
            "size": [a.size.0, a.size.1],
            "box": glass_box(&q.geom, view_proj),
        })),
        "hide_me": gpu_ref!(state).renderer.mirror.hide_me,
        "draws": { "opaque": state.mirrors.draw_counts[0], "opaque_in_scene": state.mirrors.draw_counts[1], "transparent": state.mirrors.draw_counts[2], "transparent_in_scene": state.mirrors.draw_counts[3] },
        "window": [gpu_ref!(state).renderer.surface_size().0, gpu_ref!(state).renderer.surface_size().1],
    });
    let path = std::path::Path::new("debug/mirror_probe.json");
    match std::fs::write(path, serde_json::to_string_pretty(&report).unwrap_or_default()) {
        Ok(()) => log::info!("[Mirrors] probe: {passes} reflection pass(es) in {} frame(s) over {} s", p.frames, p.secs),
        Err(e) => log::warn!("[Mirrors] could not write {}: {e}", path.display()),
    }
}

/// The eye in the HOME frame (the live camera's is in the render frame).
pub(crate) fn eye_home(state: &EngineState) -> Vec3 {
    state.camera.effective_position() - state.station_off
}

/// The glass's rectangle and the body's local quad for a placed mirror.
fn glass(p: &PlacedMachine, def: &MirrorDef) -> (QuadGeom, QuadGeom) {
    let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
    (quad_from_placement(pos, p.size, p.rotation, &def.face, def.frame_m), quad_from_placement(Vec3::ZERO, p.size, 0.0, &def.face, def.frame_m))
}

/// The glass quad's vertices: the screen quad's, with u running the other way (texture x = 1 - the
/// glass's u, `renderer::mirror::reflected_view`). Pure, so a test pins the flip.
pub fn glass_vertices(local: &QuadGeom) -> ([humanity_core::vertex::Vertex; 4], [u32; 6]) {
    let (mut vertices, indices) = screen_quad_vertices(local);
    for v in vertices.iter_mut() {
        v.uv[0] = 1.0 - v.uv[0];
    }
    (vertices, indices)
}

/// Rebuild the mirrors from the current placements (the world load and every editor rebuild): a
/// glass quad and a plain material per placed machine whose def has a `mirror`, keeping the slots
/// of a mirror that was already there.
pub(crate) fn sync(state: &mut EngineState, placements: &[PlacedMachine]) {
    let old = std::mem::take(&mut state.mirrors.quads);
    let mut quads = Vec::new();
    for p in placements {
        let Some(def) = p.mirror.as_ref() else { continue };
        let (geom, local) = glass(p, def);
        let (vertices, indices) = glass_vertices(&local);
        let renderer = &mut gpu!(state).renderer;
        let mesh = Mesh::from_vertices(&renderer.device, &vertices, &indices);
        let tint = [def.tint.0, def.tint.1, def.tint.2, 1.0];
        let (mesh_slot, plain) = match old.iter().find(|q| q.id == p.id) {
            Some(prior) => {
                renderer.replace_mesh(prior.mesh, mesh);
                renderer.update_material_full(prior.plain, tint, 1.0, 0.06, 0.0, 0.0);
                (prior.mesh, prior.plain)
            }
            // Plain silvered glass: a smooth metal in the glass's tint, which catches the lights
            // as highlights and shows no picture (type 0 with metal on draws no panel grid).
            None => (renderer.add_mesh(mesh), renderer.add_material_full(tint, 1.0, 0.06, 0.0, 0.0)),
        };
        quads.push(MirrorQuad { id: p.id.clone(), geom, mesh: mesh_slot, plain, position: Vec3::new(p.pos.0, p.pos.1, p.pos.2), yaw_deg: p.rotation, def: def.clone() });
    }
    if !quads.is_empty() || !old.is_empty() {
        log::info!("[Mirrors] {} mirror(s) placed", quads.len());
    }
    state.mirrors.quads = quads;
    state.mirrors.active = None;
    if state.mirrors.quads.is_empty() {
        gpu!(state).renderer.mirror_release();
    }
}

/// The mirrors follow their bodies on an editor move (geometry only, no renderer slot changes).
pub(crate) fn update_poses(mirrors: &mut Mirrors, placements: &[PlacedMachine]) {
    for q in &mut mirrors.quads {
        let Some(p) = placements.iter().find(|p| p.id == q.id) else { continue };
        let Some(def) = p.mirror.as_ref() else { continue };
        q.geom = glass(p, def).0;
        q.position = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        q.yaw_deg = p.rotation;
    }
}

/// The glass quads for the scene pass, each placed as its body: the reflecting one with the
/// reflection's material once its picture is drawn, every other with plain silvered glass.
pub(crate) fn push_render_objects(mirrors: &Mirrors, out: &mut Vec<RenderObject>) {
    for (i, q) in mirrors.quads.iter().enumerate() {
        let reflecting = mirrors.active.is_some_and(|a| a.index == i);
        let material = match mirrors.reflect_material {
            Some(m) if reflecting => m,
            _ => q.plain,
        };
        out.push(RenderObject { fade: 0.0, position: q.position, rotation: Quat::from_rotation_y(q.yaw_deg.to_radians()), scale: Vec3::ONE, mesh: q.mesh, material });
    }
}

/// THE NEAREST MIRROR IN VIEW, and its glass's size on screen (width, height in pixels). A mirror
/// is in view when the eye is in front of its glass, within `max_m` of it, at least one of its four
/// corners and centre is not hidden from the eye (`visible`), and the glass is not wholly outside
/// one side of the view's frustum (`view_proj` takes a home-frame point to clip space, `screen` is
/// the window's size). The size is the longer of each pair of opposite edges projected; a glass
/// with a corner behind the eye fills the screen. Pure.
pub fn pick(quads: &[QuadGeom], eye: Vec3, view_proj: Mat4, screen: (u32, u32), max_m: f32, visible: impl Fn(Vec3) -> bool) -> Option<(usize, (f32, f32))> {
    let (sw, sh) = (screen.0.max(1) as f32, screen.1.max(1) as f32);
    let mut best: Option<(usize, (f32, f32), f32)> = None;
    for (i, g) in quads.iter().enumerate() {
        let n = g.normal.normalize_or_zero();
        if (eye - g.origin).dot(n) <= rm::MIN_EYE_DIST_M {
            continue;
        }
        let corners = [g.origin, g.origin + g.u_axis, g.origin + g.u_axis + g.v_axis, g.origin + g.v_axis];
        let centre = g.origin + (g.u_axis + g.v_axis) * 0.5;
        let dist = (centre - eye).length();
        if dist > max_m {
            continue;
        }
        // Outside the frustum: every corner beyond the same plane (reverse-Z: near is z <= w, far z >= 0).
        let clip: Vec<glam::Vec4> = corners.iter().map(|c| view_proj * c.extend(1.0)).collect();
        let planes: [fn(&glam::Vec4) -> bool; 6] =
            [|c| c.x < -c.w, |c| c.x > c.w, |c| c.y < -c.w, |c| c.y > c.w, |c| c.z > c.w, |c| c.z < 0.0];
        if planes.iter().any(|out| clip.iter().all(|c| out(c))) {
            continue;
        }
        // Hidden behind the home's walls and floors: every sample hidden.
        let inset = |c: Vec3| c + (centre - c) * 0.1;
        if !std::iter::once(centre).chain(corners.iter().map(|&c| inset(c))).any(|p| visible(p)) {
            continue;
        }
        let size = if clip.iter().any(|c| c.w <= 1.0e-4) {
            (sw, sh)
        } else {
            let px: Vec<glam::Vec2> = clip.iter().map(|c| glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * sw, (0.5 - c.y / c.w * 0.5) * sh)).collect();
            let w = (px[0] - px[1]).length().max((px[3] - px[2]).length());
            let h = (px[0] - px[3]).length().max((px[1] - px[2]).length());
            (w.min(sw * 2.0), h.min(sh * 2.0))
        };
        if best.as_ref().is_none_or(|b| dist < b.2) {
            best = Some((i, size, dist));
        }
    }
    best.map(|(i, s, _)| (i, s))
}

/// The level id in force: the showcase pin, else Settings.
pub(crate) fn level_id(state: &EngineState) -> String {
    gpu_ref!(state).renderer.mirror.pin.clone().unwrap_or_else(|| state.gui_state.settings.mirror_reflections.clone())
}

/// Decide this frame's reflecting mirror (step 1 of the module doc). Call once a frame before the
/// scene lists are built and the bodies posed.
pub(crate) fn choose(state: &mut EngineState) {
    tick_probe(state);
    state.mirrors.drawn = false;
    // An empty id is the showcase's "0" (no reflection whatever the levels file lists).
    let id = level_id(state);
    let level = (!id.is_empty()).then(|| rm::level(&id)).filter(|l| l.scale > 0.0);
    let Some(level) = level else {
        state.mirrors.active = None;
        rm::publish_now(None);
        gpu!(state).renderer.mirror_release();
        return;
    };
    // A page covering the world (Settings itself, say) draws no scene and no reflection; what the
    // mirrors drew in the world stays published for the Settings line to say.
    if !(state.world_loaded && state.gui_state.active_page == crate::gui::GuiPage::None) {
        state.mirrors.active = None;
        return;
    }
    rm::publish_now(None);
    if state.mirrors.quads.is_empty() {
        state.mirrors.active = None;
        return;
    }
    let eye = eye_home(state);
    let so = state.station_off;
    let view_proj = state.camera.view_projection_matrix() * Mat4::from_translation(so);
    let screen = gpu_ref!(state).renderer.surface_size();
    let blockers = crate::engine::built_uses::reach_blockers(state, state.aboard_station);
    let geoms: Vec<QuadGeom> = state.mirrors.quads.iter().map(|q| q.geom).collect();
    let picked = pick(&geoms, eye, view_proj, screen, MAX_DISTANCE_M, |p| !blockers.blocked(eye, p));
    state.mirrors.active = picked.and_then(|(index, projected)| rm::picture_size(level, projected).map(|size| Active { index, size }));
}

/// Whether the player's whole body is queued for the glass this frame (step 2): a mirror reflects
/// and the showcase has not hidden the player from it.
pub(crate) fn wants_my_body(state: &EngineState) -> bool {
    state.mirrors.active.is_some() && state.gpu.as_ref().is_some_and(|g| !g.renderer.mirror.hide_me)
}

/// The opaque list the reflection draws: `opaque` without the reflecting glass itself (a mirror
/// never sees its own glass: the texture it samples is the one being drawn) and without the
/// meshes drawn only for the player's eye (the first-person body with its head folded away, whose
/// whole twin the reflection draws instead). Pure.
pub fn reflection_list(opaque: &[RenderObject], reflect_material: Option<usize>, eye_only: &[usize]) -> Vec<RenderObject> {
    opaque.iter().filter(|o| Some(o.material) != reflect_material && !eye_only.contains(&o.mesh)).cloned().collect()
}

/// Draw this frame's reflection (step 3), from the scene lists as built so far (HOME frame).
pub(crate) fn render(state: &mut EngineState, lists: &SceneDrawLists) {
    let Some(active) = state.mirrors.active else { return };
    let Some(q) = state.mirrors.quads.get(active.index) else { return };
    let (geom, tint) = (q.geom, q.def.tint);
    // The reflection reaches this far behind the glass and no farther (what lies beyond is culled
    // and clipped): a home's far rooms and fields are hidden by its walls in a mirror anyway, and
    // every draw sent costs the processor whatever it covers.
    let far = REFLECTION_FAR_M;
    let Some(view) = rm::reflected_view(geom.origin, geom.u_axis, geom.v_axis, geom.normal, eye_home(state), far) else {
        state.mirrors.active = None;
        return;
    };
    let mut camera = crate::renderer::camera::Camera::new();
    camera.position = view.eye;
    camera.aspect = active.size.0 as f32 / active.size.1.max(1) as f32;
    camera.far = far;
    camera.off_axis = Some(view);
    let renderer = &mut gpu!(state).renderer;
    // What the glass can show: the lists without its own glass and the eye's folded body, culled to
    // the reflection's frustum (the glass is a narrow window onto the room).
    let (opaque, transparent) = {
        let _cost = humanity_core::frame_costs::stage("cpu.mirror_cull");
        let vp = view.proj * view.view;
        let opaque = reflection_list(lists.opaque, state.mirrors.reflect_material, &renderer.mirror.eye_only);
        (renderer.cull_to_view(&opaque, vp), renderer.cull_to_view(lists.transparent, vp))
    };
    state.mirrors.draw_counts = [opaque.len(), lists.opaque.len(), transparent.len(), lists.transparent.len()];
    let fresh = renderer.mirror_begin(active.size);
    let Some(target) = renderer.mirror_target_view() else {
        renderer.mirror_end();
        return;
    };
    let who = crate::renderer::gpu_costs::SceneView::Mirror;
    renderer.render_scene_onto(&camera, &opaque, &target, who);
    renderer.render_transparent_onto(&camera, &transparent, &target, who);
    renderer.mirror_end();
    // The glass's material: made at the first reflection, bound again when the texture is new,
    // tinted as the reflecting mirror's glass.
    let tint4 = [tint.0, tint.1, tint.2, 1.0];
    let material = match state.mirrors.reflect_material {
        Some(m) => {
            if fresh {
                renderer.set_material_albedo_view(m, &target);
            }
            if state.mirrors.reflect_tint != Some(tint) {
                renderer.update_material_full(m, tint4, 0.0, 1.0, crate::renderer::materials::MATERIAL_TYPE_SCREEN, 1.0);
            }
            m
        }
        None => {
            let m = renderer.add_material_with_albedo_view(tint4, 0.0, 1.0, crate::renderer::materials::MATERIAL_TYPE_SCREEN, 1.0, &target);
            renderer.update_material_params2(m, rm::MIRROR_PARAMS2);
            m
        }
    };
    state.mirrors.reflect_material = Some(material);
    state.mirrors.reflect_tint = Some(tint);
    state.mirrors.drawn = true;
    rm::publish_now(Some(active.size));
}

#[cfg(test)]
#[path = "mirrors_tests.rs"]
mod tests;

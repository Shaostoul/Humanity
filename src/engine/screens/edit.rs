//! Build mode's handles on an in-world screen (rung 2 of "Aim assist, any size, and each screen's
//! own settings", docs/design/in-world-screens.md, 2026-11-06). The operator: "add the scaling
//! lines for the TV to the edges of the TV in build mode. Just aim at the edge and drag. Then click
//! on the central area of the screen allows moving it around. Then there could be like a cog like
//! at the top right of the screen for editing that screens settings."
//!
//! What lives here:
//!
//! * [`handle_at`]: which handle a point on the picture is (an edge, a corner, the cog, or the
//!   middle), from the picture's size in metres and the data file's handle band.
//! * [`Frame`], [`ScreenGrab`] and [`drag`]: the drag, worked in the picture's own plane fixed when
//!   the grab starts. An edge moves that edge and the opposite one stays; a corner keeps the aspect
//!   unless Shift is held; the middle moves the screen along its own plane (along the wall and up
//!   or down for a wall screen, never in or out of the wall). The result is the placed instance's
//!   new offset and picture size, which `MachineHome::placements` turns into the body, the quad and
//!   the surface.
//! * The engine glue: [`try_pick`] (a build-mode press on a screen), [`apply_drag`] (each frame of
//!   a grab), [`draw_handles`] (the handles painted over the picture).
//!
//! The cog's panel itself is a GUI page module, `gui::pages::screen_settings`.

use crate::gui::screen_tuning::SizeTuning;
use super::{QuadGeom, ScreenQuad};
use crate::engine::state::{EngineState, ObjectGrab};
use glam::{Mat4, Vec2, Vec3};

/// Which handle of a screen a build-mode press is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// The middle: move the screen along its own plane.
    Move,
    /// The cog near the top-right corner: open the screen's settings panel.
    Cog,
    /// An edge or a corner: `x` is -1 for the left edge, 1 for the right, 0 for neither; `y` is -1
    /// for the top, 1 for the bottom, 0 for neither (left and right as the viewer facing it sees
    /// them, top and bottom as the page is drawn).
    Edges { x: i8, y: i8 },
}

/// The width of the edge handles in metres for a picture `w` x `h`: the data file's share of the
/// shorter side, kept between 3 and 25 cm so a tiny screen still has grabbable edges and a huge
/// one does not turn half its picture into edges.
pub fn band_m(w: f32, h: f32, sizes: &SizeTuning) -> f32 {
    (sizes.handle_band * w.min(h)).clamp(0.03, 0.25)
}

/// The cog's centre, metres from the picture's top-left corner (x right, y down), and its radius:
/// two bands in from the top-right corner, clear of both edge bands, so the corner stays a corner.
pub fn cog(w: f32, h: f32, sizes: &SizeTuning) -> (Vec2, f32) {
    let b = band_m(w, h, sizes);
    (Vec2::new(w - 2.0 * b, 2.0 * b), 0.75 * b)
}

/// Which handle the picture point (`u`, `v`) in 0..1 is, on a picture `w` x `h` metres: the cog
/// first (it is drawn over everything), then an edge band (both bands near a corner make a corner),
/// else the middle.
pub fn handle_at(u: f32, v: f32, w: f32, h: f32, sizes: &SizeTuning) -> Handle {
    let p = Vec2::new(u * w, v * h);
    let (c, r) = cog(w, h, sizes);
    if p.distance(c) <= r {
        return Handle::Cog;
    }
    let b = band_m(w, h, sizes);
    let x = if p.x <= b { -1 } else if p.x >= w - b { 1 } else { 0 };
    let y = if p.y <= b { -1 } else if p.y >= h - b { 1 } else { 0 };
    if x == 0 && y == 0 {
        Handle::Move
    } else {
        Handle::Edges { x, y }
    }
}

/// A screen's picture as a drag sees it: its top-left corner, its unit right and down directions
/// and its normal (world), and its size in metres. Fixed when a grab starts, so the cursor is
/// projected onto the plane the screen started on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub right: Vec3,
    pub down: Vec3,
    pub normal: Vec3,
    pub w: f32,
    pub h: f32,
}

impl Frame {
    pub fn of(q: &QuadGeom) -> Self {
        Self {
            origin: q.origin,
            right: q.u_axis.normalize_or_zero(),
            down: q.v_axis.normalize_or_zero(),
            normal: q.normal,
            w: q.u_axis.length(),
            h: q.v_axis.length(),
        }
    }

    /// Where a ray meets this plane, in metres from the top-left corner along right and down.
    /// From either side (a drag may swing the camera past the plane's edge), never along it.
    pub fn hit(&self, eye: Vec3, dir: Vec3) -> Option<Vec2> {
        let denom = dir.dot(self.normal);
        if denom.abs() < 1.0e-5 {
            return None;
        }
        let t = (self.origin - eye).dot(self.normal) / denom;
        if t <= 0.0 {
            return None;
        }
        let rel = eye + dir * t - self.origin;
        Some(Vec2::new(rel.dot(self.right), rel.dot(self.down)))
    }
}

/// A build-mode grab of a screen's handle, carried in `ObjectGrab::Screen`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenGrab {
    /// The placed machine instance id.
    pub id: String,
    pub handle: Handle,
    pub frame: Frame,
    /// Where the cursor met the plane when the grab started (metres, `Frame::hit`).
    pub start_hit: Vec2,
    /// The instance's offset when the grab started (zone-local metres, `MachineInstance::offset`).
    pub start_offset: (f32, f32, f32),
    /// The def's body size, the screen's face and its bezel: what turns a picture size back into
    /// a body (`humanity_ship::machines::body_size_for_display`).
    pub def_size: (f32, f32, f32),
    pub face: String,
    pub bezel: f32,
}

/// What a drag frame makes of the screen: its picture size and its offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragOut {
    pub display: (f32, f32),
    pub offset: (f32, f32, f32),
}

/// One frame of a grab with the cursor on the plane at `hit`. `keep_aspect` (no Shift) makes a
/// corner keep the picture's aspect; `grid` (the editor's grid snap) steps a move and an edge by
/// that many metres; the size stays within the data file's limits.
pub fn drag(g: &ScreenGrab, hit: Vec2, keep_aspect: bool, grid: Option<f32>, sizes: &SizeTuning) -> DragOut {
    let f = &g.frame;
    let (w0, h0) = (f.w.max(1.0e-3), f.h.max(1.0e-3));
    let mut d = hit - g.start_hit;
    let snap = |v: f32| grid.filter(|s| *s > 0.0).map_or(v, |s| (v / s).round() * s);
    let add = |o: (f32, f32, f32), v: Vec3| (o.0 + v.x, o.1 + v.y, o.2 + v.z);
    match g.handle {
        Handle::Cog => DragOut { display: (w0, h0), offset: g.start_offset },
        Handle::Move => {
            d = Vec2::new(snap(d.x), snap(d.y));
            DragOut { display: (w0, h0), offset: add(g.start_offset, f.right * d.x + f.down * d.y) }
        }
        Handle::Edges { x, y } => {
            // The dragged edges follow the cursor; the others stay where they are.
            let (mut w, mut h) = (w0, h0);
            if x != 0 {
                w = w0 + d.x * x as f32;
            }
            if y != 0 {
                h = h0 + d.y * y as f32;
            }
            let (min, max) = (sizes.min_m, sizes.max_m);
            if x != 0 && y != 0 && keep_aspect {
                // A corner: the larger stretch leads, both sides follow it, within the limits.
                let lo = (min.0 / w0).max(min.1 / h0);
                let hi = (max.0 / w0).min(max.1 / h0);
                let mut s = (w / w0).max(h / h0).clamp(lo, hi.max(lo));
                if let Some(step) = grid.filter(|s| *s > 0.0) {
                    s = ((w0 * s / step).round() * step / w0).clamp(lo, hi.max(lo));
                }
                w = w0 * s;
                h = h0 * s;
            } else {
                if x != 0 {
                    w = snap(w).clamp(min.0, max.0);
                }
                if y != 0 {
                    h = snap(h).clamp(min.1, max.1);
                }
            }
            // The picture's new centre, from the edges that stayed put.
            let cx = if x > 0 { w * 0.5 } else if x < 0 { w0 - w * 0.5 } else { w0 * 0.5 };
            let cy = if y > 0 { h * 0.5 } else if y < 0 { h0 - h * 0.5 } else { h0 * 0.5 };
            let moved = f.right * (cx - w0 * 0.5) + f.down * (cy - h0 * 0.5);
            // The offset is the body's BASE: a body that grew taller sits lower by half the growth
            // for the same centre (a wall screen's picture is centred on its body's face).
            let body = |disp: (f32, f32)| humanity_ship::machines::body_size_for_display(g.def_size, &g.face, g.bezel, disp).1;
            let grew = body((w, h)) - body((w0, h0));
            DragOut { display: (w, h), offset: add(g.start_offset, moved - Vec3::Y * (grew * 0.5)) }
        }
    }
}

/// The screen quad (world frame) under a ray, nearest first: (quad index, u, v, distance).
fn screen_under(quads: &[ScreenQuad], so: Vec3, eye: Vec3, dir: Vec3) -> Option<(usize, f32, f32, f32)> {
    let mut best: Option<(usize, f32, f32, f32)> = None;
    for (i, q) in quads.iter().enumerate() {
        let g = QuadGeom { origin: q.geom.origin + so, ..q.geom };
        if let Some((u, v, t)) = super::ray_hit_uv(eye, dir, &g) {
            if best.map_or(true, |b| t < b.3) {
                best = Some((i, u, v, t));
            }
        }
    }
    best
}

/// The cursor's world ray in the build editor.
fn cursor_ray(state: &EngineState) -> (Vec3, Vec3) {
    let sz = state.window.inner_size();
    state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32))
}

/// The placed screen `q`'s grab for `handle`, from the home's data as it is now.
fn grab_for(state: &EngineState, q: &ScreenQuad, handle: Handle, hit: Vec2, frame: Frame) -> Option<ScreenGrab> {
    let home = state.gui_state.home_machines.as_ref()?;
    let inst = home.all_instances().into_iter().find(|i| i.id == q.id)?;
    let def = home.catalog.get(&inst.machine)?;
    let sd = def.screen.as_ref()?;
    Some(ScreenGrab {
        id: q.id.clone(),
        handle,
        frame,
        start_hit: hit,
        start_offset: inst.offset,
        def_size: def.size,
        face: sd.face.clone(),
        bezel: sd.bezel_m,
    })
}

/// A build-mode press on a placed screen's picture: the cog opens its settings panel; an edge, a
/// corner or the middle selects the screen and arms a grab (a tap only selects, as for every
/// object: the drag starts past the editor's drag threshold). Returns true when a screen took the
/// press. A screen the person may not edit (a ship machine outside the Dev mode) is not offered.
pub(crate) fn try_pick(state: &mut EngineState) -> bool {
    let (eye, dir) = cursor_ray(state);
    let Some((qi, u, v, _)) = screen_under(&state.screens.quads, state.station_off, eye, dir) else { return false };
    let q = state.screens.quads[qi].clone();
    // Only on the storey being edited, as every editor pick (a screen upstairs seen through a
    // stairwell is not grabbed).
    let centre_y = (q.geom.origin + (q.geom.u_axis + q.geom.v_axis) * 0.5).y + state.station_off.y;
    if !crate::engine::editor::in_storey_band(crate::engine::editor::storey_pick_band(state), centre_y) {
        return false;
    }
    let locked = state.gui_state.home_machines.as_ref().is_some_and(|h| h.is_locked(&q.id));
    if locked {
        return false;
    }
    let world = QuadGeom { origin: q.geom.origin + state.station_off, ..q.geom };
    let frame = Frame::of(&world);
    let sizes = &crate::gui::screen_tuning::tuning().sizes;
    let handle = handle_at(u, v, frame.w, frame.h, sizes);
    state.gui_state.construction_machine_selected = Some(q.id.clone());
    state.gui_state.construction_wall_selected = None;
    state.gui_state.construction_light_selected = None;
    state.gui_state.construction_structure_selected = None;
    state.gui_state.construction_storeys.slab_selected = None;
    if handle == Handle::Cog {
        state.gui_state.screen_settings_open = Some(q.id.clone());
        return true;
    }
    let Some(hit) = frame.hit(eye, dir) else { return false };
    let Some(grab) = grab_for(state, &q, handle, hit, frame) else { return false };
    state.construction_object_grab = Some(ObjectGrab::Screen(Box::new(grab)));
    state.construction_grab_press = Some(state.cursor_pos);
    true
}

/// One frame of a screen grab: the cursor's ray on the grab's plane, through [`drag`], into the
/// placed instance (an array cell is detached into its own instance first, as a machine drag
/// does). Shift frees a corner's aspect; the editor's grid snap steps the move and the size.
pub(crate) fn apply_drag(state: &mut EngineState, grab: &ScreenGrab) {
    let (eye, dir) = cursor_ray(state);
    let Some(hit) = grab.frame.hit(eye, dir) else { return };
    let sizes = &crate::gui::screen_tuning::tuning().sizes;
    let grid = state.gui_state.construction_grid_snap.then_some(sizes.grid_m);
    let out = drag(grab, hit, !state.shift_held, grid, sizes);
    let Some(home) = state.gui_state.home_machines.as_mut() else { return };
    if write_drag(home, grab, out) {
        state.gui_state.construction_machines_dirty = true;
    }
}

/// Write a drag frame into the home's data: the instance's offset and, for an edge or a corner,
/// its picture size. An array cell is detached into its own instance first; a screen the person
/// may not edit is left alone. Returns whether anything changed.
pub fn write_drag(home: &mut humanity_ship::machines::MachineHome, grab: &ScreenGrab, out: DragOut) -> bool {
    if home.is_locked(&grab.id) {
        return false;
    }
    if !home.instances.iter().any(|m| m.id == grab.id) {
        home.detach_array_member(&grab.id);
    }
    let Some(inst) = home.instances.iter_mut().find(|m| m.id == grab.id) else { return false };
    let size = if matches!(grab.handle, Handle::Edges { .. }) { Some(out.display) } else { inst.screen_size_m };
    if inst.offset == out.offset && inst.screen_size_m == size {
        return false;
    }
    inst.offset = out.offset;
    inst.screen_size_m = size;
    true
}

/// Project a world point to egui points (the same projection the HUD's labels use).
fn project(world: Vec3, view_proj: Mat4, screen: egui::Rect) -> Option<egui::Pos2> {
    let clip = view_proj * world.extend(1.0);
    if clip.w <= 1.0e-4 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    Some(egui::pos2(screen.left() + (ndc.x * 0.5 + 0.5) * screen.width(), screen.top() + (0.5 - ndc.y * 0.5) * screen.height()))
}

/// Paint the handles of the screen under the cursor and of the selected screen, over the build
/// editor's viewport, on the world's own layer (under every panel): the picture's outline, its
/// four edges (the one under the cursor or being dragged in the accent colour, thicker), its
/// corners, a move cross in the middle, the cog, and the picture's size under it.
///
/// It takes the engine's parts rather than the engine, because lib.rs calls it inside the main
/// egui run, which holds the engine's own egui context: the screens' `quads` and `so` (the station
/// offset), the camera, the cursor and the window size in pixels, the editor's grab and its
/// selected machine.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_handles(
    ctx: &egui::Context,
    theme: &crate::gui::theme::Theme,
    quads: &[ScreenQuad],
    so: Vec3,
    camera: &crate::renderer::camera::Camera,
    cursor: (f32, f32),
    window: (f32, f32),
    grab: Option<&ObjectGrab>,
    selected: Option<&str>,
) {
    let view_proj = camera.view_projection_matrix();
    let screen = ctx.screen_rect();
    let sizes = &crate::gui::screen_tuning::tuning().sizes;
    let (eye, dir) = camera.pick_ray(cursor, window);
    let under = screen_under(quads, so, eye, dir);
    let grabbed = match grab {
        Some(ObjectGrab::Screen(g)) => Some((g.id.clone(), g.handle)),
        _ => None,
    };
    let painter = crate::gui::layers::painter(ctx, egui::Id::new("screen_build_handles"), crate::gui::layers::Layer::Page);
    for (qi, q) in quads.iter().enumerate() {
        let hovered = under.filter(|u| u.0 == qi);
        let is_grabbed = grabbed.as_ref().filter(|(id, _)| *id == q.id).map(|(_, h)| *h);
        if hovered.is_none() && is_grabbed.is_none() && selected != Some(q.id.as_str()) {
            continue;
        }
        let g = QuadGeom { origin: q.geom.origin + so, ..q.geom };
        let f = Frame::of(&g);
        let active = is_grabbed.or_else(|| hovered.map(|(_, u, v, _)| handle_at(u, v, f.w, f.h, sizes)));
        let at = |x: f32, y: f32| project(f.origin + f.right * x + f.down * y, view_proj, screen);
        let (Some(tl), Some(tr), Some(br), Some(bl)) = (at(0.0, 0.0), at(f.w, 0.0), at(f.w, f.h), at(0.0, f.h)) else { continue };
        let accent = theme.accent();
        let quiet = theme.text_secondary();
        let edge = |a: egui::Pos2, b: egui::Pos2, on: bool| {
            painter.line_segment([a, b], egui::Stroke::new(if on { 4.0_f32 } else { 2.0 }, if on { accent } else { quiet }));
        };
        let on = |hx: i8, hy: i8| match active {
            Some(Handle::Edges { x, y }) => (hx != 0 && x == hx) || (hy != 0 && y == hy),
            _ => false,
        };
        edge(tl, tr, on(0, -1));
        edge(tr, br, on(1, 0));
        edge(br, bl, on(0, 1));
        edge(bl, tl, on(-1, 0));
        for (p, cx, cy) in [(tl, -1, -1), (tr, 1, -1), (br, 1, 1), (bl, -1, 1)] {
            let lit = active == Some(Handle::Edges { x: cx, y: cy });
            painter.rect_filled(egui::Rect::from_center_size(p, egui::vec2(9.0, 9.0)), 1.0, if lit { accent } else { quiet });
        }
        // The move cross in the middle.
        if let (Some(c), Some(cr), Some(cd)) = (at(f.w * 0.5, f.h * 0.5), at(f.w * 0.5 + 0.08, f.h * 0.5), at(f.w * 0.5, f.h * 0.5 + 0.08)) {
            let col = if active == Some(Handle::Move) { accent } else { quiet };
            let (rx, ry) = (cr - c, cd - c);
            let s = egui::Stroke::new(2.0_f32, col);
            painter.line_segment([c - rx, c + rx], s);
            painter.line_segment([c - ry, c + ry], s);
        }
        // The cog: a ring with eight teeth.
        let (cc, cr) = cog(f.w, f.h, sizes);
        if let (Some(c), Some(edge_pt)) = (at(cc.x, cc.y), at(cc.x + cr, cc.y)) {
            let r = (edge_pt - c).length().max(5.0);
            let col = if active == Some(Handle::Cog) { accent } else { quiet };
            painter.circle_filled(c, r * 0.95, theme.bg_secondary());
            painter.circle_stroke(c, r * 0.55, egui::Stroke::new(2.0_f32, col));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = egui::vec2(a.cos(), a.sin());
                painter.line_segment([c + d * (r * 0.6), c + d * r], egui::Stroke::new(2.5_f32, col));
            }
        }
        // The picture's size, under its bottom edge.
        let label = crate::gui::screen_tuning::size_label(f.w, f.h);
        let mid = egui::pos2((bl.x + br.x) * 0.5, bl.y.max(br.y) + 14.0);
        painter.text(mid, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(theme.font_size_small), theme.text_primary());
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;

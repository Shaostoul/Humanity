//! Aim assist on in-world screens (rung 1 of "Aim assist, any size, and each screen's own
//! settings", docs/design/in-world-screens.md, 2026-11-06). The operator: "within X degrees
//! highlight the nearest widget, like play or whatever. Let's draw a line between the crosshair and
//! the gui element it is interacting with. That way I don't have to have perfect aim on a tiny play
//! button on a screen very far away."
//!
//! What lives here, all of it pure except [`targets`] (which reads a screen's egui context) and
//! [`draw_overlay`] (which paints):
//!
//! * [`targets`]: the clickable widgets egui itself recorded on a screen's last pass, exactly what
//!   its own hit-test would offer: sensing a click, enabled, not clipped away, on top at its centre.
//! * [`plane_px`], [`px_to_world`], [`angle_deg`], [`reach_m`]: the geometry. The look ray meets the
//!   screen's PLANE (inside the picture or not), and every distance that decides anything is an
//!   ANGLE from the look ray, so a small button on a far screen is as easy as a big one near by.
//! * [`pick`]: the choice, with the hysteresis that keeps it from flickering.
//! * [`Shown`] and [`draw_overlay`]: the outline round the chosen widget and the line from the
//!   crosshair to it, painted on the world's own layer (gui/layers.rs, under every panel).

use crate::gui::screen_tuning::AimTuning;
use super::QuadGeom;
use egui::{Id, Pos2, Rect};
use glam::{Mat4, Vec3};

/// A clickable widget on a screen: its egui id (stable across frames, which is what the hysteresis
/// follows) and its rectangle in the surface's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub id: Id,
    pub rect: Rect,
}

/// The clickable widgets of a screen's last pass, in a stable order (by id).
///
/// egui keeps every widget it laid out, with what it senses, in the finished pass
/// (`ViewportState::prev_pass.widgets`); its own click hit-test reads the same list. A widget is a
/// target when it senses a click AND takes keyboard focus (a button, a link, a checkbox, a text
/// field, a clickable row: egui's click senses all carry focus; selectable text senses a click for
/// selecting but is left out of the Tab order, so the words on a page are not offered), is enabled,
/// has a rectangle left after clipping (a row scrolled out of view has none), and the topmost layer
/// at its centre is its own (a button under an open window is not offered; a panel is not an egui
/// area, so "no area here" counts as its own).
pub fn targets(ctx: &egui::Context) -> Vec<Target> {
    let mut raw: Vec<(egui::LayerId, Id, Rect)> = Vec::new();
    ctx.viewport(|v| {
        for (layer, rects) in v.prev_pass.widgets.layers() {
            for w in rects {
                if w.enabled && w.sense.senses_click() && w.sense.is_focusable() && w.interact_rect.is_positive() {
                    raw.push((*layer, w.id, w.interact_rect));
                }
            }
        }
    });
    let mut out = Vec::with_capacity(raw.len());
    for (layer, id, rect) in raw {
        let rect = match ctx.layer_transform_to_global(layer) {
            Some(t) => t * rect,
            None => rect,
        };
        if ctx.layer_id_at(rect.center()).is_some_and(|top| top != layer) {
            continue;
        }
        out.push(Target { id, rect });
    }
    out.sort_by_key(|t| t.id.value());
    out
}

/// Where the ray from `eye` along the unit `dir` meets `quad`'s plane, in the pixels of a surface
/// `px` big (x right, y down, as the page is drawn), and how far along the ray. Unclamped: outside
/// `0..w` x `0..h` when the ray misses the picture. `None` from behind the screen, along its plane,
/// or with the plane behind the eye.
pub fn plane_px(eye: Vec3, dir: Vec3, quad: &QuadGeom, px: (u32, u32)) -> Option<(Pos2, f32)> {
    let denom = dir.dot(quad.normal);
    if denom >= -1.0e-6 {
        return None;
    }
    let t = (quad.origin - eye).dot(quad.normal) / denom;
    if t <= 0.0 {
        return None;
    }
    let rel = eye + dir * t - quad.origin;
    let (uu, vv) = (quad.u_axis.length_squared(), quad.v_axis.length_squared());
    if uu <= 0.0 || vv <= 0.0 {
        return None;
    }
    let (u, v) = (rel.dot(quad.u_axis) / uu, rel.dot(quad.v_axis) / vv);
    Some((egui::pos2(u * px.0 as f32, v * px.1 as f32), t))
}

/// Whether a plane point (surface pixels) is on the picture, with the same hair of slack the
/// screen's own ray test gives its edge.
pub fn on_picture(p: Pos2, px: (u32, u32)) -> bool {
    let (w, h) = (px.0 as f32, px.1 as f32);
    let eps = 1.0e-4 * w.max(h);
    p.x >= -eps && p.y >= -eps && p.x <= w + eps && p.y <= h + eps
}

/// The world point of surface pixel `p` on `quad`.
pub fn px_to_world(quad: &QuadGeom, px: (u32, u32), p: Pos2) -> Vec3 {
    quad.origin + quad.u_axis * (p.x / px.0.max(1) as f32) + quad.v_axis * (p.y / px.1.max(1) as f32)
}

/// The angle in degrees between the unit look direction `dir` and the direction from `eye` to `at`.
pub fn angle_deg(eye: Vec3, dir: Vec3, at: Vec3) -> f32 {
    let to = (at - eye).normalize_or_zero();
    if to == Vec3::ZERO {
        return 0.0;
    }
    to.dot(dir).clamp(-1.0, 1.0).acos().to_degrees()
}

/// How far a screen answers from (the design's reach rule): the base reach, or the picture's
/// diagonal times `reach_per_diagonal`, whichever is more, never past `reach_max_m`. The shipped
/// 79-inch wall screen (a 2.0 m diagonal) answers from 3.5 m as it always did; a 200" TV from
/// about 8.9 m.
pub fn reach_m(quad: &QuadGeom, t: &AimTuning) -> f32 {
    let diagonal = (quad.u_axis.length_squared() + quad.v_axis.length_squared()).sqrt();
    (diagonal * t.reach_per_diagonal).max(t.reach_base_m).min(t.reach_max_m.max(t.reach_base_m))
}

/// The point of `rect` nearest to `p` (`p` itself when it is inside).
fn nearest_in(rect: Rect, p: Pos2) -> Pos2 {
    egui::pos2(p.x.clamp(rect.min.x, rect.max.x), p.y.clamp(rect.min.y, rect.max.y))
}

/// The angle from the look ray to the nearest point of the whole picture: what decides whether a
/// ray that misses a screen is still near enough for the assist to look at its widgets.
pub fn picture_angle_deg(eye: Vec3, dir: Vec3, quad: &QuadGeom, px: (u32, u32), raw: Pos2) -> f32 {
    let r = Rect::from_min_size(Pos2::ZERO, egui::vec2(px.0 as f32, px.1 as f32));
    angle_deg(eye, dir, px_to_world(quad, px, nearest_in(r, raw)))
}

/// What the assist chose on one screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pick {
    /// The widget a click goes to, if any is within the cone.
    pub target: Option<Target>,
    /// Where the screen's pointer goes, in surface pixels: where the crosshair really is when it is
    /// on the target (or on no target), else the target's point nearest the crosshair, a few pixels
    /// inside it.
    pub pointer: Pos2,
    /// The pointer was moved off the crosshair onto the target (the line is drawn).
    pub snapped: bool,
    /// The target's angle from the look ray, in degrees (0 when the crosshair is on it).
    pub angle_deg: f32,
}

/// Choose the widget a click on this screen goes to.
///
/// `raw` is where the look ray meets the screen's plane ([`plane_px`]); `current` the widget chosen
/// last frame on this screen. The rule (the design's "How it picks"):
///
/// 1. A target the crosshair is ON wins at once, the smallest when several hold it (a button inside
///    a clickable row): the one egui's own hit-test would pick. The pointer stays on the crosshair.
/// 2. Otherwise the target whose nearest point is the smallest ANGLE from the look ray, within
///    `cone_deg`, with hysteresis: the current target is kept while its angle stays within
///    `cone_deg * exit_scale`, unless a rival's angle is below the current one's times
///    `switch_ratio`. The pointer snaps to the target's point nearest the crosshair, `inset_px`
///    inside its rectangle.
/// 3. No target: the pointer stays on the crosshair (on or off the picture; the caller decides
///    whether an off-picture crosshair with no target means "not on this screen").
#[allow(clippy::too_many_arguments)]
pub fn pick(
    eye: Vec3,
    dir: Vec3,
    quad: &QuadGeom,
    px: (u32, u32),
    raw: Pos2,
    targets: &[Target],
    current: Option<Id>,
    cone_deg: f32,
    tuning: &AimTuning,
) -> Pick {
    let none = Pick { target: None, pointer: raw, snapped: false, angle_deg: 0.0 };
    // 1. Under the crosshair.
    if let Some(t) = targets
        .iter()
        .filter(|t| t.rect.contains(raw))
        .min_by(|a, b| a.rect.area().partial_cmp(&b.rect.area()).unwrap_or(std::cmp::Ordering::Equal))
    {
        return Pick { target: Some(*t), pointer: raw, snapped: false, angle_deg: 0.0 };
    }
    // 2. The nearest by angle, with hysteresis.
    let angle_of = |t: &Target| angle_deg(eye, dir, px_to_world(quad, px, nearest_in(t.rect, raw)));
    let scored: Vec<(Target, f32)> = targets.iter().map(|t| (*t, angle_of(t))).collect();
    let best = scored
        .iter()
        .filter(|(_, a)| *a <= cone_deg)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .copied();
    let kept = current.and_then(|id| scored.iter().find(|(t, _)| t.id == id)).copied().filter(|(_, a)| *a <= cone_deg * tuning.exit_scale);
    let chosen = match (kept, best) {
        (Some(k), Some(b)) if b.0.id != k.0.id && b.1 < k.1 * tuning.switch_ratio => Some(b),
        (Some(k), _) => Some(k),
        (None, b) => b,
    };
    let Some((target, angle)) = chosen else { return none };
    let r = target.rect;
    let inset = tuning.inset_px.min(r.width() * 0.5 - 0.5).min(r.height() * 0.5 - 0.5).max(0.0);
    Pick { target: Some(target), pointer: nearest_in(r.shrink(inset), raw), snapped: true, angle_deg: angle }
}

/// What the overlay draws for the chosen widget, in WORLD space (the station offset included), so
/// the main UI projects it with the live camera: the widget's four corners (top-left, top-right,
/// bottom-right, bottom-left as the page is drawn), the point the pointer went to, and whether that
/// point was snapped (the line is drawn only then).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shown {
    pub corners: [Vec3; 4],
    pub point: Vec3,
    pub snapped: bool,
}

impl Shown {
    /// The overlay for `pick` on a screen whose world rectangle is `quad` and surface `px` big.
    pub fn of(quad: &QuadGeom, px: (u32, u32), target: &Target, pointer: Pos2, snapped: bool) -> Self {
        let r = target.rect;
        let w = |p: Pos2| px_to_world(quad, px, p);
        Self {
            corners: [w(r.left_top()), w(r.right_top()), w(r.right_bottom()), w(r.left_bottom())],
            point: w(pointer),
            snapped,
        }
    }
}

/// A screen press in progress (`Screens::press`): which surface, where the press landed (surface
/// pixels) and where the look ray met the plane at that moment. While it is held the pointer moves
/// by the ray's movement since the press, so a click (press and release with the pointer still,
/// egui refuses one that moved more than a few pixels) is never spoiled by the look ray's jitter,
/// and a drag (a slider) still moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressHold {
    pub surface: usize,
    pub pointer: Pos2,
    pub raw: Pos2,
}

impl PressHold {
    /// The pointer for a ray that now meets the plane at `raw_now` (`None`: it does not meet it;
    /// the pointer stays where the press landed), kept on the surface.
    pub fn pointer_for(&self, raw_now: Option<Pos2>, px: (u32, u32)) -> Pos2 {
        let p = match raw_now {
            Some(r) => self.pointer + (r - self.raw),
            None => self.pointer,
        };
        egui::pos2(p.x.clamp(0.0, px.0 as f32), p.y.clamp(0.0, px.1 as f32))
    }
}

/// Project a world point to egui points on a `screen` rectangle (wgpu NDC, reverse-Z or not: any
/// depth from 0 to 1 is in front). `None` behind the camera.
fn project(world: Vec3, view_proj: Mat4, screen: Rect) -> Option<Pos2> {
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

/// Paint the outline round the chosen widget and, when the pointer was snapped, the line from the
/// crosshair `from` (egui points) to it, on the world's own layer (under every panel and popup,
/// like the crosshair). Theme colours only: the accent, the line a little lighter.
pub fn draw_overlay(ctx: &egui::Context, theme: &crate::gui::theme::Theme, shown: &Shown, view_proj: Mat4, from: Pos2) {
    let screen = ctx.screen_rect();
    let painter = crate::gui::layers::painter(ctx, Id::new("screen_aim_assist"), crate::gui::layers::Layer::Page);
    let accent = theme.accent();
    let corners: Option<Vec<Pos2>> = shown.corners.iter().map(|c| project(*c, view_proj, screen)).collect();
    if let Some(points) = corners {
        painter.add(egui::Shape::closed_line(points, egui::Stroke::new(2.0_f32, accent)));
    }
    if shown.snapped {
        if let Some(p) = project(shown.point, view_proj, screen) {
            if p.distance(from) > 3.0 {
                painter.line_segment([from, p], egui::Stroke::new(1.5_f32, accent.gamma_multiply(0.85)));
                painter.circle_filled(p, 2.5, accent);
            }
        }
    }
}

#[cfg(test)]
#[path = "aim_tests.rs"]
mod tests;

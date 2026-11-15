//! The model inspector's view (docs/design/model-inspector.md, rung 1): the lines drawn over the
//! model ([`overlay`]: the origin's axes with up and front named, every empty's own axes, the
//! bounds, the catalog box the model is fitted to, the collision shapes) and the panel beside it
//! ([`panel`]: the report, what is shown, the size and pivot, the catalog boxes, the node tree with
//! every empty selectable, the textures, the clips). What it draws comes from [`Inspection`]
//! (inspect.rs); the website mirrors it in web/shared/model-view.js.
//!
//! The overlay is painted with egui over the GPU's picture, projected with the view's own camera,
//! so the lines land on the model and are never hidden inside it (an x-ray, which is what a person
//! checking a collision shape or an attachment point inside a model needs).

use super::inspect::{Inspection, Level, NodeKind, Verdict};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui};
use glam::{Mat4, Vec3, Vec4};
use humanity_core::i18n::{t, t_dyn, tf};

/// What the person chose to see, kept with the open viewer.
#[derive(Debug, Clone)]
pub struct Look {
    /// The node selected in the tree (its place in `Inspection::nodes`).
    pub selected: Option<usize>,
    /// Which catalog row's box is drawn.
    pub row: usize,
    pub axes: bool,
    pub bounds: bool,
    pub boxes: bool,
    pub collision: bool,
}

impl Default for Look {
    fn default() -> Self {
        Look { selected: None, row: 0, axes: true, bounds: true, boxes: true, collision: true }
    }
}

/// A projection of model points onto the view's rectangle.
struct Projector {
    view_proj: Mat4,
    rect: Rect,
}

impl Projector {
    fn clip(&self, p: Vec3) -> Vec4 {
        self.view_proj * p.extend(1.0)
    }

    fn screen(&self, c: Vec4) -> Pos2 {
        let n = c.truncate() / c.w;
        Pos2::new(self.rect.left() + (n.x * 0.5 + 0.5) * self.rect.width(), self.rect.top() + (0.5 - n.y * 0.5) * self.rect.height())
    }

    /// A point on the screen, when it is in front of the camera.
    fn point(&self, p: Vec3) -> Option<Pos2> {
        let c = self.clip(p);
        (c.w > 1e-5).then(|| self.screen(c))
    }

    /// A line on the screen, cut where it passes behind the camera.
    fn line(&self, a: Vec3, b: Vec3) -> Option<[Pos2; 2]> {
        let (mut ca, mut cb) = (self.clip(a), self.clip(b));
        const NEAR: f32 = 1e-5;
        if ca.w <= NEAR && cb.w <= NEAR {
            return None;
        }
        if ca.w <= NEAR {
            ca = ca + (cb - ca) * ((NEAR - ca.w) / (cb.w - ca.w));
        } else if cb.w <= NEAR {
            cb = cb + (ca - cb) * ((NEAR - cb.w) / (ca.w - cb.w));
        }
        Some([self.screen(ca), self.screen(cb)])
    }
}

/// The twelve edges of the box `min..max`.
fn box_edges(min: Vec3, max: Vec3) -> [[Vec3; 2]; 12] {
    let c = |i: u8| Vec3::new(if i & 1 != 0 { max.x } else { min.x }, if i & 2 != 0 { max.y } else { min.y }, if i & 4 != 0 { max.z } else { min.z });
    let e = |a: u8, b: u8| [c(a), c(b)];
    [e(0, 1), e(2, 3), e(4, 5), e(6, 7), e(0, 2), e(1, 3), e(4, 6), e(5, 7), e(0, 4), e(1, 5), e(2, 6), e(3, 7)]
}

/// Paint the inspector's lines over the view `rect`, seen through `view_proj`.
pub fn overlay(painter: &egui::Painter, theme: &Theme, rect: Rect, view_proj: Mat4, insp: &Inspection, look: &Look) {
    let pr = Projector { view_proj, rect };
    let seg = |a: Vec3, b: Vec3, stroke: Stroke| {
        if let Some([p, q]) = pr.line(a, b) {
            painter.line_segment([p, q], stroke);
        }
    };
    let size = insp.size();
    let radius = (size.length() * 0.5).max(1e-4);
    let font = egui::FontId::proportional(theme.font_size_small);

    if look.collision {
        let stroke = Stroke::new(1.0_f32, theme.inspect_collision());
        for w in &insp.colliders {
            for [a, b] in &w.edges {
                seg(*a, *b, stroke);
            }
        }
    }
    if look.bounds {
        let stroke = Stroke::new(1.0_f32, theme.inspect_bounds());
        for [a, b] in box_edges(insp.min, insp.max) {
            seg(a, b, stroke);
        }
    }
    if look.boxes {
        if let Some(f) = insp.catalog.get(look.row).or_else(|| insp.catalog.first()) {
            let stroke = Stroke::new(2.0_f32, theme.inspect_box());
            // The box is in the model's turned frame (the row's model_yaw): turned back onto the file.
            for [a, b] in box_edges(f.min, f.max) {
                if let Some([p, q]) = pr.line(f.file_point(a), f.file_point(b)) {
                    painter.extend(egui::Shape::dashed_line(&[p, q], stroke, 6.0, 4.0));
                }
            }
        }
    }
    let axes = [theme.axis_x(), theme.axis_y(), theme.axis_z()];
    if look.axes {
        // The model's origin: up and front named.
        let len = radius * 0.35;
        for (k, dir) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
            seg(Vec3::ZERO, dir * len, Stroke::new(2.5_f32, axes[k]));
        }
        for (dir, words, colour) in [(Vec3::Y, t("model_inspect.up"), axes[1]), (Vec3::Z, t("model_inspect.front"), axes[2])] {
            if let Some(p) = pr.point(dir * len * 1.12) {
                painter.text(p, egui::Align2::CENTER_CENTER, words, font.clone(), colour);
            }
        }
        // Every empty's own axes, the selected one larger and named.
        for (slot, n) in insp.nodes.iter().enumerate() {
            if !matches!(n.kind, NodeKind::Empty(_)) {
                continue;
            }
            let chosen = look.selected == Some(slot);
            let at = n.world.w_axis.truncate();
            let len = radius * if chosen { 0.22 } else { 0.1 };
            let width: f32 = if chosen { 2.5 } else { 1.5 };
            for (k, col) in [n.world.x_axis, n.world.y_axis, n.world.z_axis].into_iter().enumerate() {
                seg(at, at + col.truncate().normalize_or_zero() * len, Stroke::new(width, axes[k]));
            }
            if let Some(p) = pr.point(at) {
                painter.circle_filled(p, if chosen { 4.0 } else { 2.5 }, theme.text_primary());
                if chosen {
                    let name = if n.name.is_empty() { t("model_inspect.unnamed") } else { n.name.as_str() };
                    painter.text(p + egui::vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, name, font.clone(), theme.text_primary());
                }
            }
        }
    }
    // A selected node that is not an empty (a mesh, a group): its axes and name too.
    if let Some(n) = look.selected.and_then(|s| insp.nodes.get(s)).filter(|n| !matches!(n.kind, NodeKind::Empty(_))) {
        let at = n.world.w_axis.truncate();
        for (k, col) in [n.world.x_axis, n.world.y_axis, n.world.z_axis].into_iter().enumerate() {
            seg(at, at + col.truncate().normalize_or_zero() * radius * 0.22, Stroke::new(2.5_f32, axes[k]));
        }
        if let Some(p) = pr.point(at) {
            painter.text(p + egui::vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, n.name.as_str(), font, theme.text_primary());
        }
    }
}

/// The colour a finding's level is said in.
fn level_colour(theme: &Theme, level: Level) -> Color32 {
    match level {
        Level::Problem => theme.danger(),
        Level::Check => theme.warning(),
        Level::Note => theme.text_muted(),
    }
}

/// A node's kind in words.
fn kind_words(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Mesh => t("model_inspect.kind_mesh").to_string(),
        NodeKind::Collision => t("model_inspect.kind_collision").to_string(),
        NodeKind::Lod(n) => tf("model_inspect.kind_lod", &[("n", n)]),
        NodeKind::Empty(Some(k)) => tf("model_inspect.kind_empty", &[("kind", k)]),
        NodeKind::Empty(None) => t("model_inspect.kind_empty_none").to_string(),
        NodeKind::Group => t("model_inspect.kind_group").to_string(),
    }
}

fn line(ui: &mut Ui, theme: &Theme, text: impl Into<String>, colour: Color32) {
    ui.add(egui::Label::new(RichText::new(text.into()).size(theme.font_size_small).color(colour)).wrap());
}

/// The panel beside the view: the report first, then what is shown, the size and pivot, the
/// catalog boxes, the nodes, the textures and the clips. `height` points tall, scrolling.
pub fn panel(ui: &mut Ui, theme: &Theme, insp: &Inspection, look: &mut Look, height: f32) {
    egui::ScrollArea::vertical().id_salt("model_inspect_panel").max_height(height).auto_shrink([false, false]).show(ui, |ui| {
        widgets::subsection_label(ui, theme, t("model_inspect.report"));
        let counts = tf(
            "model_inspect.counts",
            &[("problems", &insp.count(Level::Problem)), ("checks", &insp.count(Level::Check)), ("notes", &insp.count(Level::Note))],
        );
        line(ui, theme, counts, theme.text_secondary());
        for f in &insp.findings {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(t_dyn(&format!("model_inspect.level.{}", f.level.id()))).size(theme.font_size_small).strong().color(level_colour(theme, f.level)));
                line(ui, theme, f.words(), theme.text_primary());
            });
        }
        if insp.edges_left_out > 0 {
            line(ui, theme, tf("model_inspect.edges_left_out", &[("n", &insp.edges_left_out)]), theme.text_muted());
        }

        ui.add_space(theme.spacing_sm);
        widgets::subsection_label(ui, theme, t("model_inspect.show_heading"));
        widgets::with_hint(ui, "model_inspect.show_axes_hint", |ui| widgets::toggle(ui, theme, t("model_inspect.show_axes"), &mut look.axes));
        widgets::with_hint(ui, "model_inspect.show_bounds_hint", |ui| widgets::toggle(ui, theme, t("model_inspect.show_bounds"), &mut look.bounds));
        if !insp.catalog.is_empty() {
            widgets::with_hint(ui, "model_inspect.show_box_hint", |ui| widgets::toggle(ui, theme, t("model_inspect.show_box"), &mut look.boxes));
        }
        if !insp.colliders.is_empty() {
            widgets::with_hint(ui, "model_inspect.show_collision_hint", |ui| widgets::toggle(ui, theme, t("model_inspect.show_collision"), &mut look.collision));
        }

        ui.add_space(theme.spacing_sm);
        widgets::subsection_label(ui, theme, t("model_inspect.size_heading"));
        let s = insp.size();
        line(ui, theme, tf("model_inspect.size_line", &[("w", &super::inspect::m2(s.x)), ("h", &super::inspect::m2(s.y)), ("d", &super::inspect::m2(s.z))]), theme.text_primary());
        line(ui, theme, t(if insp.stl { "model_inspect.units_stl" } else { "model_inspect.units_gltf" }), theme.text_secondary());
        line(ui, theme, tf("model_inspect.parts_line", &[("parts", &insp.parts), ("materials", &insp.materials)]), theme.text_secondary());

        if !insp.stl {
            ui.add_space(theme.spacing_sm);
            widgets::subsection_label(ui, theme, t("model_inspect.catalog_heading"));
            if insp.catalog.is_empty() {
                line(ui, theme, t("model_inspect.no_catalog"), theme.text_muted());
            }
            for (i, f) in insp.catalog.iter().enumerate() {
                let words = tf(
                    "model_inspect.catalog_row",
                    &[
                        ("row", &f.row.id),
                        ("file", &f.row.file),
                        ("box_w", &super::inspect::m2(f.box_wd[0])),
                        ("box_d", &super::inspect::m2(f.box_wd[1])),
                        ("w", &super::inspect::m2(f.fitted[0])),
                        ("d", &super::inspect::m2(f.fitted[2])),
                    ],
                );
                let colour = if f.verdict == Verdict::Fits { theme.text_primary() } else { theme.danger() };
                if ui.selectable_label(look.row == i, RichText::new(words).size(theme.font_size_small).color(colour)).on_hover_text(t("model_inspect.catalog_row_hint")).clicked() {
                    look.row = i;
                    look.boxes = true;
                }
            }

            ui.add_space(theme.spacing_sm);
            widgets::subsection_label(ui, theme, t("model_inspect.nodes_heading"));
            if insp.nodes.is_empty() {
                line(ui, theme, t("model_inspect.no_nodes"), theme.text_muted());
            }
            for (slot, n) in insp.nodes.iter().enumerate() {
                let name = if n.name.is_empty() { t("model_inspect.unnamed") } else { n.name.as_str() };
                let words = format!("{name} ({})", kind_words(&n.kind));
                let colour = if matches!(n.kind, NodeKind::Empty(_)) { theme.accent() } else { theme.text_primary() };
                ui.horizontal(|ui| {
                    ui.add_space(theme.spacing_md * n.depth.min(12) as f32);
                    let chosen = look.selected == Some(slot);
                    if ui.selectable_label(chosen, RichText::new(words).size(theme.font_size_small).color(colour)).on_hover_text(t("model_inspect.node_hint")).clicked() {
                        look.selected = if chosen { None } else { Some(slot) };
                    }
                });
            }

            ui.add_space(theme.spacing_sm);
            widgets::subsection_label(ui, theme, t("model_inspect.textures_heading"));
            if insp.textures.is_empty() {
                line(ui, theme, t("model_inspect.no_textures"), theme.text_muted());
            }
            for tex in &insp.textures {
                let (words, colour) = match tex.size {
                    Some([w, h]) => (tf("model_inspect.texture_line", &[("material", &tex.material), ("slot", &tex.slot), ("file", &tex.file), ("w", &w), ("h", &h)]), theme.text_primary()),
                    None => (tf("model_inspect.texture_missing_line", &[("material", &tex.material), ("slot", &tex.slot), ("file", &tex.file)]), theme.danger()),
                };
                line(ui, theme, words, colour);
            }

            ui.add_space(theme.spacing_sm);
            widgets::subsection_label(ui, theme, t("model_inspect.clips_heading"));
            if insp.clips.is_empty() {
                line(ui, theme, t("model_inspect.no_clips"), theme.text_muted());
            }
            for c in &insp.clips {
                line(ui, theme, tf("model_inspect.clip_line", &[("name", &c.name), ("seconds", &format!("{:.2}", c.seconds)), ("channels", &c.channels)]), theme.text_primary());
            }
            if insp.skins > 0 {
                line(ui, theme, tf("model_inspect.skins", &[("n", &insp.skins)]), theme.text_secondary());
            }
        }
    });
}

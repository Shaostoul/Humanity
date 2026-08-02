//! Pie chart - a universal widget (resource budgets increment 1).
//!
//! The operator's framing: "pie charts, each hand is in reference to another".
//! A pie is the honest shape for a BUDGET, because a slice can only grow by
//! taking from its neighbours - which is exactly the trade the Performance page
//! exists to make visible ("more grass IS less rain, on your hardware").
//!
//! This file is the drawing only: it takes finished slices and paints them.
//! What the slices MEAN comes from `data/performance/budget_systems.ron` via
//! `gui::pages::performance`, so adding a system to the pie is a data row.
//!
//! Layout rule (operator: "single stacking the pie charts on mobile; on PC it
//! keeps the widget small"): `pie_grid` stacks pies in one column when the
//! panel is narrow and lays them out two-across when it is wide.
//!
//! Colours come from theme tokens ONLY (`theme.rs` accessors, selected by
//! name in the registry). The arc geometry below is computed maths, not a
//! palette, so there is nothing here for the theme lint to catch.

use egui::{Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2};

use crate::gui::theme::Theme;

/// One wedge of a pie.
#[derive(Clone, Debug)]
pub struct PieSlice {
    /// Row label, e.g. "World surface".
    pub label: String,
    /// Magnitude driving the wedge angle (milliseconds, bytes - any unit, as
    /// long as every slice in one pie shares it). Negatives are clamped away.
    pub value: f64,
    /// Pre-formatted live value, e.g. "4.2 ms" or "612 MB".
    pub display: String,
    /// Wedge colour, already resolved from a theme token.
    pub color: Color32,
    /// A floor system the operator may not starve (increment 2). Shown with a
    /// "locked" marker so the rule is visible before it is enforceable.
    pub locked: bool,
    /// The share this system is EXPECTED to want, 0..1. Shown next to the
    /// measured share so "expected vs actual" needs no arithmetic.
    pub base_frac: f32,
    /// Plain-English hover text.
    pub note: String,
}

impl PieSlice {
    pub fn new(label: impl Into<String>, value: f64, display: impl Into<String>, color: Color32) -> Self {
        Self {
            label: label.into(),
            value: value.max(0.0),
            display: display.into(),
            color,
            locked: false,
            base_frac: 0.0,
            note: String::new(),
        }
    }
}

/// Everything one pie needs: a title, its slices, and a footer that says how
/// the numbers were obtained (honesty is a feature here, not decoration).
#[derive(Clone, Debug)]
pub struct PieSpec {
    pub title: String,
    /// Headline number under the title, e.g. "13.9 ms/frame".
    pub headline: String,
    pub slices: Vec<PieSlice>,
    /// One line about measurement limits, drawn under the legend.
    pub footer: String,
}

/// Diameter of the drawn circle. Deliberately small: four of these share a
/// page, and the operator asked that the widget "stay small" on PC.
const PIE_DIAMETER: f32 = 132.0;
/// Inner hole, as a fraction of the radius. A donut reads better than a solid
/// pie at this size because the labels can sit in the middle.
const HOLE_FRAC: f32 = 0.52;
/// Angular resolution of the arc approximation (degrees per segment).
const SEG_DEG: f32 = 3.0;

/// Fraction of the total each slice represents. Returns an empty vec when the
/// total is zero, which is how "nothing measured yet" is detected.
pub fn fractions(slices: &[PieSlice]) -> Vec<f32> {
    let total: f64 = slices.iter().map(|s| s.value.max(0.0)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    slices.iter().map(|s| (s.value.max(0.0) / total) as f32).collect()
}

/// Draw one pie with its legend. Returns the hovered slice index, if any.
pub fn pie(ui: &mut Ui, theme: &Theme, spec: &PieSpec) -> Option<usize> {
    let mut hovered: Option<usize> = None;
    ui.vertical(|ui| {
        ui.label(
            RichText::new(&spec.title)
                .size(theme.font_size_body)
                .strong()
                .color(theme.text_primary()),
        );
        if !spec.headline.is_empty() {
            ui.label(
                RichText::new(&spec.headline)
                    .size(theme.font_size_small)
                    .color(theme.accent()),
            );
        }
        ui.add_space(theme.spacing_xs);

        let fracs = fractions(&spec.slices);
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(PIE_DIAMETER, PIE_DIAMETER),
            Sense::hover(),
        );
        if fracs.is_empty() {
            // Nothing measured yet: an empty ring plus an honest label beats a
            // fake chart. (Before world entry this is the normal state.)
            if ui.is_rect_visible(rect) {
                ring(ui, rect, theme.bg_card(), theme.border());
            }
            ui.label(
                RichText::new("No measurements yet - enter the world.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        } else {
            // Which wedge is the pointer over? Pure geometry: angle from the
            // centre, radius inside the ring.
            let center = rect.center();
            let radius = rect.width() * 0.5;
            if let Some(p) = resp.hover_pos() {
                hovered = slice_at(center, radius, p, &fracs);
            }
            if ui.is_rect_visible(rect) {
                paint_pie(ui, rect, &spec.slices, &fracs, hovered, theme);
            }
            // Centre label: the hovered slice, or the slice count at rest.
            let (mid, sub) = match hovered {
                Some(i) => (
                    format!("{:.0}%", fracs[i] * 100.0),
                    spec.slices[i].label.clone(),
                ),
                None => (format!("{}", spec.slices.len()), "systems".to_string()),
            };
            let painter = ui.painter();
            painter.text(
                rect.center() - Vec2::new(0.0, 7.0),
                egui::Align2::CENTER_CENTER,
                mid,
                egui::FontId::proportional(theme.font_size_body),
                theme.text_primary(),
            );
            painter.text(
                rect.center() + Vec2::new(0.0, 9.0),
                egui::Align2::CENTER_CENTER,
                sub,
                egui::FontId::proportional(theme.font_size_small * 0.9),
                theme.text_muted(),
            );
        }

        ui.add_space(theme.spacing_xs);
        for (i, s) in spec.slices.iter().enumerate() {
            let pct = fracs.get(i).copied().unwrap_or(0.0);
            let row = legend_row(ui, theme, s, pct, hovered == Some(i));
            if row.hovered() {
                hovered = Some(i);
            }
        }
        if !spec.footer.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(&spec.footer)
                    .size(theme.font_size_small * 0.92)
                    .color(theme.text_muted()),
            );
        }
    });
    hovered
}

/// Lay several pies out: one column when narrow (the phone shape the operator
/// asked for), two columns when there is room. Returns nothing - each pie
/// handles its own hover.
pub fn pie_grid(ui: &mut Ui, theme: &Theme, specs: &[PieSpec]) {
    // 2 x (pie + legend column) plus the gap between them. Below this the
    // legends would wrap into noise, so we stack instead.
    let wide = ui.available_width() >= 620.0;
    if !wide {
        for spec in specs {
            pie(ui, theme, spec);
            ui.add_space(theme.spacing_md);
        }
        return;
    }
    let col = (ui.available_width() - theme.spacing_md) * 0.5;
    for pair in specs.chunks(2) {
        ui.horizontal_top(|ui| {
            for spec in pair {
                ui.allocate_ui(Vec2::new(col, 0.0), |ui| {
                    ui.set_width(col);
                    pie(ui, theme, spec);
                });
            }
        });
        ui.add_space(theme.spacing_md);
    }
}

/// One legend line: swatch, label, measured share, live value.
fn legend_row(
    ui: &mut Ui,
    theme: &Theme,
    s: &PieSlice,
    pct: f32,
    highlight: bool,
) -> egui::Response {
    let resp = ui
        .horizontal(|ui| {
            let (sw, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
            ui.painter().rect_filled(sw, 2.0, s.color);
            let name = if s.locked {
                format!("{} (locked)", s.label)
            } else {
                s.label.clone()
            };
            let name_color = if highlight { theme.text_primary() } else { theme.text_secondary() };
            ui.label(RichText::new(name).size(theme.font_size_small).color(name_color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(&s.display)
                        .size(theme.font_size_small)
                        .strong()
                        .color(theme.text_primary()),
                );
                ui.label(
                    RichText::new(format!("{:.0}%", pct * 100.0))
                        .size(theme.font_size_small)
                        .color(theme.accent()),
                );
            });
        })
        .response;
    let hover = if s.note.is_empty() {
        format!("{} - {:.1}% of this budget", s.label, pct * 100.0)
    } else if s.base_frac > 0.0 {
        format!(
            "{}\nMeasured {:.1}%, expected about {:.0}%.",
            s.note,
            pct * 100.0,
            s.base_frac * 100.0
        )
    } else {
        s.note.clone()
    };
    resp.on_hover_text(hover)
}

/// Empty ring, for the "nothing measured yet" state.
fn ring(ui: &Ui, rect: Rect, fill: Color32, border: Color32) {
    let painter = ui.painter();
    let c = rect.center();
    let r = rect.width() * 0.5;
    painter.circle_filled(c, r, fill);
    painter.circle_stroke(c, r, Stroke::new(1.0, border));
    painter.circle_filled(c, r * HOLE_FRAC, ui.style().visuals.panel_fill);
}

/// Paint every wedge. Each wedge is a polygon fan between the inner and outer
/// radius, which is how you draw an arc with egui's convex-polygon painter.
fn paint_pie(
    ui: &Ui,
    rect: Rect,
    slices: &[PieSlice],
    fracs: &[f32],
    hovered: Option<usize>,
    theme: &Theme,
) {
    let painter = ui.painter();
    let c = rect.center();
    let r = rect.width() * 0.5;
    // Start at 12 o'clock and run clockwise, the way people read a clock.
    let mut a0 = -std::f32::consts::FRAC_PI_2;
    for (i, f) in fracs.iter().enumerate() {
        let sweep = f * std::f32::consts::TAU;
        if sweep <= 0.0 {
            continue;
        }
        let a1 = a0 + sweep;
        let lifted = hovered == Some(i);
        // Hover lift: the wedge grows a few pixels instead of changing colour,
        // so the colour keeps meaning "this system" at all times.
        let ro = if lifted { r } else { r - 4.0 };
        let ri = (r * HOLE_FRAC).min(ro - 2.0);
        let col = if lifted { s_bright(slices[i].color) } else { slices[i].color };
        painter.add(Shape::convex_polygon(
            wedge_points(c, ri, ro, a0, a1),
            col,
            Stroke::NONE,
        ));
        a0 = a1;
    }
    // Hole: cut the middle out so the centre label sits on flat colour.
    painter.circle_filled(c, r * HOLE_FRAC - 1.0, theme.bg_card());
}

/// Points of one wedge: outer arc forward, inner arc back.
fn wedge_points(c: Pos2, ri: f32, ro: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let steps = (((a1 - a0).abs().to_degrees() / SEG_DEG).ceil() as usize).max(2);
    let mut pts = Vec::with_capacity(steps * 2 + 2);
    for k in 0..=steps {
        let a = a0 + (a1 - a0) * (k as f32 / steps as f32);
        pts.push(c + Vec2::new(a.cos(), a.sin()) * ro);
    }
    for k in (0..=steps).rev() {
        let a = a0 + (a1 - a0) * (k as f32 / steps as f32);
        pts.push(c + Vec2::new(a.cos(), a.sin()) * ri);
    }
    pts
}

/// Lighten a slice for the hover highlight. Computed from the slice's own
/// theme colour, so it follows whatever palette the operator sets.
fn s_bright(c: Color32) -> Color32 {
    c.gamma_multiply(1.35)
}

/// Which wedge is under `p`? `None` when the pointer is in the hole, outside
/// the circle, or the pie is empty.
pub fn slice_at(center: Pos2, radius: f32, p: Pos2, fracs: &[f32]) -> Option<usize> {
    let d = p - center;
    let len = d.length();
    if len > radius || len < radius * HOLE_FRAC {
        return None;
    }
    // Angle measured the same way the wedges are drawn: 0 at 12 o'clock,
    // increasing clockwise, wrapped into 0..TAU.
    let mut a = d.y.atan2(d.x) + std::f32::consts::FRAC_PI_2;
    while a < 0.0 {
        a += std::f32::consts::TAU;
    }
    while a >= std::f32::consts::TAU {
        a -= std::f32::consts::TAU;
    }
    let mut acc = 0.0;
    for (i, f) in fracs.iter().enumerate() {
        acc += f * std::f32::consts::TAU;
        if a <= acc {
            return Some(i);
        }
    }
    fracs.len().checked_sub(1)
}

/// Bytes as a short human string ("612 MB"), for slice values.
pub fn fmt_bytes(b: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{b:.0} B")
    }
}

/// Milliseconds as a short string, with enough precision to see a sub-ms pass.
pub fn fmt_ms(ms: f64) -> String {
    if ms >= 10.0 {
        format!("{ms:.1} ms")
    } else if ms >= 1.0 {
        format!("{ms:.2} ms")
    } else {
        format!("{ms:.3} ms")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(v: f64) -> PieSlice {
        PieSlice::new("s", v, "x", Color32::TRANSPARENT)
    }

    #[test]
    fn fractions_sum_to_one_and_ignore_negatives() {
        let s = vec![slice(1.0), slice(3.0), slice(-5.0)];
        let f = fractions(&s);
        assert_eq!(f.len(), 3);
        let total: f32 = f.iter().sum();
        assert!((total - 1.0).abs() < 1e-5, "fractions summed to {total}");
        assert_eq!(f[2], 0.0, "a negative value must not draw a wedge");
    }

    #[test]
    fn an_all_zero_pie_reports_empty_rather_than_dividing_by_zero() {
        assert!(fractions(&[slice(0.0), slice(0.0)]).is_empty());
    }

    /// The hover hit-test must agree with the drawing: wedge 0 starts at 12
    /// o'clock and runs clockwise. On a half-and-half pie that puts the RIGHT
    /// side (12 to 6 going clockwise) in slice 0 and the LEFT side in slice 1.
    /// Screen y grows downward, so 6 o'clock is y + r.
    #[test]
    fn hit_test_matches_the_clockwise_drawing_order() {
        let c = Pos2::new(100.0, 100.0);
        let r = 50.0;
        let fracs = vec![0.5, 0.5];
        let up = Pos2::new(100.0, 100.0 - r * 0.8);
        let right = Pos2::new(100.0 + r * 0.8, 100.0);
        let left = Pos2::new(100.0 - r * 0.8, 100.0);
        assert_eq!(slice_at(c, r, up, &fracs), Some(0), "12 o'clock starts slice 0");
        assert_eq!(slice_at(c, r, right, &fracs), Some(0), "3 o'clock is still slice 0");
        assert_eq!(slice_at(c, r, left, &fracs), Some(1), "9 o'clock is slice 1");
    }

    #[test]
    fn hit_test_ignores_the_hole_and_the_outside() {
        let c = Pos2::new(0.0, 0.0);
        let r = 50.0;
        let fracs = vec![1.0];
        assert_eq!(slice_at(c, r, Pos2::new(0.0, 0.0), &fracs), None, "hole");
        assert_eq!(slice_at(c, r, Pos2::new(90.0, 0.0), &fracs), None, "outside");
        assert_eq!(slice_at(c, r, Pos2::new(0.0, -45.0), &fracs), Some(0));
    }

    #[test]
    fn a_wedge_is_a_closed_ring_segment() {
        let pts = wedge_points(Pos2::new(0.0, 0.0), 10.0, 20.0, 0.0, 1.0);
        assert!(pts.len() >= 6, "arc needs enough points to look round");
        // First point on the outer radius, last on the inner one.
        assert!((pts[0].to_vec2().length() - 20.0).abs() < 0.01);
        assert!((pts[pts.len() - 1].to_vec2().length() - 10.0).abs() < 0.01);
    }

    #[test]
    fn byte_and_millisecond_formatting_stays_short() {
        assert_eq!(fmt_bytes(0.0), "0 B");
        assert_eq!(fmt_bytes(1536.0), "2 KB");
        assert_eq!(fmt_bytes(700.0 * 1024.0 * 1024.0), "700 MB");
        assert_eq!(fmt_bytes(2.0 * 1024.0 * 1024.0 * 1024.0), "2.00 GB");
        assert_eq!(fmt_ms(0.25), "0.250 ms");
        assert_eq!(fmt_ms(16.7), "16.7 ms");
    }
}

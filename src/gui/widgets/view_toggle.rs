//! Table or cards: one list, two ways to see it (operator, 2026-10-30, of the Home page's
//! animals: "we could add a multiview list to it. Like an excel spreadsheet for compact detail and
//! tiles/cards for each animal in a more visually friendly style").
//!
//! [`show`] draws the toggle: two joined segments, each a small painted picture (rows for the
//! table, four tiles for the cards: shapes, never font glyphs, so they cannot draw as boxes) and
//! its word, the chosen one filled in the accent. [`remembered`] keeps the choice for a list by
//! its id while the app runs, the way a section's open or closed state is kept. Any list can use
//! it: the page draws its table (`widgets::data_table`) or its cards (`widgets::cards::Grid`) by
//! the answer.
//!
//! The words and hints are keys (`common.view_table`, `common.view_cards` and their `_hint`s).
//! The website's twin is `web/shared/view-toggle.js` with `.view-toggle` in
//! `web/shared/list-views.css`.

use egui::{Color32, Id, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use serde::{Deserialize, Serialize};

use super::hint::Hint;
use crate::gui::i18n::t;
use crate::gui::theme::Theme;

/// How a list is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListView {
    /// A card each, easy on the eye.
    #[default]
    Cards,
    /// A row each, its facts in columns, to compare at a glance.
    Table,
}

impl ListView {
    /// Both, in the order the toggle draws them.
    pub const ALL: [ListView; 2] = [ListView::Table, ListView::Cards];

    /// Its word on the toggle.
    pub fn word(self) -> &'static str {
        match self {
            ListView::Table => t("common.view_table"),
            ListView::Cards => t("common.view_cards"),
        }
    }

    /// The key of its hover hint.
    pub fn hint_key(self) -> &'static str {
        match self {
            ListView::Table => "common.view_table_hint",
            ListView::Cards => "common.view_cards_hint",
        }
    }
}

/// The toggle. `view` is changed when the other segment is clicked; returns whether it was.
pub fn show(ui: &mut Ui, theme: &Theme, id_salt: impl std::hash::Hash, view: &mut ListView) -> bool {
    let base = Id::new(("hos_view_toggle", id_salt));
    let size = theme.font_size_small;
    let pad = Vec2::new(theme.spacing_sm, theme.spacing_xs);
    let icon = size;
    // Measure both segments first, so the outline can be drawn round the pair. Laid out in the
    // placeholder colour so each is painted in its segment's ink (an explicit colour here would
    // win over the ink: the chosen word drew grey on the accent).
    let galleys: Vec<_> = ListView::ALL
        .iter()
        .map(|v| ui.painter().layout_no_wrap(v.word().to_string(), egui::FontId::proportional(size), Color32::PLACEHOLDER))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| pad.x * 2.0 + icon + theme.spacing_xs + g.size().x).collect();
    let height = (size + pad.y * 2.0).max(theme.compact_button_height);
    let (outer, _) = ui.allocate_exact_size(Vec2::new(widths.iter().sum(), height), Sense::hover());
    let radius = theme.pill_radius.min(height / 2.0).round().clamp(0.0, 255.0) as u8;
    let painter = ui.painter().clone();
    painter.rect_filled(outer, egui::CornerRadius::same(radius), theme.bg_secondary());
    let mut changed = false;
    let mut x = outer.left();
    for (k, v) in ListView::ALL.iter().copied().enumerate() {
        let rect = Rect::from_min_size(Pos2::new(x, outer.top()), Vec2::new(widths[k], height));
        x += widths[k];
        let resp = ui.interact(rect, base.with(k), Sense::click());
        let resp = resp.hint_dyn(v.hint_key());
        let on = *view == v;
        let corners = if k == 0 {
            egui::CornerRadius { nw: radius, sw: radius, ne: 0, se: 0 }
        } else {
            egui::CornerRadius { nw: 0, sw: 0, ne: radius, se: radius }
        };
        if on {
            painter.rect_filled(rect, corners, theme.accent());
        } else if resp.hovered() {
            painter.rect_filled(rect, corners, theme.bg_tertiary());
        }
        let ink = if on { theme.text_on_accent() } else if resp.hovered() { theme.text_primary() } else { theme.text_secondary() };
        let icon_rect = Rect::from_min_size(Pos2::new(rect.left() + pad.x, rect.center().y - icon / 2.0), Vec2::splat(icon));
        paint_icon(&painter, v, icon_rect, ink);
        let text_pos = Pos2::new(icon_rect.right() + theme.spacing_xs, rect.center().y - galleys[k].size().y / 2.0);
        painter.galley(text_pos, galleys[k].clone(), ink);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, v.word()));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && !on {
            *view = v;
            changed = true;
        }
    }
    painter.rect_stroke(outer, egui::CornerRadius::same(radius), Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
    changed
}

/// The toggle with its choice kept by `id_salt` while the app runs (`default` until one is
/// picked). Returns the view to draw.
pub fn remembered(ui: &mut Ui, theme: &Theme, id_salt: impl std::hash::Hash + Copy, default: ListView) -> ListView {
    let id = memory_id(id_salt);
    let mut view = ui.data(|d| d.get_temp::<ListView>(id)).unwrap_or(default);
    if show(ui, theme, id_salt, &mut view) {
        ui.data_mut(|d| d.insert_temp(id, view));
    }
    view
}

/// Set the view kept by `id_salt` (a snapshot shows a list as a table).
pub fn set_remembered(ctx: &egui::Context, id_salt: impl std::hash::Hash, view: ListView) {
    ctx.data_mut(|d| d.insert_temp(memory_id(id_salt), view));
}

fn memory_id(id_salt: impl std::hash::Hash) -> Id {
    Id::new(("hos_view_toggle_view", id_salt))
}

/// The view's small picture in `rect`: three rows under a header line for the table, four tiles
/// for the cards.
fn paint_icon(painter: &egui::Painter, view: ListView, rect: Rect, ink: Color32) {
    let r = rect.shrink(rect.width() * 0.08);
    match view {
        ListView::Table => {
            let stroke = Stroke::new(1.0, ink);
            painter.rect_stroke(r, egui::CornerRadius::same(1), stroke, egui::StrokeKind::Inside);
            for k in 1..4 {
                let y = r.top() + r.height() * k as f32 / 4.0;
                painter.hline(r.x_range(), y, stroke);
            }
            painter.vline(r.left() + r.width() * 0.38, r.y_range(), stroke);
        }
        ListView::Cards => {
            let gap = (r.width() * 0.14).max(1.0);
            let s = (r.width() - gap) / 2.0;
            for (i, j) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let tile = Rect::from_min_size(Pos2::new(r.left() + i * (s + gap), r.top() + j * (s + gap)), Vec2::splat(s));
                painter.rect_filled(tile, egui::CornerRadius::same(1), ink);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE TOGGLE WORKS, not only shows (the "shows != works" trap, docs/design/ui-system.md): a
    /// synthetic click on the Table segment, in separate frames as egui needs, turns the cards into
    /// a table and the remembered view says so; a click on the segment already chosen changes
    /// nothing.
    #[test]
    fn a_click_on_table_shows_the_table() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
        let mut rect = Rect::NOTHING;
        let mut seen = ListView::Cards;
        let mut frame = |events: Vec<egui::Event>, rect: &mut Rect, seen: &mut ListView| {
            let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Away from the top-left corner: the first frame a hint shows, egui measures its
                    // tooltip at the screen's origin, and a toggle drawn there was under it (the
                    // press then went to the tooltip; seen 2026-10-30, a test artifact, not the app's).
                    ui.add_space(150.0);
                    let top = ui.cursor().min;
                    *seen = remembered(ui, &theme, "test_list", ListView::Cards);
                    *rect = Rect::from_min_size(top, Vec2::new(20.0, 20.0));
                });
            });
        };
        frame(Vec::new(), &mut rect, &mut seen);
        frame(Vec::new(), &mut rect, &mut seen);
        assert_eq!(seen, ListView::Cards, "cards until a click");
        // The Table segment is first: a point just inside its left end.
        let at = rect.left_center() + Vec2::new(8.0, 0.0);
        let m = egui::Modifiers::default();
        frame(vec![egui::Event::PointerMoved(at)], &mut rect, &mut seen);
        frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], &mut rect, &mut seen);
        frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], &mut rect, &mut seen);
        frame(Vec::new(), &mut rect, &mut seen);
        assert_eq!(seen, ListView::Table, "a click on Table shows the table");
        // Again on the same segment: still the table.
        frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], &mut rect, &mut seen);
        frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], &mut rect, &mut seen);
        frame(Vec::new(), &mut rect, &mut seen);
        assert_eq!(seen, ListView::Table);
    }
}

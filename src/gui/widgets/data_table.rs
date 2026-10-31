//! A compact table, spreadsheet-like (operator, 2026-10-30: "Like an excel spreadsheet for compact
//! detail"): a heading row, then one striped row per thing, every column the same width from row to
//! row. The columns share the width there is by their weights, none narrower than its minimum; when
//! even the minimums do not fit, the table scrolls sideways rather than squeezing a column to
//! nothing. A cell's words are cut to one line ("...") and shown whole on hover by the page's cell
//! closure ([`text_cell`] does both).
//!
//! The other half of `widgets::view_toggle` (the cards being `widgets::cards::Grid`). Every colour
//! and size from the theme. The website's twin is `.data-table` in `web/shared/list-views.css`.

use egui::{Color32, RichText, Stroke, Ui, Vec2};

use super::cover_tile::fixed_cell;
use crate::gui::theme::Theme;

/// One column: its heading, its share of spare width, and the narrowest it may be.
#[derive(Debug, Clone, Copy)]
pub struct Column<'a> {
    pub title: &'a str,
    pub weight: f32,
    pub min: f32,
}

impl<'a> Column<'a> {
    pub fn new(title: &'a str, weight: f32, min: f32) -> Self {
        Self { title, weight, min }
    }
}

/// The columns' widths in `width`: each its minimum, then the spare width shared by weight. When
/// the minimums alone are wider than `width`, the minimums (the table then scrolls).
pub fn widths(width: f32, columns: &[Column]) -> Vec<f32> {
    let mins: f32 = columns.iter().map(|c| c.min).sum();
    let spare = (width - mins).max(0.0);
    let weights: f32 = columns.iter().map(|c| c.weight.max(0.0)).sum();
    columns
        .iter()
        .map(|c| c.min + if weights > 0.0 { spare * c.weight.max(0.0) / weights } else { 0.0 })
        .map(|w| w.floor())
        .collect()
}

/// The table: `columns` heads, `rows` rows, `cell(ui, row, column)` drawing each cell (a
/// left-to-right Ui, centred on the row's line, clipped to the cell). `id_salt` keeps its
/// sideways scroll apart from another table's.
pub fn show(ui: &mut Ui, theme: &Theme, id_salt: impl std::hash::Hash, columns: &[Column], rows: usize, mut cell: impl FnMut(&mut Ui, usize, usize)) {
    let room = ui.available_width();
    let ws = widths(room, columns);
    let total: f32 = ws.iter().sum();
    let pad = theme.spacing_sm;
    let row_h = (theme.font_size_body + theme.spacing_md).max(theme.row_height);
    let head_h = theme.font_size_small + theme.spacing_md;
    let mut draw = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        // The heading row and the line under it.
        ui.horizontal(|ui| {
            for (c, w) in columns.iter().zip(&ws) {
                fixed_cell(ui, *w, head_h, |ui| {
                    ui.add_space(pad);
                    ui.add(egui::Label::new(RichText::new(c.title).size(theme.font_size_small).strong().color(theme.text_secondary())).truncate());
                });
            }
        });
        let y = ui.cursor().min.y;
        ui.painter().hline(ui.cursor().min.x..=ui.cursor().min.x + total, y, Stroke::new(1.0, theme.border()));
        ui.add_space(theme.spacing_xs);
        let saved_y = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        for r in 0..rows {
            super::shelf::list_row(ui, theme, r, total, row_h, false, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (k, w) in ws.iter().enumerate() {
                    fixed_cell(ui, *w, row_h, |ui| {
                        ui.add_space(pad);
                        cell(ui, r, k);
                    });
                }
            });
        }
        ui.spacing_mut().item_spacing.y = saved_y;
    };
    if total > room + 0.5 {
        egui::ScrollArea::horizontal().id_salt(("hos_data_table", id_salt)).auto_shrink([false, true]).show(ui, |ui| draw(ui));
    } else {
        draw(ui);
    }
}

/// A cell's words on one line, cut with "..." when they do not fit, whole on hover when cut.
pub fn text_cell(ui: &mut Ui, theme: &Theme, text: &str, color: Color32, strong: bool) {
    let mut rich = RichText::new(text).size(theme.font_size_small).color(color);
    if strong {
        rich = rich.strong();
    }
    let resp = ui.add(egui::Label::new(rich).truncate());
    if resp.rect.width() + 0.5 < ui.fonts(|f| f.layout_no_wrap(text.to_string(), egui::FontId::proportional(theme.font_size_small), color).size().x) {
        resp.on_hover_ui(|ui| {
            ui.set_max_width(theme.modal_width.max(320.0));
            ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_primary()));
        });
    }
}

/// A small dot in `color` before a cell's words (a row's state at a glance).
pub fn dot(ui: &mut Ui, theme: &Theme, color: Color32) {
    let d = theme.status_dot_size.max(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(d + theme.spacing_xs, d), egui::Sense::hover());
    ui.painter().circle_filled(egui::pos2(rect.left() + d / 2.0, rect.center().y), d / 2.0, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE COLUMNS SHARE THE WIDTH BY WEIGHT, none under its minimum: 600 points over minimums of
    /// 100 + 100 + 200 leaves 200, shared 1 : 3 : 0; with too little room each column keeps its
    /// minimum (and the table scrolls).
    #[test]
    fn columns_share_the_width() {
        let cols = [Column::new("a", 1.0, 100.0), Column::new("b", 3.0, 100.0), Column::new("c", 0.0, 200.0)];
        assert_eq!(widths(600.0, &cols), vec![150.0, 250.0, 200.0]);
        assert_eq!(widths(300.0, &cols), vec![100.0, 100.0, 200.0], "the minimums, and a sideways scroll");
    }
}

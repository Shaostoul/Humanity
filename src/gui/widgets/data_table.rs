//! Universal data table widget.
//! Sortable columns, selectable rows, scrollable body.
//! Used by: inventory, market, tasks, recipes, skills, resources.

use egui::{Color32, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// Column alignment.
#[derive(Clone, Copy, Debug, Default)]
pub enum ColumnAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Column definition for a data table.
pub struct Column {
    pub header: String,
    /// Width in pixels. 0.0 means auto-fill remaining space.
    pub width: f32,
    pub sortable: bool,
    pub align: ColumnAlign,
}

/// Current sort state.
pub struct SortState {
    pub column: usize,
    pub ascending: bool,
}

/// A single cell value.
pub enum CellValue {
    Text(String),
    Number(f64),
    /// Progress fraction 0.0..=1.0, rendered as a mini bar.
    Progress(f32),
    /// Colored pill badge.
    Badge(String, Color32),
}

/// Render a data table with sortable headers, selectable rows, and scroll.
///
/// Returns the index of a row that was clicked this frame, if any.
/// `sort` is updated in-place when a sortable header is clicked.
/// `selected_row` is updated in-place when a row is clicked.
pub fn data_table(
    ui: &mut Ui,
    theme: &Theme,
    columns: &[Column],
    rows: &[Vec<CellValue>],
    sort: &mut SortState,
    selected_row: &mut Option<usize>,
    max_visible_rows: usize,
) -> Option<usize> {
    let mut clicked_row: Option<usize> = None;
    let rounding = Rounding::same(theme.border_radius as u8);
    let row_h = theme.row_height + theme.item_padding * 2.0;
    let available_w = ui.available_width();

    // Resolve column widths: fixed columns keep their width, zero-width columns share the rest.
    let fixed_total: f32 = columns.iter().map(|c| if c.width > 0.0 { c.width } else { 0.0 }).sum();
    let auto_count = columns.iter().filter(|c| c.width <= 0.0).count().max(1) as f32;
    let auto_width = ((available_w - fixed_total) / auto_count).max(40.0);

    let col_widths: Vec<f32> = columns
        .iter()
        .map(|c| if c.width > 0.0 { c.width } else { auto_width })
        .collect();

    // ---- Header row ----
    ui.horizontal(|ui| {
        for (i, col) in columns.iter().enumerate() {
            let w = col_widths[i];
            let is_sorted = sort.column == i;
            let arrow = if !col.sortable {
                ""
            } else if is_sorted && sort.ascending {
                " \u{25B2}"
            } else if is_sorted {
                " \u{25BC}"
            } else {
                ""
            };

            let label = format!("{}{}", col.header, arrow);
            let text_color = if is_sorted { theme.accent() } else { theme.text_secondary() };
            let text = RichText::new(label).size(theme.font_size_small).color(text_color);

            let layout = match col.align {
                ColumnAlign::Left => egui::Layout::left_to_right(egui::Align::Center),
                ColumnAlign::Center => egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                ColumnAlign::Right => egui::Layout::right_to_left(egui::Align::Center),
            };

            ui.allocate_ui_with_layout(Vec2::new(w, row_h), layout, |ui| {
                let resp = ui.label(text);
                if col.sortable && resp.clicked() {
                    if sort.column == i {
                        sort.ascending = !sort.ascending;
                    } else {
                        sort.column = i;
                        sort.ascending = true;
                    }
                }
            });
        }
    });

    // Separator
    ui.add(egui::Separator::default().spacing(0.0));

    // ---- Body (scrollable) ----
    let scroll_h = row_h * max_visible_rows as f32;
    egui::ScrollArea::vertical()
        .max_height(scroll_h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (row_idx, row) in rows.iter().enumerate() {
                let is_selected = *selected_row == Some(row_idx);
                let bg = if is_selected {
                    theme.accent_pressed()
                } else if row_idx % 2 == 0 {
                    theme.bg_primary()
                } else {
                    theme.bg_secondary()
                };

                let (full_rect, response) =
                    ui.allocate_exact_size(Vec2::new(available_w, row_h), Sense::click());

                if ui.is_rect_visible(full_rect) {
                    let painter = ui.painter();

                    // Hover highlight
                    let fill = if response.hovered() && !is_selected {
                        Theme::c32(&theme.bg_tertiary)
                    } else {
                        bg
                    };
                    painter.rect_filled(full_rect, Rounding::ZERO, fill);

                    // Draw cells
                    let mut x = full_rect.min.x;
                    for (col_idx, cell) in row.iter().enumerate() {
                        let w = col_widths.get(col_idx).copied().unwrap_or(auto_width);
                        let cell_rect = Rect::from_min_size(
                            egui::pos2(x, full_rect.min.y),
                            Vec2::new(w, row_h),
                        );
                        let align = columns.get(col_idx).map(|c| c.align).unwrap_or_default();
                        draw_cell(painter, theme, cell_rect, cell, align);
                        x += w;
                    }
                }

                if response.clicked() {
                    *selected_row = Some(row_idx);
                    clicked_row = Some(row_idx);
                }
            }
        });

    clicked_row
}

/// Draw a single cell value into the given rect.
fn draw_cell(
    painter: &egui::Painter,
    theme: &Theme,
    rect: Rect,
    cell: &CellValue,
    align: ColumnAlign,
) {
    let pad = theme.item_padding;
    let inner = Rect::from_min_max(
        egui::pos2(rect.min.x + pad, rect.min.y),
        egui::pos2(rect.max.x - pad, rect.max.y),
    );

    match cell {
        CellValue::Text(s) => {
            let anchor = match align {
                ColumnAlign::Left => egui::Align2::LEFT_CENTER,
                ColumnAlign::Center => egui::Align2::CENTER_CENTER,
                ColumnAlign::Right => egui::Align2::RIGHT_CENTER,
            };
            let pos = match align {
                ColumnAlign::Left => egui::pos2(inner.min.x, inner.center().y),
                ColumnAlign::Center => inner.center(),
                ColumnAlign::Right => egui::pos2(inner.max.x, inner.center().y),
            };
            painter.text(
                pos,
                anchor,
                s,
                egui::FontId::proportional(theme.font_size_body),
                theme.text_primary(),
            );
        }
        CellValue::Number(n) => {
            let text = if n.fract().abs() < f64::EPSILON {
                format!("{}", *n as i64)
            } else {
                format!("{:.2}", n)
            };
            let anchor = match align {
                ColumnAlign::Left => egui::Align2::LEFT_CENTER,
                ColumnAlign::Center => egui::Align2::CENTER_CENTER,
                ColumnAlign::Right => egui::Align2::RIGHT_CENTER,
            };
            let pos = match align {
                ColumnAlign::Left => egui::pos2(inner.min.x, inner.center().y),
                ColumnAlign::Center => inner.center(),
                ColumnAlign::Right => egui::pos2(inner.max.x, inner.center().y),
            };
            painter.text(
                pos,
                anchor,
                &text,
                egui::FontId::proportional(theme.font_size_body),
                theme.text_primary(),
            );
        }
        CellValue::Progress(frac) => {
            let bar_h = 6.0;
            let bar_y = inner.center().y - bar_h / 2.0;
            let track = Rect::from_min_size(
                egui::pos2(inner.min.x, bar_y),
                Vec2::new(inner.width(), bar_h),
            );
            painter.rect_filled(track, Rounding::same(3), theme.bg_secondary());
            let fill_w = inner.width() * frac.clamp(0.0, 1.0);
            if fill_w > 0.0 {
                let fill = Rect::from_min_size(
                    egui::pos2(inner.min.x, bar_y),
                    Vec2::new(fill_w, bar_h),
                );
                painter.rect_filled(fill, Rounding::same(3), theme.accent());
            }
        }
        CellValue::Badge(text, color) => {
            let font = egui::FontId::proportional(theme.font_size_small);
            let galley = painter.layout_no_wrap(text.clone(), font, Color32::WHITE);
            let pill_w = galley.size().x + 8.0;
            let pill_h = galley.size().y + 4.0;
            let pill_rect = Rect::from_center_size(
                inner.center(),
                Vec2::new(pill_w, pill_h),
            );
            painter.rect_filled(pill_rect, Rounding::same(3), *color);
            painter.galley(
                egui::pos2(pill_rect.min.x + 4.0, pill_rect.min.y + 2.0),
                galley,
                Color32::WHITE,
            );
        }
    }
}

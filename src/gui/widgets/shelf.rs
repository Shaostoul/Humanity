//! A library shelf (2026-10-19): the parts the Apps page (pages/apps.rs) draws for each of its
//! parts, so they look and behave alike and a fix to one reaches all. The Games page drew these
//! first; they were lifted out when the Tools page became a library like it, and the two pages
//! became the one Apps page the same day. A tile's card is snug (its margin the theme's
//! spacing_xs plus a pixel): the Apps page's tiles are short, the operator's "remove some
//! superfluousness/spacing".
//!
//! - [`Layout`]: Grid or List, kept in the settings by each page (its own field).
//! - [`toolbar`]: the search field, the Sort pills and the Show (Grid or List) pills, on one line.
//! - [`grid`] and [`tile_frame`]: as many tiles to a row as fit, each stretched so the rows fill the
//!   width, each in a card that lights its edge on hover and shows the accent when chosen.
//! - [`list_headings`], [`list_row`] and [`columns_that_fit`]: a List's heading row (a click on a
//!   heading that sorts sorts by it), its striped rows, and which of its columns fit the width.
//! - [`details_panel`], [`section_heading`] and [`facts`]: the details at the right of the shelf,
//!   their section headings, and their label-and-value rows.
//!
//! The covers and the small pieces (a cover slot, a thin bar, pills, a fixed table cell) are in
//! cover_tile.rs. Every colour and size comes from the theme. The Apps page is desktop-only; the web
//! Tools page mirrors this shelf in HTML (web/pages/tools-app.js, since v0.1521.1).

use egui::{Rect, RichText, Sense, Stroke, Ui, Vec2};
use serde::{Deserialize, Serialize};

use super::cover_tile;
use crate::gui::theme::Theme;

/// How a shelf is shown, kept in the settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// A tile per thing.
    #[default]
    Grid,
    /// A row per thing, its facts in columns.
    List,
}

/// The views' names on their pills.
pub const GRID: &str = "Grid";
pub const LIST: &str = "List";

/// What the toolbar's pills picked this frame (each `None` when that pill row was not clicked).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Picked<S> {
    pub sort: Option<S>,
    pub layout: Option<Layout>,
}

/// The toolbar's line: the search field (`hint` in it while empty), "Sort" and one pill per
/// `sorts` entry (its value, its words, its hover words), "Show" and the Grid and List pills
/// (`layout_tips`, their hover words). Returns what a click picked; the page keeps it.
#[allow(clippy::too_many_arguments)]
pub fn toolbar<S: Copy + PartialEq>(ui: &mut Ui, theme: &Theme, search: &mut String, hint: &str, sorts: &[(S, &str, &str)], sort: S, layout: Layout, layout_tips: [&str; 2]) -> Picked<S> {
    let mut picked = Picked { sort: None, layout: None };
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(search).hint_text(hint).desired_width(220.0));
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Sort").size(theme.font_size_small).color(theme.text_secondary()));
        let words: Vec<String> = sorts.iter().map(|(_, w, _)| w.to_string()).collect();
        let tips: Vec<&str> = sorts.iter().map(|(_, _, t)| *t).collect();
        let chosen = sorts.iter().position(|(s, _, _)| *s == sort).unwrap_or(0);
        if let Some(i) = cover_tile::choice_pills(ui, theme, &words, chosen, &tips) {
            picked.sort = Some(sorts[i].0);
        }
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Show").size(theme.font_size_small).color(theme.text_secondary()));
        let layouts = [Layout::Grid, Layout::List];
        let names = vec![GRID.to_string(), LIST.to_string()];
        let chosen = layouts.iter().position(|l| *l == layout).unwrap_or(0);
        if let Some(i) = cover_tile::choice_pills(ui, theme, &names, chosen, &layout_tips) {
            picked.layout = Some(layouts[i]);
        }
    });
    picked
}

/// The grid: `items` (whatever the page indexes by) as many to a row as fit at `min_tile_w` or
/// wider, each stretched so the rows fill the width; `tile` draws one at the width given.
pub fn grid(ui: &mut Ui, theme: &Theme, items: &[usize], min_tile_w: f32, mut tile: impl FnMut(&mut Ui, usize, f32)) {
    let gap = theme.spacing_md;
    let width = ui.available_width();
    let per_row = ((width + gap) / (min_tile_w + gap)).floor().max(1.0) as usize;
    let tile_w = ((width - gap * (per_row as f32 - 1.0)) / per_row as f32).floor();
    for row in items.chunks(per_row) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for &i in row {
                tile(ui, i, tile_w);
            }
        });
        ui.add_space(gap);
    }
}

/// One tile's card, `width` wide: the card colour, its edge in the accent when `selected`, lit on
/// hover. `add` draws its contents top to bottom at the inner width it is given.
pub fn tile_frame(ui: &mut Ui, theme: &Theme, width: f32, selected: bool, add: impl FnOnce(&mut Ui, f32)) {
    let margin = theme.spacing_xs + 1.0;
    let inner = egui::Frame::none()
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, if selected { theme.accent() } else { theme.border() }))
        .rounding(egui::Rounding::same(theme.border_radius_lg as u8))
        .inner_margin(margin)
        .show(ui, |ui| {
            let w = width - 2.0 * margin - 2.0;
            ui.set_width(w);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = theme.spacing_xs;
                add(ui, w);
            });
        });
    // Hover: the tile's edge lights up.
    let rect = inner.response.rect;
    if !selected && ui.rect_contains_pointer(rect) {
        ui.painter().rect_stroke(rect, egui::Rounding::same(theme.border_radius_lg as u8), Stroke::new(1.0, theme.accent_hover()), egui::StrokeKind::Inside);
    }
}

/// One cell of a List's heading row.
#[derive(Debug, Clone, Copy)]
pub struct Heading<'a> {
    /// Its words (none for a cell over buttons).
    pub text: &'a str,
    pub width: f32,
    /// Whether a click on it sorts by it.
    pub sorts: bool,
    /// Whether the list is sorted by it now (drawn in the accent).
    pub on: bool,
}

/// A List's heading row and the line under it. Returns the cell clicked, when it sorts.
pub fn list_headings(ui: &mut Ui, theme: &Theme, cells: &[Heading]) -> Option<usize> {
    let head_h = theme.font_size_small + theme.spacing_md;
    let mut clicked = None;
    ui.horizontal(|ui| {
        for (i, cell) in cells.iter().enumerate() {
            cover_tile::fixed_cell(ui, cell.width, head_h, |ui| {
                if cell.text.is_empty() {
                    return;
                }
                let color = if cell.on { theme.accent() } else { theme.text_muted() };
                let label = egui::Label::new(RichText::new(cell.text).size(theme.font_size_small).color(color)).truncate();
                if cell.sorts {
                    if ui.add(label.sense(Sense::click())).on_hover_text("Sort by this").clicked() {
                        clicked = Some(i);
                    }
                } else {
                    ui.add(label);
                }
            });
        }
    });
    let line = ui.cursor().min.y;
    ui.painter().hline(ui.max_rect().x_range(), line, Stroke::new(1.0, theme.border()));
    ui.add_space(theme.spacing_xs);
    clicked
}

/// One row of a List, `width` by `row_h`, its cells laid left to right on its middle line: striped
/// every other row (`n` is its place), lighter under the pointer, in the accent when `selected`.
pub fn list_row(ui: &mut Ui, theme: &Theme, n: usize, width: f32, row_h: f32, selected: bool, add: impl FnOnce(&mut Ui)) {
    let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(width, row_h));
    let fill = if selected {
        Some(theme.accent().gamma_multiply(0.16))
    } else if ui.rect_contains_pointer(rect) {
        Some(theme.row_stripe().gamma_multiply(1.6))
    } else if n % 2 == 1 {
        Some(theme.row_stripe())
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, egui::Rounding::same(theme.border_radius as u8), fill);
    }
    ui.allocate_ui_with_layout(Vec2::new(width, row_h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_height(row_h);
        add(ui);
    });
}

/// The columns that fit in `room`: `wanted` taken most wanted first (each with its width) while
/// they fit, then returned in the order `drawn` gives.
pub fn columns_that_fit<C: Copy + PartialEq>(room: f32, wanted: &[(C, f32)], drawn: &[C]) -> Vec<C> {
    let mut room = room;
    let mut fit: Vec<C> = Vec::new();
    for &(c, w) in wanted {
        if room >= w {
            room -= w;
            fit.push(c);
        }
    }
    drawn.iter().copied().filter(|c| fit.contains(c)).collect()
}

/// The details, at the right of the shelf: resizable (from `default_width`), the card colour with
/// a border, scrolling, and their contents kept clear of the scroll bar (which floats over the
/// right edge of what it scrolls). Returns the scrolling area's own rectangle (its right edge is
/// where the scroll bar is).
pub fn details_panel(ctx: &egui::Context, theme: &Theme, id: &str, default_width: f32, add: impl FnOnce(&mut Ui)) -> Rect {
    egui::SidePanel::right(id.to_string())
        .resizable(true)
        .default_width(default_width)
        .width_range(340.0..=760.0)
        .frame(egui::Frame::none().fill(theme.bg_card()).inner_margin(theme.spacing_lg).stroke(Stroke::new(1.0, theme.border())))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(format!("{id}_scroll"))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // The room the scroll bar takes when the pointer is on it, and a little more.
                    let clear = theme.spacing_lg.round().clamp(0.0, 127.0) as i8;
                    egui::Frame::none().inner_margin(egui::Margin { right: clear, ..Default::default() }).show(ui, add);
                })
                .inner_rect
        })
        .inner
}

/// A details section's heading, a note beside it (a count, say), and a line under it.
pub fn section_heading(ui: &mut Ui, theme: &Theme, text: &str, note: Option<&str>) {
    ui.add_space(theme.spacing_md);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(text).size(theme.font_size_heading).color(theme.text_primary()));
        if let Some(note) = note {
            ui.label(RichText::new(note).size(theme.font_size_body).color(theme.text_secondary()));
        }
    });
    let y = ui.cursor().min.y + 1.0;
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme.border()));
    ui.add_space(theme.spacing_sm);
}

/// Label-and-value rows (a long value wraps beside its label). `id` keeps the grid's own state
/// apart from another's.
pub fn facts(ui: &mut Ui, theme: &Theme, id: impl std::hash::Hash, rows: &[(&str, String)]) {
    egui::Grid::new(id).num_columns(2).spacing(Vec2::new(theme.spacing_md, theme.spacing_xs)).show(ui, |ui| {
        for (label, value) in rows {
            ui.label(RichText::new(*label).size(theme.font_size_small).color(theme.text_muted()));
            ui.add(egui::Label::new(RichText::new(value).size(theme.font_size_small).color(theme.text_primary())).wrap());
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE COLUMNS THAT FIT: taken most wanted first while there is room, returned in the order
    /// drawn; the least wanted give way first; no room, none.
    ///
    /// Seen red 2026-10-19 with the columns returned in the order taken (`fit` returned as is):
    /// "in the order drawn" got [B, A] for [A, B].
    #[test]
    fn the_columns_that_fit() {
        #[derive(Debug, Clone, Copy, PartialEq)]
        enum C {
            A,
            B,
            Z,
        }
        let wanted = [(C::B, 100.0), (C::A, 50.0), (C::Z, 80.0)];
        let drawn = [C::A, C::B, C::Z];
        assert_eq!(columns_that_fit(150.0, &wanted, &drawn), vec![C::A, C::B], "in the order drawn");
        assert_eq!(columns_that_fit(130.0, &wanted, &drawn), vec![C::B], "the least wanted give way first");
        assert!(columns_that_fit(40.0, &wanted, &drawn).is_empty());
    }

    /// THE DETAILS KEEP CLEAR OF THE SCROLL BAR (2026-10-19, seen on the operator's computer at
    /// 2560 wide: an achievement's right-aligned date and the banner ran against the details' scroll
    /// bar): text laid out at the right of the details ends at least the theme's spacing_lg short of
    /// the scrolling area's right edge, where the scroll bar floats over it, and that is wider than
    /// the bar at its widest.
    ///
    /// A first version measured against the content's clip rectangle and stayed GREEN with the
    /// margin taken out (2026-10-19): a scroll area that scrolls only up and down clips its content
    /// at its panel's edge, past the bar, so that measure could not tell. Against the scrolling
    /// area's own edge it went red, 2026-10-19, with the margin at 0: "the right-aligned text keeps
    /// clear: a gap of 0 against 14".
    #[test]
    fn the_details_keep_clear_of_the_scroll_bar() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut right = 0.0f32;
        let mut area = Rect::NOTHING;
        for _ in 0..3 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                area = details_panel(ctx, &theme, "shelf_test_details", 440.0, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("left");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            right = ui.label("21 Sep 2026").rect.right();
                        });
                    });
                    // Long enough to scroll, so the scroll bar is there.
                    for i in 0..80 {
                        ui.label(format!("line {i}"));
                    }
                });
            });
        }
        let gap = area.right() - right;
        assert!(area.width() > 300.0, "the details are there: {area:?}");
        assert!(gap >= theme.spacing_lg - 0.5, "the right-aligned text keeps clear: a gap of {gap} against {}", theme.spacing_lg);
        assert!(gap > ctx.style().spacing.scroll.bar_width, "wider than the bar at its widest");
    }
}

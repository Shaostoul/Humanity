//! THE LEVEL PICKER (Maps increment 3, docs/design/maps-zoom-to-anything.md 5.7): a column of
//! buttons, one per level of a place with more than one, listed TOP TO BOTTOM as a building's
//! directory and a lift's panel list them, with the level you are on marked, the level shown
//! highlighted, and "Whole" for no level (the place as a whole, nothing cut away).
//!
//! The pattern is the indoor map's (Google Maps' `IndoorBuilding`: levels "are in 'display
//! order' from top to bottom", an active level and a default level; the Indoor Mapping Data
//! Format's Level, OGC 20-094, with its `ordinal` and `short_name`), and one widget serves every
//! multi-level place the map shows: a ship's levels today, a home's floors, a building's floors
//! on Earth, basements and caves when the map reaches them (increment 7).
//!
//! The rows are worked out by `picker_rows` (pure, tested); `show` draws them. The web twin is
//! `web/shared/level-picker.js`, whose `pickerRows` is the same rule.

use egui::{Pos2, Rect, RichText, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets;

/// One level as the picker is given it: its stacking position (higher is up), its short name
/// on the button ("2", "B1") and its full name in the tooltip ("Storey 2").
#[derive(Debug, Clone, PartialEq)]
pub struct PickerLevel {
    pub ordinal: i32,
    pub short: String,
    pub name: String,
}

/// One row of the picker, top to bottom: a level (its ordinal) or Whole (None).
#[derive(Debug, Clone, PartialEq)]
pub struct PickerRow {
    pub ordinal: Option<i32>,
    pub label: String,
    pub tooltip: String,
    /// The level shown (or Whole, when none is).
    pub selected: bool,
    /// The level you are on.
    pub you: bool,
}

/// The picker's rows: the levels from the top one down (whatever order they come in), then
/// Whole. `you` is the ordinal of the level you are on, `shown` the one shown (None: Whole).
pub fn picker_rows(levels: &[PickerLevel], you: Option<i32>, shown: Option<i32>) -> Vec<PickerRow> {
    let mut sorted: Vec<&PickerLevel> = levels.iter().collect();
    sorted.sort_by(|a, b| b.ordinal.cmp(&a.ordinal));
    let mut rows: Vec<PickerRow> = sorted
        .into_iter()
        .map(|l| {
            let you_here = you == Some(l.ordinal);
            PickerRow {
                ordinal: Some(l.ordinal),
                label: if you_here { format!("{}  · you", l.short) } else { l.short.clone() },
                tooltip: if you_here { format!("{} (you are on this level)", l.name) } else { l.name.clone() },
                selected: shown == Some(l.ordinal),
                you: you_here,
            }
        })
        .collect();
    rows.push(PickerRow {
        ordinal: None,
        label: "Whole".to_string(),
        tooltip: "The whole place, nothing cut away".to_string(),
        selected: shown.is_none(),
        you: false,
    });
    rows
}

/// What a click on the picker asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickerClick {
    /// Show this level (an ordinal), or the whole place (None).
    Level(Option<i32>),
    /// The straight-down plan on or off.
    Plan(bool),
}

/// The column's width, points.
pub const PICKER_WIDTH: f32 = 112.0;

/// The column's rectangle for `rows` rows with its top-right corner at `top_right` (laid out
/// before the names are placed, so they keep off it; `show` draws in it).
pub fn rect_for(theme: &Theme, top_right: Pos2, rows: usize) -> Rect {
    let row_h = 26.0;
    let height = 22.0 + row_h * (rows as f32 + 1.0) + theme.spacing_sm * 2.0;
    Rect::from_min_size(top_right - Vec2::new(PICKER_WIDTH, 0.0), Vec2::new(PICKER_WIDTH, height))
}

/// Draw the picker with its top-right corner at `top_right`: a "Plan" switch, then the rows.
/// Returns the column's rectangle (`rect_for`) and the click, if any.
pub fn show(ui: &mut egui::Ui, theme: &Theme, top_right: Pos2, title: &str, rows: &[PickerRow], plan: bool) -> (Rect, Option<PickerClick>) {
    let rect = rect_for(theme, top_right, rows.len());
    ui.painter().rect(
        rect,
        theme.border_radius,
        theme.bg_panel().gamma_multiply(0.9),
        egui::Stroke::new(theme.border_width, theme.border()),
        egui::StrokeKind::Inside,
    );
    let mut click = None;
    let inner = rect.shrink(theme.spacing_sm);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    child.spacing_mut().item_spacing.y = 2.0;
    child.label(RichText::new(title).size(theme.font_size_small).color(theme.text_secondary()).strong());
    let plan_label = if plan { "Plan: on" } else { "Plan: off" };
    if widgets::Button::secondary(plan_label)
        .active(plan)
        .tooltip("The level shown straight down, as a floor plan.")
        .show(&mut child, theme)
    {
        click = Some(PickerClick::Plan(!plan));
    }
    for row in rows {
        let button = if row.you { widgets::Button::primary(&row.label) } else { widgets::Button::secondary(&row.label) };
        if button.active(row.selected).tooltip(&row.tooltip).show(&mut child, theme) {
            click = Some(PickerClick::Level(row.ordinal));
        }
    }
    (rect, click)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(ordinal: i32) -> PickerLevel {
        PickerLevel { ordinal, short: crate::ship::levels::ShipLevel { ordinal, floor: 0.0, top: 0.0 }.short_name(), name: crate::ship::levels::level_name(ordinal) }
    }

    /// The rows read top to bottom (the highest level first, then down to the basements, then
    /// Whole), the level you are on marked "you", the level shown selected, and Whole selected
    /// when no level is.
    ///
    /// Seen red 2026-10-22 with the levels left in the order given (bottom up): left
    /// `[Some(-1), Some(0), Some(1), None]`, right `[Some(1), Some(0), Some(-1), None]`.
    #[test]
    fn the_rows_read_top_to_bottom_with_you_marked() {
        let levels = [level(-1), level(0), level(1)];
        let rows = picker_rows(&levels, Some(0), Some(1));
        let order: Vec<Option<i32>> = rows.iter().map(|r| r.ordinal).collect();
        assert_eq!(order, vec![Some(1), Some(0), Some(-1), None]);
        assert_eq!(rows[0].label, "2");
        assert!(rows[0].selected && !rows[3].selected, "the level shown is selected, Whole is not");
        assert_eq!((rows[1].label.as_str(), rows[1].you), ("1  · you", true));
        assert_eq!(rows[1].tooltip, "Storey 1 (ground) (you are on this level)");
        assert_eq!(rows[2].label, "B1");
        let whole = picker_rows(&levels, None, None);
        assert!(whole.last().is_some_and(|r| r.selected && r.ordinal.is_none()), "with no level shown, Whole is selected");
        assert!(whole.iter().all(|r| !r.you), "nobody is marked when you are on no level");
    }
}

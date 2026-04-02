//! Stat/metric display: label + value + optional bar or trend arrow.
//! Used by: HUD, profile, skills, resources, civilization dashboard.

use egui::{Color32, RichText, Rounding, Ui, Vec2};
use crate::gui::theme::Theme;

/// Visual style for a stat value.
pub enum StatStyle {
    /// Plain text, no decoration.
    Plain,
    /// Progress bar (0.0..=1.0) drawn below the value.
    Bar(f32),
    /// Trend indicator: positive = up arrow (green), negative = down arrow (red).
    Trend(f32),
}

/// Render a single stat with label, value, and optional decoration.
pub fn stat_display(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    value: &str,
    style: StatStyle,
) {
    ui.vertical(|ui| {
        // Label
        ui.label(
            RichText::new(label)
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );

        // Value
        ui.label(
            RichText::new(value)
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );

        // Decoration
        match style {
            StatStyle::Plain => {}
            StatStyle::Bar(frac) => {
                let bar_w = ui.available_width().min(120.0);
                let bar_h = 4.0;
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(bar_w, bar_h), egui::Sense::hover());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    painter.rect_filled(rect, Rounding::same(2), theme.bg_secondary());
                    let fill_w = bar_w * frac.clamp(0.0, 1.0);
                    if fill_w > 0.0 {
                        let fill = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, bar_h));
                        painter.rect_filled(fill, Rounding::same(2), theme.accent());
                    }
                }
            }
            StatStyle::Trend(delta) => {
                let (arrow, color) = if delta > 0.0 {
                    ("\u{25B2}", theme.success())
                } else if delta < 0.0 {
                    ("\u{25BC}", theme.danger())
                } else {
                    ("\u{25CF}", theme.text_muted())
                };
                let text = format!("{} {:.1}%", arrow, delta.abs());
                ui.label(
                    RichText::new(text)
                        .size(theme.font_size_small)
                        .color(color),
                );
            }
        }
    });
}

/// Compact horizontal row of plain stats for dashboards.
///
/// Each entry is a `(label, value)` pair rendered side by side.
pub fn stat_row(ui: &mut Ui, theme: &Theme, stats: &[(&str, &str)]) {
    ui.horizontal(|ui| {
        for (i, (label, value)) in stats.iter().enumerate() {
            if i > 0 {
                ui.add_space(theme.spacing_md);
                // Vertical separator
                ui.separator();
                ui.add_space(theme.spacing_md);
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(*label)
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                ui.label(
                    RichText::new(*value)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
            });
        }
    });
}

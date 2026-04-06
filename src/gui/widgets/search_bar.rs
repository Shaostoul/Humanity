//! Universal search bar with optional filter chips.
//! Used by: inventory, market, tasks, chat, files, recipes.

use egui::{Color32, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// A toggleable filter chip displayed below the search input.
pub struct FilterChip {
    pub label: String,
    pub active: bool,
    pub color: Option<Color32>,
}

/// Render a search bar with optional filter chips.
///
/// Returns `true` if the query text or any filter state changed this frame.
pub fn search_bar(
    ui: &mut Ui,
    theme: &Theme,
    query: &mut String,
    placeholder: &str,
    filters: &mut Vec<FilterChip>,
) -> bool {
    let mut changed = false;
    let rounding = Rounding::same(theme.border_radius as u8);

    // ---- Search input row ----
    ui.horizontal(|ui| {
        // Magnifying glass icon
        ui.label(
            RichText::new("\u{1F50D}")
                .size(theme.font_size_body)
                .color(theme.text_muted()),
        );

        // Text input
        let resp = ui.add_sized(
            Vec2::new(ui.available_width() - 30.0, theme.input_height),
            egui::TextEdit::singleline(query)
                .hint_text(placeholder)
                .desired_width(f32::INFINITY),
        );
        if resp.changed() {
            changed = true;
        }

        // Clear button when query is non-empty
        if !query.is_empty() {
            let clear_btn = egui::Button::new(
                RichText::new("\u{2715}").size(theme.font_size_body).color(theme.text_muted()),
            )
            .fill(Color32::TRANSPARENT)
            .rounding(rounding);
            if ui.add(clear_btn).clicked() {
                query.clear();
                changed = true;
            }
        }
    });

    // ---- Filter chips row ----
    if !filters.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.horizontal_wrapped(|ui| {
            for chip in filters.iter_mut() {
                let bg = if chip.active {
                    chip.color.unwrap_or_else(|| theme.accent())
                } else {
                    theme.bg_secondary()
                };
                let text_color = if chip.active {
                    theme.text_on_accent()
                } else {
                    theme.text_secondary()
                };

                let btn = egui::Button::new(
                    RichText::new(&chip.label)
                        .size(theme.font_size_small)
                        .color(text_color),
                )
                .fill(bg)
                .stroke(Stroke::new(1.0, theme.border()))
                .rounding(Rounding::same(12));

                if ui.add(btn).clicked() {
                    chip.active = !chip.active;
                    changed = true;
                }
            }
        });
    }

    changed
}

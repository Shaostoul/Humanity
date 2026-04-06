//! Button widgets: primary, secondary, danger, icon variants.

use egui::{Color32, RichText, Rounding, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// Orange accent button. Returns true if clicked.
pub fn btn_primary(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(label).color(theme.text_on_accent()).size(theme.font_size_body),
    )
    .fill(theme.accent())
    .min_size(Vec2::new(0.0, theme.button_height))
    .rounding(Rounding::same(theme.border_radius as u8));
    ui.add(btn).clicked()
}

/// Outline button with border. Returns true if clicked.
pub fn btn_secondary(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(label).color(theme.text_primary()).size(theme.font_size_body),
    )
    .fill(Color32::TRANSPARENT)
    .stroke(Stroke::new(1.0, theme.border()))
    .min_size(Vec2::new(0.0, theme.button_height))
    .rounding(Rounding::same(theme.border_radius as u8));
    ui.add(btn).clicked()
}

/// Red danger button for destructive actions. Returns true if clicked.
pub fn btn_danger(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(label).color(Color32::WHITE).size(theme.font_size_body),
    )
    .fill(theme.danger())
    .min_size(Vec2::new(0.0, theme.button_height))
    .rounding(Rounding::same(theme.border_radius as u8));
    ui.add(btn).clicked()
}

/// Small icon-only button with tooltip.
pub fn btn_icon(ui: &mut Ui, _theme: &Theme, icon: &str, tooltip: &str) -> bool {
    ui.small_button(icon).on_hover_text(tooltip).clicked()
}

// Backward compat aliases
pub use btn_primary as primary_button;
pub use btn_secondary as secondary_button;
pub use btn_danger as danger_button;

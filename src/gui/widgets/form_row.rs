//! Form row widget — label + control pair with consistent label-column width
//! and theme-driven spacing.
//!
//! Replaces ad-hoc `ui.horizontal(|ui| { ui.label("..."); ui.add(...); });`
//! patterns scattered across settings, ai_usage, onboarding, and server_settings.
//!
//! Two flavours:
//! - `form_row(ui, theme, label, content)` — standard label width
//! - `form_row_wide(ui, theme, label, content)` — wider label column for long labels
//! - `form_row_with_help(ui, theme, label, help, content)` — adds a small help string under the label

use egui::{RichText, Ui};
use super::super::theme::Theme;

/// Standard label-column width in pixels. Matches the pattern used in
/// settings.rs Appearance and Account sections.
const LABEL_WIDTH: f32 = 140.0;
const LABEL_WIDTH_WIDE: f32 = 200.0;

/// Render a label-on-the-left, control-on-the-right form row.
///
/// ```ignore
/// widgets::form_row(ui, theme, "Display name", |ui| {
///     ui.add(egui::TextEdit::singleline(&mut state.display_name));
/// });
/// ```
pub fn form_row(ui: &mut Ui, theme: &Theme, label: &str, content: impl FnOnce(&mut Ui)) {
    form_row_inner(ui, theme, label, None, LABEL_WIDTH, content);
}

/// Same as `form_row` but with a wider label column for long captions.
pub fn form_row_wide(ui: &mut Ui, theme: &Theme, label: &str, content: impl FnOnce(&mut Ui)) {
    form_row_inner(ui, theme, label, None, LABEL_WIDTH_WIDE, content);
}

/// Form row with a small help string rendered under the label.
pub fn form_row_with_help(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    help: &str,
    content: impl FnOnce(&mut Ui),
) {
    form_row_inner(ui, theme, label, Some(help), LABEL_WIDTH, content);
}

fn form_row_inner(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    help: Option<&str>,
    label_width: f32,
    content: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.label(
                    RichText::new(label)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
                if let Some(h) = help {
                    ui.label(
                        RichText::new(h)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                }
            },
        );
        ui.add_space(theme.spacing_md);
        content(ui);
    });
    ui.add_space(theme.spacing_sm);
}

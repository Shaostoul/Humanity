//! Placeholder page for features not yet built in egui.
//! Shows the page name with a "Coming to native app" message.

use egui::RichText;
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Draw a placeholder page.
pub fn draw(ctx: &egui::Context, theme: &Theme, _state: &mut GuiState, page_name: &str) {
    egui::CentralPanel::default()
        // The standard page surface every other page uses, so this inherits the
        // user's panel token instead of freezing an old literal grey.
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(
                    RichText::new(page_name)
                        .size(theme.font_size_title)
                        .color(theme.accent()),
                );
                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("This page is available on the web version.")
                        .size(theme.font_size_body)
                        .color(theme.text_muted()),
                );
                ui.label(
                    RichText::new("Native version coming soon.")
                        .size(theme.font_size_body)
                        .color(theme.text_muted()),
                );
            });
        });
}

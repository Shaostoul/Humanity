//! Placeholder page for features not yet built in egui.
//! Shows the page name with a "Coming to native app" message.

use egui::{Color32, Frame, RichText};
use crate::gui::GuiState;

/// Draw a placeholder page.
pub fn draw(ctx: &egui::Context, state: &mut GuiState, page_name: &str) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(
                    RichText::new(page_name)
                        .size(28.0)
                        .color(Color32::from_rgb(237, 140, 36)),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new("This page is available on the web version.")
                        .size(14.0)
                        .color(Color32::from_rgb(150, 150, 160)),
                );
                ui.label(
                    RichText::new("Native version coming soon.")
                        .size(14.0)
                        .color(Color32::from_rgb(150, 150, 160)),
                );
            });
        });
}

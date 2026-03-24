//! Placeholder page for features not yet built in egui.
//! Shows the page name with a "Coming to native app" message.

use egui::{Align2, Area, Color32, Frame, RichText, Vec2};
use crate::gui::{GuiPage, GuiState};

/// Draw a placeholder page with a back button.
pub fn draw(ctx: &egui::Context, state: &mut GuiState, page_name: &str) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 200));

    Area::new(egui::Id::new("placeholder_page"))
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            Frame::none()
                .inner_margin(32.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
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
                        ui.add_space(20.0);
                        if ui
                            .add_sized(Vec2::new(200.0, 36.0), egui::Button::new("Back"))
                            .clicked()
                        {
                            state.active_page = GuiPage::EscapeMenu;
                        }
                    });
                });
        });
}

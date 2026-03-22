//! Main menu page — title screen overlay on the 3D scene.

use egui::{Align2, Area, Color32, Frame, Margin};
use crate::gui::{GuiPage, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Draw the main menu as a centered overlay.
/// The 3D scene renders behind this as a live background.
pub fn draw(ctx: &egui::Context, theme: &Theme, gui_state: &mut GuiState, quit: &mut bool) {
    // Semi-transparent backdrop so the 3D scene shows through
    Area::new(egui::Id::new("main_menu_backdrop"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::hover());
            ui.painter().rect_filled(
                rect,
                egui::Rounding::ZERO,
                Color32::from_rgba_premultiplied(0, 0, 0, 120),
            );
        });

    // Centered menu panel
    Area::new(egui::Id::new("main_menu_panel"))
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            Frame::none()
                .fill(theme.panel_bg)
                .rounding(theme.rounding)
                .inner_margin(Margin::same(40.0))
                .show(ui, |ui| {
                    ui.set_min_width(300.0);
                    ui.vertical_centered(|ui| {
                        // Title
                        ui.label(
                            egui::RichText::new("HumanityOS")
                                .size(48.0)
                                .color(theme.primary)
                        );
                        ui.add_space(8.0);
                        ui.label(theme.dimmed("Unite Humanity"));
                        ui.add_space(32.0);

                        // Menu buttons
                        if widgets::primary_button(ui, theme, "Play") {
                            gui_state.active_page = GuiPage::None;
                        }
                        ui.add_space(8.0);

                        if widgets::secondary_button(ui, theme, "Settings") {
                            gui_state.active_page = GuiPage::Settings;
                        }
                        ui.add_space(8.0);

                        if widgets::secondary_button(ui, theme, "Quit") {
                            *quit = true;
                        }

                        ui.add_space(24.0);

                        // Version text at bottom
                        ui.label(theme.dimmed("v0.35.1"));
                    });
                });
        });
}

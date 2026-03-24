//! Title screen: Play, Settings, Quit.

use egui::{Align2, Area, Color32, RichText, Vec2};
use crate::gui::{GuiPage, GuiState, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Semi-transparent backdrop
    Area::new(egui::Id::new("main_menu_backdrop"))
        .fixed_pos([0.0, 0.0])
        .show(ctx, |ui| {
            let screen = ctx.screen_rect();
            ui.allocate_rect(screen, egui::Sense::click());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(180));
        });

    egui::Window::new("main_menu_window")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(320.0, 300.0))
        .frame(egui::Frame::none().fill(Color32::TRANSPARENT))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("HumanityOS").size(theme.font_size_title).color(theme.accent()));
                ui.add_space(8.0);
                ui.label(RichText::new("End poverty. Unite humanity.").size(theme.font_size_body).color(theme.text_secondary()));
                ui.add_space(40.0);

                if widgets::primary_button(ui, theme, "   Play   ") {
                    state.active_page = GuiPage::None;
                }
                ui.add_space(8.0);
                if widgets::secondary_button(ui, theme, " Settings ") {
                    state.active_page = GuiPage::Settings;
                }
                ui.add_space(8.0);
                if widgets::danger_button(ui, theme, "   Quit   ") {
                    state.quit_requested = true;
                }
                ui.add_space(30.0);
                ui.label(RichText::new(format!("v{}", VERSION)).size(theme.font_size_small).color(theme.text_muted()));
            });
        });
}

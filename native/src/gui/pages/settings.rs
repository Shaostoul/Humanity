//! Settings panel with Graphics, Audio, Controls tabs.

use egui::{RichText, Vec2};
use crate::gui::{GuiPage, GuiState, SettingsCategory};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Settings")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(500.0, 400.0))
        .show(ctx, |ui| {
            // Tab bar
            let tabs = ["Graphics", "Audio", "Controls"];
            let mut active = match state.settings.category {
                SettingsCategory::Graphics => 0,
                SettingsCategory::Audio => 1,
                SettingsCategory::Controls => 2,
            };
            if widgets::tab_bar(ui, theme, &tabs, &mut active) {
                state.settings.category = match active {
                    0 => SettingsCategory::Graphics,
                    1 => SettingsCategory::Audio,
                    _ => SettingsCategory::Controls,
                };
            }

            ui.separator();
            ui.add_space(theme.spacing_sm);

            match state.settings.category {
                SettingsCategory::Graphics => {
                    widgets::toggle(ui, theme, "Fullscreen", &mut state.settings.fullscreen);
                    widgets::toggle(ui, theme, "VSync", &mut state.settings.vsync);
                    widgets::labeled_slider(ui, theme, "FOV", &mut state.settings.fov, 60.0..=120.0);
                    widgets::labeled_slider(ui, theme, "Render Distance", &mut state.settings.render_distance, 50.0..=2000.0);
                }
                SettingsCategory::Audio => {
                    widgets::labeled_slider(ui, theme, "Master Volume", &mut state.settings.master_volume, 0.0..=1.0);
                    widgets::labeled_slider(ui, theme, "Music Volume", &mut state.settings.music_volume, 0.0..=1.0);
                    widgets::labeled_slider(ui, theme, "SFX Volume", &mut state.settings.sfx_volume, 0.0..=1.0);
                }
                SettingsCategory::Controls => {
                    widgets::labeled_slider(ui, theme, "Mouse Sensitivity", &mut state.settings.mouse_sensitivity, 0.5..=10.0);
                    widgets::toggle(ui, theme, "Invert Y-Axis", &mut state.settings.invert_y);
                }
            }

            ui.add_space(theme.spacing_lg);
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, theme, "Back") {
                    state.active_page = GuiPage::MainMenu;
                }
            });
        });
}

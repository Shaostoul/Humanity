//! Settings page — graphics, audio, controls, game, and account options.

use egui::{Align2, Area, Frame, Margin};
use crate::gui::{GuiPage, GuiState, SettingsCategory};
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Resolution presets available in the dropdown.
const RESOLUTIONS: &[&str] = &[
    "1280x720",
    "1600x900",
    "1920x1080",
    "2560x1440",
    "3840x2160",
];

/// Draw the settings panel as a centered overlay.
pub fn draw(ctx: &egui::Context, theme: &Theme, gui_state: &mut GuiState) {
    Area::new(egui::Id::new("settings_panel"))
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            Frame::none()
                .fill(theme.panel_bg)
                .rounding(theme.rounding)
                .inner_margin(Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(500.0);
                    ui.set_max_height(600.0);

                    // Header
                    ui.horizontal(|ui| {
                        ui.label(theme.heading("Settings"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::secondary_button(ui, theme, "Back") {
                                gui_state.active_page = GuiPage::MainMenu;
                            }
                        });
                    });
                    ui.separator();
                    ui.add_space(8.0);

                    // Category tabs
                    ui.horizontal(|ui| {
                        for cat in &[
                            SettingsCategory::Graphics,
                            SettingsCategory::Audio,
                            SettingsCategory::Controls,
                            SettingsCategory::Game,
                            SettingsCategory::Account,
                        ] {
                            let label = match cat {
                                SettingsCategory::Graphics => "Graphics",
                                SettingsCategory::Audio => "Audio",
                                SettingsCategory::Controls => "Controls",
                                SettingsCategory::Game => "Game",
                                SettingsCategory::Account => "Account",
                            };
                            let selected = gui_state.settings.active_category == *cat;
                            let text = if selected {
                                egui::RichText::new(label).color(theme.primary)
                            } else {
                                egui::RichText::new(label).color(theme.text_dim)
                            };
                            if ui.selectable_label(selected, text).clicked() {
                                gui_state.settings.active_category = *cat;
                            }
                        }
                    });
                    ui.add_space(8.0);

                    // Category content
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        match gui_state.settings.active_category {
                            SettingsCategory::Graphics => draw_graphics(ui, theme, gui_state),
                            SettingsCategory::Audio => draw_audio(ui, theme, gui_state),
                            SettingsCategory::Controls => draw_controls(ui, theme, gui_state),
                            SettingsCategory::Game => draw_game(ui, theme),
                            SettingsCategory::Account => draw_account(ui, theme),
                        }
                    });
                });
        });
}

fn draw_graphics(ui: &mut egui::Ui, theme: &Theme, gui_state: &mut GuiState) {
    widgets::collapsible_section(ui, theme, "Display", true, |ui| {
        // Resolution dropdown
        ui.horizontal(|ui| {
            ui.label(theme.body("Resolution"));
            egui::ComboBox::from_id_salt("resolution")
                .selected_text(RESOLUTIONS[gui_state.settings.resolution_index])
                .show_ui(ui, |ui| {
                    for (i, res) in RESOLUTIONS.iter().enumerate() {
                        ui.selectable_value(
                            &mut gui_state.settings.resolution_index,
                            i,
                            *res,
                        );
                    }
                });
        });

        // Fullscreen toggle
        ui.checkbox(&mut gui_state.settings.fullscreen, theme.body("Fullscreen"));

        // VSync toggle
        ui.checkbox(&mut gui_state.settings.vsync, theme.body("VSync"));
    });

    widgets::collapsible_section(ui, theme, "Rendering", true, |ui| {
        // FOV slider
        widgets::labeled_slider(ui, theme, "Field of View", &mut gui_state.settings.fov, 60.0..=120.0);

        // Render distance slider
        widgets::labeled_slider(ui, theme, "Render Distance", &mut gui_state.settings.render_distance, 100.0..=2000.0);
    });
}

fn draw_audio(ui: &mut egui::Ui, theme: &Theme, gui_state: &mut GuiState) {
    widgets::collapsible_section(ui, theme, "Volume", true, |ui| {
        widgets::labeled_slider(ui, theme, "Master Volume", &mut gui_state.settings.master_volume, 0.0..=1.0);
        widgets::labeled_slider(ui, theme, "Music Volume", &mut gui_state.settings.music_volume, 0.0..=1.0);
        widgets::labeled_slider(ui, theme, "SFX Volume", &mut gui_state.settings.sfx_volume, 0.0..=1.0);
    });
}

fn draw_controls(ui: &mut egui::Ui, theme: &Theme, gui_state: &mut GuiState) {
    widgets::collapsible_section(ui, theme, "Mouse", true, |ui| {
        widgets::labeled_slider(ui, theme, "Sensitivity", &mut gui_state.settings.mouse_sensitivity, 0.1..=5.0);
        ui.checkbox(&mut gui_state.settings.invert_y, theme.body("Invert Y Axis"));
    });
}

fn draw_game(ui: &mut egui::Ui, theme: &Theme) {
    widgets::collapsible_section(ui, theme, "Gameplay", true, |ui| {
        ui.label(theme.dimmed("No game settings available yet."));
    });
}

fn draw_account(ui: &mut egui::Ui, theme: &Theme) {
    widgets::collapsible_section(ui, theme, "Identity", true, |ui| {
        ui.label(theme.dimmed("Ed25519 identity management coming soon."));
    });
}

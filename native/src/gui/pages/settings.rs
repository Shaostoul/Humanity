//! Settings panel with Graphics, Audio, Controls, Updates tabs.
//! Toggles and sliders write to GuiState.settings; lib.rs reads these
//! and applies them to the camera, controller, window, and audio manager.

use egui::{RichText, Vec2};
use crate::gui::{GuiPage, GuiState, SettingsCategory, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::updater::{UpdateChannel, UpdateState};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Settings")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(520.0, 450.0))
        .show(ctx, |ui| {
            // Tab bar
            let tabs = ["Graphics", "Audio", "Controls", "Updates"];
            let mut active = match state.settings.category {
                SettingsCategory::Graphics => 0,
                SettingsCategory::Audio => 1,
                SettingsCategory::Controls => 2,
                SettingsCategory::Updates => 3,
            };
            if widgets::tab_bar(ui, theme, &tabs, &mut active) {
                state.settings.category = match active {
                    0 => SettingsCategory::Graphics,
                    1 => SettingsCategory::Audio,
                    2 => SettingsCategory::Controls,
                    _ => SettingsCategory::Updates,
                };
            }

            ui.separator();
            ui.add_space(theme.spacing_sm);

            match state.settings.category {
                SettingsCategory::Graphics => {
                    if widgets::toggle(ui, theme, "Fullscreen", &mut state.settings.fullscreen) {
                        state.settings_dirty = true;
                    }
                    if widgets::toggle(ui, theme, "VSync", &mut state.settings.vsync) {
                        state.settings_dirty = true;
                    }
                    if widgets::labeled_slider(ui, theme, "FOV", &mut state.settings.fov, 60.0..=120.0) {
                        state.settings_dirty = true;
                    }
                    if widgets::labeled_slider(ui, theme, "Render Distance", &mut state.settings.render_distance, 50.0..=2000.0) {
                        state.settings_dirty = true;
                    }
                }
                SettingsCategory::Audio => {
                    if widgets::labeled_slider(ui, theme, "Master Volume", &mut state.settings.master_volume, 0.0..=1.0) {
                        state.settings_dirty = true;
                    }
                    if widgets::labeled_slider(ui, theme, "Music Volume", &mut state.settings.music_volume, 0.0..=1.0) {
                        state.settings_dirty = true;
                    }
                    if widgets::labeled_slider(ui, theme, "SFX Volume", &mut state.settings.sfx_volume, 0.0..=1.0) {
                        state.settings_dirty = true;
                    }
                }
                SettingsCategory::Controls => {
                    if widgets::labeled_slider(ui, theme, "Mouse Sensitivity", &mut state.settings.mouse_sensitivity, 0.5..=10.0) {
                        state.settings_dirty = true;
                    }
                    if widgets::toggle(ui, theme, "Invert Y-Axis", &mut state.settings.invert_y) {
                        state.settings_dirty = true;
                    }
                }
                SettingsCategory::Updates => {
                    draw_updates_tab(ui, theme, state);
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

fn draw_updates_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Current version
    ui.label(RichText::new(format!("Current Version: v{}", VERSION)).strong());
    ui.add_space(theme.spacing_sm);

    // Update channel
    ui.label(RichText::new("Update Channel").color(theme.text_secondary()));
    let mut is_latest = state.updater.channel == UpdateChannel::AlwaysLatest;
    let mut is_disabled = state.updater.channel == UpdateChannel::Disabled;

    if ui.radio_value(&mut is_latest, true, "Always Latest (recommended)").changed() && is_latest {
        state.updater.channel = UpdateChannel::AlwaysLatest;
    }
    if ui.radio_value(&mut is_disabled, true, "Disabled (never check)").changed() && is_disabled {
        state.updater.channel = UpdateChannel::Disabled;
    }

    ui.add_space(theme.spacing_md);

    // Status
    let status_text = match &state.updater.state {
        UpdateState::Idle => "Not checked yet".to_string(),
        UpdateState::Checking => "Checking for updates...".to_string(),
        UpdateState::UpToDate => "You're on the latest version".to_string(),
        UpdateState::Available { version, .. } => format!("Update available: {}", version),
        UpdateState::Downloading { version, progress } => {
            format!("Downloading {}: {:.0}%", version, progress * 100.0)
        }
        UpdateState::Ready { version, .. } => format!("{} ready. Restart to apply.", version),
        UpdateState::Error(e) => format!("Error: {}", e),
    };
    ui.label(RichText::new(&status_text).color(
        match &state.updater.state {
            UpdateState::Available { .. } => theme.accent(),
            UpdateState::Error(_) => theme.danger(),
            UpdateState::Ready { .. } => theme.success(),
            _ => theme.text_secondary(),
        }
    ));

    ui.add_space(theme.spacing_sm);

    // Action buttons
    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "Check Now") {
            state.updater.check_now();
        }

        if let UpdateState::Available { version, .. } = &state.updater.state {
            let ver = version.clone();
            if widgets::primary_button(ui, theme, "Download Update") {
                state.updater.download_version(&ver);
            }
        }
    });

    // Download progress bar
    if let UpdateState::Downloading { progress, .. } = &state.updater.state {
        ui.add_space(theme.spacing_sm);
        widgets::progress_bar(ui, theme, *progress, Some("Downloading..."));
    }

    // Version picker
    ui.add_space(theme.spacing_lg);
    ui.label(RichText::new("Available Versions").color(theme.text_secondary()));

    let versions = state.updater.available_versions();
    if versions.is_empty() {
        ui.label(RichText::new("Check for updates to see available versions.").color(theme.text_muted()));
    } else {
        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            for (tag, date, is_current) in &versions {
                ui.horizontal(|ui| {
                    let label = if *is_current {
                        RichText::new(format!("{} (current)", tag)).strong().color(theme.success())
                    } else {
                        RichText::new(tag).color(theme.text_primary())
                    };
                    ui.label(label);
                    ui.label(RichText::new(date).small().color(theme.text_muted()));

                    if !is_current {
                        let tag_clone = tag.clone();
                        if ui.small_button("Install").clicked() {
                            state.updater.download_version(&tag_clone);
                        }
                    }
                });
            }
        });
    }
}

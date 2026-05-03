//! Per-category Settings sub-pages (v0.182.0).
//!
//! Each function here is a thin wrapper that calls into the existing
//! `draw_X_content` function in `settings.rs`. The wrappers exist so
//! every settings category is a real `GuiPage` variant — the sub-tier
//! nav can navigate straight to it without going through a sidebar
//! TOC + scroll-jump.
//!
//! Header style (small label + bold title + summary line) matches the
//! category overview header so the page feels like a destination, not
//! a fragment of a longer scroll.

use egui::{Frame, RichText, ScrollArea, Stroke};

use crate::gui::pages::settings;
use crate::gui::theme::Theme;
use crate::gui::GuiState;

/// Helper: render the section heading + run the content closure inside
/// a centered scrollable column. Uses `&mut Theme` because some content
/// fns (Appearance, Animations, Widgets) write to it; the others ignore.
fn settings_subpage_frame(
    ctx: &egui::Context,
    theme: &mut Theme,
    state: &mut GuiState,
    label: &str,
    summary: &str,
    body: impl FnOnce(&mut egui::Ui, &mut Theme, &mut GuiState),
) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header values must be snapshot before the &mut borrow into body.
            let small = theme.font_size_small;
            let title = theme.font_size_title;
            let body_sz = theme.font_size_body;
            let xs = theme.spacing_xs;
            let sm = theme.spacing_sm;
            let md = theme.spacing_md;
            let xl = theme.spacing_xl;
            let accent = theme.nav_settings();
            let text_primary = theme.text_primary();
            let text_secondary = theme.text_secondary();

            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(xs);
                ui.label(
                    RichText::new(label.to_uppercase())
                        .size(small).color(accent).strong()
                );
                ui.add_space(sm);
                ui.label(
                    RichText::new(label)
                        .size(title).color(text_primary).strong()
                );
                if !summary.is_empty() {
                    ui.add_space(sm);
                    ui.label(
                        RichText::new(summary)
                            .size(body_sz).color(text_secondary)
                    );
                }
                ui.add_space(md);
                Frame::none()
                    .fill(theme.bg_card())
                    .rounding(egui::Rounding::same(theme.border_radius as u8))
                    .stroke(Stroke::new(1.0, theme.border()))
                    .inner_margin(theme.card_padding)
                    .show(ui, |ui| {
                        body(ui, theme, state);
                    });
                ui.add_space(xl);
            });
        });
}

pub fn draw_account(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Account",
        "Display name, public key, ECDH DM key, seed-phrase backup.",
        |ui, theme, state| settings::draw_account_content(ui, theme, state));
}

pub fn draw_appearance(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Appearance",
        "Dark mode, font size, every theme color token, nav category colors.",
        |ui, theme, state| settings::draw_appearance_content(ui, theme, state));
}

pub fn draw_animations(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Animations",
        "Master switch + per-element style (RGB cycle / solid / pulse / off) + attack indicator picker.",
        |ui, theme, state| settings::draw_animations_content(ui, theme, state));
}

pub fn draw_widgets(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Widgets",
        "Sizing, spacing, fonts, borders, slider + checkbox + nav presence.",
        |ui, theme, state| settings::draw_widgets_content(ui, theme, state));
}

pub fn draw_notifications(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Notifications",
        "DM notifications, mentions, task reminders, do-not-disturb window.",
        |ui, theme, state| settings::draw_notifications_content(ui, theme, state));
}

pub fn draw_wallet(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Wallet",
        "Solana RPC endpoint, network selector, default tip amounts.",
        |ui, theme, state| settings::draw_wallet_content(ui, theme, state));
}

pub fn draw_audio(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Audio",
        "Master / music / SFX volume sliders + voice device selectors.",
        |ui, theme, state| settings::draw_audio_content(ui, theme, state));
}

pub fn draw_graphics(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Graphics",
        "Fullscreen, vsync, FOV, render distance.",
        |ui, theme, state| settings::draw_graphics_content(ui, theme, state));
}

pub fn draw_controls(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Controls",
        "Mouse sensitivity, key rebinds, gamepad mappings.",
        |ui, theme, state| settings::draw_controls_content(ui, theme, state));
}

pub fn draw_privacy(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Privacy",
        "Public profile fields, message visibility, federation opt-ins.",
        |ui, theme, state| settings::draw_privacy_content(ui, theme, state));
}

pub fn draw_data(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Data",
        "Local storage, vault sync, export, restore from seed.",
        |ui, theme, state| settings::draw_data_content(ui, theme, state));
}

pub fn draw_updates(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    settings_subpage_frame(ctx, theme, state, "Updates",
        "Current version, check for updates, channel selector.",
        |ui, theme, state| settings::draw_updates_content(ui, theme, state));
}

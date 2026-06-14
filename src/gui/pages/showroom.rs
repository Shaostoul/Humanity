//! Character-select / customize showroom panel (v0.441).
//!
//! Drawn over the 3D view while `gui_state.showroom_active`. The 3D side (orbit camera,
//! hidden home, backdrop ground, live avatar) is wired in `lib.rs`; this panel only edits
//! `gui_state` (appearance + backdrop index + the confirm flag), which the main loop then
//! applies. Same edit-buffer-then-sync pattern as Settings.

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Character Creation")
        .anchor(egui::Align2::RIGHT_CENTER, egui::vec2(-24.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(
                RichText::new("This is you. Drag to orbit. Customize, then enter your home.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);

            // ── Appearance ──
            ui.label(RichText::new("Appearance").strong().color(theme.text_primary()));
            ui.horizontal(|ui| {
                ui.label(RichText::new("Skin").color(theme.text_secondary()));
                if ui.color_edit_button_rgb(&mut state.appearance.skin_tone).changed() {
                    state.appearance_dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Hair").color(theme.text_secondary()));
                if ui.color_edit_button_rgb(&mut state.appearance.hair_color).changed() {
                    state.appearance_dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Eyes").color(theme.text_secondary()));
                if ui.color_edit_button_rgb(&mut state.appearance.eye_color).changed() {
                    state.appearance_dirty = true;
                }
            });
            if widgets::labeled_slider(ui, theme, "Height", &mut state.appearance.height_scale, 0.8..=1.2) {
                state.appearance_dirty = true;
            }
            ui.add_space(theme.spacing_sm);

            // ── Backdrop ──
            ui.label(RichText::new("Backdrop").strong().color(theme.text_primary()));
            let n = state.showroom_backdrop_names.len().max(1);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("  <  ")).clicked() {
                    state.showroom_backdrop = (state.showroom_backdrop + n - 1) % n;
                }
                let name = state
                    .showroom_backdrop_names
                    .get(state.showroom_backdrop)
                    .cloned()
                    .unwrap_or_default();
                ui.label(RichText::new(name).color(theme.text_secondary()));
                if ui.button(RichText::new("  >  ")).clicked() {
                    state.showroom_backdrop = (state.showroom_backdrop + 1) % n;
                }
            });
            ui.add_space(theme.spacing_sm);

            ui.label(
                RichText::new("Outfits: change them at the bedroom wardrobe.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_md);

            if ui
                .button(RichText::new("Enter your home").size(theme.font_size_body).strong())
                .clicked()
            {
                state.showroom_confirm = true;
            }
        });
}

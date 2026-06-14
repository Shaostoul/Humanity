//! Character showroom panel (v0.441/442). One orbiting-avatar scene, three modes:
//!   0 = character select (on spawn): edit appearance + backdrop, "Enter your home".
//!   1 = appearance editor (wetroom mirror): edit appearance, "Done".
//!   2 = wardrobe (bedroom): equip cosmetics per slot, "Done".
//! The panel only edits `gui_state` (appearance / outfit / backdrop / confirm); the main
//! loop applies it to the avatar mesh, camera, backdrop, and save (edit-buffer-then-sync).

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

const SLOTS: [&str; 6] = ["head", "chest", "legs", "feet", "hands", "back"];

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    let (title, confirm_label) = match state.showroom_mode {
        1 => ("Appearance", "Done"),
        2 => ("Wardrobe", "Done"),
        _ => ("Character Creation", "Enter your home"),
    };
    egui::Window::new(title)
        .anchor(egui::Align2::RIGHT_CENTER, egui::vec2(-24.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(
                RichText::new("This is you. Drag to orbit.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);

            if state.showroom_mode == 2 {
                draw_wardrobe(ui, theme, state);
            } else {
                draw_appearance(ui, theme, state);
            }

            ui.add_space(theme.spacing_sm);
            draw_backdrop(ui, theme, state);
            ui.add_space(theme.spacing_md);

            if ui
                .button(RichText::new(confirm_label).size(theme.font_size_body).strong())
                .clicked()
            {
                state.showroom_confirm = true;
            }
        });
}

fn draw_appearance(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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
    if state.showroom_mode == 0 {
        ui.label(
            RichText::new("Outfits: change them at the bedroom wardrobe.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    }
}

fn draw_wardrobe(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Wardrobe").strong().color(theme.text_primary()));
    for slot in SLOTS {
        let current = state.outfit.equipped.get(slot).cloned();
        // Cosmetics available for this slot (id, name).
        let items: Vec<(String, String)> = state
            .cosmetics_list
            .iter()
            .filter(|(_, _, s)| s == slot)
            .map(|(id, name, _)| (id.clone(), name.clone()))
            .collect();
        if items.is_empty() {
            continue;
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(cap(slot)).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(current.is_none(), "None").clicked() {
                state.outfit.equipped.remove(slot);
                state.outfit_dirty = true;
            }
            for (id, name) in &items {
                let selected = current.as_deref() == Some(id.as_str());
                if ui.selectable_label(selected, name).clicked() {
                    state.outfit.equipped.insert(slot.to_string(), id.clone());
                    state.outfit_dirty = true;
                }
            }
        });
    }
}

fn draw_backdrop(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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
}

/// Capitalize a slot id for display ("chest" -> "Chest").
fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

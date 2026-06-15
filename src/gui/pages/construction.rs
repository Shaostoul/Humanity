//! Construction editor panel (v0.455): craft the homestead's walls in-app, so the floor
//! plan is shaped in the tool instead of guessed in the RON. A right panel lists every room;
//! each wall (N/S/W/E) is a dropdown of `WallKind`. Changing one (or the ceiling height)
//! sets `construction_dirty`, which the main loop watches to rebuild the home live. "Save
//! layout" writes the layout back to `data/blueprints/homestead_layout.ron`.
//!
//! The panel only edits `gui_state` (the mirrored `construction_rooms` + `construction_height`
//! and the dirty/save flags); the engine owns the actual layout + the rebuild + the save.

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::GuiState;
use crate::ship::fibonacci::WallKind;

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    egui::SidePanel::right("construction_editor")
        .resizable(true)
        .default_width(340.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new("Construction")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new("Set each wall; the home rebuilds live. Press B to close.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);

            // Uniform ceiling height (mirrors layout.default_wall_height).
            ui.horizontal(|ui| {
                ui.label(RichText::new("Ceiling height").color(theme.text_secondary()));
                if ui
                    .add(egui::Slider::new(&mut state.construction_height, 3.0..=20.0).suffix(" m"))
                    .changed()
                {
                    state.construction_dirty = true;
                }
            });
            ui.add_space(theme.spacing_sm);
            ui.separator();

            let wall_labels = ["North", "South", "West", "East"];
            egui::ScrollArea::vertical().show(ui, |ui| {
                let room_count = state.construction_rooms.len();
                for ri in 0..room_count {
                    let room_name = state.construction_rooms[ri].0.clone();
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new(room_name)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    for wi in 0..4 {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(wall_labels[wi])
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            let cur = state.construction_rooms[ri].1[wi];
                            egui::ComboBox::from_id_salt((ri, wi))
                                .selected_text(cur.label())
                                .show_ui(ui, |ui| {
                                    for k in WallKind::ALL {
                                        if ui
                                            .selectable_value(
                                                &mut state.construction_rooms[ri].1[wi],
                                                k,
                                                k.label(),
                                            )
                                            .changed()
                                        {
                                            state.construction_dirty = true;
                                        }
                                    }
                                });
                        });
                    }
                }
            });

            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Save layout").strong())
                    .clicked()
                {
                    state.construction_save = true;
                }
                if ui.button("Close").clicked() {
                    state.construction_active = false;
                }
            });
            ui.label(
                RichText::new("Save writes data/blueprints/homestead_layout.ron.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
}

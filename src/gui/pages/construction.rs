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
                    .add(egui::Slider::new(&mut state.construction_height, 2.5..=12.0).suffix(" m"))
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
                    let room_name = state.construction_rooms[ri].id.clone();
                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        RichText::new(room_name)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    // Per-wall kinds.
                    for wi in 0..4 {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(wall_labels[wi])
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            let cur = state.construction_rooms[ri].walls[wi];
                            egui::ComboBox::from_id_salt((ri, wi))
                                .selected_text(cur.label())
                                .show_ui(ui, |ui| {
                                    for k in WallKind::ALL {
                                        if ui
                                            .selectable_value(
                                                &mut state.construction_rooms[ri].walls[wi],
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
                    // Position: pin (explicit) vs computed by the auto-layout.
                    ui.horizontal(|ui| {
                        let mut pinned = state.construction_rooms[ri].position.is_some();
                        if ui
                            .checkbox(&mut pinned, RichText::new("Pin position")
                                .size(theme.font_size_small).color(theme.text_muted()))
                            .changed()
                        {
                            state.construction_rooms[ri].position =
                                if pinned { Some([0.0, 0.0, 0.0]) } else { None };
                            state.construction_dirty = true;
                        }
                    });
                    if state.construction_rooms[ri].position.is_some() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("X").size(theme.font_size_small).color(theme.text_muted()));
                            let pos = state.construction_rooms[ri].position.as_mut().unwrap();
                            let cx = ui.add(egui::DragValue::new(&mut pos[0]).speed(0.1).suffix(" m").range(-200.0..=200.0)).changed();
                            ui.label(RichText::new("Z").size(theme.font_size_small).color(theme.text_muted()));
                            let cz = ui.add(egui::DragValue::new(&mut pos[2]).speed(0.1).suffix(" m").range(-200.0..=200.0)).changed();
                            if cx || cz { state.construction_dirty = true; }
                        });
                    }
                    // Size (width x depth; height is the global ceiling slider).
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("W").size(theme.font_size_small).color(theme.text_muted()));
                        let d = &mut state.construction_rooms[ri].dimensions;
                        let cw = ui.add(egui::DragValue::new(&mut d[0]).speed(0.1).suffix(" m").range(0.5..=80.0)).changed();
                        ui.label(RichText::new("D").size(theme.font_size_small).color(theme.text_muted()));
                        let cd = ui.add(egui::DragValue::new(&mut d[2]).speed(0.1).suffix(" m").range(0.5..=80.0)).changed();
                        if cw || cd { state.construction_dirty = true; }
                    });
                    if ui.button(RichText::new("Delete room").color(theme.danger())).clicked() {
                        state.construction_remove = Some(ri);
                    }
                    ui.separator();
                }
            });

            // Apply a deferred delete AFTER the scroll loop (so the index stays valid).
            if let Some(ri) = state.construction_remove.take() {
                if ri < state.construction_rooms.len() {
                    state.construction_rooms.remove(ri);
                    state.construction_dirty = true;
                }
            }

            ui.add_space(theme.spacing_sm);
            ui.separator();
            // Add a new room of a chosen type (pinned at origin; drag it into place).
            ui.label(RichText::new("Add room").strong().color(theme.text_primary()));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("construction_add_type")
                    .selected_text(state.construction_add_type.clone())
                    .show_ui(ui, |ui| {
                        let opts = state.construction_room_types.clone();
                        for t in opts {
                            ui.selectable_value(&mut state.construction_add_type, t.clone(), t);
                        }
                    });
                if ui.button(RichText::new("Add").strong()).clicked()
                    && !state.construction_add_type.is_empty()
                {
                    let base = state.construction_add_type.clone();
                    let mut id = base.clone();
                    let mut n = 2;
                    while state.construction_rooms.iter().any(|r| r.id == id) {
                        id = format!("{base}_{n}");
                        n += 1;
                    }
                    let h = state.construction_height.max(2.5);
                    state.construction_rooms.push(crate::gui::ConstructionRoom {
                        id,
                        walls: [WallKind::Auto; 4],
                        position: Some([0.0, 0.0, 0.0]),
                        dimensions: [4.0, h, 4.0],
                        material_type: 1,
                        color: [0.5, 0.5, 0.55, 1.0],
                    });
                    state.construction_dirty = true;
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

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
                RichText::new("Drag to orbit, middle-drag to pan, wheel to zoom, WASD to fly, Space/Shift up+down. Press B to close.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.checkbox(&mut state.construction_plan_view,
                RichText::new("Top-down plan overlay").size(theme.font_size_small).color(theme.text_muted()));
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

    // Top-down floor-plan overlay (v0.463), shown only when toggled on (v0.464). By default the
    // construction view is the free ORBIT "astral" camera (drag/pan/dolly/fly) so you can
    // navigate the whole structure incl. multiple stories; the 2D plan is an optional overlay.
    if state.construction_plan_view {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(150)))
            .show(ctx, |ui| {
                draw_floorplan_canvas(ui, theme, state);
            });
    }
}

/// Draw the top-down floor plan: every room as a rectangle seen from above (world X -> right,
/// world Z -> down, so North is up). Dragging a room moves it on a 0.25 m grid and triggers a
/// live rebuild. (v0.463)
fn draw_floorplan_canvas(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 40.0 || rect.height() < 40.0 {
        return;
    }
    let painter = ui.painter_at(rect);

    if state.construction_rooms.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No rooms yet -- use Add room.",
            egui::FontId::proportional(16.0),
            theme.text_muted(),
        );
        return;
    }

    // World bounds (x, z) over every room's footprint.
    let (mut min_x, mut min_z, mut max_x, mut max_z) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for r in &state.construction_rooms {
        let p = r.position.unwrap_or([0.0, 0.0, 0.0]);
        min_x = min_x.min(p[0]);
        min_z = min_z.min(p[2]);
        max_x = max_x.max(p[0] + r.dimensions[0]);
        max_z = max_z.max(p[2] + r.dimensions[2]);
    }
    let world_w = (max_x - min_x).max(1.0);
    let world_d = (max_z - min_z).max(1.0);
    let scale = (rect.width() / world_w).min(rect.height() / world_d) * 0.9;
    let (cx, cz) = ((min_x + max_x) * 0.5, (min_z + max_z) * 0.5);
    let to_canvas = |wx: f32, wz: f32| -> egui::Pos2 {
        egui::pos2(
            rect.center().x + (wx - cx) * scale,
            rect.center().y + (wz - cz) * scale,
        )
    };

    painter.text(
        rect.left_top() + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        "Top-down plan -- drag a room to move it (North is up).",
        egui::FontId::proportional(13.0),
        theme.text_secondary(),
    );

    // Draw rooms; collect a drag so we mutate state.construction_rooms after the loop.
    let mut moved: Option<(usize, f32, f32)> = None;
    let count = state.construction_rooms.len();
    for ri in 0..count {
        let r = &state.construction_rooms[ri];
        let p = r.position.unwrap_or([0.0, 0.0, 0.0]);
        let room_rect = egui::Rect::from_two_pos(
            to_canvas(p[0], p[2]),
            to_canvas(p[0] + r.dimensions[0], p[2] + r.dimensions[2]),
        );
        let fill = egui::Color32::from_rgba_unmultiplied(
            (r.color[0] * 255.0) as u8,
            (r.color[1] * 255.0) as u8,
            (r.color[2] * 255.0) as u8,
            205,
        );
        painter.rect_filled(room_rect, egui::Rounding::same(2), fill);
        let resp = ui.interact(room_rect, egui::Id::new(("plan_room", ri)), egui::Sense::drag());
        let stroke = if resp.hovered() || resp.dragged() {
            egui::Stroke::new(2.0, theme.accent())
        } else {
            egui::Stroke::new(1.0, theme.border())
        };
        painter.rect_stroke(room_rect, egui::Rounding::same(2), stroke, egui::StrokeKind::Inside);
        painter.text(
            room_rect.center(),
            egui::Align2::CENTER_CENTER,
            &r.id,
            egui::FontId::proportional(12.0),
            theme.text_primary(),
        );
        if resp.dragged() {
            let d = resp.drag_delta();
            moved = Some((ri, d.x / scale, d.y / scale));
        }
    }

    if let Some((ri, dwx, dwz)) = moved {
        let r = &mut state.construction_rooms[ri];
        let mut p = r.position.unwrap_or([0.0, 0.0, 0.0]);
        let snap = |v: f32| (v / 0.25).round() * 0.25;
        p[0] = snap(p[0] + dwx);
        p[2] = snap(p[2] + dwz);
        r.position = Some(p);
        state.construction_dirty = true;
    }
}

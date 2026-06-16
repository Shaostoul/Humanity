//! Construction editor (v0.455+). Three columns: a LEFT room tree (the table of contents for
//! the home, the "main room"), the CENTER 3D orbit viewport (the astral camera), and a RIGHT
//! details pane for the selected room (its walls, position, size). The number panel + the 3D
//! grab edit the same `gui_state.construction_rooms` mirror; `construction_dirty` drives the
//! live rebuild; "Save layout" writes the RON. (3-column restructure v0.467.)
//!
//! The panel only edits `gui_state`; the engine owns the layout + rebuild + save + the 3D grab.

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::{EditorOpening, EditorOpeningKind, GuiState};
use crate::ship::fibonacci::WallKind;

const WALL_LABELS: [&str; 4] = ["North", "South", "West", "East"];

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // ── LEFT: the room tree / table of contents for the home (the "main room") ──
    egui::SidePanel::left("construction_rooms")
        .resizable(true)
        .default_width(190.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            // The top-level container is a STRUCTURE (a home / mall / ship / base); it contains
            // Rooms. (terminology locked with the operator, v0.468.)
            ui.label(RichText::new("Structure").size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.label(RichText::new("Home").size(theme.font_size_small).color(theme.text_muted()));
            ui.add_space(theme.spacing_sm);
            egui::ScrollArea::vertical().id_salt("rooms_tree").show(ui, |ui| {
                let n = state.construction_rooms.len();
                for ri in 0..n {
                    let name = state.construction_rooms[ri].id.clone();
                    let selected = state.construction_selected_room == Some(ri);
                    let label = RichText::new(format!("  {name}"))
                        .color(if selected { theme.accent() } else { theme.text_secondary() });
                    if ui.selectable_label(selected, label).clicked() {
                        state.construction_selected_room = Some(ri);
                    }
                }
            });
            ui.add_space(theme.spacing_sm);
            ui.separator();
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
                    let mut k = 2;
                    while state.construction_rooms.iter().any(|r| r.id == id) {
                        id = format!("{base}_{k}");
                        k += 1;
                    }
                    let h = state.construction_height.max(2.5);
                    state.construction_rooms.push(crate::gui::ConstructionRoom {
                        id,
                        walls: [WallKind::Auto; 4],
                        wall_offsets: [0.0; 4],
                        openings: Vec::new(),
                        position: Some([0.0, 0.0, 0.0]),
                        dimensions: [4.0, h, 4.0],
                        material_type: 1,
                        color: [0.5, 0.5, 0.55, 1.0],
                    });
                    state.construction_selected_room = Some(state.construction_rooms.len() - 1);
                    state.construction_dirty = true;
                }
            });
        });

    // ── RIGHT: details for the selected room + home-level controls ──
    egui::SidePanel::right("construction_details")
        .resizable(true)
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(RichText::new("Construction").size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.label(
                RichText::new("Drag orbit, middle-drag pan, wheel zoom, WASD fly, Space/Shift up+down. B closes. Hold F1 for keys.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.checkbox(&mut state.construction_plan_view,
                RichText::new("Top-down plan overlay").size(theme.font_size_small).color(theme.text_muted()));
            ui.horizontal(|ui| {
                ui.label(RichText::new("Ceiling height").color(theme.text_secondary()));
                if ui.add(egui::Slider::new(&mut state.construction_height, 2.5..=12.0).suffix(" m")).changed() {
                    state.construction_dirty = true;
                }
            });
            ui.add_space(theme.spacing_sm);
            ui.separator();

            // Clamp a stale selection (a room may have been deleted).
            if let Some(ri) = state.construction_selected_room {
                if ri >= state.construction_rooms.len() {
                    state.construction_selected_room = None;
                }
            }

            egui::ScrollArea::vertical().id_salt("details_scroll").show(ui, |ui| {
                if let Some(ri) = state.construction_selected_room {
                    let name = state.construction_rooms[ri].id.clone();
                    ui.label(RichText::new(name).strong().size(theme.font_size_body).color(theme.text_primary()));
                    ui.add_space(theme.spacing_xs);
                    // ---- Wall character (whole-wall): Solid / Auto / Open / Mirror. Doors and
                    // windows are now PLACED OBJECTS (the Openings list below), not wall kinds.
                    ui.label(RichText::new("Walls").strong().color(theme.text_primary()));
                    const WALL_CHOICES: [WallKind; 4] =
                        [WallKind::Solid, WallKind::Auto, WallKind::Open, WallKind::Mirror];
                    for wi in 0..4 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(WALL_LABELS[wi]).size(theme.font_size_small).color(theme.text_muted()));
                            let cur = state.construction_rooms[ri].walls[wi];
                            egui::ComboBox::from_id_salt(("wall", ri, wi))
                                .selected_text(cur.label())
                                .show_ui(ui, |ui| {
                                    for kd in WALL_CHOICES {
                                        if ui.selectable_value(&mut state.construction_rooms[ri].walls[wi], kd, kd.label()).changed() {
                                            state.construction_dirty = true;
                                        }
                                    }
                                });
                        });
                    }
                    ui.add_space(theme.spacing_xs);
                    ui.separator();
                    // ---- Openings (placed objects cut into otherwise-solid walls) ----
                    // Re-clamp every opening to its wall first: a room resize can shrink a wall,
                    // so this keeps the stored value equal to the real on-wall placement.
                    {
                        let ceiling = state.construction_height.max(2.5);
                        let room = &mut state.construction_rooms[ri];
                        for op in room.openings.iter_mut() {
                            let len = if op.wall < 2 { room.dimensions[0] } else { room.dimensions[2] };
                            op.w = op.w.clamp(0.4, len.max(0.4));
                            op.h = op.h.clamp(0.4, ceiling.max(0.4));
                            let hw = op.w * 0.5;
                            op.u = op.u.clamp(hw, (len - hw).max(hw));
                            if op.kind.floor_pinned() {
                                op.v = op.h * 0.5;
                            } else {
                                let hh = op.h * 0.5;
                                op.v = op.v.clamp(hh, (ceiling - hh).max(hh));
                            }
                        }
                    }
                    ui.label(RichText::new("Openings").strong().color(theme.text_primary()));
                    ui.label(
                        RichText::new("Add one, then drag its glowing handle in the view (or set the numbers). Doors sit on the floor; windows move up/down and resize.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.horizontal(|ui| {
                        for kind in EditorOpeningKind::ALL {
                            if ui.button(format!("Add {}", kind.label())).clicked() {
                                let (w, h) = kind.default_size();
                                let room = &state.construction_rooms[ri];
                                let wall = (0..4).find(|&wi| room.walls[wi] != WallKind::Open).unwrap_or(0);
                                let len = if wall < 2 { room.dimensions[0] } else { room.dimensions[2] };
                                let ceiling = state.construction_height.max(2.5);
                                let u = (len * 0.5).clamp(w * 0.5, (len - w * 0.5).max(w * 0.5));
                                let v = if kind.floor_pinned() {
                                    h * 0.5
                                } else {
                                    (0.9 + h * 0.5).min(ceiling - h * 0.5).max(h * 0.5)
                                };
                                state.construction_rooms[ri].openings.push(EditorOpening { kind, wall, u, v, w, h });
                                state.construction_dirty = true;
                            }
                        }
                    });
                    let mut remove_op: Option<usize> = None;
                    let n_op = state.construction_rooms[ri].openings.len();
                    for oi in 0..n_op {
                        let ceiling = state.construction_height.max(2.5);
                        ui.separator();
                        ui.horizontal(|ui| {
                            let cur = state.construction_rooms[ri].openings[oi].kind;
                            egui::ComboBox::from_id_salt(("op_kind", ri, oi))
                                .selected_text(cur.label())
                                .show_ui(ui, |ui| {
                                    for k in EditorOpeningKind::ALL {
                                        if ui.selectable_value(&mut state.construction_rooms[ri].openings[oi].kind, k, k.label()).changed() {
                                            if k.floor_pinned() {
                                                let oh = state.construction_rooms[ri].openings[oi].h;
                                                state.construction_rooms[ri].openings[oi].v = oh * 0.5;
                                            }
                                            state.construction_dirty = true;
                                        }
                                    }
                                });
                            let cur_wall = state.construction_rooms[ri].openings[oi].wall;
                            egui::ComboBox::from_id_salt(("op_wall", ri, oi))
                                .selected_text(WALL_LABELS[cur_wall])
                                .show_ui(ui, |ui| {
                                    for wi in 0..4 {
                                        if ui.selectable_value(&mut state.construction_rooms[ri].openings[oi].wall, wi, WALL_LABELS[wi]).changed() {
                                            state.construction_dirty = true;
                                        }
                                    }
                                });
                            if ui.small_button(RichText::new("Remove").color(theme.danger())).clicked() {
                                remove_op = Some(oi);
                            }
                        });
                        let wall = state.construction_rooms[ri].openings[oi].wall;
                        let len = state.construction_rooms[ri].wall_len(wall);
                        let floor = state.construction_rooms[ri].openings[oi].kind.floor_pinned();
                        ui.horizontal(|ui| {
                            let op = &mut state.construction_rooms[ri].openings[oi];
                            let hw = (op.w * 0.5).min(len * 0.5);
                            ui.label(RichText::new("Along").size(theme.font_size_small).color(theme.text_muted()));
                            if ui.add(egui::DragValue::new(&mut op.u).speed(0.05).suffix(" m").range(hw..=(len - hw).max(hw))).changed() {
                                state.construction_dirty = true;
                            }
                            if !floor {
                                let hh = (op.h * 0.5).min(ceiling * 0.5);
                                ui.label(RichText::new("Up").size(theme.font_size_small).color(theme.text_muted()));
                                if ui.add(egui::DragValue::new(&mut op.v).speed(0.05).suffix(" m").range(hh..=(ceiling - hh).max(hh))).changed() {
                                    state.construction_dirty = true;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            let op = &mut state.construction_rooms[ri].openings[oi];
                            ui.label(RichText::new("W").size(theme.font_size_small).color(theme.text_muted()));
                            if ui.add(egui::DragValue::new(&mut op.w).speed(0.05).suffix(" m").range(0.4..=len.max(0.4))).changed() {
                                state.construction_dirty = true;
                            }
                            ui.label(RichText::new("H").size(theme.font_size_small).color(theme.text_muted()));
                            if ui.add(egui::DragValue::new(&mut op.h).speed(0.05).suffix(" m").range(0.4..=ceiling.max(0.4))).changed() {
                                state.construction_dirty = true;
                            }
                        });
                    }
                    if let Some(oi) = remove_op {
                        if oi < state.construction_rooms[ri].openings.len() {
                            state.construction_rooms[ri].openings.remove(oi);
                            state.construction_dirty = true;
                        }
                    }
                    ui.add_space(theme.spacing_xs);
                    ui.separator();
                    // Position: pin (explicit) vs computed.
                    ui.horizontal(|ui| {
                        let mut pinned = state.construction_rooms[ri].position.is_some();
                        if ui.checkbox(&mut pinned, RichText::new("Pin position").size(theme.font_size_small).color(theme.text_muted())).changed() {
                            state.construction_rooms[ri].position = if pinned { Some([0.0, 0.0, 0.0]) } else { None };
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
                    ui.add_space(theme.spacing_xs);
                    if ui.button(RichText::new("Delete room").color(theme.danger())).clicked() {
                        state.construction_remove = Some(ri);
                    }
                } else {
                    ui.label(
                        RichText::new("Select a room -- click it in the view, or in the list on the left.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                }
            });

            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save layout").strong()).clicked() {
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

    // Apply a deferred delete after both panels (the index stays valid through the closures).
    if let Some(ri) = state.construction_remove.take() {
        if ri < state.construction_rooms.len() {
            state.construction_rooms.remove(ri);
            state.construction_dirty = true;
        }
        state.construction_selected_room = None;
    }

    // ── CENTER: top-down plan overlay (optional; default OFF so the orbit cam is primary) ──
    if state.construction_plan_view {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(150)))
            .show(ctx, |ui| {
                draw_floorplan_canvas(ui, theme, state);
            });
    }
}

/// Draw the top-down floor plan: every room as a rectangle seen from above (world X -> right,
/// world Z -> down, so North is up). Click selects; dragging moves on a 0.25 m grid + rebuilds
/// live. (v0.463)
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
        egui::pos2(rect.center().x + (wx - cx) * scale, rect.center().y + (wz - cz) * scale)
    };

    painter.text(
        rect.left_top() + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        "Top-down plan -- drag a room to move it (North is up).",
        egui::FontId::proportional(13.0),
        theme.text_secondary(),
    );

    let mut moved: Option<(usize, f32, f32)> = None;
    let mut clicked: Option<usize> = None;
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
        let resp = ui.interact(room_rect, egui::Id::new(("plan_room", ri)), egui::Sense::click_and_drag());
        let selected = state.construction_selected_room == Some(ri);
        let stroke = if selected {
            egui::Stroke::new(2.5, theme.accent())
        } else if resp.hovered() || resp.dragged() {
            egui::Stroke::new(2.0, theme.accent())
        } else {
            egui::Stroke::new(1.0, theme.border())
        };
        painter.rect_stroke(room_rect, egui::Rounding::same(2), stroke, egui::StrokeKind::Inside);
        painter.text(room_rect.center(), egui::Align2::CENTER_CENTER, &r.id, egui::FontId::proportional(12.0), theme.text_primary());
        if resp.clicked() || resp.drag_started() {
            clicked = Some(ri);
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            moved = Some((ri, d.x / scale, d.y / scale));
        }
    }

    if let Some(ri) = clicked {
        state.construction_selected_room = Some(ri);
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

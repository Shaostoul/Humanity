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
            ui.add_space(theme.spacing_xs);
            // ── Level (storey) selector (v0.471): focus the tree on one floor at a time. New
            //    rooms are created on the active level; multistory homes + the mall stack in Y.
            ui.horizontal(|ui| {
                ui.label(RichText::new("Level").size(theme.font_size_small).color(theme.text_muted()));
                if ui.small_button("-").clicked() { state.construction_level -= 1; }
                let lvl = state.construction_level;
                let lbl = if lvl == 0 { "Ground".to_string() } else { format!("{lvl}") };
                ui.label(RichText::new(lbl).strong().color(theme.accent()));
                if ui.small_button("+").clicked() { state.construction_level += 1; }
            });
            let active_level = state.construction_level;
            let here = state.construction_rooms.iter().filter(|r| r.level == active_level).count();
            let total = state.construction_rooms.len();
            ui.label(RichText::new(format!("{here} room(s) on this floor, {total} total"))
                .size(theme.font_size_small).color(theme.text_muted()));
            ui.add_space(theme.spacing_sm);
            egui::ScrollArea::vertical().id_salt("rooms_tree").show(ui, |ui| {
                let n = state.construction_rooms.len();
                for ri in 0..n {
                    // Filter the tree to the active storey (the room indices stay real, so a
                    // selection on another level remains valid; the level stepper reveals it).
                    if state.construction_rooms[ri].level != active_level { continue; }
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
                        level: active_level, // new rooms land on the floor you are viewing
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
                    // ---- Storey (v0.471): move this room between floors. World Y = level * story
                    // height; adjacency is level-aware so a stacked room never cuts a door downward.
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Storey").size(theme.font_size_small).color(theme.text_muted()));
                        if ui.small_button("-").clicked() {
                            state.construction_rooms[ri].level -= 1;
                            state.construction_dirty = true;
                        }
                        let rl = state.construction_rooms[ri].level;
                        let rlbl = if rl == 0 { "Ground".to_string() } else { format!("Level {rl}") };
                        ui.label(RichText::new(rlbl).color(theme.text_secondary()));
                        if ui.small_button("+").clicked() {
                            state.construction_rooms[ri].level += 1;
                            state.construction_dirty = true;
                        }
                    });
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
                    ui.separator();
                    // ── Machines in this room (v0.519: home-design parity -- players can
                    //    finally place machines, the same home.ron the AI edits). ──
                    ui.label(RichText::new("Machines").strong().color(theme.text_primary()));
                    {
                        let room_id = state.construction_rooms[ri].id.clone();
                        // Offset reach is derived from THIS room's real size (v0.522): a machine
                        // can be placed anywhere from the center out to a wall, but not past it.
                        // Was a fixed +/-40 m / 0..10 m, which left big rooms (the 144 m hangar)
                        // mostly unreachable and let machines clip through low (5 m) ceilings.
                        let room_dims = state.construction_rooms[ri].dimensions; // [W, H, D]
                        let hx = (room_dims[0] * 0.5).max(0.5); // half-width  -> x reach
                        let hz = (room_dims[2] * 0.5).max(0.5); // half-depth  -> z reach
                        let hy = room_dims[1].max(0.5); //          ceiling height -> y reach
                        // Collect display data under an immutable borrow, then mutate after.
                        let mut catalog_types: Vec<(String, String)> = Vec::new(); // (id, label)
                        let mut in_room: Vec<(usize, String)> = Vec::new(); // (instance idx, label)
                        if let Some(home) = &state.home_machines {
                            catalog_types = home
                                .catalog
                                .iter()
                                .map(|(id, d)| (id.clone(), if d.label.is_empty() { id.clone() } else { d.label.clone() }))
                                .collect();
                            catalog_types.sort_by(|a, b| a.1.cmp(&b.1));
                            for (i, inst) in home.instances.iter().enumerate() {
                                if inst.room == room_id {
                                    let label = home
                                        .catalog
                                        .get(&inst.machine)
                                        .map(|d| if d.label.is_empty() { inst.machine.clone() } else { d.label.clone() })
                                        .unwrap_or_else(|| inst.machine.clone());
                                    in_room.push((i, label));
                                }
                            }
                        }
                        let mut remove_idx: Option<usize> = None;
                        let mut add_machine: Option<String> = None;
                        if state.home_machines.is_none() {
                            ui.label(RichText::new("No machine layout loaded (home.ron).").size(theme.font_size_small).color(theme.text_muted()));
                        } else {
                            if in_room.is_empty() {
                                ui.label(RichText::new("None placed here yet.").size(theme.font_size_small).color(theme.text_muted()));
                            }
                            for (idx, label) in &in_room {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_secondary()));
                                    if ui.small_button(RichText::new("Remove").size(theme.font_size_small).color(theme.danger())).clicked() {
                                        remove_idx = Some(*idx);
                                    }
                                });
                                // Offset from the room center: x/z place it on the floor, y is
                                // height off the floor. Without this every machine sat at the
                                // center (stacked); now they can be positioned. Persists on
                                // "Save machines"; visible in-world on entry (editor 3D preview
                                // of placed machines is a follow-up).
                                if let Some(inst) =
                                    state.home_machines.as_mut().and_then(|h| h.instances.get_mut(*idx))
                                {
                                    ui.horizontal(|ui| {
                                        ui.add_space(theme.spacing_sm);
                                        ui.add(egui::DragValue::new(&mut inst.offset.0).speed(0.05).prefix("x ").suffix(" m").range(-hx..=hx));
                                        ui.add(egui::DragValue::new(&mut inst.offset.2).speed(0.05).prefix("z ").suffix(" m").range(-hz..=hz));
                                        ui.add(egui::DragValue::new(&mut inst.offset.1).speed(0.05).prefix("y ").suffix(" m").range(0.0..=hy));
                                    });
                                }
                            }
                            ui.add_space(theme.spacing_xs);
                            if state.home_machine_add_type.is_empty() {
                                if let Some((id, _)) = catalog_types.first() {
                                    state.home_machine_add_type = id.clone();
                                }
                            }
                            ui.horizontal(|ui| {
                                let cur = catalog_types
                                    .iter()
                                    .find(|(id, _)| *id == state.home_machine_add_type)
                                    .map(|(_, l)| l.clone())
                                    .unwrap_or_else(|| state.home_machine_add_type.clone());
                                egui::ComboBox::from_id_salt(("add_machine", ri))
                                    .selected_text(cur)
                                    .show_ui(ui, |ui| {
                                        for (id, label) in &catalog_types {
                                            ui.selectable_value(&mut state.home_machine_add_type, id.clone(), label.as_str());
                                        }
                                    });
                                if ui.button("Add").clicked() {
                                    add_machine = Some(state.home_machine_add_type.clone());
                                }
                            });
                        }
                        // Apply mutations after the display borrows are dropped.
                        if let Some(home) = state.home_machines.as_mut() {
                            if let Some(i) = remove_idx {
                                if i < home.instances.len() {
                                    // Capture the id first, then remove via the helper so any
                                    // connections touching this machine are pruned too (v0.522);
                                    // a bare instances.remove would leave them dangling.
                                    let id = home.instances[i].id.clone();
                                    home.remove_instance(&id);
                                }
                            }
                            if let Some(mtype) = add_machine {
                                if home.catalog.contains_key(&mtype) {
                                    let id = home.unique_instance_id(&mtype);
                                    home.instances.push(crate::machines::MachineInstance {
                                        id,
                                        machine: mtype,
                                        room: room_id.clone(),
                                        offset: (0.0, 0.0, 0.0),
                                    });
                                }
                            }
                        }
                    }
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
                if state.home_machines.is_some()
                    && ui.button(RichText::new("Save machines").strong()).clicked()
                {
                    state.home_machines_save = true;
                }
                if ui.button("Close").clicked() {
                    state.construction_active = false;
                }
            });
            ui.label(
                RichText::new("Save layout -> homestead_layout.ron;  Save machines -> home.ron.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });

    // Apply a deferred delete after both panels (the index stays valid through the closures).
    if let Some(ri) = state.construction_remove.take() {
        if ri < state.construction_rooms.len() {
            // Drop this room's machines too, so they do not become orphaned -- invisible
            // in-world AND un-removable through the GUI (you could no longer select the
            // deleted room to reach them). Capture the id BEFORE removing the room; persist
            // the cleanup so home.ron stays consistent with the layout. (v0.522)
            let dead_room = state.construction_rooms[ri].id.clone();
            if let Some(home) = state.home_machines.as_mut() {
                if home.remove_room(&dead_room) {
                    state.home_machines_save = true;
                }
            }
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
        // Multistory (v0.471): the top-down plan shows one storey at a time (the active level).
        if r.level != state.construction_level { continue; }
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

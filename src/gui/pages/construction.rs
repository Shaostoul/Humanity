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
use crate::ship::home_structure::{Opening, OpeningKind};

const WALL_LABELS: [&str; 4] = ["North", "South", "West", "East"];

/// Data-driven door/window animation styles (v0.534). The opening stores the chosen string; a later
/// stage animates from it. Listed here so the editor offers them; new styles are added by appending.
const OPENING_STYLES: [&str; 8] =
    ["swing", "slide", "iris", "rotate", "fold", "energy", "nanowall", "fixed"];

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // v0.534: when the home is a HomeStructure (a FIXED box + freely-drawn interior walls), the
    // editor is the node/wall editor. The legacy room-AABB editor below stays for other structures.
    if state.home_structure.is_some() {
        draw_wall_editor(ctx, theme, state);
        return;
    }
    // ── FOOTER: the placement palette (v0.527), a game-style bottom bar. Added first so it spans
    //    the full width with the side panels above it. Pick a category, click an item to place it
    //    in the selected room (viewport click-to-place is the next step). ──
    draw_palette(ctx, theme, state);

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
                        let mut room_machine_ids: Vec<(String, String)> = Vec::new(); // (id, label) in this room
                        let mut all_machine_ids: Vec<(String, String)> = Vec::new(); // (id, "label (room)") all machines
                        let mut conns_here: Vec<(usize, String, String, String)> = Vec::new(); // (conn idx, from-disp, to-disp, kind)
                        if let Some(home) = &state.home_machines {
                            catalog_types = home
                                .catalog
                                .iter()
                                .map(|(id, d)| (id.clone(), if d.label.is_empty() { id.clone() } else { d.label.clone() }))
                                .collect();
                            catalog_types.sort_by(|a, b| a.1.cmp(&b.1));
                            // One pass over the explicit instances: the per-room list, the
                            // connection-picker pools, and an id -> "label (room)" display map.
                            let mut id_disp: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
                            for (i, inst) in home.instances.iter().enumerate() {
                                let label = home
                                    .catalog
                                    .get(&inst.machine)
                                    .map(|d| if d.label.is_empty() { inst.machine.clone() } else { d.label.clone() })
                                    .unwrap_or_else(|| inst.machine.clone());
                                let disp = format!("{label} ({})", inst.room);
                                if inst.room == room_id {
                                    in_room.push((i, label.clone()));
                                    room_machine_ids.push((inst.id.clone(), label.clone()));
                                }
                                all_machine_ids.push((inst.id.clone(), disp.clone()));
                                id_disp.insert(inst.id.as_str(), disp);
                            }
                            // Connections whose source is a machine in THIS room (anchor on `from`
                            // so a connection is listed once, in its source room).
                            for (ci, c) in home.connections.iter().enumerate() {
                                if room_machine_ids.iter().any(|(id, _)| id == &c.from) {
                                    let from_d = id_disp.get(c.from.as_str()).cloned().unwrap_or_else(|| c.from.clone());
                                    let to_d = id_disp.get(c.to.as_str()).cloned().unwrap_or_else(|| c.to.clone());
                                    conns_here.push((ci, from_d, to_d, c.kind.clone()));
                                }
                            }
                        }
                        let mut remove_idx: Option<usize> = None;
                        let mut add_machine: Option<String> = None;
                        let mut remove_conn_idx: Option<usize> = None;
                        let mut add_conn: Option<(String, String, String)> = None;
                        // Any machine edit this frame -> ask the engine to refresh the live 3D view
                        // (so a move/add/remove/connect shows immediately, not on re-entry). (v0.525)
                        let mut machines_changed = false;
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
                                // height off the floor. Dragging now moves the machine LIVE in the
                                // 3D view (v0.525); persists on "Save machines".
                                if let Some(inst) =
                                    state.home_machines.as_mut().and_then(|h| h.instances.get_mut(*idx))
                                {
                                    ui.horizontal(|ui| {
                                        ui.add_space(theme.spacing_sm);
                                        machines_changed |= ui.add(egui::DragValue::new(&mut inst.offset.0).speed(0.05).prefix("x ").suffix(" m").range(-hx..=hx)).changed();
                                        machines_changed |= ui.add(egui::DragValue::new(&mut inst.offset.2).speed(0.05).prefix("z ").suffix(" m").range(-hz..=hz)).changed();
                                        machines_changed |= ui.add(egui::DragValue::new(&mut inst.offset.1).speed(0.05).prefix("y ").suffix(" m").range(0.0..=hy)).changed();
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
                            // ── Connections (Stage 2, v0.523): wire this room's machines to
                            //    others (power / water / nutrient / fuel / air / waste) -- the same
                            //    connections the AI authors in home.ron, now player-editable. ──
                            ui.add_space(theme.spacing_xs);
                            ui.label(RichText::new("Connections").strong().color(theme.text_primary()));
                            if conns_here.is_empty() {
                                ui.label(RichText::new("None from this room's machines.").size(theme.font_size_small).color(theme.text_muted()));
                            }
                            for (ci, from_d, to_d, kind) in &conns_here {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{from_d}  ->  {to_d}  ({kind})")).size(theme.font_size_small).color(theme.text_secondary()));
                                    if ui.small_button(RichText::new("Remove").size(theme.font_size_small).color(theme.danger())).clicked() {
                                        remove_conn_idx = Some(*ci);
                                    }
                                });
                            }
                            // Add a connection: from a machine in this room -> any machine, by kind.
                            if !room_machine_ids.is_empty() && !all_machine_ids.is_empty() {
                                // Keep the pickers pointing at valid ids (a placement/removal can
                                // invalidate the previous pick).
                                if !room_machine_ids.iter().any(|(id, _)| id == &state.home_conn_from) {
                                    state.home_conn_from = room_machine_ids[0].0.clone();
                                }
                                if !all_machine_ids.iter().any(|(id, _)| id == &state.home_conn_to) {
                                    state.home_conn_to = all_machine_ids[0].0.clone();
                                }
                                ui.horizontal(|ui| {
                                    let from_disp = room_machine_ids.iter().find(|(id, _)| id == &state.home_conn_from).map(|(_, l)| l.clone()).unwrap_or_default();
                                    egui::ComboBox::from_id_salt(("conn_from", ri)).selected_text(from_disp).width(90.0).show_ui(ui, |ui| {
                                        for (id, label) in &room_machine_ids {
                                            ui.selectable_value(&mut state.home_conn_from, id.clone(), label.as_str());
                                        }
                                    });
                                    ui.label(RichText::new("->").size(theme.font_size_small).color(theme.text_muted()));
                                    let to_disp = all_machine_ids.iter().find(|(id, _)| id == &state.home_conn_to).map(|(_, l)| l.clone()).unwrap_or_default();
                                    egui::ComboBox::from_id_salt(("conn_to", ri)).selected_text(to_disp).width(120.0).show_ui(ui, |ui| {
                                        for (id, label) in &all_machine_ids {
                                            ui.selectable_value(&mut state.home_conn_to, id.clone(), label.as_str());
                                        }
                                    });
                                });
                                ui.horizontal(|ui| {
                                    ui.add_space(theme.spacing_sm);
                                    egui::ComboBox::from_id_salt(("conn_kind", ri)).selected_text(state.home_conn_kind.clone()).width(80.0).show_ui(ui, |ui| {
                                        for k in ["power", "water", "nutrient", "fuel", "air", "waste"] {
                                            ui.selectable_value(&mut state.home_conn_kind, k.to_string(), k);
                                        }
                                    });
                                    if ui.button("Connect").clicked() {
                                        add_conn = Some((state.home_conn_from.clone(), state.home_conn_to.clone(), state.home_conn_kind.clone()));
                                    }
                                });
                            }
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
                                    machines_changed = true;
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
                                        rotation: 0.0,
                                    });
                                    machines_changed = true;
                                }
                            }
                            // Connection edits (v0.523). remove by index first (indices come from
                            // the just-collected list, valid this frame); add validates endpoints.
                            if let Some(ci) = remove_conn_idx {
                                machines_changed |= home.remove_connection(ci);
                            }
                            if let Some((from, to, kind)) = add_conn {
                                machines_changed |= home.add_connection(&from, &to, &kind);
                            }
                        }
                        // A machine edit happened -> the engine refreshes the live 3D view. (v0.525)
                        if machines_changed {
                            state.construction_machines_dirty = true;
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

            // ── Buildability (Stage 3, v0.524): the whole-home real-world-validity check --
            //    power balances, the battery carries the night, wiring is intact. Shown always
            //    (not per-room); the same report an AI can call before committing a design. ──
            if let Some(home) = &state.home_machines {
                draw_buildability(ui, theme, home);
            }

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
                    // Drop any held placement item so reopening doesn't drop a machine on the first
                    // click (the ghost slot itself is harmless + reused on reopen). (v0.531)
                    state.construction_place_type = None;
                    state.construction_structure_type = None; // safety, though structures need a HomeStructure
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

    // (v0.529: placement moved to the viewport. The palette now sets construction_place_type, the
    // engine renders it as a ghost on the cursor + drops it where you click the floor.)

    // ── CENTER: top-down plan overlay (optional; default OFF so the orbit cam is primary) ──
    if state.construction_plan_view {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(150)))
            .show(ctx, |ui| {
                draw_floorplan_canvas(ui, theme, state);
            });
    }
}

/// The home-structure editor (v0.534): a FIXED outer box + freely-drawn INTERIOR WALLS. The LEFT
/// panel lists the walls + the "Add wall" tool (click corner nodes on the floor, chaining segment
/// to segment); the RIGHT panel edits the selected wall's corners, height, and openings (doors /
/// windows, each with a data-driven animation STYLE). The footer palette still places machines.
/// Edits set `construction_structure_dirty` so the engine rebuilds the mesh live; Save persists
/// home_structure.ron (the same file the AI edits -- one model, edited the same way by both).
fn draw_wall_editor(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // Footer: the machine palette (places into the box's single "home" room for now).
    draw_palette(ctx, theme, state);

    // ── LEFT: the fixed box, the interior-wall list, and the wall-drawing tool ──
    egui::SidePanel::left("home_structure_walls")
        .resizable(true)
        .default_width(238.0)
        .show(ctx, |ui| {
            // Pin Save/Close at the BOTTOM so they never get pushed off-screen no matter how many
            // walls/machines/connections the home has (operator, v0.569). Declared before the scroll
            // so egui reserves its space; the scrollable content fills what remains above it.
            egui::TopBottomPanel::bottom("hs_left_actions")
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    ui.add_space(theme.spacing_xs);
                    ui.separator();
                    ui.add_space(theme.spacing_xs);
                    if ui.button(RichText::new("Save home").color(theme.text_primary())).clicked() {
                        state.construction_save = true;
                    }
                    if ui.button(RichText::new("Close").color(theme.text_muted())).clicked() {
                        state.construction_wall_mode = false;
                        state.construction_wall_start = None;
                        state.construction_place_type = None;
                        state.construction_structure_type = None;
                        state.construction_structure_selected = None;
                        state.construction_active = false;
                    }
                    ui.add_space(theme.spacing_xs);
                });

            egui::ScrollArea::vertical().id_salt("hs_left_scroll").show(ui, |ui| {
                ui.add_space(theme.spacing_md);
                ui.label(RichText::new("Home structure").size(theme.font_size_body).strong().color(theme.text_primary()));
                if let Some(hs) = &state.home_structure {
                    ui.label(RichText::new(format!("Fixed box  {:.0} x {:.0} x {:.0} m", hs.width, hs.depth, hs.height))
                        .size(theme.font_size_small).color(theme.text_muted()));
                }
                ui.add_space(theme.spacing_sm);

                // Wall drawing + all structural pieces moved to the footer "Structure" palette
                // (v0.583, operator: a dedicated section to the left of Defense). This panel keeps the
                // live status hint so the active tool's flow is always visible.
                if state.construction_wall_mode {
                    let hint = if state.construction_wall_start.is_some() {
                        "Drawing wall: click the next corner. Right-click to finish."
                    } else {
                        "Drawing wall: click the first corner on the floor."
                    };
                    ui.label(RichText::new(hint).size(theme.font_size_small).color(theme.accent()));
                } else if state.construction_structure_type.is_some() {
                    ui.label(RichText::new("Placing a structure: click the floor to drop it. [ and ] rotate it. Right-click cancels.")
                        .size(theme.font_size_small).color(theme.accent()));
                    // Place-at-height (v0.588): drop the piece this far above the floor -- set it to a
                    // staircase's top so a deck lands as an upper-level landing.
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Place at height").size(theme.font_size_small).color(theme.text_muted()));
                        ui.add(egui::DragValue::new(&mut state.construction_structure_place_y).speed(0.1).range(0.0..=200.0).suffix(" m"));
                        if ui.small_button("floor").clicked() {
                            state.construction_structure_place_y = 0.0;
                        }
                    });
                } else {
                    ui.label(RichText::new("Build from the footer palette below -- Structure (walls, stairs, ladders, ...) is the leftmost tab. Drag corner pins to move walls.")
                        .size(theme.font_size_small).color(theme.text_muted()));
                }
                // Object SEARCH at the very top of the panel (v0.603, operator: "move the search to the
                // top of the left panel above options"). Filters the Objects browser below.
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(egui::TextEdit::singleline(&mut state.construction_object_filter).hint_text("object name...").desired_width(150.0));
                    if ui.small_button("clear").clicked() {
                        state.construction_object_filter.clear();
                    }
                });
                ui.add_space(theme.spacing_xs);
                // Options / Dev (v0.595): a dedicated, collapsible home for every build-mode toggle so
                // they stop cluttering the panel top (operator: "a section dedicated to the toggles").
                egui::CollapsingHeader::new(RichText::new("Options / Dev").strong().color(theme.text_primary()))
                    .id_salt("hs_options_sec")
                    .default_open(false)
                    .show(ui, |ui| {
                        // Grid snap: endpoint + edge snapping are always on (airtight seals); this is the 0.25 m grid.
                        ui.checkbox(&mut state.construction_grid_snap, RichText::new("Grid snap (0.25 m)").size(theme.font_size_small).color(theme.text_primary()));
                        // Dimension overlay (v0.595): the floating measurement text -- toggle off to de-clutter.
                        ui.checkbox(&mut state.construction_dimension_overlay, RichText::new("Dimension overlay (measurements)").size(theme.font_size_small).color(theme.text_primary()));
                        // Dev overlay: keep the dimension overlay + door interaction rings visible in normal play too.
                        ui.checkbox(&mut state.construction_dev_overlay, RichText::new("Dev overlay in play").size(theme.font_size_small).color(theme.text_primary()));
                        // Helper gizmos: quiet the bounds/range overlays on machines/structures/roads/conduits + the wall wireframe.
                        ui.checkbox(&mut state.construction_show_helpers, RichText::new("Helper gizmos (bounds / wireframe / nodes)").size(theme.font_size_small).color(theme.text_primary()));
                        // GI master switch: off = only LOCAL placed lights illuminate. Rebuilds room_lights.
                        if ui.checkbox(&mut state.gi_enabled, RichText::new("Sun / global light (off = local lights only)").size(theme.font_size_small).color(theme.text_primary())).changed() {
                            state.construction_structure_dirty = true;
                        }
                        // Undo depth (Blender-style): how many editor actions Ctrl+Z can step back.
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Undo steps (Ctrl+Z / Ctrl+Shift+Z)").size(theme.font_size_small).color(theme.text_muted()));
                            ui.add(egui::DragValue::new(&mut state.construction_undo_depth).speed(1.0).range(1..=4096));
                        });
                    });
                ui.add_space(theme.spacing_sm);

                // Unified object browser (v0.596): the one consistent single-line-per-object list.
                // Click a row -> edit on the right; the per-category sections below fold away (their
                // ADD controls + graph/connection editors stay) and are removed once this is confirmed.
                draw_object_browser(ui, theme, state);
                ui.add_space(theme.spacing_sm);

                // Walls + Structures lists REMOVED (v0.597): they're in the unified Objects browser
                // above now (one consistent row each; click -> edit on the right). Walls are drawn from
                // the Structure palette; structural pieces are placed from it too.

                // Machines + utility-line CONNECTIONS (v0.536): the machine list moved to the browser;
                // this keeps the utility-line connection editor (wire machine A -> B by kind).
                draw_machines_and_connections(ui, theme, state);

                // Lights: the list moved to the browser; this keeps the "Add light..." picker. (v0.597)
                draw_lights_editor(ui, theme, state);

                // Conduit node graph (v0.581): place junction nodes + branch edges; pipes auto-route.
                draw_conduit_nodes(ui, theme, state);

                // Road graph (v0.586): nodes + edges; each edge a ribbon of a fixed road-class stack.
                draw_roads_editor(ui, theme, state);

                // Zones (v0.631, superstructure M1): labelled macro VOLUMES the structure is carved into.
                draw_zones_editor(ui, theme, state);

                // Rail graph (v0.635, superstructure M2): a multi-stop rail line as nodes + edges.
                draw_rail_editor(ui, theme, state);

                // Console (v0.578): a text-command ACT surface for an AI + a human -- the same struct
                // edits the gizmos make, driven by typed verbs. `help` lists them.
                egui::CollapsingHeader::new(RichText::new("Console (AI / dev)").strong().color(theme.text_primary()))
                    .id_salt("hs_console")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Type a command; 'help' lists them. State -> debug/home_snapshot.json")
                            .size(theme.font_size_small).color(theme.text_muted()));
                        let resp = ui.add(egui::TextEdit::singleline(&mut state.construction_console_input)
                            .hint_text("e.g. add_light ceiling_panel 27 2.7 44")
                            .desired_width(f32::INFINITY));
                        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.button("Run").clicked() || enter {
                            let line = state.construction_console_input.clone();
                            state.construction_console_output = exec_construction_command(state, &line);
                            state.construction_console_input.clear();
                        }
                        if !state.construction_console_output.is_empty() {
                            ui.label(RichText::new(&state.construction_console_output)
                                .size(theme.font_size_small).color(theme.text_secondary()));
                        }
                    });
            });
        });

    // ── RIGHT: the selected wall's corners + openings (doors/windows with animation styles) ──
    egui::SidePanel::right("home_structure_wall_details")
        .resizable(true)
        .default_width(252.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            // HELD building / structure to place takes the panel (v0.602): show its details +
            // connection points BEFORE you drop it (operator: "click a building to build... bring up
            // details + pertinent connection points -- water, electricity, internet, conveyor").
            if state.construction_place_type.is_some() {
                draw_building_info(ui, theme, state);
                return;
            }
            if state.construction_structure_type.is_some() {
                draw_held_structure_info(ui, theme, state);
                return;
            }
            // A selected ROAD / CONDUIT node takes the panel (v0.597): clicked its list row / gizmo.
            // Gate on NO other object being selected so a viewport pick of a wall/light/etc. (which
            // doesn't clear the browser's node selection) is never SHADOWED by a stale node detail --
            // node selections are only ever set by the browser, which clears the others first. (v0.597)
            let other_selected = state.construction_structure_selected.is_some()
                || state.construction_light_selected.is_some()
                || state.construction_machine_selected.is_some()
                || state.construction_wall_selected.is_some();
            if !other_selected && state.construction_road_node_selected.is_some() {
                draw_road_node_detail(ui, theme, state);
                return;
            }
            if !other_selected && state.construction_conduit_node_selected.is_some() {
                draw_conduit_node_detail(ui, theme, state);
                return;
            }
            // A selected PIPE/WIRE takes the panel (v0.626): clicked the connection in the 3D view.
            // try_pick_connection clears every other selection when it fires, so this is unambiguous.
            if !other_selected && state.construction_connection_selected.is_some() {
                draw_connection_detail(ui, theme, state);
                return;
            }
            // A selected ZONE takes the panel (v0.634): clicked its box in the 3D view.
            if !other_selected && state.construction_zone_selected.is_some() {
                draw_zone_detail(ui, theme, state);
                return;
            }
            // A selected STRUCTURE takes the panel (v0.583): clicked its gizmo or list row.
            if state.construction_structure_selected.is_some() {
                draw_structure_detail(ui, theme, state);
                return;
            }
            // A selected LIGHT takes the panel (v0.576): clicked its diamond gizmo in the viewport.
            if state.construction_light_selected.is_some() {
                draw_light_detail(ui, theme, state);
                return;
            }
            // A selected machine takes the panel (v0.553): clicked in the viewport or the list.
            if state.construction_machine_selected.is_some() {
                draw_machine_detail(ui, theme, state);
                return;
            }
            let sel = match state.construction_wall_selected {
                Some(s) => s,
                None => {
                    ui.label(RichText::new("Select a wall to edit its corners and openings, or use Add wall to draw one.")
                        .size(theme.font_size_small).color(theme.text_muted()));
                    return;
                }
            };
            let n = state.home_structure.as_ref().map_or(0, |h| h.walls.len());
            if sel >= n {
                state.construction_wall_selected = None;
                return;
            }
            ui.label(RichText::new(format!("Wall {}", sel + 1)).strong().size(theme.font_size_body).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);

            let mut changed = false;
            let mut wall_len = 0.0f32;
            if let Some(hs) = state.home_structure.as_mut() {
                let w = hs.width;
                let d = hs.depth;
                let hmax = hs.height;
                let wall = &mut hs.walls[sel];
                ui.horizontal(|ui| {
                    ui.label(RichText::new("A").color(theme.text_muted()));
                    ui.label("x");
                    changed |= ui.add(egui::DragValue::new(&mut wall.a.0).speed(0.1).range(0.0..=w)).changed();
                    ui.label("z");
                    changed |= ui.add(egui::DragValue::new(&mut wall.a.1).speed(0.1).range(0.0..=d)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("B").color(theme.text_muted()));
                    ui.label("x");
                    changed |= ui.add(egui::DragValue::new(&mut wall.b.0).speed(0.1).range(0.0..=w)).changed();
                    ui.label("z");
                    changed |= ui.add(egui::DragValue::new(&mut wall.b.1).speed(0.1).range(0.0..=d)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Height").color(theme.text_muted()));
                    changed |= ui.add(egui::DragValue::new(&mut wall.height).speed(0.1).range(0.1..=hmax)).changed();
                });
                // Material picker (v0.552): pick a real material; the wall re-colors to match and the
                // panel shows its real properties so the builder learns while building.
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Material").color(theme.text_muted()));
                    let mats = crate::ship::home_structure::wall_materials();
                    let cur_name = mats
                        .iter()
                        .find(|m| m.id == wall.material)
                        .map(|m| m.name.as_str())
                        .unwrap_or("(default)");
                    egui::ComboBox::from_id_salt("wall_material")
                        .selected_text(cur_name)
                        .show_ui(ui, |ui| {
                            for m in mats {
                                changed |= ui
                                    .selectable_value(&mut wall.material, m.id, format!("{} ({})", m.name, m.category))
                                    .changed();
                            }
                        });
                });
                if let Some(m) = crate::ship::home_structure::wall_material(wall.material) {
                    ui.label(RichText::new(format!("Density {:.0} kg/m3   Tensile {:.0} MPa", m.density_kg_m3, m.tensile_mpa))
                        .size(theme.font_size_small).color(theme.text_muted()));
                    ui.label(RichText::new(format!("Cost {:.2}/kg   {}", m.cost_per_kg, if m.renewable { "renewable" } else { "non-renewable" }))
                        .size(theme.font_size_small).color(theme.text_muted()));
                    ui.label(RichText::new(m.note.clone()).size(theme.font_size_small).color(theme.text_muted()));
                }
                // Thickness (v0.556): per-wall, down to a 1 mm paper screen. Drives the mesh + the
                // collider. "auto" reverts to the material's default. Shows cm for readability.
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Thickness").color(theme.text_muted()));
                    let mut t = wall.resolved_thickness();
                    if ui.add(egui::DragValue::new(&mut t).speed(0.005).range(0.001..=2.0).suffix(" m").fixed_decimals(3)).changed() {
                        wall.thickness = Some(t.max(0.001));
                        changed = true;
                    }
                    ui.label(RichText::new(format!("({:.0} cm)", t * 100.0)).size(theme.font_size_small).color(theme.text_muted()));
                    if wall.thickness.is_some() && ui.small_button("auto").clicked() {
                        wall.thickness = None;
                        changed = true;
                    }
                });
                let dx = wall.b.0 - wall.a.0;
                let dz = wall.b.1 - wall.a.1;
                wall_len = (dx * dx + dz * dz).sqrt();
                ui.label(RichText::new(format!("Length {wall_len:.1} m")).size(theme.font_size_small).color(theme.text_muted()));

                // Surface layers (v0.585): coat the wall top-to-bottom (rhino-lining, cladding, ...).
                // Layer 1 is the EXPOSED face -- it drives the rendered colour. Add/remove/reorder.
                ui.add_space(theme.spacing_sm);
                ui.label(RichText::new(format!("Surface layers ({})", wall.layers.len())).strong().color(theme.text_secondary()));
                ui.label(RichText::new(format!("Total {:.0} cm with layers; exposed = {}",
                        wall.total_thickness() * 100.0,
                        crate::ship::home_structure::wall_material(wall.exposed_material()).map(|m| m.name.clone()).unwrap_or_default()))
                    .size(theme.font_size_small).color(theme.text_muted()));
                let mut rm_layer: Option<usize> = None;
                let mut mv_layer: Option<(usize, usize)> = None;
                let nlayers = wall.layers.len();
                for li in 0..nlayers {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}.", li + 1)).size(theme.font_size_small).color(theme.text_muted()));
                        let mname = crate::ship::home_structure::wall_material(wall.layers[li].material)
                            .map(|m| m.name.clone())
                            .unwrap_or_else(|| format!("#{}", wall.layers[li].material));
                        ui.label(RichText::new(mname).size(theme.font_size_small).color(theme.text_primary()));
                        changed |= ui.add(egui::DragValue::new(&mut wall.layers[li].thickness_m)
                            .speed(0.002).range(0.001..=1.0).suffix(" m").fixed_decimals(3)).changed();
                        if li > 0 && ui.small_button("up").clicked() { mv_layer = Some((li, li - 1)); }
                        if li + 1 < nlayers && ui.small_button("dn").clicked() { mv_layer = Some((li, li + 1)); }
                        if ui.small_button("x").clicked() { rm_layer = Some(li); }
                    });
                }
                if let Some(i) = rm_layer { wall.layers.remove(i); changed = true; }
                if let Some((i, j)) = mv_layer { wall.layers.swap(i, j); changed = true; }
                egui::ComboBox::from_id_salt("wall_add_layer")
                    .selected_text("Add surface layer...")
                    .show_ui(ui, |ui| {
                        for m in crate::ship::home_structure::wall_materials() {
                            if ui.selectable_label(false, format!("{} ({})", m.name, m.category)).clicked() {
                                // New coat goes on TOP (index 0) -> it becomes the exposed face.
                                wall.layers.insert(0, crate::ship::home_structure::SurfaceLayer {
                                    material: m.id,
                                    thickness_m: 0.01,
                                });
                                changed = true;
                            }
                        }
                    });
            }

            ui.add_space(theme.spacing_md);
            ui.label(RichText::new("Openings").strong().color(theme.text_primary()));
            ui.horizontal(|ui| {
                if ui.button("+ Door").clicked() {
                    if let Some(hs) = state.home_structure.as_mut() {
                        hs.walls[sel].openings.push(Opening {
                            kind: OpeningKind::Door,
                            at: (wall_len * 0.5 - 0.5).max(0.0),
                            width: 1.0,
                            sill: 0.0,
                            height: 2.1,
                            style: "swing".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new()
                        });
                    }
                    changed = true;
                }
                if ui.button("+ Window").clicked() {
                    if let Some(hs) = state.home_structure.as_mut() {
                        hs.walls[sel].openings.push(Opening {
                            kind: OpeningKind::Window,
                            at: (wall_len * 0.5 - 0.75).max(0.0),
                            width: 1.5,
                            sill: 1.0,
                            height: 1.2,
                            style: "fixed".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new()
                        });
                    }
                    changed = true;
                }
            });

            let n_op = state.home_structure.as_ref().map_or(0, |h| h.walls[sel].openings.len());
            let mut remove_op: Option<usize> = None;
            for oi in 0..n_op {
                ui.add_space(theme.spacing_xs);
                ui.group(|ui| {
                    if let Some(hs) = state.home_structure.as_mut() {
                        let op = &mut hs.walls[sel].openings[oi];
                        let kind_label = match op.kind {
                            OpeningKind::Door => "Door",
                            OpeningKind::Window => "Window",
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(kind_label).strong().color(theme.accent()));
                            if ui.small_button("Remove").clicked() {
                                remove_op = Some(oi);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("at");
                            changed |= ui.add(egui::DragValue::new(&mut op.at).speed(0.1).range(0.0..=wall_len)).changed();
                            ui.label("width");
                            changed |= ui.add(egui::DragValue::new(&mut op.width).speed(0.1).range(0.1..=wall_len)).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("sill");
                            changed |= ui.add(egui::DragValue::new(&mut op.sill).speed(0.1).range(0.0..=3.0)).changed();
                            ui.label("height");
                            changed |= ui.add(egui::DragValue::new(&mut op.height).speed(0.1).range(0.1..=3.0)).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("style").color(theme.text_muted()));
                            egui::ComboBox::from_id_salt(("op_style", sel, oi))
                                .selected_text(op.style.clone())
                                .show_ui(ui, |ui| {
                                    for s in OPENING_STYLES {
                                        if ui.selectable_label(op.style == s, s).clicked() {
                                            op.style = s.to_string();
                                            changed = true;
                                        }
                                    }
                                });
                        });
                        // Door OPEN MODE (v0.564, operator's model): AUTO-open within a radius, or
                        // MANUAL (stays shut; locked/unlocked). Windows are fixed panes -- no mode.
                        if op.kind == OpeningKind::Door {
                            changed |= ui
                                .checkbox(&mut op.auto_open, RichText::new("Auto-open (vs manual)").size(theme.font_size_small).color(theme.text_muted()))
                                .changed();
                            if op.auto_open {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("open dist").color(theme.text_muted()));
                                    changed |= ui.add(egui::DragValue::new(&mut op.open_dist).speed(0.1).range(0.5..=12.0).suffix(" m")).changed();
                                });
                            } else {
                                changed |= ui
                                    .checkbox(&mut op.locked, RichText::new("Locked (stays shut; energy door glows red)").size(theme.font_size_small).color(theme.text_muted()))
                                    .changed();
                                // A manual door needs a way to open it (v0.567): a wall-mounted control
                                // panel beside the door the player walks up to and presses E.
                                changed |= ui
                                    .checkbox(&mut op.control_panel, RichText::new("Control panel (walk up + press E)").size(theme.font_size_small).color(theme.text_muted()))
                                    .changed();
                            }
                            // LOCKS (v0.570): a list of locks on this door; it is passable only when
                            // every lock is Unlocked/Broken. A locked door with a control panel is
                            // unlocked at the panel (E). Each lock is a type from lock_types.ron, so an
                            // AI or human sees every available kind in the Add picker.
                            ui.add_space(theme.spacing_xs);
                            ui.label(RichText::new(format!("Locks ({})", op.locks.len())).size(theme.font_size_small).color(theme.text_muted()));
                            let mut remove_lock: Option<usize> = None;
                            for li in 0..op.locks.len() {
                                ui.horizontal(|ui| {
                                    let name = crate::ship::lock_types::lock_type(&op.locks[li].type_id)
                                        .map(|t| t.name.clone())
                                        .unwrap_or_else(|| op.locks[li].type_id.clone());
                                    ui.label(RichText::new(name).size(theme.font_size_small).color(theme.text_primary()));
                                    egui::ComboBox::from_id_salt(("lock_state", sel, oi, li))
                                        .width(80.0)
                                        .selected_text(format!("{:?}", op.locks[li].state))
                                        .show_ui(ui, |ui| {
                                            for s in [
                                                crate::ship::lock_types::LockState::Locked,
                                                crate::ship::lock_types::LockState::Unlocked,
                                                crate::ship::lock_types::LockState::Broken,
                                            ] {
                                                if ui.selectable_value(&mut op.locks[li].state, s, format!("{s:?}")).clicked() {
                                                    changed = true;
                                                }
                                            }
                                        });
                                    if ui.small_button("x").clicked() {
                                        remove_lock = Some(li);
                                    }
                                });
                            }
                            if let Some(li) = remove_lock {
                                op.locks.remove(li);
                                changed = true;
                            }
                            egui::ComboBox::from_id_salt(("add_lock", sel, oi))
                                .selected_text("Add lock...")
                                .show_ui(ui, |ui| {
                                    for lt in crate::ship::lock_types::lock_types() {
                                        if ui.selectable_label(false, RichText::new(lt.name.clone()).size(theme.font_size_small)).clicked() {
                                            op.locks.push(crate::ship::home_structure::LockInstance {
                                                type_id: lt.id.clone(),
                                                state: crate::ship::lock_types::LockState::Locked,
                                                secret: None,
                                                offset: 0.0,
                                            });
                                            changed = true;
                                        }
                                    }
                                });
                        }
                    }
                });
            }
            if let Some(oi) = remove_op {
                if let Some(hs) = state.home_structure.as_mut() {
                    if oi < hs.walls[sel].openings.len() {
                        hs.walls[sel].openings.remove(oi);
                    }
                }
                changed = true;
            }

            if changed {
                state.construction_structure_dirty = true;
            }
        });
}

/// Machines + connections for the wall editor (v0.536). Lists the machines placed in the box (place
/// them from the footer palette) and lets you WIRE two of them with a resource -- each connection
/// then routes as a conduit (potable water = rigid copper, power = a flexible cord, else a hose) with
/// procedural ceiling hangers + material-aware passthrough gaskets. Edits flag
/// construction_machines_dirty so the conduits rebuild live.
/// Right-panel detail for the selected machine (v0.553): its type, room, position, power role, live
/// stats, and the connections it participates in -- so clicking a machine in the viewport or the
/// list surfaces everything about it, the same way clicking a wall does.
/// Right-panel detail for the SELECTED placed light (v0.576): its type, on-state, position, and
/// intensity/range overrides -- editable, so clicking a light's diamond gizmo brings up its info the
/// same way clicking a wall does. Deselect / Remove at the bottom.
fn draw_light_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let li = match state.construction_light_selected {
        Some(i) => i,
        None => return,
    };
    let n = state.home_structure.as_ref().map_or(0, |h| h.lights.len());
    if li >= n {
        state.construction_light_selected = None;
        return;
    }
    let mut changed = false;
    let mut remove = false;
    {
        let hs = match state.home_structure.as_mut() {
            Some(h) => h,
            None => return,
        };
        let t = crate::renderer::light::light_type(&hs.lights[li].type_id);
        let name = t.map(|t| t.name.clone()).unwrap_or_else(|| hs.lights[li].type_id.clone());
        let (def_i, def_r) = t.map(|t| (t.intensity, t.range)).unwrap_or((4.0, 4.0));
        ui.label(RichText::new(format!("Light: {name}")).strong().size(theme.font_size_body).color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(format!("Type  {}", hs.lights[li].type_id)).size(theme.font_size_small).color(theme.text_muted()));
        if let Some(t) = t {
            ui.label(RichText::new(format!("Kind  {:?}", t.kind)).size(theme.font_size_small).color(theme.text_muted()));
        }
        let light = &mut hs.lights[li];
        changed |= ui.checkbox(&mut light.on, RichText::new("On").size(theme.font_size_small).color(theme.text_primary())).changed();
        ui.horizontal(|ui| {
            ui.label(RichText::new("pos").size(theme.font_size_small).color(theme.text_muted()));
            changed |= ui.add(egui::DragValue::new(&mut light.pos.0).speed(0.2).prefix("x ").suffix(" m")).changed();
            changed |= ui.add(egui::DragValue::new(&mut light.pos.1).speed(0.2).prefix("y ").suffix(" m")).changed();
            changed |= ui.add(egui::DragValue::new(&mut light.pos.2).speed(0.2).prefix("z ").suffix(" m")).changed();
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("intensity").size(theme.font_size_small).color(theme.text_muted()));
            let mut v = light.intensity.unwrap_or(def_i);
            if ui.add(egui::DragValue::new(&mut v).speed(0.2).range(0.0..=50.0)).changed() {
                light.intensity = Some(v);
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("range").size(theme.font_size_small).color(theme.text_muted()));
            let mut v = light.range.unwrap_or(def_r);
            if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.1..=40.0).suffix(" m")).changed() {
                light.range = Some(v);
                changed = true;
            }
        });
        // Aim (v0.639): only a Spot light has a cone to point, so this is invisible for
        // Point/Bar/Emissive -- the same yaw/pitch convention the free camera uses (see
        // Camera::forward), so "yaw 0, pitch -90" reads intuitively as "straight down."
        if t.map(|t| t.kind) == Some(crate::renderer::light::LightKind::Spot) {
            ui.horizontal(|ui| {
                ui.label(RichText::new("aim").size(theme.font_size_small).color(theme.text_muted()));
                let d = glam::Vec3::new(light.dir.0, light.dir.1, light.dir.2);
                let d = if d.length_squared() > 1e-6 { d.normalize() } else { glam::Vec3::new(0.0, -1.0, 0.0) };
                let mut pitch_deg = d.y.clamp(-1.0, 1.0).asin().to_degrees();
                let mut yaw_deg = d.x.atan2(-d.z).to_degrees();
                let mut aim_changed = false;
                aim_changed |= ui.add(egui::DragValue::new(&mut yaw_deg).speed(1.0).prefix("yaw ").suffix(" deg")).changed();
                aim_changed |= ui.add(egui::DragValue::new(&mut pitch_deg).speed(1.0).range(-89.0..=89.0).prefix("pitch ").suffix(" deg")).changed();
                if aim_changed {
                    let yaw = yaw_deg.to_radians();
                    let pitch = pitch_deg.to_radians();
                    light.dir = (yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
                    changed = true;
                }
            });
        }
    }
    ui.add_space(theme.spacing_md);
    ui.horizontal(|ui| {
        if ui.button(RichText::new("Deselect").color(theme.text_muted())).clicked() {
            state.construction_light_selected = None;
        }
        if ui.button(RichText::new("Remove").color(theme.text_primary())).clicked() {
            remove = true;
        }
    });
    if remove {
        if let Some(hs) = state.home_structure.as_mut() {
            if li < hs.lights.len() {
                hs.lights.remove(li);
            }
        }
        state.construction_light_selected = None;
        changed = true;
    }
    if changed {
        state.construction_structure_dirty = true;
    }
}

/// Right-panel info for the building (machine) currently HELD for placement (v0.602): name, category,
/// size, electrical role, stat readouts, and its CONNECTION POINTS -- the utility kinds it ties into
/// (power / water / nutrient / fuel / air / waste), coloured to match the pipes. Derived from the
/// machine def's stats + power role (no schema change). Read-only; shown while you hold it to place.
fn draw_building_info(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(tid) = state.construction_place_type.clone() else { return };
    let def = state.home_machines.as_ref().and_then(|h| h.catalog.get(&tid).cloned());
    let Some(def) = def else {
        ui.label(RichText::new(format!("Placing {tid}")).strong().color(theme.text_primary()));
        ui.label(RichText::new("Click the floor to place. Right-click cancels.").size(theme.font_size_small).color(theme.text_muted()));
        return;
    };
    let name = if def.label.is_empty() { tid.clone() } else { def.label.clone() };
    ui.label(RichText::new(name).strong().size(theme.font_size_body).color(theme.text_primary()));
    ui.label(RichText::new(format!("{} -- {:.1} x {:.1} x {:.1} m", def.category, def.size.0, def.size.1, def.size.2))
        .size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_xs);
    if let Some(power) = &def.power {
        use crate::machines::MachinePower::*;
        let s = match power {
            Solar { peak_watts } => format!("Solar source -- up to {peak_watts:.0} W in full sun"),
            Generator { watts } => format!("Generator -- {watts:.0} W steady"),
            Consumer { watts, priority } => format!("Draws {watts:.0} W (shed priority {priority})"),
            Battery { capacity_wh, max_charge_w, max_discharge_w } => format!("Battery -- {capacity_wh:.0} Wh ({max_charge_w:.0}/{max_discharge_w:.0} W)"),
        };
        ui.label(RichText::new(s).size(theme.font_size_small).color(theme.text_secondary()));
    }
    if !def.stats.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Readouts").size(theme.font_size_small).strong().color(theme.text_muted()));
        for st in &def.stats {
            ui.label(RichText::new(format!("  {} : {}  ({})", st.kind, st.value, st.status)).size(theme.font_size_small).color(theme.text_muted()));
        }
    }
    // Connection points (v0.605): the machine's real declared PORTS -- which utility, which way it
    // flows (in/out/both), and the load (W) or flow (L/min), coloured to match the pipes. Falls back
    // to derive_ports() so a machine that only declares an electrical `power` role still shows a port.
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Connection points").strong().size(theme.font_size_small).color(theme.text_primary()));
    let ports = def.derive_ports();
    if ports.is_empty() {
        ui.label(RichText::new("  (standalone -- no utility hookups)").size(theme.font_size_small).color(theme.text_muted()));
    } else {
        for p in &ports {
            ui.label(RichText::new(format!("  {}", port_line(p))).size(theme.font_size_small).color(port_color(p)));
        }
    }
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Click the floor to place. Right-click cancels.").size(theme.font_size_small).color(theme.text_muted()));
}

/// One human-readable line for a machine port: direction arrow + utility + label + the load/flow.
/// Arrows (U+2190..U+21FF) are confirmed-rendering glyphs in our font (icon_glyph_lint).
fn port_line(p: &crate::utilities::Port) -> String {
    use crate::utilities::{PortDir, Utility};
    let arrow = match p.dir {
        PortDir::In => "\u{2190}",            // <- consumes
        PortDir::Out => "\u{2192}",           // -> supplies
        PortDir::Bidirectional => "\u{2194}", // <-> either way
    };
    let detail = if p.utility == Utility::Electricity {
        if p.watts > 0.0 { format!(" {:.0} W", p.watts) } else { String::new() }
    } else if p.flow_lpm > 0.0 {
        format!(" {:.1} L/min", p.flow_lpm)
    } else {
        String::new()
    };
    let label = if p.label.is_empty() { String::new() } else { format!("  ({})", p.label) };
    format!("{arrow} {}{detail}{label}", p.utility.id())
}

/// Pipe colour for a port's utility, matching the in-world connection tubes.
fn port_color(p: &crate::utilities::Port) -> egui::Color32 {
    let c = crate::machines::MachineHome::connection_color(p.utility.id());
    egui::Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8) // theme-exempt: utility-kind colour matching the pipes
}

/// Right-panel info for the STRUCTURE piece currently held for placement (v0.602): label, category,
/// size, and its note. Read-only.
fn draw_held_structure_info(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(tid) = state.construction_structure_type.clone() else { return };
    let Some(ty) = crate::ship::structure::structure_type(&tid) else {
        ui.label(RichText::new(format!("Placing {tid}")).strong().color(theme.text_primary()));
        return;
    };
    ui.label(RichText::new(&ty.label).strong().size(theme.font_size_body).color(theme.text_primary()));
    ui.label(RichText::new(format!("{} -- {:.1} x {:.1} x {:.1} m", ty.category, ty.size.0, ty.size.1, ty.size.2))
        .size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new(&ty.note).size(theme.font_size_small).color(theme.text_secondary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Click the floor to place. [ and ] rotate it. Right-click cancels.")
        .size(theme.font_size_small).color(theme.text_muted()));
}

fn draw_machine_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let id = match &state.construction_machine_selected {
        Some(id) => id.clone(),
        None => return,
    };
    // Resolve the instance + its catalog def + its connections as OWNED data so the immutable
    // home_machines borrow ends before the Remove/Deselect buttons mutate state.
    let resolved = state.home_machines.as_ref().and_then(|home| {
        home.all_instances().into_iter().find(|i| i.id == id).map(|inst| {
            let def = home.catalog.get(&inst.machine).cloned();
            let conns: Vec<(String, String, String)> = home
                .connections
                .iter()
                .filter(|c| c.from == id || c.to == id)
                .map(|c| (c.from.clone(), c.to.clone(), c.kind.clone()))
                .collect();
            // A direct instance can be removed; an array-derived one cannot (it is synthesized from
            // a MachineArray at load, so remove_instance would silently no-op).
            let is_direct = home.instances.iter().any(|i| i.id == id);
            (inst, def, conns, is_direct)
        })
    });
    let (inst, def, conns, is_direct) = match resolved {
        Some(t) => t,
        None => {
            // The machine was removed out from under the selection.
            state.construction_machine_selected = None;
            return;
        }
    };

    let title = def
        .as_ref()
        .map(|d| d.label.clone())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| inst.machine.clone());
    ui.label(RichText::new(title).strong().size(theme.font_size_body).color(theme.text_primary()));
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new(format!("Type  {}", inst.machine)).size(theme.font_size_small).color(theme.text_muted()));
    ui.label(RichText::new(format!("Room  {}", inst.room)).size(theme.font_size_small).color(theme.text_muted()));
    ui.label(RichText::new(format!("Position  {:.1}, {:.1}, {:.1} m", inst.offset.0, inst.offset.1, inst.offset.2))
        .size(theme.font_size_small).color(theme.text_muted()));

    // ROTATION (v0.633): yaw the machine about its vertical axis. Direct instances only (an array
    // member has no individual offset/rotation -- it is synthesized from its MachineArray).
    if is_direct {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Rotation").size(theme.font_size_small).color(theme.text_muted()));
            let mut rot = inst.rotation;
            let mut apply: Option<f32> = None;
            if ui.add(egui::DragValue::new(&mut rot).speed(1.0).suffix(" deg").range(0.0..=360.0)).changed() {
                apply = Some(rot.rem_euclid(360.0));
            }
            if ui.small_button("+90").clicked() {
                apply = Some((inst.rotation + 90.0).rem_euclid(360.0));
            }
            if let Some(r) = apply {
                if let Some(home) = state.home_machines.as_mut() {
                    if let Some(m) = home.instances.iter_mut().find(|m| m.id == id) {
                        m.rotation = r;
                    }
                }
                state.construction_machines_dirty = true;
            }
        });
    }

    if let Some(d) = &def {
        if let Some(power) = &d.power {
            ui.add_space(theme.spacing_xs);
            let role = match power {
                crate::machines::MachinePower::Solar { peak_watts } => format!("Solar  peak {peak_watts:.0} W"),
                crate::machines::MachinePower::Generator { watts } => format!("Generator  {watts:.0} W"),
                crate::machines::MachinePower::Consumer { watts, priority } => format!("Consumer  {watts:.0} W  (priority {priority})"),
                crate::machines::MachinePower::Battery { capacity_wh, .. } => format!("Battery  {capacity_wh:.0} Wh"),
            };
            ui.label(RichText::new(role).size(theme.font_size_small).color(theme.text_primary()));
        }
        if !d.stats.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new("Stats").strong().color(theme.text_primary()));
            for s in &d.stats {
                ui.label(RichText::new(format!("{}  {}", s.kind, s.value)).size(theme.font_size_small).color(theme.text_muted()));
            }
        }
        // Ports (v0.605): the physical hookups this machine needs/provides, coloured to the pipes.
        let ports = d.derive_ports();
        if !ports.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new("Ports").strong().color(theme.text_primary()));
            for p in &ports {
                ui.label(RichText::new(port_line(p)).size(theme.font_size_small).color(port_color(p)));
            }
            // v0.625: the ports also show as coloured handles above this machine in the 3D view --
            // DRAG one onto another machine to wire them (no dropdowns needed).
            ui.label(RichText::new("Tip: drag a port handle in the 3D view onto another machine to wire them.")
                .size(theme.font_size_small).italics().color(theme.text_muted()));
        }
    }

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Connections").strong().color(theme.text_primary()));
    if conns.is_empty() {
        ui.label(RichText::new("None.").size(theme.font_size_small).color(theme.text_muted()));
    } else {
        for (from, to, kind) in &conns {
            let other = if *from == id { to } else { from };
            ui.label(RichText::new(format!("{kind}: {other}")).size(theme.font_size_small).color(theme.text_muted()));
        }
    }

    ui.add_space(theme.spacing_md);
    ui.horizontal(|ui| {
        if ui.button("Deselect").clicked() {
            state.construction_machine_selected = None;
        }
        // Only a direct instance can be removed here; an array machine would silently no-op, so we
        // hide the button and explain instead of pretending it worked.
        if is_direct && ui.button("Remove").clicked() {
            if let Some(h) = state.home_machines.as_mut() {
                h.remove_instance(&id);
            }
            state.construction_machine_selected = None;
            state.construction_machines_dirty = true;
        }
    });
    if !is_direct {
        ui.label(RichText::new("Part of a machine array; edit the array to add or remove it.")
            .size(theme.font_size_small).color(theme.text_muted()));
    }
}

/// Per-home LIGHTS editor (v0.571): place local lights from the light_types.ron catalog so a room is
/// lit even with the sun / global illumination off. List + on-toggle + xyz position + remove, and an
/// "Add light..." picker enumerating every type. Edits flag the structure dirty so room_lights rebuild.
/// A construction-console verb (v0.578): name + usage + description. `help` and the parser both read
/// this list, so the ACT surface is enumerable -- an AI fetches the verbs, a human sees a cheat sheet.
struct ConsoleVerb {
    usage: &'static str,
    desc: &'static str,
}
const CONSOLE_VERBS: &[ConsoleVerb] = &[
    ConsoleVerb { usage: "help", desc: "List all commands." },
    ConsoleVerb { usage: "list", desc: "Summarise the home (full state in debug/home_snapshot.json)." },
    ConsoleVerb { usage: "add_wall x1 z1 x2 z2 [mat]", desc: "Add an interior wall; mat id optional (default 1=steel)." },
    ConsoleVerb { usage: "rm_wall <n>", desc: "Remove interior wall #n (1-based, as listed)." },
    ConsoleVerb { usage: "set_material <wall> <mat>", desc: "Set a wall's material (1 steel 2 concrete 3 oak 4 glass 5 aluminum 6 pine 7 granite 8 hdpe)." },
    ConsoleVerb { usage: "add_door <wall> <at> <width>", desc: "Add a door to a wall at distance `at` m, `width` m wide." },
    ConsoleVerb { usage: "add_window <wall> <at> <width> <sill> <height>", desc: "Add a window to a wall." },
    ConsoleVerb { usage: "set_style <wall> <opening> <style>", desc: "Set an opening's style (swing/slide/iris/energy/nanowall/fixed)." },
    ConsoleVerb { usage: "add_lock <wall> <opening> <type>", desc: "Lock an opening (type from lock_types.ron: metal_key/keypad/knob/crank/biometric)." },
    ConsoleVerb { usage: "add_light <type> x y z", desc: "Place a light (type from light_types.ron, e.g. ceiling_panel)." },
    ConsoleVerb { usage: "rm_light <n>", desc: "Remove light #n (1-based)." },
    ConsoleVerb { usage: "add_structure <type> x y z [yaw]", desc: "Place a structural piece (type from structure_types.ron: stairs/ramp/ladder/elevator/teleporter/train/road)." },
    ConsoleVerb { usage: "rm_structure <n>", desc: "Remove structural piece #n (1-based)." },
    ConsoleVerb { usage: "add_layer <wall> <material> <thickness>", desc: "Coat a wall: add a surface layer (material 1-8, thickness m). New layer becomes the exposed face." },
    ConsoleVerb { usage: "rm_layer <wall> <n>", desc: "Remove surface layer #n from a wall (1-based, top-first)." },
    ConsoleVerb { usage: "add_road_node <x> <z>", desc: "Add a road-graph node at (x,z); returns its N-id." },
    ConsoleVerb { usage: "rm_road_node <id>", desc: "Remove road node Nid + any edges touching it." },
    ConsoleVerb { usage: "add_road <fromN> <toN> <class> [width]", desc: "Wire a road edge of a class (footpath/residential/highway/runway)." },
    ConsoleVerb { usage: "rm_road <n>", desc: "Remove road edge #n (1-based)." },
];

/// Execute a construction console command against the LIVE home (v0.578) and return a result string.
/// Mutates gui_state.home_structure and flags it dirty, so the SAME live rebuild the gizmos use redraws
/// -- one edit path for an AI (typed verbs) and a human (the gizmos). Verbs are enumerable via `help`.
pub fn exec_construction_command(state: &mut GuiState, line: &str) -> String {
    let line = line.trim();
    if line.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    let f = |i: usize| -> Option<f32> { parts.get(i).and_then(|s| s.parse::<f32>().ok()) };
    let u = |i: usize| -> Option<usize> { parts.get(i).and_then(|s| s.parse::<usize>().ok()) };
    match parts[0] {
        "help" => {
            let mut s = String::from("Commands:\n");
            for v in CONSOLE_VERBS {
                s.push_str(&format!("  {} -- {}\n", v.usage, v.desc));
            }
            s.push_str("Also: drop debug/screenshot_request.json to capture the live 3D viewport -- \
                       the engine writes debug/screenshot_N.png + debug/screenshot_done.json within a frame.\n");
            s
        }
        "list" => match &state.home_structure {
            Some(h) => format!("{} walls, {} openings, {} lights. Full JSON: debug/home_snapshot.json",
                h.walls.len(), h.walls.iter().map(|w| w.openings.len()).sum::<usize>(), h.lights.len()),
            None => "No home loaded.".into(),
        },
        "add_wall" => {
            let (Some(x1), Some(z1), Some(x2), Some(z2)) = (f(1), f(2), f(3), f(4)) else {
                return "usage: add_wall x1 z1 x2 z2 [mat]".into();
            };
            let mat = u(5).unwrap_or(1) as u32;
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            let height = h.height;
            h.walls.push(crate::ship::home_structure::InteriorWall {
                a: (x1, z1), b: (x2, z2), height, material: mat, openings: Vec::new(), thickness: None, layers: Vec::new(),
            });
            state.construction_structure_dirty = true;
            format!("added wall #{} ({x1},{z1})->({x2},{z2}) mat {mat}", h.walls.len())
        }
        "rm_wall" => {
            let Some(i) = u(1) else { return "usage: rm_wall <n>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if i >= 1 && i <= h.walls.len() {
                h.walls.remove(i - 1);
                state.construction_structure_dirty = true;
                format!("removed wall #{i}")
            } else {
                format!("no wall #{i} (have {})", h.walls.len())
            }
        }
        "set_material" => {
            let (Some(w), Some(mat)) = (u(1), u(2)) else { return "usage: set_material <wall> <mat>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if w >= 1 && w <= h.walls.len() {
                h.walls[w - 1].material = mat as u32;
                state.construction_structure_dirty = true;
                format!("wall #{w} material -> {mat}")
            } else {
                format!("no wall #{w}")
            }
        }
        "add_door" => {
            let Some(w) = u(1) else { return "usage: add_door <wall> <at> <width>".into(); };
            let (Some(at), Some(width)) = (f(2), f(3)) else { return "usage: add_door <wall> <at> <width>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if w >= 1 && w <= h.walls.len() {
                h.walls[w - 1].openings.push(crate::ship::home_structure::Opening {
                    kind: crate::ship::home_structure::OpeningKind::Door,
                    at, width, sill: 0.0, height: 2.1, style: "swing".into(), open_dist: 2.6,
                    locked: false, auto_open: true, control_panel: false, locks: Vec::new(),
                });
                state.construction_structure_dirty = true;
                format!("added door to wall #{w} at {at} w {width}")
            } else {
                format!("no wall #{w}")
            }
        }
        "add_light" => {
            let Some(tid) = parts.get(1) else { return "usage: add_light <type> x y z".into(); };
            let (Some(x), Some(y), Some(z)) = (f(2), f(3), f(4)) else { return "usage: add_light <type> x y z".into(); };
            if crate::renderer::light::light_type(tid).is_none() {
                let ids: Vec<&str> = crate::renderer::light::light_types().iter().map(|t| t.id.as_str()).collect();
                return format!("unknown light type '{tid}'. types: {}", ids.join(", "));
            }
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            h.lights.push(crate::ship::home_structure::PlacedLight {
                type_id: tid.to_string(), pos: (x, y, z), dir: (0.0, -1.0, 0.0), on: true, color: None, intensity: None, range: None,
            });
            state.construction_structure_dirty = true;
            format!("added light #{} ({tid}) at ({x},{y},{z})", h.lights.len())
        }
        "rm_light" => {
            let Some(i) = u(1) else { return "usage: rm_light <n>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if i >= 1 && i <= h.lights.len() {
                h.lights.remove(i - 1);
                state.construction_structure_dirty = true;
                format!("removed light #{i}")
            } else {
                format!("no light #{i} (have {})", h.lights.len())
            }
        }
        "add_structure" => {
            let Some(tid) = parts.get(1) else { return "usage: add_structure <type> x y z [yaw]".into(); };
            let (Some(x), Some(y), Some(z)) = (f(2), f(3), f(4)) else { return "usage: add_structure <type> x y z [yaw]".into(); };
            if crate::ship::structure::structure_type(tid).is_none() {
                let ids: Vec<&str> = crate::ship::structure::structure_types().iter().map(|t| t.id.as_str()).collect();
                return format!("unknown structure type '{tid}'. types: {}", ids.join(", "));
            }
            if *tid == "wall" {
                return "wall is drawn, not placed -- use add_wall x1 z1 x2 z2 [mat].".into();
            }
            let yaw = f(5).unwrap_or(0.0);
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            h.structures.push(crate::ship::home_structure::PlacedStructure {
                type_id: tid.to_string(), pos: (x, y, z), rot_deg: yaw, pair: None,
            });
            state.construction_structure_dirty = true;
            format!("added structure #{} ({tid}) at ({x},{y},{z}) yaw {yaw}", h.structures.len())
        }
        "rm_structure" => {
            let Some(i) = u(1) else { return "usage: rm_structure <n>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if i >= 1 && i <= h.structures.len() {
                h.structures.remove(i - 1);
                for s in &mut h.structures {
                    if let Some(p) = s.pair {
                        if p + 1 == i { s.pair = None; } else if p + 1 > i { s.pair = Some(p - 1); }
                    }
                }
                // Keep the right-panel selection consistent (same fixup the GUI removers do), so a
                // console remove never leaves the detail panel pointed at a shifted piece. (v0.583)
                state.construction_structure_selected = match state.construction_structure_selected {
                    Some(s) if s + 1 == i => None,
                    Some(s) if s + 1 > i => Some(s - 1),
                    other => other,
                };
                state.construction_structure_dirty = true;
                format!("removed structure #{i}")
            } else {
                format!("no structure #{i} (have {})", h.structures.len())
            }
        }
        "add_layer" => {
            let (Some(w), Some(mat)) = (u(1), u(2)) else { return "usage: add_layer <wall> <material> <thickness>".into(); };
            let Some(th) = f(3) else { return "usage: add_layer <wall> <material> <thickness>".into(); };
            if crate::ship::home_structure::wall_material(mat as u32).is_none() {
                return format!("unknown material {mat} (1-8).");
            }
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if w >= 1 && w <= h.walls.len() {
                // New coat goes on TOP (index 0) -> the exposed face, matching the gizmo editor.
                h.walls[w - 1].layers.insert(0, crate::ship::home_structure::SurfaceLayer {
                    material: mat as u32, thickness_m: th.max(0.001),
                });
                state.construction_structure_dirty = true;
                format!("wall #{w}: added {:.0} cm layer of material {mat} on top", th * 100.0)
            } else {
                format!("no wall #{w}")
            }
        }
        "rm_layer" => {
            let (Some(w), Some(n)) = (u(1), u(2)) else { return "usage: rm_layer <wall> <n>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            match h.walls.get_mut(w.wrapping_sub(1)) {
                Some(wl) if w >= 1 && n >= 1 && n <= wl.layers.len() => {
                    wl.layers.remove(n - 1);
                    state.construction_structure_dirty = true;
                    format!("wall #{w}: removed layer #{n}")
                }
                _ => format!("no wall #{w} layer #{n}"),
            }
        }
        "add_window" => {
            let Some(w) = u(1) else { return "usage: add_window <wall> <at> <width> <sill> <height>".into(); };
            let (Some(at), Some(width), Some(sill), Some(height)) = (f(2), f(3), f(4), f(5)) else {
                return "usage: add_window <wall> <at> <width> <sill> <height>".into();
            };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if w >= 1 && w <= h.walls.len() {
                h.walls[w - 1].openings.push(crate::ship::home_structure::Opening {
                    kind: crate::ship::home_structure::OpeningKind::Window,
                    at, width, sill, height, style: "fixed".into(), open_dist: 2.6,
                    locked: false, auto_open: true, control_panel: false, locks: Vec::new(),
                });
                state.construction_structure_dirty = true;
                format!("added window to wall #{w}")
            } else {
                format!("no wall #{w}")
            }
        }
        "set_style" => {
            let (Some(w), Some(o)) = (u(1), u(2)) else { return "usage: set_style <wall> <opening> <style>".into(); };
            let Some(style) = parts.get(3) else { return "usage: set_style <wall> <opening> <style>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            match h.walls.get_mut(w.wrapping_sub(1)).and_then(|wl| wl.openings.get_mut(o.wrapping_sub(1))) {
                Some(op) if w >= 1 && o >= 1 => {
                    op.style = style.to_string();
                    state.construction_structure_dirty = true;
                    format!("wall #{w} opening #{o} style -> {style}")
                }
                _ => format!("no wall #{w} opening #{o}"),
            }
        }
        "add_lock" => {
            let (Some(w), Some(o)) = (u(1), u(2)) else { return "usage: add_lock <wall> <opening> <type>".into(); };
            let Some(tid) = parts.get(3) else { return "usage: add_lock <wall> <opening> <type>".into(); };
            if crate::ship::lock_types::lock_type(tid).is_none() {
                let ids: Vec<&str> = crate::ship::lock_types::lock_types().iter().map(|t| t.id.as_str()).collect();
                return format!("unknown lock type '{tid}'. types: {}", ids.join(", "));
            }
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            match h.walls.get_mut(w.wrapping_sub(1)).and_then(|wl| wl.openings.get_mut(o.wrapping_sub(1))) {
                Some(op) if w >= 1 && o >= 1 => {
                    op.locks.push(crate::ship::home_structure::LockInstance {
                        type_id: tid.to_string(), state: crate::ship::lock_types::LockState::Locked, secret: None, offset: 0.0,
                    });
                    state.construction_structure_dirty = true;
                    format!("locked wall #{w} opening #{o} with {tid}")
                }
                _ => format!("no wall #{w} opening #{o}"),
            }
        }
        "add_road_node" => {
            let (Some(x), Some(z)) = (f(1), f(2)) else { return "usage: add_road_node <x> <z>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            let id = h.unique_road_node_id();
            h.road_nodes.push(crate::ship::home_structure::RoadNode { id, pos: (x, z) });
            state.construction_structure_dirty = true;
            format!("added road node N{id} at ({x},{z})")
        }
        "rm_road_node" => {
            let Some(id) = u(1).map(|v| v as u32) else { return "usage: rm_road_node <id>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if h.road_node_pos(id).is_some() {
                h.remove_road_node(id);
                state.construction_structure_dirty = true;
                format!("removed road node N{id} (+ its edges)")
            } else {
                format!("no road node N{id}")
            }
        }
        "add_road" => {
            let (Some(from), Some(to)) = (u(1).map(|v| v as u32), u(2).map(|v| v as u32)) else {
                return "usage: add_road <fromN> <toN> <class> [width]".into();
            };
            let Some(cls) = parts.get(3) else { return "usage: add_road <fromN> <toN> <class> [width]".into(); };
            if crate::ship::structure::road_type(cls).is_none() {
                let ids: Vec<&str> = crate::ship::structure::road_types().iter().map(|t| t.id.as_str()).collect();
                return format!("unknown road class '{cls}'. classes: {}", ids.join(", "));
            }
            if from == to {
                return "a road edge needs two different nodes.".into();
            }
            let width = f(4).unwrap_or(4.0).max(0.5);
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if h.road_node_pos(from).is_none() || h.road_node_pos(to).is_none() {
                return format!("need both nodes (N{from}, N{to}).");
            }
            h.road_edges.push(crate::ship::home_structure::RoadEdge { from, to, class: cls.to_string(), width });
            state.construction_structure_dirty = true;
            format!("added {cls} road N{from}->N{to} ({width} m wide)")
        }
        "rm_road" => {
            let Some(i) = u(1) else { return "usage: rm_road <n>".into(); };
            let Some(h) = state.home_structure.as_mut() else { return "No home loaded.".into(); };
            if i >= 1 && i <= h.road_edges.len() {
                h.road_edges.remove(i - 1);
                state.construction_structure_dirty = true;
                format!("removed road edge #{i}")
            } else {
                format!("no road edge #{i} (have {})", h.road_edges.len())
            }
        }
        other => format!("unknown command '{other}'. try: help"),
    }
}

/// Display string for a conduit endpoint (v0.581).
fn conduit_end_str(e: &crate::machines::ConduitEnd) -> String {
    match e {
        crate::machines::ConduitEnd::Machine(id) => format!("M:{id}"),
        crate::machines::ConduitEnd::Node(id) => format!("N:{id}"),
    }
}
/// Parse a combo key ("m:id" / "n:id") back to a ConduitEnd (v0.581).
fn conduit_parse_end(k: &str) -> Option<crate::machines::ConduitEnd> {
    if let Some(id) = k.strip_prefix("m:") {
        Some(crate::machines::ConduitEnd::Machine(id.to_string()))
    } else {
        k.strip_prefix("n:").map(|id| crate::machines::ConduitEnd::Node(id.to_string()))
    }
}

/// Human label for a zone type id (v0.631), falling back to the raw id.
fn zone_label(type_id: &str) -> String {
    crate::ship::structure::zone_type(type_id).map(|t| t.label.clone()).unwrap_or_else(|| type_id.to_string())
}

/// ZONES editor (v0.631, superstructure M1): add/list/resize the macro VOLUMES the structure is carved
/// into (residential, industrial, hangar, the civic mall, ...). Render is a wireframe box per zone in
/// build mode (lib.rs). Deferred actions so it never holds a home_structure borrow across the closures.
fn draw_zones_editor(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let zones: Vec<(String, String, (f32, f32, f32), (f32, f32, f32))> = match state.home_structure.as_ref() {
        Some(h) => h.zones.iter().map(|z| (z.id.clone(), z.type_id.clone(), z.origin, z.size)).collect(),
        None => return,
    };
    let types = crate::ship::structure::zone_types();
    if types.is_empty() {
        return;
    }
    let mut add_type: Option<String> = None;
    let mut remove: Option<String> = None;
    let mut set: Option<(String, (f32, f32, f32), (f32, f32, f32))> = None;
    egui::CollapsingHeader::new(RichText::new(format!("Zones ({})", zones.len())).strong().color(theme.text_primary()))
        .id_salt("hs_zones")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Macro districts: residential, industrial, hangar, the mall... (M1)")
                .size(theme.font_size_small).color(theme.text_muted()));
            if state.zone_add_type.is_empty() {
                state.zone_add_type = types[0].id.clone();
            }
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("zone_add").width(160.0)
                    .selected_text(zone_label(&state.zone_add_type))
                    .show_ui(ui, |ui| {
                        for t in types {
                            ui.selectable_value(&mut state.zone_add_type, t.id.clone(), &t.label);
                        }
                    });
                if ui.button("Add zone").clicked() {
                    add_type = Some(state.zone_add_type.clone());
                }
            });
            for (id, type_id, origin, size) in &zones {
                let c = crate::ship::structure::zone_type(type_id).map(|t| t.color).unwrap_or((0.6, 0.6, 0.6));
                let col = egui::Color32::from_rgb((c.0 * 255.0) as u8, (c.1 * 255.0) as u8, (c.2 * 255.0) as u8);
                ui.add_space(theme.spacing_xs);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(zone_label(type_id)).size(theme.font_size_small).strong().color(col));
                    ui.label(RichText::new(id).size(theme.font_size_small).color(theme.text_muted()));
                    if ui.small_button("x").clicked() {
                        remove = Some(id.clone());
                    }
                });
                let (mut o, mut s, mut ch) = (*origin, *size, false);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("at").size(theme.font_size_small).color(theme.text_muted()));
                    ch |= ui.add(egui::DragValue::new(&mut o.0).speed(0.5).prefix("x ").suffix(" m")).changed();
                    ch |= ui.add(egui::DragValue::new(&mut o.1).speed(0.5).prefix("y ").suffix(" m")).changed();
                    ch |= ui.add(egui::DragValue::new(&mut o.2).speed(0.5).prefix("z ").suffix(" m")).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("size").size(theme.font_size_small).color(theme.text_muted()));
                    ch |= ui.add(egui::DragValue::new(&mut s.0).speed(0.5).prefix("w ").range(1.0..=400.0)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut s.1).speed(0.5).prefix("h ").range(1.0..=100.0)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut s.2).speed(0.5).prefix("d ").range(1.0..=400.0)).changed();
                });
                if ch {
                    set = Some((id.clone(), o, s));
                }
            }
        });
    if let Some(hs) = state.home_structure.as_mut() {
        if let Some(t) = add_type {
            let sz = crate::ship::structure::zone_type(&t).map(|zt| zt.default_size).unwrap_or((20.0, 4.0, 20.0));
            // Seed the new zone centred in the structure footprint.
            let origin = ((hs.width * 0.5 - sz.0 * 0.5).max(0.0), 0.0, (hs.depth * 0.5 - sz.2 * 0.5).max(0.0));
            hs.add_zone(&t, origin, sz);
        }
        if let Some(id) = remove {
            hs.remove_zone(&id);
        }
        if let Some((id, o, s)) = set {
            if let Some(z) = hs.zones.iter_mut().find(|z| z.id == id) {
                z.origin = o;
                z.size = s;
            }
        }
    }
}

/// Conduit NODE-GRAPH editor (v0.581): place junction nodes + branch edges (machine/node -> machine/
/// node); each edge auto-routes as a real pipe (reusing route_conduit). The node-graph model the
/// operator asked for; main/sub/subsub hierarchy is a later stage. Uses deferred actions so it never
/// holds a home_machines borrow across the egui closures.
fn draw_conduit_nodes(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (bw, bd, bh) = state.home_structure.as_ref().map(|h| (h.width, h.depth, h.height)).unwrap_or((55.0, 89.0, 3.0));
    let machine_ids: Vec<String> = state.home_machines.as_ref().map(|h| h.all_instances().into_iter().map(|i| i.id).collect()).unwrap_or_default();
    let nodes: Vec<(String, (f32, f32, f32))> = state.home_machines.as_ref().map(|h| h.conduit_nodes.iter().map(|n| (n.id.clone(), n.pos)).collect()).unwrap_or_default();
    let edges: Vec<(String, String, String)> = state.home_machines.as_ref().map(|h| h.conduit_edges.iter().map(|e| (conduit_end_str(&e.from), conduit_end_str(&e.to), e.kind.clone())).collect()).unwrap_or_default();
    let mut add_node = false;
    let mut remove_node: Option<String> = None;
    let mut set_pos: Option<(String, (f32, f32, f32))> = None;
    let mut add_edge: Option<(String, String, String)> = None;
    let mut remove_edge: Option<usize> = None;
    egui::CollapsingHeader::new(RichText::new(format!("Conduit nodes ({}) / edges ({})", nodes.len(), edges.len())).strong().color(theme.text_primary()))
        .id_salt("hs_conduit")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Junction nodes; pipes auto-route through them. (Stage 1)").size(theme.font_size_small).color(theme.text_muted()));
            ui.horizontal(|ui| {
                if ui.button("Add node (box centre)").clicked() {
                    add_node = true;
                }
                // v0.629: place nodes by CLICKING the floor in the 3D view (a "main line" you then drag
                // machine ports onto). Toggles a place mode; right-click in the view cancels it.
                let placing = state.construction_place_conduit_node;
                let label = if placing { "Placing... (right-click to stop)" } else { "Place in view" };
                if ui.button(RichText::new(label).color(if placing { theme.accent() } else { theme.text_secondary() })).clicked() {
                    state.construction_place_conduit_node = !placing;
                }
            });
            for (id, pos) in &nodes {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(id).size(theme.font_size_small).color(theme.text_primary()));
                    let mut p = *pos;
                    let mut ch = false;
                    ch |= ui.add(egui::DragValue::new(&mut p.0).speed(0.2).prefix("x ").range(0.0..=bw)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut p.1).speed(0.1).prefix("y ").range(0.0..=bh)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut p.2).speed(0.2).prefix("z ").range(0.0..=bd)).changed();
                    if ch {
                        set_pos = Some((id.clone(), p));
                    }
                    if ui.small_button("x").clicked() {
                        remove_node = Some(id.clone());
                    }
                });
            }
            let endpoints: Vec<(String, String)> = machine_ids.iter().map(|id| (format!("m:{id}"), format!("M:{id}")))
                .chain(nodes.iter().map(|(id, _)| (format!("n:{id}"), format!("N:{id}")))).collect();
            if endpoints.len() >= 2 {
                if state.conduit_from.is_empty() { state.conduit_from = endpoints[0].0.clone(); }
                if state.conduit_to.is_empty() { state.conduit_to = endpoints[1].0.clone(); }
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("cd_from").width(78.0).selected_text(state.conduit_from.clone()).show_ui(ui, |ui| {
                        for (k, l) in &endpoints { ui.selectable_value(&mut state.conduit_from, k.clone(), l); }
                    });
                    ui.label("->");
                    egui::ComboBox::from_id_salt("cd_to").width(78.0).selected_text(state.conduit_to.clone()).show_ui(ui, |ui| {
                        for (k, l) in &endpoints { ui.selectable_value(&mut state.conduit_to, k.clone(), l); }
                    });
                });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("cd_kind").width(82.0).selected_text(state.conduit_kind.clone()).show_ui(ui, |ui| {
                        for k in ["water", "power", "greywater", "gas"] { ui.selectable_value(&mut state.conduit_kind, k.to_string(), k); }
                    });
                    if ui.button("Branch").clicked() {
                        add_edge = Some((state.conduit_from.clone(), state.conduit_to.clone(), state.conduit_kind.clone()));
                    }
                });
            } else {
                ui.label(RichText::new("Place 2+ machines/nodes to branch.").size(theme.font_size_small).color(theme.text_muted()));
            }
            for (i, (fr, to, k)) in edges.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{fr} -> {to} ({k})")).size(theme.font_size_small).color(theme.text_muted()));
                    if ui.small_button("x").clicked() {
                        remove_edge = Some(i);
                    }
                });
            }
        });
    let mut changed = false;
    if let Some(h) = state.home_machines.as_mut() {
        if add_node {
            h.add_conduit_node((bw * 0.5, bh * 0.5, bd * 0.5), "water");
            changed = true;
        }
        if let Some((id, p)) = set_pos {
            h.move_conduit_node(&id, p);
            changed = true;
        }
        if let Some(id) = remove_node {
            h.remove_conduit_node(&id);
            changed = true;
        }
        if let Some((fk, tk, kind)) = add_edge {
            if let (Some(from), Some(to)) = (conduit_parse_end(&fk), conduit_parse_end(&tk)) {
                if h.add_conduit_edge(from, to, &kind) {
                    changed = true;
                }
            }
        }
        if let Some(i) = remove_edge {
            h.remove_conduit_edge(i);
            changed = true;
        }
    }
    if changed {
        state.construction_machines_dirty = true;
    }
}

fn draw_lights_editor(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // The light LIST + on/off + position editing moved to the Objects browser (toggle) + the right
    // detail panel (v0.597). This keeps only the "Add light" picker. One consistent style.
    let mut changed = false;
    {
        let hs = match state.home_structure.as_mut() {
            Some(h) => h,
            None => return,
        };
        egui::CollapsingHeader::new(RichText::new("Add light").strong().color(theme.text_primary()))
            .id_salt("hs_lights_sec")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(RichText::new("Lights appear in Objects above (checkbox = on/off, click = edit). Turn off Sun / global light to see them alone.")
                    .size(theme.font_size_small).color(theme.text_muted()));
                egui::ComboBox::from_id_salt("hs_add_light")
                    .selected_text("Add light...")
                    .show_ui(ui, |ui| {
                        for lt in crate::renderer::light::light_types() {
                            if ui.selectable_label(false, RichText::new(lt.name.clone()).size(theme.font_size_small)).clicked() {
                                let pos = (hs.width * 0.5, (hs.height - 0.3).max(0.3), hs.depth * 0.5);
                                hs.lights.push(crate::ship::home_structure::PlacedLight {
                                    type_id: lt.id.clone(),
                                    pos,
                                    dir: (0.0, -1.0, 0.0),
                                    on: true,
                                    color: None,
                                    intensity: None,
                                    range: None,
                                });
                                changed = true;
                            }
                        }
                    });
            });
    }
    if changed {
        // Rebuild the homestead so room_lights pick up the new placed lights (home_lights).
        state.construction_structure_dirty = true;
    }
}

// draw_structures_editor REMOVED (v0.597): structures are in the unified Objects browser now.

/// The selected-structure detail (v0.583): type, pose (x/y/z + yaw), teleporter pairing, remove.
/// Edits mark the home dirty so the mesh rebuilds live.
fn draw_structure_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let sel = match state.construction_structure_selected {
        Some(s) => s,
        None => return,
    };
    let n = state.home_structure.as_ref().map_or(0, |h| h.structures.len());
    if sel >= n {
        state.construction_structure_selected = None;
        return;
    }
    let mut changed = false;
    let mut deselect = false;
    // Snapshot fields needed for the pairing combo (other teleporters) before the mutable borrow.
    let pieces: Vec<(usize, String)> = state
        .home_structure
        .as_ref()
        .map(|h| {
            h.structures
                .iter()
                .enumerate()
                .map(|(i, s)| (i, s.type_id.clone()))
                .collect()
        })
        .unwrap_or_default();
    if let Some(hs) = state.home_structure.as_mut() {
        let ps = &mut hs.structures[sel];
        let ty = crate::ship::structure::structure_type(&ps.type_id);
        let label = ty.map(|t| t.label.clone()).unwrap_or_else(|| ps.type_id.clone());
        let kind = ty.map(|t| t.kind);
        ui.label(RichText::new(label).strong().size(theme.font_size_body).color(theme.text_primary()));
        if let Some(t) = ty {
            ui.label(RichText::new(&t.note).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.label(RichText::new("pos").size(theme.font_size_small).color(theme.text_muted()));
            changed |= ui.add(egui::DragValue::new(&mut ps.pos.0).speed(0.1).prefix("x ").suffix(" m")).changed();
            changed |= ui.add(egui::DragValue::new(&mut ps.pos.1).speed(0.1).prefix("y ").suffix(" m")).changed();
            changed |= ui.add(egui::DragValue::new(&mut ps.pos.2).speed(0.1).prefix("z ").suffix(" m")).changed();
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("yaw").size(theme.font_size_small).color(theme.text_muted()));
            changed |= ui.add(egui::DragValue::new(&mut ps.rot_deg).speed(5.0).range(0.0..=360.0).suffix(" deg")).changed();
            if ui.small_button("rotate 90").clicked() {
                ps.rot_deg = (ps.rot_deg + 90.0) % 360.0;
                changed = true;
            }
        });
        // Pairing (the generic `pair` field): a TELEPORTER pairs with another teleporter (jump
        // destination, v0.584); a TRAIN platform pairs with another platform (a rail line connects
        // them, v0.592). Pick a SAME-TYPE partner. The kind decides how the pair is interpreted.
        let pairable = matches!(
            kind,
            Some(crate::ship::structure::StructureKind::Teleporter)
                | Some(crate::ship::structure::StructureKind::Train)
        );
        if pairable {
            ui.add_space(theme.spacing_xs);
            let cur = ps.pair;
            let my_type = ps.type_id.clone();
            let noun = if kind == Some(crate::ship::structure::StructureKind::Train) { "platform" } else { "teleporter" };
            let cur_txt = cur.map(|p| format!("-> #{}", p + 1)).unwrap_or_else(|| "(no pair)".into());
            egui::ComboBox::from_id_salt("hs_structure_pair")
                .selected_text(cur_txt)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(cur.is_none(), "(no pair)").clicked() {
                        ps.pair = None;
                        changed = true;
                    }
                    for (i, tid) in &pieces {
                        if *i == sel || *tid != my_type {
                            continue;
                        }
                        if ui.selectable_label(cur == Some(*i), format!("#{} {noun}", i + 1)).clicked() {
                            ps.pair = Some(*i);
                            changed = true;
                        }
                    }
                });
        }
        ui.add_space(theme.spacing_sm);
        if ui.button(RichText::new("Remove").color(theme.danger())).clicked() {
            hs.structures.remove(sel);
            for s in &mut hs.structures {
                if let Some(p) = s.pair {
                    if p == sel { s.pair = None; } else if p > sel { s.pair = Some(p - 1); }
                }
            }
            deselect = true;
            changed = true;
        }
    }
    if deselect {
        state.construction_structure_selected = None;
    }
    if changed {
        state.construction_structure_dirty = true;
    }
}

/// The road-graph editor (v0.586): nodes + edges. Each edge is a ribbon of a fixed road CLASS (its
/// material stack). Add nodes, wire edges between them with a class + width; the mesh rebuilds live.
/// RAIL-graph editor (v0.635, superstructure M2): drop rail stops + wire straight edges into a multi-stop
/// line. Mirrors the road editor (simpler -- no class/width). Renders as a gizmo (lib.rs); cars are M2b.
fn draw_rail_editor(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (nn, ne) = state.home_structure.as_ref().map_or((0, 0), |h| (h.rail_nodes.len(), h.rail_edges.len()));
    egui::CollapsingHeader::new(RichText::new(format!("Rail ({nn} nodes, {ne} edges)")).strong().color(theme.text_primary()))
        .id_salt("hs_rail_sec")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Lay a rail line as a graph: drop stops, wire edges. Cars run it (M2b).")
                .size(theme.font_size_small).color(theme.text_muted()));
            if ui.button("+ Rail node").clicked() {
                if let Some(h) = state.home_structure.as_mut() {
                    let pos = (h.width * 0.5, h.depth * 0.5);
                    h.add_rail_node(pos);
                }
            }
            let mut rm_node: Option<u32> = None;
            for i in 0..nn {
                let id = state.home_structure.as_ref().unwrap().rail_nodes[i].id;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("N{id}")).size(theme.font_size_small).color(theme.text_secondary()));
                    if let Some(h) = state.home_structure.as_mut() {
                        let p = &mut h.rail_nodes[i].pos;
                        ui.add(egui::DragValue::new(&mut p.0).speed(0.25).prefix("x ").suffix(" m"));
                        ui.add(egui::DragValue::new(&mut p.1).speed(0.25).prefix("z ").suffix(" m"));
                    }
                    if ui.small_button("x").clicked() {
                        rm_node = Some(id);
                    }
                });
            }
            if let Some(id) = rm_node {
                if let Some(h) = state.home_structure.as_mut() {
                    h.remove_rail_node(id);
                }
            }
            if nn >= 2 {
                ui.separator();
                let ids: Vec<u32> = state.home_structure.as_ref().unwrap().rail_nodes.iter().map(|n| n.id).collect();
                if !ids.contains(&state.rail_edge_from) {
                    state.rail_edge_from = ids[0];
                }
                if !ids.contains(&state.rail_edge_to) {
                    state.rail_edge_to = ids[1];
                }
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("rail_from").width(54.0).selected_text(format!("N{}", state.rail_edge_from)).show_ui(ui, |ui| {
                        for id in &ids { ui.selectable_value(&mut state.rail_edge_from, *id, format!("N{id}")); }
                    });
                    ui.label("->");
                    egui::ComboBox::from_id_salt("rail_to").width(54.0).selected_text(format!("N{}", state.rail_edge_to)).show_ui(ui, |ui| {
                        for id in &ids { ui.selectable_value(&mut state.rail_edge_to, *id, format!("N{id}")); }
                    });
                    if ui.button("Add rail").clicked() {
                        if let Some(h) = state.home_structure.as_mut() {
                            h.add_rail_edge(state.rail_edge_from, state.rail_edge_to);
                        }
                    }
                });
            }
            let mut rm_edge: Option<usize> = None;
            let ne_now = state.home_structure.as_ref().map_or(0, |h| h.rail_edges.len());
            for i in 0..ne_now {
                let Some((f, t)) = state.home_structure.as_ref().and_then(|h| h.rail_edges.get(i)).map(|e| (e.from, e.to)) else {
                    break;
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("N{f} -> N{t}")).size(theme.font_size_small).color(theme.text_secondary()));
                    if ui.small_button("x").clicked() {
                        rm_edge = Some(i);
                    }
                });
            }
            if let Some(i) = rm_edge {
                if let Some(h) = state.home_structure.as_mut() {
                    h.remove_rail_edge(i);
                }
            }
        });
}

fn draw_roads_editor(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (nn, ne) = state.home_structure.as_ref().map_or((0, 0), |h| (h.road_nodes.len(), h.road_edges.len()));
    let mut changed = false;
    // Default the class picker to the first road type.
    if state.construction_road_class.is_empty() {
        if let Some(rt) = crate::ship::structure::road_types().first() {
            state.construction_road_class = rt.id.clone();
        }
    }
    egui::CollapsingHeader::new(RichText::new(format!("Roads ({nn} nodes, {ne} edges)")).strong().color(theme.text_primary()))
        .id_salt("hs_roads_sec")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Lay roads as a graph: drop nodes, then wire edges of a road class.")
                .size(theme.font_size_small).color(theme.text_muted()));
            // Add a node at the home centre (drag it from there, or set x/z below).
            if ui.button("+ Road node").clicked() {
                if let Some(h) = state.home_structure.as_mut() {
                    let id = h.unique_road_node_id();
                    let pos = (h.width * 0.5, h.depth * 0.5);
                    h.road_nodes.push(crate::ship::home_structure::RoadNode { id, pos });
                    changed = true;
                }
            }
            // Node list: id + x/z drag + remove.
            let mut rm_node: Option<u32> = None;
            for i in 0..nn {
                let id = state.home_structure.as_ref().unwrap().road_nodes[i].id;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("N{id}")).size(theme.font_size_small).color(theme.text_secondary()));
                    if let Some(h) = state.home_structure.as_mut() {
                        let p = &mut h.road_nodes[i].pos;
                        changed |= ui.add(egui::DragValue::new(&mut p.0).speed(0.25).prefix("x ").suffix(" m")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut p.1).speed(0.25).prefix("z ").suffix(" m")).changed();
                    }
                    if ui.small_button("x").clicked() {
                        rm_node = Some(id);
                    }
                });
            }
            if let Some(id) = rm_node {
                if let Some(h) = state.home_structure.as_mut() {
                    h.remove_road_node(id);
                }
                changed = true;
            }
            // Add-edge form: from/to node + class + width.
            if nn >= 2 {
                ui.separator();
                ui.horizontal(|ui| {
                    let ids: Vec<u32> = state.home_structure.as_ref().map(|h| h.road_nodes.iter().map(|n| n.id).collect()).unwrap_or_default();
                    egui::ComboBox::from_id_salt("road_from").selected_text(format!("from N{}", state.construction_road_from))
                        .show_ui(ui, |ui| { for id in &ids { ui.selectable_value(&mut state.construction_road_from, *id, format!("N{id}")); } });
                    egui::ComboBox::from_id_salt("road_to").selected_text(format!("to N{}", state.construction_road_to))
                        .show_ui(ui, |ui| { for id in &ids { ui.selectable_value(&mut state.construction_road_to, *id, format!("N{id}")); } });
                });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("road_class").selected_text(state.construction_road_class.clone())
                        .show_ui(ui, |ui| {
                            for rt in crate::ship::structure::road_types() {
                                ui.selectable_value(&mut state.construction_road_class, rt.id.clone(), rt.label.clone());
                            }
                        });
                    ui.add(egui::DragValue::new(&mut state.construction_road_width).speed(0.25).range(0.5..=30.0).prefix("w ").suffix(" m"));
                    if ui.button("Add edge").clicked() {
                        let (f, t, cls, w) = (state.construction_road_from, state.construction_road_to, state.construction_road_class.clone(), state.construction_road_width);
                        if f != t {
                            if let Some(h) = state.home_structure.as_mut() {
                                if h.road_node_pos(f).is_some() && h.road_node_pos(t).is_some() {
                                    h.road_edges.push(crate::ship::home_structure::RoadEdge { from: f, to: t, class: cls, width: w });
                                    changed = true;
                                }
                            }
                        }
                    }
                });
            }
            // Edge list + remove. Re-read the count: a node removal above may have PRUNED edges this
            // same frame, so the captured `ne` is stale -- indexing it would panic. (v0.586 fix)
            let mut rm_edge: Option<usize> = None;
            let ne_now = state.home_structure.as_ref().map_or(0, |h| h.road_edges.len());
            for i in 0..ne_now {
                let Some((f, t, cls)) = state
                    .home_structure
                    .as_ref()
                    .and_then(|h| h.road_edges.get(i))
                    .map(|e| (e.from, e.to, e.class.clone()))
                else {
                    break;
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("N{f}->N{t}  {cls}")).size(theme.font_size_small).color(theme.text_secondary()));
                    if ui.small_button("x").clicked() {
                        rm_edge = Some(i);
                    }
                });
            }
            if let Some(i) = rm_edge {
                if let Some(h) = state.home_structure.as_mut() {
                    if i < h.road_edges.len() {
                        h.road_edges.remove(i);
                    }
                }
                changed = true;
            }
        });
    if changed {
        state.construction_structure_dirty = true;
    }
}

/// Right-panel detail for a selected ROAD-graph node (v0.597): its id, editable x/z position, how
/// many road edges connect here, and Remove. Edits flag the home dirty so the mesh rebuilds live.
fn draw_road_node_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let id = match state.construction_road_node_selected {
        Some(i) => i,
        None => return,
    };
    let mut changed = false;
    let mut deselect = false;
    let edges = state.home_structure.as_ref().map_or(0, |h| h.road_edges.iter().filter(|e| e.from == id || e.to == id).count());
    if let Some(hs) = state.home_structure.as_mut() {
        if let Some(idx) = hs.road_nodes.iter().position(|n| n.id == id) {
            ui.label(RichText::new(format!("Road node N{id}")).strong().size(theme.font_size_body).color(theme.text_primary()));
            ui.label(RichText::new(format!("{edges} road segment(s) connect here")).size(theme.font_size_small).color(theme.text_muted()));
            ui.add_space(theme.spacing_xs);
            let p = &mut hs.road_nodes[idx].pos;
            ui.horizontal(|ui| {
                ui.label(RichText::new("pos").size(theme.font_size_small).color(theme.text_muted()));
                changed |= ui.add(egui::DragValue::new(&mut p.0).speed(0.25).prefix("x ").suffix(" m")).changed();
                changed |= ui.add(egui::DragValue::new(&mut p.1).speed(0.25).prefix("z ").suffix(" m")).changed();
            });
            ui.add_space(theme.spacing_sm);
            if ui.button(RichText::new("Remove").color(theme.danger())).clicked() {
                hs.remove_road_node(id);
                deselect = true;
                changed = true;
            }
        } else {
            deselect = true; // stale id -- the node was removed
        }
    }
    if deselect {
        state.construction_road_node_selected = None;
    }
    if changed {
        state.construction_structure_dirty = true;
    }
}

/// Right-panel detail for a ZONE selected by clicking its box in the 3D view (v0.634): type + purpose,
/// editable origin/size, Duplicate, Remove, Deselect. Zones render live, so edits show immediately.
fn draw_zone_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let id = match &state.construction_zone_selected {
        Some(i) => i.clone(),
        None => return,
    };
    let z = state.home_structure.as_ref().and_then(|hs| hs.zones.iter().find(|z| z.id == id).cloned());
    let z = match z {
        Some(z) => z,
        None => {
            state.construction_zone_selected = None; // the zone was removed out from under the selection
            return;
        }
    };
    let c = crate::ship::structure::zone_type(&z.type_id).map(|t| t.color).unwrap_or((0.6, 0.6, 0.6));
    let col = egui::Color32::from_rgb((c.0 * 255.0) as u8, (c.1 * 255.0) as u8, (c.2 * 255.0) as u8);
    ui.label(RichText::new(zone_label(&z.type_id)).strong().size(theme.font_size_body).color(col));
    ui.label(RichText::new(format!("zone {id}")).size(theme.font_size_small).color(theme.text_muted()));
    if let Some(zt) = crate::ship::structure::zone_type(&z.type_id) {
        ui.label(RichText::new(&zt.purpose).size(theme.font_size_small).color(theme.text_muted()));
    }
    let (mut o, mut s, mut ch) = (z.origin, z.size, false);
    ui.horizontal(|ui| {
        ui.label(RichText::new("at").size(theme.font_size_small).color(theme.text_muted()));
        ch |= ui.add(egui::DragValue::new(&mut o.0).speed(0.5).prefix("x ").suffix(" m")).changed();
        ch |= ui.add(egui::DragValue::new(&mut o.1).speed(0.5).prefix("y ").suffix(" m")).changed();
        ch |= ui.add(egui::DragValue::new(&mut o.2).speed(0.5).prefix("z ").suffix(" m")).changed();
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("size").size(theme.font_size_small).color(theme.text_muted()));
        ch |= ui.add(egui::DragValue::new(&mut s.0).speed(0.5).prefix("w ").range(1.0..=400.0)).changed();
        ch |= ui.add(egui::DragValue::new(&mut s.1).speed(0.5).prefix("h ").range(1.0..=100.0)).changed();
        ch |= ui.add(egui::DragValue::new(&mut s.2).speed(0.5).prefix("d ").range(1.0..=400.0)).changed();
    });
    ui.add_space(theme.spacing_xs);
    let (mut dup, mut rem, mut deselect) = (false, false, false);
    ui.horizontal(|ui| {
        if ui.small_button("Duplicate").clicked() {
            dup = true;
        }
        if ui.button(RichText::new("Remove").color(theme.danger())).clicked() {
            rem = true;
        }
        if ui.small_button("Deselect").clicked() {
            deselect = true;
        }
    });
    let mut new_sel: Option<Option<String>> = None;
    if let Some(hs) = state.home_structure.as_mut() {
        if ch {
            if let Some(zz) = hs.zones.iter_mut().find(|z| z.id == id) {
                zz.origin = o;
                zz.size = s;
            }
        }
        if dup {
            new_sel = Some(hs.duplicate_zone(&id));
        }
        if rem {
            hs.remove_zone(&id);
            new_sel = Some(None);
        }
    }
    if let Some(sel) = new_sel {
        state.construction_zone_selected = sel;
    }
    if deselect {
        state.construction_zone_selected = None;
    }
}

/// Right-panel detail for a selected CONDUIT-graph (pipe) node (v0.597): id, editable x/y/z, kind,
/// connected-edge count, Remove. Edits flag the machines dirty so the routing rebuilds.
fn draw_conduit_node_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    use crate::machines::ConduitEnd;
    let id = match &state.construction_conduit_node_selected {
        Some(i) => i.clone(),
        None => return,
    };
    let mut changed = false;
    let mut deselect = false;
    let touches = |e: &crate::machines::ConduitEdge| {
        matches!(&e.from, ConduitEnd::Node(n) if *n == id) || matches!(&e.to, ConduitEnd::Node(n) if *n == id)
    };
    let edges = state.home_machines.as_ref().map_or(0, |h| h.conduit_edges.iter().filter(|e| touches(e)).count());
    if let Some(h) = state.home_machines.as_mut() {
        if let Some(cn) = h.conduit_nodes.iter_mut().find(|n| n.id == id) {
            ui.label(RichText::new(format!("Pipe node {id}")).strong().size(theme.font_size_body).color(theme.text_primary()));
            ui.label(RichText::new(format!("kind: {}   {edges} pipe(s) connect here", if cn.kind.is_empty() { "any" } else { &cn.kind }))
                .size(theme.font_size_small).color(theme.text_muted()));
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                ui.label(RichText::new("pos").size(theme.font_size_small).color(theme.text_muted()));
                changed |= ui.add(egui::DragValue::new(&mut cn.pos.0).speed(0.2).prefix("x ").suffix(" m")).changed();
                changed |= ui.add(egui::DragValue::new(&mut cn.pos.1).speed(0.2).prefix("y ").suffix(" m")).changed();
                changed |= ui.add(egui::DragValue::new(&mut cn.pos.2).speed(0.2).prefix("z ").suffix(" m")).changed();
            });
            // TIER (v0.632): main / sub / subsub -- the trunk hierarchy. A main line is the spine a
            // residential block ties into; subs branch off it; subsubs reach fixtures.
            ui.horizontal(|ui| {
                ui.label(RichText::new("tier").size(theme.font_size_small).color(theme.text_muted()));
                for (t, lbl) in [(0u8, "Main"), (1, "Sub"), (2, "Subsub")] {
                    if ui.selectable_label(cn.tier == t, lbl).clicked() {
                        cn.tier = t;
                        changed = true;
                    }
                }
            });
            // SERVICE ENTRANCE / GRID TIE (v0.632): mark this node as the home's connection to the
            // external mothership/fleet grid (rendered distinctly; foundation for the grid hierarchy).
            let mut gt = cn.grid_tie;
            if ui.checkbox(&mut gt, "Grid tie (connects to the external grid)").changed() {
                cn.grid_tie = gt;
                changed = true;
            }
            ui.add_space(theme.spacing_sm);
            if ui.button(RichText::new("Remove").color(theme.danger())).clicked() {
                h.remove_conduit_node(&id);
                deselect = true;
                changed = true;
            }
        } else {
            deselect = true; // stale id
        }
    }
    if deselect {
        state.construction_conduit_node_selected = None;
    }
    if changed {
        state.construction_machines_dirty = true;
    }
}

/// Right-panel detail for a PIPE/WIRE selected by clicking it in the 3D view (v0.626). Shows the wire's
/// utility + endpoints and a Remove button, so a connection is a first-class editable object like a wall.
fn draw_connection_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some((from, to)) = state.construction_connection_selected.clone() else {
        return;
    };
    let kind = state
        .home_machines
        .as_ref()
        .and_then(|h| {
            h.connections
                .iter()
                .find(|c| (c.from == from && c.to == to) || (c.from == to && c.to == from))
                .map(|c| c.kind.clone())
        })
        .unwrap_or_default();
    if kind.is_empty() {
        // The connection vanished (e.g. an endpoint was deleted) -- drop the stale selection.
        state.construction_connection_selected = None;
        return;
    }
    let c = crate::machines::MachineHome::connection_color(&kind);
    let col = egui::Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);
    ui.label(RichText::new("Wire / pipe").strong().size(theme.font_size_body).color(theme.text_primary()));
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new(kind.to_uppercase()).strong().color(col));
    ui.label(RichText::new(format!("{from}  ->  {to}")).size(theme.font_size_small).color(theme.text_secondary()));
    ui.add_space(theme.spacing_sm);
    ui.horizontal(|ui| {
        if ui.button(RichText::new("Remove").color(theme.danger())).clicked() {
            if let Some(h) = state.home_machines.as_mut() {
                h.remove_connection_between(&from, &to);
            }
            state.construction_connection_selected = None;
            state.construction_machines_dirty = true;
        }
        if ui.button("Deselect").clicked() {
            state.construction_connection_selected = None;
        }
    });
}

/// Unified single-line OBJECT BROWSER (v0.596): every placed object -- walls, structures, machines,
/// lights -- as ONE consistent row "[type] name (x,z) [x]". Click selects it (its full detail shows
/// on the RIGHT panel, where the editing lives); double-click snaps the camera to it; [x] removes it.
/// Replaces the per-category list widgets so the left panel reads consistently (operator: "single line
/// per object, unify the styling, click pulls up the detail on the right"). Snapshot-then-apply so the
/// borrow on the home ends before any mutation; egui delivers one click per frame, so the single
/// pending action is index-safe even though removes shift indices.
fn draw_object_browser(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    #[derive(Clone)]
    enum Key {
        Wall(usize),
        Structure(usize),
        Light(usize),
        Machine(String),
        RoadNode(u32),
        ConduitNode(String),
    }
    struct Row {
        tag: &'static str,
        key: Key,
        name: String,
        pos: (f32, f32, f32),
        selected: bool,
        /// Whether the [x] remove affordance applies -- false for ARRAY-derived machines (not a direct
        /// instance; you edit the array, mirroring draw_machine_detail's is_direct gate). (v0.596)
        removable: bool,
        /// On/off state for a LIGHT row (v0.597): Some(on) shows a checkbox that toggles power; None
        /// for everything else. Clicking the NAME selects (the buggy "click name toggles" is gone now
        /// the light gets a dedicated checkbox, per the operator).
        on: Option<bool>,
    }
    let short = |s: &str| -> String {
        let p: Vec<&str> = s.split('_').collect();
        if p.len() >= 2 { format!("{}_{}", p[p.len() - 2], p[p.len() - 1]) } else { s.to_string() }
    };
    let mut rows: Vec<Row> = Vec::new();
    if let Some(hs) = state.home_structure.as_ref() {
        for (i, w) in hs.walls.iter().enumerate() {
            rows.push(Row {
                tag: "Wall",
                key: Key::Wall(i),
                name: format!("Wall {}", i + 1),
                pos: ((w.a.0 + w.b.0) * 0.5, hs.height * 0.5, (w.a.1 + w.b.1) * 0.5),
                selected: state.construction_wall_selected == Some(i),
                removable: true,
                on: None,
            });
        }
        for (i, ps) in hs.structures.iter().enumerate() {
            let name = crate::ship::structure::structure_type(&ps.type_id)
                .map(|t| t.label.clone())
                .unwrap_or_else(|| ps.type_id.clone());
            rows.push(Row { tag: "Struct", key: Key::Structure(i), name, pos: ps.pos, selected: state.construction_structure_selected == Some(i), removable: true, on: None });
        }
        for (i, l) in hs.lights.iter().enumerate() {
            let name = crate::renderer::light::light_type(&l.type_id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| l.type_id.clone());
            rows.push(Row { tag: "Light", key: Key::Light(i), name, pos: l.pos, selected: state.construction_light_selected == Some(i), removable: true, on: Some(l.on) });
        }
        for n in &hs.road_nodes {
            rows.push(Row { tag: "Road", key: Key::RoadNode(n.id), name: format!("Node N{}", n.id), pos: (n.pos.0, 0.0, n.pos.1), selected: state.construction_road_node_selected == Some(n.id), removable: true, on: None });
        }
    }
    if let Some(h) = state.home_machines.as_ref() {
        // DIRECT instances can be removed; ARRAY-derived ones (from all_instances() but not in
        // `instances`) are edited via their array, so they get no [x] (matches draw_machine_detail).
        let direct: std::collections::HashSet<String> = h.instances.iter().map(|m| m.id.clone()).collect();
        for inst in h.all_instances() {
            let selected = state.construction_machine_selected.as_deref() == Some(inst.id.as_str());
            let removable = direct.contains(&inst.id);
            rows.push(Row { tag: "Machine", key: Key::Machine(inst.id.clone()), name: short(&inst.machine), pos: inst.offset, selected, removable, on: None });
        }
        for cn in &h.conduit_nodes {
            let selected = state.construction_conduit_node_selected.as_deref() == Some(cn.id.as_str());
            rows.push(Row { tag: "Pipe", key: Key::ConduitNode(cn.id.clone()), name: format!("Node {}", cn.id), pos: cn.pos, selected, removable: true, on: None });
        }
    }
    let total = rows.len();
    // Stable "tag:id" key for the multi-select set (v0.612). Index-based for wall/struct/light (resolved
    // at delete time, descending, to dodge shift bugs); id-based for machines + nodes.
    fn key_str(k: &Key) -> String {
        match k {
            Key::Wall(i) => format!("Wall:{i}"),
            Key::Structure(i) => format!("Struct:{i}"),
            Key::Light(i) => format!("Light:{i}"),
            Key::Machine(id) => format!("Machine:{id}"),
            Key::RoadNode(id) => format!("Road:{id}"),
            Key::ConduitNode(id) => format!("Pipe:{id}"),
        }
    }
    // Snapshot the multi-set + locked/hidden-types so the row loop can read them without borrowing state.
    let multi = state.construction_multi.clone();
    let locked_types = state.construction_locked_types.clone();
    let hidden_types = state.construction_hidden_types.clone();
    enum Act {
        Select(Key),
        ToggleMulti(String),
        Focus((f32, f32, f32)),
        Remove(Key),
        ToggleLight(usize),
        DeleteMulti,
        NudgeMulti(f32, f32),
        ClearMulti,
        ToggleLock(String),
        UnlockAll,
        ToggleHidden(String),
    }
    let mut act: Option<Act> = None;
    egui::CollapsingHeader::new(RichText::new(format!("Objects ({total})")).strong().color(theme.text_primary()))
        .id_salt("hs_object_browser")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(RichText::new("Click a row to edit it; Ctrl+click to multi-select; double-click to fly there; Ctrl+D duplicates.")
                .size(theme.font_size_small).color(theme.text_muted()));
            // LOCKED-TYPES banner (v0.623): a locked type can't be picked in the viewport, which reads
            // as "I can't click my machines any more" if you forgot a lock was on. Make it loud + one
            // click to clear, so a stray lock is never an invisible footgun.
            if !locked_types.is_empty() {
                let mut names: Vec<&str> = locked_types.iter().map(|s| s.as_str()).collect();
                names.sort_unstable();
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("LOCKED (can't click in 3D): {}", names.join(", ")))
                        .size(theme.font_size_small).strong().color(theme.warning()));
                    if ui.small_button("Unlock all").clicked() {
                        act = Some(Act::UnlockAll);
                    }
                });
            }
            // Group-action bar (v0.612): appears whenever the multi-select set is non-empty.
            if !multi.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} selected", multi.len())).strong().color(theme.accent()));
                    if ui.small_button("Delete").clicked() {
                        act = Some(Act::DeleteMulti);
                    }
                    if ui.small_button("Clear").clicked() {
                        act = Some(Act::ClearMulti);
                    }
                });
                // Nudge the whole set on the floor by 0.5 m (keeps each object's height/Y).
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Nudge").size(theme.font_size_small).color(theme.text_muted()));
                    if ui.small_button("-X").clicked() { act = Some(Act::NudgeMulti(-0.5, 0.0)); }
                    if ui.small_button("+X").clicked() { act = Some(Act::NudgeMulti(0.5, 0.0)); }
                    if ui.small_button("-Z").clicked() { act = Some(Act::NudgeMulti(0.0, -0.5)); }
                    if ui.small_button("+Z").clicked() { act = Some(Act::NudgeMulti(0.0, 0.5)); }
                });
            }
            // Filter comes from the box at the TOP of the left panel (v0.603). NO inner scroll area --
            // the left panel's single scrollbar handles overflow (nested scroll is confusing, operator).
            let filter = state.construction_object_filter.to_lowercase();
            {
                // Group rows by type into collapsible sub-headers, in a stable order. (v0.598)
                for (tag, plural) in [("Wall", "Walls"), ("Struct", "Structures"), ("Machine", "Machines"), ("Light", "Lights"), ("Road", "Road nodes"), ("Pipe", "Pipe nodes")] {
                    let group: Vec<&Row> = rows.iter()
                        .filter(|r| r.tag == tag && (filter.is_empty() || r.name.to_lowercase().contains(filter.as_str())))
                        .collect();
                    if group.is_empty() {
                        continue;
                    }
                    // Auto-open while filtering, or when the group is small enough to not dominate.
                    let open = !filter.is_empty() || group.len() <= 12;
                    // FORCE the group open when it holds the current selection, so a selected machine's
                    // row is always reachable -- v0.623, the "I can't click objects in the list" report:
                    // a >12-row Machines group collapses by default and egui then REMEMBERS it collapsed
                    // (id_salt persists), so default_open alone can't reopen it. Some(true) overrides.
                    let force_open = if group.iter().any(|r| r.selected) { Some(true) } else { None };
                    let is_locked = locked_types.contains(tag);
                    let is_hidden = hidden_types.contains(tag);
                    let badge = match (is_locked, is_hidden) {
                        (true, true) => "  [locked, hidden]",
                        (true, false) => "  [locked]",
                        (false, true) => "  [hidden]",
                        (false, false) => "",
                    };
                    let title = format!("{plural} ({}){badge}", group.len());
                    egui::CollapsingHeader::new(RichText::new(title)
                            .color(if is_locked || is_hidden { theme.warning() } else { theme.text_secondary() }))
                        .id_salt(tag)
                        .open(force_open)
                        .default_open(open)
                        .show(ui, |ui| {
                            // Per-type LOCK + HIDE toggles (v0.614/v0.636): a locked type can't be picked
                            // in the viewport; a hidden type's meshes + gizmos are skipped (declutter).
                            ui.horizontal(|ui| {
                                if ui.small_button(if is_locked { "Unlock type" } else { "Lock type" }).clicked() {
                                    act = Some(Act::ToggleLock(tag.to_string()));
                                }
                                if ui.small_button(if is_hidden { "Show" } else { "Hide" }).clicked() {
                                    act = Some(Act::ToggleHidden(tag.to_string()));
                                }
                            });
                            for row in &group {
                                ui.horizontal(|ui| {
                                    // A LIGHT row gets a dedicated on/off checkbox; clicking it toggles
                                    // power and ONLY that (the name no longer toggles -- operator's fix).
                                    if let (Some(on), Key::Light(i)) = (row.on, &row.key) {
                                        let mut v = on;
                                        if ui.add(egui::Checkbox::without_text(&mut v)).clicked() {
                                            act = Some(Act::ToggleLight(*i));
                                        }
                                    }
                                    let in_multi = multi.contains(&key_str(&row.key));
                                    let txt = format!("{}{}  ({:.0},{:.0})", if in_multi { "* " } else { "" }, row.name, row.pos.0, row.pos.2);
                                    let resp = ui.selectable_label(row.selected || in_multi, RichText::new(txt).size(theme.font_size_small)
                                        .color(if row.selected || in_multi { theme.accent() } else { theme.text_secondary() }));
                                    if resp.clicked() {
                                        // Ctrl+click toggles multi-select membership; a plain click is the
                                        // usual single-select (which the handler also clears the set on).
                                        if ui.input(|i| i.modifiers.ctrl) {
                                            act = Some(Act::ToggleMulti(key_str(&row.key)));
                                        } else {
                                            act = Some(Act::Select(row.key.clone()));
                                        }
                                    }
                                    if resp.double_clicked() {
                                        act = Some(Act::Focus((row.pos.0, row.pos.1 + 0.5, row.pos.2)));
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if row.removable && !is_locked {
                                            if ui.small_button("x").clicked() {
                                                act = Some(Act::Remove(row.key.clone()));
                                            }
                                        }
                                    });
                                });
                            }
                        });
                }
            }
        });
    // Apply the single pending action after the borrow on `rows`/home ends.
    let clear_sel = |s: &mut GuiState| {
        s.construction_wall_selected = None;
        s.construction_structure_selected = None;
        s.construction_light_selected = None;
        s.construction_machine_selected = None;
        s.construction_road_node_selected = None;
        s.construction_conduit_node_selected = None;
        s.construction_connection_selected = None; // v0.626: a list select drops the pipe highlight
        s.construction_zone_selected = None; // v0.634: ... and the zone highlight
    };
    match act {
        Some(Act::Select(k)) => {
            clear_sel(state);
            state.construction_multi.clear(); // a plain single-select resets the multi-set
            match k {
                Key::Wall(i) => state.construction_wall_selected = Some(i),
                Key::Structure(i) => state.construction_structure_selected = Some(i),
                Key::Light(i) => state.construction_light_selected = Some(i),
                Key::Machine(id) => state.construction_machine_selected = Some(id),
                Key::RoadNode(id) => state.construction_road_node_selected = Some(id),
                Key::ConduitNode(id) => state.construction_conduit_node_selected = Some(id),
            }
        }
        Some(Act::ToggleMulti(key)) => {
            if !state.construction_multi.remove(&key) {
                state.construction_multi.insert(key);
            }
        }
        Some(Act::ClearMulti) => state.construction_multi.clear(),
        Some(Act::ToggleLock(tag)) => {
            if !state.construction_locked_types.remove(&tag) {
                state.construction_locked_types.insert(tag);
            }
        }
        Some(Act::ToggleHidden(tag)) => {
            if !state.construction_hidden_types.remove(&tag) {
                state.construction_hidden_types.insert(tag);
            }
        }
        Some(Act::UnlockAll) => state.construction_locked_types.clear(),
        Some(Act::DeleteMulti) => {
            group_delete(state);
            clear_sel(state);
            state.construction_multi.clear();
        }
        Some(Act::NudgeMulti(dx, dz)) => group_nudge(state, dx, dz),
        Some(Act::Focus(p)) => state.construction_focus_request = Some(p),
        Some(Act::ToggleLight(i)) => {
            if let Some(hs) = state.home_structure.as_mut() {
                if let Some(l) = hs.lights.get_mut(i) {
                    l.on = !l.on;
                }
            }
            state.construction_structure_dirty = true;
        }
        Some(Act::Remove(k)) => {
            match k {
                Key::Wall(i) => {
                    if let Some(hs) = state.home_structure.as_mut() {
                        if i < hs.walls.len() {
                            hs.walls.remove(i);
                        }
                    }
                    state.construction_wall_selected = None;
                    state.construction_structure_dirty = true;
                }
                Key::Structure(i) => {
                    if let Some(hs) = state.home_structure.as_mut() {
                        if i < hs.structures.len() {
                            hs.structures.remove(i);
                            for s in &mut hs.structures {
                                if let Some(p) = s.pair {
                                    if p == i { s.pair = None; } else if p > i { s.pair = Some(p - 1); }
                                }
                            }
                        }
                    }
                    state.construction_structure_selected = None;
                    state.construction_structure_dirty = true;
                }
                Key::Light(i) => {
                    if let Some(hs) = state.home_structure.as_mut() {
                        if i < hs.lights.len() {
                            hs.lights.remove(i);
                        }
                    }
                    state.construction_light_selected = None;
                    state.construction_structure_dirty = true;
                }
                Key::Machine(id) => {
                    if let Some(h) = state.home_machines.as_mut() {
                        h.remove_instance(&id); // prunes connections + conduit edges too (v0.596 fix)
                    }
                    state.construction_machine_selected = None;
                    state.construction_machines_dirty = true;
                }
                Key::RoadNode(id) => {
                    if let Some(hs) = state.home_structure.as_mut() {
                        hs.remove_road_node(id); // prunes edges touching it
                    }
                    state.construction_road_node_selected = None;
                    state.construction_structure_dirty = true;
                }
                Key::ConduitNode(id) => {
                    if let Some(h) = state.home_machines.as_mut() {
                        h.remove_conduit_node(&id); // prunes edges touching it
                    }
                    state.construction_conduit_node_selected = None;
                    state.construction_machines_dirty = true;
                }
            }
        }
        None => {}
    }
}

/// Delete every object in the multi-select set (v0.612). Index-keyed types (walls/structures/lights)
/// are removed in DESCENDING index order so earlier removals don't shift later indices; id-keyed types
/// (machines, road/pipe nodes) remove via the pruning helpers (so connections/edges stay consistent).
fn group_delete(state: &mut GuiState) {
    let keys: Vec<String> = state.construction_multi.iter().cloned().collect();
    let (mut wall_idx, mut struct_idx, mut light_idx) = (Vec::new(), Vec::new(), Vec::new());
    let (mut machine_ids, mut road_ids, mut pipe_ids) = (Vec::new(), Vec::new(), Vec::new());
    for k in &keys {
        let Some((tag, rest)) = k.split_once(':') else { continue };
        match tag {
            "Wall" => if let Ok(i) = rest.parse::<usize>() { wall_idx.push(i); },
            "Struct" => if let Ok(i) = rest.parse::<usize>() { struct_idx.push(i); },
            "Light" => if let Ok(i) = rest.parse::<usize>() { light_idx.push(i); },
            "Machine" => machine_ids.push(rest.to_string()),
            "Road" => if let Ok(i) = rest.parse::<u32>() { road_ids.push(i); },
            "Pipe" => pipe_ids.push(rest.to_string()),
            _ => {}
        }
    }
    let mut struct_dirty = false;
    let mut mach_dirty = false;
    if let Some(hs) = state.home_structure.as_mut() {
        struct_idx.sort_unstable();
        struct_idx.dedup();
        for &i in struct_idx.iter().rev() {
            if i < hs.structures.len() {
                hs.structures.remove(i);
                // Re-point teleporter/train pairs across the removed index (mirrors single-remove).
                for s in &mut hs.structures {
                    if let Some(p) = s.pair {
                        if p == i { s.pair = None; } else if p > i { s.pair = Some(p - 1); }
                    }
                }
            }
        }
        wall_idx.sort_unstable();
        wall_idx.dedup();
        for &i in wall_idx.iter().rev() {
            if i < hs.walls.len() { hs.walls.remove(i); }
        }
        light_idx.sort_unstable();
        light_idx.dedup();
        for &i in light_idx.iter().rev() {
            if i < hs.lights.len() { hs.lights.remove(i); }
        }
        for id in &road_ids { hs.remove_road_node(*id); }
        struct_dirty = !struct_idx.is_empty() || !wall_idx.is_empty() || !light_idx.is_empty() || !road_ids.is_empty();
    }
    if let Some(h) = state.home_machines.as_mut() {
        for id in &machine_ids { h.remove_instance(id); }
        for id in &pipe_ids { h.remove_conduit_node(id); }
        mach_dirty = !machine_ids.is_empty() || !pipe_ids.is_empty();
    }
    if struct_dirty { state.construction_structure_dirty = true; }
    if mach_dirty { state.construction_machines_dirty = true; }
}

/// Nudge every object in the multi-select set by (dx, dz) metres on the floor, keeping each object's
/// height (v0.612). Walls move both corners; array-derived machines are skipped (only direct instances
/// can move). Mirrors the per-object drag, just applied to the whole set at once.
fn group_nudge(state: &mut GuiState, dx: f32, dz: f32) {
    let keys: Vec<String> = state.construction_multi.iter().cloned().collect();
    let mut struct_dirty = false;
    let mut mach_dirty = false;
    if let Some(hs) = state.home_structure.as_mut() {
        for k in &keys {
            let Some((tag, rest)) = k.split_once(':') else { continue };
            match tag {
                "Wall" => if let Ok(i) = rest.parse::<usize>() {
                    if let Some(w) = hs.walls.get_mut(i) {
                        w.a.0 += dx; w.a.1 += dz; w.b.0 += dx; w.b.1 += dz;
                        struct_dirty = true;
                    }
                },
                "Struct" => if let Ok(i) = rest.parse::<usize>() {
                    if let Some(s) = hs.structures.get_mut(i) { s.pos.0 += dx; s.pos.2 += dz; struct_dirty = true; }
                },
                "Light" => if let Ok(i) = rest.parse::<usize>() {
                    if let Some(l) = hs.lights.get_mut(i) { l.pos.0 += dx; l.pos.2 += dz; struct_dirty = true; }
                },
                "Road" => if let Ok(id) = rest.parse::<u32>() {
                    if let Some(n) = hs.road_nodes.iter_mut().find(|n| n.id == id) { n.pos.0 += dx; n.pos.1 += dz; struct_dirty = true; }
                },
                _ => {}
            }
        }
    }
    if let Some(h) = state.home_machines.as_mut() {
        for k in &keys {
            let Some((tag, rest)) = k.split_once(':') else { continue };
            match tag {
                "Machine" => if let Some(m) = h.instances.iter_mut().find(|m| m.id == rest) {
                    m.offset.0 += dx; m.offset.2 += dz; mach_dirty = true;
                },
                "Pipe" => if let Some(n) = h.conduit_nodes.iter_mut().find(|n| n.id == rest) {
                    n.pos.0 += dx; n.pos.2 += dz; mach_dirty = true;
                },
                _ => {}
            }
        }
    }
    if struct_dirty { state.construction_structure_dirty = true; }
    if mach_dirty { state.construction_machines_dirty = true; }
}

fn draw_machines_and_connections(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    /// Friendly short label for a machine id: its last two underscore segments (e.g. "tower_2").
    fn label(s: &str) -> String {
        let p: Vec<&str> = s.split('_').collect();
        if p.len() >= 2 { format!("{}_{}", p[p.len() - 2], p[p.len() - 1]) } else { s.to_string() }
    }

    ui.add_space(theme.spacing_sm);

    // Snapshot the placed machines (id, type, room) for the list + the connection combos.
    let machines: Vec<(String, String, String)> = state
        .home_machines
        .as_ref()
        .map(|h| h.all_instances().into_iter().map(|i| (i.id, i.machine, i.room)).collect())
        .unwrap_or_default();

    // Machines list REMOVED (v0.597): machines are in the unified Objects browser above now (click
    // a row -> edit on the right). This function keeps only the utility-line connection editor below.
    let _ = &machines; // still used to build the connection-endpoint pickers

    // ── Utility lines / connections (collapsible, grouped by kind, v0.569). The operator wanted
    // "utility lines for gas, liquid, solids, electricity": each kind is its own sub-section.
    let conns: Vec<(String, String, String, Option<String>)> = state
        .home_machines
        .as_ref()
        .map(|h| h.connections.iter().map(|c| (c.from.clone(), c.to.clone(), c.kind.clone(), c.spec.clone())).collect())
        .unwrap_or_default();
    // The electrical cable choices for the per-power-connection picker (v0.615): "auto" + every
    // electrical conduit in the registry (copper AWG ... the room-temp superconductor).
    let cable_choices: Vec<(String, String)> = crate::utilities::conduit_types()
        .iter()
        .filter(|c| c.utility == crate::utilities::Utility::Electricity)
        .map(|c| (c.id.clone(), c.label.clone()))
        .collect();
    // The data-medium choices for the per-data-connection picker (v0.621): ethernet / fibre / WiFi.
    let data_choices: Vec<(String, String)> = crate::utilities::data_media()
        .map(|c| (c.id.clone(), c.label.clone()))
        .collect();
    egui::CollapsingHeader::new(RichText::new(format!("Utility lines ({})", conns.len())).strong().color(theme.text_primary()))
        .id_salt("hs_conns_sec")
        .default_open(true)
        .show(ui, |ui| {
            // The wire tool: from -> to + kind + Connect.
            if machines.len() >= 2 {
                if state.home_conn_from.is_empty() {
                    state.home_conn_from = machines[0].0.clone();
                }
                if state.home_conn_to.is_empty() {
                    state.home_conn_to = machines[1].0.clone();
                }
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("hs_conn_from").width(68.0).selected_text(label(&state.home_conn_from)).show_ui(ui, |ui| {
                        for (id, _, _) in &machines {
                            ui.selectable_value(&mut state.home_conn_from, id.clone(), label(id));
                        }
                    });
                    ui.label("->");
                    egui::ComboBox::from_id_salt("hs_conn_to").width(68.0).selected_text(label(&state.home_conn_to)).show_ui(ui, |ui| {
                        for (id, _, _) in &machines {
                            ui.selectable_value(&mut state.home_conn_to, id.clone(), label(id));
                        }
                    });
                });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("hs_conn_kind").width(82.0).selected_text(state.home_conn_kind.clone()).show_ui(ui, |ui| {
                        for k in ["water", "power", "data", "greywater", "gas"] {
                            ui.selectable_value(&mut state.home_conn_kind, k.to_string(), k);
                        }
                    });
                    if ui.button("Connect").clicked() {
                        let (from, to, kind) = (state.home_conn_from.clone(), state.home_conn_to.clone(), state.home_conn_kind.clone());
                        if from != to && !from.is_empty() && !to.is_empty() {
                            if let Some(h) = state.home_machines.as_mut() {
                                h.add_connection(&from, &to, &kind);
                            }
                            state.construction_machines_dirty = true;
                        }
                    }
                });
            } else {
                ui.label(RichText::new("Place at least two machines to wire them.").size(theme.font_size_small).color(theme.text_muted()));
            }

            // Superconductor upgrade (v0.616): the late-game payoff -- swap every power run to the
            // room-temperature superconductor in one click (near-zero loss, huge ampacity, so the
            // Conduits check goes all-green). "Reset to auto" reverts to cheapest-copper auto-sizing.
            // (A future quest gates earning it; the action itself is here now.)
            let power_runs = conns.iter().filter(|(_, _, k, _)| k == "power").count();
            let mut bulk_spec: Option<Option<String>> = None;
            if power_runs > 0 {
                ui.horizontal(|ui| {
                    if ui.button(format!("Upgrade {power_runs} power run(s) to superconductor")).clicked() {
                        bulk_spec = Some(Some("sc_room_temp".to_string()));
                    }
                    if ui.small_button("Reset to auto").clicked() {
                        bulk_spec = Some(None);
                    }
                });
                if let Some(sp) = bulk_spec {
                    if let Some(h) = state.home_machines.as_mut() {
                        for c in h.connections.iter_mut().filter(|c| c.kind == "power") {
                            c.spec = sp.clone();
                        }
                    }
                    state.construction_machines_dirty = true;
                }
            }

            // The existing lines, grouped under a sub-section per utility kind, each removable.
            if conns.is_empty() {
                ui.label(RichText::new("No lines yet.").size(theme.font_size_small).color(theme.text_muted()));
            } else {
                let mut kinds: Vec<String> = conns.iter().map(|(_, _, k, _)| k.clone()).collect();
                kinds.sort();
                kinds.dedup();
                let mut remove_conn: Option<usize> = None;
                // (connection index, new spec) when the operator picks a cable for a power run. (v0.615)
                let mut set_spec: Option<(usize, Option<String>)> = None;
                for k in &kinds {
                    let count = conns.iter().filter(|(_, _, ck, _)| ck == k).count();
                    egui::CollapsingHeader::new(RichText::new(format!("{k} ({count})")).color(theme.text_secondary()))
                        .id_salt(format!("hs_conn_kind_{k}"))
                        .default_open(true)
                        .show(ui, |ui| {
                            for (i, (from, to, kind, spec)) in conns.iter().enumerate() {
                                if kind != k {
                                    continue;
                                }
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{} -> {}", label(from), label(to)))
                                        .size(theme.font_size_small).color(theme.text_muted()));
                                    // Per-power-run CABLE picker (v0.615): "auto" (cheapest copper that
                                    // carries the load) or a pinned type. The Conduits buildability check
                                    // validates the choice (over-ampacity / voltage-drop -> warn/fail).
                                    if kind == "power" {
                                        let cur = spec
                                            .as_ref()
                                            .and_then(|id| cable_choices.iter().find(|(cid, _)| cid == id).map(|(_, l)| l.clone()))
                                            .unwrap_or_else(|| "auto".to_string());
                                        egui::ComboBox::from_id_salt(format!("cable_{i}"))
                                            .width(120.0)
                                            .selected_text(cur)
                                            .show_ui(ui, |ui| {
                                                if ui.selectable_label(spec.is_none(), "auto (cheapest copper)").clicked() {
                                                    set_spec = Some((i, None));
                                                }
                                                for (cid, clabel) in &cable_choices {
                                                    if ui.selectable_label(spec.as_deref() == Some(cid.as_str()), clabel).clicked() {
                                                        set_spec = Some((i, Some(cid.clone())));
                                                    }
                                                }
                                            });
                                    } else if kind == "data" {
                                        // Per-data-run MEDIUM picker (v0.621): ethernet / fibre / WiFi.
                                        // WiFi is wireless -- the Data-links buildability check warns it
                                        // emits RF (harms a nearby grow).
                                        let cur = spec
                                            .as_ref()
                                            .and_then(|id| data_choices.iter().find(|(cid, _)| cid == id).map(|(_, l)| l.clone()))
                                            .unwrap_or_else(|| "auto".to_string());
                                        egui::ComboBox::from_id_salt(format!("data_{i}"))
                                            .width(120.0)
                                            .selected_text(cur)
                                            .show_ui(ui, |ui| {
                                                if ui.selectable_label(spec.is_none(), "auto (cheapest medium)").clicked() {
                                                    set_spec = Some((i, None));
                                                }
                                                for (cid, clabel) in &data_choices {
                                                    if ui.selectable_label(spec.as_deref() == Some(cid.as_str()), clabel).clicked() {
                                                        set_spec = Some((i, Some(cid.clone())));
                                                    }
                                                }
                                            });
                                    }
                                    if ui.small_button("x").clicked() {
                                        remove_conn = Some(i);
                                    }
                                });
                            }
                        });
                }
                if let Some((i, sp)) = set_spec {
                    if let Some(h) = state.home_machines.as_mut() {
                        if let Some(c) = h.connections.get_mut(i) {
                            c.spec = sp;
                        }
                    }
                    state.construction_machines_dirty = true;
                }
                if let Some(i) = remove_conn {
                    if let Some(h) = state.home_machines.as_mut() {
                        h.remove_connection(i);
                    }
                    state.construction_machines_dirty = true;
                }
            }
        });
}

/// The placement palette (v0.527): a game-style footer bar. Category tabs across the top, then a
/// grid of placeable machine types in the selected category -- 10 wide, one row by default,
/// Expand for more. Clicking an item asks to place it (added to the selected room for now; viewport
/// click-to-place is the next step). Data-driven: categories + items come from the catalog.
fn draw_palette(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // STRUCTURE category first (leftmost -- the operator's "dedicated section to the left of
    // Defense"): walls + stairs/ladders/elevators/teleporters/trains/roads, from structure_types.ron.
    // Then the machine catalog's categories. One palette, two data sources. The Structure category is
    // gated to the new HomeStructure editor (placement needs a HomeStructure) so the legacy room-AABB
    // home never shows a placeable-looking ghost that can't drop. (v0.583)
    let mut categories: Vec<(String, Vec<(String, String)>)> = if state.home_structure.is_some() {
        crate::ship::structure::palette_categories()
    } else {
        Vec::new()
    };
    if let Some(h) = &state.home_machines {
        categories.extend(h.palette_categories());
    }
    if categories.is_empty() {
        return;
    }
    // Keep the selected category valid; default to the largest one for a full first view.
    if !categories.iter().any(|(c, _)| c == &state.construction_palette_category) {
        state.construction_palette_category = categories
            .iter()
            .max_by_key(|(_, items)| items.len())
            .map(|(c, _)| c.clone())
            .unwrap_or_default();
    }
    let expanded = state.construction_palette_expanded;
    let panel_h = if expanded { 210.0 } else { 96.0 };
    egui::TopBottomPanel::bottom("construction_palette")
        .exact_height(panel_h)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_xs);
            // Category tabs + the expand toggle (right-aligned).
            let is_structure = state.construction_palette_category == "Structure";
            ui.horizontal(|ui| {
                ui.label(RichText::new("Place").strong().color(theme.text_primary()));
                ui.separator();
                for (cat, items) in &categories {
                    let selected = cat == &state.construction_palette_category;
                    let txt = RichText::new(format!("{cat} ({})", items.len()))
                        .color(if selected { theme.accent() } else { theme.text_secondary() });
                    if ui.selectable_label(selected, txt).clicked() {
                        state.construction_palette_category = cat.clone();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(if expanded { "Collapse" } else { "Expand" }).clicked() {
                        state.construction_palette_expanded = !expanded;
                    }
                    if state.construction_selected_room.is_none() {
                        ui.label(RichText::new("select a room to place into")
                            .size(theme.font_size_small).color(theme.text_muted()));
                    }
                });
            });
            ui.separator();
            // The item grid for the selected category, 10 columns. Collapsed clips to ~1 row +
            // scrolls; expanded shows ~5 rows.
            let items: Vec<(String, String)> = categories
                .iter()
                .find(|(c, _)| c == &state.construction_palette_category)
                .map(|(_, its)| its.clone())
                .unwrap_or_default();
            egui::ScrollArea::vertical().max_height(panel_h - 48.0).show(ui, |ui| {
                egui::Grid::new("palette_grid")
                    .num_columns(10)
                    .spacing([theme.spacing_xs, theme.spacing_xs])
                    .show(ui, |ui| {
                        for (i, (id, label)) in items.iter().enumerate() {
                            // The HELD item (the one you are placing) is filled accent + outlined,
                            // so you can see what is attached to the cursor. (v0.529) For the
                            // Structure category, "held" tracks the structure type OR -- for the
                            // Wall tool -- the wall-DRAW mode (Wall is drawn, not placed). (v0.583)
                            let is_wall_tool = is_structure && id == "wall";
                            let held = if is_wall_tool {
                                state.construction_wall_mode
                            } else if is_structure {
                                state.construction_structure_type.as_deref() == Some(id.as_str())
                            } else {
                                state.construction_place_type.as_deref() == Some(id.as_str())
                            };
                            let mut btn = egui::Button::new(
                                RichText::new(label)
                                    .size(theme.font_size_small)
                                    .color(if held { theme.bg_primary() } else { theme.text_secondary() }),
                            );
                            if held {
                                btn = btn.fill(theme.accent()).stroke(egui::Stroke::new(2.0, theme.warning()));
                            }
                            if ui.add_sized([92.0, 30.0], btn).clicked() {
                                // Toggle: click the held item to cancel; click another to switch.
                                // Selecting any one tool clears the others (can't hold two at once).
                                if is_wall_tool {
                                    state.construction_wall_mode = !held;
                                    state.construction_wall_start = None;
                                    state.construction_place_type = None;
                                    state.construction_structure_type = None;
                                } else if is_structure {
                                    state.construction_structure_type = if held { None } else { Some(id.clone()) };
                                    state.construction_place_type = None;
                                    state.construction_wall_mode = false;
                                } else {
                                    state.construction_place_type = if held { None } else { Some(id.clone()) };
                                    state.construction_structure_type = None;
                                    state.construction_wall_mode = false;
                                }
                            }
                            if (i + 1) % 10 == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
        });
}

/// The whole-home buildability report (v0.524, home-design Stage 3): a power source exists for
/// the load, energy balances over a representative day with the battery carrying the solar-off
/// window, and the wiring is intact. Read-only; the same MachineHome::buildability_report an AI
/// can call before committing a design. 4.5 = the self-sufficiency model's representative sun-hours.
fn draw_buildability(ui: &mut egui::Ui, theme: &Theme, home: &crate::machines::MachineHome) {
    use crate::machines::CheckStatus;
    ui.add_space(theme.spacing_md);
    ui.separator();
    ui.label(RichText::new("Buildability").strong().color(theme.text_primary()));
    let report = home.buildability_report(4.5);
    if report.checks.is_empty() {
        ui.label(
            RichText::new("No systems to check yet -- place a panel, battery, and a load.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        return;
    }
    for c in &report.checks {
        // ✓ and ⚠ are confirmed-rendering glyphs; "!" (ASCII) for fail. Color carries the verdict.
        let (mark, color) = match c.status {
            CheckStatus::Pass => ("✓", theme.success()),
            CheckStatus::Warn => ("⚠", theme.warning()),
            CheckStatus::Fail => ("!", theme.danger()),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(mark).strong().color(color));
            ui.label(RichText::new(&c.name).size(theme.font_size_small).strong().color(theme.text_secondary()));
            ui.label(RichText::new(&c.detail).size(theme.font_size_small).color(theme.text_muted()));
        });
    }
    // Usage / self-sufficiency METERS (v0.630, grid S2): per-utility daily generation vs demand, framed
    // to TEACH (how much you make + use, how self-sufficient you are) -- never a penalty for consuming.
    let meters = home.utility_meters(4.5);
    if !meters.is_empty() {
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new("Usage + self-sufficiency").strong().color(theme.text_primary()));
        for m in &meters {
            let c = crate::machines::MachineHome::connection_color(&m.utility);
            let col = egui::Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{}:", m.utility)).size(theme.font_size_small).strong().color(col));
                ui.label(RichText::new(&m.summary).size(theme.font_size_small).color(theme.text_muted()));
            });
        }
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

#[cfg(test)]
mod multi_select_tests {
    use super::*;
    use crate::machines::{MachineConnection, MachineDef, MachineHome, MachineInstance};
    use std::collections::BTreeMap;

    fn box_def() -> MachineDef {
        MachineDef {
            shape: "box".into(),
            size: (1.0, 1.0, 1.0),
            color: (0.5, 0.5, 0.5),
            label: String::new(),
            category: "Machines".into(),
            stats: Vec::new(),
            power: None,
            ports: Vec::new(),
            storage: Vec::new(),
            rf_emission: 0.0,
        }
    }

    fn home_abc() -> MachineHome {
        let mut catalog = BTreeMap::new();
        catalog.insert("box".to_string(), box_def());
        let inst = |id: &str| MachineInstance { id: id.into(), machine: "box".into(), room: "g".into(), offset: (0.0, 0.0, 0.0), rotation: 0.0 };
        MachineHome {
            catalog,
            instances: vec![inst("a"), inst("b"), inst("c")],
            arrays: Vec::new(),
            connections: vec![MachineConnection { from: "a".into(), to: "b".into(), kind: "power".into(), spec: None }],
            loops: Vec::new(),
            conduit_nodes: Vec::new(),
            conduit_edges: Vec::new(),
        }
    }

    /// v0.612: group_delete removes every selected machine + prunes connections that touched them.
    #[test]
    fn group_delete_removes_selected_machines_and_prunes_connections() {
        let mut state = GuiState::default();
        state.home_machines = Some(home_abc());
        state.construction_multi.insert("Machine:a".into());
        state.construction_multi.insert("Machine:b".into());
        group_delete(&mut state);
        let h = state.home_machines.as_ref().unwrap();
        assert_eq!(h.instances.len(), 1, "a + b deleted, c remains");
        assert_eq!(h.instances[0].id, "c");
        assert!(h.connections.is_empty(), "the a-b connection was pruned");
        assert!(state.construction_machines_dirty, "machines marked dirty");
    }

    /// v0.612: group_nudge shifts only the selected machines' floor offsets, keeping the rest put.
    #[test]
    fn group_nudge_shifts_only_selected_machines() {
        let mut state = GuiState::default();
        state.home_machines = Some(home_abc());
        state.construction_multi.insert("Machine:a".into());
        group_nudge(&mut state, 0.5, -0.5);
        let h = state.home_machines.as_ref().unwrap();
        let a = h.instances.iter().find(|i| i.id == "a").unwrap();
        assert!((a.offset.0 - 0.5).abs() < 1e-6 && (a.offset.2 + 0.5).abs() < 1e-6, "selected 'a' nudged");
        let b = h.instances.iter().find(|i| i.id == "b").unwrap();
        assert!(b.offset.0.abs() < 1e-6 && b.offset.2.abs() < 1e-6, "unselected 'b' unchanged");
    }

    /// The exact descending-index removal pattern group_delete uses for index-keyed types
    /// (walls/lights/structures): removing sorted-desc indices never shifts a not-yet-removed one.
    #[test]
    fn descending_index_removal_keeps_remaining_valid() {
        let mut v = vec!['a', 'b', 'c', 'd', 'e'];
        let mut idx = vec![3usize, 1, 1];
        idx.sort_unstable();
        idx.dedup();
        for &i in idx.iter().rev() {
            if i < v.len() {
                v.remove(i);
            }
        }
        assert_eq!(v, vec!['a', 'c', 'e'], "removed b (idx 1) + d (idx 3), order preserved");
    }
}

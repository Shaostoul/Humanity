//! STOREYS in the B editor (ship homes increment 7, docs/design/ship-homes-increment-7-plan.md,
//! "Editor (C2)"): the storey selector in the left panel, the floor-slab tools beside it, and the
//! detail panel of a selected floor slab. A home's storeys are derived from its floor slabs
//! (`HomeStructure::storeys`), so making a storey means drawing a floor, and the selector lists
//! whatever the slabs make. The engine half (the floor pick on the chosen storey, placing on it,
//! the cut-away that hides the storeys above, drawing and dragging slabs in the view) is
//! engine/editor_storeys.rs. A child of the page in the 2018 directory form, like ship_tools.rs.

use super::*;
use crate::engine::editor::{
    choose_storey, left_standing_without, new_slab_top, remove_slab_in_editor, select_slab, set_slab_tool, slab_name,
    storey_name, EditStorey,
};
use crate::ship::home_structure::SlabMaterial;

/// What the storey tools remember between frames (`GuiState::construction_storeys`). None of it is
/// a setting: like the zone being edited, it is where the editor is looking, and it starts again
/// at the ground storey of the next editing session's home if that home has fewer storeys.
#[derive(Debug, Clone, Default)]
pub struct StoreyEditor {
    /// The storey the editor works on: an index into the edited zone's `storeys()`, bottom up (0
    /// is the ground). Kept in range when it is read (`EditStorey::of`), since adding, removing or
    /// moving a slab changes how many storeys there are.
    pub chosen: usize,
    /// The cut-away the drawn home meshes show: (zone, storey) when the storeys above `storey`
    /// are taken out of that zone (`editor::storey_cut_key`), None when the whole ship is drawn.
    /// `editor::follow_storey_view` redraws when this and what should be drawn differ.
    pub drawn: Option<(usize, usize)>,
    /// Index into the edited body's `floors` of the selected slab, whose detail fills the right
    /// panel. Every pick of something else lets go of it.
    pub slab_selected: Option<usize>,
    /// The floor-slab drawing tool, while it is on. It is a shape of the drawing tool: the wall
    /// tool's `construction_wall_mode` is on with it, so the view's clicks, its corner marker and
    /// its right-click-to-stop all work as they do for walls (engine/editor_storeys.rs).
    pub draw: Option<SlabDraw>,
    /// The first corner clicked while drawing a slab (body-local x, z), until the opposite one is.
    pub draw_start: Option<(f32, f32)>,
    /// The selected slab's corner being dragged in the view: (slab index, corner 0..4 round its
    /// footprint from its smallest x and z).
    pub corner_grab: Option<(usize, u8)>,
    /// Where the cursor was pressed on a slab corner (window pixels), until it moves far enough
    /// to be a drag rather than a tap (`editor::DRAG_THRESHOLD_PX`).
    pub press: Option<(f32, f32)>,
    /// A slab whose Remove was pressed once and waits for "Remove anyway", because walls or
    /// zones stand on the storey it is the last floor of.
    pub confirm_remove: Option<usize>,
    /// The last reason a floor could not be drawn ("the home is 3.0 m tall..."), shown under the
    /// tools until the next try. Empty when there is none.
    pub note: String,
}

/// Which floor the slab drawing tool draws (`StoreyEditor::draw`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlabDraw {
    /// The floor of the storey ABOVE the chosen one: at that storey's floor when there is one,
    /// else a new storey 3.0 m up. When the first slab is down the selector follows up onto it
    /// and the tool keeps drawing that floor (`Here`).
    Above,
    /// More of the chosen storey's own floor (a storey above the ground: the ground's floor is the
    /// home's own floor, everywhere).
    Here,
}

/// The STOREY SELECTOR and the floor-slab tools, in the left panel under the zone selector: which
/// storey of the zone the editor works on (the legacy room editor's Level stepper, its look), what
/// that storey is, and the two ways to draw a floor. The storeys come from the zone's slabs, so
/// a floor drawn above adds one, and the selector follows onto it (engine `try_place_slab_corner`).
pub(super) fn draw_storey_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // The slab tool is a shape of the drawing tool: when that stops (a right-click, the palette's
    // Wall, Close), so does it.
    if !state.construction_wall_mode && state.construction_storeys.draw.is_some() {
        state.construction_storeys.draw = None;
        state.construction_storeys.draw_start = None;
    }
    // So does the palette's Guard, the drawing tool drawing guards.
    if !state.construction_wall_mode {
        state.construction_wall_guard = false;
    }
    let Some(hs) = zone_body(&state.ship_structure, state.construction_zone) else {
        return;
    };
    let es = EditStorey::of(hs, state.construction_storeys.chosen);
    state.construction_storeys.chosen = es.index;
    let above = new_slab_top(es, SlabDraw::Above);
    // This storey's own floor, slab by slab (above the ground: the ground's floor is the home's).
    let floor_slabs: Vec<(usize, String)> = hs
        .floors
        .iter()
        .enumerate()
        .filter(|(_, s)| es.index > 0 && s.is_floor_of(&es.storey))
        .map(|(i, s)| (i, slab_name(i, s)))
        .collect();
    let draw = state.construction_storeys.draw;
    let mut go_to: Option<usize> = None;
    let mut tool: Option<Option<SlabDraw>> = None;
    let mut pick: Option<usize> = None;

    ui.add_space(theme.spacing_xs);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Storey").size(theme.font_size_small).color(theme.text_muted()));
        if ui.add_enabled(es.index > 0, egui::Button::new("-").small()).on_hover_text("The storey below").clicked() {
            go_to = Some(es.index - 1);
        }
        ui.label(RichText::new(storey_name(es.index)).strong().color(theme.accent()));
        if ui.add_enabled(es.has_above(), egui::Button::new("+").small()).on_hover_text("The storey above").clicked() {
            go_to = Some(es.index + 1);
        }
        ui.label(RichText::new(format!("of {}", es.count)).size(theme.font_size_small).color(theme.text_muted()));
    });
    let reach = if es.has_above() { "to the next floor" } else { "to the roof" };
    ui.label(
        RichText::new(format!("Its floor is {:.2} m up, {:.2} m {reach}.", es.storey.floor, es.storey.height()))
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
    if es.has_above() {
        ui.label(
            RichText::new("The storeys above are hidden while you work on this one. Walls, lights, machines and pieces you place go on it.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    }
    ui.horizontal_wrapped(|ui| {
        let on = draw == Some(SlabDraw::Above);
        let b = ui
            .add_enabled(above.is_ok(), egui::Button::new(RichText::new("Draw a floor above").size(theme.font_size_small)).selected(on))
            .on_hover_text("Click two opposite corners on this storey's plan: the slab becomes the floor of the storey above it.");
        if b.clicked() {
            tool = Some(if on { None } else { Some(SlabDraw::Above) });
        }
        if es.index > 0 {
            let on = draw == Some(SlabDraw::Here);
            let b = ui
                .add(egui::Button::new(RichText::new("Draw more of this floor").size(theme.font_size_small)).selected(on))
                .on_hover_text("Click two opposite corners: the slab adds to this storey's floor, at its height.");
            if b.clicked() {
                tool = Some(if on { None } else { Some(SlabDraw::Here) });
            }
        }
    });
    if let Err(why) = &above {
        ui.label(RichText::new(why).size(theme.font_size_small).color(theme.text_muted()));
    }
    if draw.is_some() {
        let hint = if state.construction_storeys.draw_start.is_some() {
            "Drawing a floor: click the opposite corner. Right-click to stop."
        } else {
            "Drawing a floor: click one corner on this storey's plan, then the opposite corner. Right-click to stop."
        };
        ui.label(RichText::new(hint).size(theme.font_size_small).color(theme.accent()));
    }
    if !state.construction_storeys.note.is_empty() {
        ui.label(RichText::new(&state.construction_storeys.note).size(theme.font_size_small).color(theme.warning()));
    }
    if !floor_slabs.is_empty() {
        ui.label(RichText::new(format!("This storey's floor: {} slab(s)", floor_slabs.len())).size(theme.font_size_small).color(theme.text_muted()));
        for (i, name) in &floor_slabs {
            let selected = state.construction_storeys.slab_selected == Some(*i);
            let label = RichText::new(format!("  {name}"))
                .size(theme.font_size_small)
                .color(if selected { theme.text_on_accent() } else { theme.text_secondary() });
            if ui.selectable_label(selected, label).clicked() {
                pick = Some(*i);
            }
        }
    }
    ui.add_space(theme.spacing_xs);

    if let Some(k) = go_to {
        choose_storey(state, k);
        state.construction_storeys.slab_selected = None;
    }
    if let Some(t) = tool {
        set_slab_tool(state, t);
    }
    if let Some(i) = pick {
        select_slab(state, i);
    }
}

/// The right panel for the SELECTED FLOOR SLAB: its name, which storey it is the floor of, its
/// label, corners (the footprint), height and thickness, solid or glass, and Remove. Every field
/// of `FloorSlab` is here, so nothing about a floor needs its file edited by hand.
pub(super) fn draw_slab_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(i) = state.construction_storeys.slab_selected else {
        return;
    };
    let Some(hs) = zone_body(&state.ship_structure, state.construction_zone) else {
        return;
    };
    let Some(s) = hs.floors.get(i) else {
        state.construction_storeys.slab_selected = None;
        return;
    };
    let (w, d, h) = (hs.width, hs.depth, hs.height);
    let name = slab_name(i, s);
    let storey = hs.storey_of_y(s.top);
    let starts = hs.storeys().iter().any(|st| s.is_floor_of(st));
    let standing = left_standing_without(hs, i);

    ui.label(RichText::new(name).strong().size(theme.font_size_body).color(theme.text_primary()));
    let place = if starts {
        format!("The floor of {}, {:.2} m up.", storey_name(storey).to_lowercase(), s.top)
    } else {
        format!("At {:.2} m it starts no storey: a floor has to be above the ground and under the roof.", s.top)
    };
    ui.label(RichText::new(place).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_xs);

    let mut changed = false;
    let mut moved_storey = false;
    if let Some(s) = zone_body_mut(&mut state.ship_structure, state.construction_zone).and_then(|hs| hs.floors.get_mut(i)) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Label").color(theme.text_muted()));
            let mut text = s.label.clone().unwrap_or_default();
            if ui.add(egui::TextEdit::singleline(&mut text).hint_text("upper floor, landing...").desired_width(140.0)).changed() {
                s.label = if text.trim().is_empty() { None } else { Some(text) };
                changed = true;
            }
        });
        // The footprint by its two corners (the view's corner handles drag the same fields).
        for (tag, which) in [("Corner A", 0usize), ("Corner B", 1)] {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tag).color(theme.text_muted()));
                let c = if which == 0 { &mut s.a } else { &mut s.b };
                ui.label("x");
                changed |= ui.add(egui::DragValue::new(&mut c.0).speed(0.1).range(0.0..=w).suffix(" m")).changed();
                ui.label("z");
                changed |= ui.add(egui::DragValue::new(&mut c.1).speed(0.1).range(0.0..=d).suffix(" m")).changed();
            });
        }
        let (x0, z0, x1, z1) = s.rect();
        ui.label(
            RichText::new(format!("{:.2} x {:.2} m, {:.1} m2", x1 - x0, z1 - z0, (x1 - x0) * (z1 - z0)))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Height").color(theme.text_muted()));
            let r = ui.add(egui::DragValue::new(&mut s.top).speed(0.05).range(0.5..=(h - 0.5).max(0.5)).suffix(" m"));
            moved_storey |= r.changed();
            changed |= r.changed();
            ui.label(RichText::new("Thickness").color(theme.text_muted()));
            changed |= ui.add(egui::DragValue::new(&mut s.thickness).speed(0.01).range(0.02..=1.0).suffix(" m")).changed();
        });
        ui.label(
            RichText::new(format!("Its top is the floor you stand on; the room under it has {:.2} m to its underside.", s.underside()))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Material").color(theme.text_muted()));
            changed |= ui.selectable_value(&mut s.material, SlabMaterial::Solid, "Solid").changed();
            changed |= ui.selectable_value(&mut s.material, SlabMaterial::Glass, "Glass").changed();
        });
        let what = match s.material {
            SlabMaterial::Solid => "Solid: blocks sunlight and lamp light, like any floor.",
            SlabMaterial::Glass => "Glass: lets sunlight and lamp light through, and is still a floor to stand on and a lid on the room under it.",
        };
        ui.label(RichText::new(what).size(theme.font_size_small).color(theme.text_muted()));
    }

    ui.add_space(theme.spacing_md);
    let (walls, zones, pieces) = standing;
    let armed = state.construction_storeys.confirm_remove == Some(i);
    let mut remove = false;
    if armed {
        ui.label(
            RichText::new(format!(
                "This is the last slab of {}'s floor: {walls} wall(s), {zones} zone(s) and {pieces} piece(s) stand on it, and stay where they are, in the air, until you move or remove them.",
                storey_name(storey).to_lowercase()
            ))
            .size(theme.font_size_small)
            .color(theme.warning()),
        );
        ui.horizontal(|ui| {
            remove = ui.button(RichText::new("Remove anyway").color(theme.danger())).clicked();
            if ui.button("Keep it").clicked() {
                state.construction_storeys.confirm_remove = None;
            }
        });
    } else if ui.button(RichText::new("Remove floor slab").color(theme.danger())).clicked() {
        if walls + zones + pieces > 0 {
            state.construction_storeys.confirm_remove = Some(i);
        } else {
            remove = true;
        }
    }
    if ui.small_button("Deselect").clicked() {
        state.construction_storeys.slab_selected = None;
    }
    if remove {
        remove_slab_in_editor(state, i);
        return;
    }
    if changed {
        state.construction_structure_dirty = true;
    }
    if moved_storey {
        // The slab's height moved it, maybe onto another storey: keep working on its storey, so it
        // stays in view.
        select_slab(state, i);
    }
}

/// The console's storey and floor-slab verbs (`exec_construction_command` hands them here):
/// `storey [n]`, `add_slab x0 z0 x1 z1 [above|here]`, `rm_slab <n>`, `slab_glass <n> on|off`. The
/// same edits the selector and the slab tools make, counted from 1 like every other console verb.
pub(super) fn exec_storey_command(state: &mut GuiState, parts: &[&str]) -> String {
    let f = |i: usize| -> Option<f32> { parts.get(i).and_then(|s| s.parse::<f32>().ok()) };
    let n = |i: usize| -> Option<usize> { parts.get(i).and_then(|s| s.parse::<usize>().ok()).filter(|n| *n >= 1) };
    let Some(hs) = zone_body(&state.ship_structure, state.construction_zone) else {
        return "No home loaded.".into();
    };
    let es = EditStorey::of(hs, state.construction_storeys.chosen);
    match parts.first().copied().unwrap_or_default() {
        "storey" => {
            if let Some(k) = n(1) {
                if k > es.count {
                    return format!("no storey {k} (have {})", es.count);
                }
                choose_storey(state, k - 1);
                state.construction_storeys.slab_selected = None;
                return format!("working on {}", storey_name(k - 1).to_lowercase());
            }
            let list: Vec<String> = hs
                .storeys()
                .iter()
                .enumerate()
                .map(|(k, s)| format!("{}{}: {:.2} to {:.2} m", if k == es.index { "* " } else { "" }, storey_name(k), s.floor, s.top))
                .collect();
            list.join("\n")
        }
        "add_slab" => {
            let (Some(x0), Some(z0), Some(x1), Some(z1)) = (f(1), f(2), f(3), f(4)) else {
                return "usage: add_slab x0 z0 x1 z1 [above|here]".into();
            };
            let draw = if parts.get(5) == Some(&"here") { SlabDraw::Here } else { SlabDraw::Above };
            let Some(hs) = zone_body_mut(&mut state.ship_structure, state.construction_zone) else {
                return "No home loaded.".into();
            };
            match crate::engine::editor::add_floor_slab(hs, es, draw, (x0, z0), (x1, z1)) {
                Ok(i) => {
                    let top = hs.floors[i].top;
                    state.construction_structure_dirty = true;
                    select_slab(state, i);
                    format!("added floor slab #{} at {top:.2} m; working on {}", i + 1, storey_name(state.construction_storeys.chosen).to_lowercase())
                }
                Err(why) => why,
            }
        }
        "rm_slab" => {
            let Some(i) = n(1) else { return "usage: rm_slab <n>".into() };
            if i > hs.floors.len() {
                return format!("no floor slab #{i} (have {})", hs.floors.len());
            }
            remove_slab_in_editor(state, i - 1);
            format!("removed floor slab #{i}")
        }
        "slab_glass" => {
            let (Some(i), Some(on)) = (n(1), parts.get(2)) else { return "usage: slab_glass <n> on|off".into() };
            let Some(s) = zone_body_mut(&mut state.ship_structure, state.construction_zone).and_then(|hs| hs.floors.get_mut(i - 1)) else {
                return format!("no floor slab #{i}");
            };
            s.material = if *on == "on" { SlabMaterial::Glass } else { SlabMaterial::Solid };
            state.construction_structure_dirty = true;
            format!("floor slab #{i} is {}", if *on == "on" { "glass" } else { "solid" })
        }
        other => format!("unknown storey verb '{other}'"),
    }
}

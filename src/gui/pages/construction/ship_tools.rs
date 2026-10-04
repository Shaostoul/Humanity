//! The SHIP-level tools at the top of the Construction page's left panel: the zone selector, the
//! corridors, and the plots and districts. They edit the ship as a whole rather than the zone being
//! edited, most of them only in the Dev mode. Moved out of construction.rs on 2026-10-03, when
//! increment 1a's Plots and Districts panel (docs/design/ship-homes-and-logistics.md) took the page
//! past its line budget (tests/file_size_ratchet.rs); the zone selector and the corridors moved
//! unchanged. A child of the page in the 2018 directory form, like pages/chat/.

use super::*;

/// SHIP ZONE selector (v0.754, ship-superstructure increment A), at the top of the Home structure
/// panel: a combo of the ship's zones (which one the editor is EDITING -- every tool operates on
/// it), an "Add zone" button (a modest default 10 x 10 x 3 m box placed clear of existing zones),
/// per-zone label / purpose / origin fields, and a two-click confirmed Delete for non-home zones.
/// Switching zones clears zone-scoped selections (their indices point into the new body) and moves
/// the build avatar to the new zone's spawn.
pub(super) fn draw_ship_zone_selector(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(n_zones) = state.ship_structure.as_ref().map(|s| s.zones.len()) else {
        return;
    };
    state.construction_zone = state.construction_zone.min(n_zones.saturating_sub(1));
    // PLAY-MODE SCOPE (task #50): only the Dev play mode may touch the ship's
    // superstructure. Outside Dev the editor is PINNED to the HOME zone -- no
    // zone dropdown, no add/delete, no label/purpose/origin edits -- so the
    // operator's multi-zone mothership is untouchable while every tool below
    // (walls, openings, lights, machines) still works on your own homestead.
    let ship_scope = state
        .settings
        .play_mode
        .allows(crate::config::Capability::ShipStructureEditing);
    // Owned display strings so the ship borrow ends before the mutations below.
    let zone_names: Vec<String> = state
        .ship_structure
        .as_ref()
        .map(|s| {
            s.zones
                .iter()
                .map(|z| if z.label.trim().is_empty() { z.id.clone() } else { format!("{} ({})", z.label, z.id) })
                .collect()
        })
        .unwrap_or_default();
    let home_idx = state.ship_structure.as_ref().map_or(0, |s| s.home_zone_index());
    let mut switch_to: Option<usize> = None;
    if !ship_scope {
        // Snap back to home if the mode changed mid-edit (or a stale selection
        // survived from an earlier Dev session). Routed through the shared
        // switch_to block below so zone-scoped selections are cleared the same
        // way a legitimate zone switch clears them.
        if state.construction_zone != home_idx {
            switch_to = Some(home_idx);
        }
        state.construction_zone_delete_arm = false; // no armed delete outside Dev
        ui.horizontal(|ui| {
            ui.label(RichText::new("Ship zone").size(theme.font_size_small).color(theme.text_muted()));
            ui.label(
                RichText::new(zone_names.get(home_idx).cloned().unwrap_or_default())
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        });
        ui.label(
            RichText::new(
                "Editing is scoped to your homestead. The Dev play mode \
                 (Settings > Gameplay) unlocks whole-ship editing.",
            )
            .size(theme.font_size_small)
            .color(theme.text_muted()),
        );
    } else {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Ship zone").size(theme.font_size_small).color(theme.text_muted()));
        egui::ComboBox::from_id_salt("ship_zone_selector")
            .selected_text(zone_names.get(state.construction_zone).cloned().unwrap_or_default())
            .show_ui(ui, |ui| {
                for (i, name) in zone_names.iter().enumerate() {
                    if ui.selectable_label(i == state.construction_zone, name).clicked() {
                        switch_to = Some(i);
                    }
                }
            });
        if ui.small_button("Add zone").clicked() {
            if let Some(ship) = state.ship_structure.as_mut() {
                let idx = ship.add_zone("New Zone", "commons");
                switch_to = Some(idx);
            }
            state.construction_structure_dirty = true; // render + collide the new box immediately
        }
    });
    // Per-zone metadata: label, purpose tag, world origin. Origin edits rebuild live (the whole
    // zone box moves). Deferred flags keep the ship borrow local to each block.
    let mut zone_edited = false;
    let home_plot_id: Option<String> = state
        .ship_structure
        .as_ref()
        .and_then(|s| s.home.as_ref())
        .map(|a| a.plot.clone());
    if let Some(z) = state
        .ship_structure
        .as_mut()
        .and_then(|s| s.zones.get_mut(state.construction_zone))
    {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Label").size(theme.font_size_small).color(theme.text_muted()));
            ui.add(egui::TextEdit::singleline(&mut z.label).desired_width(120.0));
            egui::ComboBox::from_id_salt("ship_zone_purpose")
                .selected_text(z.purpose.clone())
                .width(96.0)
                .show_ui(ui, |ui| {
                    for p in ["residence", "commons", "street", "bay", "agriculture", "corridor"] {
                        ui.selectable_value(&mut z.purpose, p.to_string(), p);
                    }
                });
        });
        // The home's origin is its PLOT's (increment 1a of
        // docs/design/ship-homes-and-logistics.md): it is never saved, so it is shown, not
        // edited. Move a home by moving its plot (Plots, below the corridors).
        let plot_of_home = if z.id == crate::ship::ship_structure::HOME_ZONE_ID { home_plot_id.clone() } else { None };
        if let Some(plot) = plot_of_home {
            ui.label(
                RichText::new(format!(
                    "Origin {:.1}, {:.1}, {:.1}: set by your plot {plot} (move the plot under Plots to move the home)",
                    z.origin.0, z.origin.1, z.origin.2
                ))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Origin").size(theme.font_size_small).color(theme.text_muted()));
                let mut o = z.origin;
                let rx = ui.add(egui::DragValue::new(&mut o.0).speed(0.5).suffix(" x"));
                let ry = ui.add(egui::DragValue::new(&mut o.1).speed(0.5).suffix(" y"));
                let rz = ui.add(egui::DragValue::new(&mut o.2).speed(0.5).suffix(" z"));
                if rx.changed() || ry.changed() || rz.changed() {
                    z.origin = o;
                    zone_edited = true;
                }
            });
        }
    }
    if zone_edited {
        state.construction_structure_dirty = true;
        // Machine offsets are zone-local since increment 1a, so the zone's machines move with
        // it: place them again from the new origin.
        state.construction_machines_dirty = true;
    }
    // Delete the EDITED zone (never the home zone, never the last zone) with a 2-click confirm.
    if state.construction_zone != home_idx && n_zones > 1 {
        ui.horizontal(|ui| {
            if state.construction_zone_delete_arm {
                ui.label(RichText::new("Delete this zone?").size(theme.font_size_small).color(theme.warning()));
                if ui.small_button(RichText::new("Confirm delete").color(theme.danger())).clicked() {
                    let removed = state
                        .ship_structure
                        .as_mut()
                        .map_or(false, |s| {
                            let idx = state.construction_zone;
                            s.remove_zone(idx)
                        });
                    state.construction_zone_delete_arm = false;
                    if removed {
                        switch_to = Some(home_idx.min(state.construction_zone));
                        state.construction_structure_dirty = true;
                    }
                }
                if ui.small_button("Cancel").clicked() {
                    state.construction_zone_delete_arm = false;
                }
            } else if ui.small_button(RichText::new("Delete zone").color(theme.danger())).clicked() {
                state.construction_zone_delete_arm = true;
            }
        });
    }
    } // end ship_scope (Dev-only zone tools)
    if let Some(mut i) = switch_to {
        // Runs unconditionally (even i == current): after a DELETE the indices shifted, so the
        // old selections must clear regardless of whether the index number happens to match.
        let count = state.ship_structure.as_ref().map_or(0, |s| s.zones.len());
        i = i.min(count.saturating_sub(1));
        state.construction_zone = i;
        state.construction_zone_delete_arm = false;
        // Zone-scoped selections index into the OLD body; clear them all.
        state.construction_wall_selected = None;
        state.construction_light_selected = None;
        state.construction_structure_selected = None;
        state.construction_road_node_selected = None;
        state.construction_zone_selected = None;
        state.construction_machine_selected = None;
        state.construction_wall_start = None;
        // Move the build avatar to the new zone's spawn (its box centre if none saved).
        let spawn = zone_body(&state.ship_structure, i)
            .map(|b| b.spawn.unwrap_or((b.width * 0.5, b.depth * 0.5)));
        state.build_char_pos = spawn;
        state.construction_structure_dirty = true; // refresh introspection + gizmo state
    }
    ui.add_space(theme.spacing_xs);
}

/// CORRIDORS section (ship-superstructure increment B), directly under the zone selector: lists the
/// ship's corridors (each with a delete X and, when broken, the honest reason it cannot generate)
/// and an "Add corridor" flow -- pick zone A and zone B, drag the world `lat` centreline (or hit
/// Center to snap it to the middle of the zones' shared span), size the door mouth, set the tube
/// width + glass top, Create. The corridor OWNS its door mouths since the rework (the old per-zone
/// door dropdowns indexed authored doors by ordinal, which desynced on every door edit). Creation
/// validates through `ShipStructure::corridor_geometry` (the same resolver mesh + collision use)
/// and shows the error verbatim on failure. Creating/deleting flags
/// `construction_structure_dirty`, the same live rebuild every zone edit takes.
pub(super) fn draw_corridor_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    use crate::ship::ship_structure::ShipCorridor;
    // PLAY-MODE SCOPE (task #50): corridors are whole-ship superstructure --
    // outside the Dev play mode they are read-only world geometry, not
    // editable rows. The section vanishes entirely (rather than disabling
    // piecemeal) so the homestead editor stays uncluttered for players; the
    // zone selector above already explains how to unlock whole-ship editing.
    if !state
        .settings
        .play_mode
        .allows(crate::config::Capability::ShipStructureEditing)
    {
        return;
    }
    let Some(ship) = state.ship_structure.as_ref() else {
        return;
    };
    if ship.zones.len() < 2 {
        return; // nothing to connect until a second zone exists
    }
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new("Corridors").size(theme.font_size_small).strong().color(theme.text_primary()));
    // Existing rows: owned display data first, so the ship borrow ends before any mutation below.
    // (label, why it cannot generate, is it the home's own corridor)
    let rows: Vec<(String, Option<String>, bool)> = ship
        .corridors
        .iter()
        .map(|c| {
            let label = format!(
                "{} -> {}  (lat {:.1} m, tube {:.1} m, door {:.1} x {:.1} m{})",
                c.from_zone,
                c.to_zone,
                c.lat,
                c.width,
                c.door_width,
                c.door_height,
                if c.glass_top { ", glass" } else { "" }
            );
            (label, ship.corridor_geometry(c).err(), ship.is_plot_door(c))
        })
        .collect();
    let zone_names: Vec<String> = ship.zones.iter().map(|z| z.id.clone()).collect();
    let n_zones = zone_names.len();
    state.construction_corridor_from_zone = state.construction_corridor_from_zone.min(n_zones - 1);
    state.construction_corridor_to_zone = state.construction_corridor_to_zone.min(n_zones - 1);
    let (from_id, to_id) = (
        zone_names[state.construction_corridor_from_zone].clone(),
        zone_names[state.construction_corridor_to_zone].clone(),
    );
    // The Center suggestion: midpoint of the two SELECTED zones' overlapping span on the axis
    // ACROSS the run. The run axis picks itself from the larger clear gap between the boxes,
    // mirroring `corridor_geometry` exactly so the button never disagrees with the validator.
    let center_lat = {
        let zf = &ship.zones[state.construction_corridor_from_zone];
        let zt = &ship.zones[state.construction_corridor_to_zone];
        let (fo, to) = (zf.origin_vec(), zt.origin_vec());
        let gap_x = (to.x - (fo.x + zf.body.width)).max(fo.x - (to.x + zt.body.width));
        let gap_z = (to.z - (fo.z + zf.body.depth)).max(fo.z - (to.z + zt.body.depth));
        if gap_x >= gap_z {
            // X run: centre the tube on the shared z span.
            (fo.z.max(to.z) + (fo.z + zf.body.depth).min(to.z + zt.body.depth)) * 0.5
        } else {
            // Z run: centre on the shared x span.
            (fo.x.max(to.x) + (fo.x + zf.body.width).min(to.x + zt.body.width)) * 0.5
        }
    };
    let mut delete: Option<usize> = None;
    for (i, (label, err, plot_door)) in rows.iter().enumerate() {
        ui.horizontal(|ui| {
            // The home's own corridor is its plot's door, rebuilt from the plot on every load
            // (increment 1a), so it is changed under Plots below, never deleted here.
            if !*plot_door && ui.small_button("X").on_hover_text("Delete this corridor").clicked() {
                delete = Some(i);
            }
            let color = if err.is_some() { theme.warning() } else { theme.text_primary() };
            ui.label(RichText::new(label).size(theme.font_size_small).color(color));
        });
        if *plot_door {
            ui.label(RichText::new("Your plot's door: move it under Plots.").size(theme.font_size_small).color(theme.text_muted()));
        }
        if let Some(e) = err {
            // The honest reason this row currently generates nothing (e.g. its door was deleted,
            // or a zone was dragged out of alignment). Saving drops broken rows.
            ui.label(RichText::new(e).size(theme.font_size_small).color(theme.warning()));
        }
    }
    if rows.is_empty() {
        ui.label(RichText::new("None yet. A corridor is a straight tube between two zones; it cuts its own door mouths.")
            .size(theme.font_size_small).color(theme.text_muted()));
    }
    // Add flow: from zone, to zone, lat centreline (+ Center helper), door mouth, width, glass
    // top, Create.
    let mut clear_error = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("From").size(theme.font_size_small).color(theme.text_muted()));
        egui::ComboBox::from_id_salt("corridor_from_zone")
            .selected_text(from_id.clone())
            .width(80.0)
            .show_ui(ui, |ui| {
                for (i, name) in zone_names.iter().enumerate() {
                    if ui.selectable_label(i == state.construction_corridor_from_zone, name).clicked() {
                        state.construction_corridor_from_zone = i;
                        clear_error = true;
                    }
                }
            });
        ui.label(RichText::new("To").size(theme.font_size_small).color(theme.text_muted()));
        egui::ComboBox::from_id_salt("corridor_to_zone")
            .selected_text(to_id.clone())
            .width(80.0)
            .show_ui(ui, |ui| {
                for (i, name) in zone_names.iter().enumerate() {
                    if ui.selectable_label(i == state.construction_corridor_to_zone, name).clicked() {
                        state.construction_corridor_to_zone = i;
                        clear_error = true;
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        // The centreline in WORLD metres (z for an X-run corridor, x for a Z-run) -- the corridor
        // owns this position; no authored door is consulted.
        ui.label(RichText::new("Lat").size(theme.font_size_small).color(theme.text_muted()));
        if ui
            .add(egui::DragValue::new(&mut state.construction_corridor_lat).speed(0.1).suffix(" m"))
            .changed()
        {
            clear_error = true;
        }
        if ui
            .small_button("Center")
            .on_hover_text("Snap the centreline to the middle of the two zones' shared span")
            .clicked()
        {
            state.construction_corridor_lat = center_lat;
            clear_error = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Door").size(theme.font_size_small).color(theme.text_muted()));
        ui.add(
            egui::DragValue::new(&mut state.construction_corridor_door_w)
                .speed(0.1)
                .range(0.8..=6.0)
                .suffix(" m"),
        );
        ui.label(RichText::new("x").size(theme.font_size_small).color(theme.text_muted()));
        ui.add(
            egui::DragValue::new(&mut state.construction_corridor_door_h)
                .speed(0.1)
                .range(1.8..=4.0)
                .suffix(" m"),
        );
    });
    let mut create = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Width").size(theme.font_size_small).color(theme.text_muted()));
        ui.add(
            egui::DragValue::new(&mut state.construction_corridor_width)
                .speed(0.1)
                .range(0.5..=20.0)
                .suffix(" m"),
        );
        ui.checkbox(&mut state.construction_corridor_glass, RichText::new("Glass top").size(theme.font_size_small));
        create = ui.small_button("Create").clicked();
    });
    if clear_error {
        state.construction_corridor_error.clear();
    }
    if create {
        let candidate = ShipCorridor {
            from_zone: from_id,
            to_zone: to_id,
            lat: state.construction_corridor_lat,
            width: state.construction_corridor_width,
            door_width: state.construction_corridor_door_w,
            door_height: state.construction_corridor_door_h,
            glass_top: state.construction_corridor_glass,
        };
        // Validate through the SAME resolver generation uses; show the failure verbatim. A row
        // to the home is refused first: the home's corridor comes from its plot's door
        // (increment 1a), so a second one would be left out of every save.
        let verdict = state
            .ship_structure
            .as_ref()
            .map(|s| {
                if s.is_plot_door(&candidate) {
                    Err("The home's corridor is its plot's door: change it under Plots.".to_string())
                } else {
                    s.corridor_geometry(&candidate).map(|_| ())
                }
            })
            .unwrap_or(Err("no ship loaded".to_string()));
        match verdict {
            Ok(()) => {
                if let Some(s) = state.ship_structure.as_mut() {
                    s.corridors.push(candidate);
                }
                state.construction_corridor_error.clear();
                state.construction_structure_dirty = true; // render + collide the new tube now
            }
            Err(e) => state.construction_corridor_error = e,
        }
    }
    if !state.construction_corridor_error.is_empty() {
        ui.label(
            RichText::new(&state.construction_corridor_error)
                .size(theme.font_size_small)
                .color(theme.warning()),
        );
    }
    if let Some(i) = delete {
        if let Some(s) = state.ship_structure.as_mut() {
            if i < s.corridors.len() {
                s.corridors.remove(i);
                state.construction_structure_dirty = true;
            }
        }
    }
    ui.add_space(theme.spacing_xs);
}

/// PLOTS and DISTRICTS (increment 1a of docs/design/ship-homes-and-logistics.md, Dev mode
/// only): the ship-file records that have no zone of their own, so the zone tools cannot reach
/// them. Plots: move one (its door and, for your own plot, your home with its corridor,
/// machines and spawn move with it: `ShipStructure::move_plot`), resize it, point its door at
/// another shared zone and size its corridor, pick the plot offline play uses, add one beyond another, remove one
/// (never your own). Each plot shows whether it would still take the home design on the next
/// load, and the save refuses a ship where one would not (`save_assembled`). Districts: add by
/// type, move, resize, remove. Everything here is written by the Dev save to
/// data/blueprints/ship_structure.ron. Deferred actions, so no ship borrow is held across the
/// egui closures.
pub(super) fn draw_plots_and_districts(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !state.settings.play_mode.allows(crate::config::Capability::ShipStructureEditing) {
        return;
    }
    let Some(ship) = state.ship_structure.as_ref() else { return };
    if ship.home.is_none() {
        return; // a ship that was not assembled from a plot has no plots to edit
    }
    enum Act {
        Move(String, (f32, f32, f32)),
        Resize(String, (f32, f32, f32)),
        DoorZone(String, String),
        /// (plot, tube width, door width, door height, glass top)
        DoorShape(String, f32, f32, f32, bool),
        Default(String),
        AddLike(String),
        Remove(String),
        AddDistrict(String),
        RemoveDistrict(String),
        SetDistrict(String, (f32, f32, f32), (f32, f32, f32)),
    }
    let mut act: Option<Act> = None;
    let own = ship.home.as_ref().map(|a| a.plot.clone()).unwrap_or_default();
    let default_plot = ship.default_plot_id().unwrap_or_default();
    let door_zones: Vec<String> = ship
        .zones
        .iter()
        .filter(|z| z.id != crate::ship::ship_structure::HOME_ZONE_ID)
        .map(|z| z.id.clone())
        .collect();
    let plots: Vec<crate::ship::ship_structure::Plot> = ship.plots.clone();
    let plot_ids: Vec<String> = plots.iter().map(|p| p.id.clone()).collect();
    egui::CollapsingHeader::new(RichText::new(format!("Plots ({})", plots.len())).strong().color(theme.text_primary()))
        .id_salt("ship_plots")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "Where homes go. Your home stands on {own}. A plot's door moves with it; moving yours moves your home."
                ))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            for p in &plots {
                ui.add_space(theme.spacing_xs);
                ui.horizontal(|ui| {
                    let mut title = format!("{}  {}", p.id, p.kind);
                    if p.id == own {
                        title.push_str("  (yours)");
                    }
                    if p.id == default_plot {
                        title.push_str("  (offline)");
                    }
                    ui.label(RichText::new(title).size(theme.font_size_small).strong().color(theme.text_primary()));
                    if p.id != own && ui.small_button("x").on_hover_text("Remove this plot").clicked() {
                        act = Some(Act::Remove(p.id.clone()));
                    }
                });
                let (mut o, mut s) = (p.origin, p.size);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("at").size(theme.font_size_small).color(theme.text_muted()));
                    let ch = ui.add(egui::DragValue::new(&mut o.0).speed(0.5).prefix("x ").suffix(" m")).changed()
                        | ui.add(egui::DragValue::new(&mut o.1).speed(0.5).prefix("y ").suffix(" m")).changed()
                        | ui.add(egui::DragValue::new(&mut o.2).speed(0.5).prefix("z ").suffix(" m")).changed();
                    if ch {
                        act = Some(Act::Move(p.id.clone(), o));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("size").size(theme.font_size_small).color(theme.text_muted()));
                    let ch = ui.add(egui::DragValue::new(&mut s.0).speed(0.5).prefix("w ").range(1.0..=2000.0)).changed()
                        | ui.add(egui::DragValue::new(&mut s.1).speed(0.5).prefix("h ").range(1.0..=200.0)).changed()
                        | ui.add(egui::DragValue::new(&mut s.2).speed(0.5).prefix("d ").range(1.0..=2000.0)).changed();
                    if ch {
                        act = Some(Act::Resize(p.id.clone(), s));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("door to").size(theme.font_size_small).color(theme.text_muted()));
                    egui::ComboBox::from_id_salt(("plot_door_zone", p.id.as_str()))
                        .selected_text(p.door.zone.clone())
                        .width(96.0)
                        .show_ui(ui, |ui| {
                            for z in &door_zones {
                                if ui.selectable_label(&p.door.zone == z, z).clicked() && &p.door.zone != z {
                                    act = Some(Act::DoorZone(p.id.clone(), z.clone()));
                                }
                            }
                        });
                    // The lat follows the home design's door point, so it is shown, not edited:
                    // a hand-set lat that missed the door would stop the plot from loading.
                    ui.label(RichText::new(format!("lat {:.1} m", p.door.lat)).size(theme.font_size_small).color(theme.text_muted()));
                });
                let (mut tw, mut dw, mut dh, mut glass) = (p.door.width, p.door.door_width, p.door.door_height, p.door.glass_top);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("corridor").size(theme.font_size_small).color(theme.text_muted()));
                    let ch = ui.add(egui::DragValue::new(&mut tw).speed(0.1).range(0.5..=20.0).prefix("w ").suffix(" m")).changed()
                        | ui.add(egui::DragValue::new(&mut dw).speed(0.1).range(0.8..=6.0).prefix("door ").suffix(" m")).changed()
                        | ui.add(egui::DragValue::new(&mut dh).speed(0.1).range(1.8..=4.0).prefix("x ").suffix(" m")).changed()
                        | ui.checkbox(&mut glass, RichText::new("glass").size(theme.font_size_small)).changed();
                    if ch {
                        act = Some(Act::DoorShape(p.id.clone(), tw, dw, dh, glass));
                    }
                });
                // Would this plot still take the home design on the next load? The same refusal
                // the load would give, so a problem shows here before the save refuses it.
                if let Some(Err(e)) = state.ship_structure.as_ref().and_then(|s| s.plot_check(&p.id)) {
                    ui.label(RichText::new(e).size(theme.font_size_small).color(theme.warning()));
                }
            }
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Offline play uses").size(theme.font_size_small).color(theme.text_muted()));
                egui::ComboBox::from_id_salt("ship_default_plot")
                    .selected_text(default_plot.clone())
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for id in &plot_ids {
                            if ui.selectable_label(id == &default_plot, id).clicked() && id != &default_plot {
                                act = Some(Act::Default(id.clone()));
                            }
                        }
                    });
                if let Some(last) = plot_ids.last() {
                    if ui.small_button("Add plot").on_hover_text(format!("A copy of {last}, past it")).clicked() {
                        act = Some(Act::AddLike(last.clone()));
                    }
                }
            });
            if let Some(Err(e)) = state.ship_structure.as_ref().map(|s| s.validate()) {
                ui.label(RichText::new(format!("The ship will not save as it is: {e}")).size(theme.font_size_small).color(theme.warning()));
            }
        });

    // DISTRICTS: the mothership's labelled volumes, in ship metres (they were sub-zones of the
    // home body until increment 1a).
    let districts: Vec<(String, String, (f32, f32, f32), (f32, f32, f32))> = state
        .ship_structure
        .as_ref()
        .map(|s| s.districts.iter().map(|d| (d.id.clone(), d.type_id.clone(), d.origin, d.size)).collect())
        .unwrap_or_default();
    let types = crate::ship::structure::zone_types();
    egui::CollapsingHeader::new(RichText::new(format!("Districts ({})", districts.len())).strong().color(theme.text_primary()))
        .id_salt("ship_districts")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                RichText::new("The mothership's districts: hangar, the Concourse, transit... Residential ones draw nothing until each plot draws its own home.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            if !types.is_empty() {
                // The type to add, kept in egui's memory (it is only this combo's state).
                let id = egui::Id::new("ship_district_add_type");
                let mut pick: String = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| types[0].id.clone());
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("ship_district_add")
                        .width(160.0)
                        .selected_text(zone_label(&pick))
                        .show_ui(ui, |ui| {
                            for t in types {
                                ui.selectable_value(&mut pick, t.id.clone(), &t.label);
                            }
                        });
                    if ui.button("Add district").clicked() {
                        act = Some(Act::AddDistrict(pick.clone()));
                    }
                });
                ui.data_mut(|d| d.insert_temp(id, pick));
            }
            for (id, type_id, origin, size) in &districts {
                ui.add_space(theme.spacing_xs);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(zone_label(type_id)).size(theme.font_size_small).strong().color(theme.text_primary()));
                    ui.label(RichText::new(id).size(theme.font_size_small).color(theme.text_muted()));
                    if ui.small_button("x").on_hover_text("Remove this district").clicked() {
                        act = Some(Act::RemoveDistrict(id.clone()));
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
                    ch |= ui.add(egui::DragValue::new(&mut s.0).speed(0.5).prefix("w ").range(1.0..=2000.0)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut s.1).speed(0.5).prefix("h ").range(1.0..=200.0)).changed();
                    ch |= ui.add(egui::DragValue::new(&mut s.2).speed(0.5).prefix("d ").range(1.0..=2000.0)).changed();
                });
                if ch {
                    act = Some(Act::SetDistrict(id.clone(), o, s));
                }
            }
        });

    let Some(ship) = state.ship_structure.as_mut() else { return };
    match act {
        None => return,
        Some(Act::Move(id, o)) => {
            ship.move_plot(&id, o);
        }
        Some(Act::Resize(id, s)) => {
            if let Some(p) = ship.plots.iter_mut().find(|p| p.id == id) {
                p.size = s;
            }
        }
        Some(Act::DoorZone(id, z)) => {
            ship.set_plot_door_zone(&id, &z);
        }
        Some(Act::DoorShape(id, tw, dw, dh, glass)) => {
            if let Some(p) = ship.plots.iter_mut().find(|p| p.id == id) {
                p.door.width = tw;
                p.door.door_width = dw;
                p.door.door_height = dh;
                p.door.glass_top = glass;
            }
            ship.sync_home_to_plot(); // the home's corridor is rebuilt from its plot's door
        }
        Some(Act::Default(id)) => ship.default_plot = Some(id),
        Some(Act::AddLike(like)) => {
            ship.add_plot_like(&like, 10.0);
        }
        Some(Act::Remove(id)) => {
            ship.remove_plot(&id);
        }
        Some(Act::AddDistrict(t)) => {
            // A new district starts past everything else on the ship, at the type's size.
            let size = crate::ship::structure::zone_type(&t).map(|zt| zt.default_size).unwrap_or((20.0, 4.0, 20.0));
            let origin = ship.next_free_origin(10.0);
            ship.add_district(&t, origin, size);
        }
        Some(Act::RemoveDistrict(id)) => {
            ship.remove_district(&id);
        }
        Some(Act::SetDistrict(id, o, s)) => {
            if let Some(d) = ship.districts.iter_mut().find(|d| d.id == id) {
                d.origin = o;
                d.size = s;
            }
        }
    }
    // A plot move can move the home (its machines are zone-local) and every edit here changes
    // what is drawn: rebuild both.
    state.construction_structure_dirty = true;
    state.construction_machines_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;

    /// One headless frame of the Plots and Districts panel in a plain panel, with `events`.
    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 1400.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_plots_and_districts(ui, theme, state));
        })
    }

    /// Click the drawn text `label` (move, press, release: egui's three frames) and settle.
    fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, label: &str) {
        let out = frame(ctx, theme, state, Vec::new());
        let pos = find_text_in_shapes(&out.shapes, label).unwrap_or_else(|| panic!("{label:?} was not drawn")).rect.center();
        let m = egui::Modifiers::default();
        frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }

    /// The Plots and Districts panel is the in-app control the critic found missing in 1a, so it
    /// is proven the way a player meets it, drawn and clicked, not just built: outside the Dev
    /// mode it is not there at all; in the Dev mode "Plots (2)" opens to the shipped plots, yours
    /// marked, and "Add plot" really adds a plot and asks for a rebuild; "Districts (13)" opens
    /// to the districts. Red check, run: the panel with its early `return` for the Dev mode
    /// removed draws "Plots (2)" in Normal mode and fails the first assertion.
    #[test]
    fn the_plots_panel_is_dev_only_and_its_buttons_edit_the_ship() {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        // No opening animation: headless frames carry no time, so a header would stay half open.
        ctx.style_mut(|s| s.animation_time = 0.0);
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let mut state = GuiState::default();
        state.ship_structure = Some(crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, None).expect("assembles"));

        state.settings.play_mode = crate::config::PlayMode::Normal;
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        assert!(find_text_in_shapes(&out.shapes, "Plots (2)").is_none(), "no plot editing outside the Dev mode");

        state.settings.play_mode = crate::config::PlayMode::Dev;
        click(&ctx, &theme, &mut state, "Plots (2)");
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        assert!(find_text_in_shapes(&out.shapes, "p1  homestead  (yours)  (offline)").is_some(), "your plot, marked");
        assert!(find_text_in_shapes(&out.shapes, "lat 40.0 m").is_some(), "p1's door lat, shown");
        state.construction_structure_dirty = false;
        click(&ctx, &theme, &mut state, "Add plot");
        let ship = state.ship_structure.as_ref().unwrap();
        assert_eq!(ship.plots.len(), 3, "Add plot added a plot");
        assert_eq!(ship.plots[2].origin, (0.0, 0.0, 198.0), "past p2");
        assert!(state.construction_structure_dirty && state.construction_machines_dirty, "and asked for a rebuild");

        click(&ctx, &theme, &mut state, "Districts (13)");
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        assert!(find_text_in_shapes(&out.shapes, "res-1").is_some(), "the districts are listed");
    }
}

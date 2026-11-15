//! ROOM FINISHES in the build editor (room finishes rung 1, docs/design/room-finishes.md): the
//! selected room's (zone's) skirting, crown and trim material, and the selected wall's own choice
//! per side, each picked from the catalog (data/blueprints/room_finishes.ron) with a drawing of the
//! cross-sections it resolves to. Every choice starts open, taking the default for what the room
//! is for, and the home rebuilds at once on a change, so the 3D view is the full-size preview. A
//! child of the page in the 2018 directory form, like storeys.rs.

use super::*;
use crate::gui::widgets::Hint;
use humanity_core::i18n::{t, t_dyn, tf};
use humanity_ship::home_structure::{storey_index, HomeStructure};
use humanity_ship::room_finishes::{catalog, Finish, ProfileKind, TrimProfile, NONE_ID};

/// A profile's or material's name (finishes.profile.<id>, finishes.material.<id>).
fn profile_name(id: &str) -> String {
    if id == NONE_ID {
        return t("construction.finish_none").to_string();
    }
    t_dyn(&format!("finishes.profile.{id}")).to_string()
}

fn material_name(id: &str) -> String {
    t_dyn(&format!("finishes.material.{id}")).to_string()
}

/// The pickers for one choice: skirting, crown, material, each "as inherited" (None) or a pick.
/// `inherited` is what each open field resolves to, named in its first entry under `inherit_key`
/// ("Default: X" for a room, "As the room: X" for a wall side). True when the choice changed.
fn finish_pickers(ui: &mut egui::Ui, theme: &Theme, f: &mut Finish, inherited: &Finish, salt: &str, inherit_key: &'static str) -> bool {
    let cat = catalog();
    let mut changed = false;
    let rows: [(&'static str, &'static str, Option<ProfileKind>); 3] = [
        ("construction.finish_skirting", "construction.finish_skirting_hint", Some(ProfileKind::Skirting)),
        ("construction.finish_crown", "construction.finish_crown_hint", Some(ProfileKind::Crown)),
        ("construction.finish_material", "construction.finish_material_hint", None),
    ];
    for (label_key, hint_key, kind) in rows {
        let (field, inherit_id) = match kind {
            Some(ProfileKind::Skirting) => (&mut f.skirting, inherited.skirting.clone()),
            Some(_) => (&mut f.crown, inherited.crown.clone()),
            None => (&mut f.material, inherited.material.clone()),
        };
        let name_of = |id: &str| if kind.is_some() { profile_name(id) } else { material_name(id) };
        let inherit_text = tf(inherit_key, &[("name", &name_of(inherit_id.as_deref().unwrap_or(NONE_ID)))]);
        let shown = field.as_deref().map_or_else(|| inherit_text.clone(), name_of);
        ui.horizontal(|ui| {
            ui.label(RichText::new(t(label_key)).size(theme.font_size_small).color(theme.text_muted()));
            egui::ComboBox::from_id_salt((salt, label_key))
                .width(170.0)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(field.is_none(), inherit_text.as_str()).hint("construction.finish_inherit_hint").clicked() {
                        *field = None;
                        changed = true;
                    }
                    let mut ids: Vec<String> = match kind {
                        Some(k) => cat.profiles_of(k).map(|p| p.id.clone()).collect(),
                        None => cat.materials.iter().map(|m| m.id.clone()).collect(),
                    };
                    if kind.is_some() {
                        ids.insert(0, NONE_ID.to_string());
                    }
                    for id in ids {
                        let on = field.as_deref() == Some(id.as_str());
                        if ui.selectable_label(on, name_of(&id)).hint("construction.finish_option_hint").clicked() {
                            *field = Some(id);
                            changed = true;
                        }
                    }
                })
                .response
                .hint(hint_key);
        });
    }
    let unknown = cat.unknown_ids(f);
    if !unknown.is_empty() {
        ui.label(RichText::new(tf("construction.finish_unknown", &[("ids", &unknown.join(", "))])).size(theme.font_size_small).color(theme.warning()));
    }
    changed
}

/// A drawing of the skirting and crown a choice resolves to, side by side at the same scale, in the
/// trim material's colour: the preview beside the pickers (the 3D view shows it full size).
fn profile_preview(ui: &mut egui::Ui, theme: &Theme, resolved: &Finish) {
    let cat = catalog();
    let pick = |id: &Option<String>| id.as_deref().filter(|i| *i != NONE_ID).and_then(|i| cat.profile(i));
    let (sk, cr) = (pick(&resolved.skirting), pick(&resolved.crown));
    let mat = resolved.material.as_deref().and_then(|m| cat.material(m));
    let fill = mat.map_or(theme.text_muted(), |m| {
        egui::Color32::from_rgb((m.color.0 * 255.0) as u8, (m.color.1 * 255.0) as u8, (m.color.2 * 255.0) as u8) // theme-exempt: the trim material's own colour, room_finishes.ron data
    });
    // Tall enough that a 20 mm deep skirting reads as a shape at true scale (about 12 px deep).
    let size = egui::vec2(ui.available_width().min(240.0), 130.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    resp.hint("construction.finish_preview_hint");
    let painter = ui.painter_at(rect);
    painter.rect_stroke(rect, theme.border_radius, egui::Stroke::new(1.0_f32, theme.border()), egui::StrokeKind::Inside);
    // 0.2 m of profile fills the height; each drawn against its wall (a vertical line) and its floor
    // or ceiling (a horizontal one).
    let scale = (rect.height() - 12.0) / 0.2;
    let draw = |p: &TrimProfile, origin: egui::Pos2, down: bool| {
        let to = |(o, u): (f32, f32)| egui::pos2(origin.x + o * scale, if down { origin.y + u * scale } else { origin.y - u * scale });
        // The wall corner first: egui fills a polygon as a fan from its first point, and a moulding's
        // section is fan-shaped from that corner (the same assumption the end caps make).
        let mut poly: Vec<egui::Pos2> = vec![to((0.0, 0.0))];
        poly.extend(p.points.iter().map(|&q| to(q)));
        painter.add(egui::Shape::convex_polygon(poly.clone(), fill, egui::Stroke::NONE));
        painter.add(egui::Shape::line(poly, egui::Stroke::new(1.0_f32, theme.text_secondary())));
        let wall = egui::Stroke::new(1.0_f32, theme.text_muted());
        let along = if down { origin.y + 0.18 * scale } else { origin.y - 0.18 * scale };
        painter.line_segment([origin, egui::pos2(origin.x, along)], wall);
        painter.line_segment([origin, egui::pos2(origin.x + 0.12 * scale, origin.y)], wall);
    };
    let pad = 6.0;
    if let Some(p) = sk {
        draw(p, egui::pos2(rect.left() + pad + 4.0, rect.bottom() - pad), false);
    }
    if let Some(p) = cr {
        draw(p, egui::pos2(rect.center().x + 4.0, rect.top() + pad), true);
    }
    if sk.is_none() && cr.is_none() {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, t("construction.finish_preview_none"), egui::FontId::proportional(theme.font_size_small), theme.text_muted());
    }
}

/// What a room's open fields resolve to: its own choice over its purpose's default.
fn room_resolved(z: Option<&humanity_ship::home_structure::Zone>) -> Finish {
    let base = catalog().default_for(z.and_then(|z| z.room_type.as_deref()));
    z.map_or(base.clone(), |z| z.finish.over(&base))
}

/// The selected room's finish, under its room type in the zone panel.
pub(super) fn zone_finish_controls(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, zone_id: &str) {
    let Some(z) = zone_body(&state.ship_structure, state.construction_zone).and_then(|hs| hs.zones.iter().find(|z| z.id == zone_id).cloned()) else {
        return;
    };
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("construction.finish_title")).strong().color(theme.text_secondary()));
    ui.label(RichText::new(t("construction.finish_intro")).size(theme.font_size_small).color(theme.text_muted()));
    let default = catalog().default_for(z.room_type.as_deref());
    let mut f = z.finish.clone();
    let mut changed = finish_pickers(ui, theme, &mut f, &default, &format!("zone_finish_{zone_id}"), "construction.finish_inherit_default");
    if !f.is_empty() && ui.small_button(t("construction.finish_reset")).hint("construction.finish_reset_hint").clicked() {
        f = Finish::default();
        changed = true;
    }
    profile_preview(ui, theme, &f.over(&default));
    if changed {
        if let Some(zz) = zone_body_mut(&mut state.ship_structure, state.construction_zone).and_then(|hs| hs.zones.iter_mut().find(|z| z.id == zone_id)) {
            zz.finish = f;
        }
        state.construction_structure_dirty = true;
    }
}

/// The room a side of wall `wi` faces: the smallest zone of its storey holding a point 0.3 m out
/// from the middle of that side.
fn room_facing(hs: &HomeStructure, wi: usize, left: bool) -> Option<&humanity_ship::home_structure::Zone> {
    let w = hs.walls.get(wi)?;
    let (dx, dz) = (w.b.0 - w.a.0, w.b.1 - w.a.1);
    let len = (dx * dx + dz * dz).sqrt().max(1e-6);
    // Your left walking from a to b, x east and z south, is (dz, -dx) / len.
    let s = if left { 1.0 } else { -1.0 };
    let reach = w.resolved_thickness() * 0.5 + 0.3;
    let (px, pz) = ((w.a.0 + w.b.0) * 0.5 + s * dz / len * reach, (w.a.1 + w.b.1) * 0.5 - s * dx / len * reach);
    let storeys = hs.storeys();
    let k = hs.storey_of_wall(w);
    hs.zones
        .iter()
        .filter(|z| storey_index(&storeys, z.origin.1) == k && z.contains_xz(px, pz))
        .min_by(|a, b| (a.size.0 * a.size.2).partial_cmp(&(b.size.0 * b.size.2)).unwrap_or(std::cmp::Ordering::Equal))
}

/// The selected wall's own finish, one block per side, each open field taking the room on that
/// side. True when a choice changed (the wall panel rebuilds the home then).
pub(super) fn wall_finish_controls(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sel: usize) -> bool {
    let Some(hs) = zone_body(&state.ship_structure, state.construction_zone) else {
        return false;
    };
    let Some(wall) = hs.walls.get(sel) else {
        return false;
    };
    let mut fin = wall.finish.clone();
    let sides: Vec<(bool, Option<String>, Finish)> = [true, false]
        .into_iter()
        .map(|left| {
            let z = room_facing(hs, sel, left);
            let label = z.map(|z| if z.label.is_empty() { z.id.clone() } else { z.label.clone() });
            (left, label, room_resolved(z))
        })
        .collect();
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("construction.finish_wall_title")).strong().color(theme.text_secondary()));
    ui.label(RichText::new(t("construction.finish_wall_intro")).size(theme.font_size_small).color(theme.text_muted()));
    let mut changed = false;
    for (left, label, inherited) in sides {
        let head = match &label {
            Some(room) => tf("construction.finish_wall_side", &[("room", room)]),
            None => t("construction.finish_wall_side_open").to_string(),
        };
        ui.label(RichText::new(head).size(theme.font_size_small).color(theme.text_primary()));
        let side = fin.side_mut(left);
        changed |= finish_pickers(ui, theme, side, &inherited, &format!("wall_finish_{sel}_{left}"), "construction.finish_inherit_room");
        profile_preview(ui, theme, &side.over(&inherited));
    }
    if !fin.is_empty() && ui.small_button(t("construction.finish_wall_reset")).hint("construction.finish_wall_reset_hint").clicked() {
        fin = Default::default();
        changed = true;
    }
    if changed {
        if let Some(w) = zone_body_mut(&mut state.ship_structure, state.construction_zone).and_then(|hs| hs.walls.get_mut(sel)) {
            w.finish = fin;
        }
    }
    changed
}

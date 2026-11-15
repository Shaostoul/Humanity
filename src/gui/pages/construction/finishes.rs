//! ROOM FINISHES in the build editor (room finishes rungs 1 and 2, docs/design/room-finishes.md):
//! the selected room's (zone's) skirting, crown, casing, window sill, chair rail, picture rail and
//! trim material; the selected wall's own choice per side; and the inside face of each outer wall
//! the selected room runs along (the per-edge finish), each picked from the catalog
//! (data/blueprints/room_finishes.ron) with a to-scale drawing of the cross-sections it resolves
//! to. Every choice starts open, taking the default for what the room is for, and the home
//! rebuilds at once on a change, so the 3D view is the full-size preview. A child of the page in
//! the 2018 directory form, like storeys.rs.

use super::*;
use crate::gui::widgets::Hint;
use humanity_core::i18n::{t, t_dyn, tf};
use humanity_ship::home_structure::{storey_index, HomeStructure, Zone};
use humanity_ship::room_finishes::{catalog, closed_section, triangulate, Finish, ProfileKind, TrimProfile, NONE_ID};

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

/// The picker rows in the order shown: (label key, hint key, the profile kind it picks; None for
/// the trim material).
const ROWS: [(&str, &str, Option<ProfileKind>); 7] = [
    ("construction.finish_skirting", "construction.finish_skirting_hint", Some(ProfileKind::Skirting)),
    ("construction.finish_crown", "construction.finish_crown_hint", Some(ProfileKind::Crown)),
    ("construction.finish_casing", "construction.finish_casing_hint", Some(ProfileKind::Casing)),
    ("construction.finish_sill", "construction.finish_sill_hint", Some(ProfileKind::Sill)),
    ("construction.finish_chair_rail", "construction.finish_chair_rail_hint", Some(ProfileKind::ChairRail)),
    ("construction.finish_picture_rail", "construction.finish_picture_rail_hint", Some(ProfileKind::PictureRail)),
    ("construction.finish_material", "construction.finish_material_hint", None),
];

/// The pickers for one choice, one per row of `ROWS`, each "as inherited" (None) or a pick.
/// `inherited` is what each open field resolves to, named in its first entry under `inherit_key`
/// ("Usual for this room: X" for a room, "As the room: X" for a wall side). True when the choice
/// changed.
fn finish_pickers(ui: &mut egui::Ui, theme: &Theme, f: &mut Finish, inherited: &Finish, salt: &str, inherit_key: &'static str) -> bool {
    let cat = catalog();
    let mut changed = false;
    for (label_key, hint_key, kind) in ROWS {
        let (field, inherit_id) = match kind {
            Some(k) => (f.field_mut(k), inherited.field(k).clone()),
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

/// A drawing of every piece of trim a choice resolves to, each a cut through it at the same true
/// scale, in the trim material's colour, in four columns: the skirting on the floor; the crown
/// under the ceiling; the picture rail high and the chair rail low; the casing seen from above
/// beside its opening, and a window's sill with its apron under it. Each against the wall it is
/// fixed to. The preview beside the pickers (the 3D view shows it full size).
fn profile_preview(ui: &mut egui::Ui, theme: &Theme, resolved: &Finish) {
    let cat = catalog();
    let pick = |k: ProfileKind| resolved.field(k).as_deref().filter(|i| *i != NONE_ID).and_then(|i| cat.profile(i)).filter(|p| p.kind == k);
    let mat = resolved.material.as_deref().and_then(|m| cat.material(m));
    let fill = mat.map_or(theme.text_muted(), |m| {
        egui::Color32::from_rgb((m.color.0 * 255.0) as u8, (m.color.1 * 255.0) as u8, (m.color.2 * 255.0) as u8) // theme-exempt: the trim material's own colour, room_finishes.ron data
    });
    // Tall enough that a 20 mm deep skirting reads as a shape at true scale (about 14 px deep).
    let size = egui::vec2(ui.available_width().min(300.0), 150.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    resp.hint("construction.finish_preview_hint");
    let painter = ui.painter_at(rect);
    painter.rect_stroke(rect, theme.border_radius, egui::Stroke::new(1.0_f32, theme.border()), egui::StrokeKind::Inside);
    let pad = 6.0;
    // 0.2 m of profile fills the height.
    let scale = (rect.height() - 2.0 * pad) / 0.2;
    let surface = egui::Stroke::new(1.0_f32, theme.text_muted());
    // One section, filled the way the bake caps it (room_finishes.rs `triangulate`), outlined;
    // `to` puts a profile point (out, up) on the screen.
    let draw = |p: &TrimProfile, to: &dyn Fn((f32, f32)) -> egui::Pos2| {
        let sec = closed_section(&p.points);
        let mut mesh = egui::Mesh::default();
        for q in &sec {
            mesh.colored_vertex(to(*q), fill);
        }
        for tr in triangulate(&sec) {
            mesh.add_triangle(tr[0] as u32, tr[1] as u32, tr[2] as u32);
        }
        painter.add(egui::Shape::mesh(mesh));
        let mut outline: Vec<egui::Pos2> = sec.iter().map(|q| to(*q)).collect();
        outline.push(outline[0]);
        painter.add(egui::Shape::line(outline, egui::Stroke::new(1.0_f32, theme.text_secondary())));
    };
    let col = rect.width() / 4.0;
    let (top, bottom) = (rect.top() + pad, rect.bottom() - pad);
    let wall = |x: f32, y0: f32, y1: f32| painter.line_segment([egui::pos2(x, y0), egui::pos2(x, y1)], surface);
    let level = |y: f32, x0: f32, x1: f32| painter.line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], surface);
    let (sk, cr, ca, si, ch, pi) = (pick(ProfileKind::Skirting), pick(ProfileKind::Crown), pick(ProfileKind::Casing), pick(ProfileKind::Sill), pick(ProfileKind::ChairRail), pick(ProfileKind::PictureRail));
    // Column 1: the skirting on the floor, against the wall.
    let x = rect.left() + pad + 4.0;
    if let Some(p) = sk {
        wall(x, bottom - 0.19 * scale, bottom);
        level(bottom, x, x + 0.09 * scale);
        draw(p, &|(o, u)| egui::pos2(x + o * scale, bottom - u * scale));
    }
    // Column 2: the crown under the ceiling, against the wall.
    let x = rect.left() + col + 4.0;
    if let Some(p) = cr {
        wall(x, top, top + 0.19 * scale);
        level(top, x, x + 0.09 * scale);
        draw(p, &|(o, u)| egui::pos2(x + o * scale, top + u * scale));
    }
    // Column 3: the picture rail high and the chair rail low, against the wall.
    let x = rect.left() + 2.0 * col + 4.0;
    if pi.is_some() || ch.is_some() {
        wall(x, top, bottom);
    }
    if let Some(p) = pi {
        let y = top + 4.0 + p.height() * scale;
        draw(p, &|(o, u)| egui::pos2(x + o * scale, y - u * scale));
    }
    if let Some(p) = ch {
        let y = bottom - 4.0;
        draw(p, &|(o, u)| egui::pos2(x + o * scale, y - u * scale));
    }
    // Column 4: the casing from above, the wall's face level and the opening's edge on the right
    // where the board ends; under it a cut through a window's foot, the sill on the wall with the
    // casing's apron under it.
    let x = rect.left() + 3.0 * col + 4.0;
    if let Some(p) = ca {
        let (ex, y) = (x + p.height() * scale, top + 4.0 + p.depth() * scale);
        level(y, x, ex);
        wall(ex, y, y + 0.03 * scale);
        draw(p, &|(o, u)| egui::pos2(ex - u * scale, y - o * scale));
    }
    if let Some(s) = si {
        // The stool's underside, under the casing's drawing with a gap.
        let y = top + 0.075 * scale + s.height() * scale;
        wall(x, y - s.height() * scale - 0.02 * scale, bottom);
        draw(s, &|(o, u)| egui::pos2(x + o * scale, y - u * scale));
        if let Some(c) = ca {
            draw(c, &|(o, u)| egui::pos2(x + o * scale, y + u * scale));
        }
    }
    if [sk, cr, ca, si, ch, pi].iter().all(|p| p.is_none()) {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, t("construction.finish_preview_none"), egui::FontId::proportional(theme.font_size_small), theme.text_muted());
    }
}

/// What a room's open fields resolve to: its own choice over its purpose's default.
fn room_resolved(z: Option<&Zone>) -> Finish {
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
    shell_finish_controls(ui, theme, state, &z);
}

/// The outer walls (perimeter edges: 0 north at z = 0, 1 east at x = width, 2 south at z = depth,
/// 3 west at x = 0) a room's zone runs along: its box comes within the shell's half thickness and
/// 0.3 m of that edge.
fn shell_edges_of(hs: &HomeStructure, z: &Zone) -> Vec<usize> {
    let reach = hs.shell_resolved_thickness() * 0.5 + 0.3;
    let (x0, z0, x1, z1) = (z.origin.0, z.origin.2, z.origin.0 + z.size.0, z.origin.2 + z.size.2);
    [(0, z0 <= reach), (1, x1 >= hs.width - reach), (2, z1 >= hs.depth - reach), (3, x0 <= reach)].into_iter().filter(|e| e.1).map(|e| e.0).collect()
}

/// THE OUTER WALLS' OWN FINISH (rung 2, the per-edge finish), folded under the room's: one block
/// for each outer wall the selected room runs along, each open field taking the rooms along that
/// wall (named by this room's choice). It sets that whole wall's inside face, every room along it.
fn shell_finish_controls(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, z: &Zone) {
    let Some(hs) = zone_body(&state.ship_structure, state.construction_zone) else {
        return;
    };
    let edges = shell_edges_of(hs, z);
    if edges.is_empty() {
        return;
    }
    let inherited = room_resolved(Some(z));
    let mut sf = hs.shell_finish.clone();
    let mut changed = false;
    let names = ["construction.finish_shell_north", "construction.finish_shell_east", "construction.finish_shell_south", "construction.finish_shell_west"];
    egui::CollapsingHeader::new(RichText::new(t("construction.finish_shell_title")).color(theme.text_secondary()))
        .id_salt(("shell_finish", &z.id))
        .default_open(!sf.is_empty())
        .show(ui, |ui| {
            ui.label(RichText::new(t("construction.finish_shell_intro")).size(theme.font_size_small).color(theme.text_muted()));
            for e in edges {
                ui.label(RichText::new(t(names[e])).size(theme.font_size_small).color(theme.text_primary()));
                let side = sf.edge_mut(e);
                changed |= finish_pickers(ui, theme, side, &inherited, &format!("shell_finish_{e}"), "construction.finish_inherit_rooms");
                profile_preview(ui, theme, &side.over(&inherited));
            }
            if !sf.is_empty() && ui.small_button(t("construction.finish_shell_reset")).hint("construction.finish_shell_reset_hint").clicked() {
                sf = Default::default();
                changed = true;
            }
        })
        .header_response
        .hint("construction.finish_shell_hint");
    if changed {
        if let Some(hs) = zone_body_mut(&mut state.ship_structure, state.construction_zone) {
            hs.shell_finish = sf;
        }
        state.construction_structure_dirty = true;
    }
}

/// The room a side of wall `wi` faces: the smallest zone of its storey holding a point 0.3 m out
/// from the middle of that side.
fn room_facing(hs: &HomeStructure, wi: usize, left: bool) -> Option<&Zone> {
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

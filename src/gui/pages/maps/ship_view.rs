//! The Maps page's SHIP VIEW (Maps increment 3, "into the ship, deck by deck",
//! docs/design/maps-zoom-to-anything.md 5.7). The operator, 2026-10-20: "Eventually we need to
//! use the map page to navigate the multikilometer long spaceships AND planet surfaces including
//! subterranean structures."
//!
//! Close to your ship (within `PICKER_SHIP_LENGTHS` of it, aboard) the LEVEL PICKER shows the
//! ship's levels top to bottom (ship/levels.rs: its distinct storey floors, from the data), yours
//! marked. Picking one shows it CUT OPEN: everything above it taken away (engine/map_cut.rs, the
//! build editor's cut-away rule, so the map and the editor cut a home alike), and over it the
//! level's ROOMS, each filled in its room type's colour (`default_color` in data/rooms.ron) and
//! named as the HUD names it (`RoomBounds::display_name`, the HUD's own rooms), and YOUR DOT with
//! the way you face, on the level you are on (the level and the room come from the engine,
//! engine/map_feed.rs, by the game's own rules). "Plan" turns the view straight down and draws
//! the level from baked PLAN TILES (engine/map_tiles.rs) instead of rendering the ship every
//! frame, the way a slippy map pans over kept tiles.
//!
//! Everything is placed in the ship's own frame and projected with the page's camera
//! (`MapSelf::ship_to_world`, `MapProjector`), so a room's fill sits on the room the world layer
//! draws. With no 3D world (a snapshot, or before the world loads) the level's walls are drawn as
//! lines from the data instead, so the plan still reads.

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, Pos2, Rect, Shape, Stroke, Vec2};
use glam::DVec3;

use crate::gui::pages::maps::tile_grid::{keys_covering, rects_touch, zoom_for, TileKey};
use crate::gui::state_types::{MapSelf, MapsState};
use crate::gui::theme::Theme;
use crate::gui::widgets::label_placer::LabelPlacer;
use crate::gui::widgets::level_picker::{self, PickerClick, PickerLevel};
use crate::gui::GuiState;
use crate::ship::room_types::RoomBounds;
use crate::ship::home_structure::{storey_index, wall_in_storey};
use crate::ship::levels::{self, ShipLevel};

use super::camera::{MapAnchor, MapProjector, MapScene, PITCH_LIMIT_RAD};
use super::world_layer::WorldDrawn;
use super::MapView;

/// Within this many ship lengths of the ship's centre the level picker shows, and a level picked
/// is shown (the page opens 1.5 lengths out).
pub const PICKER_SHIP_LENGTHS: f64 = 3.0;
/// Picking a level from farther than three times this flies to you, this far, looking down at
/// the rooms (a 90 m view is about 100 m across: a room's name reads).
pub const INSIDE_VIEW_M: f64 = 90.0;
/// How steeply the level view looks down (the plan is straight down).
pub const INSIDE_PITCH_RAD: f64 = -60.0 * std::f64::consts::PI / 180.0;
/// The most plan tiles a view asks for: past it, the next coarser zoom.
const MAX_TILES_WANTED: usize = 48;
/// Fills and lines sit this far over a floor, metres, so they never meet it.
const FLOOR_LIFT_M: f64 = 0.03;
/// A body's walls are drawn as lines (no world) only once its box is this big on screen, pixels.
const WALLS_MIN_PX: f32 = 60.0;
/// How far your dot and its wedge reach from where you stand, pixels: no name goes over them.
const YOU_REACH_PX: f32 = 18.0;

/// Whether the level picker shows: aboard, the Solar System view (the map's own camera), not the
/// Diagram, the camera within `PICKER_SHIP_LENGTHS` of the ship's centre.
pub fn picker_shown(maps: &MapsState) -> bool {
    let Some(me) = maps.me.as_ref().filter(|m| m.aboard) else { return false };
    if maps.view != MapView::System || maps.diagram {
        return false;
    }
    (maps.camera.pose(&maps.scene()).eye - me.ship_centre_m()).length() <= me.ship_length_m() * PICKER_SHIP_LENGTHS
}

/// The level shown cut open (its ordinal): the one picked, or yours on the plan; only where the
/// picker shows. None is the whole ship.
pub fn shown_level(maps: &MapsState) -> Option<i32> {
    if !picker_shown(maps) {
        return None;
    }
    maps.level_pick.or_else(|| if maps.plan { maps.me.as_ref()?.level } else { None })
}

/// Whether the straight-down plan is shown (a level shown, the plan on).
pub fn plan_shown(maps: &MapsState) -> bool {
    maps.plan && shown_level(maps).is_some()
}

/// Fly to you (`MapAnchor::You`, followed as you walk), `distance` from you, at this pitch and yaw.
fn fly_to_you(maps: &mut MapsState, distance: f64, pitch: f64, yaw: f64) {
    let me = maps.me.clone();
    let scene = MapScene::new(me.as_ref(), maps.sim_time_seconds, maps.live).with_diagram(maps.diagram);
    maps.camera.fly_to(MapAnchor::You, distance, yaw, pitch, &scene);
}

/// Show level `pick` (an ordinal) cut open, or the whole ship (None, the plan off with it), as
/// the picker and the rig's `map_level` do. From far off (and not on the plan) the camera flies
/// to you, close enough to read the rooms.
pub fn pick_level(maps: &mut MapsState, pick: Option<i32>) {
    maps.level_pick = pick;
    if pick.is_none() {
        maps.plan = false;
        return;
    }
    let far = maps.camera.distance() > INSIDE_VIEW_M * 3.0 || maps.camera.anchor != MapAnchor::You;
    if !maps.plan && far {
        fly_to_you(maps, INSIDE_VIEW_M, INSIDE_PITCH_RAD, maps.camera.yaw);
    }
}

/// The plan on (straight down over you, the ship's bow, its -z, up the screen) or off (back to
/// the level view's slant), at the camera's distance.
pub fn set_plan(maps: &mut MapsState, on: bool) {
    maps.plan = on;
    let d = maps.camera.distance();
    if on {
        fly_to_you(maps, d, -PITCH_LIMIT_RAD, 0.0);
    } else {
        fly_to_you(maps, d, INSIDE_PITCH_RAD, maps.camera.yaw);
    }
}

/// "Back to me" and the Home key: with a level shown, back to you on it (at the level view's
/// distance, straight down on the plan); otherwise the camera's own (your ship, followed).
pub fn back_to_me(maps: &mut MapsState) {
    if shown_level(maps).is_some() {
        let (pitch, yaw) = if maps.plan { (-PITCH_LIMIT_RAD, 0.0) } else { (INSIDE_PITCH_RAD, maps.camera.yaw) };
        fly_to_you(maps, INSIDE_VIEW_M, pitch, yaw);
    } else {
        let me = maps.me.clone();
        let scene = MapScene::new(me.as_ref(), maps.sim_time_seconds, maps.live).with_diagram(maps.diagram);
        maps.camera.back_to_me(&scene);
    }
}

/// The rooms on level `index` of `levels`: those whose floor (the box's foot) is on it, by the
/// rule a height's level is found by (`levels::level_at`). A room open through two storeys (the
/// greenhouse to its 6 m roof) is on the level its floor is.
pub fn rooms_on_level<'a>(rooms: &'a [RoomBounds], levels: &[ShipLevel], index: usize) -> Vec<&'a RoomBounds> {
    rooms.iter().filter(|r| levels::level_at(levels, r.min.y) == Some(index)).collect()
}

/// Whether a room is NAMED on the map: the data gives it a name. A room it does not (a corridor
/// tube, `corridor_0`; a zone's leftover floor, `commons:room_1`) is shown by the HUD under its
/// id, which on a map is noise: it is filled and outlined, not named.
pub fn room_is_named(r: &RoomBounds) -> bool {
    r.display_name != r.id
}

/// A room's fill: its room type's colour from data/rooms.ron at the theme's fill opacity, or the
/// theme's own for a room with no type (a corridor, an unnamed room).
fn room_fill(theme: &Theme, r: &RoomBounds) -> Color32 {
    let a = theme.map_room_fill_alpha.clamp(0.0, 1.0);
    match r.fill {
        Some([cr, cg, cb, _]) => super::bodies::faded(Color32::from_rgb((cr * 255.0) as u8, (cg * 255.0) as u8, (cb * 255.0) as u8), a), // theme-exempt: the room type's own colour, data/rooms.ron
        None => super::bodies::faded(theme.map_room_fill(), a),
    }
}

/// The four corners of a ship-frame rectangle at height `y`, on the canvas, or None when any is
/// behind the camera.
fn floor_quad(proj: &MapProjector, me: &MapSelf, min: (f64, f64), max: (f64, f64), y: f64) -> Option<[Pos2; 4]> {
    let at = |x: f64, z: f64| proj.project(me.ship_to_world(DVec3::new(x, y, z))).map(|(p, _)| p);
    Some([at(min.0, min.1)?, at(max.0, min.1)?, at(max.0, max.1)?, at(min.0, max.1)?])
}

/// The ship's levels (none without a ship).
fn ship_levels(state: &GuiState) -> Vec<ShipLevel> {
    state.ship_structure.as_ref().map(levels::ship_levels).unwrap_or_default()
}

/// Where the canvas's four corners meet the floor at height `floor` (ship frame), or None when a
/// corner looks above it.
fn floor_hits(proj: &MapProjector, me: &MapSelf, floor: f64) -> Option<[DVec3; 4]> {
    let (r, p) = (proj.rect, proj.pose);
    let per_px = (p.fov_y * 0.5).tan() / (r.height() as f64 * 0.5);
    let inv = me.ship_rot.inverse();
    let eye = inv * (p.eye - me.ship_pos_m);
    let mut out = [DVec3::ZERO; 4];
    for (i, c) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().enumerate() {
        let (x, y) = ((c.x - r.center().x) as f64 * per_px, (r.center().y - c.y) as f64 * per_px);
        let d = inv * (p.forward + p.right * x + p.up * y);
        let t = (floor - eye.y) / d.y;
        if d.y > -1e-6 || t <= 0.0 {
            return None;
        }
        out[i] = eye + d * t;
    }
    Some(out)
}

/// The plan tiles this view covers on `level`: at the zoom that keeps a tile pixel no bigger than
/// a screen pixel (`zoom_for`), coarser while that would be more than `MAX_TILES_WANTED`, only
/// over the ship, nearest the view's centre first.
pub fn tiles_wanted(proj: &MapProjector, me: &MapSelf, level: &ShipLevel) -> Vec<TileKey> {
    let Some(h) = floor_hits(proj, me, level.floor as f64) else { return Vec::new() };
    let (xs, zs) = (h.map(|p| p.x as f32), h.map(|p| p.z as f32));
    let lo = |v: [f32; 4]| v.iter().copied().fold(f32::MAX, f32::min);
    let hi = |v: [f32; 4]| v.iter().copied().fold(f32::MIN, f32::max);
    let view = [lo(xs), lo(zs), hi(xs), hi(zs)];
    let ship = me.ship_box_m.map(|(a, b)| [a.x as f32, a.z as f32, b.x as f32, b.z as f32]).unwrap_or(view);
    let m_per_px = (h[1] - h[0]).length() / proj.rect.width().max(1.0) as f64;
    let mut z = zoom_for(m_per_px);
    loop {
        let mut keys: Vec<TileKey> = keys_covering(level.ordinal, z, view).into_iter().filter(|k| rects_touch(k.rect(), ship)).collect();
        if keys.len() <= MAX_TILES_WANTED || z == 0 {
            let c = ((view[0] + view[2]) * 0.5, (view[1] + view[3]) * 0.5);
            let dist = |k: &TileKey| {
                let r = k.rect();
                ((r[0] + r[2]) * 0.5 - c.0).hypot((r[1] + r[3]) * 0.5 - c.1)
            };
            keys.sort_by(|a, b| dist(a).total_cmp(&dist(b)));
            return keys;
        }
        z -= 1;
    }
}

/// Paint `key`'s square of the floor from `outer`'s texture (itself, or a coarser tile holding it).
fn paint_tile(paint: &egui::Painter, proj: &MapProjector, me: &MapSelf, key: &TileKey, outer: &TileKey, tex: egui::TextureId, floor: f64) {
    let (r, uv) = (key.rect().map(f64::from), key.uv_in(outer));
    let Some(q) = floor_quad(proj, me, (r[0], r[1]), (r[2], r[3]), floor) else { return };
    let mut mesh = Mesh::with_texture(tex);
    for (pos, (u, v)) in q.into_iter().zip([(uv[0], uv[1]), (uv[2], uv[1]), (uv[2], uv[3]), (uv[0], uv[3])]) {
        mesh.vertices.push(Vertex { pos, uv: Pos2::new(u, v), color: Color32::WHITE });
    }
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    paint.add(Shape::mesh(mesh));
}

/// THE PLAN'S BACKDROP: the tiles this view wants (told to the engine through `MapsState::tiles`)
/// drawn where baked, a coarser tile's part where the finer one is not yet.
pub(super) fn draw_tiles(paint: &egui::Painter, proj: &MapProjector, state: &mut GuiState) {
    let levels = ship_levels(state);
    let maps = &mut state.maps;
    let (Some(me), Some(level)) = (maps.me.clone(), shown_level(maps).and_then(|o| levels::by_ordinal(&levels, o))) else {
        maps.tiles.want.clear();
        return;
    };
    let want = tiles_wanted(proj, &me, &level);
    for key in &want {
        let mut outer = Some(*key);
        while let Some(o) = outer {
            if let Some(tex) = maps.tiles.have.get(&o) {
                paint_tile(paint, proj, &me, key, &o, *tex, level.floor as f64);
                break;
            }
            outer = o.parent();
        }
    }
    maps.tiles.want = want;
}

/// THE LEVEL SHOWN: with no world under it, its walls as lines; its rooms filled and named; you
/// and the way you face when you are on it. Records what it drew for the rig's report.
pub(super) fn draw_inside(paint: &egui::Painter, theme: &Theme, proj: &MapProjector, state: &mut GuiState, placer: &mut LabelPlacer, world: WorldDrawn) {
    let levels = ship_levels(state);
    let mut drawn = crate::gui::MapShipDrawn {
        levels: levels.iter().rev().map(ShipLevel::name).collect(),
        ..Default::default()
    };
    if !plan_shown(&state.maps) {
        state.maps.tiles.want.clear();
    }
    let me = state.maps.me.clone();
    let shown = shown_level(&state.maps).and_then(|o| levels.iter().position(|l| l.ordinal == o));
    let (Some(me), Some(index)) = (me, shown) else {
        state.maps.ship_drawn = drawn;
        return;
    };
    let level = levels[index];
    drawn.level = Some(level.name());
    let floor = level.floor as f64 + FLOOR_LIFT_M;
    // The level's walls as lines from the data wherever the world is not drawn under them: on
    // the plan, over its tiles (a floor plan reads by its walls, which from straight above are
    // thin tops), and with no world at all.
    if world == WorldDrawn::Nothing {
        if let Some(ship) = state.ship_structure.as_ref() {
            draw_walls(paint, theme, proj, &me, ship, &level, floor);
        }
    }
    let rooms = rooms_on_level(&state.room_bounds, &levels, index);
    let mut offers: Vec<(&RoomBounds, Pos2, f32)> = Vec::new();
    for r in &rooms {
        let (min, max) = ((r.min.x as f64, r.min.z as f64), (r.max.x as f64, r.max.z as f64));
        let Some(q) = floor_quad(proj, &me, min, max, floor) else { continue };
        paint.add(Shape::convex_polygon(q.to_vec(), room_fill(theme, r), Stroke::new(1.0, theme.map_room_edge())));
        drawn.rooms_filled += 1;
        if !room_is_named(r) {
            continue;
        }
        let centre = Pos2::new(q.iter().map(|p| p.x).sum::<f32>() / 4.0, q.iter().map(|p| p.y).sum::<f32>() / 4.0);
        let width = q.iter().map(|p| p.x).fold(f32::MIN, f32::max) - q.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        offers.push((r, centre, width));
    }
    // Your dot is placed before the names, so your room's name goes beside it, not under it.
    let you_at = if me.level == Some(level.ordinal) { you_on_screen(proj, &me, floor) } else { None };
    if let Some((at, _)) = you_at {
        placer.reserve_exact(Rect::from_center_size(at, Vec2::splat(2.0 * YOU_REACH_PX)));
    }
    // Your room first, then the bigger rooms; a name goes inside its room or not at all.
    let yours = me.you_room.as_ref().map(|(id, _)| id.clone());
    offers.sort_by(|a, b| (Some(&b.0.id) == yours.as_ref()).cmp(&(Some(&a.0.id) == yours.as_ref())).then(b.2.total_cmp(&a.2)));
    for (r, centre, width) in offers {
        let galley = paint.layout_no_wrap(r.display_name.clone(), egui::FontId::proportional(theme.map_label_size), theme.map_label());
        let size = galley.size();
        if size.x > width * 0.95 {
            continue;
        }
        let step = YOU_REACH_PX + size.y / 2.0 + 2.0;
        let beside = you_at.filter(|_| Some(&r.id) == yours.as_ref()).map(|(at, _)| [Pos2::new(centre.x, at.y + step), Pos2::new(centre.x, at.y - step)]);
        let spots = std::iter::once(centre).chain(beside.into_iter().flatten()).map(|c| Rect::from_center_size(c, size));
        if let Some(spot) = placer.place(spots) {
            super::labels::paint_with_halo(paint, theme, spot.min, r.display_name.clone(), theme.map_label_size, theme.map_label());
            drawn.rooms_named.push(r.display_name.clone());
        }
    }
    if let Some((at, ahead)) = you_at {
        draw_you_here(paint, theme, at, ahead);
        drawn.you_drawn = true;
    }
    state.maps.ship_drawn = drawn;
}

/// Where you stand on screen, and a point a metre the way you face.
fn you_on_screen(proj: &MapProjector, me: &MapSelf, floor: f64) -> Option<(Pos2, Pos2)> {
    let feet = DVec3::new(me.you_ship_m.x, floor, me.you_ship_m.z);
    let face = DVec3::new(me.you_facing_ship.x, 0.0, me.you_facing_ship.z).normalize_or_zero();
    Some((proj.project(me.ship_to_world(feet))?.0, proj.project(me.ship_to_world(feet + face))?.0))
}

/// Your dot on the floor where you stand, and a wedge the way you face.
fn draw_you_here(paint: &egui::Painter, theme: &Theme, at: Pos2, ahead: Pos2) {
    let dir = (ahead - at).normalized();
    if dir.length() > 0.5 {
        let side = Vec2::new(-dir.y, dir.x);
        let tip = at + dir * 16.0;
        paint.add(Shape::convex_polygon(vec![tip, at + side * 6.0 + dir * 4.0, at - side * 6.0 + dir * 4.0], theme.info().gamma_multiply(0.85), Stroke::NONE));
    }
    paint.circle_filled(at, 5.0, theme.info());
    paint.circle_stroke(at, 8.0, Stroke::new(1.5, theme.info()));
}

/// With no world drawn: every body's walls on `level` (`wall_in_storey`) and its outline as lines
/// on its floor, for each body big enough on screen to read.
fn draw_walls(paint: &egui::Painter, theme: &Theme, proj: &MapProjector, me: &MapSelf, ship: &crate::ship::ship_structure::ShipStructure, level: &ShipLevel, floor: f64) {
    let stroke = Stroke::new(1.5, theme.map_room_edge());
    let line = |a: (f64, f64), b: (f64, f64)| {
        if let (Some((p, _)), Some((q, _))) = (proj.project(me.ship_to_world(DVec3::new(a.0, floor, a.1))), proj.project(me.ship_to_world(DVec3::new(b.0, floor, b.1)))) {
            paint.line_segment([p, q], stroke);
        }
    };
    for (o, b) in levels::standing_bodies(ship) {
        let (x0, z0, x1, z1) = (o.x as f64, o.z as f64, (o.x + b.width) as f64, (o.z + b.depth) as f64);
        let Some(q) = floor_quad(proj, me, (x0, z0), (x1, z1), floor) else { continue };
        if (q[2] - q[0]).length() < WALLS_MIN_PX {
            continue;
        }
        for (a, c) in [((x0, z0), (x1, z0)), ((x1, z0), (x1, z1)), ((x1, z1), (x0, z1)), ((x0, z1), (x0, z0))] {
            line(a, c);
        }
        let storeys = b.storeys();
        let k = storey_index(&storeys, level.floor - o.y);
        for w in b.walls.iter().filter(|w| wall_in_storey(&storeys, w, k)) {
            line(((o.x + w.a.0) as f64, (o.z + w.a.1) as f64), ((o.x + w.b.0) as f64, (o.z + w.b.1) as f64));
        }
    }
}

/// Where the picker's column goes on the canvas (its top-right corner): under the layer bar.
fn picker_corner(canvas: Rect) -> Pos2 {
    Pos2::new(canvas.right() - 8.0, canvas.top() + 70.0)
}

/// The levels as the picker is given them, and the one you are on. None where the picker does
/// not show, or for a ship of one level (a one-level place has no picker).
fn picker_levels(state: &GuiState) -> Option<(Vec<PickerLevel>, Option<i32>)> {
    if !picker_shown(&state.maps) {
        return None;
    }
    let levels = ship_levels(state);
    if levels.len() < 2 {
        return None;
    }
    let list = levels.iter().map(|l| PickerLevel { ordinal: l.ordinal, short: l.short_name(), name: l.name() }).collect();
    Some((list, state.maps.me.as_ref().and_then(|m| m.level)))
}

/// The picker's rectangle this frame, when it shows (the names keep off it).
pub(super) fn picker_rect(theme: &Theme, state: &GuiState, canvas: Rect) -> Option<Rect> {
    picker_levels(state).map(|(l, _)| level_picker::rect_for(theme, picker_corner(canvas), l.len() + 1))
}

/// Draw the level picker and act on a click (`pick_level`, `set_plan`).
pub(super) fn draw_picker(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, canvas: Rect) {
    let Some((list, you)) = picker_levels(state) else { return };
    let rows = level_picker::picker_rows(&list, you, shown_level(&state.maps));
    let title = state.maps.me.as_ref().map(|m| m.ship_name.clone()).unwrap_or_default();
    let (_, click) = level_picker::show(ui, theme, picker_corner(canvas), &title, &rows, state.maps.plan);
    match click {
        Some(PickerClick::Level(o)) => pick_level(&mut state.maps, o),
        Some(PickerClick::Plan(on)) => {
            if on && state.maps.level_pick.is_none() && you.is_none() {
                return; // no level to plan: you are on none, and none is picked
            }
            set_plan(&mut state.maps, on)
        }
        None => {}
    }
}

#[cfg(test)]
#[path = "ship_view_tests.rs"]
mod tests;

//! STOREYS in the B editor (ship homes increment 7, docs/design/ship-homes-increment-7-plan.md,
//! "Editor (C2)"), the engine half. A home's storeys are derived from its floor slabs
//! (`HomeStructure::storeys`); the editor works on one of them at a time
//! (`StoreyEditor::chosen`, pages/construction/storeys.rs draws the selector):
//! - THE CUT-AWAY: while it works on a storey with others above, it draws the home with those
//!   taken out (`cut_away_body`), so the ground floor can be edited under the upper one; the rooms,
//!   the lights and everything a player walks on stay the whole ship's.
//! - THE FLOOR PICK takes the chosen storey's floor (`storey_floor_hit`): the rooms on it, then,
//!   above the ground, its plane where no room is yet (a slab just drawn has none until a zone of
//!   its storey says so).
//! - PLACING goes on that storey: a new wall stands on its floor, a light hangs under its ceiling,
//!   a machine or a piece sits on its floor; corners, openings and wall picks are its own.
//! - FLOOR SLABS are drawn, resized by their corners and removed (`add_floor_slab`,
//!   `move_slab_corner`, `remove_floor_slab`); a new slab makes a storey and the selector follows.
//! A child of engine/editor.rs, which re-exports all of it (`pub(crate) use storeys::*`).

use glam::Vec3;

use super::{active_zone_origin, DRAG_THRESHOLD_PX};
use crate::engine::geom::snap_node_position;
use crate::engine::home_meshes::rebuild_homestead;
use crate::engine::state::EngineState;
pub(crate) use crate::gui::pages::construction::SlabDraw;
use crate::gui::GuiState;
use crate::ship::home_structure::{
    storey_index, wall_in_storey, FloorSlab, HomeStructure, InteriorWall, SlabMaterial, Storey, ROOM_DIVIDER_MIN_M,
    STOREY_EPS,
};
use crate::ship::ship_structure::{zone_body, zone_body_mut, ShipStructure};

/// Floor to floor of a storey drawn over the top one (metres): 3.0, the increment's storey (plan
/// section 4, decision 2: a 0.25 m slab leaves 2.75 m of room under the next floor).
pub(crate) const NEW_STOREY_M: f32 = 3.0;
/// A new slab's thickness when no slab of its floor says otherwise (metres): 0.25, what a slab's
/// file means when it leaves its thickness out (`FloorSlab::thickness`).
pub(crate) const NEW_SLAB_THICKNESS_M: f32 = 0.25;
/// The shortest side a drawn slab may have (metres): two clicks closer than this on either axis
/// are a slip of the hand, not a floor.
pub(crate) const MIN_SLAB_SIDE_M: f32 = 0.25;
/// How near a room's floor must be to the storey's floor to be on it (metres). A room's floor is
/// its storey's floor exactly (`detect_rooms`), so this only absorbs float error.
const FLOOR_MATCH_M: f32 = 0.05;
/// How near the cursor ray must pass a slab's corner handle to grab it (metres): the wall corner
/// orbs' pick radius in `compute_construction_hover`.
const SLAB_CORNER_PICK_M: f32 = 0.45;
/// How near a slab's outline a click on its plane must land to pick the slab (metres).
const SLAB_EDGE_PICK_M: f32 = 0.3;

/// The storey the editor works on, in the zone it is editing: which (`index`, bottom up, 0 the
/// ground), of how many, and its band of height (body-local).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EditStorey {
    pub(crate) index: usize,
    pub(crate) count: usize,
    pub(crate) storey: Storey,
}

impl EditStorey {
    /// Storey `chosen` of `hs`, or its top storey when it has fewer (a slab removed, another zone
    /// chosen): `chosen` is kept in range here, where it is read.
    pub(crate) fn of(hs: &HomeStructure, chosen: usize) -> Self {
        let storeys = hs.storeys();
        let index = chosen.min(storeys.len() - 1);
        EditStorey { index, count: storeys.len(), storey: storeys[index] }
    }

    /// True when there is a storey above this one, so the cut-away hides something.
    pub(crate) fn has_above(&self) -> bool {
        self.index + 1 < self.count
    }

    /// True when something standing at body-local height `y` is drawn in the cut-away: below
    /// this storey's top, or anywhere when nothing is above it.
    pub(crate) fn shows(&self, y: f32) -> bool {
        !self.has_above() || y < self.storey.top - STOREY_EPS
    }
}

/// The storey the editor works on in the zone it is editing (None without a ship).
pub(crate) fn edit_storey(state: &EngineState) -> Option<EditStorey> {
    zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
        .map(|hs| EditStorey::of(hs, state.gui_state.construction_storeys.chosen))
}

/// Storey `k`'s name as the panel shows it, counted from 1 as people count floors in a plan:
/// "Storey 1 (ground)", "Storey 2", ...
pub(crate) fn storey_name(k: usize) -> String {
    if k == 0 {
        "Storey 1 (ground)".to_string()
    } else {
        format!("Storey {}", k + 1)
    }
}

/// A floor slab's name: its label, else "Floor slab N" (counted from 1).
pub(crate) fn slab_name(i: usize, s: &FloorSlab) -> String {
    match s.label.as_deref().map(str::trim) {
        Some(l) if !l.is_empty() => l.to_string(),
        _ => format!("Floor slab {}", i + 1),
    }
}

/// The floor plan the pick keeps to (`storey_floor_hit`), world metres: the storey's floor height,
/// the body's footprint, and whether its plane counts where no room is (above the ground only:
/// the ground storey's rooms cover its whole floor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StoreyPlan {
    pub(crate) floor_y: f32,
    pub(crate) min: (f32, f32),
    pub(crate) max: (f32, f32),
    pub(crate) plane: bool,
}

/// Where a pick ray met a floor (world metres), and the room (an index into the rooms it was
/// given) whose floor it was, None when it was the storey's plane with no room there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StoreyFloorHit {
    pub(crate) room: Option<usize>,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
}

/// THE FLOOR PICK (the cursor ray `origin` + t `dir`, world metres): the nearest room floor the
/// ray lands on, of `rooms` (each its (min, max) box). With a `plan` only the rooms whose floor is
/// the plan's count, so the ground can be picked through the floor above it; and when none is
/// hit, the plan's own plane inside the body's footprint counts if `plan.plane`. With no plan (a
/// one-storey zone) every room counts, as before storeys.
pub(crate) fn storey_floor_hit(
    origin: Vec3,
    dir: Vec3,
    rooms: impl IntoIterator<Item = (Vec3, Vec3)>,
    plan: Option<StoreyPlan>,
) -> Option<StoreyFloorHit> {
    if dir.y.abs() < 1e-6 {
        return None; // a level ray meets no floor
    }
    let mut best: Option<(f32, StoreyFloorHit)> = None;
    for (i, (min, max)) in rooms.into_iter().enumerate() {
        if plan.is_some_and(|p| (min.y - p.floor_y).abs() > FLOOR_MATCH_M) {
            continue; // a floor of another storey
        }
        let t = (min.y - origin.y) / dir.y;
        if t <= 0.0 {
            continue;
        }
        let (hx, hz) = (origin.x + dir.x * t, origin.z + dir.z * t);
        if hx >= min.x && hx <= max.x && hz >= min.z && hz <= max.z && best.map_or(true, |(bt, _)| t < bt) {
            best = Some((t, StoreyFloorHit { room: Some(i), x: hx, y: min.y, z: hz }));
        }
    }
    if best.is_none() {
        if let Some(p) = plan.filter(|p| p.plane) {
            let t = (p.floor_y - origin.y) / dir.y;
            let (hx, hz) = (origin.x + dir.x * t, origin.z + dir.z * t);
            if t > 0.0 && hx >= p.min.0 && hx <= p.max.0 && hz >= p.min.1 && hz <= p.max.1 {
                best = Some((t, StoreyFloorHit { room: None, x: hx, y: p.floor_y, z: hz }));
            }
        }
    }
    best.map(|(_, h)| h)
}

/// The plan the editor's floor pick keeps to: the chosen storey of the zone being edited, in
/// world metres. None with one storey (or no ship), so a one-storey zone, every zone today, picks
/// exactly as before storeys.
pub(crate) fn storey_plan(state: &EngineState) -> Option<StoreyPlan> {
    let hs = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)?;
    let es = EditStorey::of(hs, state.gui_state.construction_storeys.chosen);
    if es.count <= 1 {
        return None;
    }
    let zo = active_zone_origin(state);
    Some(StoreyPlan {
        floor_y: zo.y + es.storey.floor,
        min: (zo.x, zo.z),
        max: (zo.x + hs.width, zo.z + hs.depth),
        plane: es.index > 0,
    })
}

/// The band of WORLD height the editor's picks keep to: from just under the floor of the storey
/// being edited to just under the next floor, so a machine, light, piece or zone of another
/// storey (hidden above the cut-away, or seen below through a stairwell) is not picked. None with
/// one storey: every height, as before storeys. `in_storey_band` tests a height against it.
pub(crate) fn storey_pick_band(state: &EngineState) -> Option<(f32, f32)> {
    let es = edit_storey(state).filter(|es| es.count > 1)?;
    let y = active_zone_origin(state).y;
    Some((y + es.storey.floor - 0.1, y + es.storey.top - STOREY_EPS))
}

/// True when world height `y` is in `band` (`storey_pick_band`); always with no band.
pub(crate) fn in_storey_band(band: Option<(f32, f32)>, y: f32) -> bool {
    band.map_or(true, |(lo, hi)| y >= lo && y < hi)
}

/// Where the cursor's ray meets the floor of the storey being edited (`storey_floor_hit` over the
/// HUD's rooms, `room_bounds`). Every placing and dragging tool of the editor stands on this.
pub(crate) fn cursor_storey_hit(state: &EngineState) -> Option<StoreyFloorHit> {
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    storey_floor_hit(origin, dir, state.gui_state.room_bounds.iter().map(|r| (r.min, r.max)), storey_plan(state))
}

/// THE BUILD-MODE AVATAR STANDS ON THE CHOSEN STOREY (ship homes increment 7, the review of
/// 2026-10-21, finding 2): it is drawn, grabbed and dragged on that storey's floor, one plane, and
/// closing the editor stands the player on that storey (home_plot.rs `editor_pick_at`). It used to
/// be drawn and grabbed on the zone's ground floor while it was dragged on the chosen storey's
/// plane and the close stood the player on the storey they had left from: three storeys for one
/// figure. The world height of its feet: the chosen storey's floor in the zone being edited.
pub(crate) fn build_avatar_floor_y(state: &EngineState) -> f32 {
    active_zone_origin(state).y + edit_storey(state).map_or(0.0, |es| es.storey.floor)
}

/// Where the editor seeds the build-mode avatar (`seed_build_avatar`): the player's own spot
/// across the floor (their eye at `eye`, world metres, `eye_height` over their feet), zone-local
/// and kept 0.3 m inside the body `hs` at `zone_origin`, and the storey their feet are on.
pub(crate) fn avatar_seed(hs: &HomeStructure, zone_origin: Vec3, eye: Vec3, eye_height: f32) -> ((f32, f32), usize) {
    let p = eye - zone_origin;
    let spot = (p.x.clamp(0.3, (hs.width - 0.3).max(0.3)), p.z.clamp(0.3, (hs.depth - 0.3).max(0.3)));
    (spot, storey_index(&hs.storeys(), eye.y - eye_height - zone_origin.y))
}

/// When the editor OPENS (lib.rs) with no build spot yet (the first time since the world loaded,
/// engine/world_load.rs): put the build-mode avatar where the player stands, ON THEIR STOREY, and
/// open the storey selector on that storey, so the avatar is where they are and on the plane it is
/// drawn and dragged on. Toggling the editor without moving it then keeps them put (v0.557, "where
/// I'm at" when I leave build mode). After that the avatar stays where it was left, the BUILD SPOT
/// the next close stands them at (v0.557; verify-copresence's editor jump and the relay's declared
/// editor close rely on it), on the storey the selector stays on. It used to be put at the home's
/// spawn when the world loaded (v0.582), which is on the ground floor, while the selector opened on
/// the ground and the close stood the player on the storey they left from: upstairs, that spot is
/// on the upper hall's south wall, over the glass roof of the house's south half.
pub(crate) fn seed_build_avatar(state: &mut EngineState) {
    if state.gui_state.build_char_pos.is_some() {
        return;
    }
    let zo = active_zone_origin(state);
    let (eye, eye_height) = (state.construction_return_pos, state.controller.eye_height());
    let g = &mut state.gui_state;
    let Some((spot, k)) = zone_body(&g.ship_structure, g.construction_zone).map(|hs| avatar_seed(hs, zo, eye, eye_height)) else {
        return;
    };
    choose_storey(g, k);
    g.build_char_pos = Some(spot);
    g.construction_storeys.avatar_seed = Some((g.construction_zone, spot, g.construction_storeys.chosen));
}

/// The home's spawn the editor's Save writes (lib.rs, "Save home"; `HomeStructure::spawn`, where
/// the player enters the world, a spot on the GROUND floor): the build-mode avatar's spot, when it
/// was moved since it was put at the player's feet (`seed_build_avatar`) and stands on the ground
/// storey. None leaves the spawn as it is: a Save without moving the avatar must not make wherever
/// the player stood the home's door, and a spot upstairs is no ground spot (the spawn has no
/// height; written as one, it would put the door under the upper floor, perhaps in a wall).
///
/// A PLACE TO STAND ON THE GROUND FLOOR (the seams review of 2026-10-21, finding 1): world entry
/// stands the player on the ground floor at the spawn, and the footing steps a body up no more than
/// a step from there, so a spot over a stair's flight where its steps are higher than that is a
/// spawn inside the flight, from which every world entry after the Save was pushed out sideways
/// through the wall beside it. The same for a placed box taller than a step (a platform, a deck, the
/// wall piece; the increment 7 follow-ups, 2026-10-21), whose top the spawn never stands on either.
/// The spawn written is the nearest place to stand on the ground storey with the flights and the
/// boxes kept out (ship/standing.rs `nearest_standing_clear_of`, `solid_keep_outs`); a spot that is
/// one is written as it is.
pub(crate) fn spawn_from_avatar(g: &GuiState) -> Option<(f32, f32)> {
    use crate::ship::standing::{nearest_standing_clear_of, solid_keep_outs};
    let spot = g.build_char_pos?;
    let hs = zone_body(&g.ship_structure, g.construction_zone)?;
    let k = EditStorey::of(hs, g.construction_storeys.chosen).index;
    let moved = g.construction_storeys.avatar_seed != Some((g.construction_zone, spot, k));
    if !(moved && k == 0) {
        return None;
    }
    nearest_standing_clear_of(hs, 0, spot.0, spot.1, &solid_keep_outs(hs, 0))
}

/// The ray distance at which wall `w` is hit (its zone's origin at world `zo`), or None. A wall is
/// a vertical sheet from its BASE to its top, no higher than the body-local `top_cap` (the
/// cut-away's top, where the view cuts it): a ray that passes under an upstairs wall misses it.
pub(crate) fn ray_hits_wall(w: &InteriorWall, zo: Vec3, origin: Vec3, dir: Vec3, top_cap: f32) -> Option<f32> {
    let a = Vec3::new(w.a.0, 0.0, w.a.1) + zo;
    let b = Vec3::new(w.b.0, 0.0, w.b.1) + zo;
    let along = b - a;
    let len = along.length();
    if len < 1e-4 {
        return None;
    }
    let along_n = along / len;
    // Horizontal normal of the (vertical) wall plane.
    let normal = Vec3::new(-along_n.z, 0.0, along_n.x);
    let denom = dir.dot(normal);
    if denom.abs() < 1e-6 {
        return None; // ray parallel to the wall face
    }
    let t = (a - origin).dot(normal) / denom;
    if t < 0.0 {
        return None; // behind the camera
    }
    let hit = origin + dir * t;
    let s = (hit - a).dot(along_n); // distance along the wall from a
    let (y0, y1) = (zo.y + w.base, zo.y + w.top().min(top_cap));
    (s >= -0.1 && s <= len + 0.1 && hit.y >= y0 - 0.1 && hit.y <= y1 + 0.1).then_some(t)
}

/// A new wall from `a` to `b` on the chosen storey: it stands on the storey's floor and rises to
/// the next floor (or the roof), in the body's shell material.
pub(crate) fn new_wall_on_storey(hs: &HomeStructure, es: EditStorey, a: (f32, f32), b: (f32, f32)) -> InteriorWall {
    InteriorWall {
        a,
        b,
        base: es.storey.floor,
        height: es.storey.height(),
        material: hs.shell_material,
        openings: Vec::new(),
        thickness: None,
        layers: Vec::new(),
    }
}

/// The structure type a guard is drawn as, and the palette's Guard entry (`structure_types.ron`).
pub(crate) const GUARD_TYPE: &str = "guard";

/// A new GUARD from `a` to `b` on the chosen storey: the palette's Guard entry draws these with the
/// wall tool (ship homes increment 7). A guard is a wall in a balustrade material, whose form is
/// the `guard` structure type (`InteriorWall::guard`; it draws as posts, balusters and a rail and
/// collides like a wall), so this is the first such material ("Steel balustrade"; Oak is a pick
/// in the wall's detail panel), as tall as that type (1.0 m), standing on the storey's floor. A
/// loose `guard` piece placed on its own would stop nobody. None when the data has no balustrade.
pub(crate) fn new_guard_on_storey(es: EditStorey, a: (f32, f32), b: (f32, f32)) -> Option<InteriorWall> {
    let material = crate::ship::home_structure::wall_materials().iter().find(|m| m.form.as_deref() == Some(GUARD_TYPE))?;
    let height = crate::ship::structure::structure_type(GUARD_TYPE)?.size.1;
    Some(InteriorWall {
        a,
        b,
        base: es.storey.floor,
        height,
        material: material.id,
        openings: Vec::new(),
        thickness: None,
        layers: Vec::new(),
    })
}

/// How high a new light of `type_id` goes on the chosen storey at (x, z), body-local metres. A warm
/// lamp sits 0.8 m up its storey (table height); a ceiling type hangs 0.3 m under the ceiling over
/// that spot: the underside of the lowest slab above the storey's floor that covers it, else the
/// roof (a room open to the roof, a stairwell).
pub(crate) fn light_height_on_storey(hs: &HomeStructure, es: EditStorey, x: f32, z: f32, type_id: &str) -> f32 {
    let floor = es.storey.floor;
    if type_id == "warm_lamp" {
        return floor + 0.8;
    }
    let roof = hs.storeys().last().map_or(hs.height, |s| s.top);
    let ceiling = hs
        .floors
        .iter()
        .filter(|s| s.top > floor + STOREY_EPS && s.contains(x, z))
        .map(FloorSlab::underside)
        .fold(roof, f32::min);
    (ceiling - 0.3).max(floor + 0.3)
}

/// True when wall `w` is on storey `k` of `storeys`: its base is on it. Its corners are that
/// storey's to show and drag.
fn wall_on(storeys: &[Storey], w: &InteriorWall, k: usize) -> bool {
    storey_index(storeys, w.base) == k
}

/// True when two corners are the same corner (the editor's 5 cm tolerance).
fn same_corner(p: (f32, f32), q: (f32, f32)) -> bool {
    (p.0 - q.0).abs() < 0.05 && (p.1 - q.1).abs() < 0.05
}

/// Every corner of the walls on storey `k` (`wall_on`), each once: the corner orbs the editor
/// shows and drags on that storey (the editor kept every storey's corners as one set before storeys).
pub(crate) fn storey_wall_corners(hs: &HomeStructure, k: usize) -> Vec<(f32, f32)> {
    let storeys = hs.storeys();
    let mut out: Vec<(f32, f32)> = Vec::new();
    for wall in hs.walls.iter().filter(|w| wall_on(&storeys, w, k)) {
        for c in [wall.a, wall.b] {
            if !out.iter().any(|o| same_corner(*o, c)) {
                out.push(c);
            }
        }
    }
    out
}

/// Move the corner at `from` to `to` on storey `k`: every end of a wall on that storey at `from`
/// moves, so walls sharing the corner move together, and a wall of another storey under or over
/// it stays. True when one moved.
pub(crate) fn move_storey_corner(hs: &mut HomeStructure, k: usize, from: (f32, f32), to: (f32, f32)) -> bool {
    let storeys = hs.storeys();
    let mut moved = false;
    for wall in hs.walls.iter_mut().filter(|w| wall_on(&storeys, w, k)) {
        if same_corner(wall.a, from) {
            wall.a = to;
            moved = true;
        }
        if same_corner(wall.b, from) {
            wall.b = to;
            moved = true;
        }
    }
    moved
}

/// The first wall on storey `k` with an end at corner `c`: the wall a tap on that corner selects.
pub(crate) fn storey_wall_at_corner(hs: &HomeStructure, k: usize, c: (f32, f32)) -> Option<usize> {
    let storeys = hs.storeys();
    hs.walls.iter().position(|w| wall_on(&storeys, w, k) && (same_corner(w.a, c) || same_corner(w.b, c)))
}

/// `opening_gizmos` for the walls standing in storey `k` (`wall_in_storey`): the door and window
/// handles the editor shows and grabs on that storey.
pub(crate) fn opening_gizmos_on_storey(hs: &HomeStructure, zo: Vec3, k: usize) -> Vec<((usize, usize), Vec3)> {
    let storeys = hs.storeys();
    super::opening_gizmos(hs, zo)
        .into_iter()
        .filter(|((wi, _), _)| hs.walls.get(*wi).is_some_and(|w| wall_in_storey(&storeys, w, k)))
        .collect()
}

/// `opening_resize_handles` for the walls standing in storey `k`.
pub(crate) fn opening_resize_handles_on_storey(hs: &HomeStructure, zo: Vec3, k: usize) -> Vec<((usize, usize, u8), Vec3)> {
    let storeys = hs.storeys();
    super::opening_resize_handles(hs, zo)
        .into_iter()
        .filter(|((wi, _, _), _)| hs.walls.get(*wi).is_some_and(|w| wall_in_storey(&storeys, w, k)))
        .collect()
}

/// Where a new floor drawn from storey `es` goes, its top (body-local), or why it cannot be drawn:
/// - `Above`: the floor of the storey above, at that storey's floor when there is one (the new
///   slab is more of it), else `NEW_STOREY_M` up, if that leaves a person room to stand
///   (`ROOM_DIVIDER_MIN_M`) under the roof. The body's size is its plot's, so the editor does not
///   raise it.
/// - `Here`: more of the chosen storey's own floor. The ground storey's floor is the home's own,
///   everywhere, so there is none to add there.
pub(crate) fn new_slab_top(es: EditStorey, draw: SlabDraw) -> Result<f32, String> {
    match draw {
        SlabDraw::Here if es.index == 0 => Err(
            "The ground storey's floor is the home's own floor, everywhere: draw a floor above it instead.".to_string(),
        ),
        SlabDraw::Here => Ok(es.storey.floor),
        SlabDraw::Above if es.has_above() => Ok(es.storey.top),
        SlabDraw::Above => {
            let top = es.storey.floor + NEW_STOREY_M;
            let roof = es.storey.top; // the top storey ends at the body's top
            if roof - top >= ROOM_DIVIDER_MIN_M - STOREY_EPS {
                Ok(top)
            } else {
                Err(format!(
                    "No room for a storey above {}: the home is {roof:.1} m tall, and a floor {NEW_STOREY_M:.1} m up with \
                     {ROOM_DIVIDER_MIN_M:.1} m to stand in over it needs {:.1} m. A home's size is its plot's.",
                    storey_name(es.index).to_lowercase(),
                    top + ROOM_DIVIDER_MIN_M,
                ))
            }
        }
    }
}

/// ADD A FLOOR SLAB drawn from corner `a` to the opposite corner `b` (body-local, either way
/// round, kept inside the body) on storey `es`, at the height `new_slab_top` gives `draw`. Its
/// thickness is its floor's (a floor drawn in pieces is one floor), else `NEW_SLAB_THICKNESS_M`;
/// it is solid, and unnamed. A new top makes a new storey (storeys are derived). Ok(its index), or
/// why nothing was added.
pub(crate) fn add_floor_slab(hs: &mut HomeStructure, es: EditStorey, draw: SlabDraw, a: (f32, f32), b: (f32, f32)) -> Result<usize, String> {
    let top = new_slab_top(es, draw)?;
    let (w, d) = (hs.width, hs.depth);
    let (x0, x1) = (a.0.min(b.0).clamp(0.0, w), a.0.max(b.0).clamp(0.0, w));
    let (z0, z1) = (a.1.min(b.1).clamp(0.0, d), a.1.max(b.1).clamp(0.0, d));
    if x1 - x0 < MIN_SLAB_SIDE_M || z1 - z0 < MIN_SLAB_SIDE_M {
        return Err(format!(
            "A floor needs at least {MIN_SLAB_SIDE_M:.2} m on each side inside the home: click two opposite corners further apart."
        ));
    }
    let thickness = hs.floors.iter().find(|s| (s.top - top).abs() < STOREY_EPS).map_or(NEW_SLAB_THICKNESS_M, |s| s.thickness);
    hs.floors.push(FloorSlab { a: (x0, z0), b: (x1, z1), top, thickness, material: SlabMaterial::Solid, label: None });
    Ok(hs.floors.len() - 1)
}

/// REMOVE floor slab `i`, giving it back (None when there is none). A storey whose last slab it
/// was is gone with it (storeys are derived); what stood on it stays where it was.
pub(crate) fn remove_floor_slab(hs: &mut HomeStructure, i: usize) -> Option<FloorSlab> {
    (i < hs.floors.len()).then(|| hs.floors.remove(i))
}

/// What stands on the storey floor slab `i` is the floor of, when it is that floor's LAST slab
/// (removing it would leave them in the air): (walls, zones, pieces). (0, 0, 0) otherwise.
pub(crate) fn left_standing_without(hs: &HomeStructure, i: usize) -> (usize, usize, usize) {
    let Some(s) = hs.floors.get(i) else { return (0, 0, 0) };
    let storeys = hs.storeys();
    // The storey above the ground this slab is a floor of; a slab at the ground or the roof
    // starts none, and holds nothing up.
    let Some(k) = storeys.iter().position(|st| s.is_floor_of(st)).filter(|k| *k > 0) else {
        return (0, 0, 0);
    };
    if hs.floors.iter().enumerate().any(|(j, o)| j != i && o.is_floor_of(&storeys[k])) {
        return (0, 0, 0); // another slab of the same floor is still there
    }
    let on = |y: f32| storey_index(&storeys, y) == k;
    let walls = hs.walls.iter().filter(|w| on(w.base)).count();
    let zones = hs.zones.iter().filter(|z| on(z.origin.1)).count();
    let pieces = hs.structures.iter().filter(|p| on(p.pos.1)).count();
    (walls, zones, pieces)
}

/// Corner `c` (0..4) of a slab's footprint, round it from its smallest x and z:
/// (x0, z0), (x1, z0), (x1, z1), (x0, z1).
pub(crate) fn slab_corner(s: &FloorSlab, c: u8) -> (f32, f32) {
    let (x0, z0, x1, z1) = s.rect();
    match c % 4 {
        0 => (x0, z0),
        1 => (x1, z0),
        2 => (x1, z1),
        _ => (x0, z1),
    }
}

/// Drag corner `c` of a slab to `to`, the opposite corner staying put; the footprint is written
/// smallest corner first. Returns which corner the dragged point is now (it changes when the drag
/// crosses the opposite corner), so the grab follows it.
pub(crate) fn move_slab_corner(s: &mut FloorSlab, c: u8, to: (f32, f32)) -> u8 {
    let keep = slab_corner(s, (c + 2) % 4);
    s.a = (to.0.min(keep.0), to.1.min(keep.1));
    s.b = (to.0.max(keep.0), to.1.max(keep.1));
    match (to.0 > keep.0, to.1 > keep.1) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }
}

/// THE CUT-AWAY of body `hs` while storey `k` is edited: a copy with everything above that
/// storey taken out, so it can be seen and edited from above. Out go the floors of the storeys
/// above (the slabs whose top is over storey k's floor), the walls standing on them, the pieces,
/// lights and zones up there; a wall rising through the cut is cut at it, with any opening that
/// would be over the cut. The shell keeps its full height, so there is no lid at the cut. None
/// when nothing is above storey `k` (or there is no storey `k`).
pub(crate) fn cut_away_body(hs: &HomeStructure, k: usize) -> Option<HomeStructure> {
    let storeys = hs.storeys();
    if k + 1 >= storeys.len() {
        return None;
    }
    let (floor, cut) = (storeys[k].floor, storeys[k].top);
    let mut out = hs.clone();
    out.floors.retain(|s| s.top < floor + STOREY_EPS);
    out.walls.retain(|w| w.base < cut - STOREY_EPS);
    for w in out.walls.iter_mut() {
        let under = cut - w.base;
        if w.height > under {
            w.height = under;
            w.openings.retain(|o| o.sill < under - STOREY_EPS);
            for o in w.openings.iter_mut() {
                o.height = o.height.min(under - o.sill);
            }
        }
    }
    out.structures.retain(|p| p.pos.1 < cut - STOREY_EPS);
    out.lights.retain(|l| l.pos.1 < cut - STOREY_EPS);
    out.zones.retain(|z| z.origin.1 < cut - STOREY_EPS);
    Some(out)
}

/// `cut_away_body` for zone `zone` of the ship, the rest of the ship as it is. None when nothing is
/// above storey `k` of that zone.
pub(crate) fn cut_away_ship(ship: &ShipStructure, zone: usize, k: usize) -> Option<ShipStructure> {
    let body = cut_away_body(&ship.zones.get(zone)?.body, k)?;
    let mut out = ship.clone();
    out.zones[zone].body = body;
    Some(out)
}

/// Which cut-away the editor should be drawing now: (zone, storey) while it is open on a storey
/// with others above, else None (the whole ship).
pub(crate) fn storey_cut_key(state: &EngineState) -> Option<(usize, usize)> {
    if !state.gui_state.construction_active {
        return None;
    }
    let es = edit_storey(state)?;
    es.has_above().then_some((state.gui_state.construction_zone, es.index))
}

/// The ship the editor should DRAW (engine/home_meshes.rs `rebuild_homestead` asks): the cut-away
/// of the storey being edited, or None to draw the ship as it is.
pub(crate) fn editor_cut_away(state: &EngineState) -> Option<ShipStructure> {
    let (zone, k) = storey_cut_key(state)?;
    cut_away_ship(state.gui_state.ship_structure.as_ref()?, zone, k)
}

/// How far outside the cut zone's box a thing may stand and still be cut with it (metres): a door
/// in the shell stands on the box's edge, and the port gizmos of a unit against the shell
/// (`home_meshes::port_gizmo_pos`, a ring round the machine) and the risers to them stand just past
/// it (the study's uplink and router, x 55.05 on a 55 m box).
pub(crate) const CUT_EDGE_M: f32 = 0.3;

/// THE CUT-AWAY FOR WHAT IS NOT IN THE BODY (ship homes increment 7 review): the home's machines,
/// their screens, its door panels and its elevator cars are drawn from the whole ship, not from
/// the cut body, so they follow the cut by where they stand. World metres: the cut zone's footprint
/// and the cut's height, the top of the storey being edited.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CutAway {
    pub(crate) min: (f32, f32),
    pub(crate) max: (f32, f32),
    pub(crate) cut_y: f32,
}

impl CutAway {
    /// True when the cut-away takes out a thing standing at world point `at` (its foot: a machine's
    /// placement, a door panel's bottom, an elevator car's floor): in the cut zone's box and at or
    /// over the cut, the rule `cut_away_body` keeps the body's pieces and lights by.
    pub(crate) fn hides(&self, at: Vec3) -> bool {
        at.y >= self.cut_y - STOREY_EPS
            && at.x >= self.min.0 - CUT_EDGE_M
            && at.x <= self.max.0 + CUT_EDGE_M
            && at.z >= self.min.1 - CUT_EDGE_M
            && at.z <= self.max.1 + CUT_EDGE_M
    }
}

/// The cut-away of storey `k` of zone `zone` (`CutAway`): None when nothing is above that storey,
/// as `cut_away_body`.
pub(crate) fn cut_away_of(ship: &ShipStructure, zone: usize, k: usize) -> Option<CutAway> {
    let z = ship.zones.get(zone)?;
    let storeys = z.body.storeys();
    if k + 1 >= storeys.len() {
        return None;
    }
    let o = z.origin_vec();
    Some(CutAway { min: (o.x, o.z), max: (o.x + z.body.width, o.z + z.body.depth), cut_y: o.y + storeys[k].top })
}

/// The cut-away the editor should draw now (`storey_cut_key`), as `CutAway`; None for the whole
/// ship. engine/home_meshes.rs `rebuild_homestead` keeps it with the meshes it draws.
pub(crate) fn editor_cut(state: &EngineState) -> Option<CutAway> {
    let (zone, k) = storey_cut_key(state)?;
    cut_away_of(state.gui_state.ship_structure.as_ref()?, zone, k)
}

/// Per frame (from `construction_history_tick`): when the cut-away that should be drawn is not
/// the one that was, redraw the home. Choosing a storey, another zone, adding the slab that makes
/// a storey above, or closing the editor all come here. The redraw is called directly, not through
/// the dirty flags, so it is no edit: it arms no autosave and makes no undo step (the way
/// `editor_restore` rebuilds); and when an edit's own rebuild is due this frame, that one draws it.
pub(crate) fn follow_storey_view(state: &mut EngineState) {
    let want = storey_cut_key(state);
    if state.gui_state.construction_storeys.drawn == want {
        return;
    }
    state.gui_state.construction_storeys.drawn = want;
    if !state.gui_state.construction_structure_dirty && state.gui_state.ship_structure.is_some() {
        rebuild_homestead(state);
    }
}

/// Work on storey `k` of the edited body (kept in range). Drops a half-drawn slab's or wall's
/// first corner, which was on the old storey's plan.
pub(crate) fn choose_storey(g: &mut GuiState, k: usize) {
    let count = zone_body(&g.ship_structure, g.construction_zone).map_or(1, |hs| hs.storeys().len());
    let k = k.min(count.saturating_sub(1));
    if k != g.construction_storeys.chosen {
        g.construction_storeys.chosen = k;
        g.construction_storeys.draw_start = None;
        g.construction_wall_start = None;
    }
}

/// Let go of every selection in the editor but the slab's (a slab selected takes the right panel
/// alone).
fn clear_other_selections(g: &mut GuiState) {
    g.construction_wall_selected = None;
    g.construction_structure_selected = None;
    g.construction_light_selected = None;
    g.construction_machine_selected = None;
    g.construction_road_node_selected = None;
    g.construction_conduit_node_selected = None;
    g.construction_connection_selected = None;
    g.construction_zone_selected = None;
}

/// SELECT floor slab `i` of the edited body (its detail fills the right panel) and work on the
/// storey it is the floor of, so it is in view and its corners are on the plan.
pub(crate) fn select_slab(g: &mut GuiState, i: usize) {
    let Some(k) = zone_body(&g.ship_structure, g.construction_zone).and_then(|hs| hs.floors.get(i).map(|s| hs.storey_of_y(s.top))) else {
        return;
    };
    clear_other_selections(g);
    g.construction_storeys.slab_selected = Some(i);
    g.construction_storeys.confirm_remove = None;
    choose_storey(g, k);
}

/// REMOVE floor slab `i` of the edited body (the panel's Remove, the object list's x): it is gone,
/// nothing is selected, and the chosen storey stays in range (its storey may have gone with it).
pub(crate) fn remove_slab_in_editor(g: &mut GuiState, i: usize) {
    if zone_body_mut(&mut g.ship_structure, g.construction_zone).and_then(|hs| remove_floor_slab(hs, i)).is_some() {
        g.construction_structure_dirty = true;
    }
    g.construction_storeys.slab_selected = None;
    g.construction_storeys.confirm_remove = None;
    let k = g.construction_storeys.chosen;
    choose_storey(g, k);
    // `choose_storey` changes nothing when the index is still in range; the storey under it may
    // still have gone, so clamp the stored index too.
    let count = zone_body(&g.ship_structure, g.construction_zone).map_or(1, |hs| hs.storeys().len());
    g.construction_storeys.chosen = g.construction_storeys.chosen.min(count.saturating_sub(1));
}

/// Turn the floor-slab drawing tool on (`draw`) or off (None). It is a shape of the drawing tool,
/// so the wall tool's mode comes on and off with it, and a half-drawn wall is dropped.
pub(crate) fn set_slab_tool(g: &mut GuiState, draw: Option<SlabDraw>) {
    g.construction_storeys.draw = draw;
    g.construction_storeys.draw_start = None;
    g.construction_storeys.note.clear();
    g.construction_wall_start = None;
    g.construction_wall_mode = draw.is_some();
    g.construction_wall_guard = false;
    if draw.is_some() {
        // The drawing tool takes the click from every held placement (the press chain in lib.rs).
        g.construction_place_type = None;
        g.construction_place_light = None;
        g.construction_structure_type = None;
        g.construction_place_conduit_node = false;
    }
}

/// A click of the FLOOR-SLAB TOOL (`try_place_wall_node` hands the click here while the tool is
/// on): the first sets one corner on the chosen storey's plan, snapped the way wall corners are;
/// the second the opposite corner, and the slab is added (`add_floor_slab`) and selected. The
/// selector follows it onto the storey it is the floor of, and a floor drawn "above" goes on as
/// more of that floor. Why a floor could not be drawn is said in a notice and under the tool.
pub(crate) fn try_place_slab_corner(state: &mut EngineState) {
    let Some(draw) = state.gui_state.construction_storeys.draw else {
        return;
    };
    let Some(hit) = cursor_storey_hit(state) else {
        return;
    };
    let zo = active_zone_origin(state);
    let grid = state.gui_state.construction_grid_snap;
    let g = &mut state.gui_state;
    let Some(hs) = zone_body(&g.ship_structure, g.construction_zone) else {
        return;
    };
    let p = snap_node_position(hs, (f32::NAN, f32::NAN), (hit.x - zo.x, hit.z - zo.z), grid);
    let es = EditStorey::of(hs, g.construction_storeys.chosen);
    let Some(start) = g.construction_storeys.draw_start else {
        g.construction_storeys.draw_start = Some(p);
        return;
    };
    if (start.0 - p.0).abs() < 0.05 && (start.1 - p.1).abs() < 0.05 {
        return; // a second click on the first corner (a double-click) draws nothing
    }
    let Some(hs) = zone_body_mut(&mut g.ship_structure, g.construction_zone) else {
        return;
    };
    match add_floor_slab(hs, es, draw, start, p) {
        Ok(i) => {
            g.construction_storeys.draw_start = None;
            g.construction_storeys.note.clear();
            g.construction_structure_dirty = true;
            select_slab(g, i);
            if draw == SlabDraw::Above {
                g.construction_storeys.draw = Some(SlabDraw::Here);
            }
        }
        Err(why) => {
            g.construction_storeys.draw_start = None;
            g.construction_storeys.note = why.clone();
            g.pending_notices.push(why);
        }
    }
}

/// True when the floor-slab type is locked or hidden in the object list (its picks and handles
/// are off, like every other type's).
fn slabs_blocked(g: &GuiState) -> bool {
    g.construction_locked_types.contains("Floor") || g.construction_hidden_types.contains("Floor")
}

/// The SELECTED slab's four corner handles, (corner, world position at its top): for the view to
/// draw and `try_grab_slab_corner` to grab. Empty when no slab is selected, it is not drawn in
/// the cut-away, or slabs are locked or hidden.
pub(crate) fn slab_corner_handles(state: &EngineState) -> Vec<(u8, Vec3)> {
    let g = &state.gui_state;
    let Some(i) = g.construction_storeys.slab_selected else {
        return Vec::new();
    };
    let Some(hs) = zone_body(&g.ship_structure, g.construction_zone) else {
        return Vec::new();
    };
    let Some(s) = hs.floors.get(i) else {
        return Vec::new();
    };
    if slabs_blocked(g) || !EditStorey::of(hs, g.construction_storeys.chosen).shows(s.top) {
        return Vec::new();
    }
    let zo = active_zone_origin(state);
    (0..4u8)
        .map(|c| {
            let (x, z) = slab_corner(s, c);
            (c, Vec3::new(x + zo.x, zo.y + s.top, z + zo.z))
        })
        .collect()
}

/// On a press, before the wall corners (`try_grab_node` asks first): grab the selected slab's
/// corner handle under the cursor. The drag rides the corner-node grab (`construction_node_grab`,
/// whose per-frame `apply_node_drag` hands it to `apply_slab_corner_drag`), with a tap-or-drag
/// threshold of its own, so a tap on it does not select a wall that shares the corner.
pub(crate) fn try_grab_slab_corner(state: &mut EngineState) -> bool {
    state.gui_state.construction_storeys.corner_grab = None;
    let handles = slab_corner_handles(state);
    let Some(i) = state.gui_state.construction_storeys.slab_selected.filter(|_| !handles.is_empty()) else {
        return false;
    };
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(u8, Vec3, f32)> = None;
    for (c, p) in handles {
        let t = (p - origin).dot(dir);
        if t < 0.0 {
            continue; // behind the camera
        }
        let dd = (p - (origin + dir * t)).length();
        if dd < SLAB_CORNER_PICK_M && best.map_or(true, |(_, _, bd)| dd < bd) {
            best = Some((c, p, dd));
        }
    }
    let Some((c, p, _)) = best else {
        return false;
    };
    let zo = active_zone_origin(state);
    state.gui_state.construction_storeys.corner_grab = Some((i, c));
    state.gui_state.construction_storeys.press = Some(state.cursor_pos);
    state.construction_node_grab = Some((p.x - zo.x, p.z - zo.z));
    state.construction_grab_press = None;
    true
}

/// Per frame while a slab corner is grabbed (`apply_node_drag` hands it here first): past the
/// drag threshold, the corner follows the cursor on the storey's plan, snapped the way wall
/// corners are, the opposite corner staying put. True when a slab corner is grabbed (handled).
pub(crate) fn apply_slab_corner_drag(state: &mut EngineState) -> bool {
    let Some((i, c)) = state.gui_state.construction_storeys.corner_grab else {
        return false;
    };
    if let Some(press) = state.gui_state.construction_storeys.press {
        let d = ((state.cursor_pos.0 - press.0).powi(2) + (state.cursor_pos.1 - press.1).powi(2)).sqrt();
        if d < DRAG_THRESHOLD_PX {
            return true; // still a tap: nothing moves
        }
        state.gui_state.construction_storeys.press = None;
    }
    let Some(hit) = cursor_storey_hit(state) else {
        return true;
    };
    let zo = active_zone_origin(state);
    let grid = state.gui_state.construction_grid_snap;
    let g = &mut state.gui_state;
    let Some(p) = zone_body(&g.ship_structure, g.construction_zone).map(|hs| {
        let (w, d) = (hs.width, hs.depth);
        let p = snap_node_position(hs, (f32::NAN, f32::NAN), (hit.x - zo.x, hit.z - zo.z), grid);
        (p.0.clamp(0.0, w), p.1.clamp(0.0, d))
    }) else {
        return true;
    };
    if let Some(s) = zone_body_mut(&mut g.ship_structure, g.construction_zone).and_then(|hs| hs.floors.get_mut(i)) {
        if slab_corner(s, c) != p {
            let now = move_slab_corner(s, c, p);
            g.construction_storeys.corner_grab = Some((i, now));
            g.construction_structure_dirty = true;
        }
    }
    true
}

/// On a press: select the floor slab whose OUTLINE the cursor is on (within 0.3 m of it, on its
/// top's plane), of the slabs drawn in the cut-away; nearest to the camera wins. Edges only, so a
/// click on a floor still reaches the zone or the room it is in. True on a hit.
pub(crate) fn try_pick_slab(state: &mut EngineState) -> bool {
    if slabs_blocked(&state.gui_state) {
        return false;
    }
    let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) else {
        return false;
    };
    let es = EditStorey::of(hs, state.gui_state.construction_storeys.chosen);
    let zo = active_zone_origin(state);
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    if dir.y.abs() < 1e-6 {
        return false;
    }
    let mut best: Option<(usize, f32)> = None;
    for (i, s) in hs.floors.iter().enumerate().filter(|(_, s)| es.shows(s.top)) {
        let t = (zo.y + s.top - origin.y) / dir.y;
        if t <= 0.0 {
            continue;
        }
        let (hx, hz) = (origin.x + dir.x * t - zo.x, origin.z + dir.z * t - zo.z);
        let (x0, z0, x1, z1) = s.rect();
        let e = SLAB_EDGE_PICK_M;
        let near_box = hx >= x0 - e && hx <= x1 + e && hz >= z0 - e && hz <= z1 + e;
        let deep = hx > x0 + e && hx < x1 - e && hz > z0 + e && hz < z1 - e;
        if near_box && !deep && best.map_or(true, |(_, bt)| t < bt) {
            best = Some((i, t));
        }
    }
    let Some((i, _)) = best else {
        return false;
    };
    select_slab(&mut state.gui_state, i);
    true
}

/// The outline of every floor slab drawn in the cut-away (world corners round its top) and
/// whether it is the selected one: for the view's helper lines.
pub(crate) fn slab_outlines(state: &EngineState) -> Vec<([Vec3; 4], bool)> {
    let g = &state.gui_state;
    let Some(hs) = zone_body(&g.ship_structure, g.construction_zone) else {
        return Vec::new();
    };
    if g.construction_hidden_types.contains("Floor") {
        return Vec::new();
    }
    let es = EditStorey::of(hs, g.construction_storeys.chosen);
    let zo = active_zone_origin(state);
    hs.floors
        .iter()
        .enumerate()
        .filter(|(_, s)| es.shows(s.top))
        .map(|(i, s)| {
            let at = |c: u8| {
                let (x, z) = slab_corner(s, c);
                Vec3::new(x + zo.x, zo.y + s.top, z + zo.z)
            };
            ([at(0), at(1), at(2), at(3)], g.construction_storeys.slab_selected == Some(i))
        })
        .collect()
}

/// The slab being drawn, for the view's preview: the rectangle from its first corner to the
/// cursor on the storey's plan (world, round it), and the world height the slab will go at.
/// None while no first corner is down.
pub(crate) fn slab_draw_preview(state: &EngineState) -> Option<([Vec3; 4], f32)> {
    let g = &state.gui_state;
    let (draw, start) = (g.construction_storeys.draw?, g.construction_storeys.draw_start?);
    let hs = zone_body(&g.ship_structure, g.construction_zone)?;
    let hit = cursor_storey_hit(state)?;
    let zo = active_zone_origin(state);
    let top = new_slab_top(EditStorey::of(hs, g.construction_storeys.chosen), draw).ok()?;
    let (sx, sz) = (start.0 + zo.x, start.1 + zo.z);
    let y = hit.y;
    Some(([Vec3::new(sx, y, sz), Vec3::new(hit.x, y, sz), Vec3::new(hit.x, y, hit.z), Vec3::new(sx, y, hit.z)], zo.y + top))
}

#[cfg(test)]
#[path = "editor_storeys_tests.rs"]
mod tests;

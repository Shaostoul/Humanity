//! WHAT A STOREY EDIT DOES TO A HOME (ship homes increment 7,
//! docs/design/ship-homes-increment-7-plan.md, "Editor (C2)"), with no editor around it: which
//! storey of a body is edited (`EditStorey`), the names the panel shows, a new wall or guard
//! standing on a storey, where a new floor slab goes and adding or removing one, what a removal
//! would leave standing, and the box the cut-away hides things by (`CutAway`). Pure on the ship's
//! own types, so the build editor's three halves share it: the view's picks and drags
//! (engine/editor_storeys.rs), the panel and its choices (gui/pages/construction/storeys.rs), and
//! the console's walls (gui/pages/construction.rs). The ship's since the crate split's rung 0, part
//! B (2026-10-31): it was in engine/editor_storeys.rs and `SlabDraw` on the page, so the page
//! reached up into the engine for every name and edit.

use glam::Vec3;

use crate::ship::home_structure::{
    storey_index, FloorSlab, HomeStructure, InteriorWall, SlabMaterial, Storey, ROOM_DIVIDER_MIN_M, STOREY_EPS,
};
use crate::ship::ship_structure::ShipStructure;

/// Which floor the slab drawing tool draws (the page's `StoreyEditor::draw`).
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

/// Floor to floor of a storey drawn over the top one (metres): 3.0, the increment's storey (plan
/// section 4, decision 2: a 0.25 m slab leaves 2.75 m of room under the next floor).
pub(crate) const NEW_STOREY_M: f32 = 3.0;
/// A new slab's thickness when no slab of its floor says otherwise (metres): 0.25, what a slab's
/// file means when it leaves its thickness out (`FloorSlab::thickness`).
pub(crate) const NEW_SLAB_THICKNESS_M: f32 = 0.25;
/// The shortest side a drawn slab may have (metres): two clicks closer than this on either axis
/// are a slip of the hand, not a floor.
pub(crate) const MIN_SLAB_SIDE_M: f32 = 0.25;

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

/// Storey `k`'s name as the panel shows it, counted from 1 as people count floors in a plan:
/// "Storey 1 (ground)", "Storey 2", ...
pub(crate) fn storey_name(k: usize) -> String {
    // One naming rule with the Maps page's ship levels (ship/levels.rs, Maps increment 3).
    crate::ship::levels::level_name(k as i32)
}

/// A floor slab's name: its label, else "Floor slab N" (counted from 1).
pub(crate) fn slab_name(i: usize, s: &FloorSlab) -> String {
    match s.label.as_deref().map(str::trim) {
        Some(l) if !l.is_empty() => l.to_string(),
        _ => format!("Floor slab {}", i + 1),
    }
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

/// How far outside the cut zone's box a thing may stand and still be cut with it (metres): a door
/// in the shell stands on the box's edge. (The port gizmos of a unit against the shell, and the
/// risers to them, stood just past it too, the study's uplink and router at x 55.05 on a 55 m box,
/// until the 2026-10-21 follow-ups kept every gizmo inside its home, `home_meshes::gizmo_room`.)
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

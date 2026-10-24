//! The LEVELS of a ship (Maps increment 3, "into the ship, deck by deck",
//! docs/design/maps-zoom-to-anything.md section 5.7): the floors a person can be on aboard, from
//! the bodies standing in it, so the Maps page's level picker, its cutaway, its room plan and its
//! plan tiles all read one list.
//!
//! WHERE THEY COME FROM (infinite-of-x: no level is written in code). Every body standing in the
//! ship (each zone, and each neighbour's home as the game draws it: its kind's shipped design on
//! its plot, ship/neighbours.rs) has its STOREYS (`HomeStructure::storeys`, derived from its floor
//! slabs, ship homes increment 7). A level of the ship is one distinct storey floor among all of
//! them, in ship metres; it ends at the next one up, or at the highest top. The shipped ship has
//! two: the deck every zone stands on, and the homesteads' upper floor 3 m over it.
//!
//! NAMES. A level's stacking position is its ORDINAL, as the Indoor Mapping Data Format has it
//! ("The Level feature's 'stacking' position, relative to others", OGC 20-094): 0 is the level the
//! ship's deck datum (y = 0, "the deck height" of the ship file) is on, 1 the one over it, -1 the
//! one under. Its name is the build editor's storey name for the same count ("Storey 1 (ground)",
//! "Storey 2", `level_name`, which engine/editor_storeys.rs `storey_name` calls), and under the
//! ground "Basement 1".
//!
//! THE CUT AT A LEVEL. Seeing a level means cutting away what is above it, body by body, by the
//! one rule the build editor cuts a storey by (`HomeStructure::cut_away`): each body is cut at the
//! storey the level's floor is on, so a two-storey home on the ground level loses its upper floor
//! and a one-storey zone (the Commons, 8 m to its glass roof) stays whole. `level_cuts` says which
//! boxes are cut and at what height (what stands in them over that height is left out: machines,
//! doors, plants, the engine's `CutAway`), and `level_meshes` draws the ship that way.
//!
//! Limits, said plainly: a body standing wholly ABOVE the level shown (none does on the shipped
//! ship) is drawn whole, as a one-storey zone is; and the rooms are the ship's own detected rooms
//! (`detect_rooms`), so a neighbour's home, drawn as the shipped design and never walked as a room,
//! has no room names on the map.

use glam::Vec3;

use crate::ship::fibonacci::HomesteadMeshes;
use crate::ship::home_structure::{storey_index, HomeStructure, STOREY_EPS};
use crate::ship::ship_structure::ShipStructure;

/// One level of a ship: its stacking position and its band of height, ship metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShipLevel {
    /// 0 the level the deck datum (y = 0) is on, counting up from it, negative below it.
    pub ordinal: i32,
    /// Its floor, and where it ends: the next level's floor, or the highest top in the ship.
    pub floor: f32,
    pub top: f32,
}

/// A level's name by its ordinal: "Storey 1 (ground)", "Storey 2", ..., "Basement 1" under the
/// ground. The build editor names a home's storeys by the same rule (`storey_name`).
pub fn level_name(ordinal: i32) -> String {
    match ordinal {
        0 => "Storey 1 (ground)".to_string(),
        n if n > 0 => format!("Storey {}", n + 1),
        n => format!("Basement {}", -n),
    }
}

impl ShipLevel {
    /// The level's name (`level_name`).
    pub fn name(&self) -> String {
        level_name(self.ordinal)
    }

    /// The short name the level picker shows on its button, as a lift's buttons read: "1", "2",
    /// "B1" (IMDF's `short_name`).
    pub fn short_name(&self) -> String {
        if self.ordinal >= 0 {
            (self.ordinal + 1).to_string()
        } else {
            format!("B{}", -self.ordinal)
        }
    }
}

/// Every body standing in the ship, with the corner its body is measured from (ship metres): each
/// zone but a home put away, then each neighbour's home as the game draws it (its kind's shipped
/// design on its plot, ship/neighbours.rs).
pub fn standing_bodies(ship: &ShipStructure) -> Vec<(Vec3, &HomeStructure)> {
    let mut out: Vec<(Vec3, &HomeStructure)> = ship
        .zones
        .iter()
        .enumerate()
        .filter(|(zi, _)| !ship.is_away_home(*zi))
        .map(|(_, z)| (z.origin_vec(), &z.body))
        .collect();
    let view = crate::ship::neighbours::neighbour_view(ship);
    for n in &view.neighbours {
        let design: &'static crate::ship::ship_structure::HomeDesign = n.design;
        out.push((Vec3::new(n.plot.origin.0, n.plot.origin.1, n.plot.origin.2), &design.body));
    }
    out
}

/// The ship's levels, bottom up (see the module doc). Empty for a ship with nothing in it.
pub fn ship_levels(ship: &ShipStructure) -> Vec<ShipLevel> {
    levels_of(&standing_bodies(ship))
}

/// `ship_levels` over a list of bodies (pure, so a test builds its own).
pub fn levels_of(bodies: &[(Vec3, &HomeStructure)]) -> Vec<ShipLevel> {
    let mut floors: Vec<f32> = Vec::new();
    let mut top = f32::NEG_INFINITY;
    for (o, b) in bodies {
        let storeys = b.storeys();
        floors.extend(storeys.iter().map(|s| o.y + s.floor));
        top = top.max(o.y + storeys.last().map_or(b.height, |s| s.top));
    }
    floors.sort_by(f32::total_cmp);
    floors.dedup_by(|later, kept| (*later - *kept).abs() < STOREY_EPS);
    let ground = floors.iter().rposition(|f| *f <= STOREY_EPS).unwrap_or(0) as i32;
    floors
        .iter()
        .enumerate()
        .map(|(i, &floor)| ShipLevel {
            ordinal: i as i32 - ground,
            floor,
            top: floors.get(i + 1).copied().unwrap_or_else(|| top.max(floor + 1.0)),
        })
        .collect()
}

/// Which of `levels` (an index) a height `y` (ship metres) is on: the highest whose floor is at
/// or under it, the rule a body's storey is found by (`storey_index`); under the lowest, the
/// lowest. None with no levels.
pub fn level_at(levels: &[ShipLevel], y: f32) -> Option<usize> {
    if levels.is_empty() {
        return None;
    }
    Some(levels.iter().rposition(|l| l.floor <= y + STOREY_EPS).unwrap_or(0))
}

/// The level with this ordinal, if the ship has one.
pub fn by_ordinal(levels: &[ShipLevel], ordinal: i32) -> Option<ShipLevel> {
    levels.iter().copied().find(|l| l.ordinal == ordinal)
}

/// One body's cut at a level: its footprint (ship metres, x and z) and the height from which
/// what stands in it is cut away (the top of its storey on that level).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelCut {
    pub min: (f32, f32),
    pub max: (f32, f32),
    pub cut_y: f32,
}

/// The storey of body `b` (its corner at `o`) that level `level` is on, when something of the
/// body is above it (so `HomeStructure::cut_away` cuts it there), else None.
fn storey_cut(o: Vec3, b: &HomeStructure, level: &ShipLevel) -> Option<usize> {
    let k = storey_index(&b.storeys(), level.floor - o.y);
    (k + 1 < b.storeys().len()).then_some(k)
}

/// Every body cut at `level` (see the module doc): one `LevelCut` per body with a storey above
/// the level's. Empty when nothing in the ship is above it (its top level, or a ship of one
/// storey everywhere): the whole ship is the view.
pub fn level_cuts(ship: &ShipStructure, level: &ShipLevel) -> Vec<LevelCut> {
    cuts_of(&standing_bodies(ship), level)
}

/// `level_cuts` over a list of bodies (pure).
pub fn cuts_of(bodies: &[(Vec3, &HomeStructure)], level: &ShipLevel) -> Vec<LevelCut> {
    bodies
        .iter()
        .filter_map(|(o, b)| {
            let k = storey_cut(*o, b, level)?;
            Some(LevelCut { min: (o.x, o.z), max: (o.x + b.width, o.z + b.depth), cut_y: o.y + b.storeys()[k].top })
        })
        .collect()
}

/// The ship's shell drawn at `level`: every zone cut at its storey on that level
/// (`HomeStructure::cut_away`) and every neighbour's home the same (`generate_meshes_at`). None
/// when nothing is cut (`level_cuts` empty): the whole ship is drawn. The rooms in it are the cut
/// ship's; a caller that keeps rooms keeps the whole ship's (engine/home_meshes.rs).
pub fn level_meshes(ship: &ShipStructure, level: &ShipLevel) -> Option<HomesteadMeshes> {
    if level_cuts(ship, level).is_empty() {
        return None;
    }
    let mut cut = ship.clone();
    for z in cut.zones.iter_mut() {
        if let Some(body) = storey_cut(z.origin_vec(), &z.body, level).and_then(|k| z.body.cut_away(k)) {
            z.body = body;
        }
    }
    Some(cut.generate_meshes_at(Some(level.floor)))
}

#[cfg(test)]
#[path = "levels_tests.rs"]
mod tests;

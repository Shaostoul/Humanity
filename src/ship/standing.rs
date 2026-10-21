//! WHERE A PERSON CAN STAND on a storey of a home (ship homes increment 7, the review of the
//! homestead's upper floor, 2026-10-21).
//!
//! The homestead has 23 floor slabs at 3.0 m: SOLID ones exactly under the upstairs rooms, GLASS
//! lids over every other ground room (docs/design/homestead-upper-floor.md section 12). Nobody may
//! stand on a glass lid: a reachable lid makes every lid a roof, and then every opening in them
//! needs a guard (section 6). Walking never reaches one (the stairs, the elevator and the ladder
//! all land inside the upstairs rooms, which walls and guards close), but a PLACEMENT can: the
//! build editor's close used to stand the player on whatever slab top was under the build-mode
//! avatar, a glass lid included, with no way off.
//!
//! So a placement asks this file. A spot on storey `k` of a body is a place to stand when:
//!   - there is at least `moves::STANDING_ROOM_M` (2.0 m) between the storey's floor and what is
//!     over the spot (the next slab's underside, else the roof);
//!   - on the GROUND storey, it is on the body's own floor, which is everywhere in its footprint,
//!     clear of the shell by `STAND_CLEAR_M`;
//!   - above the ground, it is over a SOLID slab of that storey's floor (never a glass one, never
//!     a stairwell, shaft, hatch or a room open to the roof, which are simply where no slab is),
//!     inside a ZONE of that storey (a room: a solid slab outside every room is a roof), and the
//!     whole body fits: `STAND_CLEAR_M` inside the room's edges (its walls) and over solid floor
//!     all round.
//!
//! `standing_on` says whether a spot is one and why not; `nearest_standing` finds the nearest spot
//! on the same storey that is. Pure: body-local metres, no world, no runtime state, so the game and
//! its tests ask the same question. The relay holds no copy of anyone's home and cannot ask it
//! (src/relay/handlers/move_check.rs `on_a_floor_of` says what it does instead).

use crate::ship::home_structure::{storey_index, FloorSlab, HomeStructure, STOREY_EPS};
use crate::ship::moves::STANDING_ROOM_M;

/// How far a standing spot must be from a room's edge (a wall stands there) and from the edge of
/// the floor it is on, metres: 0.45, a body's radius (0.3 m, `wall_collision::PLAYER_RADIUS`) and
/// half of a thick wall (0.15 m). A body put closer could be pushed out of the wall on its far side,
/// onto a glass lid or into a stairwell.
pub const STAND_CLEAR_M: f32 = 0.45;

/// Why a spot on a storey is no place to stand (`standing_on`).
#[derive(Debug, Clone, PartialEq)]
pub enum NoStanding {
    /// Over a glass slab of that storey's floor and no solid one: a glass roof over a room. Its
    /// label when it has one ("House roof, south half").
    GlassRoof(Option<String>),
    /// Over no slab of that storey's floor: a stairwell, an elevator shaft, a ladder hatch, a room
    /// open to the roof.
    Opening,
    /// Over a solid slab of that storey's floor but outside every room of the storey: a roof.
    OutsideRooms,
    /// Too near a wall or the edge of the floor for a body (`STAND_CLEAR_M`).
    Edge,
    /// Less than `STANDING_ROOM_M` from the storey's floor to what is over the spot.
    Low,
    /// The body has no such storey.
    NoStorey,
}

/// The storey's floor slabs: those whose top is storey `floor` (within `STOREY_EPS`).
fn floor_slabs(hs: &HomeStructure, floor: f32) -> impl Iterator<Item = &FloorSlab> {
    hs.floors.iter().filter(move |s| (s.top - floor).abs() < STOREY_EPS)
}

/// True when (x, z) is over a SOLID slab of the floor at `floor`.
fn over_solid(hs: &HomeStructure, floor: f32, x: f32, z: f32) -> bool {
    floor_slabs(hs, floor).any(|s| !s.passes_light() && s.contains(x, z))
}

/// The rooms of storey `k`, each as (min x, min z, max x, max z): the zones whose floor (origin y)
/// is on it.
fn storey_rooms(hs: &HomeStructure, k: usize) -> Vec<(f32, f32, f32, f32)> {
    let storeys = hs.storeys();
    hs.zones
        .iter()
        .filter(|z| storey_index(&storeys, z.origin.1) == k)
        .map(|z| (z.origin.0, z.origin.2, z.origin.0 + z.size.0, z.origin.2 + z.size.2))
        .collect()
}

/// Is the spot (x, z), body-local metres, on storey `k` of `hs` a place a person can stand? See
/// the top of this file for the rule. Ok, or the first reason it is not, in the order a person
/// would name it: no room to stand, a glass roof, an opening, a roof outside the rooms, too near
/// an edge.
pub fn standing_on(hs: &HomeStructure, k: usize, x: f32, z: f32) -> Result<(), NoStanding> {
    let storeys = hs.storeys();
    let storey = *storeys.get(k).ok_or(NoStanding::NoStorey)?;
    let floor = storey.floor;
    let roof = storeys.last().map_or(hs.height, |s| s.top);
    let ceiling = hs
        .floors
        .iter()
        .filter(|s| s.top > floor + STOREY_EPS && s.contains(x, z))
        .map(FloorSlab::underside)
        .fold(roof, f32::min);
    if ceiling - floor < STANDING_ROOM_M - STOREY_EPS {
        return Err(NoStanding::Low);
    }
    let c = STAND_CLEAR_M;
    if k == 0 {
        // The ground storey's floor is the body's own, everywhere in its footprint.
        let inside = x >= c && x <= hs.width - c && z >= c && z <= hs.depth - c;
        return if inside { Ok(()) } else { Err(NoStanding::Edge) };
    }
    if !over_solid(hs, floor, x, z) {
        let glass = floor_slabs(hs, floor).find(|s| s.passes_light() && s.contains(x, z));
        return Err(match glass {
            Some(g) => NoStanding::GlassRoof(g.label.clone()),
            None => NoStanding::Opening,
        });
    }
    let rooms = storey_rooms(hs, k);
    let in_room = |inset: f32| rooms.iter().any(|&(x0, z0, x1, z1)| x >= x0 + inset && x <= x1 - inset && z >= z0 + inset && z <= z1 - inset);
    if !in_room(0.0) {
        return Err(NoStanding::OutsideRooms);
    }
    if !in_room(c) {
        return Err(NoStanding::Edge);
    }
    // The whole body over solid floor: its four corners as well as its middle.
    let all_round = [(-c, -c), (c, -c), (c, c), (-c, c)].iter().all(|&(dx, dz)| over_solid(hs, floor, x + dx, z + dz));
    if all_round {
        Ok(())
    } else {
        Err(NoStanding::Edge)
    }
}

/// The NEAREST place to stand on storey `k` of `hs` to the spot (x, z), body-local metres, across
/// the floor; None when the storey has none. On the ground storey, the spot kept `STAND_CLEAR_M`
/// inside the body. Above it, the nearest point of a room of the storey that lies over one solid
/// slab of its floor, `STAND_CLEAR_M` in from both, looked for first in the room the spot is in (so
/// a spot over the stairwell in the upper hall stays in the hall, rather than going through a wall
/// into the bedroom beside it) and then in every room of the storey. Each candidate is checked with
/// `standing_on`, so a spot with too little room over it is never given. A spot that is already a
/// place to stand comes back as it is.
pub fn nearest_standing(hs: &HomeStructure, k: usize, x: f32, z: f32) -> Option<(f32, f32)> {
    if standing_on(hs, k, x, z).is_ok() {
        return Some((x, z));
    }
    let storeys = hs.storeys();
    let floor = storeys.get(k)?.floor;
    let c = STAND_CLEAR_M;
    // The nearest standing point of the given areas (each a rectangle, body-local).
    let nearest_of = |areas: &[(f32, f32, f32, f32)]| -> Option<(f32, f32)> {
        let mut best: Option<((f32, f32), f32)> = None;
        for &(x0, z0, x1, z1) in areas {
            let (x0, z0, x1, z1) = (x0 + c, z0 + c, x1 - c, z1 - c);
            if x0 > x1 || z0 > z1 {
                continue; // narrower than a body
            }
            let p = (x.clamp(x0, x1), z.clamp(z0, z1));
            let d = (p.0 - x).hypot(p.1 - z);
            if best.is_none_or(|(_, bd)| d < bd) && standing_on(hs, k, p.0, p.1).is_ok() {
                best = Some((p, d));
            }
        }
        best.map(|(p, _)| p)
    };
    if k == 0 {
        return nearest_of(&[(0.0, 0.0, hs.width, hs.depth)]);
    }
    // Each room's parts over solid floor: the room's rectangle cut to each solid slab of the floor.
    let over_floor = |rooms: &[(f32, f32, f32, f32)]| -> Vec<(f32, f32, f32, f32)> {
        let mut out = Vec::new();
        for &(rx0, rz0, rx1, rz1) in rooms {
            for s in floor_slabs(hs, floor).filter(|s| !s.passes_light()) {
                let (sx0, sz0, sx1, sz1) = s.rect();
                out.push((rx0.max(sx0), rz0.max(sz0), rx1.min(sx1), rz1.min(sz1)));
            }
        }
        out
    };
    let rooms = storey_rooms(hs, k);
    let here: Vec<_> = rooms.iter().copied().filter(|&(x0, z0, x1, z1)| x >= x0 && x <= x1 && z >= z0 && z <= z1).collect();
    nearest_of(&over_floor(&here)).or_else(|| nearest_of(&over_floor(&rooms)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped homestead (data/homes/homestead.ron), as the game loads it.
    fn homestead() -> HomeStructure {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/homes/homestead.ron");
        let text = std::fs::read_to_string(&path).expect("the homestead design");
        ron::from_str::<crate::ship::ship_structure::HomeDesign>(&text).expect("the homestead parses").body
    }

    /// THE HOMESTEAD'S GLASS LIDS ARE NO PLACE TO STAND, ITS UPSTAIRS ROOMS ARE. Upstairs (storey
    /// index 1, the floor at 3.0): the glass roof over the kitchen, the stairwell, the greenhouse
    /// open to the roof and the elevator shaft are refused, each for its own reason; the bedroom,
    /// the study and the upper hall are places to stand. Every ground spot inside the shell is.
    ///
    /// Seen red first (2026-10-21) with `standing_on` answering Ok for every spot, which is what the
    /// build editor's close did before (it stood the player on any slab top): "the glass roof over
    /// the kitchen / left: Ok(()) / right: Err(GlassRoof(Some(\"House roof, south half\")))".
    #[test]
    fn the_homesteads_glass_lids_are_no_place_to_stand_its_upstairs_rooms_are() {
        let h = homestead();
        assert_eq!(
            standing_on(&h, 1, 38.0, 48.0),
            Err(NoStanding::GlassRoof(Some("House roof, south half".to_string()))),
            "the glass roof over the kitchen"
        );
        assert_eq!(standing_on(&h, 1, 38.5, 38.6), Err(NoStanding::Opening), "the stairwell");
        assert_eq!(standing_on(&h, 1, 30.0, 60.0), Err(NoStanding::Opening), "the greenhouse, open to the roof");
        assert_eq!(standing_on(&h, 1, 31.0, 26.0), Err(NoStanding::Opening), "the elevator shaft");
        assert_eq!(standing_on(&h, 1, 10.0, 80.0), Err(NoStanding::GlassRoof(Some("Fields roof".to_string()))), "the fields' roof");
        assert_eq!(standing_on(&h, 1, 38.0, 34.0), Ok(()), "the upstairs bedroom");
        assert_eq!(standing_on(&h, 1, 52.5, 34.0), Ok(()), "the study");
        assert_eq!(standing_on(&h, 1, 45.0, 39.3), Ok(()), "the upper hall, where two of its slabs meet");
        assert_eq!(standing_on(&h, 1, 53.5, 40.5), Err(NoStanding::Edge), "the home's spawn, upstairs: on the upper hall's south wall");
        for (x, z) in [(38.0, 48.0), (10.0, 10.0), (30.0, 60.0), (53.5, 40.5)] {
            assert_eq!(standing_on(&h, 0, x, z), Ok(()), "the ground floor at ({x}, {z})");
        }
        assert_eq!(standing_on(&h, 0, 0.1, 10.0), Err(NoStanding::Edge), "in the shell");
        assert_eq!(standing_on(&h, 2, 38.0, 34.0), Err(NoStanding::NoStorey));
    }

    /// THE NEAREST PLACE TO STAND IS ON THE SAME STOREY, OFF THE LID. From the glass roof over the
    /// great room, the nearest place upstairs is the upper hall just north of its south wall (z
    /// 40.5 less the clearance); from the stairwell, the hall's gallery beside it (in the hall the
    /// stairwell is in, not through the wall into the bedroom, which is a little nearer); from the
    /// home's spawn (on that wall), just inside the hall. Each answer is a place to stand. A spot
    /// that is one comes back unchanged, and a storey with no room on it has none.
    ///
    /// Seen red first (2026-10-21) with `nearest_standing` giving every spot back as it was (the
    /// close before this fix): "from the great room's glass roof / left: Some((45.0, 45.0)) / right:
    /// Some((45.0, 40.05))".
    #[test]
    fn the_nearest_place_to_stand_is_on_the_same_storey_off_the_lid() {
        let h = homestead();
        let near = |x: f32, z: f32| nearest_standing(&h, 1, x, z).map(|(a, b)| ((a * 100.0).round() / 100.0, (b * 100.0).round() / 100.0));
        assert_eq!(near(45.0, 45.0), Some((45.0, 40.05)), "from the great room's glass roof");
        assert_eq!(near(53.5, 40.5), Some((53.5, 40.05)), "from the home's spawn, on the upper hall's south wall");
        assert_eq!(near(38.5, 38.6), Some((38.5, 39.75)), "from the stairwell, onto the gallery beside it");
        for (x, z) in [(45.0, 45.0), (53.5, 40.5), (38.5, 38.6), (10.0, 80.0), (30.0, 60.0)] {
            let (sx, sz) = nearest_standing(&h, 1, x, z).expect("a place to stand upstairs");
            assert_eq!(standing_on(&h, 1, sx, sz), Ok(()), "the place given for ({x}, {z}), ({sx}, {sz})");
        }
        assert_eq!(near(38.0, 34.0), Some((38.0, 34.0)), "the bedroom is already a place to stand");
        // A slab with no room on its storey: nowhere to stand on it.
        let bare: HomeStructure =
            ron::from_str("(width: 10.0, depth: 10.0, height: 6.0, floors: [(a: (0.0, 0.0), b: (10.0, 5.0), top: 3.0)])").unwrap();
        assert_eq!(nearest_standing(&bare, 1, 5.0, 2.0), None, "a floor with no room on it is a roof");
        assert_eq!(nearest_standing(&bare, 0, 5.0, 2.0), Some((5.0, 2.0)), "its ground floor is");
    }

    /// A STOREY WITH TOO LITTLE ROOM OVER IT IS NO PLACE TO STAND. A slab at 5.0 in a 6.0 m body
    /// (the editor's detail panel allows tops up to the height less 0.5) leaves 1.0 m to the roof,
    /// and the room under a slab at 1.0 leaves 0.75 m: neither stands anyone up.
    ///
    /// Seen red first (2026-10-21) with `standing_on` answering Ok for every spot: "1.0 m under the
    /// roof / left: Ok(()) / right: Err(Low)".
    #[test]
    fn a_storey_with_too_little_room_over_it_is_no_place_to_stand() {
        let low: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 5.0), (a: (0.0, 0.0), b: (5.0, 5.0), top: 1.0)], \
             zones: [(id: \"loft\", type_id: \"room_hall\", origin: (0.0, 5.0, 0.0), size: (10.0, 1.0, 10.0)), \
                     (id: \"bed\", type_id: \"room_hall\", origin: (0.0, 1.0, 0.0), size: (5.0, 4.0, 5.0))])",
        )
        .unwrap();
        let storeys = low.storeys();
        assert_eq!(storeys.len(), 3, "{storeys:?}");
        assert_eq!(standing_on(&low, 2, 5.0, 5.0), Err(NoStanding::Low), "1.0 m under the roof");
        assert_eq!(standing_on(&low, 0, 2.0, 2.0), Err(NoStanding::Low), "0.75 m under the slab at 1.0");
        assert_eq!(standing_on(&low, 1, 2.0, 2.0), Ok(()), "on the slab at 1.0, 3.75 m to the one at 5.0");
    }
}

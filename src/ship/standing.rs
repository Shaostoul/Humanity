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
//!     all round;
//!   - on any storey, it is not INSIDE A SOLID PIECE (the seams review of 2026-10-21, finding 1;
//!     boxes since the increment 7 follow-ups of the same day): a stair's flight where its steps
//!     are more than a step up (`wall_collision::STEP_UP_M`) over the storey's floor is solid, and
//!     so is a placed box (a platform, a road, a deck, the wall piece) whose top is, and a body put
//!     on the floor there is inside it, which the piece's sides then push out sideways through the
//!     walls beside it. Where the piece's TOP there (a step's tread, a box's top) is a place to
//!     stand (on the same storey, with room to stand over it), the spot is one, and the body stands
//!     on that top (`top_at`); otherwise it is not. And a caller that knows the runtime state names
//!     more footprints to keep out of (`KeepOut`): the build editor's close keeps out of an
//!     elevator's shaft whose car is on another floor.
//!
//! `standing_on` says whether a spot is one and why not; `nearest_standing` finds the nearest spot
//! on the same storey that is (the `_clear_of` forms with the caller's footprints kept out). Pure:
//! body-local metres, no world, no runtime state, so the game and its tests ask the same question.
//! The relay holds no copy of anyone's home and cannot ask it (src/relay/handlers/move_check.rs
//! `on_a_floor_of` says what it does instead).

use crate::ship::home_structure::{storey_index, FloorSlab, HomeStructure, PlacedStructure, Storey, STOREY_EPS};
use crate::ship::moves::STANDING_ROOM_M;
use crate::ship::structure::{self, MeshShape, StructureKind, StructureType};
use crate::ship::wall_collision::{self, SegmentKind, EYE_HEIGHT_M, STEP_UP_M};

/// How far a standing spot must be from a room's edge (a wall stands there) and from the edge of
/// the floor it is on, metres: 0.45, a body's radius (0.3 m, `wall_collision::PLAYER_RADIUS`) and
/// half of a thick wall (0.15 m). A body put closer could be pushed out of the wall on its far side,
/// onto a glass lid or into a stairwell. A spot moved out of a solid piece (`KeepOut`) goes this far
/// past its side.
pub const STAND_CLEAR_M: f32 = 0.45;

/// How far round a placed piece's footprint the footing reads it, metres: the tolerance
/// `structure::in_footprint` and `structure::walk_surface` give its edges, so a spot that is on a
/// stair or in a shaft to the footing is in it here too.
const FOOTPRINT_SLACK_M: f32 = 0.05;

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
    /// Inside the flight of a stair, where its steps are more than a step up (`STEP_UP_M`) over
    /// the storey's floor and the tread there is no place to stand (it is on another storey, or
    /// has too little room over it): a body put on the floor there is inside the solid stair, whose
    /// sides push it out sideways through whatever wall stands beside the flight.
    InStair,
    /// Inside a SOLID PIECE that is no stair (a box or a slab placed with the editor: the wall
    /// piece, a train platform, a road, a deck), where it stands more than a step up
    /// (`STEP_UP_M`) over the storey's floor and its top is no place to stand: a body put on the
    /// floor there is inside it, whose sides push it out sideways through whatever wall stands
    /// beside it (the increment 7 follow-ups, 2026-10-21).
    InPiece,
    /// In an elevator's shaft whose car is not at the storey's floor (a `KeepOut` the caller names,
    /// which knows where the cars are): a body put there stands under the car, or over the drop.
    InShaft,
    /// Too near a wall or the edge of the floor for a body (`STAND_CLEAR_M`).
    Edge,
    /// Less than `STANDING_ROOM_M` from the storey's floor to what is over the spot.
    Low,
    /// The body has no such storey.
    NoStorey,
}

/// A FOOTPRINT NOBODY IS PUT IN on a storey (the seams review of 2026-10-21, findings 1 and 5): the
/// rectangle `half` (x, z) metres each way about `centre` (body-local), turned `yaw` radians the
/// way a placed piece is (`structure::structure_mesh`'s turn), and why. The part of a stair's flight
/// more than a step up and a box taller than a step (`solid_keep_outs`, always kept), and what a
/// caller that knows the runtime state names (`standing_on_clear_of`): the build editor's close
/// keeps out of an elevator's shaft whose car is on another floor. A spot in one is moved straight
/// out of its nearest side (`nearest_standing_clear_of`).
#[derive(Debug, Clone, PartialEq)]
pub struct KeepOut {
    pub centre: (f32, f32),
    pub yaw: f32,
    pub half: (f32, f32),
    pub why: NoStanding,
}

impl KeepOut {
    /// Part of a placed piece's footprint: the piece at (x, z) `pos`, turned `yaw` radians,
    /// `half_w` each way across it and from `z0` to `z1` along its own z (its run), each side
    /// `FOOTPRINT_SLACK_M` further out, as the footing reads it.
    pub fn of_piece(pos: (f32, f32), yaw: f32, half_w: f32, z0: f32, z1: f32, why: NoStanding) -> Self {
        let (s, c) = yaw.sin_cos();
        let mid = (z0 + z1) * 0.5;
        KeepOut {
            centre: (pos.0 + mid * s, pos.1 + mid * c),
            yaw,
            half: (half_w + FOOTPRINT_SLACK_M, (z1 - z0).abs() * 0.5 + FOOTPRINT_SLACK_M),
            why,
        }
    }

    /// The spot (x, z) in its own frame (the inverse of the piece's turn).
    fn local(&self, x: f32, z: f32) -> (f32, f32) {
        let (s, c) = self.yaw.sin_cos();
        let (dx, dz) = (x - self.centre.0, z - self.centre.1);
        (dx * c - dz * s, dx * s + dz * c)
    }

    /// A point of its own frame, body-local.
    fn world(&self, lx: f32, lz: f32) -> (f32, f32) {
        let (s, c) = self.yaw.sin_cos();
        (self.centre.0 + lx * c + lz * s, self.centre.1 - lx * s + lz * c)
    }

    /// True when the spot (x, z) is in it, its edges included.
    pub fn holds(&self, x: f32, z: f32) -> bool {
        let (lx, lz) = self.local(x, z);
        lx.abs() <= self.half.0 && lz.abs() <= self.half.1
    }

    /// The four ways out of it from (x, z): straight across each side to `pad` metres past it.
    fn ways_out(&self, x: f32, z: f32, pad: f32) -> [(f32, f32); 4] {
        let (lx, lz) = self.local(x, z);
        let (hx, hz) = (self.half.0 + pad, self.half.1 + pad);
        [self.world(hx, lz), self.world(-hx, lz), self.world(lx, hz), self.world(lx, -hz)]
    }
}

/// The storey's floor slabs: those whose top is storey `floor` (within `STOREY_EPS`).
fn floor_slabs(hs: &HomeStructure, floor: f32) -> impl Iterator<Item = &FloorSlab> {
    hs.floors.iter().filter(move |s| (s.top - floor).abs() < STOREY_EPS)
}

/// True when (x, z) is over a SOLID slab of the floor at `floor`.
fn over_solid(hs: &HomeStructure, floor: f32, x: f32, z: f32) -> bool {
    floor_slabs(hs, floor).any(|s| !s.passes_light() && s.contains(x, z))
}

/// What is over (x, z) above the height `y` (body-local): the underside of the lowest slab there
/// whose top is over `y`, else the roof (the top storey's top).
fn ceiling_over(hs: &HomeStructure, storeys: &[Storey], y: f32, x: f32, z: f32) -> f32 {
    let roof = storeys.last().map_or(hs.height, |s| s.top);
    hs.floors.iter().filter(|s| s.top > y + STOREY_EPS && s.contains(x, z)).map(FloorSlab::underside).fold(roof, f32::min)
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

/// The SOLID PIECES whose solid can stand where a body on the floor at `floor` stands: the stairs
/// (pieces of kind Stairs: a flight or a ramp, solid from the floor up to each step) and the solid
/// boxes (a Box or Slab row, `structure::solid_box_height`: the wall piece, a train platform, a
/// road, a deck, solid from their foot to their top; the increment 7 follow-ups, 2026-10-21), those
/// whose foot is under the eye of a body standing there (on the floor, under it and rising through
/// it, or raised but low enough to stand across the body): the walk's own rule, which stops such a
/// body at a piece more than a step over its feet (`wall_collision::SegmentKind::Step`). A piece
/// whose foot is over that eye (a deck overhead, a stair standing on a floor above) is over the
/// body's head. Each with whether it is a stair.
fn solids_from(hs: &HomeStructure, floor: f32) -> impl Iterator<Item = (&PlacedStructure, &'static StructureType, bool)> + '_ {
    hs.structures.iter().filter_map(move |ps| {
        let ty = structure::structure_type(&ps.type_id)?;
        let stair = ty.kind == StructureKind::Stairs;
        ((stair || structure::solid_box_height(ty).is_some()) && ps.pos.1 < floor + EYE_HEIGHT_M).then_some((ps, ty, stair))
    })
}

/// What a solid piece makes of the spot (x, z) on storey `k`'s floor: None when none rises there
/// more than a step over the floor (the floor, or a low step or box the footing steps a body up
/// onto), else its top there (the highest, a stair's tread or a box's top), Ok when it is a place
/// to stand (still on storey `k`, with `STANDING_ROOM_M` over it) and Err with why not when the
/// spot is inside the piece under a top that is not (`NoStanding::InStair` inside a flight,
/// `NoStanding::InPiece` inside a box).
fn solid_at(hs: &HomeStructure, storeys: &[Storey], k: usize, x: f32, z: f32) -> Option<Result<f32, NoStanding>> {
    let floor = storeys.get(k)?.floor;
    let (t, stair) = solids_from(hs, floor)
        .filter_map(|(ps, ty, stair)| structure::walk_surface(ty, ps.pos, ps.rot_deg.to_radians(), x, z).map(|t| (t, stair)))
        .filter(|(t, _)| *t > floor + STEP_UP_M + 1e-4)
        .reduce(|a, b| if b.0 > a.0 { b } else { a })?;
    let room = ceiling_over(hs, storeys, t, x, z) - t;
    Some(if storey_index(storeys, t) == k && room >= STANDING_ROOM_M - STOREY_EPS {
        Ok(t)
    } else if stair {
        Err(NoStanding::InStair)
    } else {
        Err(NoStanding::InPiece)
    })
}

/// THE TOP A BODY STANDS ON at the spot (x, z) of storey `k`, body-local height: the tread of a
/// stair, or the top of a solid box (a platform, a deck), there that rises more than a step
/// (`STEP_UP_M`) over the storey's floor and is a place to stand (`standing_on` takes such a spot),
/// so a body put there stands on it, not on the floor inside the piece. None where nothing rises
/// that far: the footing steps a body up onto a low step or box from the floor
/// (`engine::footing::Footing::floor_under`).
pub fn top_at(hs: &HomeStructure, k: usize, x: f32, z: f32) -> Option<f32> {
    solid_at(hs, &hs.storeys(), k, x, z)?.ok()
}

/// THE SOLID PIECES OF STOREY `k` AS FOOTPRINTS NOBODY IS PUT IN (`KeepOut`): each flight
/// (`NoStanding::InStair`) whose foot is under the eye of a body on that storey's floor
/// (`solids_from`), from its first step that rises more than a step over the floor to its high
/// end; and each solid box there whose top is more than a step over the floor
/// (`NoStanding::InPiece`), its whole footprint; as the footing reads their footprints. A place to
/// stand found near one (`nearest_standing_clear_of`) is moved out of these; and the editor's Save
/// keeps the home's spawn out of them altogether (engine/editor_storeys.rs `spawn_from_avatar`),
/// because world entry stands the player on the floor at the spawn, never on a tread or a box's
/// top.
pub fn solid_keep_outs(hs: &HomeStructure, k: usize) -> Vec<KeepOut> {
    let Some(floor) = hs.storeys().get(k).map(|s| s.floor) else {
        return Vec::new();
    };
    solids_from(hs, floor)
        .filter_map(|(ps, ty, stair)| {
            let (w, h, d) = ty.size;
            let hd = d * 0.5;
            let yaw = ps.rot_deg.to_radians();
            // How high over the piece's foot its solid must rise to be more than a step over the floor.
            let over = floor + STEP_UP_M - ps.pos.1;
            if !stair {
                let top = structure::solid_box_height(ty)?;
                return (top > over + 1e-4).then(|| KeepOut::of_piece((ps.pos.0, ps.pos.2), yaw, w * 0.5, -hd, hd, NoStanding::InPiece));
            }
            let z0 = match ty.shape {
                MeshShape::Steps => {
                    let n = ty.steps.max(1);
                    let (tread, riser) = (d / n as f32, h / n as f32);
                    let first = (0..n).find(|&i| (i + 1) as f32 * riser > over + 1e-4)?;
                    -hd + first as f32 * tread
                }
                MeshShape::Ramp if h > over + 1e-4 => -hd + d * (over / h).max(0.0),
                _ => return None,
            };
            Some(KeepOut::of_piece((ps.pos.0, ps.pos.2), yaw, w * 0.5, z0, hd, NoStanding::InStair))
        })
        .collect()
}

/// Is the spot (x, z), body-local metres, on storey `k` of `hs` a place a person can stand? See
/// the top of this file for the rule. Ok, or the first reason it is not, in the order a person
/// would name it: no room to stand, a glass roof, an opening, a roof outside the rooms, inside a
/// stair, too near an edge. `standing_on_clear_of` with nothing more kept out.
pub fn standing_on(hs: &HomeStructure, k: usize, x: f32, z: f32) -> Result<(), NoStanding> {
    standing_on_clear_of(hs, k, x, z, &[])
}

/// `standing_on`, with the footprints `keep_out` kept out too (`KeepOut`; a spot in one is no
/// place to stand, for its `why`): what a caller that knows the runtime state adds, the build
/// editor's close an elevator's shaft whose car is on another floor.
pub fn standing_on_clear_of(hs: &HomeStructure, k: usize, x: f32, z: f32, keep_out: &[KeepOut]) -> Result<(), NoStanding> {
    let storeys = hs.storeys();
    let storey = *storeys.get(k).ok_or(NoStanding::NoStorey)?;
    let floor = storey.floor;
    if ceiling_over(hs, &storeys, floor, x, z) - floor < STANDING_ROOM_M - STOREY_EPS {
        return Err(NoStanding::Low);
    }
    let c = STAND_CLEAR_M;
    if k == 0 {
        // The ground storey's floor is the body's own, everywhere in its footprint.
        let inside = x >= c && x <= hs.width - c && z >= c && z <= hs.depth - c;
        if !inside {
            return Err(NoStanding::Edge);
        }
    } else {
        if !over_solid(hs, floor, x, z) {
            let glass = floor_slabs(hs, floor).find(|s| s.passes_light() && s.contains(x, z));
            return Err(match glass {
                Some(g) => NoStanding::GlassRoof(g.label.clone()),
                None => NoStanding::Opening,
            });
        }
        if !storey_rooms(hs, k).iter().any(|&(x0, z0, x1, z1)| x >= x0 && x <= x1 && z >= z0 && z <= z1) {
            return Err(NoStanding::OutsideRooms);
        }
    }
    // Inside a solid piece (the seams review, findings 1 and 5; the increment 7 follow-ups of
    // 2026-10-21 for boxes): a stair's flight or a box more than a step up whose top is no place to
    // stand, and what the caller keeps out.
    if let Some(Err(why)) = solid_at(hs, &storeys, k, x, z) {
        return Err(why);
    }
    if let Some(ko) = keep_out.iter().find(|ko| ko.holds(x, z)) {
        return Err(ko.why.clone());
    }
    if k == 0 {
        return Ok(());
    }
    let rooms = storey_rooms(hs, k);
    let in_room = |inset: f32| rooms.iter().any(|&(x0, z0, x1, z1)| x >= x0 + inset && x <= x1 - inset && z >= z0 + inset && z <= z1 - inset);
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
/// the floor; None when the storey has none. `nearest_standing_clear_of` with nothing more kept out.
pub fn nearest_standing(hs: &HomeStructure, k: usize, x: f32, z: f32) -> Option<(f32, f32)> {
    nearest_standing_clear_of(hs, k, x, z, &[])
}

/// The NEAREST place to stand on storey `k` of `hs` to the spot (x, z), body-local metres, with the
/// footprints `keep_out` kept out too (`standing_on_clear_of`); None when the storey has none. On
/// the ground storey, the spot kept `STAND_CLEAR_M` inside the body. Above it, the nearest point of
/// a room of the storey that lies over one solid slab of its floor, `STAND_CLEAR_M` in from both,
/// looked for first in the room the spot is in (so a spot over the stairwell in the upper hall
/// stays in the hall, rather than going through a wall into the bedroom beside it) and then in
/// every room of the storey. A point in a solid piece (a stair's flight or a box, `solid_keep_outs`,
/// or a footprint kept out) is taken straight out of it across each of its sides to
/// `STAND_CLEAR_M` past it (the seams review of 2026-10-21, findings 1 and 5). Of the points found,
/// one reached from the spot without crossing a wall that stands on that floor
/// (`wall_collision::wall_segments` at a body's height there, its doorways open; a guard's rail
/// does not count, being the edge of the opening a spot over it is moved off) is taken before a
/// nearer one through a wall, so a spot in a walled stair's flight comes out at its foot or its
/// top, not into the room beside it. Each candidate is checked with `standing_on_clear_of`, so a
/// spot with too little room over it is never given. A spot that is already a place to stand comes
/// back as it is.
///
/// A SPOT EXACTLY ON A WALL'S CENTRE LINE (the increment 7 follow-ups, 2026-10-21) is on neither
/// side of it, and the crossing test is strict (`wall_collision::sight_blocked`: an end on the
/// line crosses nothing), so from there every way off it, to either side, crossed nothing, and the
/// search could move the spot through the wall as if none stood there. Such a spot is taken where
/// the walk puts a body standing on it (`wall_collision::resolve`, which settles it out of the wall
/// to one side), and the search starts from there: the room it looks in, the ways out of a solid
/// piece and the crossings; the distances are still measured from the spot itself.
pub fn nearest_standing_clear_of(hs: &HomeStructure, k: usize, x: f32, z: f32, keep_out: &[KeepOut]) -> Option<(f32, f32)> {
    if standing_on_clear_of(hs, k, x, z, keep_out).is_ok() {
        return Some((x, z));
    }
    let storeys = hs.storeys();
    let floor = storeys.get(k)?.floor;
    let c = STAND_CLEAR_M;
    let mut keeps = solid_keep_outs(hs, k);
    keeps.extend(keep_out.iter().cloned());
    let wall_segs: Vec<wall_collision::WallSegment> = wall_collision::wall_segments(hs)
        .into_iter()
        .filter(|s| s.kind == SegmentKind::Wall && s.stops_body(floor + EYE_HEIGHT_M))
        .collect();
    let walls: Vec<[f32; 4]> = wall_segs.iter().map(|s| [s.a.0, s.a.1, s.b.0, s.b.1]).collect();
    // Where the search starts from: the spot, or where the walk puts a body on a wall's line.
    let on_a_line = wall_segs.iter().any(|s| distance_to_segment((x, z), s.a, s.b) < ON_LINE_M);
    let (sx, sz) = if on_a_line {
        let eye = glam::Vec3::new(x, floor + EYE_HEIGHT_M, z);
        let held = wall_collision::resolve(eye, eye, wall_collision::PLAYER_RADIUS, &wall_segs, &[]);
        (held.x, held.z)
    } else {
        (x, z)
    };
    // The nearest standing point of the given areas (each a rectangle, body-local).
    let nearest_of = |areas: &[(f32, f32, f32, f32)]| -> Option<(f32, f32)> {
        let mut best: Option<((f32, f32), (bool, f32))> = None;
        for &(x0, z0, x1, z1) in areas {
            let (x0, z0, x1, z1) = (x0 + c, z0 + c, x1 - c, z1 - c);
            if x0 > x1 || z0 > z1 {
                continue; // narrower than a body
            }
            let clamp = |p: (f32, f32)| (p.0.clamp(x0, x1), p.1.clamp(z0, z1));
            // The spot brought into the area, then out of any solid piece it is in, twice over (out
            // of one and, when that lands in another, out of that).
            let mut found = vec![clamp((sx, sz))];
            let mut from = 0;
            for _ in 0..2 {
                let to = found.len();
                for i in from..to {
                    let p = found[i];
                    for ko in keeps.iter().filter(|ko| ko.holds(p.0, p.1)) {
                        found.extend(ko.ways_out(p.0, p.1, c).map(clamp));
                    }
                }
                from = to;
            }
            for p in found {
                let rank = (wall_collision::sight_blocked((sx, sz), p, &walls), (p.0 - x).hypot(p.1 - z));
                if best.is_none_or(|(_, r)| rank < r) && standing_on_clear_of(hs, k, p.0, p.1, keep_out).is_ok() {
                    best = Some((p, rank));
                }
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
    let here: Vec<_> = rooms.iter().copied().filter(|&(x0, z0, x1, z1)| sx >= x0 && sx <= x1 && sz >= z0 && sz <= z1).collect();
    nearest_of(&over_floor(&here)).or_else(|| nearest_of(&over_floor(&rooms)))
}

/// Within this of a wall's centre line, metres, a spot is ON the line (`nearest_standing_clear_of`):
/// far less than any wall is thick. A spot this near but not on it is settled to its own side.
const ON_LINE_M: f32 = 1e-3;

/// How far the point `p` is from the segment `a`..`b` (plan, metres).
fn distance_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dz * dz;
    let t = if len2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / len2).clamp(0.0, 1.0) };
    (p.0 - (a.0 + dx * t)).hypot(p.1 - (a.1 + dz * t))
}

#[cfg(test)]
pub(crate) mod tests {
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

    /// A SPOT INSIDE A STAIR IS NO PLACE TO STAND UNLESS ITS TREAD IS ONE, AND THE WAY OFF IT IS
    /// PAST ITS FOOT OR ITS TOP, NOT THROUGH THE WALL BESIDE IT (the seams review of 2026-10-21,
    /// finding 1). On the homestead's ground storey, step 8 of stairs-1 (1.5 m up, x 38.75 to 39.03
    /// on its middle line, z 38.65) is a place to stand, on its tread (`top_at`); its top step
    /// (3.0 m, the upper floor's height) is no tread of the ground storey, and the nearest place off
    /// it is just past the flight's top end, in the hall. Its first steps, no higher than a step up,
    /// are floor. In a 10 m box with a stair walled on both sides under a slab at 3.0 (so only its
    /// treads up to the fourth have room to stand over them), a spot in the middle of the flight
    /// comes out past the stair's foot, though the room beyond the wall beside it is nearer.
    ///
    /// Seen red first (2026-10-21) with the walls not counted in the choice (the nearest point
    /// taken whatever it crosses): "the middle of the walled flight / left: Some((5.0, 3.9)) /
    /// right: Some((3.09, 5.0))".
    #[test]
    fn a_spot_inside_a_stair_is_no_place_to_stand_unless_its_tread_is_one() {
        let h = homestead();
        assert_eq!(standing_on(&h, 0, 38.89, 38.65), Ok(()), "step 8 of stairs-1, from the ground storey");
        assert_eq!(top_at(&h, 0, 38.89, 38.65), Some(1.5), "the tread of step 8");
        assert_eq!(top_at(&h, 0, 40.6, 38.65), None, "step 2 is a step up from the floor: floor");
        assert_eq!(standing_on(&h, 0, 36.6, 38.65), Err(NoStanding::InStair), "the top step, from the ground storey");
        let off = nearest_standing(&h, 0, 36.6, 38.65).map(|(a, b)| ((a * 100.0).round() / 100.0, (b * 100.0).round() / 100.0));
        assert_eq!(off, Some((36.0, 38.65)), "off the top step, past the flight's top end");
        // A stair climbing east from x 2.75 to 7.25 (its sides z 4.4 and 5.6), walled along z 4.3
        // and 5.7, under a slab over the whole box at 3.0.
        let walled: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)], \
             walls: [(a: (2.75, 4.3), b: (7.25, 4.3), height: 3.0), (a: (2.75, 5.7), b: (7.25, 5.7), height: 3.0)], \
             structures: [(id: \"stairs-1\", type_id: \"stairs\", pos: (5.0, 0.0, 5.0), rot_deg: 90.0)])",
        )
        .unwrap();
        assert_eq!(standing_on(&walled, 0, 5.0, 5.0), Err(NoStanding::InStair), "the middle of the walled flight, 1.69 m up under a 2.75 m ceiling");
        let off = nearest_standing(&walled, 0, 5.0, 5.0).map(|(a, b)| ((a * 100.0).round() / 100.0, (b * 100.0).round() / 100.0));
        assert_eq!(off, Some((3.09, 5.0)), "the middle of the walled flight");
    }

    /// A 10 x 10 m body 6 m tall with a train platform (3 x 0.4 x 8) raised to 0.8, turned to lie
    /// along x (x 1 to 9, z 3.5 to 6.5, its top 1.2, more than a step over the floor), walled along
    /// both long sides (z 3.4 and 6.6, the ground to 3.0); `slab` puts a floor over the whole body
    /// at 3.0, under which the platform's top has 1.55 m over it, too little to stand on.
    pub(crate) fn walled_platform(slab: bool) -> HomeStructure {
        let floors = if slab { "floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)]," } else { "" };
        ron::from_str(&format!(
            "(width: 10.0, depth: 10.0, height: 6.0, {floors} \
             walls: [(a: (1.0, 3.4), b: (9.0, 3.4), height: 3.0), (a: (1.0, 6.6), b: (9.0, 6.6), height: 3.0)], \
             structures: [(id: \"train-1\", type_id: \"train\", pos: (5.0, 0.8, 5.0), rot_deg: 90.0)])"
        ))
        .expect("the walled platform parses")
    }

    /// A SPOT INSIDE A SOLID PIECE IS NO PLACE TO STAND UNLESS ITS TOP IS ONE, AND THE WAY OFF IT IS
    /// PAST ITS END, NOT THROUGH THE WALL BESIDE IT (the increment 7 follow-ups, 2026-10-21: a built
    /// box taller than a step was not solid to the editor's close, whose spot inside it the box's
    /// sides then pushed out, possibly through a wall). On the ground storey of the walled platform
    /// under a floor (`walled_platform(true)`), a spot on the platform is inside a solid piece
    /// whose top has too little room over it (`NoStanding::InPiece`), and the nearest place to stand
    /// is past its west end (x 0.5), not through the wall beside it (z 3.0 is 1.5 m nearer). With
    /// no floor over it the platform's top is a place to stand (`top_at`, 1.2). A deck lying on
    /// the floor (a step up) and one overhead (its foot over the eye) are no solid there.
    ///
    /// Seen red first (2026-10-21) with only stairs solid to `standing_on`: "on the platform under
    /// the floor / left: Ok(()) / right: Err(InPiece)".
    #[test]
    fn a_spot_inside_a_solid_piece_is_no_place_to_stand_unless_its_top_is_one() {
        let roofed = walled_platform(true);
        assert_eq!(standing_on(&roofed, 0, 4.0, 5.0), Err(NoStanding::InPiece), "on the platform under the floor");
        let off = nearest_standing(&roofed, 0, 4.0, 5.0).map(|(a, b)| ((a * 100.0).round() / 100.0, (b * 100.0).round() / 100.0));
        assert_eq!(off, Some((0.5, 5.0)), "off the platform under the floor: past its west end, not through the wall beside it");
        assert_eq!(top_at(&roofed, 0, 4.0, 5.0), None, "no top to stand on under the floor");
        let open = walled_platform(false);
        assert_eq!(standing_on(&open, 0, 4.0, 5.0), Ok(()), "on the platform open to the roof");
        assert_eq!(top_at(&open, 0, 4.0, 5.0), Some(1.2), "the platform's top, open to the roof");
        let decks: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, structures: [(id: \"deck-1\", type_id: \"deck\", pos: (3.0, 0.0, 3.0)), \
             (id: \"deck-2\", type_id: \"deck\", pos: (7.0, 2.5, 7.0))])",
        )
        .expect("the decks parse");
        for (x, z, what) in [(3.0, 3.0, "a deck on the floor"), (7.0, 7.0, "a deck overhead")] {
            assert_eq!(standing_on(&decks, 0, x, z), Ok(()), "{what}");
            assert_eq!(top_at(&decks, 0, x, z), None, "{what}: no top a step over the floor");
        }
    }

    /// A SPOT EXACTLY ON A WALL'S CENTRE LINE IS TAKEN OFF IT ON THE SIDE THE WALK WOULD PUT A BODY
    /// STANDING THERE (the increment 7 follow-ups, 2026-10-21). A spot on the line is on neither
    /// side, so the strict crossing test (`wall_collision::sight_blocked`) let the search move it to
    /// either side as if no wall stood there. Upstairs between two rooms parted by a wall at z 5:
    /// the spot (5, 5) is too near the wall, and the place given is on the side the walk pushes a
    /// body standing on the line (`wall_collision::resolve`), whichever way the wall was drawn. On
    /// the shipped homestead's upper floor the same for a spot on wall 30's line (z 38, between the
    /// upper hall and the bathrooms, at x 45.5 between two of its doors): the place given is on the
    /// walk's side of it, 0.45 m off it, and a place to stand.
    ///
    /// Seen red first (2026-10-21) with the strict crossing test from the spot as it was: "drawn
    /// west to east: left: Some((5.0, 4.55)) / right: Some((5.0, 5.45))" (the first room's side,
    /// whatever side the walk puts the body).
    #[test]
    fn a_spot_on_a_walls_centre_line_is_taken_off_it_on_the_walks_side() {
        for (a, b, what) in [((0.0, 5.0), (10.0, 5.0), "drawn west to east"), ((10.0, 5.0), (0.0, 5.0), "drawn east to west")] {
            let parted: HomeStructure = ron::from_str(&format!(
                "(width: 10.0, depth: 10.0, height: 6.0, floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)], \
                 zones: [(id: \"a\", type_id: \"room_hall\", origin: (0.0, 3.0, 0.0), size: (10.0, 3.0, 5.0)), \
                         (id: \"b\", type_id: \"room_hall\", origin: (0.0, 3.0, 5.0), size: (10.0, 3.0, 5.0))], \
                 walls: [(a: ({}, {}), b: ({}, {}), base: 3.0, height: 3.0)])",
                a.0, a.1, b.0, b.1
            ))
            .expect("the parted storey parses");
            let walk = wall_collision::wall_segments(&parted);
            let eye = glam::Vec3::new(5.0, 3.0 + EYE_HEIGHT_M, 5.0);
            let pushed = wall_collision::resolve(eye, eye, wall_collision::PLAYER_RADIUS, &walk, &[]);
            let want = if pushed.z > 5.0 { (5.0, 5.45) } else { (5.0, 4.55) };
            let got = nearest_standing(&parted, 1, 5.0, 5.0).map(|(x, z)| ((x * 100.0).round() / 100.0, (z * 100.0).round() / 100.0));
            assert_eq!(got, Some(want), "{what}");
        }
        let h = homestead();
        assert_eq!(standing_on(&h, 1, 45.5, 38.0), Err(NoStanding::Edge), "on wall 30's line upstairs");
        let eye = glam::Vec3::new(45.5, 3.0 + EYE_HEIGHT_M, 38.0);
        let pushed = wall_collision::resolve(eye, eye, wall_collision::PLAYER_RADIUS, &wall_collision::wall_segments(&h), &[]);
        assert!((pushed.z - 38.0).abs() > 0.3, "the walk does not take a body off wall 30's line: {pushed:?}");
        let (sx, sz) = nearest_standing(&h, 1, 45.5, 38.0).expect("a place to stand off wall 30's line");
        assert!((sz - 38.0) * (pushed.z - 38.0) > 0.0 && (sz - 38.0).abs() > STAND_CLEAR_M - 1e-3, "off wall 30's line the place given is ({sx}, {sz}); the walk puts a body on the line at {pushed:?}");
        assert_eq!(standing_on(&h, 1, sx, sz), Ok(()), "the place given off wall 30's line, ({sx}, {sz})");
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

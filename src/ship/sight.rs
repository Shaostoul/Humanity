//! WHAT THE EYE CAN SEE ABOARD, IN THREE DIMENSIONS (ship homes increment 7 review).
//!
//! Two things in the game ask "can the player see that from here?": the walk-up target (the
//! machine E acts on, lib.rs) and the HUD's floating machine labels and crew nameplates
//! (gui/pages/hud.rs). Until the homestead had an upper floor both answered in plan only: a wall
//! anywhere on the line between the eye and the label, at any height, hid it, and nothing else
//! did. With storeys that is wrong both ways. A wall upstairs hid a label on the ground floor
//! under it, and a ground wall hid one upstairs over it; and nothing at all hid the bedroom's
//! machines from the dining room right under them, so their cards floated at the ceiling and E
//! reached up through the floor to sleep in the bed upstairs.
//!
//! So a sight line here is a 3-D segment, eye to target, and:
//! - a WALL (`WallSegment`) hides what is behind it only where the line crosses it between the
//!   wall's own foot and top (`y0`..`y1`): a wall upstairs is over the line of a person
//!   downstairs, a ground wall under the line of one upstairs;
//! - a SOLID floor slab hides what is on the other side of it (the floor of the storey above is
//!   the ceiling of the room below); a GLASS one does not, you see through a glass lid as through
//!   the glass roof (`FloorSlab::passes_light`);
//! - a SOLID PIECE placed in the home (a box: the wall piece, a platform, a road, a deck; or a
//!   stair, a flight or a ramp) hides what is behind it through its solid, as the walk meets it
//!   (`PieceSolid`, the increment 7 follow-ups of 2026-10-22: labels showed through them);
//! - windows, guards and open doors are not blockers at all (the wall's sight spans,
//!   `wall_collision::sight_segments_with_shell_cuts`), and a shut door blocks between its sill
//!   and its head (`wall_collision::door_panel_segment`), which the caller adds.
//!
//! The walk-up target also keeps to the relay's reach (`moves::in_reach`): within 5 m and on the
//! player's storey by the home's own storeys, measured at the machine's foot (`walk_up_target`,
//! `storey_floor`: a machine with no floor of its storey under it, hung high in a room open to the
//! roof, belongs to the floor under it).
//!
//! E at a built piece and F's take-down ask the same question of what a HAND cannot pass
//! (`ReachBlockers`): the walls as the walk meets them, by height, the doors shut now, and every
//! floor slab.

use crate::ship::home_structure::{FloorSlab, HomeStructure, SlabMaterial};
use crate::ship::ship_structure::ShipStructure;
use crate::ship::wall_collision::WallSegment;
use glam::Vec3;

/// A SOLID floor slab as a sight blocker: its box, from its underside to its top over its
/// footprint, in whatever frame the blockers are in (a body's, the ship's, or the HUD's world
/// frame with the station offset added).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlabBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl SlabBox {
    /// This box moved by `by`.
    pub fn moved(self, by: Vec3) -> Self {
        SlabBox { min: self.min + by, max: self.max + by }
    }

    /// True when the straight line `eye` -> `target` passes through the INSIDE of the box. A
    /// line that only touches a face, an edge or a corner does not (the open box), so a label
    /// standing on a slab is not hidden by the slab under it.
    pub fn blocks(&self, eye: Vec3, target: Vec3) -> bool {
        // The slab method: on each axis, the part of the line (t from 0 at the eye to 1 at the
        // target) strictly between the box's two faces; the line is inside the box where all
        // three parts overlap, and it enters it only if that overlap is more than a point.
        let d = target - eye;
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for k in 0..3 {
            let (o, dk, lo, hi) = (eye[k], d[k], self.min[k], self.max[k]);
            if dk.abs() < 1e-9 {
                // Running along this axis: inside the box on it everywhere, or nowhere.
                if o <= lo || o >= hi {
                    return false;
                }
                continue;
            }
            let (a, b) = ((lo - o) / dk, (hi - o) / dk);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
            if t0 >= t1 {
                return false;
            }
        }
        true
    }
}

/// How a SOLID PIECE stands over its footprint (`PieceSolid`): what its top is along its own run
/// (local z, -half depth to +half depth), the way the walk reads it (`structure::walk_surface`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PieceTop {
    /// A box (the wall piece, a platform, a road, a deck): one top, this far over its foot.
    Flat(f32),
    /// A flight of `n` steps, each a column from the foot to its tread, `rise` higher than the
    /// last, the first at the run's low end.
    Steps { n: u32, rise: f32 },
    /// A ramp, rising evenly from the foot at the run's low end to this height at its high end.
    Ramp(f32),
}

/// A SOLID PIECE as a sight blocker (the increment 7 follow-ups, 2026-10-22): a placed box (the
/// wall piece, a train platform, a road, a deck) or a stair (a flight or a ramp), solid from its
/// foot up to its top over its footprint, turned with the piece. A built wall piece hides what is
/// behind it, and a flight hides what is behind or beyond it below its steps, as a floor slab does
/// (`SlabBox`); they were left out of the sight lines, so labels showed through them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PieceSolid {
    /// Its foot's middle (the piece's `pos`), in the blockers' frame.
    pub foot: Vec3,
    /// The sine and cosine of its turn (`structure::structure_mesh`'s: local (x, z) lies at
    /// (foot.x + x cos + z sin, foot.z - x sin + z cos)).
    pub sin_cos: (f32, f32),
    /// Half its width (local x) and half its depth (local z, its run), metres.
    pub half: (f32, f32),
    pub top: PieceTop,
    /// Its turned footprint's bounding box, from its foot to its highest top, in the blockers'
    /// frame: a line that misses it misses the piece, and most lines do, so `blocks` tests this
    /// first, one segment-box test, as a slab costs.
    pub bounds: SlabBox,
}

impl PieceSolid {
    /// This piece moved by `by`.
    pub fn moved(self, by: Vec3) -> Self {
        PieceSolid { foot: self.foot + by, bounds: self.bounds.moved(by), ..self }
    }

    /// Its highest top over its foot.
    fn height(&self) -> f32 {
        match self.top {
            PieceTop::Flat(h) | PieceTop::Ramp(h) => h,
            PieceTop::Steps { n, rise } => n as f32 * rise,
        }
    }

    /// True when the straight line `eye` -> `target` passes through the INSIDE of the piece's
    /// solid (the open solid, as `SlabBox::blocks`: a line that only touches a face does not, so a
    /// label standing on a deck is not hidden by the deck under it).
    ///
    /// COST: one segment-box test against its bounding box (`bounds`, three divisions), which
    /// most lines miss; only for a line that enters it, the line turned into the piece's own frame
    /// (four multiplies), the slab test against its own box, and for a flight one test per step it
    /// crosses (16 at most) or for a ramp one more comparison.
    pub fn blocks(&self, eye: Vec3, target: Vec3) -> bool {
        if !self.bounds.blocks(eye, target) {
            return false;
        }
        let (s, c) = self.sin_cos;
        let local = |p: Vec3| {
            let (dx, dz) = (p.x - self.foot.x, p.z - self.foot.z);
            Vec3::new(dx * c - dz * s, p.y - self.foot.y, dx * s + dz * c)
        };
        let (o, t) = (local(eye), local(target));
        let d = t - o;
        let (hw, hd) = self.half;
        // The part of the line (0 at the eye, 1 at the target) strictly inside the box from
        // `lo` to `hi` (local), if any.
        let inside = |lo: Vec3, hi: Vec3| -> Option<(f32, f32)> {
            let (mut t0, mut t1) = (0.0f32, 1.0f32);
            for k in 0..3 {
                let (ok, dk) = (o[k], d[k]);
                if dk.abs() < 1e-9 {
                    if ok <= lo[k] || ok >= hi[k] {
                        return None;
                    }
                    continue;
                }
                let (a, b) = ((lo[k] - ok) / dk, (hi[k] - ok) / dk);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
                if t0 >= t1 {
                    return None;
                }
            }
            Some((t0, t1))
        };
        let Some((t0, t1)) = inside(Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, self.height(), hd)) else {
            return false;
        };
        match self.top {
            PieceTop::Flat(_) => true,
            PieceTop::Steps { n, rise } => {
                // Each step's column, from the foot to its tread, over its slice of the run.
                let tread = 2.0 * hd / n.max(1) as f32;
                (0..n.max(1)).any(|i| {
                    let z0 = -hd + i as f32 * tread;
                    inside(Vec3::new(-hw, 0.0, z0), Vec3::new(hw, (i + 1) as f32 * rise, z0 + tread)).is_some()
                })
            }
            PieceTop::Ramp(h) => {
                // Under the ramp's slope where the line is in its box: the line's height less the
                // slope's under it is straight along the line, so its least is at an end.
                let under = |tt: f32| {
                    let p = o + d * tt;
                    p.y - h * ((p.z + hd) / (2.0 * hd)).clamp(0.0, 1.0)
                };
                under(t0).min(under(t1)) < 0.0
            }
        }
    }
}

/// Every SOLID PIECE of one body as sight blockers in the body's own frame (`PieceSolid`): each
/// stair (a flight or a ramp) and each solid box (`structure::solid_box_height`), as the walk is
/// stopped by them (`wall_collision::wall_segments`).
pub fn solid_pieces(home: &HomeStructure) -> impl Iterator<Item = PieceSolid> + '_ {
    use crate::ship::structure::{self, MeshShape, StructureKind};
    home.structures.iter().filter_map(|ps| {
        let ty = structure::structure_type(&ps.type_id)?;
        let (w, h, d) = ty.size;
        let top = match (ty.kind, ty.shape) {
            (StructureKind::Stairs, MeshShape::Steps) => PieceTop::Steps { n: ty.steps.max(1), rise: h / ty.steps.max(1) as f32 },
            (StructureKind::Stairs, MeshShape::Ramp) => PieceTop::Ramp(h),
            _ => PieceTop::Flat(structure::solid_box_height(ty)?),
        };
        let foot = Vec3::new(ps.pos.0, ps.pos.1, ps.pos.2);
        let yaw = ps.rot_deg.to_radians();
        let (bx, _, bz) = structure::rotated_half_extents(ty, yaw);
        let height = match top {
            PieceTop::Flat(t) | PieceTop::Ramp(t) => t,
            PieceTop::Steps { n, rise } => n as f32 * rise,
        };
        // A hair wider than the turned footprint, so rounding never shaves the piece's own edge.
        let pad = 1e-3;
        let bounds = SlabBox { min: foot - Vec3::new(bx + pad, pad, bz + pad), max: foot + Vec3::new(bx + pad, height + pad, bz + pad) };
        Some(PieceSolid { foot, sin_cos: yaw.sin_cos(), half: (w * 0.5, d * 0.5), top, bounds })
    })
}

/// The SOLID PIECES of the whole ship in the ship's frame: each zone's, moved to its origin, as
/// `ship_solid_slabs` places the slabs. A home put away has none.
pub fn ship_solid_pieces(ship: &ShipStructure) -> impl Iterator<Item = PieceSolid> + '_ {
    ship.zones.iter().enumerate().filter(move |(zi, _)| !ship.is_away_home(*zi)).flat_map(|(_, zone)| {
        let origin = Vec3::new(zone.origin.0, zone.origin.1, zone.origin.2);
        solid_pieces(&zone.body).map(move |p| p.moved(origin))
    })
}

/// Every sight blocker the HUD and the walk-up test against, in one frame: the walls (static
/// sight spans and the shut doors, each with the heights it stands between), the solid slabs and
/// the solid pieces (placed boxes and stairs). Rebuilt by lib.rs once a frame, reusing its lists,
/// so a frame allocates nothing.
#[derive(Clone, Debug, Default)]
pub struct SightBlockers {
    pub walls: Vec<WallSegment>,
    pub slabs: Vec<SlabBox>,
    pub pieces: Vec<PieceSolid>,
}

impl SightBlockers {
    /// Empty the lists, keeping their room.
    pub fn clear(&mut self) {
        self.walls.clear();
        self.slabs.clear();
        self.pieces.clear();
    }

    /// True when there is nothing to block sight (no home, the build editor): every label shows.
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty() && self.slabs.is_empty() && self.pieces.is_empty()
    }

    /// True when something stands between `eye` and `target`.
    ///
    /// COST: one plan-view crossing test per wall (four 2-D cross products, and the height only
    /// for a wall the line does cross), one segment-box test per slab (three divisions), and per
    /// solid piece a turn into its frame and one segment-box test, with a test per step it
    /// crosses (16 at most) only for a line that enters a flight's box (`PieceSolid::blocks`). The
    /// HUD runs it once per label inside the dot reach (21 m) per frame, and the walk-up only for
    /// a machine already in reach and in the look cone. The homestead aboard the ship is a few
    /// hundred wall spans, about ten solid slabs and three pieces (stairs-1, the ramp, train-1).
    /// MEASURED 2026-10-23 (100 of the homestead's labels from the upper hall's foot, the shipped
    /// ship's 2,491 sight walls, in the UNOPTIMIZED test build on a machine running other builds,
    /// so an upper bound; a release build is many times faster): the walls 400 to 460 us a frame,
    /// the 7 solid slabs 73 to 91 us, the 3 pieces 76 us, and 50 pieces (a busy home of the
    /// player's own) 570 to 670 us: 0.13 to 0.25 us a piece test against 0.10 to 0.13 us a slab's,
    /// most lines turned away by the piece's bounding box (`PieceSolid::bounds`).
    pub fn blocked(&self, eye: Vec3, target: Vec3) -> bool {
        self.walls.iter().any(|w| wall_blocks(w, eye, target))
            || self.slabs.iter().any(|s| s.blocks(eye, target))
            || self.pieces.iter().any(|p| p.blocks(eye, target))
    }
}

/// True when the line `eye` -> `target` crosses the wall at a height the wall stands at: it
/// crosses the wall's centre line in plan (a proper crossing, as `wall_collision::sight_blocked`
/// has always tested: grazing an end or running along the wall does not count, which errs toward
/// showing a label at a doorway's edge rather than blinking it), and at that point the line is
/// above the wall's foot and below its top.
pub fn wall_blocks(seg: &WallSegment, eye: Vec3, target: Vec3) -> bool {
    // Twice the signed area of the triangle (a, b, c) in plan: which side of a..b the point c is.
    let orient = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let (e, t) = ((eye.x, eye.z), (target.x, target.z));
    // The wall's ends on opposite sides of the line, and the line's ends on opposite sides of
    // the wall (strictly): a proper crossing.
    if orient(e, t, seg.a) * orient(e, t, seg.b) >= 0.0 {
        return false;
    }
    let (de, dt) = (orient(seg.a, seg.b, e), orient(seg.a, seg.b, t));
    if de * dt >= 0.0 {
        return false;
    }
    // `de` and `dt` are the two ends' distances from the wall's line (both scaled alike), so the
    // line meets the wall `de / (de - dt)` of the way from the eye to the target.
    let y = eye.y + (target.y - eye.y) * (de / (de - dt));
    y > seg.y0 && y < seg.y1
}

/// A floor slab as a box in its body's own frame, from its underside to its top over its footprint.
fn slab_box(s: &FloorSlab) -> SlabBox {
    let (x0, z0, x1, z1) = s.rect();
    let (y0, y1) = s.y_range();
    SlabBox { min: Vec3::new(x0, y0, z0), max: Vec3::new(x1, y1, z1) }
}

/// The SOLID slabs of one body, as boxes in the body's own frame. A glass slab is left out: it is
/// a floor and a lid, but it is seen through.
pub fn solid_slabs(home: &HomeStructure) -> impl Iterator<Item = SlabBox> + '_ {
    home.floors.iter().filter(|s| s.material == SlabMaterial::Solid).map(slab_box)
}

/// The SOLID slabs of the whole ship in the ship's frame: each zone's, moved to that zone's
/// origin, as `wall_collision::ship_sight_segments` places each zone's walls. A home put away has
/// none (it is not drawn). The neighbours' homes are left out: they are drawn but no label of
/// this game is in them, and their own walls already stand between them and this home.
pub fn ship_solid_slabs(ship: &ShipStructure) -> impl Iterator<Item = SlabBox> + '_ {
    ship.zones.iter().enumerate().filter(move |(zi, _)| !ship.is_away_home(*zi)).flat_map(|(_, zone)| {
        let origin = Vec3::new(zone.origin.0, zone.origin.1, zone.origin.2);
        solid_slabs(&zone.body).map(move |b| b.moved(origin))
    })
}

/// WHAT STANDS BETWEEN A HAND AND A THING IT REACHES FOR (the seams review of ship homes increment
/// 7, 2026-10-21, finding 2): E at a built piece (engine/built_uses.rs) and F's take-down
/// (engine/build_place.rs) ask it of the line from the eye to where the look ray meets the piece
/// (`systems::construction::uses::first_in_view`). The sight test's rules, by height
/// (`wall_blocks`, `SlabBox::blocks`), over what a hand cannot pass:
/// - the WALLS AS THE WALK MEETS THEM (`wall_collision::ship_wall_segments`, the list lib.rs keeps
///   as `EngineState::wall_colliders`): a window's glass is solid, and a doorway is open from its
///   foot to its head with the wall over it standing as its own segment (a lintel), so a piece seen
///   through a door is reached and one beyond the wall over it, or beside it, is not;
/// - EVERY floor slab, glass as well as solid: a glass floor is seen through but not reached
///   through;
/// - the DOORS SHUT NOW (the increment 7 follow-ups, 2026-10-21), each from its sill to its head,
///   the same segments the walk sweeps against (`engine::built_uses::shut_door_segments`, kept by
///   lib.rs as `EngineState::shut_doors`): a hand does not reach through a shut door any more than
///   a body walks through it. They were local to lib.rs's collision block, so E and F reached
///   through every shut door.
///
/// It used to be the walls in plan alone (`ray_crosses_walls`, which read no heights), which took
/// the lintel over every door for a wall across the doorway at every height (23 of the homestead's
/// 35 ground doors hid everything beyond them from E and F) and let a hand reach through a floor.
/// Borrowed, so a frame builds nothing for it.
#[derive(Clone, Copy, Default)]
pub struct ReachBlockers<'a> {
    pub walls: &'a [WallSegment],
    pub doors: &'a [WallSegment],
    pub ship: Option<&'a ShipStructure>,
}

impl ReachBlockers<'_> {
    /// True when something stands between `eye` and `target` that a hand does not reach through.
    pub fn blocked(&self, eye: Vec3, target: Vec3) -> bool {
        self.walls.iter().chain(self.doors).any(|w| wall_blocks(w, eye, target)) || self.ship.is_some_and(|s| ship_slabs(s).any(|b| b.blocks(eye, target)))
    }
}

/// EVERY floor slab of the ship, solid and glass, as boxes in the ship's frame (`ReachBlockers`):
/// each zone's, moved to its origin; none of a home put away.
pub fn ship_slabs(ship: &ShipStructure) -> impl Iterator<Item = SlabBox> + '_ {
    ship.zones.iter().enumerate().filter(move |(zi, _)| !ship.is_away_home(*zi)).flat_map(|(_, zone)| {
        let origin = Vec3::new(zone.origin.0, zone.origin.1, zone.origin.2);
        zone.body.floors.iter().map(move |s| slab_box(s).moved(origin))
    })
}

/// How close to straight ahead a machine must be for the walk-up to pick it: the cosine of the
/// half-angle of the look cone, 0.9 (about 25 degrees), as it has been since v0.431.
pub const WALK_UP_CONE_COS: f32 = 0.9;

/// What stands at a point `storey_floor` is asked about, which decides the storey of a point where
/// no floor is under it in plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foot {
    /// The feet of a BODY. A body is always held up by something at its height (a floor, a stair's
    /// tread, an elevator's car, a ladder's top), so its feet at a storey's floor are on that
    /// storey wherever they are in plan: a rider on the car at the top of its shaft is upstairs.
    Body,
    /// The foot of a FIXTURE (a machine's placement). A fixture may hang (a unit under a ceiling or
    /// on a wall), so it belongs to the floor under it: where no slab of its storey's floor is under
    /// it in plan (a room open to the roof, a stairwell), the storey below.
    Fixture,
}

/// THE FLOOR OF THE STOREY A POINT IS ON, from the home's own storeys (its slab tops,
/// `HomeStructure::storeys`): the zone of `ship` that holds the point `p` (ship frame; in plan,
/// and from half a metre under its floor up to its top), and that zone's storey by
/// `storey_index`, as a ship-frame height. None outside every zone (a corridor, a home put away,
/// off the ship). Two points are on one storey when their floors are one height (`STOREY_EPS`),
/// so a home with 2.5 m storeys is read as it is built, not by a 3 m rule of thumb; and two zones
/// side by side on one deck share their ground storey.
///
/// A FIXTURE'S STOREY HAS A FLOOR UNDER IT (the seams review of 2026-10-21, finding 4): for a
/// `Foot::Fixture` with no slab of its storey's floor under it in plan, the storey below, down to
/// the ground's (which is everywhere), so a room open to the roof belongs to the floor it stands
/// on. The greenhouse is 6 m to the roof with no slab over it: a unit hung there 3 m or more up used
/// to read as upstairs, and E refused it from the greenhouse floor. A `Foot::Body` is on the storey
/// of its height, slab or not (`Foot`).
///
/// COST: one plan test per zone, and the storeys of the one zone that holds the point, worked out
/// from its slabs (a small list), and for a fixture one plan test per slab of each storey walked
/// down. The walk-up asks it once for the player and once per machine that is already in reach and
/// in the look cone, so a handful of times a frame.
pub fn storey_floor(ship: &ShipStructure, p: Vec3, foot: Foot) -> Option<f32> {
    use crate::ship::home_structure::storey_index;
    let zone = ship.zones.iter().enumerate().filter(|(zi, _)| !ship.is_away_home(*zi)).map(|(_, z)| z).find(|z| {
        let (b, (ox, oy, oz)) = (&z.body, z.origin);
        p.x >= ox
            && p.x <= ox + b.width
            && p.z >= oz
            && p.z <= oz + b.depth
            && p.y >= oy - crate::ship::moves::STOREY_BAND_BELOW_M
            && p.y <= oy + b.height
    })?;
    let storeys = zone.body.storeys();
    let mut k = storey_index(&storeys, p.y - zone.origin.1);
    if foot == Foot::Fixture {
        let (x, z) = (p.x - zone.origin.0, p.z - zone.origin.2);
        while k > 0 && !zone.body.floors.iter().any(|s| s.is_floor_of(&storeys[k]) && s.contains(x, z)) {
            k -= 1;
        }
    }
    storeys.get(k).map(|s| zone.origin.1 + s.floor)
}

/// THE WALK-UP TARGET: which machine E acts on. `machines` gives each machine's label anchor
/// (the point the HUD draws its dot at, 0.4 m over its top) and the height it stands at (its
/// foot), both in the frame of `eye`; `feet` is the height of the player's feet in that frame;
/// `storey_floor` says which storey a point of that frame is on, asked of the player's feet as a
/// `Foot::Body` and of each machine's foot as a `Foot::Fixture` (the floor's height; None where
/// no home says, `storey_floor` above with the frame's offset). The answer is the index of the
/// nearest machine (by its label) that is
/// - within reach (`moves::INTERACT_REACH_M`, 5 m, to its label: the relay's reach,
///   `moves::in_reach`, is the outer limit);
/// - on the player's storey: the storey its FOOT is on is the one the player's feet are on, by
///   the home's own storeys. Its foot, not its label, which floats 0.4 m over the machine's top
///   (a 2.4 m tower's is 2.8 m over its floor) and so is on no storey in particular; and a unit
///   hung under a ceiling (the greenhouse's air handlers stand 2.7 m up) is on the storey under
///   that ceiling. Where no home says (a point in no zone), the relay's own band at the foot
///   (`moves::on_my_storey`'s, from the given feet);
/// - inside the look cone (`WALK_UP_CONE_COS` of `forward`);
/// - and in sight (`SightBlockers::blocked`: no wall across the line at its height, no solid
///   slab between).
///
/// The storey and the sight are tested last, and only for a machine that would beat the best so
/// far, so a frame pays for them a handful of times at most.
pub fn walk_up_target(
    eye: Vec3,
    feet: f32,
    forward: Vec3,
    machines: impl IntoIterator<Item = (Vec3, f32)>,
    blockers: &SightBlockers,
    storey_floor: impl Fn(Vec3, Foot) -> Option<f32>,
) -> Option<usize> {
    use crate::ship::home_structure::STOREY_EPS;
    use crate::ship::moves::{INTERACT_REACH_M, STOREY_BAND_ABOVE_M, STOREY_BAND_BELOW_M};
    let mine = storey_floor(Vec3::new(eye.x, feet, eye.z), Foot::Body);
    let mut best: Option<(usize, f32)> = None;
    for (i, (label, foot)) in machines.into_iter().enumerate() {
        let to = label - eye;
        let dist = to.length();
        if !(0.05..=INTERACT_REACH_M).contains(&dist) {
            continue;
        }
        if (to / dist).dot(forward) < WALK_UP_CONE_COS {
            continue;
        }
        if best.is_some_and(|b| dist >= b.1) {
            continue;
        }
        let same_storey = match (mine, storey_floor(Vec3::new(label.x, foot, label.z), Foot::Fixture)) {
            (Some(a), Some(b)) => (a - b).abs() < STOREY_EPS,
            _ => (-STOREY_BAND_BELOW_M..=STOREY_BAND_ABOVE_M).contains(&(foot - feet)),
        };
        if !same_storey || blockers.blocked(eye, label) {
            continue;
        }
        best = Some((i, dist));
    }
    best.map(|b| b.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::wall_collision::ship_sight_segments;
    use std::collections::HashMap;
    use std::path::Path;

    /// One machine as the game places it: (id, room, label anchor, foot), ship frame.
    type Placed = (String, String, Vec3, f32);

    /// THE HOMESTEAD AS THE GAME WALKS IT: the shipped ship with the shipped homestead on its
    /// default plot, its sight blockers as lib.rs builds them (the ship's sight spans with every
    /// door open, and its solid slabs), and every machine of data/machines/home.ron where the
    /// game places it, its label 0.4 m over its top (engine/home_meshes.rs, world_load.rs).
    struct Aboard {
        blockers: SightBlockers,
        /// The home zone's origin: the homestead's own (body-local) coordinates plus this are
        /// ship coordinates.
        origin: Vec3,
        /// The homestead's storeys, in its own heights.
        storeys: Vec<crate::ship::home_structure::Storey>,
        machines: Vec<Placed>,
        ship: ShipStructure,
    }

    fn aboard() -> Aboard {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let ship = ShipStructure::load_and_assemble_shipped(&root.join("data"), None).expect("the shipped ship assembles");
        let hz = &ship.zones[ship.home_zone_index()];
        let origin = Vec3::new(hz.origin.0, hz.origin.1, hz.origin.2);
        let blockers = SightBlockers { walls: ship_sight_segments(&ship), slabs: ship_solid_slabs(&ship).collect(), pieces: ship_solid_pieces(&ship).collect() };
        let home = crate::machines::MachineHome::load(&root.join("data").join("machines").join("home.ron")).expect("home.ron parses");
        let machines = home
            .placements(&HashMap::new(), Some(&ship.zone_rects()))
            .into_iter()
            .map(|p| (p.id, p.room, Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2), p.pos.1))
            .collect();
        let storeys = hz.body.storeys();
        Aboard { blockers, origin, storeys, machines, ship }
    }

    impl Aboard {
        /// A point of the homestead, from its own coordinates.
        fn at(&self, x: f32, y: f32, z: f32) -> Vec3 {
            self.origin + Vec3::new(x, y, z)
        }

        fn machine(&self, id: &str) -> &Placed {
            self.machines.iter().find(|m| m.0 == id).unwrap_or_else(|| panic!("{id} is not in data/machines/home.ron"))
        }

        /// The ids of `room`'s machines whose labels are within `reach` of `eye` and SHOW (no
        /// blocker between).
        fn shown(&self, eye: Vec3, room: &str, reach: f32) -> Vec<&str> {
            self.machines
                .iter()
                .filter(|m| m.1 == room && m.2.distance(eye) <= reach && !self.blockers.blocked(eye, m.2))
                .map(|m| m.0.as_str())
                .collect()
        }

        /// How many of `room`'s machines are within `reach` of `eye`, shown or not.
        fn near(&self, eye: Vec3, room: &str, reach: f32) -> usize {
            self.machines.iter().filter(|m| m.1 == room && m.2.distance(eye) <= reach).count()
        }

        /// The machine E acts on, looking from `eye` straight at `id`'s label.
        fn e_at(&self, eye: Vec3, id: &str) -> Option<&str> {
            let fwd = (self.machine(id).2 - eye).normalize();
            let storey = |p: Vec3, foot: Foot| storey_floor(&self.ship, p, foot);
            walk_up_target(eye, eye.y - 1.7, fwd, self.machines.iter().map(|m| (m.2, m.3)), &self.blockers, storey).map(|i| self.machines[i].0.as_str())
        }
    }

    /// The HUD's card reach, metres (gui/mod.rs `machine_label_card_dist`'s default).
    const CARD_M: f32 = 8.0;

    /// A WALL HIDES ONLY ACROSS ITS OWN HEIGHT. A wall upstairs (3 to 6 m) hides a label on the
    /// upper floor from an eye on it and nothing on the ground floor under it; a ground wall (0
    /// to 3 m) the reverse; a wall at every height (a blocker whose heights are not known) hides
    /// both; grazing a wall's end hides nothing, as in plan before (`sight_blocked`).
    ///
    /// Seen red first (2026-10-21) with today's rule, the plan-view crossing at any height:
    /// "a wall upstairs hid a label downstairs".
    #[test]
    fn a_wall_hides_only_across_its_own_height() {
        let upstairs = WallSegment { a: (5.0, 0.0), b: (5.0, 10.0), half_thickness: 0.075, y0: 3.0, y1: 6.0, kind: crate::ship::wall_collision::SegmentKind::Wall };
        let ground = WallSegment { y0: 0.0, y1: 3.0, ..upstairs };
        let every = WallSegment::at_every_height(upstairs.a, upstairs.b, 0.075);
        let one = |w: WallSegment| SightBlockers { walls: vec![w], slabs: Vec::new(), pieces: Vec::new() };
        let (eye_down, label_down) = (Vec3::new(2.0, 1.7, 5.0), Vec3::new(8.0, 1.0, 5.0));
        let (eye_up, label_up) = (Vec3::new(2.0, 4.7, 5.0), Vec3::new(8.0, 4.0, 5.0));
        assert!(!one(upstairs).blocked(eye_down, label_down), "a wall upstairs hid a label downstairs");
        assert!(one(upstairs).blocked(eye_up, label_up), "a wall upstairs hides a label beyond it upstairs");
        assert!(one(ground).blocked(eye_down, label_down), "a ground wall hides a label beyond it downstairs");
        assert!(!one(ground).blocked(eye_up, label_up), "a ground wall hid a label upstairs");
        assert!(one(every).blocked(eye_down, label_down) && one(every).blocked(eye_up, label_up), "a wall at every height hides both");
        // Looking down a stairwell, the line drops through the ground wall's top or passes over it.
        assert!(one(ground).blocked(Vec3::new(2.0, 4.7, 5.0), Vec3::new(8.0, 0.5, 5.0)), "the line crosses the ground wall at 2.6 m");
        assert!(!one(ground).blocked(Vec3::new(4.0, 4.7, 5.0), Vec3::new(8.0, 0.5, 5.0)), "the line passes over the ground wall at 3.65 m");
        assert!(!one(every).blocked(Vec3::new(2.0, 1.7, 10.0), Vec3::new(8.0, 1.0, 10.0)), "grazing the wall's end");
        assert!(wall_blocks(&ground, eye_down, label_down) && !wall_blocks(&upstairs, eye_down, label_down), "the one-wall test agrees");
    }

    /// A SOLID SLAB HIDES THE STOREY BEYOND IT, A GLASS ONE DOES NOT. Over a 10 m square at 3 m
    /// (its underside at 2.75): a label upstairs is hidden from an eye downstairs and the reverse;
    /// a glass slab is no blocker (`solid_slabs` leaves it out); a label standing on the slab is
    /// seen from upstairs (the line touches the top face, never enters); a line down the
    /// stairwell beside the slab passes; and a line from the eye under it to a label under it
    /// passes under it.
    ///
    /// Seen red first (2026-10-21) with today's rule, no slab blocks: "a solid slab hid nothing
    /// upstairs from downstairs".
    #[test]
    fn a_solid_slab_hides_the_storey_beyond_it_and_glass_does_not() {
        // Any body will do: only its slabs are read.
        let mut home = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the homestead is built in").body;
        home.floors = vec![crate::ship::home_structure::FloorSlab {
            a: (0.0, 0.0),
            b: (10.0, 10.0),
            top: 3.0,
            thickness: 0.25,
            material: SlabMaterial::Solid,
            label: None,
        }];
        let b = SightBlockers { walls: Vec::new(), slabs: solid_slabs(&home).collect(), pieces: Vec::new() };
        assert_eq!(b.slabs, vec![SlabBox { min: Vec3::new(0.0, 2.75, 0.0), max: Vec3::new(10.0, 3.0, 10.0) }]);
        let (down, up) = (Vec3::new(5.0, 1.7, 5.0), Vec3::new(4.0, 4.0, 5.0));
        assert!(b.blocked(down, up), "a solid slab hid nothing upstairs from downstairs");
        assert!(b.blocked(Vec3::new(5.0, 4.7, 5.0), Vec3::new(4.0, 1.0, 5.0)), "nor downstairs from upstairs");
        assert!(!b.blocked(Vec3::new(5.0, 4.7, 5.0), Vec3::new(7.0, 3.0, 7.0)), "a label standing on the slab");
        assert!(!b.blocked(Vec3::new(9.0, 4.7, 5.0), Vec3::new(12.0, 1.0, 5.0)), "down the stairwell beside it");
        assert!(!b.blocked(down, Vec3::new(8.0, 1.0, 5.0)), "under it, both");
        home.floors[0].material = SlabMaterial::Glass;
        assert_eq!(solid_slabs(&home).count(), 0, "a glass slab is seen through");
        // The box moves with its zone.
        let moved = SlabBox { min: Vec3::ZERO, max: Vec3::ONE }.moved(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(moved, SlabBox { min: Vec3::new(1.0, 2.0, 3.0), max: Vec3::new(2.0, 3.0, 4.0) });
    }

    /// THE ROOMS OVER AND UNDER A SOLID FLOOR DO NOT SEE EACH OTHER'S CARDS (review of increment
    /// 7, case a). In the homestead's dining room, right under the bedroom (34..42, 30..38 on both
    /// storeys), none of the bedroom's machines within the card reach shows (bed_1, desk_2, the
    /// nightstands, shelf_6 used to float at the ceiling), while the dining room's own do; and
    /// from the dressing room over the second bedroom, none of the ground floor's under it shows,
    /// while the dressing room's own do.
    ///
    /// Seen red first (2026-10-21) with today's rule (plan only, no slab): "from the dining room,
    /// the bedroom's cards show through its ceiling: ["bed_1", "nightstand_1", "nightstand_2",
    /// "rug_2", "standing_mirror_1", "desk_2", "chair_6", "shelf_6", "chair_9", "side_table_4"]".
    #[test]
    fn the_rooms_over_and_under_a_solid_floor_do_not_see_each_others_cards() {
        let w = aboard();
        let dining = w.at(36.5, 1.7, 34.0);
        assert!(w.near(dining, "room-bedroom", CARD_M) >= 6, "the bedroom's machines are over the dining room");
        let over = w.shown(dining, "room-bedroom", CARD_M);
        assert!(over.is_empty(), "from the dining room, the bedroom's cards show through its ceiling: {over:?}");
        assert_eq!(w.shown(dining, "room-dining", CARD_M).len(), w.near(dining, "room-dining", CARD_M), "the dining room's own cards all show");
        let dressing = w.at(43.5, 4.7, 34.0);
        for room in ["room-bedroom-2", "room-dining"] {
            let under = w.shown(dressing, room, CARD_M);
            assert!(under.is_empty(), "from the dressing room, {room}'s cards show through its floor: {under:?}");
        }
        assert!(w.near(dressing, "room-bedroom-2", CARD_M) >= 4, "the second bedroom is under the dressing room");
        assert_eq!(w.shown(dressing, "room-dressing", CARD_M).len(), w.near(dressing, "room-dressing", CARD_M), "the dressing room's own cards all show");
    }

    /// A WALL UPSTAIRS HIDES NOTHING DOWNSTAIRS (case b). The upstairs wall between the
    /// dressing room and the bathrooms (x 45, 30..38, from 3.0 m) stands over the second
    /// bedroom, whose ground walls at x 45 were taken out: from beside bed_2 the wardrobe, the
    /// desk and the rug across the room show.
    ///
    /// Seen red first (2026-10-21) with today's rule: "from beside bed_2, wardrobe_4 is hidden".
    #[test]
    fn a_wall_upstairs_hides_nothing_downstairs() {
        let w = aboard();
        let eye = w.at(43.0, 1.7, 36.5);
        for id in ["wardrobe_4", "desk_5", "rug_12"] {
            assert!(!w.blockers.blocked(eye, w.machine(id).2), "from beside bed_2, {id} is hidden");
        }
    }

    /// A WALL DOWNSTAIRS HIDES NOTHING UPSTAIRS (case c). Entering the study, the ground floor's
    /// wall between the shower room and the boot room (z 34, 50..55, up to the slab) is under the
    /// floor: the chair and the bookshelves across the study show.
    ///
    /// Seen red first (2026-10-21) with today's rule: "entering the study, chair_8 is hidden".
    #[test]
    fn a_wall_downstairs_hides_nothing_upstairs() {
        let w = aboard();
        let eye = w.at(53.45, 4.7, 37.4);
        for id in ["chair_8", "bookshelf_5", "bookshelf_2"] {
            assert!(!w.blockers.blocked(eye, w.machine(id).2), "entering the study, {id} is hidden");
        }
    }

    /// E REACHES NOTHING THROUGH A FLOOR OR A WALL (case 1). From the dining room, looking up at
    /// the bed over its ceiling (its label 2.5 m away), E does not offer the bed; from the
    /// dressing room, looking down at bed_2 under its floor, nor that one; from the dining room,
    /// looking through the solid wall at x 42 at bed_2 2.5 m away, nor that one. On their own
    /// floors, both beds are E's.
    ///
    /// Seen red first (2026-10-21) with today's rule (5 m and the look cone): "from the dining
    /// room, E reaches the bed upstairs: Some("bed_1")".
    #[test]
    fn e_reaches_nothing_through_a_floor_or_a_wall() {
        let w = aboard();
        let got = w.e_at(w.at(36.5, 1.7, 34.0), "bed_1");
        assert_ne!(got, Some("bed_1"), "from the dining room, E reaches the bed upstairs: {got:?}");
        let got = w.e_at(w.at(43.5, 4.7, 36.0), "bed_2");
        assert_ne!(got, Some("bed_2"), "from the dressing room, E reaches the bed downstairs: {got:?}");
        let got = w.e_at(w.at(41.2, 1.7, 34.0), "bed_2");
        assert_ne!(got, Some("bed_2"), "from the dining room, E reaches the bed through the wall: {got:?}");
        assert_eq!(w.e_at(w.at(37.5, 4.7, 34.0), "bed_1"), Some("bed_1"), "upstairs, the bed upstairs");
        assert_eq!(w.e_at(w.at(45.5, 1.7, 34.0), "bed_2"), Some("bed_2"), "downstairs, the bed downstairs");
    }

    /// A home's storeys as `storey_floor` reads them, over the whole plane: the storeys
    /// `HomeStructure::storeys` derives from slabs whose tops are `tops` (any body will do: only
    /// its slabs and its 6 m height are read), and the floor of the one a point is on.
    fn storeys_of(tops: &[f32]) -> impl Fn(Vec3, Foot) -> Option<f32> {
        use crate::ship::home_structure::{storey_index, FloorSlab};
        let mut home = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the homestead is built in").body;
        home.floors = tops
            .iter()
            .map(|&top| FloorSlab { a: (-50.0, -50.0), b: (50.0, 50.0), top, thickness: 0.25, material: SlabMaterial::Glass, label: None })
            .collect();
        let storeys = home.storeys();
        move |p: Vec3, _: Foot| Some(storeys[storey_index(&storeys, p.y)].floor)
    }

    /// E from `eye` (feet 1.7 m under it) looking straight at the first of `machines`.
    fn e_from(eye: Vec3, machines: &[(Vec3, f32)], storeys: impl Fn(Vec3, Foot) -> Option<f32>) -> Option<usize> {
        walk_up_target(eye, eye.y - 1.7, (machines[0].0 - eye).normalize(), machines.iter().copied(), &SightBlockers::default(), storeys)
    }

    /// THE STOREY OF A MACHINE IS WHERE IT STANDS, BY THE HOME'S OWN STOREYS. A 2.4 m tower's
    /// label is 2.8 m over its floor, past the 2.5 m a thing on one's own storey may rise: from
    /// beside it, E still reaches it; from the floor above, 0.2 m over its label, E does not. A
    /// unit hung under the ceiling (2.5 or 2.7 m up) is on the storey under that ceiling. In a
    /// home of 2.5 m storeys, a machine on the floor above (2.5 m up), seen through a glass floor
    /// or a stairwell, is not E's from the floor below, though a 3 m rule of thumb would put it
    /// on the same storey. Where no home says, the relay's own band at the foot.
    ///
    /// Seen red first (2026-10-21) four times: with today's rule (no storey at all) "from the
    /// floor above, E reaches the tower under it" (left: Some(0), right: None); with the storey
    /// tested at the LABEL (`moves::in_reach(eye, label, 5.0)`) "from beside it, E misses a 2.4 m
    /// tower" (left: None, right: Some(0)); with it tested at the foot by `moves::on_my_storey`
    /// (2.5 m over the feet at most) "from the floor under it, E misses a unit hung 2.7 m up"
    /// (left: None, right: Some(0)); and with the home's storeys ignored (the relay's band
    /// alone) "in a home of 2.5 m storeys, E reaches the floor above" (left: Some(0), right:
    /// None).
    #[test]
    fn the_storey_of_a_machine_is_where_it_stands() {
        let three = || storeys_of(&[3.0]);
        let tower = [(Vec3::new(2.0, 2.8, 0.0), 0.0)];
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &tower, three()), Some(0), "from beside it, E misses a 2.4 m tower");
        assert_eq!(e_from(Vec3::new(0.0, 4.7, 0.0), &tower, three()), None, "from the floor above, E reaches the tower under it");
        // A home of 2.5 m storeys: a desk on the floor above, its foot 2.5 m over the feet below.
        let above = [(Vec3::new(1.5, 3.4, 0.0), 2.5)];
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &above, storeys_of(&[2.5])), None, "in a home of 2.5 m storeys, E reaches the floor above");
        assert_eq!(e_from(Vec3::new(0.0, 4.2, 0.0), &above, storeys_of(&[2.5])), Some(0), "on that floor, E reaches it");
        // A machine hung under the ceiling: the greenhouse's air handlers stand 2.7 m up
        // (data/machines/home.ron), the plant room's 2.5 m, labels 3.3 m and 3.1 m up. From the
        // floor under them E reaches them; a unit standing on the floor above (3.0 m), it does not.
        for foot in [2.5, 2.7] {
            let unit = [(Vec3::new(1.5, foot + 0.64, 0.0), foot)];
            assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &unit, three()), Some(0), "from the floor under it, E misses a unit hung {foot} m up");
        }
        let upstairs = [(Vec3::new(1.5, 3.64, 0.0), 3.0)];
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &upstairs, three()), None, "a unit on the floor above");
        // Where no home says (a point in no zone), the relay's band at the foot: up to 2.5 m.
        let nowhere = || |_: Vec3, _: Foot| None::<f32>;
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &[(Vec3::new(1.5, 2.8, 0.0), 2.4)], nowhere()), Some(0), "no home: 2.4 m up");
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &upstairs, nowhere()), None, "no home: 3.0 m up");
        // Out of reach, or out of the cone, as ever.
        let none = SightBlockers::default();
        assert_eq!(walk_up_target(Vec3::new(-4.0, 1.7, 0.0), 0.0, Vec3::X, tower, &none, three()), None, "6 m away");
        assert_eq!(walk_up_target(Vec3::new(0.0, 1.7, 0.0), 0.0, Vec3::Z, tower, &none, three()), None, "looking away");
    }

    /// The REAL floor under a fixture whose foot is at `foot` over (x, z) (ship frame): the highest
    /// floor at or under its foot there that a body stands on (`Footing::floor_under`, measured so
    /// it reaches no higher than the foot), so a unit hung under a slab is over the room's floor,
    /// not the slab's top. Where the footing finds nothing (a doorway's cells belong to no room, and
    /// nothing else stands there), the home's own floor, `ground`.
    #[cfg(feature = "native")]
    fn floor_under_fixture(footing: &crate::engine::footing::Footing, x: f32, foot: f32, z: f32, ground: f32) -> f32 {
        let step = crate::engine::footing::STEP_UP_M;
        footing.floor_under(Vec3::new(x, foot + 1.7, z), foot - step + 0.01).unwrap_or(ground)
    }

    /// EVERY SHIPPED MACHINE IS E'S FROM ITS OWN FLOOR, AND SEEN FROM IT. For each machine of
    /// data/machines/home.ron, standing on the real floor under it (`Footing::floor_under`, the
    /// highest a body stands on at or under its foot: the seams review of 2026-10-21, finding 4,
    /// found this test reading "its own floor" off the storey rule under test, which put the eye
    /// 4.7 m up in the greenhouse's air for a unit hung 3 m or more up there), half a metre from
    /// it towards the middle of the home and looking at it, with nothing else about: E reaches it
    /// (`storey_floor` puts both on one storey), and no solid slab stands between that eye and its
    /// label (a label poking up into or through the floor over its own room would hide it from
    /// that room). A unit hung higher than its storey, or a label that pierces the slab over it,
    /// fails here and not on the operator's walk-through.
    ///
    /// Seen red first (2026-10-21) twice: in an earlier form, which read the storey by a fixed
    /// band over the feet, with that band at the relay's 2.5 m, "air_handler_g1 stands 2.70 m
    /// over its floor at 0.0, out of the walk-up's storey band"; and in this form, with the
    /// home's storeys ignored (the relay's band alone), "from its own floor at 0.0, E misses
    /// air_handler_g1 (its foot 2.70 m over it)".
    #[cfg(feature = "native")]
    #[test]
    fn every_shipped_machine_is_es_from_its_own_floor_and_seen_from_it() {
        let w = aboard();
        assert!(w.storeys.len() >= 2, "the homestead has an upper floor");
        let feet = crate::engine::footing::test_aboard::Aboard::of(w.ship.clone());
        let footing = feet.footing();
        let home = &w.ship.zones[w.ship.home_zone_index()].body;
        let middle = w.at(home.width * 0.5, 0.0, home.depth * 0.5);
        for (id, _room, label, foot) in &w.machines {
            let local = foot - w.origin.y;
            let floor = floor_under_fixture(&footing, label.x, *foot, label.z, w.origin.y) - w.origin.y;
            let toward = (Vec3::new(middle.x - label.x, 0.0, middle.z - label.z)).normalize_or_zero() * 0.5;
            let eye = Vec3::new(label.x + toward.x, w.origin.y + floor + 1.7, label.z + toward.z);
            let got = e_from(eye, &[(*label, *foot)], |p: Vec3, foot: Foot| storey_floor(&w.ship, p, foot));
            assert_eq!(got, Some(0), "from its own floor at {floor:.1}, E misses {id} (its foot {:.2} m over it)", local - floor);
            let slab = w.blockers.slabs.iter().any(|s| s.blocks(eye, *label));
            assert!(!slab, "{id}'s label at {:.2} m is behind a solid slab from its own floor at {floor:.1}", label.y - w.origin.y);
        }
    }

    /// THE WALL OVER AN OPENING HIDES WHAT IS BEHIND IT; THE OPENING DOES NOT (the seams review of
    /// 2026-10-21, finding 3). The plot's front-door corridor cuts the homestead's east wall (x 55)
    /// over z 39 to 41 from the floor to 2.2 m, and the shell stands over the cut to the roof, beside
    /// the upper hall. From the upper hall at (53, 4.7, 39.8), a nameplate over a person in that
    /// corridor at (58, 2.0, 39.8) is behind that shell (the line meets x 55 at 3.6 m) and hidden;
    /// from the entry under it at (53, 1.7, 39.8) the same nameplate is seen through the doorway.
    ///
    /// Seen red first (2026-10-21) with every opening of the sight lines open at every height
    /// (`sight_segments_with_shell_cuts` before this fix): "from the upper hall, the nameplate in
    /// the front door's corridor shows through the shell over the door".
    #[test]
    fn the_wall_over_an_opening_hides_what_is_behind_it() {
        let w = aboard();
        let nameplate = w.at(58.0, 2.0, 39.8);
        assert!(w.blockers.blocked(w.at(53.0, 4.7, 39.8), nameplate), "from the upper hall, the nameplate in the front door's corridor shows through the shell over the door");
        assert!(!w.blockers.blocked(w.at(53.0, 1.7, 39.8), nameplate), "from the entry, the nameplate in the front door's corridor is hidden by its doorway");
    }

    /// A UNIT HUNG HIGH IN A ROOM OPEN TO THE ROOF IS ON THE FLOOR UNDER IT (the seams review of
    /// 2026-10-21, finding 4). The greenhouse (x 10 to 55, z 51 to 73) is open to the roof, 6 m, with
    /// no slab of the upper floor over it. A unit hung with its foot 4.0 m up at (30, 60), its label
    /// 1.0 m over that, belongs to the greenhouse floor: from that floor (`Footing::floor_under`),
    /// half a metre from it and looking up at it, E reaches it. A unit standing on the upper floor's
    /// slab is still on the upper floor: from the dining room under the bedroom, E does not reach
    /// one standing in the bedroom. A body's feet are on the storey of their height wherever they
    /// are (`Foot::Body`): a rider on the elevator's car at the top of its shaft is upstairs.
    ///
    /// Seen red first (2026-10-21) with `storey_floor` taking the storey from the slab tops alone,
    /// wherever the point is in plan: "from the greenhouse floor at 0.0, E misses a unit hung 4.0 m
    /// up (left: None, right: Some(0))".
    #[cfg(feature = "native")]
    #[test]
    fn a_unit_hung_high_in_a_room_open_to_the_roof_is_on_the_floor_under_it() {
        let w = aboard();
        let feet = crate::engine::footing::test_aboard::Aboard::of(w.ship.clone());
        let footing = feet.footing();
        let foot = w.origin.y + 4.0;
        let label = w.at(30.0, 5.0, 60.0);
        let floor = floor_under_fixture(&footing, label.x, foot, label.z, w.origin.y);
        assert!((floor - w.origin.y).abs() < 1e-4, "the greenhouse floor under the unit is at {floor}");
        let eye = Vec3::new(label.x, floor + 1.7, label.z + 0.5);
        let got = e_from(eye, &[(label, foot)], |p: Vec3, foot: Foot| storey_floor(&w.ship, p, foot));
        assert_eq!(got, Some(0), "from the greenhouse floor at {:.1}, E misses a unit hung 4.0 m up", floor - w.origin.y);
        // A unit on the bedroom's floor (the slab at 3.0), from the dining room under it.
        let upstairs = (w.at(38.0, 3.9, 34.0), w.origin.y + 3.0);
        assert_eq!(e_from(w.at(38.0, 1.7, 34.6), &[upstairs], |p: Vec3, foot: Foot| storey_floor(&w.ship, p, foot)), None, "from the dining room, E reaches a unit in the bedroom over it");
        // A body's feet are held up by what they stand on: on the elevator's car at the top of its
        // shaft (31, 26), where no slab is, the rider is upstairs; a fixture there is not.
        let on_car = w.at(31.0, 3.0, 26.0);
        assert_eq!(storey_floor(&w.ship, on_car, Foot::Body), Some(w.origin.y + 3.0), "a rider on the car at the top of its shaft");
        assert_eq!(storey_floor(&w.ship, on_car, Foot::Fixture), Some(w.origin.y), "a fixture hung 3 m up in the shaft");
    }

    /// A BUILT WALL PIECE AND A STAIR HIDE WHAT IS BEHIND THEM (the increment 7 follow-ups,
    /// 2026-10-22). Placed boxes and stairs were no sight blockers, so a label behind a wall piece
    /// built with the editor, or on the far side of a flight, showed through it. In a 12 x 12 m
    /// body: the wall piece (0.15 x 3 x 1) at (6, 0, 2) hides a label across it at the eye's height,
    /// and a line passing over its 3 m top is seen; turned 90 degrees the wall piece hides along
    /// the other axis. A flight (stairs, 1.2 x 3 x 4.5 in 16 steps) at (6, 0, 8) climbing north to
    /// south (its high end at z 10.25, 3.0 m up): a label beyond its high end at the floor is
    /// hidden from the far side; a line across its first steps at the eye's height passes over
    /// them; one across its top steps at 2.5 m is hidden. A ramp (0.4 m at its high end) hides a
    /// line along the floor through its high end and not one over it. A label standing on a deck
    /// is seen (the line only touches its top).
    ///
    /// Seen red first (2026-10-22) with the pieces not counted (`SightBlockers::blocked` reading the
    /// walls and the slabs alone): "the wall piece hid nothing behind it".
    #[test]
    fn a_built_wall_piece_and_a_stair_hide_what_is_behind_them() {
        let home: HomeStructure = ron::from_str(
            "(width: 12.0, depth: 12.0, height: 6.0, structures: [ \
               (id: \"wall-1\", type_id: \"wall\", pos: (6.0, 0.0, 2.0)), \
               (id: \"wall-2\", type_id: \"wall\", pos: (2.0, 0.0, 6.0), rot_deg: 90.0), \
               (id: \"stairs-1\", type_id: \"stairs\", pos: (6.0, 0.0, 8.0)), \
               (id: \"ramp-1\", type_id: \"ramp\", pos: (10.0, 0.0, 6.0)), \
               (id: \"deck-1\", type_id: \"deck\", pos: (2.0, 0.0, 10.0))])",
        )
        .expect("the test body parses");
        let b = SightBlockers { walls: Vec::new(), slabs: Vec::new(), pieces: solid_pieces(&home).collect() };
        assert_eq!(b.pieces.len(), 5, "the two wall pieces, the flight, the ramp and the deck");
        assert!(b.blocked(Vec3::new(4.0, 1.7, 2.0), Vec3::new(8.0, 1.2, 2.0)), "the wall piece hid nothing behind it");
        assert!(!b.blocked(Vec3::new(4.0, 1.7, 2.0), Vec3::new(8.0, 4.5, 2.0)), "a line over the wall piece's top (3.1 m at it) is hidden");
        assert!(!b.blocked(Vec3::new(4.0, 1.7, 3.0), Vec3::new(8.0, 1.2, 3.0)), "past the wall piece's end, a label is hidden");
        assert!(b.blocked(Vec3::new(2.0, 1.7, 4.0), Vec3::new(2.0, 1.2, 8.0)), "turned 90 degrees, the wall piece hid nothing behind it");
        assert!(b.blocked(Vec3::new(6.0, 1.7, 11.5), Vec3::new(6.0, 0.3, 4.0)), "the flight hid nothing beyond its high end");
        assert!(!b.blocked(Vec3::new(4.0, 1.7, 6.0), Vec3::new(8.0, 1.7, 6.0)), "a line over the flight's first steps is hidden");
        assert!(b.blocked(Vec3::new(4.0, 2.5, 10.0), Vec3::new(8.0, 2.5, 10.0)), "a line through the flight's top steps is seen");
        assert!(b.blocked(Vec3::new(10.0, 0.2, 10.0), Vec3::new(10.0, 0.2, 2.0)), "the ramp hid nothing along the floor through its high end");
        assert!(!b.blocked(Vec3::new(10.0, 1.7, 10.0), Vec3::new(10.0, 1.0, 2.0)), "a line over the ramp is hidden");
        assert!(!b.blocked(Vec3::new(0.5, 1.7, 10.0), Vec3::new(2.0, 0.2, 10.0)), "a label standing on the deck is hidden by it");
        // The pieces move with their zone, as the slabs do.
        let moved = b.pieces[0].moved(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(moved.foot, Vec3::new(7.0, 2.0, 5.0));
    }

    /// THE SIGHT TEST IS WIRED IN: lib.rs builds the blockers with the solid slabs and the shut
    /// doors' heights and picks the walk-up target with `walk_up_target`, and the HUD tests every
    /// machine label and crew nameplate with `SightBlockers::blocked`, never the plan-only test.
    /// Without these the functions above are tested and never run.
    ///
    /// Seen red first (2026-10-21) before the wiring: "src/lib.rs never has
    /// crate::ship::sight::walk_up_target(". The solid pieces (2026-10-22) the same way, before
    /// lib.rs added them: "src/lib.rs never has crate::ship::sight::ship_solid_pieces(".
    #[test]
    fn the_sight_test_is_wired_into_the_walk_up_and_the_hud() {
        let lib = include_str!("../lib.rs").replace("\r\n", "\n");
        for wired in [
            "crate::ship::sight::walk_up_target(",
            "crate::ship::sight::ship_solid_slabs(",
            "crate::ship::sight::ship_solid_pieces(",
            "crate::ship::wall_collision::door_panel_segment(p.center + off",
        ] {
            assert!(lib.contains(wired), "src/lib.rs never has {wired}");
        }
        let hud = include_str!("../gui/pages/hud.rs").replace("\r\n", "\n");
        assert_eq!(hud.matches("state.sight_blockers.blocked(").count(), 2, "the HUD tests the machine labels and the crew nameplates in 3-D");
        assert!(!hud.contains("wall_collision::sight_blocked("), "the HUD still tests a label in plan only");
    }
}

//! First-person wall + door collision for the home_structure (v0.556).
//!
//! The player IS the camera here -- `renderer::camera` writes the camera position directly and
//! rapier3d's `PhysicsWorld` is dormant (never stepped). So rather than wake a heavy rigid-body
//! system for "the camera should not pass through a wall," we resolve collision GEOMETRICALLY: each
//! wall / perimeter edge / closed door becomes a thin 2D segment with a half-thickness, and we push
//! the camera's horizontal (XZ) position out of any segment it penetrates within the player's radius,
//! preserving the along-wall component so you SLIDE along walls instead of sticking.
//!
//! Doorways (open, unlocked doors) are gaps you walk through; windows and closed/locked doors block
//! (you cannot walk through glass, and a shut door is a wall). The push-out never moves Y.
//!
//! BY HEIGHT (ship homes increment 7, docs/design/ship-homes-increment-7-plan.md, lane M): a segment
//! stops a body only where it stands across the body's knee-to-eye band (`WallSegment::stops_body`),
//! the rule built pieces already followed. So a wall upstairs stops a body upstairs and lets the one
//! downstairs walk under it, a ground wall rising through the slab stops both, and a door shut on one
//! storey is no wall on the other.
//!
//! WHAT A SEGMENT IS (ship homes increment 7 review, 2026-10-21): every segment has a
//! `SegmentKind`, which says which bodies it stops. A WALL stops the knee-to-eye band, as above. The
//! wall over an opening (a door's head, the shell over a corridor's or a plot door's cut) is a
//! LINTEL: an opening is cut only from its foot to its head, never at every height, so a body a
//! storey up walks into the wall over a door below it, and a body passing through the doorway
//! ducks under it (a jump through a 2.2 m door is not shoved sideways). A stair's STEP stops a body
//! whose feet are lower than its top less the step up, the footing's own rule, so a flight is
//! climbed from its foot and walked into from its side, its back and under its high end. A GUARD (a
//! balustrade) stops any body whose feet are under its rail plus `GUARD_CLEAR_M`, so a jump does
//! not carry a body over it.
//!
//! THE SWEEP (review finding 4): `resolve` moves the body along its step and stops it at the first
//! segment it touches, then slides it along that segment with what is left of the step (the move
//! and slide every first-person game's character uses), instead of pushing it out of whatever it
//! landed in three times over. A body never passes a gap narrower than itself, and where a walk
//! ends no longer depends on how many frames a second cut it into.

use crate::ship::home_structure::{HomeStructure, OpeningKind, PlacedStructure, ShellCut};
use crate::ship::structure::{self, MeshShape, StructureKind, StructureType};
use glam::{Quat, Vec3};

/// Player capsule radius (metres) for the horizontal push-out.
pub const PLAYER_RADIUS: f32 = 0.3;

/// How high a standing body's eye is over its feet, metres: the walking camera's
/// (`renderer::camera::CameraController::eye_height`, held to this by a test in engine/footing.rs).
/// Every point the sweep is given is that eye, so the feet are this far under it.
pub const EYE_HEIGHT_M: f32 = 1.7;

/// The knee, metres over the feet: a blocker whose top is no higher than this is stepped over, not
/// walked into (a foundation, a floor tile, the slab a body stands on). The built pieces' rule
/// (`engine::build_place::built_piece_segments`) from before storeys, now every segment's.
pub const KNEE_M: f32 = 0.4;

/// The highest a body steps up in one go, metres: the footing's (`engine::footing::STEP_UP_M`,
/// held to this by a test below; the footing is in the desktop build only and this file is in the
/// relay's too, so the number is written twice). A stair's step stops a body whose feet are lower
/// than the step's top less this (`SegmentKind::Step`), and the footing stands a body on any step
/// no higher than this over the floor it rests on, so the two agree on which steps are climbed.
pub const STEP_UP_M: f32 = 0.6;

/// How far over a guard's rail a body's feet go before the guard stops holding them, metres (ship
/// homes increment 7 review, finding 3, decided): a jump lifts the feet 1.27 m at 1 g, over the
/// homestead's 1.0 m guards by 0.27 m, so a guard holds a body whose feet are under its rail plus
/// this (`SegmentKind::Guard`) and a jump does not carry anyone over it. Climbing over a rail on
/// purpose (a hand on the rail, a leg over, the other leg) would be an action of its own, not a
/// walk or a jump; it does not exist yet.
pub const GUARD_CLEAR_M: f32 = 0.3;

/// A move longer than this in one `resolve`, metres, is a PLACEMENT (a teleporter, a respawn, the
/// rig putting the body somewhere), not a walk: it is settled where it lands instead of swept from
/// where it began, so a wall between the two does not stop it. Far over the longest frame's walk
/// (a sprint is 0.95 m in the 0.1 s the main loop clamps a frame to), and the length the sweep
/// had given up at before (48 substeps of 0.15 m).
pub const PLACEMENT_M: f32 = 7.2;

/// The slices a ramp is cut into for its collision, metres along its run at most: each slice
/// stops a body as a step as tall as the ramp at its high side.
const RAMP_SLICE_M: f32 = 0.25;

/// How far off a segment the sweep leaves a body it stopped, metres, so the next step starts clear
/// of it.
const SKIN_M: f32 = 1e-4;

/// The most times one sweep slides off what it touched. Three is a wall, a second wall in a
/// corner and the slide out of the corner; one more for a slide that touches a third.
const MAX_SLIDES: usize = 4;

/// The most passes a push-out makes to settle a body that starts inside segments (a door shut on
/// it, a placement into a wall), and how little a pass may still move it to count as settled.
const SETTLE_PASSES: usize = 32;
const SETTLED_M: f32 = 1e-5;

/// What a segment is, which says which bodies it stops (`WallSegment::stops_body`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    /// A wall, a shut door, a gate, a corridor's side, a built piece: it stops a body whose
    /// knee-to-eye band (`KNEE_M` over the feet to the eye) it stands across. A blocker under the
    /// knee is stepped over; one over the eye is walked under.
    Wall,
    /// The wall over an opening: a door's head, the shell over a corridor's or a plot door's cut.
    /// It stops a body whose KNEE is at its height, a body standing a storey up beside it; a body
    /// whose knee is under it is passing through the opening and ducks under it, so a jump through
    /// a doorway is not shoved sideways by the wall over the door (a horizontal sweep has no way to
    /// bump a head and let the body on through). A jump INTO a room with a floor over it is still
    /// held at the doorway, by that floor's edge (`engine::footing::slab_edges`), while the head is
    /// over the floor's underside: with door heads of 2.1 to 2.4 m under a 2.75 m ceiling that is
    /// the head meeting the head of the door, so the two do not disagree: the lintel lets a jump out
    /// through a door into a space with no floor over it (the corridor beyond the front door), and
    /// the floor's edge holds one going in under a floor
    /// (`a_running_jump_in_through_the_front_door_is_held_at_the_doorway`).
    Lintel,
    /// A face of a stair's step (or a ramp's slice), standing from the stair's foot to the step's
    /// top: it stops a body whose feet are lower than that top less `STEP_UP_M`, the step the
    /// footing will not stand it on. Each step a riser above the last is stood on, so a flight is
    /// climbed from its foot; its side, its back and the solid under its high end stop a body that
    /// cannot step onto them.
    Step,
    /// A guard (a balustrade): it stops any body whose feet are under its rail plus
    /// `GUARD_CLEAR_M` and whose eye is over its foot, so a jump does not clear it, and a body on
    /// the storey below walks under it.
    Guard,
}

/// A blocking wall segment in the XZ plane (a..b), inflated by `half_thickness`, standing between
/// the world heights `y0` and `y1`.
#[derive(Clone, Copy, Debug)]
pub struct WallSegment {
    pub a: (f32, f32),
    pub b: (f32, f32),
    pub half_thickness: f32,
    /// The world heights the segment stands between, its foot and its top (ship homes increment 7,
    /// Wave 0 of docs/design/ship-homes-increment-7-plan.md). Every builder fills them: a home's
    /// wall from its base to its top, the perimeter from the floor to the roof, a corridor's sides
    /// from its deck to its lid, a door panel from its sill to its head, a built piece its box.
    /// The sweep reads them (`stops_body`): which bodies a segment stops at those heights is its
    /// `kind`'s rule. A blocker whose heights are not known stands at every height
    /// (`WallSegment::at_every_height`), which is what every segment did before it had them.
    pub y0: f32,
    pub y1: f32,
    /// What it is, which says which bodies it stops (review of increment 7): a wall, the wall over
    /// an opening, a stair's step or a guard. Every blocker made outside this file is a `Wall`.
    pub kind: SegmentKind,
}

impl WallSegment {
    /// True when this segment stops a body whose eye is at world height `eye_y` (its feet
    /// `EYE_HEIGHT_M` under it), by its kind's rule (`SegmentKind`). A WALL stands somewhere
    /// across the body's knee-to-eye band: a wall upstairs (3..6) is over the head of a body on the
    /// ground floor (band 0.4..1.7) and a ground wall (0..3) under the knees of one upstairs (band
    /// 3.4..4.7); a wall from the ground to the roof stops both. A LINTEL stands at the knee, a
    /// STEP's top is more than a step up over the feet, a GUARD's rail is less than
    /// `GUARD_CLEAR_M` under them. A segment at every height stops every body.
    pub fn stops_body(&self, eye_y: f32) -> bool {
        let feet = eye_y - EYE_HEIGHT_M;
        let knee = feet + KNEE_M;
        match self.kind {
            SegmentKind::Wall => self.y0 < eye_y && self.y1 > knee,
            SegmentKind::Lintel => self.y0 < knee && self.y1 > knee,
            SegmentKind::Step => self.y0 < eye_y && self.y1 > feet + STEP_UP_M,
            SegmentKind::Guard => self.y0 < eye_y && self.y1 + GUARD_CLEAR_M > feet,
        }
    }

    /// A segment standing at every height (`y0` minus infinity, `y1` plus infinity): for a blocker
    /// whose heights are not known, which then stops a body at any height.
    pub fn at_every_height(a: (f32, f32), b: (f32, f32), half_thickness: f32) -> Self {
        WallSegment { a, b, half_thickness, y0: f32::NEG_INFINITY, y1: f32::INFINITY, kind: SegmentKind::Wall }
    }

    /// This segment moved by `by`: a body's walls (body-local) placed at its origin in the world.
    pub fn moved(self, by: Vec3) -> Self {
        WallSegment {
            a: (self.a.0 + by.x, self.a.1 + by.z),
            b: (self.b.0 + by.x, self.b.1 + by.z),
            y0: self.y0 + by.y,
            y1: self.y1 + by.y,
            ..self
        }
    }
}

/// The blocking segment of a shut door panel (`ship::door_panels::PanelPlacement`'s closed pose):
/// along the wall through the panel's centre, its width long, at least 0.05 m either side of the
/// wall line, standing from the panel's bottom (`center.y`, the sill on the wall's base) to its
/// head. The one conversion the walk (lib.rs, the live door list) and the tests use, so a door
/// shut on one storey is no wall on another (`WallSegment::stops_body`).
pub fn door_panel_segment(center: Vec3, rotation: Quat, size: Vec3) -> WallSegment {
    let half_w = size.x * 0.5;
    let dir = rotation * Vec3::X;
    WallSegment {
        a: (center.x - dir.x * half_w, center.z - dir.z * half_w),
        b: (center.x + dir.x * half_w, center.z + dir.z * half_w),
        half_thickness: (size.z * 0.5).max(0.05),
        y0: center.y,
        y1: center.y + size.y,
        kind: SegmentKind::Wall,
    }
}

/// Build the STATIC blocking segments from a home (and every guard piece placed in it on its own,
/// ship homes increment 7, and every stair): the 4 perimeter walls + each interior wall's SOLID
/// pier spans. DOOR apertures are cut out from their foot to their head (so a doorway is a
/// walk-through gap, with the wall over it a lintel); WINDOW spans stay solid (you can't walk
/// through glass -- the pane + the sill wall block the whole width). Doors get a separate LIVE
/// collider in `resolve`'s `doors` arg, because their open state changes every frame.
pub fn wall_segments(home: &HomeStructure) -> Vec<WallSegment> {
    wall_segments_with_shell_cuts(home, &[])
}

/// `wall_segments` with corridor APERTURES cut out of the perimeter (ship-superstructure increment
/// B, the collision twin of `generate_meshes_with_shell_cuts`): where a corridor tube meets the
/// box, the perimeter gets a door-width walk-through gap instead of a solid wall, from the cut's
/// foot (`ShellCut::base`) to its head (`base + height`), as the mesh opens it; over the head the
/// shell stands (ship homes increment 7 review, finding 1: upstairs over the homestead's front door
/// a walk used to go out through the shell and fall 3 m into the corridor). With no cuts this is
/// exactly the pre-B path.
pub fn wall_segments_with_shell_cuts(home: &HomeStructure, shell_cuts: &[ShellCut]) -> Vec<WallSegment> {
    segments_impl(home, shell_cuts, false)
}

/// SIGHT blockers for one home (v0.975, nameplate-through-wall polish): same spans as
/// `wall_segments_with_shell_cuts` but with WINDOW apertures cut out too -- you can't walk
/// through glass, but you can SEE through it, so a machine card behind a window stays
/// visible while one behind a solid pier hides. Door apertures are open here exactly as in
/// the collision spans; a CLOSED door blocks sight via the live door list the caller adds
/// (the same split `resolve` uses). A guard (a balustrade, `InteriorWall::guard`, or a guard
/// piece placed on its own) is see-through and blocks no sight (ship homes increment 7), and a
/// stair hides nothing. The sight test reads heights (`ship::sight`, since increment 7's review), so
/// every opening is cut only from its foot to its head, as for the walk: the wall over a door or a
/// window, the wall under a window's sill and the shell over a corridor's or a plot door's cut all
/// stand with their heights and hide what is behind them (the seams review of 2026-10-21, finding
/// 3: from the upper hall, nameplates in the front door's corridor showed through the shell over
/// the door), while the door's gap and the window's glass are seen through.
pub fn sight_segments_with_shell_cuts(home: &HomeStructure, shell_cuts: &[ShellCut]) -> Vec<WallSegment> {
    segments_impl(home, shell_cuts, true)
}

/// One opening in a straight run of wall: its span along the run, metres from the run's start,
/// and the heights it opens, world (body-local) metres, its foot to its head.
struct Opened {
    from: f32,
    to: f32,
    y0: f32,
    y1: f32,
}

fn segments_impl(home: &HomeStructure, shell_cuts: &[ShellCut], sight: bool) -> Vec<WallSegment> {
    let mut segs = Vec::new();
    let (w, d) = (home.width.max(1.0), home.depth.max(1.0));
    // The perimeter stands from the body's floor to its roof (the height the mesh draws it to).
    let (py0, py1) = (0.0, home.height.max(1.0));
    let st = home.shell_resolved_thickness() * 0.5;
    // Perimeter edges in the same winding `ShellCut.edge` indexes (0: z=0, 1: x=w, 2: z=d, 3: x=0).
    for (ei, (a, b)) in [
        ((0.0, 0.0), (w, 0.0)),
        ((w, 0.0), (w, d)),
        ((w, d), (0.0, d)),
        ((0.0, d), (0.0, 0.0)),
    ]
    .into_iter()
    .enumerate()
    {
        let len = if ei % 2 == 0 { w } else { d };
        // Each cut opens the shell from its foot to its head (the mesh's synthetic door,
        // `generate_meshes_with_shell_cuts`), and nowhere else.
        let cuts: Vec<Opened> = shell_cuts
            .iter()
            .filter(|c| c.edge == ei)
            .map(|c| Opened { from: c.at, to: c.at + c.width, y0: py0 + c.base, y1: py0 + c.base + c.height })
            .collect();
        wall_run(&mut segs, a, b, len, st, (py0, py1), SegmentKind::Wall, &cuts);
    }
    for wall in &home.walls {
        // A GUARD is a balustrade (ship homes increment 7, `InteriorWall::guard`): posts and
        // balusters under 100 mm apart and a top rail. It stops a walk (a jump too: `SegmentKind::
        // Guard`), but it is see-through, so it is no sight blocker: a 1.0 m balustrade round a
        // stairwell does not hide the nameplates beyond it.
        let guard = wall.guard().is_some();
        if sight && guard {
            continue;
        }
        let ht = wall.resolved_thickness() * 0.5;
        let (wy0, wy1) = wall.y_range();
        let len = ((wall.b.0 - wall.a.0).powi(2) + (wall.b.1 - wall.a.1).powi(2)).sqrt();
        if len < 1e-4 {
            continue;
        }
        // DOOR openings along the wall, each from its sill to its head. For COLLISION windows are
        // NOT cut (glass blocks the walker); for SIGHT they are (glass is transparent).
        let openings: Vec<Opened> = wall
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door || (sight && o.kind == OpeningKind::Window))
            .map(|o| Opened { from: o.at, to: o.at + o.width, y0: wy0 + o.sill, y1: wy0 + o.sill + o.height })
            .collect();
        let kind = if guard { SegmentKind::Guard } else { SegmentKind::Wall };
        wall_run(&mut segs, wall.a, wall.b, len, ht, (wy0, wy1), kind, &openings);
    }
    // The pieces a walk is stopped by (not sight blockers: a balustrade is see-through, and a stair
    // hides no nameplate on the floor plan): a GUARD placed on its own, and every STAIR.
    if !sight {
        for ps in &home.structures {
            let Some(ty) = structure::structure_type(&ps.type_id) else { continue };
            match ty.kind {
                StructureKind::Guard => segs.push(guard_piece_segment(ps, ty)),
                StructureKind::Stairs => stair_faces(&mut segs, ps, ty),
                _ => {}
            }
        }
    }
    segs
}

/// A straight run of wall from `a` to `b` (`len` long, `ht` half thick, standing from `heights.0`
/// to `heights.1`) with its `opened` openings cut out, as segments of `kind`. An opening is cut
/// from its foot to its head only: the wall over it stands as a `SegmentKind::Lintel`, the wall
/// under it (a raised opening's sill) as the run's own kind. The same for the walk and for SIGHT,
/// which reads heights too (`ship::sight`; it opened every opening at every height until the seams
/// review of 2026-10-21, finding 3). A solid piece no longer than 2 cm (a sliver between two
/// openings) is left out, as it always was.
#[allow(clippy::too_many_arguments)]
fn wall_run(
    segs: &mut Vec<WallSegment>,
    a: (f32, f32),
    b: (f32, f32),
    len: f32,
    ht: f32,
    heights: (f32, f32),
    kind: SegmentKind,
    opened: &[Opened],
) {
    let (y0, y1) = heights;
    let (ux, uz) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let at = |t: f32| (a.0 + ux * t, a.1 + uz * t);
    if opened.is_empty() {
        segs.push(WallSegment { a, b, half_thickness: ht, y0, y1, kind });
        return;
    }
    // Where along the run what is solid changes: its ends and every opening's sides.
    let mut marks: Vec<f32> = vec![0.0, len];
    for o in opened {
        marks.push(o.from.clamp(0.0, len));
        marks.push(o.to.clamp(0.0, len));
    }
    marks.sort_by(f32::total_cmp);
    marks.dedup_by(|x, y| (*x - *y).abs() < 1e-4);
    // The heights that stand over each piece between two marks, a piece merged into the one
    // before it when the same heights stand over both.
    let mut runs: Vec<(f32, f32, Vec<(f32, f32)>)> = Vec::new();
    for pair in marks.windows(2) {
        let (s, e) = (pair[0], pair[1]);
        let mid = (s + e) * 0.5;
        let over: Vec<&Opened> = opened.iter().filter(|o| o.from < mid && mid < o.to).collect();
        let standing = if over.is_empty() { vec![(y0, y1)] } else { heights_left((y0, y1), &over) };
        match runs.last_mut() {
            Some(r) if r.2 == standing => r.1 = e,
            _ => runs.push((s, e, standing)),
        }
    }
    for (s, e, standing) in runs {
        if e - s <= 0.02 {
            continue;
        }
        for (h0, h1) in standing {
            let k = if h0 > y0 + 1e-4 { SegmentKind::Lintel } else { kind };
            segs.push(WallSegment { a: at(s), b: at(e), half_thickness: ht, y0: h0, y1: h1, kind: k });
        }
    }
}

/// The heights of a wall standing from `heights.0` to `heights.1` that the openings `over` leave
/// standing, bottom up.
fn heights_left(heights: (f32, f32), over: &[&Opened]) -> Vec<(f32, f32)> {
    let (y0, y1) = heights;
    let mut holes: Vec<(f32, f32)> = over.iter().map(|o| (o.y0.max(y0), o.y1.min(y1))).filter(|h| h.1 > h.0).collect();
    holes.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut out = Vec::new();
    let mut cursor = y0;
    for (h0, h1) in holes {
        if h0 - cursor > 1e-3 {
            out.push((cursor, h0));
        }
        cursor = cursor.max(h1);
    }
    if y1 - cursor > 1e-3 {
        out.push((cursor, y1));
    }
    out
}

/// A GUARD PLACED ON ITS OWN (the `guard` structure piece, kind Guard, a 1.2 m section; ship homes
/// increment 7): it stops a walk across its length, from its foot to its rail, like a guard drawn
/// with the wall tool (`SegmentKind::Guard`). The segment runs along the piece's width through its
/// middle, turned with it.
fn guard_piece_segment(ps: &PlacedStructure, ty: &StructureType) -> WallSegment {
    let (s, c) = ps.rot_deg.to_radians().sin_cos();
    let hw = ty.size.0 * 0.5;
    // A piece's local (x, 0) lies at world (x cos, -x sin) from its middle
    // (`structure::structure_mesh`'s turn).
    let (dx, dz) = (hw * c, -hw * s);
    WallSegment {
        a: (ps.pos.0 - dx, ps.pos.2 - dz),
        b: (ps.pos.0 + dx, ps.pos.2 + dz),
        half_thickness: (ty.size.2 * 0.5).max(0.03),
        y0: ps.pos.1,
        y1: ps.pos.1 + ty.size.1,
        kind: SegmentKind::Guard,
    }
}

/// A STAIR (a piece of kind Stairs: a flight of steps, or a ramp) as the walk is stopped by it
/// (ship homes increment 7 review, finding 2: stairs had no collider, so on the homestead's ground
/// floor a body walked into the back of stairs-1 and through the flight until it popped up onto
/// step 3). The flight is solid from the floor (`structure::local_mesh` builds each step as a
/// column from the floor to its tread), so it is cut into its COLUMNS, foot to top: a step each,
/// or slices of a ramp no longer than `RAMP_SLICE_M`, each as tall as the ramp at its high side.
/// Each column gives `SegmentKind::Step` segments standing from the stair's foot to the column's
/// top, which stop a body whose feet are lower than that top less a step up: its riser across the
/// width, its two sides, and a CORE through the solid from it to the high end, which settles a
/// body that came to be inside the flight (a jump over its low steps that the footing will not
/// stand on them) out to the nearest side instead of leaving it trapped among the faces. And the
/// high end's face. For a body with its feet at any height, the columns that stop it are the run
/// from the first one more than a step up over its feet to the top, one box, so the faces it meets
/// are that box's: the stair is walked into from its side, its back and under its high end, and
/// climbed from its foot one riser at a time. A column no more than a step up over the stair's
/// foot never stops a body standing on the floor the stair stands on, so it gives nothing.
fn stair_faces(segs: &mut Vec<WallSegment>, ps: &PlacedStructure, ty: &StructureType) {
    let (w, h, d) = ty.size;
    let (hw, hd) = (w * 0.5, d * 0.5);
    if hw <= 0.0 || hd <= 0.0 || h <= 0.0 {
        return;
    }
    // (near side, far side, top): local z along the run, metres over the foot.
    let columns: Vec<(f32, f32, f32)> = match ty.shape {
        MeshShape::Steps => {
            let n = ty.steps.max(1);
            let (tread, riser) = (d / n as f32, h / n as f32);
            (0..n).map(|i| (-hd + i as f32 * tread, -hd + (i + 1) as f32 * tread, (i + 1) as f32 * riser)).collect()
        }
        MeshShape::Ramp => {
            let n = (d / RAMP_SLICE_M).ceil().max(1.0) as u32;
            let slice = d / n as f32;
            (0..n).map(|i| (-hd + i as f32 * slice, -hd + (i + 1) as f32 * slice, h * (i + 1) as f32 / n as f32)).collect()
        }
        _ => return,
    };
    let (s, c) = ps.rot_deg.to_radians().sin_cos();
    // A piece's local (x, z) at world (pos.x + x cos + z sin, pos.z - x sin + z cos)
    // (`structure::structure_mesh`'s turn).
    let world = |lx: f32, lz: f32| (ps.pos.0 + lx * c + lz * s, ps.pos.2 - lx * s + lz * c);
    let face = |from: (f32, f32), to: (f32, f32), half_thickness: f32, top: f32| WallSegment {
        a: world(from.0, from.1),
        b: world(to.0, to.1),
        half_thickness,
        y0: ps.pos.1,
        y1: ps.pos.1 + top,
        kind: SegmentKind::Step,
    };
    for &(z0, z1, top) in &columns {
        if top <= STEP_UP_M + 1e-4 {
            continue;
        }
        segs.push(face((-hw, z0), (hw, z0), 0.0, top));
        segs.push(face((-hw, z0), (-hw, z1), 0.0, top));
        segs.push(face((hw, z0), (hw, z1), 0.0, top));
        // The core: the box from this column to the high end, as one thick segment whose round
        // ends stay inside it (along the run when the box is longer than it is wide, else across).
        let run = hd - z0;
        segs.push(if run >= w {
            face((0.0, z0 + hw), (0.0, hd - hw), hw, top)
        } else {
            let r = run * 0.5;
            face((-(hw - r), z0 + r), (hw - r, z0 + r), r, top)
        });
    }
    if let Some(&(_, _, top)) = columns.last() {
        if top > STEP_UP_M + 1e-4 {
            segs.push(face((-hw, hd), (hw, hd), 0.0, top));
        }
    }
}

/// Build the blocking segments for the WHOLE SHIP (v0.754, ship-superstructure increment A): each
/// zone's segments (from the unchanged per-body `wall_segments`) translated by that zone's world
/// origin, all concatenated. The push-out is in x and z (Y is never touched), but since ship homes
/// increment 7 each segment carries its world heights (`y0`, `y1`, the zone's origin y added) and
/// stops only a body whose knee-to-eye band it stands across (`WallSegment::stops_body`), so the
/// walls of a storey, or of a zone on another deck, stop only the bodies at their height.
///
/// Increment B adds corridors: each zone's perimeter gets a door-width GAP where a corridor tube
/// meets it (`ShipStructure::shell_cuts_for_zone`), and each valid corridor contributes its two
/// SIDE walls as blocking rails -- and NOTHING across its open ends, so the player walks down the
/// hallway but not through its sides.
///
/// OPEN plots too (ship homes increment 5b, `ShipStructure::open`): for each neighbour plot this
/// game may walk into, the hole its corridor makes in the shared zone it runs to is a walking gap
/// here as well, and its walls as its holder walks them (`NeighbourView::segments_of`) stop this
/// player. Every other neighbour stays render only. With nothing open this is the pre-5b path.
pub fn ship_wall_segments(ship: &crate::ship::ship_structure::ShipStructure) -> Vec<WallSegment> {
    ship_segments_impl(ship, false, false)
}

/// SIGHT blockers for the whole ship (v0.975): window apertures open, everything else as
/// `ship_wall_segments`. Corridor side rails stay opaque (they are solid hull, not glass).
///
/// The NEIGHBOURS too (ship homes increment 2 review, finding 8; src/ship/neighbours.rs): their
/// walls and corridor side walls block sight as they are drawn, and the hole each neighbour's
/// corridor makes in a shared zone's wall is OPEN to sight, as it is to the eye. That hole has a
/// door pair that opens only for the other players, and a closed door blocks sight through the
/// live door list the caller adds, so a neighbour walking home through it is seen, and nothing
/// behind a shut door is. The walking segments keep that wall whole: no one walks in there but a
/// player holding a working household permit for that plot (an OPEN plot, `ship_wall_segments`).
pub fn ship_sight_segments(ship: &crate::ship::ship_structure::ShipStructure) -> Vec<WallSegment> {
    ship_segments_impl(ship, true, true)
}

/// The walls EVERY player walks against at once: this game's own (`ship_wall_segments`) plus each
/// neighbour's, as its holder walks it (its home's walls with its door open and its windows
/// solid, its corridor's side walls, and the hole its corridor makes in the shared zone it runs
/// to open). Not a collider of this game, where a neighbour's home stops nobody: the rig's door
/// points carry it (src/ship/door_points.rs `walls`), so a scripted player's route, which walks a
/// neighbour's way home, is checked against walls that really stand there (increment 2 review,
/// finding 14).
pub fn everyones_walls(ship: &crate::ship::ship_structure::ShipStructure) -> Vec<WallSegment> {
    ship_segments_impl(ship, false, true)
}

/// The two side walls of a corridor tube, the full run, as blockers: what a ship corridor
/// contributes to collision and sight, and a neighbour's corridor to sight only.
pub(crate) fn corridor_side_walls(g: &crate::ship::ship_structure::CorridorGeom) -> [WallSegment; 2] {
    use crate::ship::ship_structure::{CorridorAxis, CORRIDOR_WALL_THICKNESS};
    let hw = g.width * 0.5;
    let ht = CORRIDOR_WALL_THICKNESS * 0.5;
    // The side walls stand from the corridor's deck to its lid.
    let (y0, y1) = (g.floor_y, g.floor_y + g.height);
    let kind = SegmentKind::Wall;
    [-1.0f32, 1.0].map(|s| match g.axis {
        CorridorAxis::X => WallSegment { a: (g.start, g.lat + hw * s), b: (g.end, g.lat + hw * s), half_thickness: ht, y0, y1, kind },
        CorridorAxis::Z => WallSegment { a: (g.lat + hw * s, g.start), b: (g.lat + hw * s, g.end), half_thickness: ht, y0, y1, kind },
    })
}

/// `sight`: windows open (glass). `neighbours`: the neighbours' walls and corridors too, and the
/// holes their corridors make in the shared zones open (`ship_sight_segments`, `everyones_walls`).
/// Without it, the OPEN plots' (`ShipStructure::open`, ship homes 5b) and no other neighbour's.
fn ship_segments_impl(ship: &crate::ship::ship_structure::ShipStructure, sight: bool, neighbours: bool) -> Vec<WallSegment> {
    let mut segs = Vec::new();
    let open = &ship.open.plots;
    // Which neighbours this path walls: all of them, or only the open ones. The view is not even
    // asked for when there are none (the pre-5b walking path, bit for bit).
    let view = (neighbours || !open.is_empty()).then(|| crate::ship::neighbours::neighbour_view(ship));
    let neighbours = view.as_ref().map(|v| (v, neighbours));
    for (zi, zone) in ship.zones.iter().enumerate() {
        // A home put away stops nobody (ship homes increment 2, `ShipStructure::put_home_away`):
        // collision tests x and z only, so its walls would stand in the ship's own footprint.
        // A neighbour's home is not a zone at all, so it never reaches here (render only).
        if ship.is_away_home(zi) {
            continue;
        }
        let origin = Vec3::new(zone.origin.0, zone.origin.1, zone.origin.2);
        let mut cuts = ship.shell_cuts_for_zone(zi);
        match neighbours {
            Some((n, true)) => cuts.extend(n.zone_cuts(zi)),
            Some((n, false)) => cuts.extend(n.zone_cuts_of(zi, open)),
            None => {}
        }
        // Body-local to world, heights included (a zone on a higher deck has its walls up there).
        segs.extend(segments_impl(&zone.body, &cuts, sight).into_iter().map(|s| s.moved(origin)));
    }
    for c in &ship.corridors {
        let Ok(g) = ship.corridor_geometry(c) else {
            continue; // a broken row blocks nothing (mesh skips it too; the editor shows why)
        };
        segs.extend(corridor_side_walls(&g));
    }
    match neighbours {
        Some((n, true)) => segs.extend(n.segments(sight)),
        Some((n, false)) => segs.extend(n.segments_of(open, sight)),
        None => {}
    }
    segs
}

/// True when the straight XZ line camera -> target crosses any blocker segment
/// (v0.975 nameplate occlusion). Blockers are packed [ax, az, bx, bz] -- the flat form
/// GuiState carries per frame so the HUD never imports ship types. Proper-crossing test
/// only (strict sign flips): grazing an endpoint or running along a wall does NOT block,
/// which errs toward showing a label at a doorway edge rather than blinking it.
pub fn sight_blocked(cam: (f32, f32), target: (f32, f32), blockers: &[[f32; 4]]) -> bool {
    let orient = |ax: f32, az: f32, bx: f32, bz: f32, cx: f32, cz: f32| -> f32 {
        (bx - ax) * (cz - az) - (bz - az) * (cx - ax)
    };
    for s in blockers {
        let (ax, az, bx, bz) = (s[0], s[1], s[2], s[3]);
        let d1 = orient(cam.0, cam.1, target.0, target.1, ax, az);
        let d2 = orient(cam.0, cam.1, target.0, target.1, bx, bz);
        let d3 = orient(ax, az, bx, bz, cam.0, cam.1);
        let d4 = orient(ax, az, bx, bz, target.0, target.1);
        if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
            return true;
        }
    }
    false
}

/// Closest point on segment a..b to (px,pz), and the distance to it. (cx, cz, dist).
fn closest_on_seg(px: f32, pz: f32, a: (f32, f32), b: (f32, f32)) -> (f32, f32, f32) {
    let (ax, az) = a;
    let (bx, bz) = b;
    let (dx, dz) = (bx - ax, bz - az);
    let len2 = dx * dx + dz * dz;
    let t = if len2 < 1e-9 { 0.0 } else { (((px - ax) * dx + (pz - az) * dz) / len2).clamp(0.0, 1.0) };
    let (cx, cz) = (ax + dx * t, az + dz * t);
    let dist = ((px - cx).powi(2) + (pz - cz).powi(2)).sqrt();
    (cx, cz, dist)
}

/// A segment as the sweep meets it: a..b and how near the body's centre may come to it, its half
/// thickness plus the body's radius.
#[derive(Clone, Copy)]
struct Reach {
    a: (f32, f32),
    b: (f32, f32),
    r: f32,
}

/// Push the body's centre `p` out of every segment it is inside, pass after pass until a pass
/// moves it no further than `SETTLED_M` (at most `SETTLE_PASSES`): for a body that starts the
/// sweep inside something (a door shut on it, a gate that closed, a placement into a wall, a step
/// that now stands across it). Wedged in a gap narrower than itself, it settles in the gap's mouth.
fn settle(mut p: (f32, f32), near: &[Reach]) -> (f32, f32) {
    for _ in 0..SETTLE_PASSES {
        let mut deepest = 0.0f32;
        for s in near {
            let (cx, cz, dist) = closest_on_seg(p.0, p.1, s.a, s.b);
            let overlap = s.r - dist;
            if overlap <= 0.0 {
                continue;
            }
            deepest = deepest.max(overlap);
            if dist > 1e-6 {
                p.0 += (p.0 - cx) / dist * overlap;
                p.1 += (p.1 - cz) / dist * overlap;
            } else {
                // Exactly on the centreline: out along the segment's normal (along x for a point).
                let (sx, sz) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
                let l = (sx * sx + sz * sz).sqrt();
                if l > 1e-6 {
                    p.0 += -sz / l * s.r;
                    p.1 += sx / l * s.r;
                } else {
                    p.0 += s.r;
                }
            }
        }
        if deepest <= SETTLED_M {
            break;
        }
    }
    p
}

/// Where along the move `d` (0 to 1 of it) the body's centre from `p` first touches the segment
/// `s`, and the segment's outward normal there: None when the move does not touch it. A body
/// already touching it touches it at once if the move goes further in, and not at all if the move
/// takes it away.
fn touch(p: (f32, f32), d: (f32, f32), s: &Reach) -> Option<(f32, (f32, f32))> {
    let (cx, cz, dist) = closest_on_seg(p.0, p.1, s.a, s.b);
    if dist < s.r + SKIN_M * 0.5 {
        let n = if dist > 1e-6 {
            ((p.0 - cx) / dist, (p.1 - cz) / dist)
        } else {
            let l = (d.0 * d.0 + d.1 * d.1).sqrt().max(1e-9);
            (-d.0 / l, -d.1 / l)
        };
        return (d.0 * n.0 + d.1 * n.1 < 0.0).then_some((0.0, n));
    }
    let mut first: Option<(f32, (f32, f32))> = None;
    // Its two long sides, `r` off its line.
    let (sx, sz) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
    let l = (sx * sx + sz * sz).sqrt();
    if l > 1e-6 {
        let (ux, uz) = (sx / l, sz / l);
        let (nx, nz) = (-uz, ux);
        let off = (p.0 - s.a.0) * nx + (p.1 - s.a.1) * nz;
        let toward = d.0 * nx + d.1 * nz;
        if off * toward < 0.0 {
            let t = (off.abs() - s.r) / toward.abs();
            if (0.0..=1.0).contains(&t) {
                let along = (p.0 + d.0 * t - s.a.0) * ux + (p.1 + d.1 * t - s.a.1) * uz;
                if (0.0..=l).contains(&along) {
                    let side = off.signum();
                    first = Some((t, (nx * side, nz * side)));
                }
            }
        }
    }
    // Its two round ends, `r` round each.
    let qa = d.0 * d.0 + d.1 * d.1;
    for e in [s.a, s.b] {
        let (fx, fz) = (p.0 - e.0, p.1 - e.1);
        let qb = 2.0 * (fx * d.0 + fz * d.1);
        if qb >= 0.0 || qa < 1e-12 {
            continue;
        }
        let disc = qb * qb - 4.0 * qa * (fx * fx + fz * fz - s.r * s.r);
        if disc < 0.0 {
            continue;
        }
        let t = (-qb - disc.sqrt()) / (2.0 * qa);
        if !(0.0..=1.0).contains(&t) || first.is_some_and(|(ft, _)| ft <= t) {
            continue;
        }
        let (hx, hz) = (p.0 + d.0 * t - e.0, p.1 + d.1 * t - e.1);
        let hl = (hx * hx + hz * hz).sqrt().max(1e-6);
        first = Some((t, (hx / hl, hz / hl)));
    }
    first
}

/// MOVE AND SLIDE: carry the body's centre from `p` along `d`, stopping at the first segment it
/// touches (`SKIN_M` short of it), then go on along that segment with what is left of the move
/// into it taken away, up to `MAX_SLIDES` times; what is left after that is dropped (a body in a
/// corner, or in the mouth of a gap narrower than itself, stays there).
fn slide(mut p: (f32, f32), mut d: (f32, f32), near: &[Reach]) -> (f32, f32) {
    for _ in 0..MAX_SLIDES {
        if d.0 * d.0 + d.1 * d.1 < 1e-12 {
            break;
        }
        let mut first: Option<(f32, (f32, f32))> = None;
        for s in near {
            if let Some((t, n)) = touch(p, d, s) {
                if first.is_none_or(|(ft, _)| t < ft) {
                    first = Some((t, n));
                }
            }
        }
        let Some((t, n)) = first else {
            p.0 += d.0;
            p.1 += d.1;
            break;
        };
        p.0 += d.0 * t + n.0 * SKIN_M;
        p.1 += d.1 * t + n.1 * SKIN_M;
        let left = (d.0 * (1.0 - t), d.1 * (1.0 - t));
        let into = left.0 * n.0 + left.1 * n.1;
        d = if into < 0.0 { (left.0 - n.0 * into, left.1 - n.1 * into) } else { left };
    }
    p
}

/// Resolve the player's move from `prev` to `pos` against the walls + live door segments, sliding
/// along surfaces. Y is taken from `pos`, which is the body's EYE (`EYE_HEIGHT_M` over its feet):
/// only the segments that stop a body there are in its way (`WallSegment::stops_body`, ship homes
/// increment 7).
///
/// SWEPT (ship homes increment 7 review, finding 4): the body is settled where the move begins
/// (`settle`, for a door shut on it), then carried along the move and stopped at the first segment
/// it touches, sliding along it with the rest (`slide`), so it never tunnels through a thin wall or
/// a shut door however long the step, and never passes a gap narrower than itself. Before, the move
/// was cut into steps of 0.15 m, each pushed out of what it landed in three times over; that did
/// not settle at steps of 0.059 m or more, so at 85 frames a second or fewer a walk squeezed
/// through a 0.6 m gap that held it at 90. Now where a walk ends does not depend on the frame
/// rate. A move longer than `PLACEMENT_M` is a placement: it is settled where it lands.
pub fn resolve(prev: Vec3, pos: Vec3, radius: f32, walls: &[WallSegment], doors: &[WallSegment]) -> Vec3 {
    let (dx, dz) = (pos.x - prev.x, pos.z - prev.z);
    let placed = dx * dx + dz * dz > PLACEMENT_M * PLACEMENT_M;
    let (start, d) = if placed { ((pos.x, pos.z), (0.0, 0.0)) } else { ((prev.x, prev.z), (dx, dz)) };
    // Only the segments that stop a body at this height and stand within reach of the move (its
    // box, a metre wider each way than the body and the thickest thing a settle can push it by).
    let (lo, hi) = ((start.0.min(start.0 + d.0) - 1.0, start.1.min(start.1 + d.1) - 1.0), (start.0.max(start.0 + d.0) + 1.0, start.1.max(start.1 + d.1) + 1.0));
    let near: Vec<Reach> = walls
        .iter()
        .chain(doors.iter())
        .filter(|s| s.stops_body(pos.y))
        .map(|s| Reach { a: s.a, b: s.b, r: radius + s.half_thickness })
        .filter(|s| {
            s.a.0.min(s.b.0) - s.r <= hi.0 && s.a.0.max(s.b.0) + s.r >= lo.0 && s.a.1.min(s.b.1) - s.r <= hi.1 && s.a.1.max(s.b.1) + s.r >= lo.1
        })
        .collect();
    let p = settle(start, &near);
    let p = slide(p, d, &near);
    let p = settle(p, &near);
    Vec3::new(p.0, pos.y, p.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> HomeStructure {
        // 10 x 10 box, one interior wall across the middle (z=5) with a 2 m door centred at x=4..6.
        use crate::ship::home_structure::{InteriorWall, Opening, OpeningKind};
        HomeStructure {
            width: 10.0,
            depth: 10.0,
            height: 3.0,
            shell_material: 1,
            roof_material: 4,
            shell_thickness: None, floors: Vec::new(), lights: Vec::new(), spawn: None, structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new(),
            walls: vec![InteriorWall {
                a: (0.0, 5.0),
                b: (10.0, 5.0),
                base: 0.0,
                height: 3.0,
                material: 1,
                thickness: Some(0.2),
                layers: Vec::new(),
                openings: vec![Opening {
                    kind: OpeningKind::Door,
                    at: 4.0,
                    width: 2.0,
                    sill: 0.0,
                    height: 2.1,
                    style: "swing".into(),
                    open_dist: 2.6,
                    locked: false,
                    auto_open: true,
                    control_panel: false,
                    locks: Vec::new(),
                }],
            }],
        }
    }

    // For a static push-out test (no movement), prev == pos.
    fn at(p: Vec3, segs: &[WallSegment], doors: &[WallSegment]) -> Vec3 {
        resolve(p, p, PLAYER_RADIUS, segs, doors)
    }

    /// `home()` with a WINDOW at x=7..8.5 added to the interior wall, for the sight tests.
    fn home_with_window() -> HomeStructure {
        use crate::ship::home_structure::{Opening, OpeningKind};
        let mut h = home();
        h.walls[0].openings.push(Opening {
            kind: OpeningKind::Window,
            at: 7.0,
            width: 1.5,
            sill: 0.9,
            height: 1.2,
            style: "fixed".into(),
            open_dist: 0.0,
            locked: false,
            auto_open: false,
            control_panel: false,
            locks: Vec::new(),
        });
        h
    }

    fn flat(segs: &[WallSegment]) -> Vec<[f32; 4]> {
        segs.iter().map(|s| [s.a.0, s.a.1, s.b.0, s.b.1]).collect()
    }

    /// Through the gap of a door and the glass of a window the eye sees; through a pier, the wall
    /// over the door, the wall under the window's sill and the wall over its head it does not (in
    /// three dimensions, `ship::sight`, since the seams review of 2026-10-21, finding 3: the sight
    /// lines used to open each opening at every height, and the plan-only test of them this test
    /// used then would now see the wall over the door across the doorway).
    #[test]
    fn sight_passes_windows_and_doors_but_not_piers() {
        let h = home_with_window();
        let sight = crate::ship::sight::SightBlockers { walls: sight_segments_with_shell_cuts(&h, &[]), slabs: Vec::new() };
        // A level line across the wall (z 5) at x and at the height y.
        let hidden = |x: f32, y: f32| sight.blocked(Vec3::new(x, y, 3.0), Vec3::new(x, y, 7.0));
        // Through the solid pier (x=1): blocked.
        assert!(hidden(1.0, 1.5), "pier must block sight");
        // Through the door gap (x=5, to 2.1 m): clear; over it, the wall over the door.
        assert!(!hidden(5.0, 1.5), "door gap must pass sight");
        assert!(hidden(5.0, 2.5), "the wall over the door hides nothing");
        // Through the window (x=7.75, its glass from 0.9 to 2.1 m): clear for SIGHT; under its
        // sill and over its head, the wall.
        assert!(!hidden(7.75, 1.5), "window must pass sight");
        assert!(hidden(7.75, 0.5), "the wall under the window's sill hides nothing");
        assert!(hidden(7.75, 2.5), "the wall over the window hides nothing");
        // ...but the COLLISION spans keep that window solid (glass blocks the walker).
        let walk = flat(&wall_segments(&h));
        assert!(sight_blocked((7.75, 3.0), (7.75, 7.0), &walk), "window stays solid for collision");
    }

    #[test]
    fn sight_blocked_handles_misses_and_closed_doors() {
        // A segment fully to one side never blocks.
        let wall = [[0.0f32, 5.0, 4.0, 5.0]];
        assert!(!sight_blocked((6.0, 3.0), (6.0, 7.0), &wall), "beside the wall: clear");
        // A live closed-door segment (the caller appends these) blocks the doorway line.
        let door = [[4.0f32, 5.0, 6.0, 5.0]];
        assert!(sight_blocked((5.0, 3.0), (5.0, 7.0), &door), "closed door blocks sight");
        // Endpoint graze (ray passes exactly through a segment end) does NOT block.
        assert!(!sight_blocked((4.0, 3.0), (4.0, 7.0), &door), "endpoint graze shows the label");
    }

    #[test]
    fn perimeter_blocks_the_player() {
        let segs = wall_segments(&home());
        let out = at(Vec3::new(0.05, 1.7, 5.0), &segs, &[]);
        assert!(out.x >= PLAYER_RADIUS - 0.05, "pushed off the x=0 hull, got {}", out.x);
    }

    #[test]
    fn interior_wall_blocks_but_its_doorway_is_open() {
        let segs = wall_segments(&home());
        // Standing in the doorway gap (x=5, the door span 4..6) on the wall line: NOT pushed.
        let in_door = at(Vec3::new(5.0, 1.7, 5.0), &segs, &[]);
        assert!((in_door.z - 5.0).abs() < 0.01, "doorway is a gap, got z={}", in_door.z);
        // Against the solid pier (x=1) on the wall line: pushed off in z.
        let at_pier = at(Vec3::new(1.0, 1.7, 5.0), &segs, &[]);
        assert!((at_pier.z - 5.0).abs() > 0.2, "solid pier blocks, got z={}", at_pier.z);
    }

    #[test]
    fn closed_door_blocks_open_door_passes() {
        let door = WallSegment::at_every_height((4.0, 5.0), (6.0, 5.0), 0.05);
        let blocked = at(Vec3::new(5.0, 1.7, 5.0), &[], &[door]);
        assert!((blocked.z - 5.0).abs() > 0.2, "closed door blocks, got z={}", blocked.z);
        let passes = at(Vec3::new(5.0, 1.7, 5.0), &[], &[]);
        assert!((passes.z - 5.0).abs() < 0.01, "open doorway passes, got z={}", passes.z);
    }

    #[test]
    fn slide_preserves_tangential_motion() {
        // Walk ALONG the z=5 wall (x: 1 -> 3) while pressed into it (z=4.95). x advances, z is held
        // off the wall -- you slide, you don't stick.
        let segs = wall_segments(&home());
        let out = resolve(Vec3::new(1.0, 1.7, 4.95), Vec3::new(3.0, 1.7, 4.95), PLAYER_RADIUS, &segs, &[]);
        assert!(out.x > 2.5, "x (tangential) advanced along the wall, got {}", out.x);
        assert!(out.z < 5.0 - 0.2, "z held clear of the wall, got {}", out.z);
    }

    #[test]
    fn ship_segments_offset_each_zone_by_its_origin() {
        use crate::ship::ship_structure::{ShipStructure, ShipZone};
        // One zone at the world origin, one at (70, 0, 10): the second zone's segments (its
        // perimeter + interior wall piers) must all be translated by exactly that origin.
        let ship = ShipStructure {
            zones: vec![
                ShipZone {
                    id: "home".into(),
                    label: String::new(),
                    purpose: "residence".into(),
                    origin: (0.0, 0.0, 0.0),
                    body: home(),
                },
                ShipZone {
                    id: "commons".into(),
                    label: String::new(),
                    purpose: "commons".into(),
                    origin: (70.0, 0.0, 10.0),
                    body: home(),
                },
            ],
            corridors: Vec::new(),
            ..Default::default()
        };
        let per_zone = wall_segments(&home());
        let all = ship_wall_segments(&ship);
        assert_eq!(all.len(), per_zone.len() * 2, "both zones contribute segments");
        // The second zone's block mirrors the first, shifted by (+70, +10).
        for (i, s) in per_zone.iter().enumerate() {
            let shifted = &all[per_zone.len() + i];
            assert!((shifted.a.0 - (s.a.0 + 70.0)).abs() < 1e-4, "x offset by the zone origin");
            assert!((shifted.a.1 - (s.a.1 + 10.0)).abs() < 1e-4, "z offset by the zone origin");
            assert!((shifted.b.0 - (s.b.0 + 70.0)).abs() < 1e-4);
            assert!((shifted.b.1 - (s.b.1 + 10.0)).abs() < 1e-4);
        }
        // Behavioral check: the second zone's x=70 hull pushes a player standing just inside it.
        let out = at(Vec3::new(70.05, 1.7, 15.0), &all, &[]);
        assert!(out.x >= 70.0 + PLAYER_RADIUS - 0.05, "pushed off the offset hull, got {}", out.x);
        // And its doorway gap still works at the offset position (door span x=4..6 local -> 74..76).
        let in_door = at(Vec3::new(75.0, 1.7, 15.0), &all, &[]);
        assert!((in_door.z - 15.0).abs() < 0.01, "offset doorway is a gap, got z={}", in_door.z);
    }

    #[test]
    fn fast_move_does_not_tunnel_a_thin_wall() {
        // Sprint straight through the solid pier (x=1) of the 0.2 m wall at z=5, from z=4 to z=7 in
        // ONE call. Substepping must STOP the player on the near side, not let them tunnel to z=7.
        let segs = wall_segments(&home());
        let out = resolve(Vec3::new(1.0, 1.7, 4.0), Vec3::new(1.0, 1.7, 7.0), PLAYER_RADIUS, &segs, &[]);
        assert!(out.z < 4.9, "blocked on the near side of the wall, got z={}", out.z);
    }

    /// A plain zone body (no interior walls): since the corridor rework, corridors own their door
    /// mouths, so the collision fixture needs nothing but the boxes.
    fn plain_body(w: f32, d: f32, h: f32) -> HomeStructure {
        ron::from_str(&format!("(width: {w}, depth: {d}, height: {h})"))
            .expect("corridor test body parses")
    }

    /// Increment B: a corridor's SIDE walls block, its RUN is open end to end (the perimeter shells
    /// gain door-width gaps where the tube meets them), and off-mouth perimeter still blocks. The
    /// mouths are the corridor's OWN cuts (1 m about world z = 5) -- no authored door walls exist,
    /// which is the point: the old coincident door wall at each mouth z-fought the shell and let
    /// the player clip through one of the two coplanar surfaces.
    #[test]
    fn corridor_sides_block_but_the_run_is_open_end_to_end() {
        use crate::ship::ship_structure::{ShipCorridor, ShipStructure, ShipZone};
        // Home 10x10x3 at the origin; commons 8x8x6 at (20, 0, 2). Tube: x 10..20, width 3,
        // centreline at world z = 5, mouth 1 m wide.
        let ship = ShipStructure {
            zones: vec![
                ShipZone {
                    id: "home".into(),
                    label: String::new(),
                    purpose: "residence".into(),
                    origin: (0.0, 0.0, 0.0),
                    body: plain_body(10.0, 10.0, 3.0),
                },
                ShipZone {
                    id: "commons".into(),
                    label: String::new(),
                    purpose: "commons".into(),
                    origin: (20.0, 0.0, 2.0),
                    body: plain_body(8.0, 8.0, 6.0),
                },
            ],
            corridors: vec![ShipCorridor {
                from_zone: "home".into(),
                to_zone: "commons".into(),
                lat: 5.0,
                width: 3.0,
                door_width: 1.0,
                door_height: 2.1,
                glass_top: false,
            }],
            ..Default::default()
        };
        let segs = ship_wall_segments(&ship);
        // The tube's two side rails (z = 3.5 and 6.5, x 10..20) block...
        let out = at(Vec3::new(15.0, 1.7, 6.5), &segs, &[]);
        assert!((out.z - 6.5).abs() > 0.2, "a corridor side wall blocks, got z={}", out.z);
        // ...but the centreline is clear (no segment across the run or its open ends).
        let mid = at(Vec3::new(15.0, 1.7, 5.0), &segs, &[]);
        assert!((mid.x - 15.0).abs() < 0.01 && (mid.z - 5.0).abs() < 0.01, "mid-tube is open");
        // The player can WALK from inside the home, through its shell cut, down the tube, through
        // the commons' shell cut: both perimeter crossings are gaps, not walls.
        let leave_home = resolve(Vec3::new(9.5, 1.7, 5.0), Vec3::new(10.5, 1.7, 5.0), PLAYER_RADIUS, &segs, &[]);
        assert!(leave_home.x > 10.3, "the home shell opens at the corridor mouth, got x={}", leave_home.x);
        let enter_commons = resolve(Vec3::new(19.5, 1.7, 5.0), Vec3::new(20.5, 1.7, 5.0), PLAYER_RADIUS, &segs, &[]);
        assert!(enter_commons.x > 20.3, "the commons shell opens at the corridor mouth, got x={}", enter_commons.x);
        // Off the door, the home's x=10 perimeter is still solid.
        let blocked = resolve(Vec3::new(9.5, 1.7, 8.5), Vec3::new(10.5, 1.7, 8.5), PLAYER_RADIUS, &segs, &[]);
        assert!(blocked.x < 9.8, "off-door perimeter still blocks, got x={}", blocked.x);
    }

    /// Corridor DOOR panels (v0.795): the two closed half-panels of a corridor mouth, converted
    /// to live door segments exactly the way the movement code does it (centre +/- half-width
    /// along rotation * X), tile the full aperture -- a shut corridor door blocks the walk-through
    /// gap the shell cut opened, and dropping the segments (the door open) restores the passage.
    #[test]
    fn closed_corridor_door_halves_block_the_mouth_they_guard() {
        use crate::ship::ship_structure::{ShipCorridor, ShipStructure, ShipZone};
        let ship = ShipStructure {
            zones: vec![
                ShipZone {
                    id: "home".into(),
                    label: String::new(),
                    purpose: "residence".into(),
                    origin: (0.0, 0.0, 0.0),
                    body: plain_body(10.0, 10.0, 3.0),
                },
                ShipZone {
                    id: "commons".into(),
                    label: String::new(),
                    purpose: "commons".into(),
                    origin: (20.0, 0.0, 2.0),
                    body: plain_body(8.0, 8.0, 6.0),
                },
            ],
            corridors: vec![ShipCorridor {
                from_zone: "home".into(),
                to_zone: "commons".into(),
                lat: 5.0,
                width: 3.0,
                door_width: 1.0,
                door_height: 2.1,
                glass_top: false,
            }],
            ..Default::default()
        };
        let segs = ship_wall_segments(&ship);
        // The same placement-to-segment conversion the engine's per-frame door path performs.
        let door_segs: Vec<WallSegment> = crate::ship::door_panels::corridor_panel_placements(&ship)
            .iter()
            .map(|p| door_panel_segment(p.center, p.rotation, p.size))
            .collect();
        assert_eq!(door_segs.len(), 4, "two mouths x two halves");
        // CLOSED: walking at the home mouth (x 9.5 -> 10.5 through the 1 m aperture at z = 5) is
        // stopped on the near side...
        let blocked = resolve(Vec3::new(9.5, 1.7, 5.0), Vec3::new(10.5, 1.7, 5.0), PLAYER_RADIUS, &segs, &door_segs);
        assert!(blocked.x < 9.8, "a shut corridor door blocks the mouth, got x={}", blocked.x);
        // ...and the halves seal the aperture EDGES too (no slipping past a half-panel's end).
        let edge = resolve(Vec3::new(9.5, 1.7, 5.4), Vec3::new(10.5, 1.7, 5.4), PLAYER_RADIUS, &segs, &door_segs);
        assert!(edge.x < 9.8, "the aperture edge is sealed, got x={}", edge.x);
        // OPEN (the engine drops a door's segments once its open fraction crosses the gate): the
        // mouth is the walk-through gap again.
        let through = resolve(Vec3::new(9.5, 1.7, 5.0), Vec3::new(10.5, 1.7, 5.0), PLAYER_RADIUS, &segs, &[]);
        assert!(through.x > 10.3, "an open corridor door walks through, got x={}", through.x);
    }

    /// A player walking toward `to`, 0.1 m a frame, the way the camera moves: each frame's step is
    /// resolved against `walls` (doors open), so a wall in the way stops them at it.
    fn walk(from: Vec3, to: Vec3, walls: &[WallSegment]) -> Vec3 {
        let mut p = from;
        for _ in 0..2000 {
            let left = Vec3::new(to.x - p.x, 0.0, to.z - p.z);
            if left.length() < 1e-3 {
                break;
            }
            let step = left.normalize() * left.length().min(0.1);
            p = resolve(p, p + step, PLAYER_RADIUS, walls, &[]);
        }
        p
    }

    /// AN OPEN PLOT LETS ITS PERMIT HOLDER WALK IN, AND A CLOSED ONE DOES NOT (ship homes increment
    /// 5b, plan section 5 lane 1B; section 3.7's table). The home on p2, so p1 is a neighbour's,
    /// its corridor running from its door (x 55, z 40) to the Commons' west wall (x 65). From the
    /// Commons, (67, 40), toward (40, 40) inside p1's home: with p1 OPEN (`ShipStructure::open`)
    /// the walk ends there; closed, the Commons' wall stops it at x 65 or more, as before 5b. Open,
    /// p1's walls stop this player too (its corridor's side walls, and its home's walls off its
    /// door), and the eye's segments are the same open or closed. The walk starts at x 67, not the
    /// plan's 70: the Commons' own inner wall at x 69 (local x 4, z 9 to 29) is solid at z 40, so
    /// (70, 40) is in the hallway beyond it, and the corridor mouth opens into the 4 m strip
    /// between that wall and the west wall (the first try, from 70, stopped at x 69.35 open or not).
    /// Seen red 2026-10-12 with the open plot's zone cut not added to the walking path
    /// (`zone_cuts_of` skipped in `ship_segments_impl`): "open p1: the walk from the Commons ends
    /// at Vec3(65.33, 1.7, 40.0), not in p1's home at (40, 40)" failed.
    #[test]
    fn an_open_plot_lets_its_permit_holder_walk_in_and_a_closed_one_does_not() {
        use crate::ship::ship_structure::{OpenPlots, ShipStructure};
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let closed = ShipStructure::load_and_assemble_shipped(&data, Some("p2")).expect("the shipped ship assembles");
        let mut open = closed.clone();
        open.open = OpenPlots { plots: ["p1".to_string()].into_iter().collect(), lit: None };
        let (from, to) = (Vec3::new(67.0, 1.7, 40.0), Vec3::new(40.0, 1.7, 40.0));

        let shut = walk(from, to, &ship_wall_segments(&closed));
        assert!(shut.x >= 65.0, "closed p1: the walk from the Commons got to x {} (past its wall at 65)", shut.x);
        let walls = ship_wall_segments(&open);
        let end = walk(from, to, &walls);
        assert!((end - to).length() < 0.1, "open p1: the walk from the Commons ends at {end:?}, not in p1's home at (40, 40)");

        // Open, p1's corridor sides and home walls stand for this player: pressed into the
        // corridor's north side (z 41.5), or into the home's east wall off its door (x 55, z 20),
        // they are pushed off. Closed, nothing of p1 collides (render only).
        let pushed = |p: Vec3, w: &[WallSegment]| (resolve(p, p, PLAYER_RADIUS, w, &[]) - p).length() > 0.1;
        for (p, what) in [(Vec3::new(60.0, 1.7, 41.5), "the corridor's side wall"), (Vec3::new(55.0, 1.7, 20.0), "the home's east wall")] {
            assert!(pushed(p, &walls), "open p1: {what} at {p:?} stops nobody");
            assert!(!pushed(p, &ship_wall_segments(&closed)), "closed p1: {what} at {p:?} collides");
        }
        // Another neighbour stays render only, and the eye sees the same either way.
        let flat = |s: Vec<WallSegment>| s.into_iter().map(|w| [w.a.0, w.a.1, w.b.0, w.b.1]).collect::<Vec<_>>();
        assert_eq!(flat(ship_sight_segments(&open)), flat(ship_sight_segments(&closed)), "opening p1 changed the sight lines");
        let p3_door = Vec3::new(60.0, 1.7, 238.0);
        assert!(!pushed(p3_door + Vec3::new(0.0, 0.0, 1.5), &walls), "p3, not open, collides");
    }

    /// A GAP NARROWER THAN THE BODY HOLDS IT AT EVERY FRAME RATE (ship homes increment 7 review,
    /// finding 4). Two 0.06 m guards end 0.6 m apart; a body (radius 0.3) needs 0.66 m. Walking
    /// 5 m/s straight at the gap, at 30, 60 and 144 frames a second, each frame's step swept as
    /// the game sweeps it, the body is held on the near side, and ends where it ends at every
    /// frame rate. Before, `resolve_once`'s three push-out passes did not settle at steps of
    /// 0.059 m or more, so a walk at 85 frames a second or fewer squeezed through.
    ///
    /// Seen red first (2026-10-21) with the three-pass push-out sweep: "at 30 frames a second
    /// the walk at x 0 went through the 0.6 m gap to Vec3(0.0, 1.7, 1.5)".
    #[test]
    fn a_gap_narrower_than_the_body_holds_it_at_every_frame_rate() {
        let gap = [
            WallSegment::at_every_height((-3.0, 0.0), (-0.3, 0.0), 0.03),
            WallSegment::at_every_height((0.3, 0.0), (3.0, 0.0), 0.03),
        ];
        for x in [0.0f32, 0.03, 0.1] {
            let mut ends = Vec::new();
            for fps in [30.0f32, 60.0, 144.0] {
                let step = 5.0 / fps;
                let (mut p, to) = (Vec3::new(x, 1.7, -1.5), Vec3::new(x, 1.7, 1.5));
                for _ in 0..(3.0 * fps) as usize {
                    let left = to - p;
                    let left = Vec3::new(left.x, 0.0, left.z);
                    if left.length() < 1e-4 {
                        break;
                    }
                    p = resolve(p, p + left.normalize() * left.length().min(step), PLAYER_RADIUS, &gap, &[]);
                }
                assert!(p.z < -0.1, "at {fps} frames a second the walk at x {x} went through the 0.6 m gap to {p:?}");
                ends.push((fps, p));
            }
            for (fps, p) in &ends[1..] {
                assert!((*p - ends[0].1).length() < 2e-3, "the walk at x {x} ends at {:?} at 30 frames a second and at {p:?} at {fps}", ends[0].1);
            }
        }
    }
}

/// The homestead's upper floor, walked (ship homes increment 7 review, the movement findings):
/// the shell over the front door upstairs, the back and the high end of stairs-1, and the guards
/// a jump used to clear. On the shipped ship with the homestead on p1, whose home stands at the
/// ship's origin, so its home-local metres are the world's.
#[cfg(all(test, feature = "native"))]
mod homestead_walk_tests {
    use super::*;
    use crate::engine::footing::test_aboard::{body_at, live_frame, Aboard, EAST, NORTH, SOUTH, WEST};
    use crate::input::bindings::GameAction as A;
    use crate::ship::ship_structure::ShipStructure;

    const EYE: f32 = EYE_HEIGHT_M;
    /// The homestead's upper floor (its slabs' tops).
    const UP: f32 = 3.0;

    fn shipped() -> ShipStructure {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let ship = ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles");
        let home = &ship.zones[ship.home_zone_index()];
        assert_eq!(home.origin, (0.0, 0.0, 0.0), "the home on p1 stands at the ship's origin");
        ship
    }

    /// Walk from `from` toward `to` 0.1 m a frame, the eye's height held (no footing): where the
    /// walk ends.
    fn walk(from: Vec3, to: Vec3, walls: &[WallSegment]) -> Vec3 {
        let mut p = from;
        for _ in 0..2000 {
            let left = Vec3::new(to.x - p.x, 0.0, to.z - p.z);
            if left.length() < 1e-3 {
                break;
            }
            p = resolve(p, p + left.normalize() * left.length().min(0.1), PLAYER_RADIUS, walls, &[]);
        }
        p
    }

    /// UPSTAIRS THE SHELL OVER THE FRONT DOOR STOPS A WALK; DOWNSTAIRS THE DOOR LETS IT THROUGH.
    /// The plot's door corridor cuts the homestead's east wall (x 55) over z 39 to 41, 2.2 m
    /// high; the shell stands over the cut, where the upper hall is. Walking east from the upper
    /// hall at (53, 39.8), the body is held inside the shell at x 55; on the ground floor the same
    /// walk at z 40 goes out through the door.
    ///
    /// Seen red first (2026-10-21) with the cut open at every height: "upstairs, the walk east
    /// from the upper hall went out through the shell over the front door to Vec3(56.99994, 4.7,
    /// 39.8)".
    #[test]
    fn upstairs_the_shell_over_the_front_door_stops_a_walk() {
        let walls = ship_wall_segments(&shipped());
        let e = walk(Vec3::new(53.0, UP + EYE, 39.8), Vec3::new(57.0, UP + EYE, 39.8), &walls);
        assert!(e.x < 55.0 - PLAYER_RADIUS + 0.05, "upstairs, the walk east from the upper hall went out through the shell over the front door to {e:?}");
        let e = walk(Vec3::new(53.0, EYE, 40.0), Vec3::new(57.0, EYE, 40.0), &walls);
        assert!((e.x - 57.0).abs() < 1e-3, "downstairs, the front door stopped the walk at {e:?}");
    }

    /// THE BACK OF STAIRS-1 STOPS A WALK ON THE GROUND FLOOR. Stairs-1 climbs west from its foot
    /// at x 41 to its top at x 36.5, where its high end is a solid face from the floor to 3.0 m.
    /// Walking east from outside the dining room's door (35.4, 38.65), along the middle of the
    /// flight, the body is held at that face, not let into the flight to pop up onto step 3.
    ///
    /// Seen red first (2026-10-21) with no stair in the walls: "on the ground floor the walk east
    /// from the dining room's door went into the back of the stair, to Vec3(41.9999, 1.7, 38.65)".
    #[test]
    fn the_back_of_the_stairs_stops_a_walk_on_the_ground_floor() {
        let walls = ship_wall_segments(&shipped());
        let e = walk(Vec3::new(35.4, EYE, 38.65), Vec3::new(42.0, EYE, 38.65), &walls);
        assert!(e.x < 36.5 - PLAYER_RADIUS + 0.05, "on the ground floor the walk east from the dining room's door went into the back of the stair, to {e:?}");
    }

    /// STAIRS-1 IS STILL CLIMBED UP AND DOWN. The whole walk, lib.rs's frame (`live_frame`: the
    /// footing, the walk's substeps, the sweep against the walls): from the hall east of its foot,
    /// west up the flight onto the upper floor, then back east down it to the ground, at 60 frames
    /// a second and at 100 ms frames. Its steps stop a body only below each step's top less the
    /// step up, so the flight is climbed from its foot.
    ///
    /// Passes before the stair had walls and after (it guards the climb against them).
    #[test]
    fn stairs_1_is_still_climbed_up_and_down() {
        for dt in [1.0 / 60.0, 0.1] {
            let mut w = Aboard::of(shipped());
            let (mut ctl, mut cam) = body_at(Vec3::new(42.5, EYE, 38.65));
            cam.yaw = WEST;
            ctl.apply_action(A::MoveForward, true);
            for _ in 0..(2.4 / dt) as usize {
                live_frame(&mut ctl, &mut cam, dt, &mut w);
            }
            let feet = cam.position.y - ctl.eye_height();
            assert!((feet - UP).abs() < 1e-3 && cam.position.x < 36.0, "at {:.0} ms frames the climb up stairs-1 ended at {:?}, feet {feet}", dt * 1000.0, cam.position);
            cam.yaw = EAST;
            for _ in 0..(2.4 / dt) as usize {
                live_frame(&mut ctl, &mut cam, dt, &mut w);
            }
            let feet = cam.position.y - ctl.eye_height();
            assert!(feet.abs() < 1e-3 && cam.position.x > 41.5, "at {:.0} ms frames the walk down stairs-1 ended at {:?}, feet {feet}", dt * 1000.0, cam.position);
        }
    }

    /// A JUMP DOES NOT CLEAR A GUARD. A jump lifts the feet 1.27 m; a guard stops any body whose
    /// feet are under its rail plus 0.3 m, so at every height of a jump from the upper floor a
    /// walk at G1 (the stairwell's long edge, from the gallery north), G2 (its east end, from the
    /// upper hall west) and G3 (the ladder hatch's west side, from the lift lobby east) is held
    /// at the guard, 2.6 to 3 m over the stairs or the service way.
    ///
    /// Seen red first (2026-10-21) with a guard stopping only the knee-to-eye band like a wall:
    /// "G1, the stairwell's long edge: a body with its feet at 3.63 m walked over it to
    /// Vec3(38.5, 5.335, 38.35)".
    #[test]
    fn a_jump_does_not_clear_a_guard() {
        let walls = ship_wall_segments(&shipped());
        let guards: [(&str, Vec3, Vec3, Box<dyn Fn(Vec3) -> bool>); 3] = [
            ("G1, the stairwell's long edge", Vec3::new(38.5, 0.0, 39.8), Vec3::new(38.5, 0.0, 37.5), Box::new(|e: Vec3| e.z > 39.3 + PLAYER_RADIUS - 0.05)),
            ("G2, the stairwell's east end", Vec3::new(41.3, 0.0, 38.65), Vec3::new(38.5, 0.0, 38.65), Box::new(|e: Vec3| e.x > 40.5 + PLAYER_RADIUS - 0.05)),
            ("G3, the ladder hatch's west side", Vec3::new(31.6, 0.0, 27.5), Vec3::new(33.5, 0.0, 27.5), Box::new(|e: Vec3| e.x < 32.5 - PLAYER_RADIUS + 0.05)),
        ];
        for (what, from, to, held) in &guards {
            for k in 0..=20 {
                let feet = UP + 1.27 * k as f32 / 20.0;
                let y = Vec3::Y * (feet + EYE);
                let e = walk(*from + y, *to + y, &walls);
                assert!(held(e), "{what}: a body with its feet at {feet:.2} m walked over it to {e:?}");
            }
        }
    }

    /// A RUNNING JUMP AT G1 LANDS BACK ON THE GALLERY. The whole walk (`live_frame`): from the
    /// gallery at (38.5, 39.8) facing north, walking and jumping at the guard over the stairwell,
    /// the body never crosses the guard's line and lands on the upper floor, not 2.6 m below on
    /// the stairs.
    ///
    /// Seen red first (2026-10-21) with a guard stopping only the knee-to-eye band: "the jump at
    /// G1 crossed it to Vec3(38.5, 5.327375, 39.54667)".
    #[test]
    fn a_running_jump_at_g1_lands_back_on_the_gallery() {
        let mut w = Aboard::of(shipped());
        let (mut ctl, mut cam) = body_at(Vec3::new(38.5, UP + EYE, 39.9));
        cam.yaw = NORTH;
        ctl.apply_action(A::MoveForward, true);
        ctl.apply_action(A::Jump, true);
        for _ in 0..90 {
            live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
            ctl.apply_action(A::Jump, false);
            assert!(cam.position.z > 39.3 + PLAYER_RADIUS - 0.05, "the jump at G1 crossed it to {:?}", cam.position);
        }
        let feet = cam.position.y - ctl.eye_height();
        assert!((feet - UP).abs() < 1e-3, "after the jump at G1 the feet are at {feet}, not on the upper floor");
    }

    /// THE SWEEP'S STEP UP IS THE FOOTING'S. A stair's step stops a body whose feet are lower than
    /// its top less `STEP_UP_M`, and the footing stands a body on a step no higher than
    /// `engine::footing::STEP_UP_M` over the floor it rests on; were the two apart, a step would
    /// be neither stood on nor walked into, or both.
    ///
    /// Seen red first (2026-10-21) with `STEP_UP_M` at 0.55: "assertion `left == right` failed:
    /// the sweep's step up is not the footing's, left: 0.55, right: 0.6".
    #[test]
    fn the_sweeps_step_up_is_the_footings() {
        assert_eq!(STEP_UP_M, crate::engine::footing::STEP_UP_M, "the sweep's step up is not the footing's");
    }

    /// A RUNNING JUMP OUT THROUGH THE FRONT DOOR GOES THROUGH IT. The shell over the front door's
    /// cut stands from 2.2 m (finding 1), and a body jumping through the doorway reaches it with
    /// its head: in the entry under the slab its eye goes to 2.63 m, in the corridor to 2.97. The
    /// wall over the door is a lintel (`SegmentKind::Lintel`), which stops a body whose knee is at
    /// its height, a body upstairs, and lets one whose knee is under it duck through, so the jump
    /// carries the body out into the corridor instead of holding it at the wall over the door
    /// until it comes down: a second of walking at 5 m/s from x 53.5 ends past x 58.2.
    ///
    /// Seen red first (2026-10-21) with the shell over a cut a `SegmentKind::Wall` (the
    /// knee-to-eye band): "the running jump out through the front door ended at
    /// Vec3(56.919865, 1.7, 40.0)" (held at the wall over the door until the eye came back under
    /// 2.2 m).
    #[test]
    fn a_running_jump_out_through_the_front_door_goes_through_it() {
        let mut w = Aboard::of(shipped());
        let (mut ctl, mut cam) = body_at(Vec3::new(53.5, EYE, 40.0));
        cam.yaw = EAST;
        ctl.apply_action(A::MoveForward, true);
        ctl.apply_action(A::Jump, true);
        for _ in 0..60 {
            live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
            ctl.apply_action(A::Jump, false);
        }
        assert!(cam.position.x > 58.2, "the running jump out through the front door ended at {:?}", cam.position);
    }

    /// A RUNNING JUMP IN THROUGH THE FRONT DOOR IS HELD AT THE DOORWAY UNTIL THE HEAD COMES DOWN
    /// (the seams review of 2026-10-21, finding 6: recorded, not changed). The other way from the
    /// jump out: from the corridor at (58, 1.7, 40), running west and jumping, the body reaches the
    /// door at x 55 with its head over the underside of the slab over the entry (2.75 m), and that
    /// slab's edge (`engine::footing::slab_edges`) holds it at the door while its feet are over
    /// about 0.95 m: from this start, 12 frames at 60 a second (0.2 s; longer the earlier in the
    /// jump the door is reached), its centre never nearer than 55.30, a body's radius off the
    /// edge. Once the feet come down it walks on into the entry (x 51.5 after 1.5 s). The lintel
    /// over the door (the shell over the cut, from 2.2 m) stops only a body whose knee is at its
    /// height, so it is not what holds the jump: it is the head meeting the head of the door, where
    /// the ceiling of the room beyond begins, as it would in a real doorway of 2.1 to 2.4 m.
    ///
    /// Passes as it is: it pins the behaviour, so a change to the lintel or to the slab's edge
    /// that lets a jumping head through a door under a ceiling shows here.
    #[test]
    fn a_running_jump_in_through_the_front_door_is_held_at_the_doorway() {
        let mut w = Aboard::of(shipped());
        let (mut ctl, mut cam) = body_at(Vec3::new(58.0, EYE, 40.0));
        cam.yaw = WEST;
        ctl.apply_action(A::MoveForward, true);
        ctl.apply_action(A::Jump, true);
        let (mut held, mut high_at_door) = (0usize, f32::MAX);
        for k in 0..90 {
            live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
            ctl.apply_action(A::Jump, false);
            let feet = cam.position.y - ctl.eye_height();
            if feet > 1.0 {
                high_at_door = high_at_door.min(cam.position.x);
                assert!(cam.position.x > 55.0 + PLAYER_RADIUS - 0.01, "frame {k}: the jump went in under the slab over the entry with its feet at {feet:.2}, to {:?}", cam.position);
            }
            if (cam.position.x - (55.0 + PLAYER_RADIUS)).abs() < 0.02 && feet > 0.9 {
                held += 1;
            }
        }
        let feet = cam.position.y - ctl.eye_height();
        assert!(held >= 6, "the jump was held at the door for only {held} frames (closest it came with its feet over 1 m: x {high_at_door:.2})");
        assert!(cam.position.x < 54.0 && feet.abs() < 1e-3, "after the jump the walk did not go on into the entry: {:?}, feet {feet:.2}", cam.position);
    }

    /// A small home with a ladder hatch guarded by STEEL BALUSTRADES (wall material 9, so
    /// `SegmentKind::Guard`) and a gap of `gap` metres in the guard at the ladder: the homestead's
    /// hatch (G3 to G5) in a 12 x 12 m box. The hatch is x 1.5 to 2.5, z 0.9 to 2.1, ladder-1 at
    /// (2, 0, 2) on its south edge, climbed facing south and stepped off south over its top.
    fn guarded_hatch(gap: f32) -> ShipStructure {
        let (w0, w1) = (2.0 - gap * 0.5, 2.0 + gap * 0.5);
        let ron_text = format!(
            r#"(id: "home", purpose: "residence", body: (width: 12.0, depth: 12.0, height: 6.0,
                floors: [
                    (a: (0.0, 0.0), b: (12.0, 0.9), top: 3.0),
                    (a: (0.0, 0.9), b: (1.5, 6.0), top: 3.0),
                    (a: (1.5, 2.1), b: (2.5, 6.0), top: 3.0),
                    (a: (2.5, 0.9), b: (12.0, 6.0), top: 3.0),
                ],
                zones: [(id: "upstairs", type_id: "room_hall", origin: (0.0, 3.0, 0.0), size: (12.0, 3.0, 6.0))],
                walls: [
                    (a: (1.5, 0.9), b: (1.5, 2.1), base: 3.0, height: 1.0, material: 9),
                    (a: (2.5, 0.9), b: (2.5, 2.1), base: 3.0, height: 1.0, material: 9),
                    (a: (1.5, 0.9), b: (2.5, 0.9), base: 3.0, height: 1.0, material: 9),
                    (a: (1.5, 2.1), b: ({w0}, 2.1), base: 3.0, height: 1.0, material: 9),
                    (a: ({w1}, 2.1), b: (2.5, 2.1), base: 3.0, height: 1.0, material: 9),
                ],
                structures: [(id: "ladder-1", type_id: "ladder", pos: (2.0, 0.0, 2.0))]))"#
        );
        let zone: crate::ship::ship_structure::ShipZone = ron::from_str(&ron_text).expect("the guarded hatch parses");
        assert!(zone.body.walls.iter().all(|w| w.guard().is_some()), "material 9 is a guard");
        ShipStructure { zones: vec![zone], ..Default::default() }
    }

    /// THE LADDER STILL CLIMBS THROUGH ITS GAP IN THE GUARD. A guard now holds a body whose feet
    /// are anywhere under its rail plus 0.3 m (a jump does not clear it), and the sweep lets no
    /// body through a gap narrower than itself; the climber at a ladder hatch passes through the
    /// guard's gap at the ladder, 0.8 m (the homestead's G4 to G5, widened in the data from the
    /// ladder's own 0.6 m, which a 0.6 m body cannot pass between two 0.06 m guards). Up with
    /// Space from the hatch, off the top south onto the upper floor; nothing on the way hurts.
    ///
    /// Seen red first (2026-10-21) with the gap at 0.6 m: "off the top of the ladder through the
    /// 0.6 m gap in the guard the body is at Vec3(2.0, 4.8999996, 1.9623556), feet 3.1999996"
    /// (held at the guard's ends, hanging at the ladder's top).
    #[test]
    fn the_ladder_still_climbs_through_its_gap_in_the_guard() {
        let gap = 0.8;
        let mut w = Aboard::of(guarded_hatch(gap));
        let (mut ctl, mut cam) = body_at(Vec3::new(2.0, EYE, 1.5));
        ctl.apply_action(A::Jump, true);
        for _ in 0..90 {
            live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
        }
        ctl.apply_action(A::Jump, false);
        cam.yaw = SOUTH;
        ctl.apply_action(A::MoveForward, true);
        for _ in 0..24 {
            live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
        }
        ctl.apply_action(A::MoveForward, false);
        for _ in 0..30 {
            live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
        }
        let feet = cam.position.y - ctl.eye_height();
        assert!(cam.position.z > 2.6 && (feet - UP).abs() < 1e-3, "off the top of the ladder through the {gap} m gap in the guard the body is at {:?}, feet {feet}", cam.position);
    }
}

/// Ship homes increment 7, Wave 0 (docs/design/ship-homes-increment-7-plan.md section 2): every
/// blocking segment knows the world heights it stands between, which the sweep reads since lane M
/// (`WallSegment::stops_body`, `storey_walk_tests` below).
#[cfg(test)]
mod storey_height_tests {
    use super::*;

    /// EVERY SEGMENT KNOWS HOW HIGH IT STANDS. In a two-storey body the perimeter stands floor to
    /// roof, a ground wall 0..3 and an upstairs wall 3..6; on a deck 10 m up, the ship's segments
    /// stand 10 m higher with their x and z where they were. In the shipped ship every segment
    /// (neighbours' and corridors' included) has a foot under its top, and every corridor's cut
    /// through a shell opens from that body's floor (`ShellCut::base` 0), so the geometry lane
    /// reading `base` changes nothing for a shipped home.
    ///
    /// Seen red 2026-10-20 with `WallSegment::moved` not adding the zone's height: "on a deck 10 m
    /// up the walls stand 10 m up", left [(0.0, 6.0) four times, (0.0, 3.0), (3.0, 6.0)].
    #[test]
    fn every_segment_knows_how_high_it_stands() {
        use crate::ship::ship_structure::{ShipStructure, ShipZone};
        let body: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, \
             walls: [(a: (0.0, 5.0), b: (10.0, 5.0)), (a: (5.0, 0.0), b: (5.0, 5.0), base: 3.0)], \
             floors: [(a: (0.0, 0.0), b: (10.0, 5.0), top: 3.0)])",
        )
        .expect("the test body parses");
        let heights = |segs: &[WallSegment]| segs.iter().map(|s| (s.y0, s.y1)).collect::<Vec<_>>();
        let local = wall_segments(&body);
        assert_eq!(heights(&local), [vec![(0.0, 6.0); 4], vec![(0.0, 3.0), (3.0, 6.0)]].concat(), "perimeter, ground wall, upstairs wall");
        let ship = ShipStructure {
            zones: vec![ShipZone { id: "deck".into(), label: String::new(), purpose: "residence".into(), origin: (100.0, 10.0, 20.0), body }],
            ..Default::default()
        };
        let placed = ship_wall_segments(&ship);
        assert_eq!(heights(&placed), [vec![(10.0, 16.0); 4], vec![(10.0, 13.0), (13.0, 16.0)]].concat(), "on a deck 10 m up the walls stand 10 m up");
        for (l, p) in local.iter().zip(&placed) {
            assert_eq!((p.a, p.b, p.half_thickness), ((l.a.0 + 100.0, l.a.1 + 20.0), (l.b.0 + 100.0, l.b.1 + 20.0), l.half_thickness), "x and z move as before");
        }
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let shipped = ShipStructure::load_and_assemble_shipped(&data, None).expect("the shipped ship assembles");
        for s in everyones_walls(&shipped) {
            assert!(s.y0.is_finite() && s.y1.is_finite() && s.y0 < s.y1, "a shipped segment stands nowhere: {s:?}");
        }
        for zi in 0..shipped.zones.len() {
            for c in shipped.shell_cuts_for_zone(zi) {
                assert_eq!(c.base, 0.0, "zone {}: a corridor's cut does not open from the floor: {c:?}", shipped.zones[zi].id);
            }
        }
        let anywhere = WallSegment::at_every_height((0.0, 0.0), (1.0, 0.0), 0.1);
        assert!(anywhere.y0 == f32::NEG_INFINITY && anywhere.y1 == f32::INFINITY);
        assert_eq!(anywhere.moved(Vec3::new(1.0, 5.0, 2.0)).y1, f32::INFINITY, "an unknown height stays every height when moved");
    }
}

/// Ship homes increment 7, lane M (docs/design/ship-homes-increment-7-plan.md section 3): the sweep
/// reads the heights Wave 0 gave every segment, so a wall stops only the bodies at its height.
#[cfg(test)]
mod storey_walk_tests {
    use super::*;

    /// A two-storey 10 x 10 x 6 m body, one slab over all of it at 3.0: a wall UPSTAIRS only
    /// (x = 5, base 3.0), a wall DOWNSTAIRS only (z = 3, the ground to 3.0), and a wall from the
    /// ground to the roof RISING THROUGH THE SLAB (z = 7). No doors.
    fn two_storeys() -> Vec<WallSegment> {
        let body: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, \
             walls: [(a: (5.0, 0.0), b: (5.0, 10.0), base: 3.0, height: 3.0), \
                     (a: (0.0, 3.0), b: (10.0, 3.0), height: 3.0), \
                     (a: (0.0, 7.0), b: (10.0, 7.0), height: 6.0)], \
             floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)])",
        )
        .expect("the test body parses");
        wall_segments(&body)
    }

    /// Walk from `from` toward `to` 0.1 m a frame (each frame's step swept against `walls` and the
    /// shut `doors`), the eye's height held: where the walk ends.
    fn walk(from: Vec3, to: Vec3, walls: &[WallSegment], doors: &[WallSegment]) -> Vec3 {
        let mut p = from;
        for _ in 0..400 {
            let left = Vec3::new(to.x - p.x, 0.0, to.z - p.z);
            if left.length() < 1e-3 {
                break;
            }
            p = resolve(p, p + left.normalize() * left.length().min(0.1), PLAYER_RADIUS, walls, doors);
        }
        p
    }

    /// The eye of a body standing on the ground floor, and of one standing on the slab.
    const DOWN: f32 = EYE_HEIGHT_M;
    const UP: f32 = 3.0 + EYE_HEIGHT_M;

    /// A WALL UPSTAIRS DOES NOT STOP A BODY DOWNSTAIRS, AND A WALL DOWNSTAIRS DOES NOT STOP ONE
    /// UPSTAIRS; A GROUND WALL RISING THROUGH THE SLAB STOPS BOTH. Each walk crosses one wall's
    /// line: across the upstairs wall (x 5) the body downstairs walks on to x 8 and the one
    /// upstairs stops short of it; across the downstairs wall (z 3) the other way about; across the
    /// wall through the slab (z 7) both stop.
    ///
    /// Seen red 2026-10-20 with `stops_body` answering true for every segment (every segment
    /// stopping every body, the sweep before this lane): "downstairs, the wall upstairs (x 5)
    /// stopped the walk at Vec3(4.67, 1.7, 5.0)".
    #[test]
    fn a_wall_stops_only_the_storey_it_stands_on() {
        let walls = two_storeys();
        let ends = |y: f32, from: (f32, f32), to: (f32, f32)| walk(Vec3::new(from.0, y, from.1), Vec3::new(to.0, y, to.1), &walls, &[]);
        let across_upstairs = ((3.0, 5.0), (8.0, 5.0));
        let across_downstairs = ((2.0, 1.0), (2.0, 5.0));
        let through_the_slab = ((2.0, 5.0), (2.0, 9.0));

        let e = ends(DOWN, across_upstairs.0, across_upstairs.1);
        assert!((e.x - 8.0).abs() < 1e-3, "downstairs, the wall upstairs (x 5) stopped the walk at {e:?}");
        let e = ends(UP, across_upstairs.0, across_upstairs.1);
        assert!(e.x < 5.0 - PLAYER_RADIUS + 0.05, "upstairs, the wall upstairs (x 5) let the walk through to {e:?}");

        let e = ends(UP, across_downstairs.0, across_downstairs.1);
        assert!((e.z - 5.0).abs() < 1e-3, "upstairs, the wall downstairs (z 3) stopped the walk at {e:?}");
        let e = ends(DOWN, across_downstairs.0, across_downstairs.1);
        assert!(e.z < 3.0 - PLAYER_RADIUS + 0.05, "downstairs, the wall downstairs (z 3) let the walk through to {e:?}");

        for (y, storey) in [(DOWN, "downstairs"), (UP, "upstairs")] {
            let e = ends(y, through_the_slab.0, through_the_slab.1);
            assert!(e.z < 7.0 - PLAYER_RADIUS + 0.05, "{storey}, the wall rising through the slab (z 7) let the walk through to {e:?}");
        }
    }

    /// A DOOR SHUT ON ONE STOREY IS NO WALL ON THE OTHER. A shut 2.1 m door panel in a doorway on
    /// the ground floor (its bottom at 0) and one upstairs (its bottom on the slab, 3.0), each the
    /// segment the walk makes of it (`door_panel_segment`): each stops the body on its own storey
    /// and lets the body on the other walk through its doorway's line.
    ///
    /// Seen red 2026-10-20 with `stops_body` answering true for every segment: "the shut door
    /// upstairs stopped the walk downstairs at Vec3(5.0, 1.7, 4.65)".
    #[test]
    fn a_shut_door_stops_only_its_own_storey() {
        let door_at = |sill: f32| door_panel_segment(Vec3::new(5.0, sill, 5.0), Quat::IDENTITY, Vec3::new(2.0, 2.1, 0.05));
        let (ground, upstairs) = (door_at(0.0), door_at(3.0));
        let through = |y: f32, door: WallSegment| walk(Vec3::new(5.0, y, 3.0), Vec3::new(5.0, y, 7.0), &[], &[door]);
        let e = through(DOWN, upstairs);
        assert!((e.z - 7.0).abs() < 1e-3, "the shut door upstairs stopped the walk downstairs at {e:?}");
        let e = through(UP, ground);
        assert!((e.z - 7.0).abs() < 1e-3, "the shut door downstairs stopped the walk upstairs at {e:?}");
        let e = through(DOWN, ground);
        assert!(e.z < 5.0 - PLAYER_RADIUS + 0.06, "the shut door downstairs let the walk downstairs through to {e:?}");
        let e = through(UP, upstairs);
        assert!(e.z < 5.0 - PLAYER_RADIUS + 0.06, "the shut door upstairs let the walk upstairs through to {e:?}");
    }

    /// A GUARD STOPS A WALK ON ITS STOREY AND HIDES NOTHING. Upstairs on a slab at 3.0: a guard
    /// drawn with the wall tool (a 1.0 m Steel balustrade wall, material 9, z = 5) and a guard
    /// piece placed on its own (the 1.2 m `guard` section at (5, 3, 8), across x 4.4 to 5.6). Each
    /// stops a body walking upstairs into it, lets a body downstairs walk under it, and is no
    /// sight blocker (a balustrade is see-through), so nameplates beyond it stay shown.
    ///
    /// Seen red 2026-10-20 with the placed guard left out of the walking segments: "upstairs, the
    /// placed guard let the walk through to Vec3(5.0, 4.7, 9.5)"; and with the guard wall kept in
    /// the sight segments: "the guard wall hides what is beyond it".
    #[test]
    fn a_guard_stops_a_walk_on_its_storey_and_hides_nothing() {
        let body: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, \
             walls: [(a: (0.0, 5.0), b: (10.0, 5.0), base: 3.0, height: 1.0, material: 9)], \
             floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)], \
             structures: [(id: \"guard-1\", type_id: \"guard\", pos: (5.0, 3.0, 8.0))])",
        )
        .expect("the test body parses");
        assert!(body.walls[0].guard().is_some(), "material 9 is a guard");
        let walls = wall_segments(&body);
        let ends = |y: f32, from: (f32, f32), to: (f32, f32)| walk(Vec3::new(from.0, y, from.1), Vec3::new(to.0, y, to.1), &walls, &[]);
        let at_the_wall = ((2.0, 3.0), (2.0, 7.0));
        let at_the_piece = ((5.0, 6.5), (5.0, 9.5));
        let e = ends(UP, at_the_wall.0, at_the_wall.1);
        assert!(e.z < 5.0 - PLAYER_RADIUS + 0.05, "upstairs, the guard wall let the walk through to {e:?}");
        let e = ends(UP, at_the_piece.0, at_the_piece.1);
        assert!(e.z < 8.0 - PLAYER_RADIUS + 0.05, "upstairs, the placed guard let the walk through to {e:?}");
        for (from, to) in [at_the_wall, at_the_piece] {
            let e = ends(DOWN, from, to);
            assert!((e.z - to.1).abs() < 1e-3, "downstairs, a guard upstairs stopped the walk at {e:?}");
        }
        let sight: Vec<[f32; 4]> = sight_segments_with_shell_cuts(&body, &[]).iter().map(|s| [s.a.0, s.a.1, s.b.0, s.b.1]).collect();
        assert!(!sight_blocked((2.0, 3.0), (2.0, 7.0), &sight), "the guard wall hides what is beyond it");
        assert!(!sight_blocked((5.0, 6.5), (5.0, 9.5), &sight), "the placed guard hides what is beyond it");
    }

    /// A DOOR ON THE GROUND IS NO WAY THROUGH THE WALL OVER IT UPSTAIRS. A wall from the ground to
    /// the roof (z 5, rising through the slab at 3.0) with a 2.1 m door at x 4 to 6: the door lets
    /// a body on the ground floor through, and the wall over the door stops a body upstairs
    /// walking along the same line, the collider's twin of the mesh, which draws the wall there.
    /// The same rule as the shell over a corridor's or a plot door's cut.
    ///
    /// Seen red first (2026-10-21) with a door cut out of its wall at every height: "upstairs,
    /// the wall over the door downstairs let the walk through to Vec3(5.0, 4.7, 6.999996)".
    #[test]
    fn a_door_on_the_ground_is_no_way_through_the_wall_over_it_upstairs() {
        let body: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, \
             walls: [(a: (0.0, 5.0), b: (10.0, 5.0), height: 6.0, \
                      openings: [(kind: Door, at: 4.0, width: 2.0, height: 2.1)])], \
             floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)])",
        )
        .expect("the test body parses");
        let walls = wall_segments(&body);
        let e = walk(Vec3::new(5.0, DOWN, 3.0), Vec3::new(5.0, DOWN, 7.0), &walls, &[]);
        assert!((e.z - 7.0).abs() < 1e-3, "downstairs, the door stopped the walk at {e:?}");
        let e = walk(Vec3::new(5.0, UP, 3.0), Vec3::new(5.0, UP, 7.0), &walls, &[]);
        assert!(e.z < 5.0 - PLAYER_RADIUS + 0.05, "upstairs, the wall over the door downstairs let the walk through to {e:?}");
    }

    /// ANY STAIR'S SIDE AND HIGH END STOP A WALK ON THE FLOOR IT STANDS ON, AND NOT ONE OVER ITS
    /// TOP. A 'stairs' piece placed at (5, 0, 5) turned 90 degrees (climbing east, its foot at
    /// x 2.75, its high end at x 7.25, its sides at z 4.4 and 5.6): walking north into its side,
    /// and west into its high end, the body on the floor is held at the solid; a body upstairs
    /// (feet 3.0, level with its top step) walks over it.
    ///
    /// Seen red first (2026-10-21) with no stair in the walls: "on the floor, the walk north into
    /// the stair's side went into it to Vec3(4.0, 1.7, 3.0000048)".
    #[test]
    fn any_stairs_side_and_high_end_stop_a_walk_on_its_floor() {
        let body: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, \
             structures: [(id: \"stairs-1\", type_id: \"stairs\", pos: (5.0, 0.0, 5.0), rot_deg: 90.0)])",
        )
        .expect("the test body parses");
        let walls = wall_segments(&body);
        let e = walk(Vec3::new(4.0, DOWN, 8.0), Vec3::new(4.0, DOWN, 3.0), &walls, &[]);
        assert!(e.z > 5.6 + PLAYER_RADIUS - 0.05, "on the floor, the walk north into the stair's side went into it to {e:?}");
        let e = walk(Vec3::new(9.5, DOWN, 5.0), Vec3::new(4.0, DOWN, 5.0), &walls, &[]);
        assert!(e.x > 7.25 + PLAYER_RADIUS - 0.05, "on the floor, the walk west into the stair's high end went into it to {e:?}");
        let e = walk(Vec3::new(4.0, UP, 8.0), Vec3::new(4.0, UP, 3.0), &walls, &[]);
        assert!((e.z - 3.0).abs() < 1e-3, "level with its top, the stair stopped the walk at {e:?}");
    }

    /// The band itself: a segment stops a body only across its knee (`KNEE_M` over the feet) to
    /// its eye, so a 0.3 m kerb is stepped over, a 2.2 m-high lintel over the head lets it pass,
    /// and a segment of unknown height stops everything.
    ///
    /// Seen red 2026-10-20 with `stops_body` answering true for every segment: "a kerb under the
    /// knee stops the walk".
    #[test]
    fn the_band_runs_from_the_knee_to_the_eye() {
        let seg = |y0: f32, y1: f32| WallSegment { a: (0.0, 0.0), b: (1.0, 0.0), half_thickness: 0.05, y0, y1, kind: SegmentKind::Wall };
        assert!(!seg(0.0, 0.3).stops_body(DOWN), "a kerb under the knee stops the walk");
        assert!(seg(0.0, 0.5).stops_body(DOWN), "a 0.5 m box over the knee does not stop the walk");
        assert!(!seg(2.2, 3.0).stops_body(DOWN), "a lintel over the head stops the walk");
        assert!(seg(1.0, 1.2).stops_body(DOWN), "a rail at the chest does not stop the walk");
        assert!(WallSegment::at_every_height((0.0, 0.0), (1.0, 0.0), 0.05).stops_body(UP), "a segment of unknown height lets a body by");
    }
}

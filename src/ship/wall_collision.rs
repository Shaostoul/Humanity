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

use crate::ship::home_structure::{HomeStructure, OpeningKind, ShellCut};
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
    /// The sweep reads them (`stops_body`): a segment stops only a body whose knee-to-eye band it
    /// stands across. A blocker whose heights are not known stands at every height
    /// (`WallSegment::at_every_height`), which is what every segment did before it had them.
    pub y0: f32,
    pub y1: f32,
}

impl WallSegment {
    /// True when this segment stops a body whose eye is at world height `eye_y`: it stands
    /// somewhere across that body's knee-to-eye band, from `KNEE_M` over its feet (`EYE_HEIGHT_M`
    /// under the eye) up to the eye. A wall upstairs (3..6) is over the head of a body on the
    /// ground floor (band 0.4..1.7) and a ground wall (0..3) under the knees of one upstairs (band
    /// 3.4..4.7); a wall from the ground to the roof stops both. A segment at every height stops
    /// every body.
    pub fn stops_body(&self, eye_y: f32) -> bool {
        let knee = eye_y - EYE_HEIGHT_M + KNEE_M;
        self.y0 < eye_y && self.y1 > knee
    }

    /// A segment standing at every height (`y0` minus infinity, `y1` plus infinity): for a blocker
    /// whose heights are not known, which then stops a body at any height.
    pub fn at_every_height(a: (f32, f32), b: (f32, f32), half_thickness: f32) -> Self {
        WallSegment { a, b, half_thickness, y0: f32::NEG_INFINITY, y1: f32::INFINITY }
    }

    /// This segment moved by `by`: a body's walls (body-local) placed at its origin in the world.
    pub fn moved(self, by: Vec3) -> Self {
        WallSegment {
            a: (self.a.0 + by.x, self.a.1 + by.z),
            b: (self.b.0 + by.x, self.b.1 + by.z),
            half_thickness: self.half_thickness,
            y0: self.y0 + by.y,
            y1: self.y1 + by.y,
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
    }
}

/// Build the STATIC blocking segments from a home (and every guard piece placed in it on its own,
/// ship homes increment 7): the 4 perimeter walls + each interior wall's SOLID
/// pier spans. DOOR apertures are cut out (so a doorway is a walk-through gap); WINDOW spans stay
/// solid (you can't walk through glass -- the pane + the sill wall block the whole width). Doors get a
/// separate LIVE collider in `resolve`'s `doors` arg, because their open state changes every frame.
pub fn wall_segments(home: &HomeStructure) -> Vec<WallSegment> {
    wall_segments_with_shell_cuts(home, &[])
}

/// `wall_segments` with corridor APERTURES cut out of the perimeter (ship-superstructure increment
/// B, the collision twin of `generate_meshes_with_shell_cuts`): where a corridor tube meets the
/// box, the perimeter gets a door-width walk-through gap instead of a solid wall. With no cuts
/// this is exactly the pre-B path.
pub fn wall_segments_with_shell_cuts(home: &HomeStructure, shell_cuts: &[ShellCut]) -> Vec<WallSegment> {
    segments_impl(home, shell_cuts, false)
}

/// SIGHT blockers for one home (v0.975, nameplate-through-wall polish): same spans as
/// `wall_segments_with_shell_cuts` but with WINDOW apertures cut out too -- you can't walk
/// through glass, but you can SEE through it, so a machine card behind a window stays
/// visible while one behind a solid pier hides. Door apertures are open here exactly as in
/// the collision spans; a CLOSED door blocks sight via the live door list the caller adds
/// (the same split `resolve` uses). A guard (a balustrade, `InteriorWall::guard`, or a guard
/// piece placed on its own) is see-through and blocks no sight (ship homes increment 7). The
/// sight check is across the floor only (`sight_blocked`): a wall on another storey still hides a
/// nameplate whose line it crosses; the heights are in each segment for when it reads them.
pub fn sight_segments_with_shell_cuts(home: &HomeStructure, shell_cuts: &[ShellCut]) -> Vec<WallSegment> {
    segments_impl(home, shell_cuts, true)
}

fn segments_impl(home: &HomeStructure, shell_cuts: &[ShellCut], see_through_windows: bool) -> Vec<WallSegment> {
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
        let mut cuts: Vec<(f32, f32)> = shell_cuts
            .iter()
            .filter(|c| c.edge == ei)
            .map(|c| (c.at.clamp(0.0, len), (c.at + c.width).clamp(0.0, len)))
            .collect();
        if cuts.is_empty() {
            segs.push(WallSegment { a, b, half_thickness: st, y0: py0, y1: py1 });
            continue;
        }
        cuts.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
        let (ux, uz) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let mut cursor = 0.0f32;
        for (s, e) in cuts.iter().chain(std::iter::once(&(len, len))) {
            if s - cursor > 0.02 {
                segs.push(WallSegment {
                    a: (a.0 + ux * cursor, a.1 + uz * cursor),
                    b: (a.0 + ux * s, a.1 + uz * s),
                    half_thickness: st,
                    y0: py0,
                    y1: py1,
                });
            }
            cursor = e.max(cursor);
        }
    }
    for wall in &home.walls {
        // A GUARD is a balustrade (ship homes increment 7, `InteriorWall::guard`): posts and
        // balusters under 100 mm apart and a top rail. It stops a walk like the wall it is, but
        // it is see-through, so it is no sight blocker: a 1.0 m balustrade round a stairwell does
        // not hide the nameplates beyond it.
        if see_through_windows && wall.guard().is_some() {
            continue;
        }
        let ht = wall.resolved_thickness() * 0.5;
        let (wy0, wy1) = wall.y_range();
        let (ax, az) = wall.a;
        let (bx, bz) = wall.b;
        let len = ((bx - ax).powi(2) + (bz - az).powi(2)).sqrt();
        if len < 1e-4 {
            continue;
        }
        let (ux, uz) = ((bx - ax) / len, (bz - az) / len);
        // DOOR cut intervals along the wall, clamped + sorted. For COLLISION windows are NOT
        // cut (glass blocks the walker); for SIGHT they are (glass is transparent).
        let mut cuts: Vec<(f32, f32)> = wall
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door || (see_through_windows && o.kind == OpeningKind::Window))
            .map(|o| (o.at.clamp(0.0, len), (o.at + o.width).clamp(0.0, len)))
            .collect();
        cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        // Emit a solid segment for each gap between the door cuts (and after the last one).
        let mut cursor = 0.0f32;
        for (s, e) in cuts.iter().chain(std::iter::once(&(len, len))) {
            if s - cursor > 0.02 {
                segs.push(WallSegment {
                    a: (ax + ux * cursor, az + uz * cursor),
                    b: (ax + ux * s, az + uz * s),
                    half_thickness: ht,
                    y0: wy0,
                    y1: wy1,
                });
            }
            cursor = e.max(cursor);
        }
    }
    // A GUARD PLACED ON ITS OWN (the `guard` structure piece, kind Guard, a 1.2 m section; ship
    // homes increment 7): it stops a walk across its length, from its foot to its rail, like a
    // guard drawn with the wall tool. Not a sight blocker (a balustrade is see-through). The
    // segment runs along the piece's width through its middle, turned with it.
    if !see_through_windows {
        for ps in &home.structures {
            let Some(ty) = crate::ship::structure::structure_type(&ps.type_id) else { continue };
            if ty.kind != crate::ship::structure::StructureKind::Guard {
                continue;
            }
            let (s, c) = ps.rot_deg.to_radians().sin_cos();
            let hw = ty.size.0 * 0.5;
            // A piece's local (x, 0) lies at world (x cos, -x sin) from its middle
            // (`structure::structure_mesh`'s turn).
            let (dx, dz) = (hw * c, -hw * s);
            segs.push(WallSegment {
                a: (ps.pos.0 - dx, ps.pos.2 - dz),
                b: (ps.pos.0 + dx, ps.pos.2 + dz),
                half_thickness: (ty.size.2 * 0.5).max(0.03),
                y0: ps.pos.1,
                y1: ps.pos.1 + ty.size.1,
            });
        }
    }
    segs
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
    [-1.0f32, 1.0].map(|s| match g.axis {
        CorridorAxis::X => WallSegment { a: (g.start, g.lat + hw * s), b: (g.end, g.lat + hw * s), half_thickness: ht, y0, y1 },
        CorridorAxis::Z => WallSegment { a: (g.lat + hw * s, g.start), b: (g.lat + hw * s, g.end), half_thickness: ht, y0, y1 },
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

/// One push-out pass: shove the point out of every penetrated segment, iterated a few times so
/// inside-corners settle. Y is preserved. A cheap per-segment range reject bounds the cost. Used per
/// substep by `resolve`. `pos` is the body's eye: a segment that does not stand across its
/// knee-to-eye band (`WallSegment::stops_body`) is not in its way at all.
fn resolve_once(pos: Vec3, radius: f32, walls: &[WallSegment], doors: &[WallSegment]) -> Vec3 {
    let mut px = pos.x;
    let mut pz = pos.z;
    for _ in 0..3 {
        for seg in walls.iter().chain(doors.iter()) {
            if !seg.stops_body(pos.y) {
                continue;
            }
            let push = radius + seg.half_thickness;
            // Range reject: skip segments whose inflated AABB doesn't contain the player point.
            let (minx, maxx) = (seg.a.0.min(seg.b.0) - push, seg.a.0.max(seg.b.0) + push);
            let (minz, maxz) = (seg.a.1.min(seg.b.1) - push, seg.a.1.max(seg.b.1) + push);
            if px < minx || px > maxx || pz < minz || pz > maxz {
                continue;
            }
            let (cx, cz, dist) = closest_on_seg(px, pz, seg.a, seg.b);
            if dist > 1e-4 {
                if dist < push {
                    let overlap = push - dist;
                    px += (px - cx) / dist * overlap;
                    pz += (pz - cz) / dist * overlap;
                }
            } else {
                // Exactly on the centerline: push out along the segment's normal.
                let (sx, sz) = (seg.b.0 - seg.a.0, seg.b.1 - seg.a.1);
                let slen = (sx * sx + sz * sz).sqrt().max(1e-5);
                px += -sz / slen * push;
                pz += sx / slen * push;
            }
        }
    }
    Vec3::new(px, pos.y, pz)
}

/// Resolve the player's move from `prev` to `pos` against the walls + live door segments, sliding
/// along surfaces. SUBSTEPS the movement so a fast/sprinting player (or a frame hitch -- dt is clamped
/// to 0.1 s) can't TUNNEL through a thin wall or a closed door in one frame: each substep advances
/// less than the thinnest collision corridor (~radius + a door's 0.05 m), so a wall in the path is
/// always penetrated and pushed out of. The push-out from each substep carries into the next, so the
/// player slides instead of snapping back onto the raw path. Y is taken from `pos`, which is the
/// body's EYE (`EYE_HEIGHT_M` over its feet): only the segments standing across the body's
/// knee-to-eye band there stop it (`WallSegment::stops_body`, ship homes increment 7).
pub fn resolve(prev: Vec3, pos: Vec3, radius: f32, walls: &[WallSegment], doors: &[WallSegment]) -> Vec3 {
    let (dx, dz) = (pos.x - prev.x, pos.z - prev.z);
    let dist = (dx * dx + dz * dz).sqrt();
    // Substep size must be below the thinnest corridor; 0.15 m is well under radius (0.3). Bound the
    // step count so a teleport doesn't spin (beyond ~7 m it just resolves at the destination).
    const MAX_STEP: f32 = 0.15;
    const MAX_SUBSTEPS: usize = 48;
    let n = ((dist / MAX_STEP).ceil() as usize).clamp(1, MAX_SUBSTEPS);
    let (sx, sz) = (dx / n as f32, dz / n as f32);
    let mut p = Vec3::new(prev.x, pos.y, prev.z);
    for _ in 0..n {
        p.x += sx;
        p.z += sz;
        p = resolve_once(p, radius, walls, doors);
    }
    p
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

    #[test]
    fn sight_passes_windows_and_doors_but_not_piers() {
        let h = home_with_window();
        let sight = flat(&sight_segments_with_shell_cuts(&h, &[]));
        // Through the solid pier (x=1): blocked.
        assert!(sight_blocked((1.0, 3.0), (1.0, 7.0), &sight), "pier must block sight");
        // Through the door gap (x=5): clear.
        assert!(!sight_blocked((5.0, 3.0), (5.0, 7.0), &sight), "door gap must pass sight");
        // Through the window (x=7.75): clear for SIGHT...
        assert!(!sight_blocked((7.75, 3.0), (7.75, 7.0), &sight), "window must pass sight");
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

    /// The band itself: a segment stops a body only across its knee (`KNEE_M` over the feet) to
    /// its eye, so a 0.3 m kerb is stepped over, a 2.2 m-high lintel over the head lets it pass,
    /// and a segment of unknown height stops everything.
    ///
    /// Seen red 2026-10-20 with `stops_body` answering true for every segment: "a kerb under the
    /// knee stops the walk".
    #[test]
    fn the_band_runs_from_the_knee_to_the_eye() {
        let seg = |y0: f32, y1: f32| WallSegment { a: (0.0, 0.0), b: (1.0, 0.0), half_thickness: 0.05, y0, y1 };
        assert!(!seg(0.0, 0.3).stops_body(DOWN), "a kerb under the knee stops the walk");
        assert!(seg(0.0, 0.5).stops_body(DOWN), "a 0.5 m box over the knee does not stop the walk");
        assert!(!seg(2.2, 3.0).stops_body(DOWN), "a lintel over the head stops the walk");
        assert!(seg(1.0, 1.2).stops_body(DOWN), "a rail at the chest does not stop the walk");
        assert!(WallSegment::at_every_height((0.0, 0.0), (1.0, 0.0), 0.05).stops_body(UP), "a segment of unknown height lets a body by");
    }
}

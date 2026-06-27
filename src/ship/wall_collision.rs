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
//! (you cannot walk through glass, and a shut door is a wall). Y is never touched.

use crate::ship::home_structure::{HomeStructure, OpeningKind};
use glam::Vec3;

/// Player capsule radius (metres) for the horizontal push-out.
pub const PLAYER_RADIUS: f32 = 0.3;

/// A blocking wall segment in the XZ plane (a..b), inflated by `half_thickness`.
#[derive(Clone, Copy, Debug)]
pub struct WallSegment {
    pub a: (f32, f32),
    pub b: (f32, f32),
    pub half_thickness: f32,
}

/// Build the STATIC blocking segments from a home: the 4 perimeter walls + each interior wall's SOLID
/// pier spans. DOOR apertures are cut out (so a doorway is a walk-through gap); WINDOW spans stay
/// solid (you can't walk through glass -- the pane + the sill wall block the whole width). Doors get a
/// separate LIVE collider in `resolve`'s `doors` arg, because their open state changes every frame.
pub fn wall_segments(home: &HomeStructure) -> Vec<WallSegment> {
    let mut segs = Vec::new();
    let (w, d) = (home.width.max(1.0), home.depth.max(1.0));
    let st = home.shell_resolved_thickness() * 0.5;
    for (a, b) in [
        ((0.0, 0.0), (w, 0.0)),
        ((w, 0.0), (w, d)),
        ((w, d), (0.0, d)),
        ((0.0, d), (0.0, 0.0)),
    ] {
        segs.push(WallSegment { a, b, half_thickness: st });
    }
    for wall in &home.walls {
        let ht = wall.resolved_thickness() * 0.5;
        let (ax, az) = wall.a;
        let (bx, bz) = wall.b;
        let len = ((bx - ax).powi(2) + (bz - az).powi(2)).sqrt();
        if len < 1e-4 {
            continue;
        }
        let (ux, uz) = ((bx - ax) / len, (bz - az) / len);
        // DOOR cut intervals along the wall (windows are NOT cut -- glass blocks), clamped + sorted.
        let mut cuts: Vec<(f32, f32)> = wall
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
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
                });
            }
            cursor = e.max(cursor);
        }
    }
    segs
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
/// substep by `resolve`.
fn resolve_once(pos: Vec3, radius: f32, walls: &[WallSegment], doors: &[WallSegment]) -> Vec3 {
    let mut px = pos.x;
    let mut pz = pos.z;
    for _ in 0..3 {
        for seg in walls.iter().chain(doors.iter()) {
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
/// player slides instead of snapping back onto the raw path. Y is taken from `pos`.
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
            shell_thickness: None,
            walls: vec![InteriorWall {
                a: (0.0, 5.0),
                b: (10.0, 5.0),
                height: 3.0,
                material: 1,
                thickness: Some(0.2),
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
        let door = WallSegment { a: (4.0, 5.0), b: (6.0, 5.0), half_thickness: 0.05 };
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
    fn fast_move_does_not_tunnel_a_thin_wall() {
        // Sprint straight through the solid pier (x=1) of the 0.2 m wall at z=5, from z=4 to z=7 in
        // ONE call. Substepping must STOP the player on the near side, not let them tunnel to z=7.
        let segs = wall_segments(&home());
        let out = resolve(Vec3::new(1.0, 1.7, 4.0), Vec3::new(1.0, 1.7, 7.0), PLAYER_RADIUS, &segs, &[]);
        assert!(out.z < 4.9, "blocked on the near side of the wall, got z={}", out.z);
    }
}

//! FURNITURE AGAINST ITS WALL (2026-11-14, BUG-309, found by BUG-306's furniture reader): every piece placed in every
//! shipped home design stands flush against the wall it was put against, and none stands in a wall.
//!
//! Seen by the storeys rig's furniture reader (scripts/lib/home-furniture.js) that day: the
//! homestead's dressing-room mirror (mirror_2, data/machines/home.ron) hung at x 44.75 with the face
//! of the upper wall U7 at 44.95, 0.185 m out in the air, and stopped a body before the wall did;
//! the bedroom's shelf (shelf_6) stood 0.08 m into the bedroom's east wall. Nothing caught either:
//! `every_placed_machine_stands_inside_the_room_it_names` holds a piece's centre in its room, not
//! its sides against the walls, and since 2026-11-13 the player's walk stops at every piece
//! (`Footprint::of_placed`), so a piece off its wall is a ledge in the air a body walks into.
//!
//! The rule, over what the game draws and collides with (`MachineHome::placements` in the home's
//! zone, the 0.3 m clamp included, turned into `Footprint::of_placed`'s box, against the walls of
//! `wall_collision::wall_segments`, each between its heights):
//! - **No piece stands in a wall** by more than `TOL_M`: its box (a cylinder's or a sphere's
//!   circle) overlaps no wall's thickness at the heights both stand at.
//! - **A piece put against a wall is flush with it**: a piece on the floor with a side parallel to
//!   a wall face and within `CATCH_M` of it stands within `TOL_M` of it (a gap of a few centimetres
//!   behind a wardrobe or a shelf is a placement error, not a choice; there is no skirting board).
//! - **A piece off the floor hangs on a wall** (a mirror, a wall-hung basin, a wall screen): it has
//!   a wall face parallel to one of its sides, alongside it, within `TOL_M`. Nothing holds a mirror
//!   up 0.185 m out from the wall.
//!
//! Every shipped design is checked with every household's machine file its starts give it
//! (data/homes/starts.ron), so a new home or a new household is checked by being added.

use crate::crowd_nav::Footprint;
use crate::machines::{MachineHome, PlacedMachine, ZoneRect};
use crate::ship_structure::HomeDesign;
use crate::starts::Starts;
use crate::wall_collision::{wall_segments, SegmentKind, WallSegment};
use std::collections::{BTreeSet, HashMap};

/// How far a piece may stand off the wall it is against, or into a wall, and still be right: a few
/// millimetres, the rounding of a hand-typed offset (44.935 for 44.95 less half of 0.03).
const TOL_M: f32 = 0.005;
/// A piece on the floor whose side is parallel to a wall face and this close to it was put against
/// that wall. Further off is a choice (a chair pulled out, a bed with a walkway beside it).
const CATCH_M: f32 = 0.10;
/// A piece whose foot is this far over its storey's floor is off the floor: it stands on another
/// piece (a monitor on a desk, a router on a shelf) or it hangs.
const HANG_M: f32 = 0.05;
/// A hanging piece with a side parallel to a wall face and this close to it hangs on that wall (the
/// mirror was 0.185 m off). Further from every wall it hangs from something else (a ceiling unit, a
/// clip-on fan), which is not this check's.
const HANG_CATCH_M: f32 = 0.30;
/// How much of a piece's side must run alongside a wall for the wall to be the one it is against.
const ALONGSIDE_M: f32 = 0.05;
/// Two directions are parallel within this sine (about 1 degree).
const PARALLEL_SIN: f32 = 0.0175;

fn repo() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT"))
}

/// The shipped design of a kind, as the checkout holds it; None when the file is absent (a start
/// not offered yet, the apartment).
fn shipped_design(kind: &str) -> Option<HomeDesign> {
    let p = repo().join("data").join("homes").join("shipped").join(format!("{kind}.ron"));
    let text = std::fs::read_to_string(&p).ok()?;
    Some(ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
}

/// Every (design kind, design, machine file) the starts ship: each start's design with each of its
/// households' machine files, once each.
fn shipped_homes() -> Vec<(String, HomeDesign, String)> {
    let starts = Starts::load(&repo().join("data"));
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for s in &starts.starts {
        let Some(design) = shipped_design(&s.design) else {
            assert!(!s.offered, "start {} is offered but data/homes/shipped/{}.ron is missing", s.id, s.design);
            continue;
        };
        for h in &s.households {
            let file = repo().join("data").join("machines").join(&h.machines);
            if !file.exists() {
                assert!(!s.offered, "start {} is offered but its household {} names data/machines/{}, which is missing", s.id, h.id, h.machines);
                continue;
            }
            if seen.insert((s.design.clone(), h.machines.clone())) {
                out.push((s.design.clone(), design.clone(), h.machines.clone()));
            }
        }
    }
    out
}

/// The household's pieces in the home's own zone as the game draws them (`placements`, with the
/// home's zone at the design's origin, so the 0.3 m clamp applies as in the game).
fn placed(design: &HomeDesign, home: &MachineHome) -> Vec<PlacedMachine> {
    let b = &design.body;
    let zones = vec![ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (b.width, b.depth, b.height) }];
    let in_home: BTreeSet<String> = home.all_instances().into_iter().filter(|i| i.zone == "home").map(|i| i.id).collect();
    home.placements(&HashMap::new(), Some(&zones)).into_iter().filter(|p| in_home.contains(&p.id)).collect()
}

/// A footprint's corners, world (home-local) x and z, as `Footprint::walls` turns them.
fn corners(f: &Footprint) -> [(f32, f32); 4] {
    let (s, c) = f.yaw.sin_cos();
    let world = |lx: f32, lz: f32| (f.centre.0 + c * lx + s * lz, f.centre.1 - s * lx + c * lz);
    let (hx, hz) = f.half;
    [world(-hx, -hz), world(hx, -hz), world(hx, hz), world(-hx, hz)]
}

/// True when the point (x, z) is on the footprint, plan view.
fn covers(f: &Footprint, at: (f32, f32)) -> bool {
    let (s, c) = f.yaw.sin_cos();
    let (dx, dz) = (at.0 - f.centre.0, at.1 - f.centre.1);
    // Turned back by the yaw: the inverse of `corners`' turn.
    let (lx, lz) = (c * dx - s * dz, s * dx + c * dz);
    lx.abs() <= f.half.0 + TOL_M && lz.abs() <= f.half.1 + TOL_M
}

/// One piece beside one wall: how far its nearest side stands off the wall's face (negative: into
/// the wall), how much of it runs alongside the wall, and whether a side of it is parallel to the
/// wall. None when the two stand at different heights or the piece is not alongside the wall.
struct Beside {
    gap: f32,
    along: f32,
    parallel: bool,
    /// The wall's normal toward the piece (x, z): the way to move the piece to change `gap`.
    toward: (f32, f32),
}

fn beside(p: &PlacedMachine, f: &Footprint, w: &WallSegment) -> Option<Beside> {
    let overlap_y = f.y1.min(w.y1) - f.y0.max(w.y0);
    if overlap_y <= 0.01 {
        return None;
    }
    let (dx, dz) = (w.b.0 - w.a.0, w.b.1 - w.a.1);
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1e-4 {
        return None;
    }
    let u = (dx / len, dz / len);
    let n = (-u.1, u.0);
    let s_of = |q: (f32, f32)| (q.0 - w.a.0) * u.0 + (q.1 - w.a.1) * u.1;
    let t_of = |q: (f32, f32)| (q.0 - w.a.0) * n.0 + (q.1 - w.a.1) * n.1;
    let ht = w.half_thickness;
    let round = p.shape == "cylinder" || p.shape == "sphere";
    let (smin, smax, tmin, tmax, parallel) = if round {
        let r = f.half.0;
        let (sc, tc) = (s_of(f.centre), t_of(f.centre));
        (sc - r, sc + r, tc - r, tc + r, false)
    } else {
        let cs = corners(f);
        let ss = cs.map(s_of);
        let ts = cs.map(t_of);
        let (s, c) = f.yaw.sin_cos();
        // The footprint's own x axis in the world is (c, -s); a box is parallel to the wall when
        // that axis is along the wall or across it.
        let cross = (u.0 * -s - u.1 * c).abs();
        let dot = (u.0 * c + u.1 * -s).abs();
        let parallel = cross < PARALLEL_SIN || dot < PARALLEL_SIN;
        (ss.iter().copied().fold(f32::MAX, f32::min), ss.iter().copied().fold(f32::MIN, f32::max), ts.iter().copied().fold(f32::MAX, f32::min), ts.iter().copied().fold(f32::MIN, f32::max), parallel)
    };
    let along = smax.min(len) - smin.max(0.0);
    if along <= TOL_M {
        return None;
    }
    let into = tmax.min(ht) - tmin.max(-ht);
    let (gap, toward) = if into > 0.0 {
        // In the wall: by how much, toward the side the piece's middle is on.
        let side = if t_of(f.centre) >= 0.0 { 1.0 } else { -1.0 };
        (-into, (n.0 * side, n.1 * side))
    } else if tmin >= ht {
        (tmin - ht, n)
    } else {
        (-ht - tmax, (-n.0, -n.1))
    };
    Some(Beside { gap, along, parallel, toward })
}

/// The wall a segment is, for a message: its ends, rounded to the centimetre.
fn wall_name(w: &WallSegment) -> String {
    format!("the wall from ({:.2}, {:.2}) to ({:.2}, {:.2}) ({:.2} to {:.2} m up)", w.a.0, w.a.1, w.b.0, w.b.1, w.y0, w.y1)
}

/// Where the piece's centre stands flush: moved along the wall's normal by the gap.
fn flush_at(p: &PlacedMachine, b: &Beside) -> String {
    format!("({:.3}, {:.3})", p.pos.0 - b.toward.0 * b.gap, p.pos.2 - b.toward.1 * b.gap)
}

/// The problems of one household's pieces in one design (each names the piece, the wall, how far
/// off or in, and where its centre would stand flush).
fn fit_problems(kind: &str, design: &HomeDesign, file: &str, home: &MachineHome) -> Vec<String> {
    let walls: Vec<WallSegment> = wall_segments(&design.body)
        .into_iter()
        .filter(|w| matches!(w.kind, SegmentKind::Wall | SegmentKind::Lintel) && w.half_thickness > 0.004)
        .collect();
    let floors: Vec<f32> = design.body.storeys().iter().map(|s| s.floor).collect();
    let pieces = placed(design, home);
    let prints: Vec<Footprint> = pieces.iter().map(Footprint::of_placed).collect();
    let mut out = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        let f = prints[i];
        let floor = floors.iter().copied().filter(|fl| *fl <= f.y0 + 0.02).fold(0.0, f32::max);
        let off_floor = f.y0 > floor + HANG_M;
        // Off the floor and standing on another piece (its foot on that piece's top, its middle
        // over it): it is that piece's, not a wall's, so only the in-a-wall half of the rule holds.
        let rests = off_floor && prints.iter().enumerate().any(|(j, g)| j != i && (g.y1 - f.y0).abs() <= 0.02 && covers(g, f.centre));
        let hangs = off_floor && !rests;
        let round = p.shape == "cylinder" || p.shape == "sphere";
        // The nearest wall a hanging piece could hang on: its gap and what to say of it.
        let mut mount: Option<(f32, String)> = None;
        for w in &walls {
            let Some(b) = beside(p, &f, w) else { continue };
            if b.gap < -TOL_M {
                out.push(format!("{kind} / {file}: {} ({}) stands {:.3} m into {}; flush, its centre is at {}", p.id, p.shape, -b.gap, wall_name(w), flush_at(p, &b)));
                continue;
            }
            if rests || round || !b.parallel || b.along < ALONGSIDE_M {
                continue;
            }
            if hangs {
                if mount.as_ref().map_or(true, |m| b.gap < m.0) {
                    mount = Some((b.gap, format!("{}; flush, its centre is at {}", wall_name(w), flush_at(p, &b))));
                }
            } else if b.gap > TOL_M && b.gap <= CATCH_M {
                out.push(format!("{kind} / {file}: {} stands {:.3} m off {}; flush, its centre is at {}", p.id, b.gap, wall_name(w), flush_at(p, &b)));
            }
        }
        if let Some((g, on)) = mount.filter(|m| m.0 > TOL_M && m.0 <= HANG_CATCH_M) {
            out.push(format!("{kind} / {file}: {} hangs {:.2} m over its floor, {:.3} m off {on}", p.id, f.y0 - floor, g));
        }
    }
    out
}

/// Every problem in every shipped home with every household's machines.
fn all_problems() -> Vec<String> {
    let mut out = Vec::new();
    for (kind, design, file) in shipped_homes() {
        let home = MachineHome::load(&repo().join("data").join("machines").join(&file)).unwrap_or_else(|| panic!("data/machines/{file} parses"));
        out.extend(fit_problems(&kind, &design, &file, &home));
    }
    out
}

/// Every piece in every shipped home flush with the wall it is against and in none.
///
/// RED on 2026-11-14's data (the 0.3 m clamp already replaced, `zone_world_pos`): 96 pieces in 9
/// machine files, 57 of them in home.ron, among them
/// "homestead / home.ron: mirror_2 hangs 1.10 m over its floor, 0.185 m off the wall from (45.00,
/// 30.00) to (45.00, 38.00) (3.00 to 6.00 m up); flush, its centre is at (44.935, 36.500)" and
/// "homestead / home.ron: shelf_6 (box) stands 0.080 m into the wall from (42.00, 35.90) to (42.00,
/// 38.00) (3.00 to 6.00 m up); flush, its centre is at (41.670, 36.600)". With the old clamp it was
/// 98, the cabin's nightstand and desk among them (pushed 0.3 m in, off the walls they were
/// written against). Each was moved to the flush centre the check names. The mirror's footprint then
/// took its model's own depth (0.266 m, bathroommirror.gltf at 0.8 m tall, not 0.03): flush as a
/// 0.03 m box, its frame stood 0.12 m inside the wall in the game. The second half moves the two
/// back on a copy (the mirror to 44.75, 0.067 m off with its real depth), so the check stays able
/// to fail.
#[test]
fn every_placed_piece_is_flush_with_its_wall_and_in_none() {
    let homes = shipped_homes();
    assert!(homes.len() >= 8, "every shipped design with its households is checked, got {}", homes.len());
    let p = all_problems();
    assert!(p.is_empty(), "{} piece(s) off or into a wall:\n{}", p.len(), p.join("\n"));

    let (kind, design, file) = homes.iter().find(|(k, _, f)| k == "homestead" && f == "home.ron").expect("the homestead's family home is checked");
    let mut home = MachineHome::load(&repo().join("data").join("machines").join(file)).expect("home.ron parses");
    for inst in home.instances.iter_mut() {
        match inst.id.as_str() {
            "mirror_2" => inst.offset.0 = 44.75,
            "shelf_6" => inst.offset.0 = 41.75,
            _ => {}
        }
    }
    let broken = fit_problems(kind, design, file, &home);
    assert!(broken.iter().any(|p| p.contains("mirror_2 hangs") && p.contains("0.067 m off")), "a mirror off its wall is caught: {broken:?}");
    assert!(broken.iter().any(|p| p.contains("shelf_6 (box) stands 0.080 m into")), "a shelf in its wall is caught: {broken:?}");
}

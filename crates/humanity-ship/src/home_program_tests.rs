//! THE ROOM PROGRAM'S HOMES, CHECKED (2026-11-13, docs/design/home-room-program.md).
//!
//! The two homesteads the operator approved that day (the homestead for a household of up to five,
//! data/homes/shipped/household_homestead.ron, and the homestead for one,
//! data/homes/shipped/one_person_homestead.ron) were drafted by a throwaway script that checked its
//! own layout file. These are those checks as tests over the DATA the game builds, so the drawings
//! (`node scripts/draw-homes.js`, which draws from the same files) and the game cannot disagree,
//! and an edit that breaks a layout fails the build instead of a walk-through:
//!
//! - each plot tiled by its ground zones with no gap or overlap, and the upper floor's zones over
//!   its slabs and the stairwell, with no overlap;
//! - every door on a wall between two rooms (and every window between a room and another room or
//!   the outside);
//! - every room reachable from the street through doors, the upper floor by the stair;
//! - every bedroom meeting England's space standard inside its walls (a double bed's room 11.5 m2,
//!   every bedroom 7.5 m2 and 2.15 m wide);
//! - every light inside a room on its storey, under its ceiling, and every room lit;
//! - no wall through a door (its opening or the passage through it);
//! - the flood fill finding every room its zones name, so each room has its function in the game;
//! - every machine of each household in the room it names.
//!
//! Each check is a function returning its problems, run once on the shipped data (none) and once
//! on a copy broken on purpose (the problem named): the second half is the check seen RED, kept so
//! it stays able to fail. A ZONE is one box of a room; a room drawn as several boxes (the L-shaped
//! greenhouse, orchard and hen range) has its primary zone, which carries the room type, and parts
//! named "<primary>-part-<n>" with the same label, joined to it with no wall between.

use crate::home_structure::{storey_index, wall_in_storey, OpeningKind, Storey, Zone};
use crate::ship_structure::HomeDesign;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const EPS: f32 = 1e-3;
type Rect = (f32, f32, f32, f32);

/// The kinds these tests hold, and their household machine files.
const KINDS: [(&str, &str); 2] = [("household_homestead", "household_homestead.ron"), ("one_person_homestead", "one_person_homestead.ron")];

fn repo() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT"))
}

/// The shipped design as the checkout holds it (so a check reruns on an edited file without a
/// rebuild); `the_shipped_files_are_what_the_exe_embeds` holds that it is the built-in copy.
fn design(kind: &str) -> HomeDesign {
    let p = repo().join("data").join("homes").join("shipped").join(format!("{kind}.ron"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn rect(z: &Zone) -> Rect {
    (z.origin.0, z.origin.2, z.origin.0 + z.size.0, z.origin.2 + z.size.2)
}
fn area(r: Rect) -> f32 {
    (r.2 - r.0).max(0.0) * (r.3 - r.1).max(0.0)
}
fn overlap(a: Rect, b: Rect) -> f32 {
    area((a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3)))
}
fn inside(r: Rect, x: f32, z: f32) -> bool {
    x > r.0 + EPS && x < r.2 - EPS && z > r.1 + EPS && z < r.3 - EPS
}

/// The room a zone belongs to: a part names its primary.
fn room_of(id: &str) -> &str {
    id.find("-part-").map_or(id, |i| &id[..i])
}

/// The zone holding (x, z) on storey `k`, if any.
fn zone_at<'a>(h: &'a HomeDesign, storeys: &[Storey], k: usize, x: f32, z: f32) -> Option<&'a Zone> {
    h.body.zones.iter().find(|zn| storey_index(storeys, zn.origin.1) == k && inside(rect(zn), x, z))
}

/// The point `t` metres along a wall from its corner `a`, and the wall's unit direction.
fn along(a: (f32, f32), b: (f32, f32), t: f32) -> ((f32, f32), (f32, f32)) {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dz * dz).sqrt().max(1e-6);
    let u = (dx / len, dz / len);
    ((a.0 + u.0 * t, a.1 + u.1 * t), u)
}

fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, abz) = (b.0 - a.0, b.1 - a.1);
    let len2 = abx * abx + abz * abz;
    let t = if len2 < 1e-9 { 0.0 } else { (((p.0 - a.0) * abx + (p.1 - a.1) * abz) / len2).clamp(0.0, 1.0) };
    ((p.0 - a.0 - abx * t).powi(2) + (p.1 - a.1 - abz * t).powi(2)).sqrt()
}

/// The two sides of an opening: the room (or "outside", past the shell) 0.2 m either side of its
/// middle, on the storey its wall stands on.
fn sides(h: &HomeDesign, storeys: &[Storey], wall: &crate::home_structure::InteriorWall, at: f32, width: f32) -> [String; 2] {
    let (m, u) = along(wall.a, wall.b, at + width * 0.5);
    let n = (-u.1, u.0);
    let k = storey_index(storeys, wall.base);
    let (w, d) = (h.body.width, h.body.depth);
    [-0.2f32, 0.2].map(|s| {
        let (x, z) = (m.0 + n.0 * s, m.1 + n.1 * s);
        if x < 0.0 || z < 0.0 || x > w || z > d {
            "outside".to_string()
        } else {
            // Upstairs, a point in no zone is the open air over the ground floor.
            zone_at(h, storeys, k, x, z).map_or_else(|| if k > 0 { "outside" } else { "nothing" }.to_string(), |zn| room_of(&zn.id).to_string())
        }
    })
}

// ───────────────────────────────────────────────────────────────────────────── the checks

/// Each plot tiled: the ground zones lie inside the plot, overlap nowhere and add up to it; the
/// upper zones overlap nowhere, and each lies over the upper floor's slabs except where the stair
/// climbs through it.
fn tiling_problems(h: &HomeDesign) -> Vec<String> {
    let b = &h.body;
    let storeys = b.storeys();
    let mut out = Vec::new();
    for k in 0..storeys.len() {
        let zs: Vec<&Zone> = b.zones.iter().filter(|z| storey_index(&storeys, z.origin.1) == k).collect();
        for (i, a) in zs.iter().enumerate() {
            let r = rect(a);
            if r.0 < -EPS || r.1 < -EPS || r.2 > b.width + EPS || r.3 > b.depth + EPS {
                out.push(format!("{} leaves the plot: {r:?}", a.id));
            }
            for c in &zs[i + 1..] {
                let o = overlap(r, rect(c));
                if o > EPS {
                    out.push(format!("{} and {} overlap by {o} m2", a.id, c.id));
                }
            }
            if k > 0 {
                let on_slab: f32 = b.floors.iter().filter(|s| (s.top - storeys[k].floor).abs() < EPS).map(|s| overlap(r, s.rect())).sum();
                // Where it has no floor, a stair must climb through it (the stairwell).
                let stair: f32 = b.structures.iter().map(|s| overlap(r, stair_footprint(s))).sum();
                let missing = area(r) - on_slab;
                if missing > stair + 0.01 {
                    out.push(format!("{} upstairs has {missing} m2 with no floor under it, and a stair climbs through {stair} m2 of it", a.id));
                }
            }
        }
        if k == 0 {
            let ground: f32 = zs.iter().map(|z| area(rect(z))).sum();
            if (ground - b.width * b.depth).abs() > 0.01 {
                out.push(format!("the ground zones cover {ground} m2 of the plot's {} (a gap or an overlap)", b.width * b.depth));
            }
        }
    }
    out
}

/// A stair's footprint in plan (none for a piece that is not a stair).
fn stair_footprint(s: &crate::home_structure::PlacedStructure) -> Rect {
    let Some(ty) = crate::structure::structure_type(&s.type_id) else { return (0.0, 0.0, 0.0, 0.0) };
    if !matches!(ty.kind, crate::structure::StructureKind::Stairs) {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let (hw, _, hd) = crate::structure::rotated_half_extents(ty, s.rot_deg.to_radians());
    (s.pos.0 - hw, s.pos.2 - hd, s.pos.0 + hw, s.pos.2 + hd)
}

/// Every door on a wall between two different rooms, and every window between a room and another
/// room or the outside.
fn door_problems(h: &HomeDesign) -> Vec<String> {
    let storeys = h.body.storeys();
    let mut out = Vec::new();
    for (wi, w) in h.body.walls.iter().enumerate() {
        for o in &w.openings {
            let [a, b] = sides(h, &storeys, w, o.at, o.width);
            let what = if o.kind == OpeningKind::Door { "door" } else { "window" };
            if a == b || a == "nothing" || b == "nothing" || (o.kind == OpeningKind::Door && (a == "outside" || b == "outside")) {
                out.push(format!("wall {wi} ({:?} to {:?}): the {what} at {} opens from {a} to {b}", w.a, w.b, o.at));
            }
            if o.at < -EPS || o.at + o.width > ((w.b.0 - w.a.0).powi(2) + (w.b.1 - w.a.1).powi(2)).sqrt() + EPS {
                out.push(format!("wall {wi}: the {what} at {} runs off its wall", o.at));
            }
        }
    }
    out
}

/// The rooms, and which rooms each reaches: through every door, from a part to its primary, from
/// the street through the plot's door, and up each stair from the room at its foot to the room at
/// its top.
fn reach_problems(h: &HomeDesign) -> Vec<String> {
    let storeys = h.body.storeys();
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut link = |a: String, b: String| {
        graph.entry(a.clone()).or_default().insert(b.clone());
        graph.entry(b).or_default().insert(a);
    };
    for w in &h.body.walls {
        for o in w.openings.iter().filter(|o| o.kind == OpeningKind::Door) {
            let [a, b] = sides(h, &storeys, w, o.at, o.width);
            link(a, b);
        }
    }
    // The street: the plot's door point, a step inside.
    let (dx, dz) = h.door;
    let (w, d) = (h.body.width, h.body.depth);
    let step = if dx < EPS { (0.3, dz) } else if dx > w - EPS { (w - 0.3, dz) } else if dz < EPS { (dx, 0.3) } else { (dx, d - 0.3) };
    match zone_at(h, &storeys, 0, step.0, step.1) {
        Some(z) => link("street".into(), room_of(&z.id).to_string()),
        None => return vec![format!("the plot's door {:?} opens into no room", h.door)],
    }
    for s in &h.body.structures {
        let Some(ty) = crate::structure::structure_type(&s.type_id) else { continue };
        if !matches!(ty.kind, crate::structure::StructureKind::Stairs) {
            continue;
        }
        let (sn, cs) = s.rot_deg.to_radians().sin_cos();
        let half = ty.size.2 * 0.5;
        let foot = (s.pos.0 - sn * (half + 0.3), s.pos.2 - cs * (half + 0.3));
        let top = (s.pos.0 + sn * (half + 0.3), s.pos.2 + cs * (half + 0.3));
        let kf = storey_index(&storeys, s.pos.1);
        let kt = storey_index(&storeys, s.pos.1 + ty.size.1);
        match (zone_at(h, &storeys, kf, foot.0, foot.1), zone_at(h, &storeys, kt, top.0, top.1)) {
            (Some(a), Some(b)) if kt > kf => link(room_of(&a.id).to_string(), room_of(&b.id).to_string()),
            _ => return vec![format!("the stair {} does not run from a room to a room on the storey above", s.id)],
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue = VecDeque::from(["street".to_string()]);
    seen.insert("street".into());
    while let Some(n) = queue.pop_front() {
        for m in graph.get(&n).into_iter().flatten() {
            if seen.insert(m.clone()) {
                queue.push_back(m.clone());
            }
        }
    }
    h.body.zones.iter().map(|z| room_of(&z.id)).filter(|r| !seen.contains(*r)).collect::<BTreeSet<_>>().into_iter().map(|r| format!("{r} cannot be reached from the street")).collect()
}

/// Every bedroom meets England's Nationally Described Space Standard (DCLG, March 2015), measured
/// inside its walls as the standard measures: "a double (or twin bedroom) has a floor area of at
/// least 11.5m2", "a single bedroom has a floor area of at least 7.5m2 and is at least 2.15m wide".
/// A bedroom is a double when a double bed ("bed") of its household stands in it.
fn bedroom_problems(h: &HomeDesign, doubles: &BTreeSet<String>) -> Vec<String> {
    let shell = h.body.shell_thickness.filter(|t| *t > 0.0).unwrap_or_else(|| crate::home_structure::wall_material(h.body.shell_material).map_or(0.06, |m| m.default_thickness_m));
    let mut out = Vec::new();
    for z in h.body.zones.iter().filter(|z| z.room_type.as_deref() == Some("bedroom")) {
        let r = rect(z);
        // Half the thickest wall on each side (or half the shell on the plot's edge).
        let side = |p: (f32, f32), q: (f32, f32), on_edge: bool| -> f32 {
            if on_edge {
                return shell * 0.5;
            }
            h.body
                .walls
                .iter()
                .filter(|w| w.y_range().0 <= z.origin.1 + EPS && w.y_range().1 >= z.origin.1 + 2.0)
                .filter(|w| seg_dist(((p.0 + q.0) * 0.5, (p.1 + q.1) * 0.5), w.a, w.b) < EPS)
                .map(|w| w.resolved_thickness() * 0.5)
                .fold(0.0, f32::max)
        };
        let west = side((r.0, r.1), (r.0, r.3), r.0 < EPS);
        let east = side((r.2, r.1), (r.2, r.3), r.2 > h.body.width - EPS);
        let north = side((r.0, r.1), (r.2, r.1), r.1 < EPS);
        let south = side((r.0, r.3), (r.2, r.3), r.3 > h.body.depth - EPS);
        let (iw, id) = (r.2 - r.0 - west - east, r.3 - r.1 - north - south);
        let a = iw * id;
        let need = if doubles.contains(&z.id) { 11.5 } else { 7.5 };
        if a < need {
            out.push(format!("{} is {a:.2} m2 inside its walls, under the standard's {need} m2", z.id));
        }
        if iw.min(id) < 2.15 {
            out.push(format!("{} is {:.2} m wide inside its walls, under the standard's 2.15 m", z.id, iw.min(id)));
        }
    }
    out
}

/// The ceiling over (x, z) for something at height `y`: the underside of the lowest slab over it
/// above `y`, else the roof.
fn ceiling_over(h: &HomeDesign, x: f32, z: f32, y: f32) -> f32 {
    h.body.floors.iter().filter(|s| s.contains(x, z) && s.underside() > y + EPS).map(|s| s.underside()).fold(h.body.height, f32::min)
}

/// Every light in a zone on its own storey and under that zone's ceiling, and every room lit.
fn light_problems(h: &HomeDesign) -> Vec<String> {
    let storeys = h.body.storeys();
    let mut lit: BTreeSet<&str> = BTreeSet::new();
    let mut out = Vec::new();
    for (i, l) in h.body.lights.iter().enumerate() {
        let (x, y, z) = l.pos;
        // The highest storey at or below the light with a zone holding it: a light high in a room
        // that rises through two storeys (the greenhouse, the garden) is that room's.
        let k = (0..=storey_index(&storeys, y)).rev().find(|k| zone_at(h, &storeys, *k, x, z).is_some());
        match k.and_then(|k| zone_at(h, &storeys, k, x, z).map(|zn| (k, zn))) {
            Some((k, zn)) => {
                let ceiling = ceiling_over(h, x, z, storeys[k].floor);
                if y <= storeys[k].floor + EPS || y >= ceiling - EPS {
                    out.push(format!("light {i} ({}) at {y} m is not between {}'s floor {} and its ceiling {ceiling}", l.type_id, zn.id, storeys[k].floor));
                }
                lit.insert(room_of(&zn.id));
            }
            None => out.push(format!("light {i} ({}) at ({x}, {y}, {z}) is in no room", l.type_id)),
        }
    }
    for z in &h.body.zones {
        if !lit.contains(room_of(&z.id)) {
            out.push(format!("{} has no light", room_of(&z.id)));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// No wall through a door: no other wall standing on the door's storey crosses its opening or the
/// metre of passage through it (half a metre each side of its middle).
fn wall_through_door_problems(h: &HomeDesign) -> Vec<String> {
    let storeys = h.body.storeys();
    let mut out = Vec::new();
    for (wi, w) in h.body.walls.iter().enumerate() {
        let k = storey_index(&storeys, w.base);
        for o in w.openings.iter().filter(|o| o.kind == OpeningKind::Door) {
            let (m, u) = along(w.a, w.b, o.at + o.width * 0.5);
            let n = (-u.1, u.0);
            // Points across the opening (its width, 5 cm in from each jamb) and along the passage.
            let mut probes: Vec<(f32, f32)> = (0..=8).map(|i| along(w.a, w.b, o.at + 0.05 + (o.width - 0.1) * i as f32 / 8.0).0).collect();
            probes.extend((1..=5).flat_map(|i| [-1.0f32, 1.0].map(|s| (m.0 + n.0 * s * 0.1 * i as f32, m.1 + n.1 * s * 0.1 * i as f32))));
            for (vi, v) in h.body.walls.iter().enumerate() {
                if vi == wi || !wall_in_storey(&storeys, v, k) {
                    continue;
                }
                let reach = v.resolved_thickness() * 0.5;
                if let Some(p) = probes.iter().find(|p| seg_dist(**p, v.a, v.b) < reach) {
                    out.push(format!("wall {vi} ({:?} to {:?}) runs through the door at {} on wall {wi}, at {p:?}", v.a, v.b, o.at));
                }
            }
        }
    }
    out
}

/// The flood fill (`detect_rooms`, what the game's rooms are) finds every room: each zone with a
/// room type names exactly one room, and no room is left unnamed (an unnamed room means a wall
/// stopped short and two rooms ran together, or a room lost its zone).
fn flood_problems(h: &HomeDesign) -> Vec<String> {
    let rooms = h.body.detect_rooms();
    let mut out: Vec<String> = rooms.iter().filter(|r| !h.body.zones.iter().any(|z| z.id == r.id)).map(|r| format!("{} at {} is named by no zone", r.id, r.center)).collect();
    for z in h.body.zones.iter().filter(|z| z.room_type.is_some()) {
        let n = rooms.iter().filter(|r| r.id == z.id).count();
        if n != 1 {
            out.push(format!("{} names {n} rooms the game finds", z.id));
        }
    }
    out
}

/// Every machine of the household stands in the room it names, on that room's floor, and the
/// rooms holding a double bed.
fn machine_problems(h: &HomeDesign, file: &str) -> (Vec<String>, BTreeSet<String>) {
    let home = crate::machines::MachineHome::load_file(&repo().join("data").join("machines").join(file)).unwrap_or_else(|| panic!("data/machines/{file} parses"));
    let mut out = Vec::new();
    let mut doubles = BTreeSet::new();
    for m in home.all_instances().iter().filter(|m| m.zone == "home") {
        match h.body.zones.iter().find(|z| z.id == m.room) {
            Some(z) => {
                let (x, y, zz) = m.offset;
                if !inside(rect(z), x, zz) {
                    out.push(format!("{file}: {} at ({x}, {zz}) is outside {}", m.id, z.id));
                }
                if (y < z.origin.1 - EPS) || y > z.origin.1 + 2.5 {
                    out.push(format!("{file}: {} at height {y} is not on {}'s floor {}", m.id, z.id, z.origin.1));
                }
                if m.machine == "bed" {
                    doubles.insert(z.id.clone());
                }
            }
            None => out.push(format!("{file}: {} names room {}, which the design does not have", m.id, m.room)),
        }
    }
    (out, doubles)
}

// ───────────────────────────────────────────────────────────────────────────── the tests

/// The shipped files are what the exe embeds (a rebuild picks up an edit), and each is a start's
/// design that stands on its plots (the starts test `each_offered_start_has_plots_and_its_places_aboard`
/// holds the plots).
#[test]
fn the_shipped_files_are_what_the_exe_embeds() {
    for (kind, _) in KINDS {
        let on_disk = std::fs::read_to_string(repo().join("data").join("homes").join("shipped").join(format!("{kind}.ron"))).unwrap().replace("\r\n", "\n");
        let built = humanity_core::embedded_data::shipped_home_design(kind).unwrap_or_else(|| panic!("{kind} is not built in"));
        assert_eq!(on_disk, built.replace("\r\n", "\n"), "data/homes/shipped/{kind}.ron is not what the exe embeds: rebuild");
        // The player's copy in the repo is the shipped design without its shipped-default paragraph.
        let own = std::fs::read_to_string(repo().join("data").join("homes").join(format!("{kind}.ron"))).unwrap().replace("\r\n", "\n");
        assert_eq!(own, humanity_core::embedded_data::own_home_text(built).replace("\r\n", "\n"), "data/homes/{kind}.ron is not the shipped design's own-home text");
        let h = design(kind);
        assert_eq!(h.kind, kind);
        assert_eq!(h.body.storeys().first().map(|s| s.floor), Some(0.0));
    }
}

/// Seen red (2026-11-13) on a copy with the yard 1 m shorter: "the ground zones cover 1860 m2 of
/// the plot's 1870 (a gap or an overlap)"; and with bedroom 2 pushed 1 m east upstairs:
/// "room-bedroom-2 and room-bedroom-3 overlap by 4 m2".
#[test]
fn each_plot_is_tiled_with_no_gap_or_overlap() {
    for (kind, _) in KINDS {
        let p = tiling_problems(&design(kind));
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("household_homestead");
    h.body.zones.iter_mut().find(|z| z.id == "room-yard").unwrap().size.2 -= 1.0;
    assert!(tiling_problems(&h).iter().any(|p| p.contains("a gap or an overlap")), "a gap in the ground floor is caught");
    let mut h = design("household_homestead");
    h.body.zones.iter_mut().find(|z| z.id == "room-bedroom-2").unwrap().origin.0 += 1.0;
    assert!(tiling_problems(&h).iter().any(|p| p.contains("room-bedroom-2 and room-bedroom-3 overlap")), "an overlap upstairs is caught");
}

/// Seen red (2026-11-13) on a copy with the shower room's hall wall slid 1 m into the hall: "the
/// door at ... opens from room-hall to room-hall".
#[test]
fn every_door_is_on_a_wall_between_two_rooms() {
    for (kind, _) in KINDS {
        let p = door_problems(&design(kind));
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("household_homestead");
    let shower = h.body.zones.iter().find(|z| z.id == "room-shower").map(rect).unwrap();
    // The hall-side wall of the shower room carries its door: slide the whole wall 1 m into the hall.
    let w = h
        .body
        .walls
        .iter_mut()
        .find(|w| w.base < EPS && (w.a.1 - shower.3).abs() < EPS && (w.b.1 - shower.3).abs() < EPS && w.a.0 <= shower.0 + EPS && w.b.0 >= shower.2 - EPS)
        .expect("the shower room's hall wall");
    w.a.1 += 1.0;
    w.b.1 += 1.0;
    let p = door_problems(&h);
    assert!(p.iter().any(|p| p.contains("opens from room-hall to room-hall")), "a door inside one room is caught: {p:?}");
}

/// Seen red (2026-11-13) on a copy with the stair taken out: "room-bathroom cannot be reached from
/// the street" (and the three bedrooms upstairs), and with the hen house's door removed:
/// "room-hen-house cannot be reached from the street".
#[test]
fn every_room_is_reached_from_the_street_and_the_upper_floor_by_the_stair() {
    for (kind, _) in KINDS {
        let p = reach_problems(&design(kind));
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("household_homestead");
    h.body.structures.clear();
    assert!(reach_problems(&h).iter().any(|p| p == "room-bathroom cannot be reached from the street"), "the upper floor needs its stair");
    let mut h = design("one_person_homestead");
    let coop = h.body.zones.iter().find(|z| z.id == "room-hen-house").map(rect).unwrap();
    for w in &mut h.body.walls {
        let mid = |o: &crate::home_structure::Opening| along(w.a, w.b, o.at + o.width * 0.5).0;
        let keep: Vec<_> = w.openings.iter().filter(|o| !(o.kind == OpeningKind::Door && seg_dist(mid(o), (coop.0, coop.1), (coop.2, coop.1)).min(seg_dist(mid(o), (coop.0, coop.1), (coop.0, coop.3))).min(seg_dist(mid(o), (coop.2, coop.1), (coop.2, coop.3))).min(seg_dist(mid(o), (coop.0, coop.3), (coop.2, coop.3))) < EPS)).cloned().collect();
        w.openings = keep;
    }
    assert!(reach_problems(&h).iter().any(|p| p == "room-hen-house cannot be reached from the street"), "a room with no door is caught");
}

/// Seen red (2026-11-13) on a copy with bedroom 3 cut to 1.9 m wide: "room-bedroom-3 is 1.85 m wide
/// inside its walls, under the standard's 2.15 m" (and its area under 7.5 m2); and with the one
/// person's double bed moved into a 2.5 x 3.5 m bedroom, under 11.5 m2.
#[test]
fn every_bedroom_meets_the_space_standard() {
    for (kind, file) in KINDS {
        let h = design(kind);
        let (_, doubles) = machine_problems(&h, file);
        assert!(!doubles.is_empty(), "{kind}: a double bed stands in a bedroom");
        let p = bedroom_problems(&h, &doubles);
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("household_homestead");
    h.body.zones.iter_mut().find(|z| z.id == "room-bedroom-3").unwrap().size.0 = 1.9;
    let p = bedroom_problems(&h, &BTreeSet::new());
    assert!(p.iter().any(|p| p.starts_with("room-bedroom-3 is") && p.contains("wide inside its walls")), "a narrow bedroom is caught: {p:?}");
    let mut h = design("one_person_homestead");
    let bed = h.body.zones.iter_mut().find(|z| z.id == "room-bedroom").unwrap();
    bed.size.0 = 2.5;
    let p = bedroom_problems(&h, &["room-bedroom".to_string()].into_iter().collect());
    assert!(p.iter().any(|p| p.contains("under the standard's 11.5 m2")), "a small double is caught: {p:?}");
}

/// Seen red (2026-11-13) on a copy with the kitchen's light dropped under the floor and the
/// mushroom room's light taken away: "light ... is not between room-kitchen's floor" and
/// "room-mushroom has no light".
#[test]
fn every_room_is_lit_and_every_light_is_inside_a_room() {
    for (kind, _) in KINDS {
        let p = light_problems(&design(kind));
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("household_homestead");
    let kitchen = h.body.zones.iter().find(|z| z.id == "room-kitchen").map(rect).unwrap();
    let mushroom = h.body.zones.iter().find(|z| z.id == "room-mushroom").map(rect).unwrap();
    h.body.lights.iter_mut().filter(|l| inside(kitchen, l.pos.0, l.pos.2)).for_each(|l| l.pos.1 = -0.5);
    h.body.lights.retain(|l| !inside(mushroom, l.pos.0, l.pos.2));
    let p = light_problems(&h);
    assert!(p.iter().any(|p| p.contains("not between room-kitchen's floor")), "a light under the floor is caught: {p:?}");
    assert!(p.iter().any(|p| p == "room-mushroom has no light"), "an unlit room is caught: {p:?}");
}

/// Seen red (2026-11-13) on a copy with a wall drawn across the front door's passage: "wall ...
/// runs through the door".
#[test]
fn no_wall_runs_through_a_door() {
    for (kind, _) in KINDS {
        let p = wall_through_door_problems(&design(kind));
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("one_person_homestead");
    let (wi, at, width) = h
        .body
        .walls
        .iter()
        .enumerate()
        .find_map(|(i, w)| w.openings.iter().find(|o| o.kind == OpeningKind::Door).map(|o| (i, o.at, o.width)))
        .unwrap();
    let w = &h.body.walls[wi];
    let (m, u) = along(w.a, w.b, at + width * 0.5);
    let mut cross = w.clone();
    cross.openings.clear();
    cross.a = (m.0 - u.1 * 0.8, m.1 + u.0 * 0.8);
    cross.b = (m.0 + u.1 * 0.8, m.1 - u.0 * 0.8);
    h.body.walls.push(cross);
    assert!(wall_through_door_problems(&h).iter().any(|p| p.contains(&format!("runs through the door at {at} on wall {wi}"))), "a wall across a doorway is caught");
}

/// Seen red (2026-11-13) on a copy with the wall between the kitchen and the living room taken out:
/// the two flood into one room, which the kitchen's zone names, so "room-living names 0 rooms the
/// game finds".
#[test]
fn the_game_finds_every_room_its_zones_name() {
    for (kind, _) in KINDS {
        let p = flood_problems(&design(kind));
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("household_homestead");
    let kitchen = h.body.zones.iter().find(|z| z.id == "room-kitchen").map(rect).unwrap();
    // The wall between the kitchen and the living room (the kitchen's street-side edge).
    h.body.walls.retain(|w| !((w.a.0 - kitchen.0).abs() < EPS && (w.b.0 - kitchen.0).abs() < EPS && w.a.1.min(w.b.1) <= kitchen.1 + EPS && w.a.1.max(w.b.1) >= kitchen.3 - EPS && w.base < EPS));
    let p = flood_problems(&h);
    assert!(p.iter().any(|p| p.contains("names 0 rooms")), "two rooms run together is caught: {p:?}");
}

/// Seen red (2026-11-13) with a single bed moved out of bedroom 2: "bed_single_2 at ... is outside
/// room-bedroom-2".
#[test]
fn every_machine_stands_in_the_room_it_names() {
    for (kind, file) in KINDS {
        let (p, _) = machine_problems(&design(kind), file);
        assert!(p.is_empty(), "{kind}:\n{}", p.join("\n"));
    }
    let mut h = design("household_homestead");
    h.body.zones.iter_mut().find(|z| z.id == "room-bedroom-2").unwrap().origin.2 += 10.0;
    let (p, _) = machine_problems(&h, "household_homestead.ron");
    assert!(p.iter().any(|p| p.contains("bed_single_2") && p.contains("is outside room-bedroom-2")), "a machine outside its room is caught: {p:?}");
}

/// The room program's stair: 16 risers of at most 196 mm on goings of at least 254 mm, the 1.0 m
/// width the drafts drew, and the stairwell left open for it upstairs with 2.032 m of headroom over
/// every step (docs/reference/findings/2026-10-20-stairs-guards-and-headroom.md).
#[test]
fn the_household_stair_meets_the_stairs_finding() {
    let h = design("household_homestead");
    let s = h.body.structures.iter().find(|s| s.type_id == "stairs_house").expect("the household homestead has its stair");
    let ty = crate::structure::structure_type(&s.type_id).unwrap();
    let riser = ty.size.1 / ty.steps as f32;
    let going = ty.size.2 / ty.steps as f32;
    assert!(riser <= 0.196 + EPS && going >= 0.254 - EPS, "riser {riser}, going {going}");
    assert!((ty.size.0 - 1.0).abs() < EPS, "1.0 m wide, not {}", ty.size.0);
    let (sn, cs) = s.rot_deg.to_radians().sin_cos();
    for i in 0..ty.steps {
        let lz = -ty.size.2 * 0.5 + (i as f32 + 0.5) * going;
        let (x, z) = (s.pos.0 + lz * sn, s.pos.2 + lz * cs);
        let tread_top = (i + 1) as f32 * riser;
        let ceiling = ceiling_over(&h, x, z, tread_top);
        assert!(ceiling - tread_top >= 2.032 - EPS || tread_top >= ty.size.1 - EPS, "step {i}: {} m of headroom", ceiling - tread_top);
    }
}

/// A DRAFT, before it is data (the row house of docs/design/home-room-program.md section 9): run every
/// check above on a home-design file anywhere on disk, so a draft is held to the same rules as the
/// shipped homes before the operator sees it. Not part of the suite (it reads a path from the
/// environment):
/// `HUMANITY_HOME_DRAFT=<file.ron> HUMANITY_HOME_DRAFT_DOUBLES=<zone,zone> node scripts/lib-tests.js
/// --crate humanity-ship a_draft_passes_every_check --ignored`. The doubles are the bedrooms drawn for
/// two (a draft has no household machine file to say where its double beds stand).
#[test]
#[ignore]
fn a_draft_passes_every_check() {
    let Ok(path) = std::env::var("HUMANITY_HOME_DRAFT") else { return };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let h: HomeDesign = ron::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let doubles: BTreeSet<String> = std::env::var("HUMANITY_HOME_DRAFT_DOUBLES").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
    let mut p = tiling_problems(&h);
    p.extend(door_problems(&h));
    p.extend(reach_problems(&h));
    p.extend(bedroom_problems(&h, &doubles));
    p.extend(light_problems(&h));
    p.extend(wall_through_door_problems(&h));
    p.extend(flood_problems(&h));
    assert!(p.is_empty(), "{path}:\n{}", p.join("\n"));
    println!("{path}: every check passes ({} zones, {} walls, {} lights)", h.body.zones.len(), h.body.walls.len(), h.body.lights.len());
}

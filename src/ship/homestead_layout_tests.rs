//! THE HOMESTEAD'S UPPER FLOOR, CHECKED (ship homes increment 7, lane C1).
//!
//! docs/design/homestead-upper-floor.md placed every slab, wall, door, room and machine of the
//! homestead's second storey and checked them with a throwaway script. These are those checks as
//! tests, run on the design the exe ships (data/homes/shipped/homestead.ron through
//! `HomeDesign::built_in`) and on the two machine files, so an edit that breaks the layout fails
//! the build instead of a walk-through:
//!
//! - two storeys, no two rooms overlapping on either, the ground rooms tiling the acre, and every
//!   room the flood fill finds named by its zone;
//! - the floor slabs and the four openings (the stairwell, the elevator shaft, the ladder hatch
//!   and the greenhouse) tiling the acre, solid under the upstairs rooms and glass over every
//!   other room, which keeps its daylight;
//! - every door and window on its wall, and every door between two rooms;
//! - every room reached from the front door, every upstairs room from the stair top;
//! - the stair to the dated finding's numbers, with its headroom;
//! - every open edge upstairs guarded, and no way onto the roofs;
//! - every machine and every light in its room, on its storey, under its ceiling.
//!
//! The guards are 1.0 m walls with base 3.0 until lane G's guard piece lands; the open-edge check
//! knows only walls, so swapping a guard for the piece means teaching it the piece.
//!
//! Every test here was seen red on the single-storey homestead (2026-10-20); each one's comment
//! says what it said.

use crate::ship::home_structure::{storey_index, FloorSlab, InteriorWall, OpeningKind, PlacedStructure, Storey, Zone, ROOM_DIVIDER_MIN_M};
use crate::ship::ship_structure::HomeDesign;
use crate::ship::structure::{self, StructureKind};
use std::collections::{BTreeSet, HashMap, VecDeque};

const EPS: f32 = 1e-3;

/// The shortest guard either code allows on the edge of an internal floor: the IRC's 36 in,
/// 914 mm (docs/reference/findings/2026-10-20-stairs-guards-and-headroom.md, finding 3).
const GUARD_MIN_M: f32 = 0.914;

/// A rectangle in plan: (min x, min z, max x, max z), body-local metres.
type Rect = (f32, f32, f32, f32);

/// The shipped homestead, read from data/homes/shipped/homestead.ron in the checkout rather than
/// the copy compiled into the test program, so a check can be rerun against an edited file
/// without a rebuild. They are the same file (`include_str!` in src/embedded_data.rs), which the
/// first test below holds.
fn home() -> HomeDesign {
    let text = std::fs::read_to_string(shipped_path()).expect("data/homes/shipped/homestead.ron");
    ron::from_str(&text).expect("the shipped homestead parses")
}

fn shipped_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("homes").join("shipped").join("homestead.ron")
}

fn zone_rect(z: &Zone) -> Rect {
    (z.origin.0, z.origin.2, z.origin.0 + z.size.0, z.origin.2 + z.size.2)
}

fn area(r: Rect) -> f32 {
    (r.2 - r.0).max(0.0) * (r.3 - r.1).max(0.0)
}

fn overlap(a: Rect, b: Rect) -> f32 {
    area((a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3)))
}

fn strictly_inside(r: Rect, x: f32, z: f32) -> bool {
    x > r.0 + EPS && x < r.2 - EPS && z > r.1 + EPS && z < r.3 - EPS
}

/// Distance in plan from (px, pz) to the segment a..b, and how far along it the nearest point is.
fn seg_dist(px: f32, pz: f32, a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    let (abx, abz) = (b.0 - a.0, b.1 - a.1);
    let len2 = abx * abx + abz * abz;
    let t = if len2 < 1e-9 { 0.0 } else { (((px - a.0) * abx + (pz - a.1) * abz) / len2).clamp(0.0, 1.0) };
    let (cx, cz) = (a.0 + abx * t, a.1 + abz * t);
    (((px - cx).powi(2) + (pz - cz).powi(2)).sqrt(), t * len2.sqrt())
}

fn wall_len(w: &InteriorWall) -> f32 {
    ((w.b.0 - w.a.0).powi(2) + (w.b.1 - w.a.1).powi(2)).sqrt()
}

fn piece<'a>(h: &'a HomeDesign, id: &str) -> &'a PlacedStructure {
    h.body.structures.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("the homestead has no {id}"))
}

/// A placed piece's local (x, z) as body-local (x, z): `structure::structure_mesh`'s yaw.
fn local_to_world(s: &PlacedStructure, lx: f32, lz: f32) -> (f32, f32) {
    let (sn, cs) = s.rot_deg.to_radians().sin_cos();
    (s.pos.0 + lx * cs + lz * sn, s.pos.2 - lx * sn + lz * cs)
}

fn footprint(s: &PlacedStructure) -> Rect {
    let ty = structure::structure_type(&s.type_id).expect("a known structure type");
    let (hw, _, hd) = structure::rotated_half_extents(ty, s.rot_deg.to_radians());
    (s.pos.0 - hw, s.pos.2 - hd, s.pos.0 + hw, s.pos.2 + hd)
}

/// The four places the slab layer at 3.0 m leaves open (homestead-upper-floor.md section 1). The
/// shaft and the greenhouse are read from the elevator and the greenhouse zone, so the shaft is
/// exactly the car's footprint, with no gap between the car and the floor for a body to fall
/// through.
fn openings(h: &HomeDesign) -> Vec<(&'static str, Rect)> {
    let greenhouse = h.body.zones.iter().find(|z| z.id == "room-greenhouse").expect("the greenhouse zone");
    vec![
        ("the stairwell", (36.5, 38.0, 40.5, 39.3)),
        ("the elevator shaft", footprint(piece(h, "elevator-1"))),
        ("the ladder hatch", (32.5, 26.9, 34.0, 28.1)),
        ("the greenhouse", zone_rect(greenhouse)),
    ]
}

/// The zones whose floor is on storey `k`.
fn zones_on<'a>(h: &'a HomeDesign, storeys: &[Storey], k: usize) -> Vec<&'a Zone> {
    h.body.zones.iter().filter(|z| storey_index(storeys, z.origin.1) == k).collect()
}

/// The ceiling over (x, z) for something standing at height `y`: the underside of the lowest slab
/// over that point whose underside is above `y`, else the body's roof.
fn ceiling_over(h: &HomeDesign, x: f32, z: f32, y: f32) -> f32 {
    h.body
        .floors
        .iter()
        .filter(|s| s.contains(x, z) && s.underside() > y + EPS)
        .map(|s| s.underside())
        .fold(h.body.height, f32::min)
}

/// THE HOMESTEAD HAS TWO STOREYS AND NO TWO ROOMS OVERLAP. The storeys are 0 to 3.0 and 3.0 to
/// 6.0; on each, no two room zones overlap; the ground rooms tile the 55 x 89 m acre; each zone
/// stands on its storey's floor and rises to its room's ceiling, the slab over it or the roof where
/// none is (so the greenhouse's zone, open through the slab layer, rises to the 6 m roof with its
/// room, and every other zone spans its storey); upstairs are the plan's eight rooms; and the flood
/// fill finds exactly one room per zone and no room without one. Until 2026-10-21 the greenhouse's
/// zone stayed a storey tall, as a stand-in for its air box in the life-support tests, which read
/// the zones then; they read the rooms the game publishes now (homestead-upper-floor.md section
/// 12). Seen red that day with the greenhouse's zone at 3.0: "room-greenhouse's room rises to 6,
/// its zone to 3".
///
/// Seen red 2026-10-20 on the single-storey homestead: "the homestead's storeys are [Storey {
/// floor: 0.0, top: 3.0 }]".
#[test]
fn the_homestead_has_two_storeys_and_no_two_rooms_overlap() {
    let h = home();
    let on_disk = std::fs::read_to_string(shipped_path()).unwrap().replace("\r\n", "\n");
    assert_eq!(on_disk, crate::embedded_data::HOME_HOMESTEAD_RON.replace("\r\n", "\n"), "data/homes/shipped/homestead.ron is not what the exe embeds: rebuild");
    let b = &h.body;
    let storeys = b.storeys();
    assert_eq!(
        storeys,
        vec![Storey { floor: 0.0, top: 3.0 }, Storey { floor: 3.0, top: 6.0 }],
        "the homestead's storeys are {storeys:?}"
    );
    for k in 0..storeys.len() {
        let zs = zones_on(&h, &storeys, k);
        for (i, a) in zs.iter().enumerate() {
            for c in &zs[i + 1..] {
                let o = overlap(zone_rect(a), zone_rect(c));
                assert!(o < EPS, "{} and {} overlap by {o} m2 on storey {k}", a.id, c.id);
            }
            let r = zone_rect(a);
            assert!(r.0 >= -EPS && r.1 >= -EPS && r.2 <= b.width + EPS && r.3 <= b.depth + EPS, "{} leaves the acre: {r:?}", a.id);
            assert!((a.origin.1 - storeys[k].floor).abs() < EPS, "{} stands at {}, not on storey {k}'s floor", a.id, a.origin.1);
            assert!(a.origin.1 + a.size.1 >= storeys[k].top - EPS, "{} reaches {}, under its storey's top {}", a.id, a.origin.1 + a.size.1, storeys[k].top);
        }
    }
    let ground: f32 = zones_on(&h, &storeys, 0).iter().map(|z| area(zone_rect(z))).sum();
    assert!((ground - b.width * b.depth).abs() < 0.01, "the ground rooms cover {ground} m2 of the acre's {}", b.width * b.depth);
    let upstairs: BTreeSet<&str> = zones_on(&h, &storeys, 1).iter().map(|z| z.id.as_str()).collect();
    let want: BTreeSet<&str> = [
        "room-bedroom",
        "room-dressing",
        "room-bathroom",
        "room-wetroom",
        "room-study",
        "room-upper-hall",
        "room-back-corridor",
        "room-lift-lobby",
    ]
    .into_iter()
    .collect();
    assert_eq!(upstairs, want, "the upstairs rooms");

    let rooms = b.detect_rooms();
    let unnamed: Vec<String> = rooms.iter().filter(|r| !b.zones.iter().any(|z| z.id == r.id)).map(|r| format!("{} at {}", r.id, r.center)).collect();
    assert!(unnamed.is_empty(), "rooms no zone names (a wall stopped short, or a floor has no zone): {unnamed:?}");
    for z in &b.zones {
        let named: Vec<&crate::ship::fibonacci::RoomInfo> = rooms.iter().filter(|r| r.id == z.id).collect();
        assert_eq!(named.len(), 1, "zone {} names {} rooms", z.id, named.len());
        let floor = named[0].center.y - named[0].dimensions.y * 0.5;
        assert!((floor - z.origin.1).abs() < EPS, "{}'s room stands at {floor}, its zone at {}", z.id, z.origin.1);
        let top = named[0].center.y + named[0].dimensions.y * 0.5;
        assert!((top - (z.origin.1 + z.size.1)).abs() < EPS, "{}'s room rises to {top}, its zone to {}", z.id, z.origin.1 + z.size.1);
    }
}

/// THE SLABS AND THE FOUR OPENINGS TILE THE ACRE, AND ONLY THE UPSTAIRS FLOOR IS SOLID. Every slab
/// is at 3.0 m, 0.25 m thick, inside the acre; no slab overlaps another or an opening; slabs and
/// openings add up to the acre exactly; a slab under an upstairs room is solid and every other
/// slab glass (the orchestrator's lid rule of 2026-10-20: the rooms not under the upstairs keep
/// today's daylight, the power terrace, the court and the fields their sun); and every upstairs
/// room stands on slab or on the opening it is built around (the stairwell in the upper hall, the
/// hatch in the lobby).
///
/// Seen red 2026-10-20 on the single-storey homestead: "slabs and openings cover 1000.24 m2 of the
/// acre's 4895".
#[test]
fn the_slabs_and_the_four_openings_tile_the_acre_and_only_the_upstairs_floor_is_solid() {
    let h = home();
    let b = &h.body;
    let storeys = b.storeys();
    let label = |s: &FloorSlab| s.label.clone().unwrap_or_else(|| format!("{:?}", s.rect()));
    let mut pieces: Vec<(String, Rect)> = Vec::new();
    for s in &b.floors {
        assert!((s.top - 3.0).abs() < EPS && (s.thickness - 0.25).abs() < EPS, "{}: top {}, {} thick", label(s), s.top, s.thickness);
        let r = s.rect();
        assert!(r.0 >= -EPS && r.1 >= -EPS && r.2 <= b.width + EPS && r.3 <= b.depth + EPS, "{} leaves the acre", label(s));
        pieces.push((label(s), r));
    }
    let holes = openings(&h);
    pieces.extend(holes.iter().map(|(n, r)| (n.to_string(), *r)));
    for (i, (an, ar)) in pieces.iter().enumerate() {
        for (bn, br) in &pieces[i + 1..] {
            let o = overlap(*ar, *br);
            assert!(o < EPS, "{an} and {bn} overlap by {o} m2");
        }
    }
    let covered: f32 = pieces.iter().map(|(_, r)| area(*r)).sum();
    assert!((covered - b.width * b.depth).abs() < 0.01, "slabs and openings cover {covered} m2 of the acre's {}", b.width * b.depth);

    let upstairs: Vec<&Zone> = zones_on(&h, &storeys, 1);
    for s in &b.floors {
        let r = s.rect();
        let under: f32 = upstairs.iter().map(|u| overlap(r, zone_rect(u))).sum();
        if s.passes_light() {
            assert!(under < EPS, "the glass slab {} is under an upstairs room ({under} m2): that room needs a solid floor", label(s));
        } else {
            assert!((under - area(r)).abs() < 0.01, "the solid slab {} reaches past the upstairs rooms ({under} of {} m2): the room under it loses its daylight", label(s), area(r));
        }
    }
    for u in &upstairs {
        let r = zone_rect(u);
        let on: f32 = b.floors.iter().map(|s| overlap(r, s.rect())).sum::<f32>() + holes.iter().map(|(_, hr)| overlap(r, *hr)).sum::<f32>();
        assert!((on - area(r)).abs() < 0.01, "{} stands on {on} m2 of slab and openings of its {} m2", u.id, area(r));
    }
}

/// What lies on one side of a door: a room zone of the door's storey, or an elevator's car.
#[derive(Debug, Clone, PartialEq)]
enum Side {
    Room(String),
    Elevator(String),
}

fn side_at(h: &HomeDesign, storeys: &[Storey], k: usize, x: f32, z: f32) -> Option<Side> {
    if let Some(z) = zones_on(h, storeys, k).into_iter().find(|zn| strictly_inside(zone_rect(zn), x, z)) {
        return Some(Side::Room(z.id.clone()));
    }
    h.body
        .structures
        .iter()
        .filter(|s| structure::structure_type(&s.type_id).map_or(false, |t| t.kind == StructureKind::Elevator))
        .find(|s| strictly_inside(footprint(s), x, z))
        .map(|s| Side::Elevator(s.id.clone()))
}

/// Every door as (storey, wall index, the two sides), read off the walls.
fn doors(h: &HomeDesign) -> Vec<(usize, String, Option<Side>, Option<Side>)> {
    let storeys = h.body.storeys();
    let mut out = Vec::new();
    for (i, w) in h.body.walls.iter().enumerate() {
        let k = storey_index(&storeys, w.base);
        let len = wall_len(w);
        let (dx, dz) = ((w.b.0 - w.a.0) / len, (w.b.1 - w.a.1) / len);
        for o in w.openings.iter().filter(|o| o.kind == OpeningKind::Door) {
            let t = o.at + o.width * 0.5;
            let (mx, mz) = (w.a.0 + dx * t, w.a.1 + dz * t);
            let what = format!("the door at {} on wall {i} ({:?} to {:?}, base {})", o.at, w.a, w.b, w.base);
            out.push((k, what, side_at(h, &storeys, k, mx - dz * 0.3, mz + dx * 0.3), side_at(h, &storeys, k, mx + dz * 0.3, mz - dx * 0.3)));
        }
    }
    out
}

/// EVERY DOOR AND WINDOW IS ON ITS WALL, AND EVERY DOOR JOINS TWO ROOMS. Each opening lies within
/// its wall's length and height, under the ceiling over it (a ground door or window under a slab
/// keeps its head below the slab's 2.75 m underside), no two openings on one wall overlap, and
/// each door opens between two different rooms of its storey, or a room and the elevator car (the
/// lift lobby's landing door).
///
/// Passes on the single-storey homestead; seen red 2026-10-20 on the built layout with the second
/// bedroom's shower-room door put 9.0 m along its 8 m wall: "the Door at 9 on wall 16 ((50.0,
/// 30.0) to (50.0, 38.0), base 0) runs past the wall's 8 m".
#[test]
fn every_door_and_window_is_on_its_wall_and_every_door_joins_two_rooms() {
    let h = home();
    let storeys = h.body.storeys();
    for (i, w) in h.body.walls.iter().enumerate() {
        let len = wall_len(w);
        let mut spans: Vec<(f32, f32)> = Vec::new();
        for o in &w.openings {
            let what = format!("the {:?} at {} on wall {i} ({:?} to {:?}, base {})", o.kind, o.at, w.a, w.b, w.base);
            assert!(o.at >= -EPS && o.at + o.width <= len + EPS, "{what} runs past the wall's {len} m");
            assert!(o.sill >= 0.0 && o.sill + o.height <= w.height + EPS, "{what} rises past the wall's {} m", w.height);
            let t = o.at + o.width * 0.5;
            let (mx, mz) = (w.a.0 + (w.b.0 - w.a.0) * t / len, w.a.1 + (w.b.1 - w.a.1) * t / len);
            let head = w.base + o.sill + o.height;
            let ceiling = ceiling_over(&h, mx, mz, w.base);
            assert!(head <= ceiling + EPS, "{what}: its head at {head} m is above the ceiling at {ceiling} m");
            spans.push((o.at, o.at + o.width));
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in spans.windows(2) {
            assert!(pair[1].0 >= pair[0].1 - EPS, "two openings on wall {i} ({:?} to {:?}) overlap: {pair:?}", w.a, w.b);
        }
    }
    for (k, what, a, b) in doors(&h) {
        let (Some(a), Some(b)) = (a.clone(), b.clone()) else {
            panic!("{what} on storey {k} opens onto nothing: {a:?} and {b:?}");
        };
        assert_ne!(a, b, "{what} opens from {a:?} into itself");
        assert!(matches!(a, Side::Room(_)) || matches!(b, Side::Room(_)), "{what} joins no room");
    }
}

/// The rooms as a graph: each door, and the stair, the elevator and the ladder between storeys.
/// (from, to, by what, the storey a door is on, or None for a way between storeys).
fn links(h: &HomeDesign) -> Vec<(String, String, String, Option<usize>)> {
    let storeys = h.body.storeys();
    let mut out = Vec::new();
    let mut lifts: HashMap<String, Vec<String>> = HashMap::new();
    for (k, what, a, b) in doors(h) {
        match (a, b) {
            (Some(Side::Room(a)), Some(Side::Room(b))) => out.push((a, b, what, Some(k))),
            (Some(Side::Room(r)), Some(Side::Elevator(e))) | (Some(Side::Elevator(e)), Some(Side::Room(r))) => lifts.entry(e).or_default().push(r),
            _ => {}
        }
    }
    let room = |k: usize, (x, z): (f32, f32)| match side_at(h, &storeys, k, x, z) {
        Some(Side::Room(r)) => r,
        other => panic!("no room on storey {k} at ({x}, {z}): {other:?}"),
    };
    // The stair: the room at its foot, on the ground, and the room at its top, upstairs.
    let stairs = piece(h, "stairs-1");
    let (_, _, run) = structure::structure_type(&stairs.type_id).expect("the stairs type").size;
    out.push((room(0, local_to_world(stairs, 0.0, -run * 0.5 - 0.3)), room(1, local_to_world(stairs, 0.0, run * 0.5 + 0.3)), "stairs-1".into(), None));
    // The elevator: the room it stands in, and every room its landing doors open into.
    for s in h.body.structures.iter().filter(|s| structure::structure_type(&s.type_id).map_or(false, |t| t.kind == StructureKind::Elevator)) {
        let base = room(0, (s.pos.0, s.pos.2));
        for r in lifts.get(&s.id).cloned().unwrap_or_default() {
            out.push((base.clone(), r, s.id.clone(), None));
        }
    }
    // The ladder: the room it stands in, and the room you step into over its top.
    let ladder = piece(h, "ladder-1");
    out.push((room(0, (ladder.pos.0, ladder.pos.2)), room(1, local_to_world(ladder, 0.0, 0.4)), "ladder-1".into(), None));
    out
}

fn reached(start: &str, links: &[(String, String, String, Option<usize>)], only_storey: Option<usize>) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = [start.to_string()].into_iter().collect();
    let mut queue: VecDeque<String> = [start.to_string()].into_iter().collect();
    while let Some(at) = queue.pop_front() {
        for (a, b, _, k) in links {
            if only_storey.is_some() && *k != only_storey {
                continue;
            }
            let next = if *a == at { b } else if *b == at { a } else { continue };
            if seen.insert(next.clone()) {
                queue.push_back(next.clone());
            }
        }
    }
    seen
}

/// EVERY ROOM IS REACHED FROM THE FRONT DOOR, EVERY UPSTAIRS ROOM FROM THE STAIR TOP, AND EVERY
/// GROUND ROOM WITHOUT CLIMBING. Through doors only (a window is not a way in): from the room
/// inside the plot's door, by the doors, the stair, the elevator and the ladder, every room in the
/// home; from the room at the top of the stair, by upstairs doors alone, every upstairs room; and
/// from the hall, by ground doors alone, every ground room, so a resident who cannot climb reaches
/// the whole ground floor, the second bedroom and the shower room among it.
///
/// Seen red 2026-10-20 on the single-storey homestead: "no room on storey 1 at (34, 6.1): None"
/// (the old stairs-1, in the vehicle bay, led to no floor).
#[test]
fn every_room_is_reached_from_the_front_door_and_every_upstairs_room_from_the_stair_top() {
    let h = home();
    let storeys = h.body.storeys();
    let links = links(&h);
    let all: BTreeSet<String> = h.body.zones.iter().map(|z| z.id.clone()).collect();
    let front = match side_at(&h, &storeys, 0, h.door.0 - 0.3, h.door.1) {
        Some(Side::Room(r)) => r,
        other => panic!("nothing inside the front door: {other:?}"),
    };
    let from_front = reached(&front, &links, None);
    let missing: Vec<&String> = all.difference(&from_front).collect();
    assert!(missing.is_empty(), "not reached from the front door ({front}): {missing:?}");

    let stairs = piece(&h, "stairs-1");
    let (_, _, run) = structure::structure_type(&stairs.type_id).expect("the stairs type").size;
    let (tx, tz) = local_to_world(stairs, 0.0, run * 0.5 + 0.3);
    let Some(Side::Room(top)) = side_at(&h, &storeys, 1, tx, tz) else { panic!("no room at the stair top ({tx}, {tz})") };
    let upstairs: BTreeSet<String> = zones_on(&h, &storeys, 1).iter().map(|z| z.id.clone()).collect();
    let from_top = reached(&top, &links, Some(1));
    let missing: Vec<&String> = upstairs.difference(&from_top).collect();
    assert!(missing.is_empty(), "upstairs rooms not reached from the stair top ({top}): {missing:?}");

    let ground: BTreeSet<String> = zones_on(&h, &storeys, 0).iter().map(|z| z.id.clone()).collect();
    let from_hall = reached("room-hall", &links, Some(0));
    let missing: Vec<&String> = ground.difference(&from_hall).collect();
    assert!(missing.is_empty(), "ground rooms not reached from the hall without climbing: {missing:?}");
}

/// THE STAIR MEETS THE FINDING, AND ITS HEADROOM IS CLEAR. The `stairs` type against the stricter
/// of the IRC 2021 and England's Approved Document K
/// (docs/reference/findings/2026-10-20-stairs-guards-and-headroom.md): a riser of at most 196 mm,
/// a tread of 279 to 300 mm (no nosing is drawn, which the IRC allows from 279 mm), a pitch of at
/// most 42 degrees, 2R + G between 550 and 700 mm, at least 914 mm wide, one flight rising at most
/// 3835 mm. Placed as stairs-1 it rises one storey, its top tread is level with an upper floor
/// slab that starts where the flight ends, a landing as deep as the flight is wide is clear in
/// front of its foot, and everywhere along it there is at least 2032 mm (6 ft 8 in) from the line
/// of the nosings to whatever is over it.
///
/// Seen red 2026-10-20 on the 14-step type: "the riser is 214.3 mm, over the IRC's 196".
#[test]
fn the_stair_meets_the_finding_and_its_headroom_is_clear() {
    let h = home();
    let ty = structure::structure_type("stairs").expect("the stairs type");
    let (w, rise, run) = ty.size;
    let n = ty.steps.max(1) as f32;
    let (r, g) = (rise / n, run / n);
    assert!(r <= 0.196 + 1e-4, "the riser is {:.1} mm, over the IRC's 196", r * 1000.0);
    assert!(g >= 0.279 - 1e-4 && g <= 0.300 + 1e-4, "the tread is {:.2} mm, outside 279 to 300", g * 1000.0);
    let pitch = r.atan2(g).to_degrees();
    assert!(pitch <= 42.0, "the pitch is {pitch:.1} degrees, over AD K's 42");
    let two_r_g = (2.0 * r + g) * 1000.0;
    assert!((550.0..=700.0).contains(&two_r_g), "2R + G is {two_r_g:.0} mm, outside 550 to 700");
    assert!(w >= 0.914, "the stair is {w} m wide, under the IRC's 914 mm");
    assert!(rise <= 3.835, "one flight rises {rise} m, over the IRC's 3835 mm");

    let s = piece(&h, "stairs-1");
    assert!((s.pos.1 + rise - 3.0).abs() < EPS, "stairs-1 rises to {}, not the upper floor at 3.0", s.pos.1 + rise);
    // Its top: just past the top tread there is an upper floor at the top tread's height, and the
    // slab's edge is where the flight ends (no gap to step over).
    let (bx, bz) = local_to_world(s, 0.0, run * 0.5 + 0.05);
    let landing = h.body.floors.iter().find(|f| f.contains(bx, bz) && (f.top - (s.pos.1 + rise)).abs() < EPS);
    let landing = landing.unwrap_or_else(|| panic!("nothing to step onto past the stair top at ({bx}, {bz})"));
    let (ex, ez) = local_to_world(s, 0.0, run * 0.5);
    let lr = landing.rect();
    let on_edge = (ex - lr.0).abs() < EPS || (ex - lr.2).abs() < EPS || (ez - lr.1).abs() < EPS || (ez - lr.3).abs() < EPS;
    assert!(on_edge, "the stair ends at ({ex}, {ez}), the landing slab {:?} does not start there", landing.label);
    // A landing at its foot, as deep as the flight is wide, with no wall in it.
    let storeys = h.body.storeys();
    for i in 0..=10 {
        for j in 0..=10 {
            let lx = -w * 0.5 + w * i as f32 / 10.0;
            let lz = -run * 0.5 - w * j as f32 / 10.0;
            let (px, pz) = local_to_world(s, lx * 0.98, lz - 0.01);
            for wall in h.body.walls.iter().filter(|wl| storey_index(&storeys, wl.base) == 0) {
                let (d, _) = seg_dist(px, pz, wall.a, wall.b);
                assert!(d > wall.resolved_thickness() * 0.5, "a wall ({:?} to {:?}) stands in the stair's foot landing at ({px}, {pz})", wall.a, wall.b);
            }
        }
    }
    // Headroom from the nosing line, y = r + d * r / g at a distance d from the foot.
    let mut least = (f32::MAX, 0.0, 0.0, 0.0);
    for i in 0..=450 {
        let d = run * i as f32 / 450.0;
        for lx in [-w * 0.5 + 0.02, 0.0, w * 0.5 - 0.02] {
            let (x, z) = local_to_world(s, lx, -run * 0.5 + d);
            let nosing = s.pos.1 + r + d * r / g;
            let over = h.body.floors.iter().filter(|f| f.contains(x, z) && f.underside() > nosing - EPS).map(|f| f.underside()).fold(h.body.height, f32::min);
            if over - nosing < least.0 {
                least = (over - nosing, d, x, z);
            }
        }
    }
    println!("stairs-1: {:.1} mm risers on {:.2} mm treads, {pitch:.1} degrees, least headroom {:.3} m at {:.2} m from the foot", r * 1000.0, g * 1000.0, least.0, least.1);
    assert!(least.0 >= 2.032, "the headroom is {:.3} m at {:.2} m up the flight ({}, {}), under the IRC's 2.032", least.0, least.1, least.2, least.3);
}

/// EVERY OPEN EDGE UPSTAIRS IS GUARDED, AND NO WAY LEADS ONTO THE ROOFS, BUT FOR THE TWO EDGES THE
/// MOVEMENT LANE CLOSES. Walking the floor of every upstairs room in 0.1 m steps, a step off the
/// floor (into the stairwell, the hatch or the shaft) must cross a guard at least 914 mm tall (the
/// IRC; these are 1.0 m) or be the stair's top; and a step onto a slab that is no upstairs room
/// (the lids over the ground rooms) must cross a wall, never a door. The two edges left open are
/// decision 9 of the plan, both lane M's: the elevator's landing door (opens only with the car at
/// the landing) and the ladder's 0.6 m access gap (a self-closing gate).
///
/// Prints the guards (`--nocapture`) so they can be swapped for lane G's guard piece.
///
/// The shell counts as closed only where it stands on the upper storey (ship homes increment 7
/// review, finding 1): the plot door's cut through the east wall opens it from the floor to
/// 2.2 m, and over that the shell stands.
///
/// Seen red 2026-10-20 on the single-storey homestead: "the homestead has no upper floor:
/// [Storey { floor: 0.0, top: 3.0 }]"; and on the built layout with guard G1 (the gallery's edge
/// along the stairwell) left out: "open edges upstairs:
/// [\"(36.55..40.35, 39.30..39.30)\"]". Seen red first (2026-10-21), reading the shell by
/// storey, with the plot door's cut open at every height: "open edges upstairs:
/// [\"(55.00..55.00, 39.15..40.35)\"]" (upstairs the upper hall opened through the shell over
/// the front door into the corridor, 3 m down).
#[test]
fn every_open_edge_upstairs_is_guarded_and_no_way_leads_onto_the_roofs() {
    let h = home();
    let b = &h.body;
    let storeys = b.storeys();
    assert!(storeys.len() >= 2, "the homestead has no upper floor: {storeys:?}");
    let up = zones_on(&h, &storeys, 1);
    let floor_at = |x: f32, z: f32| b.floors.iter().any(|s| (s.top - storeys[1].floor).abs() < EPS && s.contains(x, z));
    let in_upstairs_room = |x: f32, z: f32| up.iter().any(|zn| strictly_inside(zone_rect(zn), x, z));
    let walls: Vec<&InteriorWall> = b.walls.iter().filter(|w| storey_index(&storeys, w.base) == 1 && w.height >= GUARD_MIN_M).collect();
    let stops = |mx: f32, mz: f32| {
        walls.iter().any(|w| {
            let (d, t) = seg_dist(mx, mz, w.a, w.b);
            d <= w.resolved_thickness() * 0.5 + 0.06 && !w.openings.iter().any(|o| o.kind == OpeningKind::Door && t > o.at - EPS && t < o.at + o.width + EPS)
        })
    };
    let stairs = piece(&h, "stairs-1");
    let ty = structure::structure_type(&stairs.type_id).expect("the stairs type");
    let (sw, _, run) = ty.size;
    let top_a = local_to_world(stairs, -sw * 0.5, run * 0.5);
    let top_b = local_to_world(stairs, sw * 0.5, run * 0.5);
    let stair_top = |mx: f32, mz: f32| seg_dist(mx, mz, top_a, top_b).0 <= 0.06;
    // The two edges decision 9 leaves to lane M, as segments.
    let ladder = piece(&h, "ladder-1");
    let lw = structure::structure_type(&ladder.type_id).expect("the ladder type").size.0;
    let exceptions: [(&str, (f32, f32), (f32, f32)); 2] = [
        ("the elevator's landing door", (30.1, 26.9), (31.9, 26.9)),
        ("the ladder's access gap", (ladder.pos.0 - lw * 0.5, 28.1), (ladder.pos.0 + lw * 0.5, 28.1)),
    ];
    let excused = |mx: f32, mz: f32| exceptions.iter().any(|(_, a, bb)| seg_dist(mx, mz, *a, *bb).0 <= 0.06);
    // The shell, read by storey (ship homes increment 7 review, finding 1): the walls the walk
    // sweeps (`wall_collision::wall_segments_with_shell_cuts`) with the plot door's cut through
    // the east wall, which opens the shell only from its foot to its head, 2.2 m. A step across
    // the shell's line upstairs is closed only where a segment there stops a body standing on
    // the upper floor; this check used to count every shell edge as closed, so a cut open at
    // every height could never be seen.
    let shell = shell_walls(&h);
    let upper_eye = storeys[1].floor + crate::ship::wall_collision::EYE_HEIGHT_M;
    let shell_stands = |mx: f32, mz: f32| {
        shell.iter().any(|s| s.stops_body(upper_eye) && seg_dist(mx, mz, s.a, s.b).0 <= s.half_thickness + 0.06)
    };

    let (mut drops, mut roofs) = (Vec::new(), Vec::new());
    let cell = 0.1f32;
    for zn in &up {
        let r = zone_rect(zn);
        let (nx, nz) = (((r.2 - r.0) / cell).round() as i32, ((r.3 - r.1) / cell).round() as i32);
        for i in 0..nx {
            for j in 0..nz {
                let (px, pz) = (r.0 + (i as f32 + 0.5) * cell, r.1 + (j as f32 + 0.5) * cell);
                if !floor_at(px, pz) {
                    continue;
                }
                for (dx, dz) in [(cell, 0.0), (-cell, 0.0), (0.0, cell), (0.0, -cell)] {
                    let (qx, qz) = (px + dx, pz + dz);
                    let (mx, mz) = ((px + qx) * 0.5, (pz + qz) * 0.5);
                    if qx <= 0.0 || qz <= 0.0 || qx >= b.width || qz >= b.depth {
                        // Out through the shell: a drop to the corridor or the yard outside,
                        // unless the shell stands there on this storey.
                        if !shell_stands(mx, mz) {
                            drops.push((mx, mz));
                        }
                        continue;
                    }
                    if !floor_at(qx, qz) {
                        if !(stops(mx, mz) || stair_top(mx, mz) || excused(mx, mz)) {
                            drops.push((mx, mz));
                        }
                    } else if !in_upstairs_room(qx, qz) && !stops(mx, mz) {
                        roofs.push((mx, mz));
                    }
                }
            }
        }
    }
    let describe = |pts: &[(f32, f32)]| -> Vec<String> {
        // Group the edge points into runs, each shown as its extent.
        let mut groups: Vec<Vec<(f32, f32)>> = Vec::new();
        for p in pts {
            match groups.iter_mut().find(|g| g.iter().any(|q| (q.0 - p.0).abs() <= 0.11 && (q.1 - p.1).abs() <= 0.11)) {
                Some(g) => g.push(*p),
                None => groups.push(vec![*p]),
            }
        }
        groups
            .iter()
            .map(|g| {
                let (x0, x1) = g.iter().fold((f32::MAX, f32::MIN), |(a, c), p| (a.min(p.0), c.max(p.0)));
                let (z0, z1) = g.iter().fold((f32::MAX, f32::MIN), |(a, c), p| (a.min(p.1), c.max(p.1)));
                format!("({x0:.2}..{x1:.2}, {z0:.2}..{z1:.2})")
            })
            .collect()
    };
    for w in b.walls.iter().filter(|w| storey_index(&storeys, w.base) == 1 && w.height < ROOM_DIVIDER_MIN_M) {
        println!("guard: {:?} to {:?}, base {}, {} m tall, material {}", w.a, w.b, w.base, w.height, w.material);
        assert!(w.height >= 1.0 - EPS, "the guard {:?} to {:?} is {} m tall, under the house's 1.0 m", w.a, w.b, w.height);
    }
    assert!(drops.is_empty(), "open edges upstairs: {:?}", describe(&drops));
    assert!(roofs.is_empty(), "ways onto the roofs from upstairs: {:?}", describe(&roofs));
}

/// The homestead's walls as the walk sweeps them, home-local, with the shell cut the plot's door
/// corridor makes through its east wall: the shipped ship with the homestead on p1, whose home
/// stands at the ship's origin.
fn shell_walls(h: &HomeDesign) -> Vec<crate::ship::wall_collision::WallSegment> {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles");
    let cuts = ship.shell_cuts_for_zone(ship.home_zone_index());
    assert!(!cuts.is_empty(), "the plot's door corridor cuts no hole in the homestead's shell");
    crate::ship::wall_collision::wall_segments_with_shell_cuts(&h.body, &cuts)
}

/// A machine's height, metres (`MachineDef::size`: a sphere's is its radius).
fn machine_height(def: &crate::machines::MachineDef) -> f32 {
    if def.shape == "sphere" {
        def.size.0 * 2.0
    } else {
        def.size.1
    }
}

/// EVERY MACHINE STANDS IN ITS ROOM, ON ITS STOREY, UNDER ITS CEILING. In both machine files
/// (data/machines/home.ron, the family; home_solo.ron, one person) every machine of the home
/// names a room zone of the shipped homestead, stands inside its footprint, on its storey's floor
/// or above it and below the next, and its top is under the ceiling over it (the 2.75 m underside
/// of a slab over a ground room). The quiet wing's machines went up 3.0 m with their rooms, and
/// the plant room's and the court's air handlers came down 0.2 m: at 2.7 their tops (2.942 m)
/// were in the slab.
///
/// Passes on the single-storey homestead; seen red 2026-10-20 with the slabs in and the air
/// handlers still at 2.7 m: "home.ron:
/// air_handler_c1 (air_handler) in room-court reaches 2.942 m, the ceiling over it is 2.75".
#[test]
fn every_machine_stands_in_its_room_on_its_storey_under_its_ceiling() {
    let h = home();
    let storeys = h.body.storeys();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
    for file in ["home.ron", "home_solo.ron"] {
        let machines = crate::machines::MachineHome::load(&root.join(file)).unwrap_or_else(|| panic!("{file} parses"));
        let mut ids = BTreeSet::new();
        for inst in machines.all_instances().iter().filter(|i| i.zone == "home") {
            assert!(ids.insert(inst.id.clone()), "{file}: two machines are called {}", inst.id);
            let what = format!("{file}: {} ({}) in {}", inst.id, inst.machine, inst.room);
            let z = h.body.zones.iter().find(|z| z.id == inst.room).unwrap_or_else(|| panic!("{what}: no such room"));
            let (x, y, zz) = inst.offset;
            let r = zone_rect(z);
            assert!(x >= r.0 - EPS && x <= r.2 + EPS && zz >= r.1 - EPS && zz <= r.3 + EPS, "{what} at ({x}, {zz}) is outside it, {r:?}");
            let k = storey_index(&storeys, z.origin.1);
            assert!(y >= storeys[k].floor - EPS && y < storeys[k].top, "{what} stands at {y} m, off storey {k} ({:?})", storeys[k]);
            let def = machines.catalog.get(&inst.machine).unwrap_or_else(|| panic!("{what}: no such machine"));
            let top = y + machine_height(def);
            let ceiling = ceiling_over(&h, x, zz, y);
            assert!(top <= ceiling + EPS, "{what} reaches {top:.3} m, the ceiling over it is {ceiling}");
        }
    }
}

/// EVERY ROOM HAS A LIGHT, AND EVERY LIGHT IS IN A ROOM ON ITS STOREY UNDER ITS CEILING. A lamp
/// lights its own storey (lane L keeps it there), so each room, upstairs and down, needs one of
/// its own, and each lamp (each point of a strip's path) is inside a room of the storey its
/// height is on and below the ceiling over it.
///
/// Passes on the single-storey homestead; seen red 2026-10-20 with the layout in and its new
/// lights left out: "rooms with no light:
/// [\"room-dining\", \"room-bedroom-2\", \"room-shower\", \"room-boot\", \"room-upper-hall\",
/// \"room-back-corridor\", \"room-lift-lobby\"]".
#[test]
fn every_room_has_a_light_and_every_light_is_in_a_room_under_its_ceiling() {
    let h = home();
    let storeys = h.body.storeys();
    let mut lit: BTreeSet<String> = BTreeSet::new();
    for l in &h.body.lights {
        let mut points = vec![l.pos];
        points.extend(l.path.iter().copied());
        for (x, y, z) in points {
            let k = storey_index(&storeys, y);
            let what = format!("the {} at ({x}, {y}, {z})", l.type_id);
            let Some(zn) = zones_on(&h, &storeys, k).into_iter().find(|zn| {
                let r = zone_rect(zn);
                x >= r.0 - EPS && x <= r.2 + EPS && z >= r.1 - EPS && z <= r.3 + EPS
            }) else {
                panic!("{what} is in no room of storey {k}");
            };
            let ceiling = ceiling_over(&h, x, z, y);
            assert!(y < ceiling, "{what} hangs at or above the ceiling over it, {ceiling}");
            lit.insert(zn.id.clone());
        }
    }
    let dark: Vec<&str> = h.body.zones.iter().filter(|z| !lit.contains(&z.id)).map(|z| z.id.as_str()).collect();
    assert!(dark.is_empty(), "rooms with no light: {dark:?}");
}

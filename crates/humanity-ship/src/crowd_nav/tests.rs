//! Crowd navigation rung 1's gates (docs/design/crowd-navigation.md, section 10). Each was run RED
//! first against stubs (a mesh that is all one walkable rectangle, a path that is the straight line,
//! avoidance that takes the preferred velocity, a cache that always builds); the failures are in each
//! test's comment.

use super::crowd::{step, Agent, CrowdParams};
use super::mesh::{FloorRect, Footprint, NavCache, NavInput, NavMesh};
use super::path::find_path;
use super::BODY_RADIUS_M;
use crate::home_structure::{HomeStructure, InteriorWall, Opening, OpeningKind};
use crate::wall_collision::{wall_segments, WallSegment};
use glam::Vec2;

/// A `w` x `d` box, 3 m high, with `walls` inside it.
fn body(w: f32, d: f32, walls: Vec<InteriorWall>) -> HomeStructure {
    HomeStructure {
        width: w,
        depth: d,
        height: 3.0,
        shell_material: 1,
        roof_material: 4,
        walls,
        shell_thickness: None,
        open_shell: Vec::new(),
        floors: Vec::new(),
        lights: Vec::new(),
        spawn: None,
        structures: Vec::new(),
        road_nodes: Vec::new(),
        road_edges: Vec::new(),
        zones: Vec::new(),
        rail_nodes: Vec::new(),
        rail_edges: Vec::new(),
    }
}

/// Two rooms, 10 x 10 m, split along z = 5 by a 0.2 m wall with one 1 m door at x 4.5 to 5.5.
fn two_rooms() -> HomeStructure {
    body(
        10.0,
        10.0,
        vec![InteriorWall {
            a: (0.0, 5.0),
            b: (10.0, 5.0),
            base: 0.0,
            height: 3.0,
            material: 1,
            thickness: Some(0.2),
            layers: Vec::new(),
            openings: vec![Opening {
                kind: OpeningKind::Door,
                at: 4.5,
                width: 1.0,
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
    )
}

fn input<'a>(w: f32, d: f32, walls: &'a [WallSegment], furniture: &'a [Footprint], floor: &'a [FloorRect]) -> NavInput<'a> {
    NavInput { min: (0.0, 0.0), max: (w, d), feet_y: 0.0, walls, footprints: furniture, floor, radius: BODY_RADIUS_M }
}

/// The distance between segments p0-p1 and q0-q1 (0 when they cross).
fn seg_seg(p0: Vec2, p1: Vec2, q0: Vec2, q1: Vec2) -> f32 {
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    let (r, s) = (p1 - p0, q1 - q0);
    let den = cross(r, s);
    if den.abs() > 1e-9 {
        let t = cross(q0 - p0, s) / den;
        let u = cross(q0 - p0, r) / den;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            return 0.0;
        }
    }
    let pt_seg = |p: Vec2, a: Vec2, b: Vec2| {
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        p.distance(a + ab * t)
    };
    pt_seg(p0, q0, q1).min(pt_seg(p1, q0, q1)).min(pt_seg(q0, p0, p1)).min(pt_seg(q1, p0, p1))
}

/// How near the path's legs come to the face of any wall that stops a standing body, metres
/// (negative: through it). The wall over a doorway (a lintel) is walked under, so it is left out.
fn clearance(path: &[Vec2], walls: &[WallSegment]) -> f32 {
    let mut least = f32::MAX;
    let eye = crate::wall_collision::EYE_HEIGHT_M;
    for leg in path.windows(2) {
        for w in walls.iter().filter(|w| w.stops_body(eye)) {
            let d = seg_seg(leg[0], leg[1], Vec2::new(w.a.0, w.a.1), Vec2::new(w.b.0, w.b.1)) - w.half_thickness;
            least = least.min(d);
        }
    }
    least
}

/// THE PATH GOES THROUGH THE DOOR: from the south room's west side to the north room's west side,
/// whose straight line meets the wall 3 m west of the door, the path crosses the wall's line in the
/// doorway and keeps a body's radius off every wall.
///
/// Red first (2026-11-13, the stubs): `the path crosses z = 5 in the doorway (x 4.5 to 5.5): [Vec2(1.0,
/// 2.0), Vec2(2.0, 8.0)]`: the straight line, through the wall at x = 1.5. (A second red, on the real
/// mesh: `the least was -0.100 m`, which was this check counting the lintel over the door, walked
/// under, as a wall; it reads only what stops a standing body now.)
#[test]
fn a_path_from_one_room_to_another_goes_through_the_door() {
    let walls = wall_segments(&two_rooms());
    let nav = NavMesh::build(&input(10.0, 10.0, &walls, &[], &[]));
    let (from, to) = (Vec2::new(1.0, 2.0), Vec2::new(2.0, 8.0));
    let path = find_path(&nav, from, to).expect("the two rooms are joined by the door");
    assert_eq!((path[0], *path.last().unwrap()), (from, to), "the path starts and ends where asked");
    let crossing = path.windows(2).find_map(|l| {
        let (a, b) = (l[0], l[1]);
        ((a.y - 5.0) * (b.y - 5.0) <= 0.0 && a.y != b.y).then(|| a.x + (b.x - a.x) * (5.0 - a.y) / (b.y - a.y))
    });
    assert!(crossing.is_some_and(|x| x > 4.5 && x < 5.5), "the path crosses z = 5 in the doorway (x 4.5 to 5.5): {path:?}");
    let c = clearance(&path, &walls);
    // A body's centre keeps its radius off a wall, less a cell's half diagonal where the funnel
    // turns at a walkable cell's corner.
    assert!(c >= BODY_RADIUS_M - 0.075, "every leg keeps a body's radius off the walls, the least was {c:.3} m: {path:?}");
    // And it is the short way: under the straight line plus a detour to the door and back.
    let len: f32 = path.windows(2).map(|l| l[0].distance(l[1])).sum();
    assert!(len < from.distance(Vec2::new(5.0, 5.0)) + Vec2::new(5.0, 5.0).distance(to) + 0.5, "the path is string-pulled, not a stair of cells: {len:.2} m");
}

/// A furnished room: 12 x 12 m, four tables (one turned), a sofa along the wall and a counter.
fn furnished() -> (Vec<WallSegment>, Vec<Footprint>) {
    let walls = wall_segments(&body(12.0, 12.0, Vec::new()));
    let box_at = |x: f32, z: f32, w: f32, d: f32, yaw: f32| Footprint { centre: (x, z), half: (w * 0.5, d * 0.5), yaw, y0: 0.0, y1: 0.75 };
    let furniture = vec![
        box_at(3.0, 3.0, 1.6, 0.9, 0.0),
        box_at(8.5, 3.5, 1.6, 0.9, 0.6),
        box_at(3.5, 8.0, 0.9, 1.6, 0.0),
        box_at(8.0, 8.5, 1.2, 1.2, 0.785),
        box_at(6.0, 11.3, 2.2, 0.8, 0.0),
        box_at(11.4, 6.0, 0.6, 3.0, 0.0),
    ];
    (walls, furniture)
}

/// NO BODY ENTERS FURNITURE: sixteen people wander a furnished room for a simulated minute; no
/// body's centre is ever inside a footprint, every one keeps nearly its radius off each, and they
/// really walk (destinations reached), so the check cannot pass on a crowd standing still.
///
/// Red first (2026-11-13, the stubs): `agent 3 at Vec2(7.889012, 8.463877) is inside a footprint at
/// 0.03 s` (it started inside the turned table and nothing moved it out, then walked straight lines).
#[test]
fn no_agent_position_is_inside_a_furniture_footprint_over_a_simulated_minute() {
    let (walls, furniture) = furnished();
    let nav = NavMesh::build(&input(12.0, 12.0, &walls, &furniture, &[]));
    let params = CrowdParams::default();
    let mut agents: Vec<Agent> = (0..16)
        .map(|i| {
            let a = i as f32 * 2.399_963; // the golden angle, spreading the starts
            let r = 1.0 + 4.5 * ((i as f32 + 0.5) / 16.0).sqrt();
            let start = Vec2::new(6.0, 6.0) + Vec2::new(a.cos(), a.sin()) * r;
            Agent::new(start, Vec2::new(6.0, 6.0), 6.0, 1.34, i as u64 + 1)
        })
        .collect();
    let dt = 1.0 / 30.0;
    let mut least = f32::MAX;
    for f in 0..(60.0 / dt) as usize {
        step(&nav, &mut agents, dt, &params);
        for (n, a) in agents.iter().enumerate() {
            for fp in &furniture {
                let d = fp.distance(a.pos.x, a.pos.y);
                assert!(d >= 0.0, "agent {n} at {:?} is inside a footprint at {:.2} s", a.pos, (f + 1) as f32 * dt);
                least = least.min(d);
            }
        }
    }
    assert!(least >= BODY_RADIUS_M - 0.075, "every body keeps nearly its radius off the furniture: the least was {least:.3} m");
    let arrived: u32 = agents.iter().map(|a| a.arrivals).sum();
    assert!(arrived >= 32, "the crowd walked: {arrived} destinations reached in a minute");
}

/// TWO PEOPLE WALKING HEAD-ON PASS: in an open 12 x 6 m room, one walks east and one west along the
/// same line; they pass without their bodies ever overlapping, and both arrive.
///
/// Red first (2026-11-13, the stubs): `the bodies overlapped: their centres came 0.014 m apart (two
/// radii are 0.50)`.
#[test]
fn two_agents_walking_head_on_pass_without_overlapping() {
    let walls = wall_segments(&body(12.0, 6.0, Vec::new()));
    let nav = NavMesh::build(&input(12.0, 6.0, &walls, &[], &[]));
    let params = CrowdParams { wander: false, ..Default::default() };
    let (west, east) = (Vec2::new(2.0, 3.0), Vec2::new(10.0, 3.0));
    let mut agents = vec![Agent::new(west, west, 0.0, 1.34, 1), Agent::new(east, east, 0.0, 1.34, 2)];
    assert!(agents[0].set_goal(&nav, east, &params) && agents[1].set_goal(&nav, west, &params));
    let dt = 1.0 / 60.0;
    let mut closest = f32::MAX;
    for _ in 0..(20.0 / dt) as usize {
        step(&nav, &mut agents, dt, &params);
        closest = closest.min(agents[0].pos.distance(agents[1].pos));
        if agents.iter().all(|a| a.arrivals == 1) {
            break;
        }
    }
    assert!(closest >= 2.0 * BODY_RADIUS_M - 0.01, "the bodies overlapped: their centres came {closest:.3} m apart (two radii are {:.2})", 2.0 * BODY_RADIUS_M);
    assert!(agents.iter().all(|a| a.arrivals == 1), "both arrived: {:?}", agents.iter().map(|a| (a.pos, a.arrivals, a.given_up)).collect::<Vec<_>>());
}

/// THE MESH IS REBUILT WHEN FURNITURE MOVES AND NOT OTHERWISE: the same inputs keep the mesh, a
/// table moved 0.2 m builds it again, a table moved far outside the region does not, and with the
/// reuse switch off (`humanity_core::reuse::enabled()` false, passed in) it is built every frame.
///
/// Red first (2026-11-13, the stubs): `the same inputs keep the mesh (frame 0)`: the stub built it again.
#[test]
fn the_navmesh_rebuilds_when_furniture_moves_and_not_otherwise() {
    let (walls, mut furniture) = furnished();
    // One more piece far off, outside the region and its edge.
    furniture.push(Footprint { centre: (40.0, 40.0), half: (0.5, 0.5), yaw: 0.0, y0: 0.0, y1: 1.0 });
    let mut cache = NavCache::default();
    assert!(cache.ensure(&input(12.0, 12.0, &walls, &furniture, &[]), true), "the first frame builds the mesh");
    let before = cache.mesh().unwrap().walkable_cells();
    for frame in 0..5 {
        assert!(!cache.ensure(&input(12.0, 12.0, &walls, &furniture, &[]), true), "the same inputs keep the mesh (frame {frame})");
    }
    furniture[0].centre.0 += 0.2;
    assert!(cache.ensure(&input(12.0, 12.0, &walls, &furniture, &[]), true), "a table moved 0.2 m builds the mesh again");
    assert!(!cache.ensure(&input(12.0, 12.0, &walls, &furniture, &[]), true), "and then keeps it");
    furniture[6].centre.1 += 3.0;
    assert!(!cache.ensure(&input(12.0, 12.0, &walls, &furniture, &[]), true), "a table moved far outside the region does not");
    assert_eq!(cache.builds, 2, "two builds in nine frames");
    for frame in 0..3 {
        assert!(cache.ensure(&input(12.0, 12.0, &walls, &furniture, &[]), false), "with reuse off every frame builds (frame {frame})");
    }
    assert_eq!(cache.builds, 5);
    // What it built is the furniture's: a moved table frees the cells it left.
    let after = cache.mesh().unwrap().walkable_cells();
    assert!(before > 0 && after > 0, "the room has walkable floor: {before} then {after} cells");
}

/// THE CROWD'S REGION HOLDS WHILE IT WALKS: the mesh is built over the region round the walkers'
/// roams (`crowd::region`), so it must not move as they walk, or the mesh is built again every frame.
/// Walkers that start outside their roam (the `crowd` verb's `nav:<roam_m>` smaller than its radius)
/// walk into it, and the region stays where it was.
///
/// Red first (2026-11-13), the region as engine/crowd_walk.rs first had it, taking in where each
/// walker stood: `assertion left == right failed: the region moved as the walkers walked, at 0.03 s`
/// (the rig's run.log said it too: 141 builds in 10 s).
#[test]
fn the_crowds_region_holds_while_its_walkers_walk() {
    let (walls, furniture) = furnished();
    let nav = NavMesh::build(&input(12.0, 12.0, &walls, &furniture, &[]));
    let params = CrowdParams::default();
    let home = Vec2::new(6.0, 6.0);
    let mut agents: Vec<Agent> = (0..12)
        .map(|i| {
            let a = i as f32 * 2.399_963;
            // Starting 5.5 m out, their roam 3 m.
            Agent::new(home + Vec2::new(a.cos(), a.sin()) * 5.5, home, 3.0, 1.34, i as u64 + 1)
        })
        .collect();
    let first = super::crowd::region(&agents).expect("a crowd has a region");
    let dt = 1.0 / 30.0;
    for f in 0..(20.0 / dt) as usize {
        step(&nav, &mut agents, dt, &params);
        let now = super::crowd::region(&agents).unwrap();
        assert_eq!(now, first, "the region moved as the walkers walked, at {:.2} s", (f + 1) as f32 * dt);
    }
}

/// THE SHIP AS IT SHIPS: on the shipped ship's ground deck (the walls the walk collides with, the
/// floors of its zones and corridors), a walk from the middle of the Commons reaches the home on its
/// default plot, through the corridor, keeping clear of every wall.
///
/// Red first (2026-11-13, the stubs): `the walk keeps clear of the ship's walls, the least was -0.050 m:
/// [Vec2(82.0, 47.5), Vec2(27.5, 44.5)]` (the straight line through the Commons' west wall).
#[test]
fn a_path_from_the_commons_reaches_the_home_through_the_corridor() {
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let ship = crate::ship_structure::ShipStructure::load_and_assemble_shipped(&data, None).expect("the shipped ship assembles");
    let walls = crate::wall_collision::ship_wall_segments(&ship);
    let floor = super::floor::floor_rects(&ship, 0.0);
    assert!(floor.len() >= 3, "the ground deck has floors: {floor:?}");
    let commons = ship.zones.iter().find(|z| z.id == "commons").expect("the ship has a Commons");
    let home = ship.zones.iter().find(|z| z.id == "home").expect("the home is on its plot");
    let mid = |z: &crate::ship_structure::ShipZone| Vec2::new(z.origin.0 + z.body.width * 0.5, z.origin.2 + z.body.depth * 0.5);
    let (a, b) = (mid(commons), mid(home));
    let min = (a.x.min(b.x) - 30.0, a.y.min(b.y) - 30.0);
    let max = (a.x.max(b.x) + 30.0, a.y.max(b.y) + 30.0);
    let nav = NavMesh::build(&NavInput { min, max, feet_y: 0.0, walls: &walls, footprints: &[], floor: &floor, radius: BODY_RADIUS_M });
    let from = nav.nearest_walkable(a, 3.0).expect("the middle of the Commons is walkable");
    let to = nav.nearest_walkable(b, 3.0).expect("the middle of the home is walkable");
    let path = find_path(&nav, from, to).expect("the Commons and the home are joined");
    let c = clearance(&path, &walls);
    assert!(c >= BODY_RADIUS_M - 0.075, "the walk keeps clear of the ship's walls, the least was {c:.3} m: {path:?}");
}

/// The walking rules as shipped (data/crowd/walking.ron, read by engine/crowd_walk.rs each time a
/// crowd is made) parse whole: a misspelt number would otherwise fall back to the defaults with
/// only a log line to say so.
#[test]
fn the_walking_rules_in_data_parse() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/crowd/walking.ron");
    let text = std::fs::read_to_string(&path).expect("data/crowd/walking.ron is there");
    let p: CrowdParams = ron::from_str(&text).expect("data/crowd/walking.ron parses");
    assert!(p.tau > 0.0 && p.neighbour_m > 0.0 && p.max_neighbours > 0 && p.dwell_s.0 <= p.dwell_s.1, "the rules make sense: {p:?}");
}

//! OPEN SPANS IN A SHELL, CHECKED (2026-11-13, the row houses' open green,
//! docs/design/home-room-program.md section 10; `home_structure::OpenSpan`).
//!
//! An open span is open to the EYE and not to the WALK: the mesh draws only what marks the line
//! (a mowing strip, a kerb, a low wall), the sight blockers stand only to that height, and the
//! walking line stays whole, except where two zones touch and are open over the same stretch
//! (`ShipStructure::open_span_cuts`). A neighbour's plot is never a zone, so it is never joined.
//! Each test is seen RED once, recorded in its comment.

use crate::home_structure::{perimeter_pieces, HomeStructure, OpenSpan};
use crate::ship_structure::ShipStructure;

/// A plain 10 x 10 x 3 m box with the given open spans.
fn boxed(open: Vec<OpenSpan>) -> HomeStructure {
    let mut h: HomeStructure = ron::from_str("(width: 10.0, depth: 10.0, height: 3.0)").expect("a bare body parses");
    h.open_shell = open;
    h
}

fn span(a: (f32, f32), b: (f32, f32), height: f32) -> OpenSpan {
    OpenSpan { a, b, height, material: 2, thickness: None }
}

/// Each edge reads in the winding a corridor's cut uses, either way round, and a span that is not
/// on one edge is none; the pieces of an edge cover it with no gap. Seen red (2026-11-13) with
/// `perimeter_pieces` reading no span: the edge came back as one closed piece.
#[test]
fn an_open_span_reads_on_its_edge_and_cuts_the_edge_into_pieces() {
    assert_eq!(span((2.0, 0.0), (8.0, 0.0), 0.0).on_edge(10.0, 10.0), Some((0, 2.0, 8.0)));
    assert_eq!(span((10.0, 7.0), (10.0, 1.0), 0.0).on_edge(10.0, 10.0), Some((1, 1.0, 7.0)));
    // Edge 2 runs from (w, d) to (0, d): x 6..9 is 1..4 along it.
    assert_eq!(span((6.0, 10.0), (9.0, 10.0), 0.0).on_edge(10.0, 10.0), Some((2, 1.0, 4.0)));
    // Edge 3 runs from (0, d) to (0, 0): z 0..2 is 8..10 along it.
    assert_eq!(span((0.0, 0.0), (0.0, 2.0), 0.0).on_edge(10.0, 10.0), Some((3, 8.0, 10.0)));
    assert_eq!(span((0.0, 0.0), (5.0, 5.0), 0.0).on_edge(10.0, 10.0), None, "a span across the box is on no edge");
    let open = vec![span((2.0, 0.0), (4.0, 0.0), 0.0), span((6.0, 0.0), (10.0, 0.0), 0.0)];
    let pieces: Vec<(f32, f32, bool)> = perimeter_pieces(&open, 10.0, 10.0, 0, 10.0).into_iter().map(|(s, e, o)| (s, e, o.is_some())).collect();
    assert_eq!(pieces, [(0.0, 2.0, false), (2.0, 4.0, true), (4.0, 6.0, false), (6.0, 10.0, true)]);
    assert_eq!(perimeter_pieces(&open, 10.0, 10.0, 1, 10.0).len(), 1, "an edge with no span is one piece");
}

/// The highest the walls' mesh reaches over (x, z): the top of every triangle whose footprint
/// covers a point within `r` of it.
fn wall_top_near(h: &HomeStructure, x: f32, z: f32, r: f32) -> Option<f32> {
    let m = h.generate_meshes();
    let mut top: Option<f32> = None;
    for (v, idx, _) in &m.material_walls {
        for t in idx.chunks(3) {
            let p: Vec<[f32; 3]> = t.iter().map(|&i| v[i as usize].position).collect();
            let (x0, x1) = (p.iter().map(|q| q[0]).fold(f32::MAX, f32::min), p.iter().map(|q| q[0]).fold(f32::MIN, f32::max));
            let (z0, z1) = (p.iter().map(|q| q[2]).fold(f32::MAX, f32::min), p.iter().map(|q| q[2]).fold(f32::MIN, f32::max));
            if x1 >= x - r && x0 <= x + r && z1 >= z - r && z0 <= z + r {
                let y = p.iter().map(|q| q[1]).fold(f32::MIN, f32::max);
                top = Some(top.map_or(y, |a| a.max(y)));
            }
        }
    }
    top
}

/// Seen red (2026-11-13) with the mesh's perimeter drawn whole, before it read the open spans:
/// "the shell stands 3 m over the open span" (the wall top near (5, 0) was 3.0).
#[test]
fn an_open_span_draws_what_marks_its_line_and_not_the_shell() {
    let closed = boxed(Vec::new());
    assert_eq!(wall_top_near(&closed, 5.0, 0.0, 0.3), Some(3.0), "the shell stands where nothing is open");
    let strip = boxed(vec![span((2.0, 0.0), (8.0, 0.0), 0.03)]);
    let top = wall_top_near(&strip, 5.0, 0.0, 0.3);
    assert!(top.is_some_and(|t| t <= 0.03 + 1e-4), "the shell stands {top:?} m over the open span: only the mowing strip should");
    assert_eq!(wall_top_near(&strip, 1.0, 0.0, 0.3), Some(3.0), "the shell still stands on the rest of the edge");
    let wall = boxed(vec![span((0.0, 2.0), (0.0, 8.0), 0.9)]);
    assert!(wall_top_near(&wall, 0.0, 5.0, 0.3).is_some_and(|t| (t - 0.9).abs() < 1e-4), "a front garden wall stands 0.9 m");
    let nothing = boxed(vec![span((10.0, 2.0), (10.0, 8.0), 0.0)]);
    assert_eq!(wall_top_near(&nothing, 10.0, 5.0, 0.3), None, "nothing at all where the span marks nothing");
}

/// Seen red (2026-11-13) with the sight segments drawn whole: "a sight blocker stands 3 m over the
/// open span".
#[test]
fn sight_sees_over_an_open_span_and_the_walk_does_not_cross_it() {
    let h = boxed(vec![span((2.0, 0.0), (8.0, 0.0), 0.03)]);
    let at = |segs: &[crate::wall_collision::WallSegment]| -> f32 {
        segs.iter()
            .filter(|s| s.a.1.abs() < 1e-3 && s.b.1.abs() < 1e-3 && s.a.0.min(s.b.0) < 5.0 && s.a.0.max(s.b.0) > 5.0)
            .map(|s| s.y1)
            .fold(0.0, f32::max)
    };
    let sight = crate::wall_collision::sight_segments_with_shell_cuts(&h, &[]);
    assert!(at(&sight) <= 0.03 + 1e-4, "a sight blocker stands {} m over the open span", at(&sight));
    let walk = crate::wall_collision::wall_segments_with_shell_cuts(&h, &[]);
    assert_eq!(at(&walk), 3.0, "the walk keeps the line over the open span");
}

/// A small ship: two zones touching at x 10, each open over part of that face, a third touching
/// the second at z 10 with nothing open, and a fourth on a deck 3 m up touching the first.
fn ship() -> ShipStructure {
    let text = r#"(
        zones: [
            (id: "a", origin: (0.0, 0.0, 0.0), body: (width: 10.0, depth: 10.0, height: 4.0,
                open_shell: [(a: (10.0, 2.0), b: (10.0, 8.0))])),
            (id: "b", origin: (10.0, 0.0, 0.0), body: (width: 10.0, depth: 10.0, height: 6.0,
                open_shell: [(a: (0.0, 4.0), b: (0.0, 10.0)), (a: (0.0, 10.0), b: (10.0, 10.0))])),
            (id: "c", origin: (10.0, 0.0, 10.0), body: (width: 10.0, depth: 10.0, height: 3.0)),
            (id: "d", origin: (-10.0, 3.0, 0.0), body: (width: 10.0, depth: 10.0, height: 3.0,
                open_shell: [(a: (10.0, 0.0), b: (10.0, 10.0))])),
        ],
    )"#;
    ron::from_str(text).expect("the small ship parses")
}

/// Seen red (2026-11-13) with `shell_cuts_for_zone` cutting only corridors' mouths: "zone a has
/// no cut where it is open beside zone b" (an empty list).
#[test]
fn touching_zones_open_over_the_same_stretch_are_joined_for_the_walk() {
    let s = ship();
    // a's east face (edge 1, running +z) is open z 2..8, b's west face z 4..10: joined z 4..8,
    // 4 m tall (the lower roof).
    let a = s.shell_cuts_for_zone(0);
    assert_eq!(a.len(), 1, "zone a has one cut where it is open beside zone b: {a:?}");
    assert_eq!((a[0].edge, a[0].at, a[0].width, a[0].base, a[0].height), (1, 4.0, 4.0, 0.0, 4.0));
    // b's west face is edge 3, running -z from z 10: z 4..8 is 2..6 along it.
    let b = s.shell_cuts_for_zone(1);
    assert_eq!(b.len(), 1, "zone b is joined to a and not to c, which is open nowhere: {b:?}");
    assert_eq!((b[0].edge, b[0].at, b[0].width), (3, 2.0, 4.0));
    assert!(s.shell_cuts_for_zone(2).is_empty(), "a zone open nowhere is joined to nothing");
    assert!(s.shell_cuts_for_zone(3).is_empty(), "a zone on another deck is joined to nothing, though it touches and is open");
    // Through the cut the walk crosses; elsewhere on the open span it does not.
    let walk = crate::wall_collision::wall_segments_with_shell_cuts(&s.zones[0].body, &a);
    let blocks = |z: f32| walk.iter().any(|w| (w.a.0 - 10.0).abs() < 1e-3 && (w.b.0 - 10.0).abs() < 1e-3 && w.a.1.min(w.b.1) < z && w.a.1.max(w.b.1) > z);
    assert!(!blocks(6.0), "the walk crosses where a and b are joined");
    assert!(blocks(3.0), "the walk does not cross where only a is open");
}

/// The xz footprints of every triangle of a body's walls mesh that stands above its floor, moved
/// by `o` (the body's origin in ship metres): what a downward look at the strip meets.
fn wall_tops(h: &HomeStructure, o: (f32, f32)) -> Vec<(f32, f32, f32, f32)> {
    let m = h.generate_meshes();
    let mut out = Vec::new();
    for (v, idx, _) in &m.material_walls {
        for t in idx.chunks(3) {
            let p: Vec<[f32; 3]> = t.iter().map(|&i| v[i as usize].position).collect();
            if p.iter().map(|q| q[1]).fold(f32::MIN, f32::max) < 0.02 {
                continue;
            }
            let (x0, x1) = (p.iter().map(|q| q[0]).fold(f32::MAX, f32::min), p.iter().map(|q| q[0]).fold(f32::MIN, f32::max));
            let (z0, z1) = (p.iter().map(|q| q[2]).fold(f32::MAX, f32::min), p.iter().map(|q| q[2]).fold(f32::MIN, f32::max));
            out.push((x0 + o.0, x1 + o.0, z0 + o.1, z1 + o.1));
        }
    }
    out
}

/// NOTHING IS OPEN BESIDE A ROW STREET GATE (2026-11-14, the Row Street follow-up). A plot's gate
/// is a door in a corridor of the shortest run a corridor may have, 0.25 m, so the street's edge
/// and the plot's front line stand 0.25 m apart, and there is no floor between them (a corridor
/// is only its own width). Beside each gate the street's kerb and the front garden wall, each
/// 0.2 m thick on its own line, left a 5 cm slot down to the deck below along every frontage.
/// Now the front wall is a brick and a half (0.33 m) and meets the kerb: from above, every point
/// of the strip beside a gate is under the kerb or the wall. Read from the files on disk, as the
/// game loads them: the street from the ship file, each plot's shipped design.
///
/// Seen red 2026-11-14 before the walls were thickened: "r1: the strip beside the gate is open
/// at (93.125, 302.05)".
#[test]
fn nothing_is_open_beside_a_row_street_gate() {
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let ship = ShipStructure::load_ship_file(&data).expect("the shipped ship file");
    let street = ship.zones.iter().find(|z| z.id == "row-street-1").expect("Row Street");
    let street_x1 = street.origin.0 + street.body.width;
    let kerb = wall_tops(&street.body, (street.origin.0, street.origin.2));
    let mut checked = 0;
    for p in ship.plots.iter().filter(|p| p.door.zone == street.id) {
        let path = data.join(format!("homes/shipped/{}.ron", p.kind));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let design: crate::ship_structure::HomeDesign = ron::from_str(&text).expect("the shipped design parses");
        let wall = wall_tops(&design.body, (p.origin.0, p.origin.2));
        let covered = |x: f32, z: f32| kerb.iter().chain(&wall).any(|&(x0, x1, z0, z1)| x >= x0 && x <= x1 && z >= z0 && z <= z1);
        let (gate0, gate1) = (p.door.lat - p.door.width * 0.5, p.door.lat + p.door.width * 0.5);
        assert!(p.origin.0 - street_x1 > 0.2, "{}: the strip is the corridor's run", p.id);
        let mut z = p.origin.2 + 0.05;
        while z < p.origin.2 + p.size.2 - 0.05 {
            if z < gate0 - 0.05 || z > gate1 + 0.05 {
                for x in [street_x1 + 0.02, street_x1 + 0.09, street_x1 + 0.125, street_x1 + 0.2, p.origin.0 - 0.01] {
                    assert!(covered(x, z), "{}: the strip beside the gate is open at ({x}, {z})", p.id);
                    checked += 1;
                }
            }
            z += 0.25;
        }
    }
    assert!(checked > 1000, "every Row Street frontage was looked at ({checked} points)");
}

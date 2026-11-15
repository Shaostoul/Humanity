//! The room trim's tests (room finishes rung 1, docs/design/room-finishes.md).

use super::*;
use crate::room_finishes::Finish;

fn home(src: &str) -> HomeStructure {
    ron::from_str(src).unwrap_or_else(|e| panic!("test home: {e}"))
}

/// A 6 x 4 x 3 m box, opaque roof, one living room zone over all of it, and a stub wall from the
/// south shell 2 m into the room: inside corners where it meets the shell, outside corners round
/// its free end.
fn box_with_stub() -> HomeStructure {
    home(
        r#"(width: 6.0, depth: 4.0, height: 3.0, roof_material: 1,
            walls: [(a: (3.0, 0.0), b: (3.0, 2.0), material: 2)],
            zones: [(id: "room-a", type_id: "room_common", origin: (0.0, 0.0, 0.0), size: (6.0, 3.0, 4.0), room_type: Some("livingroom"))])"#,
    )
}

/// Two living rooms parted by a wall at x = 3 with a 0.9 m door 1.5 m along it.
fn two_rooms_with_door() -> HomeStructure {
    home(
        r#"(width: 6.0, depth: 4.0, height: 3.0, roof_material: 1,
            walls: [(a: (3.0, 0.0), b: (3.0, 4.0), material: 2, openings: [(kind: Door, at: 1.5, width: 0.9, height: 2.1)])],
            zones: [
                (id: "room-w", type_id: "room_common", origin: (0.0, 0.0, 0.0), size: (3.0, 3.0, 4.0), room_type: Some("livingroom")),
                (id: "room-e", type_id: "room_common", origin: (3.0, 0.0, 0.0), size: (3.0, 3.0, 4.0), room_type: Some("livingroom")),
            ])"#,
    )
}

/// A PROFILE SWEEPS WITH MITRED CORNERS MEETING EXACTLY. Round the box with the stub every skirting
/// and crown run turns onto the next (no run is stopped: there is no door), and at every corner,
/// inside or outside, each profile point of one run's end sits exactly where the next run's start
/// puts it. Seen red 2026-11-14 with every run's ends cut square (`run_ends` ignoring its
/// neighbours): "corner 0 of the Skirting does not meet at point 1: (3.0830, 2.0000) against
/// (3.0750, 2.0080)", the notch two square cuts leave round the stub's free end.
#[test]
fn a_profile_sweeps_with_mitred_corners_meeting_exactly() {
    let hs = box_with_stub();
    let all = runs(&hs, &[]);
    for kind in [ProfileKind::Skirting, ProfileKind::Crown] {
        let mine: Vec<&Run> = all.iter().filter(|r| r.kind == kind).collect();
        // The south shell face in two (the stub parts it), the stub's two faces and its end, the
        // other three shell faces: eight runs, one closed loop.
        assert_eq!(mine.len(), 8, "{kind:?}: {} runs", mine.len());
        for (ci, r) in mine.iter().enumerate() {
            assert!(!r.free_a && !r.free_b, "{kind:?}: run {ci} is stopped in a room with no door");
            let next = mine.iter().find(|q| (q.a - r.b).length() < JOIN_M).expect("a run starts where each ends");
            let pts = &r.profile.points;
            let (_, end) = run_ends(&all, r, pts);
            let (start, _) = run_ends(&all, next, pts);
            for (k, (e, s)) in end.iter().zip(&start).enumerate() {
                assert!((*e - *s).length() < 1e-5, "corner {ci} of the {kind:?} does not meet at point {k}: ({:.4}, {:.4}) against ({:.4}, {:.4})", e.x, e.y, s.x, s.y);
            }
        }
    }
    // A 90-degree inside corner's mitre is the diagonal: the deepest point sits `depth` out from both
    // faces. The shell is 6 cm steel, its inner faces 3 cm in.
    let sk = all.iter().find(|r| r.kind == ProfileKind::Skirting).unwrap();
    let depth = sk.profile.depth();
    let t = hs.shell_resolved_thickness() * 0.5;
    let corner = Vec2::new(t + depth, t + depth);
    let hits = all.iter().filter(|r| r.kind == ProfileKind::Skirting).any(|r| {
        let (s, e) = run_ends(&all, r, &r.profile.points);
        s.iter().chain(&e).any(|p| (*p - corner).length() < 1e-5)
    });
    assert!(hits, "no skirting point at the south-west corner's mitre ({:.4}, {:.4})", corner.x, corner.y);
}

/// TRIM STOPS AT A DOOR OPENING. On both faces of the parting wall the skirting stops square at the
/// door's jambs (1.5 and 2.4 m along it) and nothing of it lies across the doorway, while the crown,
/// above the 2.1 m door, runs on unbroken. Seen red 2026-11-14 with `blocked` returning no gaps:
/// "two pieces a side", left 2 against 4 (one skirting run a side, straight across the doorway).
#[test]
fn trim_stops_at_a_door_opening() {
    let hs = two_rooms_with_door();
    let all = runs(&hs, &[]);
    let along = |p: Vec2| p.y; // the wall runs from (3, 0) to (3, 4): metres along it are z
    let on_wall = |r: &&Run| (r.a.x - 3.0).abs() < 0.2 && (r.b.x - 3.0).abs() < 0.2 && (r.a.y - r.b.y).abs() > 1e-3;
    let skirting: Vec<&Run> = all.iter().filter(|r| r.kind == ProfileKind::Skirting).filter(on_wall).collect();
    assert_eq!(skirting.len(), 4, "two pieces a side");
    for r in &skirting {
        let (lo, hi) = (along(r.a).min(along(r.b)), along(r.a).max(along(r.b)));
        assert!(hi <= 1.5 + 1e-4 || lo >= 2.4 - 1e-4, "a skirting run crosses the doorway: {lo:.2} to {hi:.2} m along the wall");
        // The end at the jamb is stopped (capped), the other turns into the room's corner.
        let at_jamb = |p: Vec2| (along(p) - 1.5).abs() < 1e-4 || (along(p) - 2.4).abs() < 1e-4;
        assert_eq!(r.free_a, at_jamb(r.a), "start stopped only at the jamb");
        assert_eq!(r.free_b, at_jamb(r.b), "end stopped only at the jamb");
    }
    let crown: Vec<&Run> = all.iter().filter(|r| r.kind == ProfileKind::Crown).filter(on_wall).collect();
    assert_eq!(crown.len(), 2, "the crown runs over the door, one run a side");
    for r in &crown {
        assert!(!r.free_a && !r.free_b);
    }
}

/// A WALL FACE'S OWN FINISH WINS OVER ITS ROOM'S, on the face it is set on and nowhere else: the
/// parting wall's left side (walking from its a at z 0 to its b at z 4, z south, your left is east)
/// in oak over the east room's charcoal, the west face in the west room's white enamel. Seen red
/// 2026-11-14 with `resolve` taking the room's choice over the face's: the east face came out
/// "charcoal" where "oak" was set.
#[test]
fn a_per_wall_override_wins_over_the_rooms_choice() {
    let mut hs = two_rooms_with_door();
    hs.walls[0].finish.left = Finish { material: Some("oak".into()), ..Default::default() };
    hs.zones[0].finish = Finish { skirting: Some("ogee_170".into()), ..Default::default() };
    hs.zones[1].finish = Finish { material: Some("charcoal".into()), ..Default::default() };
    let all = runs(&hs, &[]);
    // Walking from (3, 0) to (3, 4) (south, z growing), your left is east: x > 3.
    for r in all.iter().filter(|r| (r.a.x - 3.0).abs() < 0.2 && (r.b.x - 3.0).abs() < 0.2 && (r.a.y - r.b.y).abs() > 1e-3) {
        let east = r.n.x > 0.5;
        assert_eq!(r.material, if east { "oak" } else { "white_enamel" }, "the {} face", if east { "east (left)" } else { "west (right)" });
        if r.kind == ProfileKind::Skirting {
            // The west room chose an ogee skirting; the east face keeps the living room's torus.
            assert_eq!(r.profile.id, if east { "torus_145" } else { "ogee_170" });
        }
    }
    // The bake puts each material in its own group.
    let groups = bake_trim(&hs, &[], TrimDetail::Full);
    let looks: Vec<f32> = groups.iter().map(|g| g.look.material_type).collect();
    assert!(looks.contains(&3.0) && looks.contains(&0.0), "an oak (wood) group and a painted one: {looks:?}");
}

/// THE SAVED HOME ROUND-TRIPS its finishes: a zone's choice and a wall side's choice come back from
/// the RON the editor saves, and a home with none writes no `finish` at all (so every home saved
/// before finishes is written as before). Seen red 2026-11-14 with `Zone::finish` marked
/// `#[serde(skip)]`: "the room's finish came back empty".
#[test]
fn the_saved_home_round_trips() {
    let mut hs = two_rooms_with_door();
    let plain = ron::ser::to_string(&hs).unwrap();
    assert!(!plain.contains("finish"), "a home with no finishes writes none: {plain}");
    hs.zones[1].finish = Finish { skirting: Some("bullnose_120".into()), crown: Some("none".into()), material: Some("soft_grey".into()) };
    hs.walls[0].finish.right = Finish { material: Some("charcoal".into()), ..Default::default() };
    let text = ron::ser::to_string(&hs).unwrap();
    let back: HomeStructure = ron::from_str(&text).unwrap();
    assert_eq!(back.zones[1].finish, hs.zones[1].finish, "the room's finish came back empty");
    assert_eq!(back.walls[0].finish, hs.walls[0].finish, "the wall's finish came back");
    assert!(back.zones[0].finish.is_empty() && back.walls[0].finish.left.is_empty());
}

/// The low detail a neighbour's home is drawn at costs a fraction of the full, and neither is empty.
#[test]
fn coarse_trim_is_a_fraction_of_the_full() {
    let hs = box_with_stub();
    let count = |g: &[TrimGroup]| g.iter().map(|g| g.verts.len()).sum::<usize>();
    let (full, coarse) = (count(&bake_trim(&hs, &[], TrimDetail::Full)), count(&bake_trim(&hs, &[], TrimDetail::Coarse)));
    assert!(coarse > 0 && full > 0);
    assert!(coarse * 3 < full, "coarse {coarse} against full {full}");
}

/// Every triangle of the bake faces the normal it is lit by (the opaque pass culls back faces).
#[test]
fn trim_triangles_face_their_normals() {
    for g in bake_trim(&two_rooms_with_door(), &[], TrimDetail::Full) {
        for t in g.indices.chunks_exact(3) {
            let p: Vec<Vec3> = t.iter().map(|&i| Vec3::from(g.verts[i as usize].position)).collect();
            let n = Vec3::from(g.verts[t[0] as usize].normal);
            let geo = (p[1] - p[0]).cross(p[2] - p[0]);
            if geo.length() > 1e-9 {
                assert!(geo.dot(n) > -1e-7, "a triangle faces away from its normal");
            }
        }
    }
}

/// THE SHIPPED HOMES' TRIM, in budget (docs/design/room-finishes.md, "What it costs"): every
/// shipped design bakes, and the homestead's full trim stays under the vertex budget the plan sets.
#[test]
fn shipped_homes_bake_their_trim_within_budget() {
    let dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/homes");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".ron") || name == "starts.ron" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let Ok(design) = ron::from_str::<crate::ship_structure::HomeDesign>(&text) else { continue };
        let t0 = std::time::Instant::now();
        let full: usize = bake_trim(&design.body, &[], TrimDetail::Full).iter().map(|g| g.verts.len()).sum();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let coarse: usize = bake_trim(&design.body, &[], TrimDetail::Coarse).iter().map(|g| g.verts.len()).sum();
        println!("{name}: full {full} vertices ({ms:.1} ms), coarse {coarse}");
        assert!(full > 0, "{name} has trim");
        assert!(full <= TRIM_VERTEX_BUDGET, "{name}: {full} trim vertices, over the {TRIM_VERTEX_BUDGET} budget");
        assert!(coarse <= TRIM_VERTEX_BUDGET / 4, "{name}: {coarse} coarse trim vertices");
        seen += 1;
    }
    assert!(seen >= 5, "read {seen} home designs");
}

/// The most trim vertices one home may bake at full detail (docs/design/room-finishes.md): about
/// four times the homestead's 15,518 (2026-11-14, each profile segment a quad of four shared
/// corners), so a home twice as finished still passes and a sweep that runs away (a profile swept
/// per cell, a run per pixel of wall) does not.
const TRIM_VERTEX_BUDGET: usize = 60_000;

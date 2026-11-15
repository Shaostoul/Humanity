//! The room trim's tests (room finishes rung 1, docs/design/room-finishes.md).

use super::*;
use crate::room_casings::frame_sweeps;
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

/// TRIM STOPS AT A DOOR OPENING, AT ITS CASING'S EDGE (rung 2). On both faces of the parting wall
/// the skirting stops square against the outer edge of the door's casing (the living room's
/// 70 mm ogee, so 1.43 and 2.47 m along the wall, the jambs at 1.5 and 2.4) and nothing of it lies
/// across the doorway or behind the casing, while the crown, above the 2.1 m door and its head
/// casing, runs on unbroken. Seen red 2026-11-14 with `blocked` returning no gaps: "two pieces a
/// side", left 2 against 4 (one skirting run a side, straight across the doorway). Seen red again
/// 2026-11-15 with rung 1's stop at the jamb (`gaps` ignoring the casing's reach): "a skirting run
/// runs in behind the casing: 0.03 to 1.50 m along the wall".
#[test]
fn the_skirting_stops_at_the_casings_edge() {
    let hs = two_rooms_with_door();
    let all = runs(&hs, &[]);
    let along = |p: Vec2| p.y; // the wall runs from (3, 0) to (3, 4): metres along it are z
    let on_wall = |r: &&Run| (r.a.x - 3.0).abs() < 0.2 && (r.b.x - 3.0).abs() < 0.2 && (r.a.y - r.b.y).abs() > 1e-3;
    let skirting: Vec<&Run> = all.iter().filter(|r| r.kind == ProfileKind::Skirting).filter(on_wall).collect();
    assert_eq!(skirting.len(), 4, "two pieces a side");
    let (c0, c1) = (1.5 - 0.07, 2.4 + 0.07);
    for r in &skirting {
        let (lo, hi) = (along(r.a).min(along(r.b)), along(r.a).max(along(r.b)));
        assert!(hi <= c0 + 1e-4 || lo >= c1 - 1e-4, "a skirting run runs in behind the casing: {lo:.2} to {hi:.2} m along the wall");
        // The end at the casing is stopped (capped), the other turns into the room's corner.
        let at_casing = |p: Vec2| (along(p) - c0).abs() < 1e-4 || (along(p) - c1).abs() < 1e-4;
        assert_eq!(r.free_a, at_casing(r.a), "start stopped only at the casing");
        assert_eq!(r.free_b, at_casing(r.b), "end stopped only at the casing");
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

/// THE SAVED HOME ROUND-TRIPS its finishes: a zone's choice, a wall side's choice and an outer
/// wall's (rung 2: the casing, the stool and the rails among them) come back from the RON the
/// editor saves, and a home with none writes no `finish` at all (so every home saved before
/// finishes is written as before). Seen red 2026-11-14 with `Zone::finish` marked
/// `#[serde(skip)]`: "the room's finish came back empty"; and 2026-11-15 with
/// `HomeStructure::shell_finish` marked `#[serde(skip)]`: "the outer wall's finish came back".
#[test]
fn the_saved_home_round_trips() {
    let mut hs = two_rooms_with_door();
    let plain = ron::ser::to_string(&hs).unwrap();
    assert!(!plain.contains("finish"), "a home with no finishes writes none: {plain}");
    hs.zones[1].finish = Finish {
        skirting: Some("bullnose_120".into()),
        crown: Some("none".into()),
        material: Some("soft_grey".into()),
        casing: Some("flat_casing_70".into()),
        sill: Some("none".into()),
        chair_rail: Some("torus_dado_58".into()),
        picture_rail: Some("picture_rail_45".into()),
    };
    hs.walls[0].finish.right = Finish { material: Some("charcoal".into()), chair_rail: Some("square_rail_45".into()), ..Default::default() };
    hs.shell_finish.south = Finish { picture_rail: Some("none".into()), casing: Some("pencil_round_casing_58".into()), ..Default::default() };
    let text = ron::ser::to_string(&hs).unwrap();
    let back: HomeStructure = ron::from_str(&text).unwrap();
    assert_eq!(back.zones[1].finish, hs.zones[1].finish, "the room's finish came back empty");
    assert_eq!(back.walls[0].finish, hs.walls[0].finish, "the wall's finish came back");
    assert_eq!(back.shell_finish, hs.shell_finish, "the outer wall's finish came back");
    assert!(back.zones[0].finish.is_empty() && back.walls[0].finish.left.is_empty() && back.shell_finish.north.is_empty());
}

/// Two living rooms parted by a wall at x = 3 with a 0.9 m door 0.5 m along it and a 1.2 m window
/// (sill 0.9 m, 1.3 m tall) 2.0 m along it.
fn two_rooms_with_door_and_window() -> HomeStructure {
    home(
        r#"(width: 6.0, depth: 4.0, height: 3.0, roof_material: 1,
            walls: [(a: (3.0, 0.0), b: (3.0, 4.0), material: 2, openings: [
                (kind: Door, at: 0.5, width: 0.9, height: 2.1),
                (kind: Window, at: 2.0, width: 1.2, sill: 0.9, height: 1.3),
            ])],
            zones: [
                (id: "room-w", type_id: "room_common", origin: (0.0, 0.0, 0.0), size: (3.0, 3.0, 4.0), room_type: Some("livingroom")),
                (id: "room-e", type_id: "room_common", origin: (3.0, 0.0, 0.0), size: (3.0, 3.0, 4.0), room_type: Some("livingroom")),
            ])"#,
    )
}

/// The frames of the parting wall at x = 3, one per opening per face.
fn wall_frames(hs: &HomeStructure) -> Vec<Frame> {
    layout(hs, &[]).frames.into_iter().filter(|f| matches!(f.face, Src::Face { wall: 0, .. })).collect()
}

/// CASING MITRES MEET AT THE HEAD (rung 2). Round the door on both faces the casing is three parts,
/// the legs standing on the floor and the head across, and at each head corner every profile point
/// of the leg's end sits exactly where the head's start (or end) puts it, the outer edge meeting on
/// the 45-degree diagonal (the casing's width out and up from the opening's corner). A window with
/// no stool is framed all round, mitred at all four corners. Seen red 2026-11-15 with every part's
/// ends cut square (`frame_sweeps` ignoring `prev` and `next`): "the door's head corner 0 does not
/// meet at point 0: (3.0750, 2.1000, 0.4300) against (3.0750, 2.1700, 0.5000)", the leg stopping
/// at the head's foot and the head starting over the jamb.
#[test]
fn casing_mitres_meet_at_the_head() {
    let mut hs = two_rooms_with_door_and_window();
    let check = |frames: &[Frame], what: &str, parts: usize| {
        assert_eq!(frames.len(), 2, "{what}: one frame a face");
        for f in frames {
            let sw = frame_sweeps(f, TrimDetail::Full);
            let ring: Vec<&Swept> = sw.iter().filter(|s| s.kind == ProfileKind::Casing && !s.cap_a).collect();
            assert_eq!(ring.len(), parts, "{what}: {parts} mitred parts");
            for c in 0..ring.len() - usize::from(parts == 3) {
                let (p, q) = (ring[c], ring[(c + 1) % ring.len()]);
                for (k, (e, s)) in p.e.iter().zip(&q.s).enumerate() {
                    assert!((*e - *s).length() < 1e-5, "the {what}'s head corner {c} does not meet at point {k}: ({:.4}, {:.4}, {:.4}) against ({:.4}, {:.4}, {:.4})", e.x, e.y, e.z, s.x, s.y, s.z);
                }
            }
            // The outer edge's point (the profile's first, at the casing's width) on the diagonal.
            let corner = ring[0].e[0];
            assert!((corner.y - (f.top + f.width)).abs() < 1e-5, "{what}: the outer corner at the head's top, {:.4}", corner.y);
            let s = (Vec2::new(corner.x, corner.z) - f.origin).dot(f.dir);
            assert!((s - (f.s0 - f.width)).abs() < 1e-5, "{what}: the outer corner the casing's width beyond the jamb, {s:.4}");
            if parts == 3 {
                assert!(ring[0].s.iter().all(|p| (p.y - f.floor).abs() < 1e-5), "{what}: the legs stand on the floor");
            }
        }
    };
    let frames = wall_frames(&hs);
    let door: Vec<Frame> = frames.iter().filter(|f| f.door).cloned().collect();
    check(&door, "door", 3);
    // The window with no stool on either side: framed on all four sides.
    for z in &mut hs.zones {
        z.finish.sill = Some("none".into());
    }
    let window: Vec<Frame> = wall_frames(&hs).into_iter().filter(|f| !f.door).collect();
    check(&window, "window", 4);
}

/// A WINDOW STANDS ON ITS STOOL: the legs start at the opening's foot, the stool's top is level
/// with it and runs the casing's width and its horns past each jamb, and the apron hangs under the
/// stool, as wide as the casing outside to outside.
#[test]
fn a_window_has_a_stool_and_an_apron() {
    let hs = two_rooms_with_door_and_window();
    let cat = catalog();
    for f in wall_frames(&hs).iter().filter(|f| !f.door) {
        let sill = f.sill.expect("a living room window has a stool");
        assert!(f.apron, "and an apron");
        assert_eq!(sill.id, "bullnose_stool_25");
        let sw = frame_sweeps(f, TrimDetail::Full);
        let stool = sw.iter().find(|s| s.kind == ProfileKind::Sill).expect("a stool");
        let top = stool.s.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((top - 0.9).abs() < 1e-5, "the stool's top level with the sill, {top:.4}");
        let reach = f.width + cat.fit.sill_horn_m;
        let span = |p: Vec3| (Vec2::new(p.x, p.z) - f.origin).dot(f.dir);
        assert!((span(stool.s[0]) - (2.0 - reach)).abs() < 1e-5 && (span(stool.e[0]) - (3.2 + reach)).abs() < 1e-5, "the stool runs past the casing by its horns");
        assert!(stool.cap_a && stool.cap_b, "the stool's ends are closed");
        let legs: Vec<&Swept> = sw.iter().filter(|s| s.kind == ProfileKind::Casing && !s.cap_a).collect();
        assert!(legs[0].s.iter().all(|p| (p.y - 0.9).abs() < 1e-5), "the legs stand on the stool");
        let apron = sw.iter().find(|s| s.kind == ProfileKind::Casing && s.cap_a).expect("an apron");
        assert!((span(apron.s[0]) - (2.0 - f.width)).abs() < 1e-5, "the apron as wide as the casing outside to outside");
        assert!(apron.s.iter().all(|p| p.y <= 0.9 - sill.height() + 1e-5), "the apron under the stool");
    }
}

/// A RAIL STOPS AT A CASING (rung 2): a chair rail chosen for both rooms runs along the parting
/// wall and stops square against the door's casing and against the window's stool (whose horns
/// run past the casing, so the rail stops there), never crossing either, while a picture rail high
/// over both runs on unbroken. Seen red 2026-11-15 with the rails stopped as an uncased opening
/// stops them (`gaps` without the casings' reach): "a chair rail run crosses the door's casing:
/// 0.03 to 0.50 m along the wall".
#[test]
fn a_rail_stops_at_a_casing() {
    let mut hs = two_rooms_with_door_and_window();
    for z in &mut hs.zones {
        z.finish.chair_rail = Some("torus_dado_58".into());
        z.finish.picture_rail = Some("picture_rail_45".into());
    }
    let all = runs(&hs, &[]);
    let along = |p: Vec2| p.y;
    let on_wall = |r: &&Run| (r.a.x - 3.0).abs() < 0.2 && (r.b.x - 3.0).abs() < 0.2 && (r.a.y - r.b.y).abs() > 1e-3;
    let horn = catalog().fit.sill_horn_m;
    let (door, window) = ((0.5 - 0.07, 1.4 + 0.07), (2.0 - 0.07 - horn, 3.2 + 0.07 + horn));
    let rail: Vec<&Run> = all.iter().filter(|r| r.kind == ProfileKind::ChairRail).filter(on_wall).collect();
    assert_eq!(rail.len(), 6, "three pieces a side: to the door, door to window, window to the corner");
    for r in &rail {
        let (lo, hi) = (along(r.a).min(along(r.b)), along(r.a).max(along(r.b)));
        for (what, (g0, g1)) in [("door's casing", door), ("window's trim", window)] {
            assert!(hi <= g0 + 1e-4 || lo >= g1 - 1e-4, "a chair rail run crosses the {what}: {lo:.2} to {hi:.2} m along the wall");
        }
        let at_trim = |p: Vec2| [door.0, door.1, window.0, window.1].iter().any(|g| (along(p) - g).abs() < 1e-4);
        assert_eq!(r.free_a, at_trim(r.a), "a rail is stopped only at a casing");
        assert_eq!(r.free_b, at_trim(r.b), "a rail is stopped only at a casing");
        assert!((r.y + r.profile.height() * 0.5 - 0.9).abs() < 1e-5, "centred at 0.9 m");
    }
    let picture: Vec<&Run> = all.iter().filter(|r| r.kind == ProfileKind::PictureRail).filter(on_wall).collect();
    assert_eq!(picture.len(), 2, "the picture rail over the door and the window, one run a side");
    assert!(picture.iter().all(|r| !r.free_a && !r.free_b && (r.y + r.profile.height() - 2.7).abs() < 1e-5), "0.3 m under the ceiling, turning into the corners");
}

/// A WALL SIDE'S RAIL CHOICE BEATS THE ROOM'S, and an outer wall's beats the rooms along it (rung
/// 2, the per-edge finish): both rooms choose a torus dado rail; the parting wall's left side (east)
/// chooses a square one; the north outer wall chooses none. Seen red 2026-11-15 with the wall
/// sides' choices left out of the resolution (`layout` passing no face choice for a wall): "the
/// east face's chair rail torus_dado_58, chosen square_rail_45"; and with the shell's faces
/// resolved as in rung 1 (no choice of their own): "a chair rail runs along the north outer wall,
/// which chose none".
#[test]
fn a_wall_sides_rail_choice_beats_the_rooms() {
    let mut hs = two_rooms_with_door_and_window();
    for z in &mut hs.zones {
        z.finish.chair_rail = Some("torus_dado_58".into());
    }
    hs.walls[0].finish.left.chair_rail = Some("square_rail_45".into());
    hs.shell_finish.north.chair_rail = Some("none".into());
    let all = runs(&hs, &[]);
    let mut seen = 0;
    for r in all.iter().filter(|r| r.kind == ProfileKind::ChairRail && (r.a.x - 3.0).abs() < 0.2 && (r.b.x - 3.0).abs() < 0.2) {
        let east = r.n.x > 0.5;
        let want = if east { "square_rail_45" } else { "torus_dado_58" };
        assert_eq!(r.profile.id, want, "the {} face's chair rail {}, chosen {want}", if east { "east" } else { "west" }, r.profile.id);
        seen += 1;
    }
    assert!(seen >= 4);
    let t = hs.shell_resolved_thickness() * 0.5;
    assert!(
        !all.iter().any(|r| r.kind == ProfileKind::ChairRail && (r.a.y - t).abs() < 1e-3 && (r.b.y - t).abs() < 1e-3),
        "a chair rail runs along the north outer wall, which chose none"
    );
    assert!(all.iter().any(|r| r.kind == ProfileKind::ChairRail && (r.a.y - (4.0 - t)).abs() < 1e-3), "the south outer wall keeps the rooms' rail");
}

/// A CASING NEAR A CORNER IS RIPPED NARROWER, never run into the wall beside it: a door 8 cm from
/// the outer wall (whose face is 3 cm in) gets a 5 cm casing on that side's edge and the head, and
/// no point of it lies inside the outer wall.
#[test]
fn a_casing_near_a_corner_is_ripped() {
    let mut hs = two_rooms_with_door();
    hs.walls[0].openings[0].at = 0.08;
    let t = hs.shell_resolved_thickness() * 0.5;
    for f in wall_frames(&hs) {
        assert!((f.width - (0.08 - t)).abs() < 1e-5, "ripped to the room beside it: {:.4}", f.width);
        for sw in frame_sweeps(&f, TrimDetail::Full) {
            assert!(sw.s.iter().chain(&sw.e).all(|p| p.z >= t - 1e-5), "a {:?} point inside the outer wall", sw.kind);
        }
    }
    // Two doors 0.1 m apart: their casings butt halfway, each ripped to 0.05 m, neither over the
    // other. Seen red 2026-11-15 without the neighbour's half: "two casings overlap between the
    // doors: 0.0700 m wide".
    let mut two = two_rooms_with_door();
    let mut second = two.walls[0].openings[0].clone();
    second.at = 1.5 + 0.9 + 0.1;
    two.walls[0].openings.push(second);
    for f in wall_frames(&two) {
        assert!((f.width - 0.05).abs() < 1e-5, "two casings overlap between the doors: {:.4} m wide", f.width);
    }
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
    let mut cased = two_rooms_with_door_and_window();
    cased.zones[0].finish.chair_rail = Some("torus_dado_58".into());
    cased.zones[1].finish.sill = Some("none".into());
    for g in [bake_trim(&two_rooms_with_door(), &[], TrimDetail::Full), bake_trim(&cased, &[], TrimDetail::Full)].into_iter().flatten() {
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

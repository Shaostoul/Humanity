//! From ship/wall_collision.rs (`homestead_walk_tests`): walking the homestead's stairs, ladder,
//! gallery and doors with the engine's footing (`footing::test_aboard`).

//! The homestead's upper floor, walked (ship homes increment 7 review, the movement findings):
//! the shell over the front door upstairs, the back and the high end of stairs-1, and the guards
//! a jump used to clear. On the shipped ship with the homestead on p1, whose home stands at the
//! ship's origin, so its home-local metres are the world's.

use crate::ship::wall_collision::*;
use crate::engine::footing::test_aboard::{body_at, live_frame, Aboard, EAST, NORTH, SOUTH, WEST};
use crate::input::bindings::GameAction as A;
use crate::ship::ship_structure::ShipStructure;
use glam::Vec3;

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

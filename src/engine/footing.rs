//! Where the floor is under a body aboard: the room it stands in, raised to the highest stair
//! tread, ramp, deck, platform or elevator car it can step onto (v0.584, v0.589, v0.590).
//!
//! Moved out of the main loop on 2026-10-20 (BUG-207) so the aboard walk can ask it more than
//! once a frame. lib.rs asks it once before the walk moves the body, as it always did, and the
//! walk asks again between the shorter steps a long frame is cut into
//! (`CameraController::update_camera_with_floor`, `renderer::camera::WALK_SUBSTEP_M`), so a
//! slow frame walks over every stair tread a fast one does.

use std::collections::HashMap;

use glam::Vec3;

use crate::gui::RoomBounds;
use crate::ship::ship_structure::ShipStructure;
use crate::ship::structure::{self, StructureKind};
use crate::ship::wall_collision::{self, WallSegment};

/// The highest a body steps up in one go, metres (v0.584): onto a stair tread or a platform,
/// never up the side of a tall box (you use the stairs). Measured from the floor the body rests
/// on (`CameraController::step_up_from`), so a jump cannot carry you onto a tall box either.
pub const STEP_UP_M: f32 = 0.6;

/// How far an elevator car travels, metres: one storey (v0.590).
pub const ELEVATOR_TRAVEL_M: f32 = 3.0;

/// Each elevator's runtime state, by (zone index, structure index): its car's place along the
/// travel (0 at the bottom, 1 at the top), where it is heading, and whether someone rides it.
/// `EngineState::elevator_state`.
pub type ElevatorCars = HashMap<(usize, usize), (f32, f32, bool)>;

/// The floor under the point `at` (world metres; its height is not read): the floor of the room
/// it is in, raised to the highest structure surface there that is no more than [`STEP_UP_M`]
/// above `step_from`. None when `at` is in no room: the walk then keeps the floor it has.
///
/// Descending is always allowed: a lower surface simply is the floor. An ELEVATOR's surface is
/// its moving car (`elevators`), so standing on it rides you up and down; every other piece's is
/// its static `structure::walk_surface`. EVERY zone's structures, at world positions (v0.754).
pub fn floor_under(
    rooms: &[RoomBounds],
    ship: Option<&ShipStructure>,
    elevators: &ElevatorCars,
    at: Vec3,
    step_from: f32,
) -> Option<f32> {
    let mut floor = rooms
        .iter()
        .find(|r| at.x >= r.min.x && at.x <= r.max.x && at.z >= r.min.z && at.z <= r.max.z)
        .map(|r| r.min.y)?;
    let Some(ship) = ship else { return Some(floor) };
    for (zi, zone) in ship.zones.iter().enumerate() {
        let o = zone.origin;
        for (i, ps) in zone.body.structures.iter().enumerate() {
            let Some(ty) = structure::structure_type(&ps.type_id) else { continue };
            let wpos = (ps.pos.0 + o.0, ps.pos.1 + o.1, ps.pos.2 + o.2);
            let yaw = ps.rot_deg.to_radians();
            let surf = if ty.kind == StructureKind::Elevator {
                if structure::in_footprint(ty, wpos, yaw, at.x, at.z) {
                    let anim = elevators.get(&(zi, i)).map_or(0.0, |e| e.0);
                    Some(wpos.1 + anim * ELEVATOR_TRAVEL_M)
                } else {
                    None
                }
            } else {
                structure::walk_surface(ty, wpos, yaw, at.x, at.z)
            };
            if let Some(top) = surf {
                if top <= step_from + STEP_UP_M && top > floor {
                    floor = top;
                }
            }
        }
    }
    Some(floor)
}

/// The floor under the point `at` a walk has reached part way through a frame (BUG-207), for a body
/// that began the frame at `frame_start` (clear of the walls: last frame's sweep put it there).
///
/// The walls are swept once the whole frame has been walked (lib.rs, `wall_collision::resolve`
/// from where the frame began, after the walk), so a long frame's substep can put `at` part way
/// into or through a wall. The floor is read where that same sweep holds the body instead, so a
/// long frame walking into a wall does not step onto a deck on its far side. `walls` are the
/// home's walls (`EngineState::wall_colliders`): the closed doors and built pieces the sweep adds
/// are gathered after the walk and are not counted, and the most a missed one can do is lift the
/// body by one step up (`STEP_UP_M`, 0.6 m) for part of a frame, a drop that lands at 3.4 m/s,
/// far below the slowest landing that hurts (data/medical/falls.ron).
pub fn floor_along(
    rooms: &[RoomBounds],
    ship: Option<&ShipStructure>,
    elevators: &ElevatorCars,
    walls: &[WallSegment],
    frame_start: Vec3,
    at: Vec3,
    step_from: f32,
) -> Option<f32> {
    let held = if walls.is_empty() { at } else { wall_collision::resolve(frame_start, at, wall_collision::PLAYER_RADIUS, walls, &[]) };
    floor_under(rooms, ship, elevators, held, step_from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::bindings::GameAction as A;
    use crate::renderer::camera::{Camera, CameraController, SPRINT_FACTOR, STEP_DOWN_M, WALK_SPEED_MPS, WALK_SUBSTEP_M};
    use crate::ship::ship_structure::ShipZone;
    use crate::systems::falls::drop_height_m;

    /// The homestead's gravity (data/game.csv `gravity_m_s2`).
    const G: f32 = 9.81;
    /// How many starting points, spread over one frame's travel, each slow walk is tried from:
    /// whether a frame's stride lands on a tread depends on where the walk began.
    const PHASES: usize = 12;

    /// A 20 x 20 m room with one piece of the homestead's "stairs" (data/blueprints/
    /// structure_types.ron: 3.0 m high, 14 steps up 3.6 m, so 0.214 m risers on 0.257 m treads)
    /// centred at z = 6, climbing +Z from its foot at z = 4.2 to its top at z = 7.8, with
    /// nothing beyond the top: the homestead's `stairs-1` leads to no upper floor either.
    struct Room {
        rooms: Vec<RoomBounds>,
        ship: ShipStructure,
        cars: ElevatorCars,
    }

    fn room_with_stairs() -> Room {
        let zone: ShipZone = ron::from_str(
            r#"(id: "home", body: (width: 20.0, depth: 20.0, height: 6.0,
                structures: [(id: "stairs-1", type_id: "stairs", pos: (10.0, 0.0, 6.0))]))"#,
        )
        .expect("the test zone parses");
        let room = RoomBounds {
            id: "room".into(),
            min: Vec3::ZERO,
            max: Vec3::new(20.0, 6.0, 20.0),
            display_name: String::new(),
            purpose: String::new(),
            actions: Vec::new(),
            action_pages: Vec::new(),
            access: String::new(),
        };
        Room { rooms: vec![room], ship: ShipStructure { zones: vec![zone], ..Default::default() }, cars: ElevatorCars::new() }
    }

    /// The stairs' tread heights, bottom to top: 3.0 / 14 each.
    fn tread_heights() -> Vec<f32> {
        let ty = structure::structure_type("stairs").expect("the stairs type");
        (1..=ty.steps).map(|k| k as f32 * ty.size.1 / ty.steps as f32).collect()
    }

    /// One frame as lib.rs runs it: the floor under the body before it moves, then the walk, which
    /// asks for the floor again between a long frame's substeps (`floor_along`; this room has no
    /// walls). `rested` collects every floor the walk was given to stand on.
    fn frame(ctl: &mut CameraController, cam: &mut Camera, dt: f32, room: &Room, rested: &mut Vec<f32>) {
        let start = cam.position;
        if let Some(f) = floor_under(&room.rooms, Some(&room.ship), &room.cars, start, ctl.step_up_from(start.y)) {
            ctl.set_ground_floor(f);
        }
        rested.push(ctl.ground_floor());
        let mut floor_at = |at: Vec3, step_from: f32| {
            let f = floor_along(&room.rooms, Some(&room.ship), &room.cars, &[], start, at, step_from);
            rested.extend(f);
            f
        };
        ctl.update_camera_with_floor(cam, dt, Some(&mut floor_at));
    }

    /// One frame of the walk as it was before BUG-207's fix: the floor asked once, before the
    /// move, and the frame walked in one step (`update_camera`, which never cuts a frame).
    fn frame_in_one_step(ctl: &mut CameraController, cam: &mut Camera, dt: f32, room: &Room) {
        let p = cam.position;
        if let Some(f) = floor_under(&room.rooms, Some(&room.ship), &room.cars, p, ctl.step_up_from(p.y)) {
            ctl.set_ground_floor(f);
        }
        ctl.update_camera(cam, dt);
    }

    fn how(sprint: bool) -> &'static str {
        if sprint { "sprinting" } else { "walking" }
    }

    /// A walker on the room's floor at `z`, facing up the stairs (+Z), W held (and Shift for a
    /// sprint).
    fn walker(z: f32, sprint: bool) -> (CameraController, Camera) {
        let mut ctl = CameraController::new(WALK_SPEED_MPS, 1.0);
        ctl.set_interior_gravity(G);
        ctl.set_ground_floor(0.0);
        let mut cam = Camera::new();
        cam.position = Vec3::new(10.0, ctl.eye_height(), z);
        cam.yaw = std::f32::consts::PI;
        ctl.apply_action(A::MoveForward, true);
        if sprint {
            ctl.apply_action(A::Sprint, true);
        }
        (ctl, cam)
    }

    /// The metres one frame of `dt` carries a walker (or a sprinter).
    fn stride(dt: f32, sprint: bool) -> f32 {
        WALK_SPEED_MPS * if sprint { SPRINT_FACTOR } else { 1.0 } * dt
    }

    /// Walk from `z` up the stairs and on off the top at `dt` a frame until the landing lib.rs
    /// would take: the height it fell, read back from the landing's speed (h = v^2 / 2g), and
    /// the speed.
    fn off_the_top(dt: f32, sprint: bool, z: f32) -> (f32, f32) {
        let room = room_with_stairs();
        let (mut ctl, mut cam) = walker(z, sprint);
        let mut rested = Vec::new();
        for _ in 0..(10.0 / dt) as usize {
            frame(&mut ctl, &mut cam, dt, &room, &mut rested);
            if let Some(v) = ctl.take_landing() {
                return (drop_height_m(v, G), v);
            }
        }
        panic!("walking off the top of the stairs at {dt} s frames from z = {z} never landed");
    }

    /// Walk from `z` up the stairs and past their top at `dt` a frame: every floor the walk was
    /// given to stand on. Walking up without jumping the body stands on each of them (a higher
    /// floor lifts it the frame it is given), so these are the treads it stood on.
    fn up_the_stairs(dt: f32, sprint: bool, z: f32) -> Vec<f32> {
        let room = room_with_stairs();
        let (mut ctl, mut cam) = walker(z, sprint);
        let mut rested = Vec::new();
        while cam.position.z < 9.0 {
            frame(&mut ctl, &mut cam, dt, &room, &mut rested);
        }
        rested
    }

    /// A SLOW FRAME FALLS OFF THE TOP OF THE STAIRS FROM THE SAME HEIGHT A FAST ONE DOES
    /// (BUG-207). Walking up the 3.0 m stairs and on off the top lands from 3.0 m, so at 7.67
    /// m/s, at 4 ms frames, and from every starting point at 54 ms frames walking and sprinting
    /// and at 100 ms frames (the longest the main loop runs, `dt` is clamped there).
    ///
    /// Seen red 2026-10-20 on the walk before the fix (the floor asked once a frame, before the
    /// move, and the frame walked in one step): "at 4 ms frames the walk fell 3.000 m (7.67 m/s);
    /// 10 slow walks did not: [\"sprinting at 54 ms from z = 2.214: fell 2.786 m (7.39 m/s)\",
    /// ... (5 of the 12 sprints at 54 ms), \"walking at 100 ms from z = 2.000: fell 2.786 m
    /// (7.39 m/s)\", ... (5 of the 12 walks at 100 ms)]". Walking at a steady 54 ms never fell
    /// short: its 0.27 m stride cannot pass over the top tread, which is 0.307 m deep with the
    /// 0.05 m the footprint reaches past its end (`structure::walk_surface`), so the game's 2.79 m
    /// came from a frame of 62 ms or more, or a sprint.
    #[test]
    fn walking_off_the_top_of_the_stairs_falls_the_same_height_at_any_frame_rate() {
        let (reference, v_ref) = off_the_top(0.004, false, 2.0);
        assert!(
            (reference - 3.0).abs() < 0.01,
            "at 4 ms frames the walk fell {reference:.3} m ({v_ref:.2} m/s) off the 3.0 m stairs"
        );
        let mut wrong = Vec::new();
        for (dt, sprint) in [(0.054, false), (0.054, true), (0.1, false)] {
            for k in 0..PHASES {
                let z = 2.0 + stride(dt, sprint) * k as f32 / PHASES as f32;
                let (h, v) = off_the_top(dt, sprint, z);
                if (h - reference).abs() >= 0.01 {
                    wrong.push(format!("{} at {:.0} ms from z = {z:.3}: fell {h:.3} m ({v:.2} m/s)", how(sprint), dt * 1000.0));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "at 4 ms frames the walk fell {reference:.3} m ({v_ref:.2} m/s); {} slow walks did not: {wrong:#?}",
            wrong.len()
        );
    }

    /// A SLOW FRAME WALKS UP EVERY STAIR TREAD (BUG-207). Walking up the 3.0 m stairs at 54 ms
    /// frames (walking and sprinting) and at 100 ms frames, from every starting point, the body
    /// stands on each of the 14 treads, the top one included, as it does at a high frame rate.
    ///
    /// Seen red 2026-10-20 on the walk before the fix: "43 slow walks up the stairs skipped a
    /// tread: [\"walking at 54 ms from z = 2.000: never stood on [\\\"0.857\\\"]\", ... (7 of the
    /// 12 walks at 54 ms, each missing one tread), \"sprinting at 54 ms from z = 2.000: never stood
    /// on [\\\"0.214\\\", \\\"0.643\\\", ... \\\"2.786\\\"]\", ... \"sprinting at 100 ms from z =
    /// 2.000: never stood on [all 14, \\\"0.214\\\" to \\\"3.000\\\"]\", ...]". Sprinting at 100
    /// ms every stride rose more than the 0.6 m step up (`STEP_UP_M`), so the body could not
    /// climb the stairs at all and walked on through them at floor level.
    #[test]
    fn walking_up_the_stairs_at_a_low_frame_rate_stands_on_every_tread() {
        let treads = tread_heights();
        assert_eq!(treads.len(), 14, "the homestead's stairs have 14 steps");
        let mut wrong = Vec::new();
        for (dt, sprint) in [(0.054, false), (0.054, true), (0.1, false), (0.1, true)] {
            for k in 0..PHASES {
                let z = 2.0 + stride(dt, sprint) * k as f32 / PHASES as f32;
                let rested = up_the_stairs(dt, sprint, z);
                let missed: Vec<String> = treads
                    .iter()
                    .filter(|t| !rested.iter().any(|r| (r - **t).abs() < 1e-4))
                    .map(|t| format!("{t:.3}"))
                    .collect();
                if !missed.is_empty() {
                    wrong.push(format!("{} at {:.0} ms from z = {z:.3}: never stood on {missed:?}", how(sprint), dt * 1000.0));
                }
            }
        }
        assert!(wrong.is_empty(), "{} slow walks up the stairs skipped a tread: {wrong:#?}", wrong.len());
    }

    /// AT 60 FRAMES A SECOND THE WALK IS EXACTLY WHAT IT WAS (BUG-207). Walking and sprinting up
    /// the stairs and on off the top, the walk that may cut a frame into substeps puts the body in
    /// the very same place every frame, to the bit, as the walk that never does
    /// (`frame_in_one_step`, the walk as it was before the fix), and hands over the same landing.
    /// At 60 a second a walk moves 0.083 m a frame and a sprint 0.158 m, both under
    /// `WALK_SUBSTEP_M`, so each frame is still one step.
    ///
    /// Seen red 2026-10-20 with WALK_SUBSTEP_M at 0.1 m, which cuts a sprint's frame in two:
    /// "sprinting at 60 frames a second, frame 13: the substepped walk left the body somewhere
    /// else; left: Vec3(10.0, 1.7, 4.2166667), right: Vec3(10.0, 1.7, 4.216666)".
    #[test]
    fn at_sixty_frames_a_second_the_walk_is_unchanged() {
        let dt = 1.0 / 60.0;
        let room = room_with_stairs();
        for sprint in [false, true] {
            let (mut cut, mut cam_cut) = walker(2.0, sprint);
            let (mut whole, mut cam_whole) = walker(2.0, sprint);
            let mut landings = 0;
            for n in 0..240 {
                frame(&mut cut, &mut cam_cut, dt, &room, &mut Vec::new());
                frame_in_one_step(&mut whole, &mut cam_whole, dt, &room);
                assert_eq!(
                    cam_cut.position,
                    cam_whole.position,
                    "{} at 60 frames a second, frame {n}: the substepped walk left the body somewhere else",
                    how(sprint)
                );
                let (a, b) = (cut.take_landing(), whole.take_landing());
                assert_eq!(a, b, "{} at 60 frames a second, frame {n}: a different landing", how(sprint));
                landings += u32::from(a.is_some());
            }
            assert_eq!(landings, 1, "{}: the walk went off the top of the stairs and landed once", how(sprint));
        }
    }

    /// EVERY STAIR TREAD IS DEEPER THAN A WALK SUBSTEP (BUG-207). A step no longer than a tread
    /// cannot pass over one, which is what lets a slow frame stand on every tread; a stair type
    /// with shallower treads would bring the skipped treads back, so this holds the data
    /// (data/blueprints/structure_types.ron, built into the game) to `WALK_SUBSTEP_M`. It counts
    /// the stair types it checked, so it cannot pass by checking none.
    ///
    /// Seen red 2026-10-20 with WALK_SUBSTEP_M at 0.3 m: "the stairs type's treads are 0.257 m
    /// deep, shallower than a 0.3 m walk substep". The same run shows why it matters: at 0.3 m a
    /// 54 ms walk (0.27 m a frame) is one step again, and
    /// `walking_up_the_stairs_at_a_low_frame_rate_stands_on_every_tread` failed with "7 slow
    /// walks up the stairs skipped a tread".
    #[test]
    fn every_stair_tread_is_deeper_than_a_walk_substep() {
        let mut checked = 0;
        for ty in structure::structure_types().iter().filter(|t| t.shape == structure::MeshShape::Steps) {
            let tread = ty.size.2 / ty.steps.max(1) as f32;
            assert!(
                tread > WALK_SUBSTEP_M,
                "the {} type's treads are {tread:.3} m deep, shallower than a {WALK_SUBSTEP_M} m walk substep",
                ty.id
            );
            checked += 1;
        }
        assert!(checked > 0, "no stair type was checked");
    }

    /// EVERY STAIR RISER IS A STEP DOWN (BUG-208). The walk follows a floor that drops no deeper
    /// than `STEP_DOWN_M` and treats a deeper drop as an edge, so a stair type with taller risers
    /// would make walking down it a run of falls again, and a ramp steep enough to drop more than
    /// that over one walk substep would too. Holds data/blueprints/structure_types.ron to it, and
    /// counts what it checked.
    ///
    /// Seen red 2026-10-20 with STEP_DOWN_M at 0.2 m: "walking down the stairs type drops 0.214 m
    /// a step, deeper than the walk's 0.2 m step down" (and in the same run walking down the
    /// stairs fell from the top again, each riser an edge).
    #[test]
    fn every_stair_riser_is_a_step_down() {
        let mut checked = 0;
        for ty in structure::structure_types() {
            let (_, h, d) = ty.size;
            let drop = match ty.shape {
                structure::MeshShape::Steps => h / ty.steps.max(1) as f32,
                structure::MeshShape::Ramp => h / d * WALK_SUBSTEP_M,
                _ => continue,
            };
            assert!(
                drop < STEP_DOWN_M,
                "walking down the {} type drops {drop:.3} m a step, deeper than the walk's {STEP_DOWN_M} m step down",
                ty.id
            );
            checked += 1;
        }
        assert!(checked >= 2, "only {checked} stair and ramp types were checked");
    }

    /// A walker standing on `floor` at `z`, facing down the stairs (-Z), W held (and Shift).
    fn walker_down(z: f32, floor: f32, sprint: bool) -> (CameraController, Camera) {
        let (mut ctl, mut cam) = walker(z, sprint);
        ctl.set_ground_floor(floor);
        cam.position.y = floor + ctl.eye_height();
        cam.yaw = 0.0;
        (ctl, cam)
    }

    /// SPACE MID-FALL DOES NOTHING (BUG-208). Walking on off the top of the 3.0 m stairs, an edge
    /// with nothing beyond it, the body is in the air: Space pressed a few frames into the fall
    /// and held must not launch a jump. The body keeps falling every frame and lands from 3.0 m
    /// (7.67 m/s), at 60 frames a second and at 54 ms frames.
    ///
    /// Seen red 2026-10-20 on the walk before the fix (6cc5ad8c9): "at 17 ms frames, Space held
    /// from 3 frames into the fall lifted the body 0.081 m on frame 0 of the fall". Walking off an
    /// edge never cleared `is_grounded`, so the first Space of the fall launched a full jump.
    #[test]
    fn space_pressed_in_mid_fall_does_not_jump() {
        for dt in [1.0 / 60.0, 0.054] {
            let room = room_with_stairs();
            let (mut ctl, mut cam) = walker(2.0, false);
            let mut rested = Vec::new();
            // Up the stairs until the floor has fallen away under the body past the top.
            let mut n = 0;
            while !(cam.position.z > 7.85 && ctl.ground_floor() == 0.0) {
                frame(&mut ctl, &mut cam, dt, &room, &mut rested);
                n += 1;
                assert!(n < 1000, "never walked off the top of the stairs");
            }
            for _ in 0..3 {
                frame(&mut ctl, &mut cam, dt, &room, &mut rested);
            }
            ctl.apply_action(A::Jump, true);
            let mut landing = None;
            for k in 0..(5.0 / dt) as usize {
                let before = cam.position.y;
                frame(&mut ctl, &mut cam, dt, &room, &mut rested);
                landing = landing.or(ctl.take_landing());
                if landing.is_some() {
                    break;
                }
                assert!(
                    cam.position.y < before,
                    "at {:.0} ms frames, Space held from 3 frames into the fall lifted the body {:.3} m on frame {k} of the fall",
                    dt * 1000.0,
                    cam.position.y - before
                );
            }
            let v = landing.expect("the fall lands");
            let h = drop_height_m(v, G);
            assert!((h - 3.0).abs() < 0.01, "at {:.0} ms frames the fall landed from {h:.3} m ({v:.2} m/s), not 3.0 m", dt * 1000.0);
        }
    }

    /// WALKING DOWN THE STAIRS IS NOT A FALL (BUG-208). Down the 3.0 m stairs from the top
    /// tread, walking and sprinting, at 60 frames a second and at 54 and 100 ms frames, the body
    /// steps down each tread: its feet are on its floor at the end of every frame, and nothing
    /// lands hard enough to count (`falls::LANDING_M_S`).
    ///
    /// Seen red 2026-10-20 on the walk before the fix (6cc5ad8c9): "walking down the stairs left
    /// the floor: [\"walking at 17 ms: feet up to 0.972 m above the floor, landing
    /// Some(7.672027)\", \"sprinting at 17 ms: feet up to 2.248 m above the floor, landing None\",
    /// \"walking at 54 ms: feet up to 0.963 m above the floor, landing Some(7.672027)\", ...
    /// \"walking at 100 ms: ... landing Some(7.672027)\", ...]". Nothing followed the floor down:
    /// at 5 m/s a body leaving the top tread flew clear of the whole flight and landed at the
    /// bottom from 3.0 m (a sprint flew on past the stairs' foot before landing).
    #[test]
    fn walking_down_the_stairs_is_not_a_fall() {
        let room = room_with_stairs();
        let mut wrong = Vec::new();
        for dt in [1.0 / 60.0, 0.054, 0.1] {
            for sprint in [false, true] {
                let (mut ctl, mut cam) = walker_down(7.7, 3.0, sprint);
                let mut rested = Vec::new();
                let mut worst_above = 0.0_f32;
                let mut landing = None;
                while cam.position.z > 3.0 {
                    frame(&mut ctl, &mut cam, dt, &room, &mut rested);
                    worst_above = worst_above.max(cam.position.y - ctl.eye_height() - ctl.ground_floor());
                    landing = landing.or(ctl.take_landing());
                }
                if worst_above > 1e-4 || landing.is_some() {
                    wrong.push(format!(
                        "{} at {:.0} ms: feet up to {worst_above:.3} m above the floor, landing {landing:?}",
                        how(sprint),
                        dt * 1000.0
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "walking down the stairs left the floor: {wrong:#?}");
    }

    /// A LONG FRAME DOES NOT STEP ONTO A DECK BEHIND A WALL (BUG-207). The walls are swept after
    /// the whole frame's walk, so a substep's point can be part way through one; the floor is read
    /// where that sweep holds the body, on this side of the wall.
    ///
    /// Seen red 2026-10-20 with `floor_along` reading the floor at the point itself: "a substep
    /// through the wall stood the body on the deck behind it; left: Some(0.2), right: Some(0.0)".
    #[test]
    fn a_substep_reads_the_floor_where_the_walls_hold_the_body() {
        let zone: ShipZone = ron::from_str(
            r#"(id: "home", body: (width: 20.0, depth: 20.0, height: 6.0,
                structures: [(id: "deck-1", type_id: "deck", pos: (10.0, 0.0, 12.0))]))"#,
        )
        .expect("the test zone parses");
        let mut room = room_with_stairs();
        room.ship = ShipStructure { zones: vec![zone], ..Default::default() };
        // A wall across the room at z = 10, and a 0.2 m deck (4 m square, z 10 to 14) against
        // its far side. The frame began 0.6 m short of the wall; a substep's point is past it.
        let wall = WallSegment { a: (0.0, 10.0), b: (20.0, 10.0), half_thickness: 0.075 };
        let (start, through) = (Vec3::new(10.0, 1.7, 9.4), Vec3::new(10.0, 1.7, 10.4));
        let raw = floor_under(&room.rooms, Some(&room.ship), &room.cars, through, 0.0);
        assert_eq!(raw, Some(0.2), "the deck is under the point past the wall");
        let held = floor_along(&room.rooms, Some(&room.ship), &room.cars, &[wall], start, through, 0.0);
        assert_eq!(held, Some(0.0), "a substep through the wall stood the body on the deck behind it");
    }
}

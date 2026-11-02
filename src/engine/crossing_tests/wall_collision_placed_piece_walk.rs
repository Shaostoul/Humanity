//! From ship/wall_collision.rs (`placed_piece_walk_tests`): walking on and under placed pieces with the
//! engine's footing (`footing::test_aboard`) and the renderer's head height.

//! Placed solid pieces walked with the whole frame (the increment 7 follow-ups, 2026-10-21): the
//! footing, the walk's substeps and the sweep, in lib.rs's order (`engine::footing::test_aboard`).

use crate::ship::wall_collision::*;
use crate::engine::footing::test_aboard::{body_at, live_frame, Aboard, EAST, NORTH};
use humanity_core::input::bindings::GameAction as A;
use crate::renderer::camera::HEAD_ABOVE_EYE_M;
use crate::ship::ship_structure::{ShipStructure, ShipZone};
use glam::Vec3;

const EYE: f32 = EYE_HEIGHT_M;

/// A 12 x 12 x 6 m home with the placed pieces `structures` (RON), the only zone of a ship.
fn aboard(structures: &str) -> Aboard {
    let zone: ShipZone = ron::from_str(&format!(
        "(id: \"home\", purpose: \"residence\", body: (width: 12.0, depth: 12.0, height: 6.0, structures: [{structures}]))"
    ))
    .expect("the test zone parses");
    Aboard::of(ShipStructure { zones: vec![zone], ..Default::default() })
}

/// A STAIR LEADS ONTO A DECK AT ITS TOP. A stair climbing east from x 2.75 to 7.25 (its sides z
/// 5.4 and 6.6) and a deck at its top, at 3.0 (x 7.25 to 11.25, z 4 to 8, its top 3.2): walking
/// east from the stair's foot, the body climbs the flight and steps onto the deck, 0.2 m up
/// from the top step; the deck's edge, a step up from there, does not stop it.
///
/// Passes before the deck had walls and after (it guards the step onto a deck against them).
#[test]
fn a_stair_leads_onto_a_deck_at_its_top() {
    let mut w = aboard(
        "(id: \"stairs-1\", type_id: \"stairs\", pos: (5.0, 0.0, 6.0), rot_deg: 90.0), \
         (id: \"deck-1\", type_id: \"deck\", pos: (9.25, 3.0, 6.0))",
    );
    let (mut ctl, mut cam) = body_at(Vec3::new(2.0, EYE, 6.0));
    cam.yaw = EAST;
    ctl.apply_action(A::MoveForward, true);
    for _ in 0..90 {
        live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
        if cam.position.x > 9.0 {
            break;
        }
    }
    let feet = cam.position.y - ctl.eye_height();
    assert!(cam.position.x > 9.0 && (feet - 3.2).abs() < 1e-3, "the walk up the stair ended at {:?}, feet {feet}, not on the deck at its top", cam.position);
}

/// A JUMP UNDER A RAISED DECK BUMPS ITS HEAD AND COMES DOWN UNDER IT. A deck at 2.0 (x 4 to 8,
/// z 4 to 8, its underside 2.0): a body standing under it walks there (its eye, 1.7, is under
/// the deck), and a jump stops with the head at the deck's underside, as under a floor slab
/// (`engine::footing::Footing::ceiling_over`), then lands where it jumped, not shoved out from
/// under the deck by its sides nor up through it.
///
/// Seen red first (2026-10-21) with no placed piece a ceiling or a wall: "frame 2: the jump
/// under the deck put the head at 2.054 m, through its underside at 2.0, the body at
/// Vec3(6.0, 1.9336501, 6.0)".
#[test]
fn a_jump_under_a_raised_deck_bumps_its_head_and_comes_down_under_it() {
    let mut w = aboard("(id: \"deck-1\", type_id: \"deck\", pos: (6.0, 2.0, 6.0))");
    let (mut ctl, mut cam) = body_at(Vec3::new(6.0, EYE, 6.0));
    ctl.apply_action(A::Jump, true);
    for k in 0..90 {
        live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
        ctl.apply_action(A::Jump, false);
        let head = cam.position.y + HEAD_ABOVE_EYE_M;
        assert!(head < 2.0 + 1e-3, "frame {k}: the jump under the deck put the head at {head:.3} m, through its underside at 2.0, the body at {:?}", cam.position);
        assert!((cam.position.x - 6.0).abs() < 1e-3 && (cam.position.z - 6.0).abs() < 1e-3, "frame {k}: the jump under the deck moved the body to {:?}", cam.position);
    }
    let feet = cam.position.y - ctl.eye_height();
    assert!(feet.abs() < 1e-3, "after the jump under the deck the feet are at {feet}");
}

/// A RUNNING JUMP ONTO A PLATFORM TOO HIGH TO STEP ONTO LANDS BESIDE IT. A train platform
/// raised to 0.6 (3 x 8 m, x 4.5 to 7.5, z 1 to 9, its top 1.0, more than a step over the
/// floor): running north at its end and jumping, the feet clear its edge (a jump lifts them
/// 1.27 m), but the footing does not stand a body on a top more than a step over the floor it
/// jumped from, so it comes down inside the platform's footprint; there the platform's core
/// settles it out of its nearest side, and it ends on the floor beside it, not inside it.
///
/// Seen red first (2026-10-21) with only stairs, ramps and guards in the walls: "after the
/// running jump at the platform the body is at Vec3(6.0, 1.7, 6.166671), feet 0.00, inside
/// it".
#[test]
fn a_running_jump_onto_a_platform_too_high_to_step_onto_lands_beside_it() {
    let mut w = aboard("(id: \"train-1\", type_id: \"train\", pos: (6.0, 0.6, 5.0))");
    let (mut ctl, mut cam) = body_at(Vec3::new(6.0, EYE, 10.0));
    cam.yaw = NORTH;
    ctl.apply_action(A::MoveForward, true);
    ctl.apply_action(A::Jump, true);
    let inside = |p: Vec3| p.x > 4.5 - PLAYER_RADIUS + 0.01 && p.x < 7.5 + PLAYER_RADIUS - 0.01 && p.z > 1.0 - PLAYER_RADIUS + 0.01 && p.z < 9.0 + PLAYER_RADIUS - 0.01;
    for k in 0..120 {
        live_frame(&mut ctl, &mut cam, 1.0 / 60.0, &mut w);
        ctl.apply_action(A::Jump, false);
        if k == 45 {
            ctl.apply_action(A::MoveForward, false);
        }
    }
    let feet = cam.position.y - ctl.eye_height();
    assert!(!inside(cam.position) && feet.abs() < 1e-3, "after the running jump at the platform the body is at {:?}, feet {feet:.2}, inside it", cam.position);
}

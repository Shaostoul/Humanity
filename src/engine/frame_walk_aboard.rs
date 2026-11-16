//! Walking aboard (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as it was: the elevators' tick, then the player's first-person movement aboard
//! the ship and the home, with the camera's position before it kept for later stages.

use crate::engine::frame_prelude::*;

/// Runs the elevators and moves the player aboard; records the camera's position before the move.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = state.clock.dt;
    humanity_core::frame_costs::span("cpu.span.walk_aboard");
    // ELEVATOR tick (v0.590; ship homes increment 7, engine/footing.rs
    // `elevator_tick`): each car runs toward the stop it is heading for; its stops
    // are the storey floors its shaft spans. A rider sends it up a stop with Space
    // and down one with Shift (and Space does not jump while riding), and a body at
    // any landing calls it there. Runtime state lives in `elevator_state` keyed by
    // (zone id, elevator id) (not saved). First person, not build. Every
    // ZONE's elevators run, at world positions (v0.754). The footing sampler below
    // reads the updated car height so the rider rides.
    // A granted interaction (character animation rung 2b, engine/interact.rs): its
    // phase moves on and the capsule is held where it puts the body (seated, lying),
    // before the walk runs.
    crate::engine::interact::tick(state, dt);
    let lift_press = state.controller.take_lift_press();
    // First or third person: both walk the body (character animation rung 1).
    if state.camera.mode.walks_body()
        && !state.gui_state.construction_active
        // v0.881: home elevators only exist where the home is.
        && state.aboard_station
    {
        let p = state.camera.position;
        let feet = Vec3::new(p.x, p.y - state.controller.eye_height(), p.z);
        let riding = match state.gui_state.ship_structure.as_ref() {
            Some(ship) => crate::engine::footing::elevator_tick(ship, &mut state.elevator_state, feet, lift_press, dt),
            None => false,
        };
        state.controller.set_on_lift(riding);
    } else {
        state.controller.set_on_lift(false);
        // Out of first person (another camera, the build editor, off the ship):
        // nobody rides and every car stays where it is, so a rider who changes
        // camera at the top is still on the car when they come back (it used to
        // reset every car to the bottom: a 3 m fall on return).
        crate::engine::footing::pause_elevators(&mut state.elevator_state);
    }

    // Keep the player grounded on the floor of whatever room they are in
    // (so gravity + jump land on the right deck), raised to the stair tread,
    // ramp, platform or elevator car under them that they can step onto
    // (engine::footing: a STEP_UP cap means a tall solid box can't yank you up
    // its side, you use the stairs). Falls back to the last floor when outside
    // every room. The walk asks the same again between a long frame's
    // substeps (BUG-207, `update_camera_with_floor` below).
    // v0.881: only ABOARD - on a planet the surface branch owns
    // footing, and in free space there is no home floor below.
    // Ship homes increment 7: found by height (slabs, built pieces), the ceiling
    // a jump stops under with it, and measured from the feet when something else
    // put the body somewhere since the walk left it (a placement).
    let built_aboard = if state.aboard_station {
        crate::engine::build_place::built_piece_boxes(
            &state.game_world.world,
            state.data_store.get::<humanity_systems::construction::BlueprintRegistry>("blueprint_registry"),
            None,
        )
    } else {
        Vec::new()
    };
    if state.aboard_station {
        let p = state.camera.position;
        let footing = crate::engine::footing::Footing {
            rooms: &state.gui_state.room_bounds,
            ship: state.gui_state.ship_structure.as_ref(),
            cars: &state.elevator_state,
            built: &built_aboard,
        };
        let step_from = if state.controller.placed_since_walk(p.y) {
            p.y - state.controller.eye_height()
        } else {
            state.controller.step_up_from(p.y)
        };
        if let Some(floor) = footing.floor_under(p, step_from) {
            state.controller.set_ground_floor(floor);
        }
        state.controller.set_ceiling(footing.ceiling_over(p));
    } else {
        state.controller.set_ceiling(None);
    }

    // Teleporter pads (v0.584): stepping onto a teleporter that has a linked pair jumps
    // the player to the partner pad, and re-arms only once they have stepped off every
    // pad (engine/move_check.rs `teleporter_tick`, ship homes increment 4).
    crate::engine::move_check::teleporter_tick(state, dt);

    // Ladder CLIMB zone (v0.589): if the player stands at a ladder, tell the controller
    // its span so an up/down input climbs it (instead of jumping/falling). First person,
    // not build. Proximity (XZ within the ladder footprint + a reach margin) is the
    // trigger -- you do not have to be pixel-perfect on the rungs. Third person too
    // since character animation rung 1: it walks the same body.
    if state.camera.mode.walks_body()
        && !state.gui_state.construction_active
    {
        // v0.754: EVERY zone's ladders, at world positions; since ship homes
        // increment 7 the one whose span holds the feet (engine/footing.rs).
        let p = state.camera.position;
        let zone = state.gui_state.ship_structure.as_ref().and_then(|ship| crate::engine::footing::ladder_at(ship, p));
        state.controller.set_climb_zone(zone);
    } else {
        state.controller.set_climb_zone(None);
    }

    // Update camera from input (capture the pre-move position for swept collision).
    let prev_cam_pos = state.camera.position;
    // v0.962: tell the controller whether the frame-lock owns
    // translation this frame (last frame's flag - bands change
    // over km, one frame of lag is nothing). In the blend band
    // surface_mode is on but nothing owns translation, and the
    // controller used to freeze all below-FTL-cap movement.
    state.controller.surface_translation_owned = state.surface_owns_translation;
    if state.aboard_station {
        // The floor (and the ceiling) between a long frame's substeps (BUG-207):
        // the same footing as above, read where the walls and the shut gates will
        // have held the body (`footing::Footing::along`), with the walls only
        // where the sweep below applies them (not in the build editor).
        let walls: &[humanity_ship::wall_collision::WallSegment] =
            if state.gui_state.construction_active { &[] } else { &state.wall_colliders[..] };
        let gates = if state.gui_state.construction_active {
            Vec::new()
        } else {
            crate::engine::footing::shut_gates(state.gui_state.ship_structure.as_ref(), &state.elevator_state, state.controller.on_ladder(prev_cam_pos.y))
        };
        let footing = crate::engine::footing::Footing {
            rooms: &state.gui_state.room_bounds,
            ship: state.gui_state.ship_structure.as_ref(),
            cars: &state.elevator_state,
            built: &built_aboard,
        };
        let mut floor_at = |at: Vec3, step_from: f32| footing.along(walls, &gates, prev_cam_pos, at, step_from);
        state.controller.update_camera_with_floor(&mut state.camera, dt, Some(&mut floor_at));
    } else {
        state.controller.update_camera(&mut state.camera, dt);
    }

    // Skills rung 1 (data/skills/practice.json): metres climbed on a ladder are climbing practice,
    // and metres walked carrying at least a quarter of what you can carry are logistics practice.
    let moved = state.camera.position - prev_cam_pos;
    if moved.y > 0.0 && state.controller.in_climb_zone() {
        humanity_systems::skills::practice_distance(&state.data_store, "climb", moved.y, None);
    }
    let carry = &state.gui_state.carry.input;
    if carry.capacity_kg > 0.0 && carry.carried_kg >= 0.25 * carry.capacity_kg {
        let walked = glam::Vec2::new(moved.x, moved.z).length();
        humanity_systems::skills::practice_distance(&state.data_store, "carry_load", walked, None);
    }

    f.prev_cam_pos = prev_cam_pos;
}

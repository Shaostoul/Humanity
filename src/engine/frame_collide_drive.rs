//! Collision and driving (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as they were: pushing the player out of the home's walls, driving a vehicle, and
//! the follow camera that chases a self-driving one.

use crate::engine::frame_prelude::*;

/// Resolves wall collision, then drives the vehicle or follows it.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = f.dt;
    let prev_cam_pos = f.prev_cam_pos;
    humanity_core::frame_costs::span("cpu.span.collide_drive");
    // Wall collision (v0.556): the player IS the camera, so push it out of the home's
    // walls / closed doors in first person -- no more walking through walls. Doors that
    // are open + unlocked are gaps; closed or locked doors block; windows are part of
    // their wall span. Skipped in the build editor (orbit cam) + when there are no
    // home walls (legacy layout / showroom -> wall_colliders empty).
    // The doors shut this frame, the one list the walk sweeps against and E and F's
    // reach reads (engine/built_uses.rs `refresh_shut_doors`, `reach_blockers`).
    crate::engine::built_uses::refresh_shut_doors(state);
    // First or third person: both walk the body, so the walls hold it in both
    // (character animation rung 1).
    if state.camera.mode.walks_body()
        && !state.gui_state.construction_active
        // Dev fly mode is noclip (v0.791.x): walls must not pin
        // the traveler inside the hull when they fly out.
        && !state.controller.fly_mode
        && !state.wall_colliders.is_empty()
        // v0.881: the walls are wherever the home is.
        && state.aboard_station
    {
        // LIVE door segments, rebuilt per frame from the animated open fractions
        // (`state.shut_doors`, refreshed above: a door blocks only when closed or
        // locked, v0.570; windows are handled by their uncut wall span; each from
        // its sill to its head, so a door shut on one storey is no wall on another).
        // This deliberately stays SEPARATE from `state.wall_colliders` (which only
        // rebuilds on structure edits): a door's blocking state changes every frame
        // as it animates, and rebuilding the whole ship's static segment list per
        // frame just to toggle a few doors would be pure waste. `resolve` already
        // takes the two lists side by side for exactly this split. Corridor-mouth
        // door pairs (v0.795) ride this same path: while closed, the two halves
        // together span the full aperture (each contributes its own half-width
        // segment), so a shut corridor door is impassable; open, both drop out and
        // the mouth is the walk-through gap the shell cut left.
        let mut doors = state.shut_doors.clone();
        // The elevators' landing doors and the ladder hatches' gates (ship homes
        // increment 7, engine/footing.rs `shut_gates`).
        doors.extend(crate::engine::footing::shut_gates(
            state.gui_state.ship_structure.as_ref(),
            &state.elevator_state,
            state.controller.on_ladder(state.camera.position.y),
        ));
        // Built pieces are solid too (2026-09-28, build_place::built_piece_segments).
        doors.extend(crate::engine::build_place::built_piece_segments(
            &state.game_world.world,
            state.data_store.get::<humanity_systems::construction::BlueprintRegistry>("blueprint_registry"),
            None,
            state.camera.position,
            state.controller.eye_height(),
        ));
        // The furniture too, the crowd's own footprints (2026-11-13, the operator:
        // "We want the ship to feel alive"): a table or a bed stops the body, a rug
        // under the knee does not, and a seat or a bed the player is sitting or
        // lying on is not in the way (engine/crowd_walk.rs `furniture_walls_near`).
        crate::engine::crowd_walk::refresh_furniture(state);
        doors.extend(crate::engine::crowd_walk::furniture_walls_near(
            state.animation.walk.furniture(),
            state.camera.position,
            state.interact.me.is_some(),
        ));
        let resolved = humanity_ship::wall_collision::resolve(
            prev_cam_pos,
            state.camera.position,
            humanity_ship::wall_collision::PLAYER_RADIUS,
            &state.wall_colliders,
            &doors,
        );
        state.camera.position = resolved;
    }
    // ── DRIVING (Stage 3 take-over, v0.690) ──
    // While driving: WASD throttles/steers the VEHICLE (arcade: it
    // goes where you look), the camera rides the cab, and the ECS
    // player Transform travels along so saves + systems see the
    // player where the vehicle is. Runs AFTER the walk/collision
    // resolve above and simply overrides the camera -- walking
    // input becomes throttle input for free via InputState.
    if let Some(veh) = state.driving_vehicle {
        if !state.game_world.world.contains(veh) {
            state.driving_vehicle = None; // despawned under us
        } else {
            // Driver wins over any self-drive route re-attached mid-drive.
            let _ = state
                .game_world
                .world
                .remove_one::<humanity_core::ecs::components::VehicleRoute>(veh);
            let (mut vpos, item_id) = {
                let t = state
                    .game_world
                    .world
                    .get::<&humanity_core::ecs::components::Transform>(veh)
                    .map(|t| t.position)
                    .unwrap_or(state.camera.position);
                let id = state
                    .game_world
                    .world
                    .get::<&humanity_core::ecs::components::Vehicle>(veh)
                    .map(|v| v.item_id.clone())
                    .unwrap_or_default();
                (t, id)
            };
            let def = state
                .data_store
                .get::<humanity_systems::vehicles::VehicleKitRegistry>("vehicle_kit_registry")
                .and_then(|r| r.get_vehicle(&item_id).cloned());
            let speed = def.as_ref().map(|d| d.speed_mps).unwrap_or(6.0);
            let cab_h = def
                .as_ref()
                .map(|d| d.wheel_radius_m + d.body_m.1 + d.cabin_m.1 + 0.35)
                .unwrap_or(2.2);
            let input = state
                .data_store
                .get::<InputState>("input_state")
                .cloned()
                .unwrap_or_default();
            let throttle = if input.forward {
                1.0
            } else if input.backward {
                -0.45
            } else {
                0.0
            };
            // A/D steer the wheel (v0.697, operator request):
            // they turn the shared yaw, so keyboard and mouse
            // steering work simultaneously -- the mouse looks,
            // A/D turn, both move the same heading. D = right =
            // +yaw (matches mouse-right). A dedicated steering-
            // mode setting (mouse-only / keys-only) is queued.
            let steer = (input.right as i32 - input.left as i32) as f32;
            if steer != 0.0 {
                state.camera.yaw += steer * 1.6 * dt;
            }
            // v0.697 fix: use the camera's OWN forward -- the old
            // recomputed (-sin, -cos) was X-mirrored, so driving
            // east/west went the wrong way while north/south was
            // fine (operator field report).
            let fwd = state.camera.forward_xz();
            if throttle != 0.0 {
                vpos += fwd * speed * throttle * dt;
                // Skills rung 1: distance driven is driving practice (data/skills/practice.json).
                humanity_systems::skills::practice_distance(&state.data_store, "drive", (speed * throttle * dt).abs(), None);
            }
            // Body long axis is +X: derive the body yaw from the
            // actual forward vector (same formula the transit
            // route tick uses), never from yaw-plus-offset math.
            let vrot = Quat::from_rotation_y((-fwd.z).atan2(fwd.x));
            if let Ok(mut t) = state
                .game_world
                .world
                .get::<&mut humanity_core::ecs::components::Transform>(veh)
            {
                t.position = vpos;
                t.rotation = vrot;
            }
            // Camera rides the cab; player entity travels along.
            state.camera.position = vpos + Vec3::Y * cab_h;
            for (_e, (tf, _c)) in state.game_world.world.query_mut::<(
                &mut humanity_core::ecs::components::Transform,
                &humanity_core::ecs::components::Controllable,
            )>() {
                tf.position = vpos;
                break;
            }
        }
    }
    // ── FOLLOW CAM (Stage 3, v0.690): chase a self-driving vehicle.
    // Engaged from the Vehicles section; broken by any WASD input
    // or by the vehicle arriving (route removed). The camera hangs
    // behind-above the vehicle and turns to face it; mouse pitch
    // stays yours.
    if let Some(bits) = state.gui_state.pending_follow_vehicle.take() {
        state.follow_vehicle = hecs::Entity::from_bits(bits);
        // Watching, not walking: close the page so the world shows. In the shared
        // world the body stays here, and so does what we report (engine/move_check.rs
        // `body_position`, ship homes increment 4).
        state.gui_state.enter_world();
        if state.game_joined && state.moves.follow_body.is_none() {
            state.moves.follow_body = Some(state.camera.position);
        }
    }
    if let Some(veh) = state.follow_vehicle {
        let input = state
            .data_store
            .get::<InputState>("input_state")
            .cloned()
            .unwrap_or_default();
        let moved = input.forward || input.backward || input.left || input.right;
        let still_routed = state
            .game_world
            .world
            .get::<&humanity_core::ecs::components::VehicleRoute>(veh)
            .is_ok();
        if moved || !still_routed || state.driving_vehicle.is_some() {
            state.follow_vehicle = None;
        } else if let Ok(t) = state
            .game_world
            .world
            .get::<&humanity_core::ecs::components::Transform>(veh)
        {
            let vpos = t.position;
            // Hang behind the direction of travel (vehicle +X).
            let back = t.rotation * Vec3::new(-9.0, 0.0, 0.0);
            state.camera.position = vpos + back + Vec3::Y * 4.0;
            let dir = (vpos - state.camera.position).normalize_or_zero();
            if dir.length_squared() > 0.0 {
                state.camera.yaw = (-dir.x).atan2(-dir.z);
            }
        } else {
            state.follow_vehicle = None;
        }
    }



}

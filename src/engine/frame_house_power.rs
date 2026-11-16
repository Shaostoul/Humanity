//! The home's power and world-scale flight (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: switched-on lights drawing their wattage through one
//! power consumer, and the dev FTL flight's share in Earth-centred metres.

use crate::engine::frame_prelude::*;

/// Updates the lights' power draw and the world-scale flight offset for this frame.
pub(crate) fn run(state: &mut EngineState) {
    let dt = state.clock.dt;
    humanity_core::frame_costs::span("cpu.span.house_power");
    // ── House lighting on the power meter (v0.967, homestead
    // increment 5): every switched-on placed light draws its
    // light_types.ron wattage through ONE aggregate
    // PowerConsumer entity (island 0, the home machines'
    // default) - flipping a light switch changes the bill. ──
    {
        let watts = state
            .gui_state
            .ship_structure
            .as_ref()
            .map(|s| {
                s.lighting_watts(|id| {
                    crate::renderer::light::light_type(id)
                        .map(|t| t.watts)
                        .unwrap_or(0.0)
                })
            })
            .unwrap_or(0.0);
        match state.house_light_consumer {
            Some(e) => {
                if let Ok(mut c) = state
                    .game_world
                    .world
                    .get::<&mut humanity_core::ecs::components::PowerConsumer>(e)
                {
                    c.draw_watts = watts;
                }
            }
            None => {
                if watts > 0.0 {
                    let e = state.game_world.world.spawn((
                        humanity_core::ecs::components::PowerConsumer {
                            draw_watts: watts,
                            priority: 3,
                            enabled: true,
                        },
                        humanity_core::ecs::components::PowerCircuit { island: 0 },
                    ));
                    state.house_light_consumer = Some(e);
                }
            }
        }
    }

    // Dev FTL flight, world-scale share (v0.791.x): above
    // LOCAL_FLY_MULT_MAX one frame of travel reaches km-to-
    // planetary scale, far beyond what the f32 local camera
    // position can hold without jitter (f32 ulp at 1.5e11 m is
    // ~16 km). So the SHIP flies instead: ship_world_pos is f64
    // Earth-centred metres and the celestial pass subtracts it,
    // so moving it carries the whole local frame (homestead +
    // player) through the solar system at full precision while
    // the camera stays at metre-scale coordinates. Below the
    // cap the controller already moved the local camera and
    // this block is a no-op.
    if state.controller.fly_mode
        && state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson
        && state.controller.fly_speed_mult
            > crate::renderer::camera::LOCAL_FLY_MULT_MAX
        // Never let the world-scale FTL integration run while
        // the SURFACE branch owns translation (2026-07-12):
        // otherwise the one wheel notch past the cap turns a
        // tiny W/Space tap into an 80-800 km/frame launch.
        // v0.872: gate on the translation-ownership flag, NOT
        // camera.surface_mode - surface_mode now stays on
        // through the 100-1000 km up-blend band where the FTL
        // wheel MUST keep working (operator: "my movement stops
        // adjusting based on my mouse wheel warp speed").
        && !state.surface_owns_translation
    {
        // v0.962 liftoff fix: Space thrusts along the camera's
        // EASED up while surface_mode rides the blend band
        // (radial at 100 km -> world Y at 1000 km). Raw world
        // +Y at southern latitudes points sideways-and-down
        // relative to the ground - the operator's "can't leave
        // the planet perpendicularly": lateral zoom at fixed
        // altitude until the equator flipped sin(lat)'s sign.
        let up_axis = if state.camera.surface_mode {
            state.camera.up
        } else {
            Vec3::Y
        };
        let wish = state.controller.fly_wish_dir_up(&state.camera, up_axis);
        if wish.length_squared() > 0.0 {
            // First world-scale departure stashes home so the
            // Dev page's "Return home" can restore it.
            if state.dev_travel_home.is_none() {
                state.dev_travel_home = Some((
                    state.ship_world_pos,
                    state.camera.position,
                    state.camera.yaw,
                    state.camera.pitch,
                ));
                state.gui_state.dev_travel_away = true;
            }
            let step = (state.controller.speed as f64)
                * (state.controller.fly_speed_mult as f64)
                * (dt as f64);
            // ── Approach governor (v0.879, operator: "I can no
            // longer make it to the surface... fast, slow,
            // fast, slow") ──
            // At high FTL gear one frame can cover hundreds of
            // km, so the 100 km co-rotate band is impossible to
            // ENTER by hand: you overshoot it whole
            // planet-diameters per frame, and each band exit
            // resumes FTL at the stale gear (the catapult half
            // of the oscillation). Cap the per-frame world step
            // to HALF the distance to the nearest landable
            // surface: closing on a planet becomes a smooth
            // exponential approach that lands you in the band
            // in a couple of frames no matter the gear, while
            // deep space (huge alt) never notices the cap.
            let cam_world = state.ship_world_pos
                + glam::DVec3::new(
                    state.camera.position.x as f64,
                    state.camera.position.y as f64,
                    state.camera.position.z as f64,
                );
            let mut min_alt = f64::MAX;
            // Earth is the frame origin and the primary
            // landable body; the locked body (if any, e.g.
            // after a travel jump) governs equally.
            let earth_r = state
                .planet_defs
                .get("earth")
                .map(|d| d.radius)
                .unwrap_or(6.371e6);
            min_alt = min_alt.min(cam_world.length() - earth_r);
            if let Some(lock) = &state.frame_lock_body {
                if lock != "earth" {
                    if let Some(d) = state.planet_defs.get(lock) {
                        // Locked-body centre: same lookup the
                        // frame-lock section performs below.
                        let sim_t = state.clock.sim_epoch_s; // the frame's clock (frame_sim::FrameClock)
                        let earth_helio = humanity_core::cosmos::find_body("earth")
                            .map(|e| {
                                humanity_core::cosmos::body_world_position_3d_au(e, sim_t)
                            })
                            .unwrap_or(glam::DVec3::ZERO);
                        if let Some(b) = humanity_core::cosmos::find_body(lock) {
                            let c = (humanity_core::cosmos::body_world_position_3d_au(
                                b, sim_t,
                            ) - earth_helio)
                                * humanity_core::cosmos::M_PER_AU;
                            min_alt =
                                min_alt.min((cam_world - c).length() - d.radius);
                        }
                    }
                }
            }
            let governed = if min_alt.is_finite() && min_alt > 0.0 {
                step.min((min_alt * 0.5).max(1_000.0))
            } else {
                step
            };
            state.ship_world_pos +=
                glam::DVec3::new(wish.x as f64, wish.y as f64, wish.z as f64)
                    * governed;
        }
    }

}

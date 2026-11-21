//! Ambient particles (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as they were: leaves among trees, dust, dive bubbles. Drawing, so it runs after the
//! simulation's step (the headless sim runner, increment 4: the probe hold that shared this file
//! is frame_probe_hold.rs, a step stage).

use crate::engine::frame_prelude::*;

/// Runs this frame's ambient particles.
pub(crate) fn run(state: &mut EngineState) {
    // Nothing to do before the world's renderer is built (a pages-only start, engine/renderer_slot.rs): the particles are drawn only in the world.
    if !gpu_ref!(state).renderer.world_built() {
        return;
    }
    let dt = state.clock.dt;
    humanity_core::frame_costs::span("cpu.span.particles");
    // ── Ambient particles (v0.966): leaves among trees, dust
    // in the void ── tick + emitter management. Particles
    // live in RENDER space, so each frame they ride the
    // floating-origin rebase (shift by -delta ship pos); an
    // FTL-scale jump just clears them.
    {
        let delta = state.prev_ship_world_pos - state.ship_world_pos;
        let d32 = Vec3::new(delta.x as f32, delta.y as f32, delta.z as f32);
        if d32.length() > 2000.0 {
            state.particle_system.clear();
        } else if d32.length_squared() > 0.0 {
            state.particle_system.shift_all(d32);
        }
        state.prev_ship_world_pos = state.ship_world_pos;
        // Leaves drift while standing among real trees.
        let want_leaves =
            state.camera.surface_mode && !state.near_trees.is_empty();
        let leaf_pos = state.camera.position + state.camera.up * 6.0;
        match state.particle_system.emitter_by_type_mut("leaf_drift") {
            Some(e) => {
                e.active = want_leaves;
                if want_leaves {
                    e.position = leaf_pos;
                }
            }
            None => {
                if want_leaves {
                    let _ = state.particle_system.spawn("leaf_drift", leaf_pos);
                }
            }
        }
        // Dive bubbles (v0.984): a rising trickle around the
        // submerged camera - the same manage-an-ambient-
        // emitter pattern as the leaves.
        let want_bubbles = state.gui_state.underwater;
        let bubble_pos =
            state.camera.position + state.camera.forward() * 1.5;
        match state.particle_system.emitter_by_type_mut("dive_bubbles") {
            Some(e) => {
                e.active = want_bubbles;
                if want_bubbles {
                    e.position = bubble_pos;
                }
            }
            None => {
                if want_bubbles {
                    let _ = state
                        .particle_system
                        .spawn("dive_bubbles", bubble_pos);
                }
            }
        }
        // Space dust: a sparse world-anchored mote field ahead
        // of the camera so motion reads against black space.
        let want_dust =
            !state.camera.surface_mode && state.controller.fly_mode;
        // ── ONLY WHILE YOU FLY (operator, 2026-09-24) ──
        //
        // "The weird glittering effect I'm seeing even when I'm not
        // looking at the planet. Kind of like stars blinking but,
        // they end up phasing out entirely and some times are very
        // wide compared to the other stars ... they also seem to
        // drift around a bit before they fade out."
        //
        // That was this field. It ran whenever he was in fly mode
        // in space, moving or not, and each mote lives 1.5-3.5 s and
        // fades out. Parked, that is motes popping in and fading at
        // random: twinkling, by definition. Close ones look wide,
        // parallax makes them drift. The field exists as a MOTION
        // reference, so it now answers to motion: nothing new spawns
        // while no movement key is held (the motes already alive
        // finish their fade), and nothing above about a kilometre a
        // second, where the camera crosses the 30 m field in a frame
        // or two and a mote can only be a one-frame flash. At his FTL
        // gear that is every frame.
        //
        // The COMMANDED speed, not a measured one, and deliberately:
        // below the co-rotate ceiling the camera is frame-locked to
        // the spinning planet, so a parked camera moves at hundreds
        // of m/s in absolute space (about 500 at 787 km). Measured
        // speed would have kept the dust on while he sat still. What
        // he perceives is his own thrust, which is this: the held
        // keys times the same speed both flight paths apply.
        let fly_intent = state.controller.fly_wish_dir(&state.camera);
        let cam_speed = if fly_intent.length_squared() > 0.0 {
            state.controller.speed * state.controller.fly_speed_mult.max(1.0)
        } else {
            0.0
        };
        const DUST_FAST_LO: f32 = 200.0; // m/s: above, a mote is a flash
        const DUST_FAST_HI: f32 = 1000.0;
        let fast = ((cam_speed - DUST_FAST_LO) / (DUST_FAST_HI - DUST_FAST_LO))
            .clamp(0.0, 1.0);
        let dust_rate = if cam_speed > 0.0 {
            1.0 - fast * fast * (3.0 - 2.0 * fast)
        } else {
            0.0
        };
        let dust_pos = state.camera.position + state.camera.forward() * 12.0;
        match state.particle_system.emitter_by_type_mut("space_dust") {
            Some(e) => {
                e.active = want_dust;
                e.rate_scale = dust_rate;
                if want_dust {
                    e.position = dust_pos;
                }
            }
            None => {
                if want_dust {
                    let _ = state.particle_system.spawn("space_dust", dust_pos);
                }
            }
        }
        // Precipitation (v0.1033): the HUD has said "Rain"
        // for months while the sky stayed dry - now the
        // weather sim drives a camera-following AREA
        // emitter. Condition picks rain vs snow, intensity
        // scales the per-instance spawn rate, and live wind
        // TILTS the fall direction (dir_override). Surface
        // mode only: ship interiors are not surface_mode,
        // so it cannot rain indoors today; a real
        // roof/interior signal joins when planet buildings
        // become enterable.
        let (want_rain, want_snow, precip_rate, precip_dir, event_id) = {
            let w = state
                .data_store
                .get::<std::sync::Mutex<Weather>>("weather")
                .and_then(|m| m.lock().ok())
                .map(|w| {
                    (
                        (w.condition, w.falling_at_player()),
                        w.intensity,
                        w.wind_speed,
                        w.wind_direction,
                        w.event_id.clone(),
                    )
                });
            match w {
                // ALTITUDE GATE (v0.1064, operator: "The rain
                // actually persists up into outer space, we need
                // to fix that"). surface_mode stays true while
                // flying away from a planet, so the emitters
                // followed the camera all the way out. Rain and
                // snow live under the cloud deck; above a few km
                // there is nothing to fall past you.
                Some(((cond, fall), inten, wind, wdir, ev))
                    if state.camera.surface_mode
                        // map_or(false, ..), NOT unwrap_or(0.0):
                        // the altitude readout is None whenever no
                        // surface is frame-locked (reset each frame
                        // at the FTL/fly branch), and defaulting
                        // None to 0 m meant "no data = standing on
                        // the ground", so the gate PASSED in orbit
                        // and snow followed the camera into space.
                        // v0.1064 closed this for low flight; the
                        // None hole reopened it from FTL (operator,
                        // 2026-07-31: "The snow is in space
                        // again."). No reading = no precipitation.
                        && state
                            .gui_state
                            .surface_altitude_m
                            .map_or(false, |a| a < 4000.0) =>
                {
                    use humanity_systems::weather::WeatherCondition as WC;
                    // Rain or snow: the air at the player decides (systems::precipitation).
                    let (r, s) = fall.emitters();
                    let storm_boost =
                        if matches!(cond, WC::Storm) { 1.6 } else { 1.0 };
                    let up = state.camera.up;
                    let wd = Vec3::new(wdir.x, wdir.y, wdir.z);
                    // Wind tilt: horizontal component only,
                    // capped so a storm slants hard but
                    // never goes sideways.
                    let horiz = (wd - up * wd.dot(up)).normalize_or_zero();
                    let tilt = (wind * 0.045).min(0.7);
                    let dir = (-up + horiz * tilt).normalize_or_zero();
                    // PRECIPITATION DENSITY (v0.1060, operator:
                    // "the heavy rain isn't really heavy, kind
                    // of like a fairly light rain"). The old
                    // curve was (0.4 + 0.8 * intensity), so
                    // intensity 0.9 produced 1.12 against 0.72
                    // at 0.4 - a 1.6x span across the ENTIRE
                    // range, which is not a difference anyone
                    // could see. And it only moved the spawn
                    // rate, while the population ceiling stayed
                    // pinned at the RON cap, so past a point a
                    // higher rate bought literally nothing.
                    // Now: a 20x span, times the operator's own
                    // density slider, and cap_scale lifts the
                    // ceiling so the extra drops can exist.
                    let dens = state
                        .gui_state
                        .settings
                        .precip_density
                        .clamp(0.1, 100.0);
                    (
                        r,
                        s,
                        (0.25 + 5.0 * inten) * storm_boost * dens,
                        dir,
                        ev,
                    )
                }
                _ => (false, false, 0.0, -state.camera.up, String::new()),
            }
        };
        // The cap multiplier rides the same slider: a downpour
        // needs BOTH a higher rate and room for the drops.
        let precip_cap = state
            .gui_state
            .settings
            .precip_density
            .clamp(0.1, 100.0);
        let precip_pos = state.camera.position + state.camera.up * 13.0;
        // Union in the active extreme event's emitters
        // (v0.1035): a thunderstorm keeps its rain, a
        // tornado adds dust, a meteor shower adds sparks -
        // and every OTHER event emitter is enumerated so
        // an ended event's particles deactivate cleanly.
        // ── GPU PARTICLE PATH (v0.1068) ──
        // When enabled, precipitation is simulated entirely on
        // the GPU and the CPU emitters for rain/snow are left
        // inactive. Everything else (event emitters, sparks,
        // dust) stays on the CPU path, which is correct: those
        // are small, bursty and short-lived, exactly the case
        // where the measured fork/join and dispatch overheads
        // outweigh the work.
        if state.gui_state.settings.gpu_particles && (want_rain || want_snow) {
            let key = if want_snow { "snow" } else { "rain" };
            if let Some(def) = state.particle_system.def(key) {
                let dens = state
                    .gui_state
                    .settings
                    .precip_density
                    .clamp(0.1, 100.0);
                // Same steady-state law the CPU path learned in
                // v0.1064: a pool of N living L seconds sustains
                // N/L spawns per second, so the POOL is what
                // density scales, and the recycling sim keeps it
                // permanently full.
                let live = ((def.max_particles as f32) * dens) as u32;
                let up = state.camera.up;
                let p = crate::renderer::particles_gpu::SimParams {
                    origin_radius: [
                        precip_pos.x,
                        precip_pos.y,
                        precip_pos.z,
                        def.spawn_radius,
                    ],
                    dir_spread: [
                        precip_dir.x,
                        precip_dir.y,
                        precip_dir.z,
                        def.spread_angle_deg.to_radians(),
                    ],
                    gravity_dt: [
                        def.gravity.x,
                        def.gravity.y,
                        def.gravity.z,
                        dt as f32,
                    ],
                    speed_life: [
                        def.speed_min,
                        def.speed_max,
                        def.lifetime_min,
                        def.lifetime_max,
                    ],
                    size_shape: [
                        def.size_start,
                        def.size_end,
                        def.emissive,
                        def.shape,
                    ],
                    color_start: def.color_start,
                    color_end: def.color_end,
                    count_frame_streak: [0.0, 0.0, def.streak_stretch, 0.0],
                };
                let _ = up;
                gpu!(state).renderer.simulate_gpu_particles(p, live, live.max(65_536));
            }
        } else {
            // Deactivate the pool when the GPU path is not
            // running this frame (condition switched to Clear,
            // altitude gate closed, or the setting turned off).
            // Without this the sim dispatch stops but the draw
            // keeps rendering the last-written verts: rain
            // frozen mid-air like stopped time (operator field
            // report 2026-07-31). live=0 makes the draw skip,
            // and the recycling sim re-seeds cleanly on the
            // next activation.
            gpu!(state).renderer.deactivate_gpu_particles();
        }
        let mut manage: Vec<(String, bool)> =
            vec![("rain".into(), want_rain), ("snow".into(), want_snow)];
        if let Some(reg) = state
            .data_store
            .get::<humanity_systems::weather_events::WeatherEventRegistry>(
                "weather_event_registry",
            )
        {
            for ev in &reg.events {
                let on = !event_id.is_empty() && ev.id == event_id;
                for em in &ev.emitters {
                    match manage.iter_mut().find(|(t, _)| t == em) {
                        Some(slot) => slot.1 = slot.1 || on,
                        None => manage.push((em.clone(), on)),
                    }
                }
            }
        }
        for (ty, want) in &manage {
            let (ty, want) = (ty.as_str(), *want);
            match state.particle_system.emitter_by_type_mut(ty) {
                Some(e) => {
                    e.active = want;
                    if want {
                        e.position = precip_pos;
                        e.dir_override = Some(precip_dir);
                        e.rate_scale = precip_rate;
                        e.cap_scale = precip_cap;
                    }
                }
                None => {
                    if want {
                        let i = state.particle_system.spawn(ty, precip_pos);
                        if let Some(e) = state.particle_system.emitters.get_mut(i)
                        {
                            e.dir_override = Some(precip_dir);
                            e.rate_scale = precip_rate;
                        e.cap_scale = precip_cap;
                        }
                    }
                }
            }
        }
        state.particle_system.tick(dt);
    }

}

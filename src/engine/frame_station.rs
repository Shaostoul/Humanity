//! The orbital home station's frame stage (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the station card's attitude edits, the station's orbit
//! propagated on the game clock with its nadir-pointing attitude, and the ride that carries the
//! player and the home with it.

use crate::engine::frame_prelude::*;

/// Moves the home station along its orbit for this frame.
pub(crate) fn run(state: &mut EngineState) {
    // -- Orbital home station (v0.881; put on the GAME clock and
    // given an attitude v0.1225) --
    //
    // Operator, 2026-08-26: "it's turning the planet but, I can't
    // get it to affect the homestead locked in orbit with the Earth.
    // It seems like I may need to add a control to the mothership to
    // actually change its orientation to also change that of the
    // lighting." Exactly right, and there were two faults under it.
    //
    // 1. The orbit was phased off SystemTime::now() - the real-world
    //    wall clock - while everything else in the sim runs on the
    //    game clock. So the time-of-day slider could not move the
    //    station even in principle.
    // 2. The station had no ORIENTATION at all, just a position
    //    vector. That is the one that actually froze the light:
    //    orbital position cannot change lighting, because the sun is
    //    1 AU away and a full geosynchronous circle moves the sun
    //    direction by 0.03 degrees. A hull only gets a day if it
    //    TURNS.
    //
    // Now: elements from data/stations/home.ron, propagated against
    // GameTime::elapsed_seconds, with a nadir-pointing (LVLH)
    // attitude that rotates once per orbit - which is how real
    // crewed stations fly and what sweeps the sun across the deck.
    {
        use humanity_ship::station::orbit;
        // The F11 station card writes attitude here. Applied before
        // propagation so a change takes effect on the same frame it is
        // made - dragging a trim slider should move the light now, not
        // next frame.
        if state.gui_state.station_attitude_dirty {
            let (y, pi, r) = (
                state.gui_state.station_yaw_deg as f64,
                state.gui_state.station_pitch_deg as f64,
                state.gui_state.station_roll_deg as f64,
            );
            state.station_def.attitude = if state.gui_state.station_nadir {
                orbit::AttitudeMode::Lvlh {
                    yaw_deg: y,
                    pitch_deg: pi,
                    roll_deg: r,
                }
            } else {
                orbit::AttitudeMode::Inertial {
                    yaw_deg: y,
                    pitch_deg: pi,
                    roll_deg: r,
                }
            };
            state.gui_state.station_attitude_dirty = false;
        }
        let (game_elapsed, game_day_s) = state
            .data_store
            .get::<std::sync::Mutex<humanity_systems::time::GameTime>>(
                "game_time",
            )
            .and_then(|m| m.lock().ok().map(|g| (g.elapsed_seconds, g.seconds_per_day())))
            .unwrap_or((0.0, humanity_systems::time::EARTH_DAY_S));
        // The parent rotation period is the SOLAR day, not the
        // sidereal one, because this engine defines a planet's spin
        // relative to the sun (planet_spin_from_time is
        // sun_azimuth + (hour - 12) * TAU / 24). Synchronous here
        // therefore means synchronous with the day/night cycle the
        // player actually sees. Using 86164 would drift the home a
        // quarter degree of longitude per game day.
        // Re-phased by the sun's azimuth so a synchronous home
        // hangs over its own longitude on every date (BUG-090).
        let (new_pos, vel) = orbit::propagate(
            &orbit::over_its_longitude(
                &state.station_def.orbit,
                crate::engine::frame_lock::sun_azimuth(state),
            ),
            orbit::MU_EARTH,
            orbit::REAL_SECONDS_PER_DAY,
            orbit::sim_seconds(game_elapsed, game_day_s),
            game_day_s,
        );
        let prev_rot = state.station_world_rot;
        let prev_pos = state.station_world_pos;
        let new_rot =
            orbit::attitude(&state.station_def.attitude, new_pos, vel);
        state.station_world_pos = new_pos;
        state.station_world_rot = new_rot;

        let cam_local = glam::DVec3::new(
            state.camera.position.x as f64,
            state.camera.position.y as f64,
            state.camera.position.z as f64,
        );
        let cam_world = state.ship_world_pos + cam_local;
        // Aboard is inside the ship's bounds (ship homes increment 4,
        // ship/ship_space.rs): a 400 m sphere let go of a player a fifth of
        // the way along the first drum.
        let aboard = humanity_ship::ship_space::aboard_at(state.aboard_bounds, cam_world - new_pos);
        // Departure is measured against where the station was when the
        // frame was last synced to it, NOT against where it has since
        // moved. Two different things break if you get this wrong:
        //
        //  * a hitchy frame or a scrub of the hour slider advances the
        //    station a long way in one step (at synchronous speed that
        //    is 3 km/s, and a slider drag is unbounded), which against
        //    the NEW position reads as the player having teleported off
        //    their own home. That is the v0.881 bug the original
        //    comment describes, and the game clock makes it far worse.
        //  * conversely it must still notice a REAL departure: dev
        //    travel and the probe rig move ship_world_pos directly, and
        //    if riding just re-asserts the station pose every frame it
        //    drags the camera straight back and no vantage away from
        //    the home can ever be reached. (It did exactly that for one
        //    build: blue-marble-12000km came back as empty starfield
        //    with the hull in the corner.)
        //
        // Measuring against prev_pos answers only "did the player move
        // relative to the home", which is the actual question.
        let departed = !humanity_ship::ship_space::aboard_at(state.aboard_bounds, cam_world - prev_pos)
            && prev_pos != glam::DVec3::ZERO;
        let was_riding = state.station_ride;

        if state.station_spawn_snap {
            // World load: the spawn room IS aboard.
            state.ship_world_pos = new_pos;
            state.station_ride = true;
            state.station_spawn_snap = false;
        } else if state.station_ride {
            if departed {
                // Left under our own power (or was teleported by dev
                // travel). Let go and leave the frame where it is.
                state.station_ride = false;
            } else {
                // RIDE by absolute pose, not by accumulating a
                // per-frame delta. The delta form was fine at a
                // wall-clock crawl but is a hazard on a clock the
                // operator can SCRUB: one drag of the hour slider is up
                // to a full orbit in a single frame, and integrating
                // that flings the player frame. Riding aboard simply
                // means the ship frame IS the station.
                state.ship_world_pos = new_pos;
            }
        } else if aboard {
            // BOARDING: adopt the station as the frame origin while
            // preserving the camera's world position, so local coords
            // become exact home coords with no visual pop.
            //
            // Drop any planet frame-lock on the way in (v0.1225).
            // Aboard we are not standing on a surface, and the
            // auto-release is gated off while riding, so a lock that
            // survives boarding stays engaged forever and fights this
            // block for ownership of ship_world_pos.
            state.frame_lock_body = None;
            state.frame_lock_anchor = glam::DVec3::ZERO;
            state.ship_world_pos = new_pos;
            let rel = cam_world - new_pos;
            state.camera.position =
                Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32);
            state.station_ride = true;
        }
        state.aboard_station = state.station_ride;

        // One-line orbit summary for the F11 card. Recomputed each
        // frame but only formatted when the panel is open, since it
        // allocates a String.
        if state.gui_state.show_weather_panel {
            let alt_km = (new_pos.length() - 6.371e6) / 1000.0;
            let period_s = orbit::period_from_axis(
                new_pos.length().max(1.0),
                orbit::MU_EARTH,
            );
            // Expressed in GAME hours, which is what the slider above
            // speaks. The physical period means little to the player;
            // "one sunrise per game day" is the useful fact.
            let period_game_h = period_s
                / (orbit::REAL_SECONDS_PER_DAY / game_day_s)
                / humanity_systems::time::SECONDS_PER_HOUR;
            state.gui_state.station_readout = format!(
                "{}: {:.0} km up, one orbit every {:.1} game hours.",
                state.station_def.name, alt_km, period_game_h
            );
        }

        // Keep the VIEW still across a ride transition. While riding,
        // the render frame is the hull's body frame; off the station
        // it is world-aligned. Boarding or departing therefore swaps
        // the basis under the camera, and without this the sky would
        // lurch at the hatch. Re-expressing the look direction in the
        // new basis holds it exactly (roll is not represented in a
        // yaw/pitch camera, so a rolled station still turns the
        // horizon - correct, and it is what the hull is doing).
        //
        // No compensation is applied WHILE riding, on purpose: someone
        // standing on a turning station turns with it and watches the
        // stars sweep past. That sweep is the entire point.
        if state.station_ride != was_riding {
            let look = humanity_core::dev_travel::look_dir(
                state.camera.yaw,
                state.camera.pitch,
            );
            let adjusted = if state.station_ride {
                new_rot.inverse() * look
            } else {
                prev_rot * look
            };
            let (yaw, pitch) = humanity_core::dev_travel::look_angles(adjusted);
            state.camera.yaw = yaw;
            state.camera.pitch = pitch;
        }

        let off = new_pos - state.ship_world_pos;
        state.station_off =
            Vec3::new(off.x as f32, off.y as f32, off.z as f32);
        // Target marker (v0.885): the station's render-space
        // position IS station_off (render origin = the ship
        // frame). Shown when tracked and not aboard.
        state.gui_state.target_markers.clear();
        let station_dist = off.length();
        if state.gui_state.settings.track_station && station_dist > 1_000.0 {
            state.gui_state.target_markers.push((
                state.station_def.name.clone(),
                state.station_off,
                station_dist,
            ));
        }
        crate::engine::map_feed::publish(state, vel); // Maps: where you are, this frame's pose (engine/map_feed.rs)
    }

    // A station camera park reports only now, after the block above
    // has ridden the camera through the clock change it asked for
    // (BUG-132): its camera_done is a reading, not an echo.
    crate::engine::ipc::advance_station_park(state);

}

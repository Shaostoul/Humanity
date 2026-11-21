//! Publishing the environment (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the FFT ocean's upkeep and the ocean disaster pin.

use crate::engine::frame_prelude::*;

/// Publishes the ocean's state for this frame.
pub(crate) fn run(state: &mut EngineState) {
    // Nothing to do before the world's renderer is built (a pages-only start, engine/renderer_slot.rs): the ocean's upkeep and its pin are the world's.
    if !crate::engine::world_prep::drawn(state) { // or before the world is entered (world_prep.rs)
        return;
    }
    let dt = state.clock.dt;
    humanity_core::frame_costs::span("cpu.span.env_publish");
    // FFT-ocean upkeep (v0.1029, water-fft.md increment 1):
    // build the spectrum on first use, realize this frame's
    // height field, and hand the SAME array to the GPU
    // displacement tile that the buoyancy twin reads -
    // drawn == sampled by construction. ~0.4 ms release;
    // moves to a worker (or GPU compute) in increment 4.
    if !state.gui_state.settings.water_fft {
        // Trains mode: fixed amplitude sum, so the crest is the
        // old constant and every bound behaves as before.
        gpu!(state).renderer.sea_crest_m =
            humanity_terrain::ocean_waves::MAX_WAVE_HEIGHT_M;
    }
    if state.gui_state.settings.water_fft {
        let ocean_seed = state
            .planet_defs
            .get("earth")
            .map(|d| d.terrain_seed as u64)
            .unwrap_or(7);
        // WIND-DRIVEN SPECTRUM (v0.1050, water-fft.md "Wind
        // changes REGENERATE h0(k)"). Until now the cascades
        // were built ONCE at a hardcoded 8 m/s and never
        // rebuilt, so in FFT mode the sea was permanently a
        // moderate breeze no matter the weather - the operator
        // could not find calm glass or a storm because neither
        // existed ("we got a great lightly disturbed water
        // but, what about calm glassy water or extremely
        // stormy water?"). sea_state only modulated SHADING.
        //
        // The target wind is the wind at the player, or the
        // showcase/rig {"sea":x} pin mapped onto 0.5..25 m/s
        // so a pinned sea state still means something here.
        // Smoothed on the same ~30 s constant as sea_state, so
        // a regime flip rolls the ocean over.
        // The wind AT THE PLAYER, the same one the sea
        // state's shading reads (2026-09-28).
        let wind_target = humanity_systems::weather::ocean_fft_wind_target(
            state.sea_state_override,
            state.gui_state.weather.as_ref().map(|w| w.wind_at_player),
        );
        // 30 s of smoothing is right for weather that rolls
        // in on its own, and far too slow for a hand on a
        // slider - the operator would click "Hurricane" and
        // watch nothing happen for half a minute. So the
        // manual panel and the rig's sea pin get a 2 s
        // constant: still smoothed, but visibly responsive.
        let tau = if state.sea_state_override.is_some()
            || state.gui_state.weather_manual
        {
            2.0
        } else {
            30.0
        };
        let k = (dt as f32 / tau).min(1.0);
        state.ocean_fft_wind += (wind_target - state.ocean_fft_wind) * k;
        if state.ocean_fft.is_none() {
            state.ocean_fft_wind = wind_target;
        }
        // Rebuild past a ~1 m/s drift, at most ~1 Hz. The SEED
        // is unchanged, so h0's Gaussian draws - and therefore
        // every wave's phase - are identical; only the
        // JONSWAP amplitudes are re-weighted. That is why this
        // reshapes the sea smoothly instead of snapping to a
        // different ocean, and why no crossfade machinery is
        // needed. The buoyancy twin reads the same arrays, so
        // drawn == sampled holds through a rebuild.
        state.ocean_fft_rebuild_cooldown =
            (state.ocean_fft_rebuild_cooldown - dt as f32).max(0.0);
        let drift = (state.ocean_fft_wind - state.ocean_fft_built_wind).abs();
        if state.ocean_fft.is_none()
            || (drift > 1.0 && state.ocean_fft_rebuild_cooldown <= 0.0)
        {
            let w = state.ocean_fft_wind.clamp(0.2, 30.0);
            state.ocean_fft = Some(
                humanity_terrain::ocean_fft::OceanCascades::build(
                    w, 0.6, 200_000.0, ocean_seed,
                ),
            );
            state.ocean_fft_built_wind = state.ocean_fft_wind;
            state.ocean_fft_rebuild_cooldown = 1.0;
            log::info!(
                "[OceanFft] spectrum rebuilt for wind {:.1} m/s (A 64 m + B 256 m tiles, 128x128 each)",
                w
            );
        }
        if let Some(c) = state.ocean_fft.as_mut() {
            // Same clock as the shader's trains + the
            // buoyancy twin (start-relative seconds).
            let tsec = state.start_time.elapsed().as_secs_f32();
            c.update(tsec);
            gpu!(state).renderer.upload_water_fft(&c.a.texels, &c.b.texels);
            // Publish the LIVE crest (v0.1051): the shader
            // scales its shoal fade by it and the backstop
            // shell drops below it, so both track the actual
            // sea instead of a worst case. A calm day keeps
            // exactly the old tight numbers.
            gpu!(state).renderer.sea_crest_m =
                humanity_terrain::ocean_fft::crest_estimate_m(c.total_rms());
        }
    }

    // Ocean disaster event pin (ABYSSAL adoption rung 2,
    // v0.1239): consume the showcase {"ocean_event":...}
    // request, build the pinned event in planet-model
    // coordinates (spin-fixed, the water vertex frame), and
    // feed the camera uniform rows + the live crest channel.
    // The rung-3 lifecycle will drive the same slot from
    // data/weather/events.ron.
    if let Some((req, bearing, dist_m)) = state.ocean_event_pin_request.take() {
        if req == "off" || req == "auto" {
            state.ocean_event_pin = None;
            log::info!("[OceanEvent] pin cleared");
        } else if state.frame_lock_body.is_some() {
            let anchor = state.frame_lock_anchor;
            // At the requested bearing + distance (defaults
            // west of the player, 900 m, matching the west-looking
            // ocean vantages): deterministic for the probe rig,
            // the operator.
            let core = humanity_systems::weather_events::place_event_core(
                anchor, bearing, dist_m,
            );
            let frame =
                humanity_terrain::ocean_events::OceanEventFrame::at(core);
            // The soliton's travel direction points from the
            // event toward the player, so its wall faces the
            // camera.
            let toward = frame.project(anchor);
            match humanity_terrain::ocean_events::dev_pin_events(&req, toward) {
                Some(events) => {
                    state.ocean_event_pin = Some(
                        humanity_terrain::ocean_events::PinnedOceanEvents {
                            events,
                            frame,
                        },
                    );
                    log::info!(
                        "[OceanEvent] pinned '{req}' {dist_m:.0} m from player"
                    );
                }
                None => log::warn!("[OceanEvent] unknown kind '{req}'"),
            }
        } else {
            log::warn!("[OceanEvent] pin '{req}' ignored: not on a body");
        }
    }
    match state.ocean_event_pin.as_ref() {
        Some(pin) => {
            gpu!(state).renderer.ocean_event_rows = pin.to_camera_rows();
            // Fold the event into the live crest channel so
            // the shoal fade and the backstop drop widen with
            // it (drawn == sampled: the buoyancy twin shoals
            // with the same value).
            gpu!(state).renderer.sea_crest_m = gpu!(state)
                .renderer
                .sea_crest_m
                .max(pin.max_abs_height_m() as f32);
        }
        None => gpu!(state).renderer.ocean_event_rows = [[0.0; 4]; 14],
    }

}

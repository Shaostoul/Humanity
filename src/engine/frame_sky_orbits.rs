//! Orbit lines (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs (the solar system block, now `frame_sky`) as it was: each body's
//! orbit drawn as a line in the theme's orbit colour, by the sky orbit mode setting.

use crate::engine::frame_prelude::*;

/// Draws the orbit lines for this frame.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars, sim_t: f64, earth_helio_au: glam::DVec3, sun_rel_earth_m: glam::DVec3) {
    let showroom = f.showroom;
    humanity_core::frame_costs::span("cpu.span.sky_orbits");
    // ── Orbit paths (v0.262.20 — thin world lines) ──
    // Per frame: offset each cached parent-frame
    // ellipse to its parent's Earth-relative position
    // (SAME frame as the bodies, so a planet sits
    // exactly on its ring; the Moon's ring is centred
    // on Earth) and emit a LineList (2 verts/segment).
    // draw_lines_onto depth-occludes any segment that
    // passes behind a planet — the directional cue
    // the operator asked for, via real occlusion.
    // Color from the THEME (v0.786 sky settings; default =
    // the near-black #020305 the rings always used). Mode
    // from Settings > Graphics: off / planets / +moons.
    crate::engine::world_load::refresh_orbit_paths(state); // the reuse switch off: on its schedule
    let oc = state.theme.orbit_line();
    let orbit_rgba: [f32; 4] = [
        oc.r() as f32 / 255.0,
        oc.g() as f32 / 255.0,
        oc.b() as f32 / 255.0,
        oc.a() as f32 / 255.0,
    ];
    let orbit_mode = state.gui_state.settings.sky_orbit_mode.as_str();
    // Skip the rings in the showroom (no AU-scale sky-rings in the
    // void — they'd reappear now that the celestial pass un-clips
    // them). v0.451. The bodies are already showroom-gated above.
    for (pts_m, parent_id, kind, body_id) in
        state.solar_orbit_paths.iter().filter(|_| !showroom)
    {
        if orbit_mode == "off" {
            break;
        }
        if kind == "moon" && orbit_mode != "planets_moons" {
            continue;
        }
        let parent_helio_au = if parent_id == "sun" {
            glam::DVec3::ZERO
        } else {
            humanity_core::cosmos::find_body(parent_id)
                .map(|p| humanity_core::cosmos::body_world_position_3d_au(p, sim_t))
                .unwrap_or(glam::DVec3::ZERO)
        };
        // ALL ring math stays in f64 until the final cast (v0.791):
        // vertices and offset are both ~1.5e11 m with opposite
        // signs; summing them in f32 left tens-of-km cancellation
        // jitter (the sphere path computes in f64, so the ring
        // visibly disagreed with its planet).
        let off_d = (parent_helio_au - earth_helio_au)
            * humanity_core::cosmos::M_PER_AU
            - state.ship_world_pos;
        // Body's live parent-frame position: anchors the v0.790
        // trail fade AND is SPLICED into the ring in place of the
        // nearest sample, so the polyline passes exactly through
        // the planet. A 96-chord ellipse otherwise cuts ~80,000 km
        // inside the true orbit at 1 AU -- from the ship (sitting
        // essentially ON Earth's orbit) that chord gap read as
        // "Earth is off its ring" (v0.791, operator field report).
        let body_at: Option<(usize, glam::DVec3)> =
            humanity_core::cosmos::find_body(body_id).map(|b| {
                let bp = (humanity_core::cosmos::body_world_position_3d_au(b, sim_t)
                    - parent_helio_au)
                    * humanity_core::cosmos::M_PER_AU;
                let mut best = 0usize;
                let mut best_d = f64::MAX;
                for (i, p) in pts_m.iter().enumerate() {
                    let d = p.distance_squared(bp);
                    if d < best_d {
                        best_d = d;
                        best = i;
                    }
                }
                (best, bp)
            });
        // ~10 of 96 samples = a ~38 degree fading wake.
        const FADE_SAMPLES: usize = 10;
        let n = pts_m.len();
        let fade = |i: usize| -> f32 {
            let Some((i0, _)) = body_at else { return 1.0 };
            let behind = (i0 + n - i) % n;
            (behind.min(FADE_SAMPLES) as f32) / FADE_SAMPLES as f32
        };
        let tint = |f: f32| -> [f32; 4] {
            [
                orbit_rgba[0] * f,
                orbit_rgba[1] * f,
                orbit_rgba[2] * f,
                orbit_rgba[3],
            ]
        };
        let world = |i: usize| -> [f32; 3] {
            let p = match body_at {
                Some((i0, bp)) if i0 == i => bp,
                _ => pts_m[i],
            };
            // Into the render frame like every body (HULL FRAME site
            // 1): aboard, a ring in the world's axes missed its body.
            let w = humanity_ship::station::to_hull(state.station_ride, state.station_world_rot, p + off_d);
            [w.x as f32, w.y as f32, w.z as f32]
        };
        for si in 0..n.saturating_sub(1) {
            f.orbit_lines.push(crate::renderer::line::LineVertex {
                position: world(si),
                color: tint(fade(si)),
            });
            f.orbit_lines.push(crate::renderer::line::LineVertex {
                position: world(si + 1),
                color: tint(fade(si + 1)),
            });
        }
    }

    // Keep the cached Sun pos + the shader's light
    // direction pointed at the REAL Sun so Earth's lit
    // hemisphere matches the visible Sun disc (was a
    // fixed [0.3,1,0.5] that disagreed with the disc).
    // The light DIRECTION is viewer-relative (v0.791.x):
    // sun_rel_earth_m alone is the sun's direction from
    // EARTH, correct only while the ship is near Earth --
    // after a dev-travel jump to, say, Neptune the sun is
    // in a completely different direction. Subtracting
    // ship_world_pos costs ~3e-7 rad at home (GEO vs 1 AU,
    // imperceptible) and keeps every lit hemisphere honest
    // anywhere in the system.
    state.sun_world_pos = sun_rel_earth_m;
}

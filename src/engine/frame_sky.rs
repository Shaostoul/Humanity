//! The real solar system around the home (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the sky's clock and the values every body shares, each
//! body of the canonical Keplerian model placed at its true position (`frame_sky_body`), then the orbit
//! lines (`frame_sky_orbits`) and the sun's light and shadows (`frame_sky_light`).

use crate::engine::frame_prelude::*;

/// Places and draws the solar system for this frame.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    // Nothing to draw into before the world's renderer is built (a pages-only start, a phone:
    // engine/renderer_slot.rs): the solar system's bodies and orbit lines.
    if !crate::engine::world_prep::drawn(state) { // or before the world is entered (world_prep.rs)
        return;
    }
    humanity_core::frame_costs::span("cpu.span.lists_sky");
    // ── The real solar system around the home ──
    // (map sync increment B; procedural surfaces v0.763.)
    // The player spawns inside their homestead at GEO
    // ~42,164 km from Earth's centre; Earth lives at world
    // origin. Every body the Maps page shows (Earth
    // included) is spawned at its TRUE position from the
    // SAME canonical Keplerian model (humanity_core::cosmos) the
    // Maps page reads:
    //   (helio(body) - helio(earth)) * metres-per-AU
    // i.e. Earth-centred world space (zero for Earth
    // itself), then offset into the camera/floating-origin
    // frame by -ship_pos. Live sim time = seconds since
    // J2000 from the real clock, so the sky matches the
    // real date (same as Maps "Now").
    // Planet spin in f64, reduced mod TAU (v0.872 SURFACE JITTER
    // FIX): the render path used f32 seconds for the spin angle
    // while the frame lock used f64 from the SAME clock. The
    // f32 quantization stair-steps a little more every frame,
    // rotating the drawn ground relative to the f64-placed
    // camera - the "not locked to the planet" jitter, growing
    // with uptime (~1 m after 5 minutes). ONE f64 spin value now
    // feeds both sides, and the angle is range-reduced before
    // any f32 quaternion is built from it.
    let spin_f64 = state.current_spin;
    {
        let sim_t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
            - 946_728_000.0; // Unix secs at J2000.0 epoch
        let earth_helio_au = humanity_core::cosmos::find_body("earth")
            .map(|e| humanity_core::cosmos::body_world_position_3d_au(e, sim_t))
            .unwrap_or(glam::DVec3::ZERO);
        let sun_rel_earth_m = glam::DVec3::ZERO;
        let moon_rel_earth_m = glam::DVec3::ZERO;

        // Screen-size-driven icosphere LOD (v0.763): each
        // body's subdivision level follows its projected
        // pixel size (terrain::planet::lod_level_for_pixels,
        // a doubling ladder), live-tunable from Settings ->
        // Graphics -> Planets. Meshes are cached per
        // (body, level), so a LOD switch re-uses any level
        // already built this session.
        let procedural = state.gui_state.settings.planet_detail;
        let px_per_level = state.gui_state.settings.planet_lod_px.max(1.0);
        let max_level = state
            .gui_state
            .settings
            .planet_max_subdiv
            .round()
            .clamp(0.0, humanity_terrain::planet::MAX_SKY_SUBDIVISION as f32)
            as u32;
        let viewport_h = gpu!(state).renderer.surface_size().1 as f32;
        let fov_deg = state.camera.fov_degrees;
        // Chunked-LOD patch builds are budgeted PER FRAME
        // across all bodies (in practice at most one body
        // is ever close enough to be chunk-active): each
        // body takes what the bodies before it left of the
        // terrain_builds_per_frame setting (`budget_left`).
        let patch_builds_this_frame = 0usize;
        // Every body surface drawn this frame, for the sun each one is lit
        // by (renderer::key_lights::sky_body_params, after the light block).
        let body_draws: Vec<crate::renderer::key_lights::BodyDraw> = Vec::new();
        // Every drawn body as a candidate for the disc the camera's air lets
        // show through (key_lights::sky_disc, the daytime Moon, 2026-10-21),
        // and the frame body's sphere for the shells' sun (layer_sun).
        let disc_cands: Vec<crate::renderer::key_lights::DiscCandidate> = Vec::new();
        gpu!(state).renderer.frame_body_sphere = [0.0; 4];
        // The Sun's disc pads and its frame (renderer/sun_disc.rs): set
        // below when the Sun is drawn, zero (no disc, no glare) when not.
        gpu!(state).renderer.sun_disc_pad = [[0.0; 4]; 2];
        gpu!(state).renderer.sun_frame = None;
        gpu!(state).renderer.sun_air_los = [1.0; 3];
        // The glare's aperture: Settings > Graphics > Light and sky (2026-10-22),
        // unless the showcase pins one (`sun_aperture_mm`).
        gpu!(state).renderer.sun_aperture_m = gpu!(state).renderer.sun_aperture_pin.unwrap_or_else(|| {
            crate::renderer::sun_disc::aperture_from_setting_mm(state.gui_state.settings.sun_aperture_mm)
        });

        humanity_core::frame_costs::span("cpu.span.sky_bodies");
        // Temporal clouds (phase 4): cleared each frame; the
        // cloud branch below re-arms it for the (single)
        // cloud-bearing body in view - at EVERY altitude
        // since 12c. The fullscreen composite frame (Wave D
        // 1b) follows the same lifecycle - a stale frame
        // would composite a stale map over a planetless
        // scene - and so does the one-frame resample order.
        gpu!(state).renderer.set_cloud_temporal(None);
        gpu!(state).renderer.cloud_composite_frame = None;
        gpu!(state).renderer.cloud_map_resample.set(None);
        gpu!(state).renderer.cloud_mode_near = false;
        gpu!(state).renderer.cloud_near_mix = 0.0;
        // Stale-anchor audit #16 (v0.1243): sea_sphere was set
        // only while chunked ocean water actually drew, but
        // poked into the camera pads EVERY frame and consumed
        // by the always-on per-ray underwater extinction - so
        // flying away from Earth (or to an airless body) left
        // a phantom water sphere at the last-drawn render-
        // space position, tinting any ray that crossed it.
        // Cleared here with the other per-frame resets; the
        // ocean draw below re-sets it while it is live.
        gpu!(state).renderer.sea_sphere = [0.0, 0.0, 0.0, 0.0];
        // The values every body shares, and the five the bodies change (frame_sky_body.rs).
        let mut sky = SkyBodies {
            sim_t, earth_helio_au, sun_rel_earth_m, moon_rel_earth_m, procedural, px_per_level, max_level,
            viewport_h, fov_deg, patch_builds_this_frame, body_draws, disc_cands, spin_f64,
        };
        for b in humanity_core::cosmos::sol_bodies() {
            crate::engine::frame_sky_body::run(state, f, b, &mut sky);
        }

        crate::engine::frame_sky_orbits::run(state, f, sim_t, earth_helio_au, sky.sun_rel_earth_m);
        crate::engine::frame_sky_light::run(state, sky.sun_rel_earth_m, sky.moon_rel_earth_m, &sky.body_draws, &sky.disc_cands);
    }
}

/// The values the solar system's bodies share in one frame: the clock and the level-of-detail
/// settings each body reads, and the five the bodies change (the sun's and the moon's places, the
/// patch builds this frame, the drawn bodies and the sun-disc candidates) for the orbit lines and
/// the lights after them. Made before the loop and dropped with the frame.
pub(crate) struct SkyBodies {
    pub sim_t: f64,
    pub earth_helio_au: glam::DVec3,
    pub sun_rel_earth_m: glam::DVec3,
    pub moon_rel_earth_m: glam::DVec3,
    pub procedural: bool,
    pub px_per_level: f32,
    pub max_level: u32,
    pub viewport_h: f32,
    pub fov_deg: f32,
    pub patch_builds_this_frame: usize,
    pub body_draws: Vec<crate::renderer::key_lights::BodyDraw>,
    pub disc_cands: Vec<crate::renderer::key_lights::DiscCandidate>,
    pub spin_f64: f64,
}

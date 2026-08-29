use glam::Vec3;
use std::time::Instant;
use crate::engine::state::EngineState;
use crate::terrain::planet::PlanetDef;

/// Altitude (m) below which the camera engages planetary SURFACE mode while
/// frame-locked to a body (task #76 increment 1): up becomes the local
/// radial, gravity rests you on the ground. Above this the world-Y orbit /
/// inspection lock keeps running, so marble views from orbit are unchanged.
pub(crate) const SURFACE_ENGAGE_ALT: f64 = 10_000.0;

/// Gravity easing rate (per second) for the surface stand/settle clamp.
/// At 4/s a drop from tens of metres settles onto the ground in ~1-2 s.
pub(crate) const SURFACE_SETTLE_RATE: f64 = 4.0;

/// Upper bound on the mouse-wheel speed multiplier while WALKING in the
/// walk band (fly mode OFF). v0.1006 cut this 2000x -> 50x after the
/// [Dive] diag showed 333 m PER FRAME of ground-level speed; v0.1007
/// narrowed the cap to walking only (operator: "I liked moving fast when
/// I had the speed maxed out") - DEV FLY keeps the full wheel at every
/// altitude and is bounded by the approach governor instead (per-frame
/// step <= half the height above ground), which is what actually prevents
/// teleport-taps and ground clipping.
pub(crate) const SURFACE_SPEED_MULT_MAX: f64 = 50.0;

/// Co-rotation ceiling (v0.872, operator: "when I desync from Earth once I
/// get like 10 miles up my camera resets... I'm still very close to
/// Earth"). The frame rides the planet's spin through the whole
/// atmosphere band - a plane at 15 km moves with the atmosphere - so the
/// old 10 km cliff no longer exists. Walk/gravity mechanics still gate on
/// SURFACE_ENGAGE_ALT; 10-100 km is free co-rotating flight.
pub(crate) const CO_ROTATE_MAX_ALT: f64 = 100_000.0;

/// Between CO_ROTATE_MAX_ALT and this altitude the camera's up-vector
/// EASES from radial (horizon level) to inertial world-Y (the orbit/ISS
/// view) instead of snapping, and position goes inertial. Beyond it the
/// surface basis fully releases. (True Hill-sphere/SOI frame hierarchy
/// governs TRANSLATION when we leave Earth's locale - Earth is the frame
/// origin today, so what the player feels is this rotation/up handoff.)
pub(crate) const INERTIAL_BLEND_MAX_ALT: f64 = 1_000_000.0;

/// Ground radius (metres from the body centre) of the DRAWN surface at a
/// unit direction in the body's UNROTATED local frame. Samples the real
/// heightmap (analytic, no mesh raycast) and applies `displaced_radius_f64`
/// so earth.ron's vertical exaggeration (`surface_relief`) is baked in and
/// the stand height matches the drawn ground. Falls back to the def's sea
/// level (no heightmap) or a bare Earth radius (no def).
pub(crate) fn ground_radius_m(
    def: Option<&PlanetDef>,
    hm: Option<&crate::terrain::planet_heightmap::PlanetHeightmap>,
    detail: Option<&crate::terrain::planet_chunks::DetailNoise>,
    tiles: Option<&crate::terrain::terrain_tiles::TerrainTiles>,
    // f64 unit dir (v0.1012, the operator f32 audit): an f32 UNIT vector
    // quantizes ground positions at ~0.4-0.8 m - the same defect class as
    // the v0.1010 tile-grid fix, but on the CLAMP side. The coarse base
    // heightmap downcasts internally; the tile + detail samplers get the
    // full double.
    unit_dir: glam::DVec3,
) -> f64 {
    match def {
        Some(d) => {
            let elev = match hm {
                // With a detail sampler (the eye-height clamp), reproduce
                // the DRAWN chunk-mesh elevation (base heightmap + land-
                // masked sub-grid detail, tile tier included) so the player
                // stands ON the drawn ground instead of INSIDE it. Base-only
                // clamps sank the 1.7 m eye below the up-to-~120 m detail
                // relief -> see-through ground (2026-07-12). Altitude-parking
                // callers pass None (the detail is negligible against 1.5 km).
                // `tiles` is EARTH's tier: callers pass it only for earth.
                Some(h) => match detail {
                    Some(dn) => {
                        crate::terrain::planet_chunks::drawn_elevation_normalized(
                            h, d, dn, tiles, unit_dir,
                        )
                    }
                    None => h.normalized_at(unit_dir.as_vec3()),
                },
                None => d.sea_level.clamp(0.0, 1.0),
            };
            // v0.903 diving: water worlds report the TRUE bathymetric
            // radius (the seafloor), not the sea-clamped one - the water
            // SURFACE floor is applied separately by the walk-band ocean
            // branch/backstop, and skipping those while descending is
            // what makes the ocean enterable down to the Marianas trench.
            if d.has_water {
                d.radius
                    * crate::terrain::planet_surface::displaced_radius_f64_true(d, elev as f64)
            } else {
                d.radius * crate::terrain::planet_surface::displaced_radius_f64(d, elev as f64)
            }
        }
        None => 6.371e6,
    }
}

/// The planet spin angle for THIS frame (v0.878 sun-frame unification):
/// pure function of the game clock (f64 hour derived from
/// GameTime::elapsed_seconds -- the f32 hour field would re-introduce
/// v0.872-style quantization jitter) and the world-frame sun azimuth.
/// Both inputs change once per frame, so every caller within a frame
/// sees the same value with no ordering hazard. See
/// dev_travel::planet_spin_from_time for the geometry + conventions.
pub(crate) fn current_planet_spin(state: &EngineState) -> f64 {
    let hour = state
        .data_store
        .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
        .and_then(|m| m.lock().ok())
        .map(|gt| {
            gt.elapsed_seconds.rem_euclid(crate::systems::time::SECONDS_PER_DAY)
                / crate::systems::time::SECONDS_PER_DAY
                * 24.0
        })
        .unwrap_or(12.0);
    let s = state.sun_world_pos;
    let sun_az = if s.length_squared() > 1e-6 {
        (-s.z).atan2(s.x)
    } else {
        0.0
    };
    crate::dev_travel::planet_spin_from_time(hour, sun_az)
}

/// Micro-detail precision anchor (v0.902): the camera's planet-frame
/// position mod 64 m. Camera-relative fragment offsets plus this anchor
/// give sub-metre ground/water texture with full f32 precision; jumps
/// are exact 64 m steps, seamless for any pattern whose period divides
/// 64 m. Zeros when not frame-locked (micro octaves are footprint-faded
/// to nothing at those distances anyway).
///
/// v0.1029: element 0 is the FFT-ocean mode flag (light0_cone_inner.x,
/// the pad beside the anchor) - 1.0 when the Settings toggle is on AND
/// the CPU spectrum is built, so the shader and the buoyancy twin flip
/// together and never disagree mid-build.
pub(crate) fn ground_anchor(state: &EngineState) -> [f32; 4] {
    let fft = if state.gui_state.settings.water_fft && state.ocean_fft.is_some() {
        1.0
    } else {
        0.0
    };
    if state.frame_lock_body.is_none() {
        return [fft, 0.0, 0.0, 0.0];
    }
    let a = state.frame_lock_anchor;
    [
        fft,
        a.x.rem_euclid(64.0) as f32,
        a.y.rem_euclid(64.0) as f32,
        a.z.rem_euclid(64.0) as f32,
    ]
}

/// FFT cascade-B anchor (v0.1040) + the water-weld LOD constant
/// (v0.1041): xyz = the camera's planet-frame position mod
/// FFT_B_ANCHOR_MOD_M for cascade B's tile (snaps are exact whole-tile
/// steps, like the 64 m anchor). v0.1055: the modulus is read from
/// ocean_fft rather than hardcoded, because the tile moved to 208 m to
/// break the composite grid and these two MUST move together; w = K = px_per_rad / split_px, the SELECTION's own
/// pixel-error scale. The vertex shader's geomorph weld must reach
/// full morph exactly where the LOD handoff really happens
/// (dist ~ 1.74 * cell * K, from screen_error_px + the 1.15/0.7
/// hysteresis) - the v0.1041 first attempt GUESSED this scale, every
/// patch collapsed a level too early, and the sea turned into giant
/// plates. Poked into light4_cone_inner.xyzw.
pub(crate) fn ocean_anchor256(state: &EngineState) -> [f32; 4] {
    let split_px = state.gui_state.settings.terrain_split_px.clamp(2.0, 24.0);
    let viewport_h = state.renderer.surface_size().1 as f32;
    let px_per_rad = viewport_h / state.camera.fov_degrees.max(1.0).to_radians();
    let k = px_per_rad / split_px.max(0.1);
    if state.frame_lock_body.is_none() {
        return [0.0, 0.0, 0.0, k];
    }
    let a = state.frame_lock_anchor;
    let m = crate::terrain::ocean_fft::FFT_B_ANCHOR_MOD_M;
    [
        a.x.rem_euclid(m) as f32,
        a.y.rem_euclid(m) as f32,
        a.z.rem_euclid(m) as f32,
        k,
    ]
}

/// Cloud-ground-shadow params for the celestial pass (v0.898): the
/// frame-locked planet's cloud seed + deck coverage + on/off. Zeroes
/// (shadows off) away from any cloudy body or with clouds disabled.
pub(crate) fn cloud_ground_params(state: &EngineState) -> (f32, f32, bool) {
    if !state.gui_state.settings.planet_clouds {
        return (0.0, 0.0, false);
    }
    let body = state.frame_lock_body.as_deref().unwrap_or("earth");
    match state.planet_defs.get(body).and_then(|d| {
        d.cloud_coverage
            .filter(|c| *c > 0.0)
            .map(|c| (crate::renderer::clouds::cloud_seed(d.terrain_seed), c.min(1.0)))
    }) {
        Some((seed, cov)) => (seed, cov, true),
        None => (0.0, 0.0, false),
    }
}

/// Overhead-cloud dimming for the god-ray pass (v0.897): sample the live
/// weather grid at the camera's planet-frame position. Under a real
/// overcast (MODIS says clouds here) the shafts fade to ~15% instead of
/// punching through the deck at full strength - thick water blocks sun.
/// Same envelope curve the sky shader uses (smoothstep 0.35..0.9).
/// Combined god-ray strength scale: overcast weather dims the shafts,
/// and geometric sun occlusion (v0.921) KILLS them when a solar body
/// sits between the camera and the sun - the screen-space march can
/// only see occluders in the frame, so without this the night side of
/// the station showed shafts "peaking around and through the planet"
/// (operator report, 2026-07-21).
pub(crate) fn godray_scale(state: &EngineState) -> f32 {
    let occ = sun_occlusion_factor(state);
    if occ <= 0.001 {
        return 0.0;
    }
    occ * godray_weather_scale(state)
}

/// Geometric sun visibility from the camera against every solar body
/// (pure math + tests: renderer::godrays::segment_sphere_visibility).
/// Also covers eclipses - the Moon crossing the sun dims the rays.
pub(crate) fn sun_occlusion_factor(state: &EngineState) -> f32 {
    let cam = state.ship_world_pos
        + glam::DVec3::new(
            state.camera.position.x as f64,
            state.camera.position.y as f64,
            state.camera.position.z as f64,
        );
    let sim_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0;
    let earth_helio = crate::cosmos::find_body("earth")
        .map(|e| crate::cosmos::body_world_position_3d_au(e, sim_t))
        .unwrap_or(glam::DVec3::ZERO);
    let mut vis = 1.0_f32;
    for body in crate::cosmos::sol_bodies() {
        if body.body_type == "star" {
            continue;
        }
        let center = (crate::cosmos::body_world_position_3d_au(body, sim_t)
            - earth_helio)
            * crate::cosmos::M_PER_AU;
        vis *= crate::renderer::godrays::segment_sphere_visibility(
            cam,
            state.sun_world_pos,
            center,
            body.radius_km * 1000.0,
        );
        if vis <= 0.001 {
            return 0.0;
        }
    }
    vis
}

/// Cloud alpha along the camera-to-sun ray, at the deck crossing: the max
/// of the pinned procedural field (the CURRENT weather_pinned_field mirror)
/// and the live MODIS grid cell there. 0.0 = clear path to the sun. Shared
/// by the god-ray dim and the per-frame sun disc/halo intensity scale, so
/// shafts and glare always agree about the same cloud bank (v0.1243, the
/// operator's "sun bleeding out the clouds as if they don't exist").
pub(crate) fn sun_cloud_alpha(state: &EngineState) -> f32 {
    if state.frame_lock_body.as_deref() != Some("earth") {
        return 0.0;
    }
    // ── SUN-WARD, not overhead (v0.1243, origin audit #20 / operator: "the
    // sun seems to be bleeding out the clouds as if they don't exist") ──
    // The old dim sampled the cloud cell at the camera's ZENITH: a low sun
    // behind a horizon cloud bank with clear sky overhead kept the shafts at
    // full strength through the bank - god rays are the one sun visual drawn
    // AFTER the cloud composite, so nothing else could save them. The right
    // cell is where the camera-to-sun ray crosses the drawn cloud shell.
    let p = state.frame_lock_anchor; // camera in the unrotated planet frame, metres
    if p.length_squared() < 0.5 {
        return 0.0;
    }
    let cam_w = state.ship_world_pos
        + glam::DVec3::new(
            state.camera.position.x as f64,
            state.camera.position.y as f64,
            state.camera.position.z as f64,
        );
    let s_w = (state.sun_world_pos - cam_w).normalize_or_zero();
    if s_w.length_squared() < 0.5 {
        return 0.0;
    }
    // Rotate the sun direction into the same unrotated planet frame the
    // anchor lives in (the anchor is re-captured with current_spin every
    // frame, so the two agree).
    let s_pf = glam::DQuat::from_rotation_y(-state.current_spin) * s_w;
    let r_deck = state
        .planet_defs
        .get("earth")
        .map(|d| d.radius as f64 * crate::renderer::clouds::CLOUD_SHELL_SCALE as f64)
        .unwrap_or(0.0);
    if r_deck <= 0.0 {
        return 0.0;
    }
    // Ray-sphere from p toward the sun against the drawn cloud shell. Above
    // the shell with the sun up there is no crossing - no cloud can occlude.
    let b = p.dot(s_pf);
    let disc = b * b - (p.length_squared() - r_deck * r_deck);
    if disc <= 0.0 {
        return 0.0;
    }
    let sq = disc.sqrt();
    let t_hit = if -b - sq > 0.0 { -b - sq } else { -b + sq };
    if t_hit <= 0.0 {
        return 0.0;
    }
    let dir0 = (p + s_pf * t_hit).normalize_or_zero();
    if dir0.length_squared() < 0.5 {
        return 0.0;
    }

    // ── Procedural term: the same five-octave pinned field the sky draws
    // (weather_pinned_field is the CURRENT mirror; clouds::cloud_weather is
    // the documented-stale one). Works with live weather OFF - the state the
    // operator reported from.
    let (seed, cov, clouds_on) = cloud_ground_params(state);
    let proc_a = if clouds_on {
        let adv_a = (state.cloud_advect + state.cloud_advect_decaying) as f32;
        let dq = crate::renderer::clouds::cloud_rot_y(
            [dir0.x as f32, dir0.y as f32, dir0.z as f32],
            adv_a,
        );
        let field = crate::renderer::cloud_reference::weather_pinned_field(
            dq,
            state.start_time.elapsed().as_secs_f32(),
            seed,
            0.0,
        );
        crate::renderer::clouds::cloud_alpha_from_field(field, cov)
    } else {
        0.0
    };

    // ── Live-grid term (unchanged mapping, now at the sun-ward cell) ──
    let Some(grid) = state.weather_grid.as_ref() else {
        return proc_a;
    };
    // v0.1032: mirror the shader's wind-advection rotation (cloud_rot_y
    // by light1_cone_inner.x) so the dim reads the same cloud cell the
    // sky actually draws there.
    let adv = (state.cloud_advect + state.cloud_advect_decaying) as f64;
    let (c, s) = (adv.cos(), adv.sin());
    let dir = glam::DVec3::new(
        c * dir0.x + s * dir0.z,
        dir0.y,
        c * dir0.z - s * dir0.x,
    );
    let w = crate::renderer::WEATHER_MAP_W as usize;
    let h = crate::renderer::WEATHER_MAP_H as usize;
    // Same equirect mapping as the sky shader's cloud_weather.
    let lon = (-dir.z).atan2(dir.x);
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let u = (lon * std::f64::consts::FRAC_1_PI * 0.5 + 0.5).rem_euclid(1.0);
    let v = (0.5 - lat * std::f64::consts::FRAC_1_PI).clamp(0.0, 1.0);
    let x = ((u * w as f64) as usize).min(w - 1);
    let y = ((v * h as f64) as usize).min(h - 1);
    let idx = (y * w + x) * 2;
    let (Some(&r), Some(&g)) = (grid.get(idx), grid.get(idx + 1)) else {
        return proc_a;
    };
    // The thicker of the two verdicts wins: the grid knows live MODIS
    // weather, the procedural field knows the pinned/drawn sky.
    proc_a.max((r as f32 / 255.0) * (g as f32 / 255.0))
}

/// The god-ray strength scale from the sun-ward cloud alpha: the same
/// smoothstep envelope the v0.897 overhead dim used, now fed by the cell the
/// shafts actually shine through.
pub(crate) fn godray_weather_scale(state: &EngineState) -> f32 {
    let a = sun_cloud_alpha(state);
    let t = ((a - 0.35) / 0.55).clamp(0.0, 1.0);
    let env = t * t * (3.0 - 2.0 * t);
    1.0 - 0.85 * env
}

/// Per-body environment snapshot (artificial-planet increment 4): publish
/// which world the player is standing on into the DataStore so the ECS
/// systems simulate THAT body instead of an Earth-global one. WeatherSystem
/// gates precipitation + retunes its temperature baseline from it;
/// AtmosphereSystem sets the outside air from it (vacuum on the Moon, thin
/// toxic CO2 on Mars); the survival context reads outside-the-hull
/// breathability from it. The frame-lock -> environment decision lives in
/// body_environment::environment_for_frame_lock (pure, unit tested): no
/// frame lock (the home station in high Earth orbit, or deep space) keeps
/// the Earth-home DEFAULT so home farming and the home sky behave exactly
/// as before this increment (`locked: false` keeps outside the hull a
/// vacuum in space), while a locked body WITHOUT a data/planets def
/// publishes as an AIRLESS rock instead of silently inheriting Earth's
/// default (it used to rain on Venus).
pub(crate) fn publish_body_environment(state: &mut EngineState) {
    let body_id = state.frame_lock_body.as_deref();
    let def = body_id.and_then(|id| state.planet_defs.get(id));
    // Latitude straight from the frame-lock anchor: the anchor is the
    // camera position in the body's rotating frame and bodies spin about
    // +Y, so asin(y/r) is spin-invariant latitude.
    let up = state.frame_lock_anchor.normalize_or_zero();
    let latitude_deg = up.y.clamp(-1.0, 1.0).asin().to_degrees() as f32;
    // Altitude for the temperature lapse + breathable ceiling: height
    // above the body's NOMINAL surface radius (anchor.length() -
    // def.radius). The anchor sits on the drawn ground, so this INCLUDES
    // terrain elevation: a 4 km plateau reads ~4 km and the mountain-cold
    // lapse actually engages. The drawn-ground readout
    // (gui_state.surface_altitude_m) is deliberately NOT used here (review
    // fix): it measures height above the ground under your feet, i.e. 0
    // whenever you stand anywhere, which silenced the documented
    // walking-up-a-mountain lapse entirely. Slightly negative readings
    // (sea floor, deep valleys) are real; consumers clamp where needed.
    // Def-less bodies have no radius to measure against; 0 is inert there
    // (airless bodies have no lapse and are never breathable).
    let altitude_m = def
        .map(|d| (state.frame_lock_anchor.length() - d.radius) as f32)
        .unwrap_or(0.0);
    // Catalog mean temperature; the <= 0.0 absent-field sentinel is
    // normalized inside the pure function.
    let catalog_mean_k = body_id
        .and_then(crate::cosmos::find_body)
        .map(|b| b.mean_temperature_k as f32);
    let env = crate::systems::body_environment::environment_for_frame_lock(
        body_id,
        def,
        catalog_mean_k,
        latitude_deg,
        altitude_m,
    );
    state.data_store.insert("body_environment", env);
}

/// Mirror the Planet Tuner page's live readout + editing copy (v0.1120).
/// Runs only while the Platform page is open (the tuner is one of its
/// sections), so the per-frame cost off-page is a single enum compare.
/// Lives here beside publish_body_environment because it derives from the
/// same frame-lock inputs; the page itself (gui/pages/planet_tuner.rs)
/// never touches EngineState.
pub(crate) fn publish_planet_tuner_readout(state: &mut EngineState) {
    if state.gui_state.active_page != crate::gui::GuiPage::Platform {
        return;
    }
    let t = &mut state.gui_state.planet_tuner;
    let Some(body_id) = state.frame_lock_body.clone() else {
        t.readout.locked = false;
        return;
    };
    let def = state.planet_defs.get(&body_id);
    let altitude_m = def
        .map(|d| (state.frame_lock_anchor.length() - d.radius) as f32)
        .unwrap_or(0.0);
    let up = state.frame_lock_anchor.normalize_or_zero();
    t.readout.locked = true;
    t.readout.body_id = body_id.clone();
    t.readout.body_name = def
        .map(|d| d.name.clone())
        .or_else(|| crate::cosmos::find_body(&body_id).map(|b| b.name.clone()))
        .unwrap_or_else(|| body_id.clone());
    t.readout.has_def = def.is_some();
    t.readout.altitude_m = altitude_m;
    t.readout.latitude_deg = up.y.clamp(-1.0, 1.0).asin().to_degrees() as f32;
    t.readout.g_at_player = def.map(|d| d.gravity_at(altitude_m as f64)).unwrap_or(0.0);
    t.readout.day_length_hours = def
        .map(|d| (d.rotation_period / 3600.0) as f32)
        .unwrap_or(24.0);
    if let Some(w) = state
        .data_store
        .get::<std::sync::Mutex<crate::systems::weather::Weather>>("weather")
        .and_then(|m| m.lock().ok())
    {
        t.readout.temp_at_player_c = w.temperature_at_player;
        t.readout.temp_global_c = w.temperature;
    }
    t.readout.breathable_outside = state
        .data_store
        .get::<crate::systems::body_environment::BodyEnvironment>("body_environment")
        .map(|e| e.breathable_outside())
        .unwrap_or(false);
    // Editor copies: `current` tracks the loaded def every frame (so a hot
    // reload after Save shows up instantly); `working` re-seeds only when
    // the locked body changes, so in-progress edits survive frames.
    t.current = def.cloned();
    if t.working_body != body_id {
        t.working_body = body_id;
        t.working = t.current.clone();
        t.curve_text = t
            .current
            .as_ref()
            .map(|d| crate::gui::pages::planet_tuner::fmt_curve(&d.gravity_curve))
            .unwrap_or_default();
        t.status.clear();
    }
}

/// F6 location bookmark save (v0.890). Captures the camera's EXACT world
/// placement + aim. When frame-locked to a body the pose is stored in that
/// body's UNROTATED local frame (same math as the frame-lock itself), so a
/// later restore lands on the same ground even after the planet has spun
/// on; away from any body it stores the absolute Earth-relative pose.
/// Appended to debug/bookmarks.json; restore via camera_request
/// {"bookmark":"bm-N"} (or "last"). Permanent dev tooling.
pub(crate) fn save_location_bookmark(state: &mut EngineState) {
    if !state.bookmark_save_requested {
        return;
    }
    state.bookmark_save_requested = false;
    if !state.world_loaded {
        return;
    }
    let cam_local = glam::DVec3::new(
        state.camera.position.x as f64,
        state.camera.position.y as f64,
        state.camera.position.z as f64,
    );
    let vantage = state.ship_world_pos + cam_local;
    let f = state.camera.forward();
    let fwd_world = glam::DVec3::new(f.x as f64, f.y as f64, f.z as f64);
    let spin = state.current_spin;
    let sim_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0;
    let earth_helio_au = crate::cosmos::find_body("earth")
        .map(|e| crate::cosmos::body_world_position_3d_au(e, sim_t))
        .unwrap_or(glam::DVec3::ZERO);
    // Frame-locked: store body-local pose (spin-independent). Free space:
    // store the absolute pose with body = null.
    let locked_body = state
        .frame_lock_body
        .as_deref()
        .and_then(crate::cosmos::find_body);
    let (body_json, anchor, fwd_store) = if let Some(body) = locked_body {
        let body_rel_earth = (crate::cosmos::body_world_position_3d_au(body, sim_t)
            - earth_helio_au)
            * crate::cosmos::M_PER_AU;
        let anchor = crate::dev_travel::frame_lock_capture(body_rel_earth, spin, vantage);
        let fwd_local = glam::DQuat::from_rotation_y(spin).inverse() * fwd_world;
        (serde_json::json!(body.id.clone()), anchor, fwd_local)
    } else {
        (serde_json::Value::Null, vantage, fwd_world)
    };
    const PATH: &str = "debug/bookmarks.json";
    let mut root = std::fs::read_to_string(PATH)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .unwrap_or_else(|| serde_json::json!({ "bookmarks": [] }));
    if !root.get("bookmarks").map(|b| b.is_array()).unwrap_or(false) {
        root = serde_json::json!({ "bookmarks": [] });
    }
    let count = root["bookmarks"].as_array().map(|a| a.len()).unwrap_or(0);
    let name = format!("bm-{}", count + 1);
    // Category (v0.913, operator: "we should be able to add categories
    // to the locations we can teleport to"): whatever is typed in the
    // Dev > Travel category box at save time tags the bookmark.
    let category = {
        let c = state.gui_state.bookmark_new_category.trim();
        if c.is_empty() { "Uncategorized".to_string() } else { c.to_string() }
    };
    let entry = serde_json::json!({
        "name": name,
        "category": category,
        "body": body_json,
        "anchor": [anchor.x, anchor.y, anchor.z],
        "fwd": [fwd_store.x, fwd_store.y, fwd_store.z],
        "saved_unix": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    if let Some(arr) = root["bookmarks"].as_array_mut() {
        arr.push(entry);
    }
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(
        PATH,
        serde_json::to_string_pretty(&root).unwrap_or_default(),
    );
    state.gui_state.location_bookmarks_dirty = true;
    state.gui_state.pending_toasts.push((
        format!("Location bookmark {name} saved ({category})"),
        crate::gui::ToastKind::Success,
    ));
    log::info!(
        "Bookmark {name} saved (body {:?}, anchor {:.0} {:.0} {:.0})",
        state.frame_lock_body,
        anchor.x,
        anchor.y,
        anchor.z
    );
}

/// Bookmark restore branch of the camera request (v0.890): places the
/// camera EXACTLY at a saved F6 pose (position AND aim), re-locking to the
/// saved body's rotating frame. Returns true if the request was a
/// bookmark (handled here, done-file written).
pub(crate) fn restore_location_bookmark(state: &mut EngineState, v: &serde_json::Value) -> bool {
    const DONE_PATH: &str = "debug/camera_done.json";
    let Some(want) = v.get("bookmark").and_then(|b| b.as_str()).map(|s| s.to_string())
    else {
        return false;
    };
    let fail = |msg: String| {
        log::warn!("Bookmark restore: {msg}");
        let _ = std::fs::create_dir_all("debug");
        let _ = std::fs::write(
            DONE_PATH,
            serde_json::json!({"ok": false, "error": msg}).to_string(),
        );
    };
    let root = std::fs::read_to_string("debug/bookmarks.json")
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    let Some(root) = root else {
        fail("debug/bookmarks.json missing or unreadable".to_string());
        return true;
    };
    let empty = Vec::new();
    let list = root["bookmarks"].as_array().unwrap_or(&empty);
    let found = if want == "last" {
        list.last()
    } else {
        list.iter().find(|b| b["name"].as_str() == Some(want.as_str()))
    };
    let Some(bm) = found else {
        fail(format!("no bookmark named {want} ({} saved)", list.len()));
        return true;
    };
    let vec3 = |key: &str| -> Option<glam::DVec3> {
        let a = bm.get(key)?.as_array()?;
        Some(glam::DVec3::new(
            a.first()?.as_f64()?,
            a.get(1)?.as_f64()?,
            a.get(2)?.as_f64()?,
        ))
    };
    let (Some(anchor), Some(fwd_store)) = (vec3("anchor"), vec3("fwd")) else {
        fail(format!("bookmark {want} has malformed anchor/fwd"));
        return true;
    };
    let spin = state.current_spin;
    let sim_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0;
    let earth_helio_au = crate::cosmos::find_body("earth")
        .map(|e| crate::cosmos::body_world_position_3d_au(e, sim_t))
        .unwrap_or(glam::DVec3::ZERO);
    let body_id = bm.get("body").and_then(|b| b.as_str()).map(|s| s.to_string());
    // Re-spin the stored local pose into the CURRENT world frame.
    let (vantage, fwd_world, lock) = if let Some(bid) = body_id.as_deref() {
        let Some(body) = crate::cosmos::find_body(bid) else {
            fail(format!("bookmark {want} references unknown body {bid}"));
            return true;
        };
        let body_rel_earth = (crate::cosmos::body_world_position_3d_au(body, sim_t)
            - earth_helio_au)
            * crate::cosmos::M_PER_AU;
        let rot = glam::DQuat::from_rotation_y(spin);
        (
            body_rel_earth + rot * anchor,
            rot * fwd_store,
            Some((bid.to_string(), body_rel_earth)),
        )
    } else {
        (anchor, fwd_store, None)
    };
    // Same teleport bookkeeping as the main camera request path.
    if state.dev_travel_home.is_none() {
        state.dev_travel_home = Some((
            state.ship_world_pos,
            state.camera.position,
            state.camera.yaw,
            state.camera.pitch,
        ));
    }
    if state.gui_state.copresence_active && !state.gui_state.copresence_solo {
        state.gui_state.copresence_solo = true;
        state.dev_travel_stepped_out = true;
    }
    if state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson {
        state
            .camera
            .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
    }
    let hull_top = state.homestead_bounds.map(|(_, mx)| mx.y).unwrap_or(20.0);
    let cam_local = Vec3::new(0.0, hull_top + 30.0, 0.0);
    state.camera.position = cam_local;
    state.ship_world_pos = vantage
        - glam::DVec3::new(cam_local.x as f64, cam_local.y as f64, cam_local.z as f64);
    state.camera.clear_surface();
    state.camera.roll = 0.0;
    let (yaw, pitch) = crate::dev_travel::look_angles(fwd_world);
    state.camera.yaw = yaw;
    state.camera.pitch = pitch;
    state.gui_state.dev_fly_mode = true;
    state.controller.fly_mode = true;
    state.gui_state.dev_travel_away = true;
    if let Some((bid, body_rel_earth)) = lock {
        state.frame_lock_anchor =
            crate::dev_travel::frame_lock_capture(body_rel_earth, spin, vantage);
        state.frame_lock_last_spin = spin;
        state.frame_lock_body = Some(bid);
    } else {
        state.frame_lock_body = None;
    }
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(
        DONE_PATH,
        serde_json::json!({
            "ok": true,
            "bookmark": want,
            "body": body_id,
        })
        .to_string(),
    );
    log::info!("Bookmark restore: {want} (body {body_id:?})");
    true
}

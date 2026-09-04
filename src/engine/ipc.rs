use glam::Vec3;
use crate::engine::frame_lock::*;
use crate::engine::ipc_parse::*;
use crate::engine::state::*;
use crate::gui::{GuiPage, GuiState};

/// data/world/showcase.ron shape (v0.863 perpetual showcase).
#[derive(serde::Deserialize)]
pub(crate) struct ShowcaseCfg {
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) bed_crops: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub(crate) bed_units: u32,
    #[serde(default)]
    pub(crate) tower_overrides: std::collections::HashMap<String, String>,
}

/// Perpetual showcase auto-seed (v0.863). Operator: "just preload everything
/// with plants at different stages... a perpetual showcase." Whenever the
/// world holds ZERO crops (fresh boot, empty save, everything harvested),
/// replant every grow surface at staggered growth stages: each tower column
/// gets its config's curated plantings (or a whole-tower override, e.g. the
/// ntower_0 strawberry hero), each bed/field/rack gets its mapped crop.
/// Crops persist in the save now, so this fires only on a genuinely empty
/// garden. Called every frame; the empty check makes it ~free.
pub(crate) fn auto_seed_showcase(state: &mut EngineState) {
    // One-shot guard diagnostics: when seeding is NOT happening, say why
    // once, so an empty garden is never a silent mystery.
    static DIAGNOSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let diag = |why: &str| {
        if !DIAGNOSED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            log::info!("[Showcase] auto-seed idle: {why}");
        }
    };
    if state
        .game_world
        .world
        .query::<&crate::ecs::components::CropInstance>()
        .iter()
        .next()
        .is_some()
    {
        diag("crops already present");
        return;
    }
    if state.gui_state.home_machines.is_none() {
        diag("home machines not loaded yet");
        return;
    }
    if state.grow_positions.is_empty() {
        diag("no machine anchors recorded yet");
        return;
    }
    let Ok(text) = std::fs::read_to_string(crate::data_dir().join("world/showcase.ron"))
    else {
        diag("data/world/showcase.ron missing");
        return;
    };
    let Ok(cfg) = ron::from_str::<ShowcaseCfg>(&text) else {
        log::warn!("showcase.ron failed to parse; auto-seed skipped");
        return;
    };
    if !cfg.enabled {
        return;
    }
    let Some(reg) = state
        .data_store
        .get::<crate::systems::farming::PlantRegistry>("plant_registry")
    else {
        return;
    };
    let elapsed = state
        .data_store
        .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
        .and_then(|m| m.lock().ok())
        .map(|gt| gt.elapsed_seconds)
        .unwrap_or(0.0);
    let tower_cfgs = crate::gui::load_tower_configs(&crate::data_dir());
    const DAY: f64 = 1200.0; // farming SECONDS_PER_DAY

    // Collect the spawn list first (no world borrow while iterating data).
    let mut to_spawn: Vec<crate::ecs::components::CropInstance> = Vec::new();
    let mut stagger = |list: &mut Vec<crate::ecs::components::CropInstance>,
                       plant_id: &str,
                       grow_id: &str,
                       slot: u32,
                       frac: f32| {
        let Some(def) = reg.get(plant_id) else { return };
        let stages = def.stages();
        let n = stages.len().max(1);
        let stage_i = ((frac * n as f32).floor() as usize).min(n - 1);
        list.push(crate::ecs::components::CropInstance {
            crop_def_id: plant_id.to_string(),
            growth_stage: stages[stage_i].to_string(),
            planted_at: elapsed - def.growth_days as f64 * DAY * frac as f64 * 0.98,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(grow_id.to_string()),
            tower_slot: Some(slot),
        });
    };
    for g in &state.grow_positions {
        if let Some(cfg_key) = g.ty.strip_prefix("aeroponic_tower_") {
            let Some(tcfg) = tower_cfgs.iter().find(|t| t.id == cfg_key) else { continue };
            let slots = tcfg.slots.max(1);
            // A whole-tower override, or the config's curated plantings cycled.
            let cycle: Vec<String> = if let Some(p) = cfg.tower_overrides.get(&g.id) {
                vec![p.clone()]
            } else {
                tcfg.plantings.iter().map(|p| p.plant.clone()).collect()
            };
            if cycle.is_empty() {
                continue;
            }
            for slot in 0..slots {
                let frac = slot as f32 / (slots.max(2) - 1) as f32;
                stagger(&mut to_spawn, &cycle[slot as usize % cycle.len()], &g.id, slot, frac);
            }
        } else if let Some(plant) = cfg.bed_crops.get(&g.ty) {
            let units = cfg.bed_units.max(1);
            for u in 0..units {
                let frac = u as f32 / (units.max(2) - 1) as f32;
                stagger(&mut to_spawn, plant, &g.id, u, frac);
            }
        }
    }
    if to_spawn.is_empty() {
        return;
    }
    let count = to_spawn.len();
    for c in to_spawn {
        state.game_world.world.spawn((c,));
    }
    log::info!("[Showcase] auto-seeded {count} crops across the grow surfaces");
}

/// Live in-game screenshot command (v0.639): an AI session (or anyone else with no eyes on the
/// running game) drops `debug/screenshot_request.json` and gets back a real capture of the
/// current 3D viewport -- no separate offscreen render, just the frame that was ALREADY drawn
/// this tick, captured right before it hits the screen. Checked once per frame via a plain
/// existence check (cheap on the common absent path -- no read/parse happens unless the file
/// is actually there). The request file is consumed (deleted) whether the capture succeeds or
/// fails, so a failure can never spin retrying forever with no request file and no done file.
pub(crate) fn poll_screenshot_request(
    state: &mut EngineState,
    frame_texture: &wgpu::Texture,
    lists: &SceneDrawLists,
) {
    const REQUEST_PATH: &str = "debug/screenshot_request.json";
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    // v0.810: the request MAY carry {"width":W,"height":H} for a hi-res
    // offscreen capture; any other content keeps the original v0.639
    // contract (window-size swapchain grab). Parsed BEFORE the delete so a
    // read failure degrades to the window capture, never a lost request.
    let size = std::fs::read_to_string(REQUEST_PATH)
        .map(|t| parse_screenshot_request(&t))
        .unwrap_or(ScreenshotSize::Window);
    // Consumed either way: a failed capture must not leave the request file in place, or the
    // next frame (and every frame after) would just retry forever.
    let _ = std::fs::remove_file(REQUEST_PATH);
    execute_screenshot_capture(state, frame_texture, size, lists);
}

/// Showcase-tower dev trigger (v0.862, same file-drop pattern as screenshots):
/// drop `debug/showcase_request.json` containing `{"tower":"nutrition",
/// "plant":"strawberry"}` while the game runs and FarmingSystem replants that
/// tower with the species at staggered growth stages (the procedural-plant
/// demo ladder). Consumed on read. An in-Garden GUI button is tracked as
/// in-app-ops debt (docs/design/in-app-ops.md).
pub(crate) fn poll_showcase_request(state: &mut EngineState) {
    const REQ: &str = "debug/showcase_request.json";
    if !std::path::Path::new(REQ).exists() {
        return;
    }
    let text = std::fs::read_to_string(REQ).unwrap_or_default();
    let _ = std::fs::remove_file(REQ);
    let grab = |key: &str| -> Option<String> {
        let k = format!("\"{key}\"");
        let at = text.find(&k)? + k.len();
        let rest = &text[at..];
        let q0 = rest.find('"')? + 1;
        let q1 = rest[q0..].find('"')? + q0;
        Some(rest[q0..q1].to_string())
    };
    // Plant only when BOTH keys are present; a cam-only request just moves
    // the camera (the perpetual-showcase auto-seed owns default planting).
    if let (Some(tower), Some(plant)) = (grab("tower"), grab("plant")) {
        if let Some(slot) = state
            .data_store
            .get::<std::sync::Mutex<Option<(String, String)>>>("showcase_tower_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some((tower, plant));
            }
        }
    }
    // Optional "time":"9.5" sets the game clock to that hour of the
    // current day (dev/screenshot control: dawn shots without waiting
    // out the night). Routed through the TimeSystem's request channel -
    // its own accumulator is authoritative and re-exports every tick, so
    // writing the DataStore GameTime copy directly is a lost update.
    if let Some(hours) = grab("time").and_then(|t| t.parse::<f32>().ok()) {
        if let Some(req) = state
            .data_store
            .get::<std::sync::Mutex<Option<f32>>>("time_set_hour_request")
        {
            if let Ok(mut r) = req.lock() {
                *r = Some(hours);
            }
        }
    }
    // Optional "sea":"0.8" pins the ocean sea state (0 = glassy calm,
    // 0.5 = ripples, 1 = storm chop + breaking crests) for dev shots;
    // "sea":"auto" returns control to the game weather's wind.
    // Rosette-bisect diagnostic (v0.1249): {"map_diag":"1|2|3|0"} renders a
    // raw march channel into the octa map with the EMA bypassed (1 = first
    // hit t, 2 = direct-sun luminance, 3 = ambient luminance; 0 = normal).
    // Pair with debug/cloudmap_request.json dumps. Dev forensics, permanent.
    // The cloud dev toggles write the GUI-STATE fields (v0.1254.4): the
    // F10 panel edits the same fields and lib.rs mirrors them into the
    // renderer every frame - one source of truth, so buttons and file
    // drops can never disagree.
    if let Some(d) = grab("map_diag").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_map_diag = d as i32;
    }
    // {"cloud_clock":"120"} freezes the cloud advection clock at that many
    // seconds; "-1" returns it live. The clock is app-start-relative, so
    // without this two runs of the same vantage render DIFFERENT cloud
    // fields and no cross-build A/B means anything (measured 2026-09-01:
    // 20% of pixels differed by >40 levels between two runs of one build).
    if let Some(c) = grab("cloud_clock").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_clock_pin = c;
    }
    // {"cloud_chord_foot":"1"} restores the pre-v0.1268 chord-frozen
    // detail scale, so one run can capture both sides of that change.
    if let Some(t) = grab("cloud_chord_foot") {
        state.gui_state.cloud_dev_chord_foot = t == "1";
    }
    // {"cloud_world_shape":"1"} evaluates the shape fields at a fixed
    // world level of detail instead of one keyed on camera distance.
    if let Some(t) = grab("cloud_world_shape") {
        state.gui_state.cloud_dev_world_shape_lod = t == "1";
    }
    // {"cloud_ring_cure":"0"} disables the mip ring cure. It defaults ON
    // and is deliberately NOT tied to cloud_dither any more.
    if let Some(t) = grab("cloud_ring_cure") {
        state.gui_state.cloud_dev_ring_cure_off = t == "0";
    }
    // {"cloud_uniform_step":"1"} / {"cloud_wide_edge":"1"}: the v0.1271
    // estimator experiments (distance-only step; radiative-width edge).
    if let Some(t) = grab("cloud_uniform_step") {
        state.gui_state.cloud_dev_uniform_step = t == "1";
    }
    if let Some(t) = grab("cloud_wide_edge") {
        state.gui_state.cloud_dev_wide_edge = t == "1";
    }
    // {"cloud_edge_mul":"40"} / {"cloud_rind_wide":"600"}: runtime values
    // for the wide-edge experiment (0 = shader constants).
    if let Some(m) = grab("cloud_edge_mul").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_edge_mul = m;
    }
    if let Some(m) = grab("cloud_rind_wide").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_rind_wide_m = m;
    }
    // {"cloud_step_m":"60"}: fixed march step in metres (needs
    // cloud_uniform_step=1); 0 = off. The estimator-bias test.
    if let Some(m) = grab("cloud_step_m").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_step_m = m;
    }
    // {"cloud_shear":"0.577"}: uniform eastward lean, metres per metre of
    // height above the deck base (the prism-wall discriminator); 0 = off.
    if let Some(m) = grab("cloud_shear").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_shear = m;
    }
    // {"cloud_est":"1"}: the sample-anchored march; {"cloud_warp_bl":"1"}:
    // warp band-limited to its own tile + rind/4 refine (v0.1272).
    if let Some(t) = grab("cloud_est") {
        state.gui_state.cloud_dev_est = t == "1";
    }
    if let Some(t) = grab("cloud_warp_bl") {
        state.gui_state.cloud_dev_warp_bl = t == "1";
    }
    // {"cloud_norm_floor":"1"}: carve normaliser floor (design 2A).
    if let Some(t) = grab("cloud_norm_floor") {
        state.gui_state.cloud_dev_norm_floor = t == "1";
    }
    // {"cloud_iso_step":"1"}: isotropic near step + bounded far angle term.
    if let Some(t) = grab("cloud_iso_step") {
        state.gui_state.cloud_dev_iso_step = t == "1";
    }
    // {"cloud_thin_deck":"1"}: band height x0.3 (the prism-wall test).
    if let Some(t) = grab("cloud_thin_deck") {
        state.gui_state.cloud_dev_thin_deck = t == "1";
    }
    // {"cloud_hv_warp":"1"}: height-varying domain warp on the noise body.
    if let Some(t) = grab("cloud_hv_warp") {
        state.gui_state.cloud_dev_hv_warp = t == "1";
    }
    // {"cloud_temporal":"0"} disables the resolve's temporal accumulation
    // OUTRIGHT (every frame runs the snap path: raw march + spatial
    // filter only, no history, no clip, no reprojection); "1" restores.
    // The operator's own bisect request (2026-08-31, the gray striping
    // that "warps the clouds like a black hole"). Works LIVE on a running
    // instance via debug/showcase_request.json. Dev forensics, permanent.
    if let Some(t) = grab("cloud_temporal") {
        state.gui_state.cloud_dev_temporal_off = t == "0";
    }
    // {"cloud_dither":"0"} disables the frozen spatial dither (depth
    // jitter + lod dither) LIVE: smooth cotton interiors, but agate
    // mip-ring arcs on uniform overcast sheets. "1" restores the
    // dithered default (stable fine grain on sheets). The operator's
    // taste toggle until the mip-response calibration lands.
    if let Some(t) = grab("cloud_dither") {
        state.gui_state.cloud_dev_dither_off = t == "0";
    }
    // {"cloud_res":"4|2|1"} sets the cloud march resolution divisor
    // (4 = quarter, the historical default; 1 = full screen resolution).
    if let Some(r) = grab("cloud_res").and_then(|t| t.parse::<u32>().ok()) {
        state.gui_state.cloud_dev_res_div = r.clamp(1, 4);
    }
    // {"cloud_shape":"0"} renders the pre-v0.1256 isotropic ball cluster.
    if let Some(t) = grab("cloud_shape") {
        state.gui_state.cloud_dev_shape_off = t == "0";
    }
    // {"cloud_discard":"1"} paints the composite's discard reasons.
    if let Some(t) = grab("cloud_discard") {
        state.gui_state.cloud_dev_discard_diag = t == "1";
    }
    if let Some(sea) = grab("sea") {
        state.sea_state_override = if sea == "auto" {
            None
        } else {
            sea.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
        };
    }
    // Optional "ocean_event":"tsunami"|"rogue"|"maelstrom"|"hurricane"|"off"
    // (ABYSSAL adoption rung 2): pin an analytic ocean disaster near the
    // player so the rig can photograph a wall of water deterministically.
    // Placement needs the planet-frame anchor, which lives in lib.rs's
    // weather block - so only the request is recorded here.
    if let Some(ev) = grab("ocean_event") {
        let bearing = grab("ocean_event_bearing")
            .and_then(|b| b.parse::<f64>().ok())
            .unwrap_or(std::f64::consts::PI);
        // Optional "ocean_event_distance" in meters (default 900; clamped so
        // a rig cannot pin an event on top of the player or over the horizon).
        let dist = grab("ocean_event_distance")
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(900.0)
            .clamp(50.0, 5000.0);
        state.ocean_event_pin_request = Some((ev, bearing, dist));
    }
    // Optional "weather":"fog" (v0.1059): drive the live weather from the rig,
    // through the SAME DataStore slot the F11 panel writes, so a scripted
    // capture can show fog, a sandstorm or a storm. Without this the rig could
    // never photograph a weather condition at all - every vantage got whatever
    // the sim happened to roll - which is precisely why "fog and sandstorm
    // change nothing in the air" went unnoticed for so long.
    if let Some(w) = grab("weather") {
        let (cond, intensity) = match w.to_ascii_lowercase().as_str() {
            "clear" => (crate::systems::weather::WeatherCondition::Clear, 0.0),
            "cloudy" => (crate::systems::weather::WeatherCondition::Cloudy, 0.4),
            "rain" => (crate::systems::weather::WeatherCondition::Rain, 0.9),
            "storm" => (crate::systems::weather::WeatherCondition::Storm, 1.0),
            "snow" => (crate::systems::weather::WeatherCondition::Snow, 0.7),
            "fog" => (crate::systems::weather::WeatherCondition::Fog, 1.0),
            "sandstorm" => (crate::systems::weather::WeatherCondition::Sandstorm, 1.0),
            _ => (crate::systems::weather::WeatherCondition::Clear, 0.0),
        };
        if let Some(m) = state
            .data_store
            .get::<std::sync::Mutex<crate::systems::weather::WeatherControl>>("weather_control")
        {
            if let Ok(mut c) = m.lock() {
                c.manual = Some(crate::systems::weather::ManualWeather {
                    condition: cond,
                    intensity,
                    wind_speed: if intensity > 0.8 { 18.0 } else { 4.0 },
                });
                c.retrigger = true;
            }
        }
        state.gui_state.weather_manual = true;
        state.gui_state.weather_pick_condition = cond;
        state.gui_state.weather_pick_intensity = intensity;
        log::info!("Showcase: weather -> {w}");
    }
    // Optional "lights":"500": N camera-pinned test point lights (clustering
    // dev-aid; the grid regenerates around the camera every frame, so it
    // survives floating-origin rebases). "lights":"0" clears. Optional
    // "lights_tiled":"1"/"0" flips the tiled-light-list SETTING live so the
    // rig measures both paths in one session.
    if let Some(n) = grab("lights").and_then(|v| v.parse::<usize>().ok()) {
        state.debug_test_light_count = n.min(2048);
        log::info!("Test lights: {} (camera-pinned)", state.debug_test_light_count);
    }
    if let Some(iv) = grab("lights_intensity").and_then(|v| v.parse::<f32>().ok()) {
        state.debug_test_light_intensity = iv.clamp(0.1, 200.0);
        log::info!("Test light intensity: {}", state.debug_test_light_intensity);
    }
    // "gpu_precip":"1"/"0" flips the experimental GPU particle SETTING live so
    // the probe rig can exercise that path (BUG-050 was invisible to every rig
    // run because the portable sandbox boots with the default config, gpu
    // particles off). Same live-flip pattern as lights_tiled below.
    if let Some(t) = grab("gpu_precip") {
        state.gui_state.settings.gpu_particles = t == "1";
        log::info!("Showcase: gpu_particles -> {}", state.gui_state.settings.gpu_particles);
    }
    if let Some(t) = grab("lights_tiled") {
        state.gui_state.settings.lights_tiled = t == "1";
        log::info!("Tiled light lists: {}", state.gui_state.settings.lights_tiled);
    }
    // Optional "clouds":"0"/"1" flips the planet cloud layer live - the
    // A/B knob that separates cloud-deck artifacts from water/sky ones
    // (v0.958, the grazing horizon sheet-band hunt).
    if let Some(c) = grab("clouds") {
        state.gui_state.settings.planet_clouds = c == "1";
        log::info!("Planet clouds: {}", state.gui_state.settings.planet_clouds);
    }
    // Optional "cloud_cover":"0.75" pins the cloud deck's effective
    // coverage (clouds depth increment); "cloud_cover":"auto" returns
    // control to live MODIS weather + event boosts. A cloud-verification
    // vantage must not depend on the real sky being cloudy on capture day.
    if let Some(c) = grab("cloud_cover") {
        state.cloud_cover_override = if c == "auto" {
            None
        } else {
            c.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
        };
        log::info!("Showcase: cloud_cover -> {:?}", state.cloud_cover_override);
    }
    // Optional "cloud_type":"0.4" pins the cloud TYPE coordinate (0 =
    // cirrus, 0.34 = cumulus, 0.5 = cumulonimbus, 0.67 = stratus, 1 =
    // stratocumulus); "auto" restores the natural field. Only takes
    // effect together with cloud_cover (the params2.w encoding requires
    // the coverage pin) - it exists so the from-below cloud gates measure
    // a KNOWN family.
    if let Some(c) = grab("cloud_type") {
        state.cloud_type_override = if c == "auto" {
            None
        } else {
            c.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
        };
        log::info!("Showcase: cloud_type -> {:?}", state.cloud_type_override);
    }
    // Optional "cloud_quality":"low"/"medium"/"high" pins the cloud tier
    // live (clouds depth increment): Medium now consumes the same
    // per-planet slab bounds as High, and BUG-049 taught us that a tier
    // with zero rig coverage rots silently - this key is what lets a
    // vantage photograph the Medium march at all.
    if let Some(q) = grab("cloud_quality") {
        state.gui_state.settings.cloud_quality = q.to_ascii_lowercase();
        log::info!(
            "Showcase: cloud_quality -> {}",
            state.gui_state.settings.cloud_quality
        );
    }
    // Optional "bake":"trees" runs the automated billboard sprite baker
    // (v0.959) over EVERY species in data/vegetation/trees.ron (v0.1083, was
    // the six hardcoded conifers): each tile renders side-on into
    // debug/bakes/tileNN_<id>_vN.png with a transparent background. The
    // verification surface for the model-to-card pipeline, and the way to
    // eyeball all 24 tiles without spending a probe-rig run.
    if grab("bake").as_deref() == Some("trees") {
        let out_dir = std::path::Path::new("debug").join("bakes");
        // Model-backed species need their glTF parsed; procedural ones build
        // themselves inside the baker.
        let mut models: std::collections::HashMap<
            String,
            crate::renderer::billboard_bake::BakeCpuModel,
        > = std::collections::HashMap::new();
        let t_parse = std::time::Instant::now();
        for t in crate::renderer::tree_mesh::registry().trees.iter() {
            if t.is_procedural() {
                continue;
            }
            for v in 1..=t.variants.max(1) {
                for suffix in ["", "_bark"] {
                    let rel = format!(
                        "assets/models/plants/{m}/{m}_v{v}{suffix}.gltf",
                        m = t.model
                    );
                    match state.asset_manager.parse_gltf_mesh_with_texture(&rel) {
                        Ok((cpu, tex)) => {
                            models.insert(
                                rel,
                                crate::renderer::billboard_bake::BakeCpuModel {
                                    vertices: cpu.vertices,
                                    indices: cpu.indices,
                                    texture: tex,
                                },
                            );
                        }
                        Err(e) => log::warn!("[Bake] {rel}: {e}"),
                    }
                }
            }
        }
        let parse_ms = t_parse.elapsed().as_secs_f32() * 1000.0;
        let report = state
            .renderer
            .bake_tree_atlas_from_registry(&models, parse_ms, Some(&out_dir));
        log::info!(
            "[Bake] dump -> {} ({} tiles)",
            out_dir.display(),
            report.tiles_baked
        );
    }
    // Optional "enter":"default" mimics the Play button: replay the last
    // WHO/WHERE pairing into the world (machines + garden come alive),
    // falling back to the character picker when none is recorded yet.
    if grab("enter").is_some() {
        if !state.gui_state.apply_last_pairing() {
            state.gui_state.launcher_open_select = true;
        }
        state.gui_state.active_page = crate::gui::GuiPage::None;
    }
    // Optional camera framing in the same request: "cam":"x,y,z,yaw,pitch"
    // parks the free camera there and drops into the world view (page None)
    // so a follow-up screenshot_request captures the framed 3D scene.
    if let Some(cam) = grab("cam") {
        let v: Vec<f32> = cam.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if v.len() == 5 {
            // In first person the camera derives from the PLAYER each
            // frame, so teleport the player body; yaw/pitch live on the
            // camera itself (mouse deltas accumulate onto them) so those
            // stick when set directly.
            for (_e, (t, _c)) in state
                .game_world
                .world
                .query_mut::<(
                    &mut crate::ecs::components::Transform,
                    &crate::ecs::components::Controllable,
                )>()
            {
                t.position = Vec3::new(v[0], v[1], v[2]);
            }
            state.camera.position = Vec3::new(v[0], v[1], v[2]);
            state.camera.yaw = v[3];
            state.camera.pitch = v[4];
            state.gui_state.active_page = crate::gui::GuiPage::None;
        }
    }
}

/// Drive the live broadcast for one frame (v0.853). Called right before
/// `present()`, so the captured frame is exactly what the operator sees.
///
/// Three jobs, all of them cheap and none of them blocking:
///   1. Act on a start/stop request from the Studio page.
///   2. Collect a readback that landed, and hand it to the encoder thread.
///   3. Kick off the next readback, but only if the encoder actually wants a
///      frame -- at 15 fps we skip the GPU copy entirely on ~3 of every 4 frames.
///   4. Mirror the publisher's real counters back so the page draws truth.
///
/// When nothing is being broadcast this is a single `Option` check.
pub(crate) fn pump_live_broadcast(state: &mut EngineState, frame_texture: &wgpu::Texture) {
    use crate::net::live::{LiveConfig, LivePublisher};
    use std::sync::atomic::Ordering;

    // --- 1. Start / stop, requested by the Studio page.
    match state.gui_state.studio.broadcast_request.take() {
        Some(true) => {
            let Some(seed) = state.gui_state.private_key_bytes.clone() else {
                state.gui_state.studio.broadcast_error =
                    "No identity loaded. Sign in first, then go live.".to_string();
                state.gui_state.studio.is_live = false;
                return;
            };
            if !state.renderer.supports_frame_capture() {
                state.gui_state.studio.broadcast_error =
                    "This graphics backend cannot capture the window, so there is nothing to \
                     broadcast."
                        .to_string();
                state.gui_state.studio.is_live = false;
                return;
            }

            // The Studio server field is a chat WS URL ("wss://host/ws"); the live
            // publisher wants the plain origin.
            let server = state
                .gui_state
                .studio
                .stream_server_url
                .trim_end_matches('/')
                .trim_end_matches("/ws")
                .replace("wss://", "https://")
                .replace("ws://", "http://");

            // Parse the height out of the "WIDTHxHEIGHT" picker value rather than
            // prefix-matching a fixed list (the old match silently mapped 1080p,
            // 1440p, and 4K all to 720). See LiveConfig::height_from_resolution.
            let target_height = crate::net::live::LiveConfig::height_from_resolution(
                &state.gui_state.studio.stream_resolution,
            );
            // MJPEG has no rate controller: you set image quality and the bitrate
            // falls out of it. So map the operator's kbps target onto a JPEG
            // quality, clamped to a sane band (below ~45 it looks like a fax, above
            // ~92 the file size explodes for no visible gain). A real rate
            // controller comes with the H.264 encoder.
            let quality = (40 + state.gui_state.studio.stream_bitrate / 120).clamp(45, 92) as u8;
            let cfg = LiveConfig {
                server,
                title: state.gui_state.studio.stream_key.clone(),
                // Empty = the relay binds the default #live-<name> room. A
                // picker at Go Live is the next rung of the studio-watch plan.
                chat: String::new(),
                target_height,
                quality,
                fps: state.gui_state.studio.stream_fps.clamp(5, 30),
            };
            state.live_publisher = Some(LivePublisher::start(cfg, &seed));
        }
        Some(false) => {
            // Dropping the publisher signals its worker to close the socket.
            state.live_publisher = None;
            state.gui_state.studio.broadcast_live = false;
            state.gui_state.studio.broadcast_url.clear();
        }
        None => {}
    }

    let Some(pubr) = state.live_publisher.as_mut() else {
        return;
    };

    // --- 2. Collect a landed readback and feed the encoder.
    if let Some(frame) = state.stream_capture.poll(&state.renderer.device) {
        pubr.submit_frame(frame);
    }

    // --- 3. Start the next one, but ONLY if the encoder is due a frame. The GPU
    // copy is the expensive part; skipping it on frames we would drop anyway is
    // most of the reason this costs nothing at 60 fps.
    if pubr.wants_frame() {
        state.stream_capture.submit(
            &state.renderer.device,
            &state.renderer.queue,
            frame_texture,
        );
    }

    // --- 4. Mirror real counters into the page.
    let stats = pubr.stats();
    let frames_sent = stats.sent.load(Ordering::Relaxed);
    let studio = &mut state.gui_state.studio;
    // ON AIR means bytes are actually reaching the relay, which the badge and its
    // tooltip promise. The socket being connected is NOT that: the relay accepts
    // the publisher before the first frame is captured, so gate ON AIR on at least
    // one frame actually sent. Until then the page shows "Connecting...".
    studio.broadcast_live =
        stats.connected.load(Ordering::Relaxed) && frames_sent > 0;
    studio.broadcast_viewers = stats.viewers.load(Ordering::Relaxed);
    studio.broadcast_frames = frames_sent;
    studio.broadcast_dropped = stats.dropped.load(Ordering::Relaxed);
    studio.broadcast_kbps = pubr.kbps();

    if let Ok(err) = stats.error.lock() {
        if !err.is_empty() && studio.broadcast_error.is_empty() {
            studio.broadcast_error = err.clone();
        }
    }
    if studio.broadcast_url.is_empty() {
        if let Ok(id) = stats.stream_id.lock() {
            if !id.is_empty() {
                let base = state
                    .gui_state
                    .studio
                    .stream_server_url
                    .trim_end_matches('/')
                    .trim_end_matches("/ws")
                    .replace("wss://", "https://")
                    .replace("ws://", "http://");
                state.gui_state.studio.broadcast_url = format!("{base}/watch?s={id}");
            }
        }
    }

    // A publisher that died must not leave the UI claiming to be on air.
    if !state.gui_state.studio.broadcast_error.is_empty() {
        state.live_publisher = None;
        state.gui_state.studio.broadcast_live = false;
        state.gui_state.studio.is_live = false;
    }
}

/// Shared executor for every capture trigger (file protocol AND the Testing
/// page buttons, v0.810): bumps the counter, runs the right capture path,
/// writes debug/screenshot_done.json, and mirrors the outcome into the GUI's
/// last-result line so the in-app buttons give feedback without a file read.
pub(crate) fn execute_screenshot_capture(
    state: &mut EngineState,
    frame_texture: &wgpu::Texture,
    size: ScreenshotSize,
    lists: &SceneDrawLists,
) {
    const DONE_PATH: &str = "debug/screenshot_done.json";
    state.screenshot_counter += 1;
    let out_path = screenshot_output_path(state.screenshot_counter);
    // Ok(None) = window capture (size implicit); Ok(Some((w,h))) = hi-res
    // capture at the ACTUAL rendered size (post-clamp), reported in done.json.
    let result: Result<Option<(u32, u32)>, String> = match size {
        ScreenshotSize::Window => state
            .renderer
            .capture_current_frame(frame_texture, std::path::Path::new(&out_path))
            .map(|()| None),
        ScreenshotSize::Custom(w, h) => {
            capture_hires_screenshot(state, w, h, lists, &out_path).map(Some)
        }
        ScreenshotSize::Invalid(msg) => Err(msg),
    };
    let mut done = screenshot_done_json(&result, &out_path);
    // Perf snapshot rides along (v0.815): the capture protocol is how
    // scripted/AI sessions see the running game, so the done file also
    // reports the live frame rate -- instantaneous fps plus the mean of
    // the F2 overlay's 120-frame ring buffer -- letting a shader-cost
    // change be measured from the same file drop that verifies it
    // visually. Note the hi-res offscreen capture itself is excluded
    // from the average (it happens after these frames were timed).
    if let Some(obj) = done.as_object_mut() {
        let ft = &state.gui_state.frame_times;
        let avg_ms = if ft.is_empty() {
            0.0
        } else {
            ft.iter().sum::<f32>() / ft.len() as f32
        };
        obj.insert("fps".to_string(), serde_json::json!(state.gui_state.fps));
        obj.insert("frame_ms_avg".to_string(), serde_json::json!(avg_ms));
    }
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(DONE_PATH, done.to_string());
    // Reference-march scene dump (environment program increment 10): write
    // everything renderer::cloud_reference needs to re-march the EXACT
    // captured scene on the CPU - the cloud shell state stashed at the
    // material fill site, plus camera pose/fov, the world sun, the aerial
    // sky hue the two-tone ambient reads, and the cloud clock. One file per
    // capture beside the done file; consumers pair it with the PNG.
    if let Some(shell) = state.cloud_ref_frame.as_ref() {
        let (sun_dir, sun_col, sun_int) = state.renderer.cloud_ref_sun();
        let cam = &state.camera;
        let cam_p = cam.effective_position();
        let dump = format!(
            concat!(
                "{{\"shell\":{},\"clock\":{},",
                "\"sun_dir\":[{},{},{}],\"sun_color\":[{},{},{}],",
                "\"sun_intensity\":{},\"aerial_sky\":[{},{},{}],",
                "\"cam_pos\":[{},{},{}],",
                "\"cam_fwd\":[{},{},{}],\"cam_right\":[{},{},{}],",
                "\"fov_deg\":{},\"aspect\":{},",
                "\"viewport\":[{},{}],\"capture\":\"{}\"}}"
            ),
            shell,
            state.start_time.elapsed().as_secs_f32(),
            sun_dir[0], sun_dir[1], sun_dir[2],
            sun_col[0], sun_col[1], sun_col[2],
            sun_int,
            state.renderer.aerial_sky[0],
            state.renderer.aerial_sky[1],
            state.renderer.aerial_sky[2],
            cam_p.x, cam_p.y, cam_p.z,
            // BASIS VECTORS, not yaw/pitch: the camera has two bases
            // (world + surface tangent) and the dump must be
            // reconstruction-ambiguity-free. True up = right x fwd.
            cam.forward().x, cam.forward().y, cam.forward().z,
            cam.right().x, cam.right().y, cam.right().z,
            cam.fov_degrees, cam.aspect,
            state.renderer.viewport_size().0,
            state.renderer.viewport_size().1,
            out_path,
        );
        let _ = std::fs::write("debug/cloud_ref_dump.json", dump);
    }
    state.gui_state.screenshot_last_result = Some(match &result {
        Ok(None) => format!("Saved {out_path} (window size)"),
        Ok(Some((w, h))) => format!("Saved {out_path} ({w}x{h})"),
        Err(e) => format!("Capture failed: {e}"),
    });
}

/// Hi-res offscreen capture (v0.810): renders ONE frame of the exact current
/// view at `req_w` x `req_h`, independent of the window size, and saves it as
/// a PNG. The camera position/orientation are untouched; only the projection
/// aspect follows the requested W/H. Approach: create an offscreen color
/// target in the swapchain's format, size the shared depth buffer to match,
/// re-run the normal scene passes (they are all target-agnostic), read the
/// pixels back, then restore the window-sized depth buffer -- one frame of
/// extra GPU work, no visible hiccup (the swapchain frame was already
/// composed). The GUI/HUD is deliberately NOT drawn into the capture: the
/// hi-res path exists for wallpapers and sharing, so it ships the clean
/// scene. Returns the ACTUAL rendered size, which differs from the request
/// only when clamped to the device's max texture dimension.
pub(crate) fn capture_hires_screenshot(
    state: &mut EngineState,
    req_w: u32,
    req_h: u32,
    lists: &SceneDrawLists,
    out_path: &str,
) -> Result<(u32, u32), String> {
    if !state.world_loaded {
        return Err(
            "3D world not loaded -- enter the world once, then capture".to_string(),
        );
    }
    // Clamp to the device's REAL capability (queried, never hardcoded). If
    // either edge exceeds the limit, scale BOTH edges down proportionally so
    // the aspect ratio -- and thus the framing -- survives the clamp. A
    // request beyond 4x the limit on either edge is a typo, not a wallpaper:
    // reject it outright, naming the device's number so the caller can retry.
    let max_dim = state.renderer.max_texture_dimension_2d().max(1);
    if req_w > max_dim.saturating_mul(4) || req_h > max_dim.saturating_mul(4) {
        return Err(format!(
            "requested {req_w}x{req_h} is far beyond this device's max texture dimension ({max_dim})"
        ));
    }
    let scale = f64::min(
        1.0,
        f64::min(max_dim as f64 / req_w as f64, max_dim as f64 / req_h as f64),
    );
    let w = ((req_w as f64 * scale) as u32).clamp(1, max_dim);
    let h = ((req_h as f64 * scale) as u32).clamp(1, max_dim);

    let (capture_tex, capture_view) = state.renderer.create_capture_target(w, h);
    let (win_w, win_h) = state.renderer.surface_size();
    state.renderer.set_depth_target_size(w, h);
    let old_aspect = state.camera.aspect;
    state.camera.aspect = w as f32 / h.max(1) as f32;

    // Pass 1: sky -- galaxy glow, stars, halos, constellations -- exactly as
    // the live frame draws it (clear to black + star renderer). If the star
    // renderer is somehow absent the clear still runs, so the target is
    // never undefined.
    {
        if let Some(ref star_r) = state.star_renderer {
            star_r.update_camera(
                                        &state.renderer.queue,
                                        &state.camera,
                                        crate::station::render_to_world_rot(
                                            state.station_ride,
                                            state.station_world_rot,
                                        )
                                        .as_quat(),
                                    );
        }
        let mut encoder = state.renderer.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("HiRes Star Encoder") },
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HiRes Star Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &capture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            if let Some(ref star_r) = state.star_renderer {
                // Menu/backdrop: always the full sky (no atmosphere in front).
                star_r.render_pass(&mut pass, false);
            }
        }
        state.renderer.queue.submit(std::iter::once(encoder.finish()));
    }

    // Passes 1.5 .. 2.7: identical order + inputs to the live frame render
    // (see the in-game branch above `match scene_result`). The sun direction
    // is recomputed from the same state fields the live path uses.
    let sun_dir_f = {
        let d = (state.sun_world_pos - state.ship_world_pos).normalize_or_zero();
        Vec3::new(d.x as f32, d.y as f32, d.z as f32)
    };
    state.renderer.render_celestial_onto(
        &state.camera,
        lists.celestial,
        lists.celestial_transparent,
        sun_dir_f,
        // Same clock as the live path: cloud decks drift with time, and a
        // capture should freeze the exact frame the player is looking at.
        state.start_time.elapsed().as_secs_f32(),
        cloud_ground_params(state),
        ground_anchor(state),
        ocean_anchor256(state),
        &capture_view,
    );
    state
        .renderer
        .draw_celestial_lines_onto(&state.camera, lists.orbit_lines, &capture_view);
    // God rays in captures too (v0.895) — same slot as the live path, so
    // screenshots show exactly what the player sees.
    state
        .renderer
        .render_godrays_onto(&state.camera, sun_dir_f, &capture_view, godray_scale(state));
    state.renderer.render_ssao_onto(&state.camera, &capture_view);
    state
        .renderer
        .render_scene_onto(&state.camera, lists.opaque, &capture_view);
    state
        .renderer
        .render_transparent_onto(&state.camera, lists.transparent, &capture_view);
    state
        .renderer
        .render_overlay_onto(&state.camera, lists.overlay, &capture_view);
    state
        .renderer
        .draw_lines_onto(&state.camera, lists.ring_lines, &capture_view);

    let result = state.renderer.read_texture_to_png(
        &capture_tex,
        w,
        h,
        std::path::Path::new(out_path),
    );

    // Restore the window-sized internals BEFORE returning (even on a readback
    // failure), or the next live frame would bind a mismatched depth buffer.
    state.camera.aspect = old_aspect;
    state.renderer.set_depth_target_size(win_w, win_h);

    result.map(|()| (w, h))
}

/// Dev camera request (v0.813): see the call site in the frame loop for the
/// full contract. Reuses the Dev Travel tool's math (`dev_travel::
/// teleport_viewpoint` for the sunlit direction, rescaled to the requested
/// altitude; `look_angles` for the aim) so scripted captures and the GUI
/// tool can never drift apart.
/// Far-frame repro knob (v0.1238): mimic a camera FLOWN here from the
/// homestead instead of teleported. Flight accumulates the whole journey into
/// the f32 `camera.position` (there is no floating-origin rebase; the camera
/// stays in ship-local coords), so after a ~36,000 km flight the camera sits
/// at x of about 3.6e7 m, where one f32 step (ulp) is 4 m, and every shader
/// quantity derived from `camera.view_pos` or a mesh `world_position` inherits
/// that axis-aligned quantization (the operator's cardinal-locked cloud
/// starburst at the feet). A teleport parks the camera ~30 m from the origin,
/// which is why the rig could never reproduce what the operator flies into.
/// This knob re-splits the SAME absolute pose: `camera.position` takes the
/// distance, `ship_world_pos` gives it back, and the split persists because
/// the frame lock recomputes `ship_world_pos` by SUBTRACTING `cam_local`
/// every frame (frame_lock_ship_pos). Permanent dev tooling: the probe rig's
/// only way to see flown-state f32 artifacts without a 20-minute flight.
fn apply_far_frame(state: &mut EngineState, v: &serde_json::Value) {
    // Rig descent (v0.1245): {"descend_mps": N} makes the probe hold sink
    // the pin N m/s down the local radial - sustained flight for the
    // temporal cloud map's epipolar-smear forensics. Absent = 0 (parked),
    // so every park resets it.
    state.probe_descend_mps = v
        .get("descend_mps")
        .and_then(|a| a.as_f64())
        .unwrap_or(0.0) as f32;
    let Some(d_km) = v.get("far_frame_km").and_then(|a| a.as_f64()) else {
        return;
    };
    let d = d_km * 1000.0;
    state.ship_world_pos.x -= d;
    state.camera.position.x += d as f32;
    // The probe hold pins the LOCAL camera offset; re-pin the shifted one or
    // the hold drags the camera straight back next frame.
    if let Some((_, t)) = state.probe_hold.take() {
        state.probe_hold = Some((state.camera.position, t));
    }
    log::info!(
        "Camera request: far-frame split applied ({d_km:.0} km into camera.position; cam x = {:.0} m)",
        state.camera.position.x
    );
}

/// Octa cloud-map dump (v0.1246 dev forensics): drop
/// `debug/cloudmap_request.json` (any content) while the game runs and the
/// CURRENT accumulated 4096^2 map is written to `debug/cloudmap_N.png` as a
/// double-wide rgb|alpha panel, with `debug/cloudmap_done.json` reporting
/// the path. The starburst discriminator: fibres in this file = content
/// side; clean file = sampling/display side.
pub(crate) fn poll_cloudmap_request(state: &mut EngineState) {
    const REQUEST_PATH: &str = "debug/cloudmap_request.json";
    const DONE_PATH: &str = "debug/cloudmap_done.json";
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let _ = std::fs::remove_file(REQUEST_PATH);
    state.screenshot_counter += 1;
    let out = format!("debug/cloudmap_{}.png", state.screenshot_counter);
    let done = match state
        .renderer
        .dump_cloud_map_png(std::path::Path::new(&out))
    {
        Ok((w, h)) => serde_json::json!({"ok": true, "path": out, "w": w, "h": h}),
        Err(e) => serde_json::json!({"ok": false, "error": e}),
    };
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(DONE_PATH, done.to_string());
}

pub(crate) fn poll_camera_request(state: &mut EngineState) {
    const REQUEST_PATH: &str = "debug/camera_request.json";
    const DONE_PATH: &str = "debug/camera_done.json";
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let parsed = std::fs::read_to_string(REQUEST_PATH)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    // Consumed either way (same rule as the other debug polls).
    let _ = std::fs::remove_file(REQUEST_PATH);
    let fail = |msg: String| {
        log::warn!("Camera request: {msg}");
        let _ = std::fs::create_dir_all("debug");
        let _ = std::fs::write(
            DONE_PATH,
            serde_json::json!({"ok": false, "error": msg}).to_string(),
        );
    };
    let Some(v) = parsed else {
        fail("malformed JSON".to_string());
        return;
    };
    if !state.world_loaded {
        fail("3D world not loaded".to_string());
        return;
    }
    // Station vantage (v0.1225): {"station":"home"} parks the camera ABOARD
    // the orbital homestead and leaves it there. Every other path in this
    // function teleports to a body-relative vantage, which is why the rig has
    // never been able to photograph the home at all - the moment it asked for
    // a camera it was somewhere else. Without this verb there is no way to
    // hold a fixed pose on the hull across several clock times, which is
    // exactly the A/B the homestead lighting needs.
    //
    // Optional "time" here is the GLOBAL clock hour, not a local solar one:
    // a station has no longitude to be local to (the lat/lon conversion
    // further down deliberately does not apply).
    if v.get("station").is_some() {
        if let Some(hours) = v.get("time").and_then(|a| a.as_f64()) {
            if let Some(req) = state
                .data_store
                .get::<std::sync::Mutex<Option<f32>>>("time_set_hour_request")
            {
                if let Ok(mut r) = req.lock() {
                    *r = Some(hours as f32);
                }
            }
        }
        if state.dev_travel_home.is_none() {
            state.dev_travel_home = Some((
                state.ship_world_pos,
                state.camera.position,
                state.camera.yaw,
                state.camera.pitch,
            ));
        }
        // Ride the station, and make sure no PLANET frame lock is left
        // engaged: aboard the home we are not standing on a surface, and a
        // stale lock leaves the double-writer live (see the station block in
        // lib.rs) and drives aerial_up off a stale anchor.
        state.ship_world_pos = state.station_world_pos;
        state.station_ride = true;
        state.aboard_station = true;
        state.frame_lock_body = None;
        state.frame_lock_anchor = glam::DVec3::ZERO;
        if state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson {
            state
                .camera
                .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        }
        // A deliberately FIXED pose over the deck: the whole point is that
        // several captures differ only by the clock, so the camera must not
        // vary by so much as a pixel between them.
        let hull_top = state.homestead_bounds.map(|(_, mx)| mx.y).unwrap_or(20.0);
        state.camera.position = Vec3::new(0.0, hull_top + 14.0, 34.0);
        state.camera.clear_surface();
        let (yaw, pitch) = crate::dev_travel::look_angles(
            glam::DVec3::new(0.0, -0.34, -1.0).normalize(),
        );
        state.camera.yaw = yaw;
        state.camera.pitch = pitch;
        state.gui_state.dev_fly_mode = true;
        state.controller.fly_mode = true;
        // GRAVITY OFF, exactly as F9 does it (v0.1269, operator catch). The
        // PHYSICS reads gui_state.dev_hover - MoveMode::from_dev_flight(dev_hover)
        // in lib.rs - not dev_fly_mode, so setting only the two flags above left
        // the movement model in Walk: gravity pulled and the ground clamped, and
        // a camera placed at altitude SANK during the settle. Every probe-rig
        // capture in the cloud arc was taken below its requested altitude (5.9 km
        // requested, 5.7 captured; 3.4 requested, 3.2 captured) and every rig HUD
        // read WALK. surface_vr is zeroed so no inherited fall velocity carries in.
        state.gui_state.dev_hover = true;
        state.surface_vr = 0.0;
        state.gui_state.dev_travel_away = false;
        state.probe_hold = Some((state.camera.position, std::time::Instant::now()));
        let _ = std::fs::create_dir_all("debug");
        let _ = std::fs::write(
            DONE_PATH,
            serde_json::json!({"ok": true, "station": "home"}).to_string(),
        );
        log::info!("Camera request: parked aboard the home station");
        return;
    }
    // Bookmark restore (v0.890): {"bookmark":"bm-N"} places the camera at
    // an F6-saved exact pose and skips the scenic/altitude path entirely.
    if restore_location_bookmark(state, &v) {
        return;
    }
    // Named scenic view (v0.825): {"view":"Oahu Coast"} loads the curated
    // coordinate from data/scenic_views.ron and supplies the body / lat /
    // lon / altitude / aim, so a Place can be jumped to by name. Any
    // explicit field in the JSON still overrides the view's value.
    let scenic = v.get("view").and_then(|s| s.as_str()).and_then(|name| {
        let path = state.asset_manager.data_dir().join("scenic_views.ron");
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| crate::scenic_views::ScenicViews::from_ron(&t))
        {
            Ok(views) => match views.find(name) {
                Some(view) => Some(view.clone()),
                None => {
                    log::warn!("Camera request: no scenic view named {name}");
                    None
                }
            },
            Err(e) => {
                log::warn!("Camera request: scenic_views.ron: {e}");
                None
            }
        }
    });
    let body_id = v
        .get("body")
        .and_then(|b| b.as_str())
        .map(|s| s.to_string())
        .or_else(|| scenic.as_ref().map(|s| s.body.clone()))
        .unwrap_or_else(|| "earth".to_string());
    let altitude_km = v
        .get("altitude_km")
        .and_then(|a| a.as_f64())
        .or_else(|| scenic.as_ref().map(|s| s.altitude_km as f64))
        .unwrap_or(12_000.0)
        // Negative altitude = UNDERWATER camera (v0.1017, water arc): the
        // probe rig needs to verify the sea surface from below. Clamped to
        // -450 m (deeper than any visual question; well above crush-depth
        // silliness like the planet core).
        .max(-0.45);
    let look_offset_deg = v
        .get("look_offset_deg")
        .and_then(|a| a.as_f64())
        .or_else(|| scenic.as_ref().map(|s| s.look_offset_deg as f64))
        .unwrap_or(0.0);
    // Optional lat/lon (v0.824 scenic camera): pin the vantage to a REAL
    // surface coordinate (matching the heightmap/albedo grid convention)
    // instead of the sunlit-heuristic vantage - so a camera can sit over a
    // named coastline/mountain. Both must be present to take effect.
    let latlon = match (v.get("lat").and_then(|a| a.as_f64()), v.get("lon").and_then(|a| a.as_f64())) {
        (Some(la), Some(lo)) => Some((la, lo)),
        _ => scenic.as_ref().map(|s| (s.lat as f64, s.lon as f64)),
    };
    // Optional "time": the LOCAL mean solar hour at this vantage's
    // longitude (sun-clock fix, 2026-08-18). The game clock is lon-0
    // solar time by construction (dev_travel::planet_spin_from_time), so
    // "local noon at Silverdale" means global hour 12 + 122.7/15 = 20.2
    // - a conversion vantage authors were doing BY HAND (every lit
    // Silverdale vantage pinned exactly 20.2). Writing the local hour
    // here converts it; the showcase "time" verb stays GLOBAL (it has no
    // site to be local to) and this later write wins over it when a
    // vantage carries both.
    if let (Some(t_local), Some((_, lon))) =
        (v.get("time").and_then(|a| a.as_f64()), latlon)
    {
        if let Some(req) = state
            .data_store
            .get::<std::sync::Mutex<Option<f32>>>("time_set_hour_request")
        {
            if let Ok(mut r) = req.lock() {
                *r = Some(((t_local - lon / 15.0).rem_euclid(24.0)) as f32);
            }
        }
    }
    let Some(body) = crate::cosmos::find_body(&body_id) else {
        fail(format!("unknown body id {body_id}"));
        return;
    };
    // Same live sim time + Earth-relative frame as the Travel tool, so the
    // vantage matches where the body is drawn this frame.
    let sim_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0;
    let earth_helio_au = crate::cosmos::find_body("earth")
        .map(|e| crate::cosmos::body_world_position_3d_au(e, sim_t))
        .unwrap_or(glam::DVec3::ZERO);
    let body_rel_earth = (crate::cosmos::body_world_position_3d_au(body, sim_t)
        - earth_helio_au)
        * crate::cosmos::M_PER_AU;
    let sun_rel_earth = -earth_helio_au * crate::cosmos::M_PER_AU;
    let radius_m = body.radius_km * 1000.0;
    // dir_out = the outward direction from the body centre to the camera.
    // Scenic (lat/lon): the surface radial at that coordinate, rotated into
    // the world frame by the planet's current spin so it lands on the real
    // ground feature. Otherwise the sunlit-heuristic Travel vantage.
    let dir_out = if let Some((lat, lon)) = latlon {
        let spin = state.current_spin;
        let d = crate::terrain::planet_heightmap::latlon_to_dir(lat as f32, lon as f32);
        let world = glam::DQuat::from_rotation_y(spin)
            * glam::DVec3::new(d.x as f64, d.y as f64, d.z as f64);
        world.normalize_or_zero()
    } else {
        let base = crate::dev_travel::teleport_viewpoint(
            body_rel_earth,
            sun_rel_earth,
            radius_m,
            body.body_type == "star",
        );
        (base - body_rel_earth).normalize_or_zero()
    };
    // Park above the DRAWN ground: for a lat/lon (scenic surface) target
    // sample the real terrain radius there (heightmap + vertical
    // exaggeration), so `altitude_km` is height above the actual ground -
    // parking at the bare sphere radius would drop the camera tens of km
    // UNDER a mountain (Everest's exaggerated relief is ~22 km). Orbit
    // (no lat/lon) targets keep the sphere radius; there is no terrain up
    // there to clear.
    let mut surface_radius = if let Some((lat, lon)) = latlon {
        let unit = crate::terrain::planet_heightmap::latlon_to_dir(lat as f32, lon as f32);
        // Tile-aware parking (v0.885): sample the DRAWN elevation (base +
        // detail noise + streamed tiles when resident) so a low park over
        // a tile-data peak (Fuji) starts ABOVE the drawn cone instead of
        // inside it. ensure_region is a cheap no-op when already
        // resident; if the tile is not loaded yet, the detail-inclusive
        // base still beats the old bare-grid estimate, and the surface
        // clamp heals the remainder once tiles stream in.
        if body_id == "earth" {
            state.terrain_tiles.ensure_region(lat as f32, lon as f32);
            let _ = state.terrain_tiles.poll();
        }
        let detail = state
            .planet_defs
            .get(&body_id)
            .map(|d| crate::terrain::planet_chunks::DetailNoise::new(d.terrain_seed));
        let tiles = (body_id == "earth" && state.terrain_tiles.tier_installed())
            .then_some(&state.terrain_tiles);
        ground_radius_m(
            state.planet_defs.get(&body_id),
            state.planet_heightmaps.get(&body_id).map(|a| a.as_ref()),
            detail.as_ref(),
            tiles,
            unit.as_dvec3(),
        )
    } else {
        radius_m
    };
    // Ocean parking (v0.894): over connected ocean the sampled terrain
    // radius is the SEAFLOOR (bathymetric relief sits below sea level), so
    // a low park submerged the camera beneath the drawn water shell
    // (probe capture 2026-07-19: "Iceland at 120 m" landed 27 m over the
    // seafloor, underwater, staring at the shell from below). Park
    // relative to the water surface instead; diving stays a deliberate
    // act, not a teleport accident.
    // v0.896: gate on the RADIUS, not the ocean mask - the mask can still
    // be loading seconds after world entry (exactly when scripted probe
    // requests fire) and its coastal cells miss fjords, both of which
    // re-submerged an Iceland park after the v0.894 fix. On Earth any
    // sampled ground below the nominal sea radius IS under the drawn
    // water shell, so park on the water. (Below-sea-level dry land like
    // the Dead Sea parks ~400 m high - a fine trade against diving.)
    if body_id == "earth" && latlon.is_some() && surface_radius < radius_m {
        surface_radius = radius_m + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64;
    }
    let vantage = body_rel_earth + dir_out * (surface_radius + altitude_km * 1000.0);
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
    state.ship_world_pos = vantage;
    if state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson {
        state
            .camera
            .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
    }
    let hull_top = state.homestead_bounds.map(|(_, mx)| mx.y).unwrap_or(20.0);
    state.camera.position = Vec3::new(0.0, hull_top + 30.0, 0.0);
    // Aim at the body center, optionally rotated toward the horizon around
    // the camera-right axis (Rodrigues; right axis picked degenerate-safe).
    // {"aim":"sun"} (v0.895) overrides everything and faces the SUN from
    // the vantage - the staged-capture rig for sunsets, god rays, and any
    // shot that needs the disc in frame (the default nadir-relative aim
    // plus the fast staged clock made sun-in-frame shots pure luck).
    if v.get("aim").and_then(|a| a.as_str()) == Some("sun") {
        let aim = (state.sun_world_pos - vantage).normalize_or_zero();
        state.ship_world_pos = vantage;
        if state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson {
            state
                .camera
                .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        }
        let hull_top = state.homestead_bounds.map(|(_, mx)| mx.y).unwrap_or(20.0);
        state.camera.position = Vec3::new(0.0, hull_top + 30.0, 0.0);
        state.camera.clear_surface();
        let (yaw, pitch) = crate::dev_travel::look_angles(aim);
        state.camera.yaw = yaw;
        state.camera.pitch = pitch;
        state.gui_state.dev_fly_mode = true;
        state.controller.fly_mode = true;
        // GRAVITY OFF, exactly as F9 does it (v0.1269, operator catch). The
        // PHYSICS reads gui_state.dev_hover - MoveMode::from_dev_flight(dev_hover)
        // in lib.rs - not dev_fly_mode, so setting only the two flags above left
        // the movement model in Walk: gravity pulled and the ground clamped, and
        // a camera placed at altitude SANK during the settle. Every probe-rig
        // capture in the cloud arc was taken below its requested altitude (5.9 km
        // requested, 5.7 captured; 3.4 requested, 3.2 captured) and every rig HUD
        // read WALK. surface_vr is zeroed so no inherited fall velocity carries in.
        state.gui_state.dev_hover = true;
        state.surface_vr = 0.0;
        state.gui_state.dev_travel_away = true;
        {
            let spin = state.current_spin;
            let cam_local = glam::DVec3::new(
                state.camera.position.x as f64,
                state.camera.position.y as f64,
                state.camera.position.z as f64,
            );
            state.frame_lock_anchor = crate::dev_travel::frame_lock_capture(
                body_rel_earth,
                spin,
                vantage + cam_local,
            );
            state.frame_lock_last_spin = spin;
            state.frame_lock_body = Some(body_id.clone());
        }
        // Probe hold (increment 2): pin the parked POSITION (grace-period
        // tracked - see the field doc) so gravity or buoyancy cannot drag
        // the probe off its altitude during the capture settle. Cleared
        // by real movement input.
        state.probe_hold = Some((state.camera.position, std::time::Instant::now()));
        apply_far_frame(state, &v);
        let _ = std::fs::create_dir_all("debug");
        let _ = std::fs::write(
            DONE_PATH,
            serde_json::json!({"ok": true, "body": body_id, "aim": "sun"}).to_string(),
        );
        log::info!("Camera request: parked at {altitude_km:.1} km, aimed at the sun");
        return;
    }
    let fwd = (body_rel_earth - vantage).normalize_or_zero();
    let aim = if look_offset_deg.abs() > 0.01 {
        let mut right = fwd.cross(glam::DVec3::Y);
        if right.length_squared() < 1e-9 {
            right = fwd.cross(glam::DVec3::X);
        }
        let right = right.normalize_or_zero();
        let ang = look_offset_deg.to_radians();
        (fwd * ang.cos() + right.cross(fwd) * ang.sin()).normalize_or_zero()
    } else {
        fwd
    };
    // Reset to the WORLD-Y basis before applying the aim: if surface FPS
    // mode is still engaged from a PREVIOUS request (task #76 increment 1),
    // these world-basis yaw/pitch would be reinterpreted in the tangent
    // basis and the aim would be wrong. Clearing here lets the frame-lock
    // re-engage surface mode next frame and PRESERVE this exact look
    // direction across the basis change (set_surface_up transition).
    state.camera.clear_surface();
    let (yaw, pitch) = crate::dev_travel::look_angles(aim);
    state.camera.yaw = yaw;
    state.camera.pitch = pitch;
    state.gui_state.dev_fly_mode = true;
    state.controller.fly_mode = true;
    // GRAVITY OFF, exactly as F9 does it (v0.1269, operator catch). The
    // PHYSICS reads gui_state.dev_hover - MoveMode::from_dev_flight(dev_hover)
    // in lib.rs - not dev_fly_mode, so setting only the two flags above left
    // the movement model in Walk: gravity pulled and the ground clamped, and
    // a camera placed at altitude SANK during the settle. Every probe-rig
    // capture in the cloud arc was taken below its requested altitude (5.9 km
    // requested, 5.7 captured; 3.4 requested, 3.2 captured) and every rig HUD
    // read WALK. surface_vr is zeroed so no inherited fall velocity carries in.
    state.gui_state.dev_hover = true;
    state.surface_vr = 0.0;
    state.gui_state.dev_travel_away = true;
    // Frame-lock (v0.819): ride this body's rotating/orbiting frame so its
    // surface holds still instead of spinning past at tens of km/s. Capture
    // the anchor at the current spin from where the camera ends up.
    {
        let spin = state.current_spin;
        let cam_local = glam::DVec3::new(
            state.camera.position.x as f64,
            state.camera.position.y as f64,
            state.camera.position.z as f64,
        );
        state.frame_lock_anchor =
            crate::dev_travel::frame_lock_capture(body_rel_earth, spin, vantage + cam_local);
        state.frame_lock_last_spin = spin;
        state.frame_lock_body = Some(body_id.clone());
    }
    // Probe hold (increment 2): pin the parked pose - see the field doc
    // in engine::state. The frame-lock ride still carries the FRAME; this
    // pins the local offset inside it, which is where gravity integrates.
    state.probe_hold = Some((state.camera.position, std::time::Instant::now()));
    apply_far_frame(state, &v);
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(
        DONE_PATH,
        serde_json::json!({
            "ok": true,
            "body": body_id,
            "altitude_km": altitude_km,
            "look_offset_deg": look_offset_deg,
        })
        .to_string(),
    );
    log::info!(
        "Camera request: parked {altitude_km:.1} km above {body_id} (look offset {look_offset_deg:.0} deg)"
    );
}

/// Dev autopilot (v0.793): drive an instance into the shared world with ZERO
/// clicks, for scripted co-presence tests (two instances on one machine) and
/// future CI smoke tests. Mirrors the proven `poll_screenshot_request`
/// pattern: drop `debug/autopilot_request.json` under the process CWD, e.g.
///   {"server_url":"https://united-humanity.us","user_name":"autotest-a","character_name":"TestPilot A"}
/// (all fields optional) and the next frame consumes it, doing exactly what
/// the manual menu flow would set:
///   1. storage undecided -> installed mode (drop a `portable.txt` beside the
///      exe BEFORE launch to make a portable/second-identity instance);
///   2. no identity in memory -> generate a fresh EPHEMERAL seed (memory-only,
///      no passphrase, never written to disk) + apply_pq_identity();
///   3. onboarding_complete + server_url/user_name -> the existing
///      auto-connect block opens the socket on its own;
///   4. active_page = None -> load_world fires -> the co-presence gate sends
///      game_join once the socket is up.
/// Writes `debug/autopilot_done.json` and deletes the request either way.
/// Permanent dev tooling (forever-development norm), not a player feature:
/// the ephemeral identity is throwaway by design and never touches a vault.
pub(crate) fn poll_autopilot_request(state: &mut EngineState) {
    const REQUEST_PATH: &str = "debug/autopilot_request.json";
    const DONE_PATH: &str = "debug/autopilot_done.json";
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let parsed = std::fs::read_to_string(REQUEST_PATH)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    // Consumed either way (same rule as the screenshot poll): a malformed
    // request must not retry every frame forever.
    let _ = std::fs::remove_file(REQUEST_PATH);
    let _ = std::fs::create_dir_all("debug");
    let Some(req) = parsed else {
        let _ = std::fs::write(
            DONE_PATH,
            serde_json::json!({"ok": false, "error": "unreadable or invalid JSON"}).to_string(),
        );
        return;
    };
    // SAFETY GUARD (2026-07-12 incident): the autopilot is a THROWAWAY dev
    // tool that fabricates a "DevBot" identity + name. It must NEVER hijack a
    // REAL user's install. If an encrypted vault already exists (a real
    // onboarded identity, loaded from %APPDATA%), refuse outright -- otherwise
    // (as happened) it activates the DevBot key + overwrites `user_name`, and
    // the next config save persists "DevBot" over the real identity, so the
    // operator's chat posted as DevBot. Dev/agent verify runs MUST use a
    // fresh/portable scratch folder (drop `portable.txt` beside the exe),
    // which has no real vault, so this guard never fires there.
    if !state.gui_state.encrypted_private_key.is_empty() {
        log::warn!(
            "Autopilot: REFUSING to run -- a real encrypted identity is present. \
             The autopilot must run in a portable/fresh scratch folder (drop \
             portable.txt beside the exe) so its throwaway DevBot identity never \
             clobbers the real user's identity + name."
        );
        let _ = std::fs::write(
            DONE_PATH,
            serde_json::json!({
                "ok": false,
                "error": "refusing to clobber the real installed identity; run portable (portable.txt beside the exe)"
            })
            .to_string(),
        );
        return;
    }
    if crate::storage::mode() == crate::storage::StorageMode::Undecided {
        crate::storage::choose_installed();
    }
    if state.gui_state.private_key_bytes.is_none() {
        // Reuse this test folder's identity across reruns: the relay caps
        // DISTINCT NEW identities per IP per hour (anti-onboarding-flood),
        // and a fresh seed every launch would eat that budget two at a
        // time. Stored as PLAINTEXT BYTES on purpose -- this is a
        // throwaway dev-test identity in a scratch folder, never a real
        // vault; anything valuable goes through the normal passphrase
        // flow, not autopilot.
        const SEED_PATH: &str = "debug/autopilot_seed.bin";
        let seed: Vec<u8> = std::fs::read(SEED_PATH)
            .ok()
            .filter(|b| b.len() == 32)
            .unwrap_or_else(|| {
                let s = crate::net::identity::generate_new_seed();
                let _ = std::fs::write(SEED_PATH, &s);
                s
            });
        state.gui_state.private_key_bytes = Some(seed);
        // Derives Dilithium+Kyber and clears the reconnect guards so the
        // auto-connect block re-fires. Deliberately NOT setting
        // passphrase_needed: this identity is throwaway and must not gate
        // on a modal.
        state.gui_state.apply_pq_identity();
    }
    if let Some(url) = req.get("server_url").and_then(|v| v.as_str()) {
        state.gui_state.server_url = url.to_string();
    }
    if let Some(n) = req.get("user_name").and_then(|v| v.as_str()) {
        state.gui_state.user_name = n.to_string();
    }
    if let Some(n) = req.get("character_name").and_then(|v| v.as_str()) {
        state.gui_state.character_name = n.to_string();
    }
    if state.gui_state.user_name.trim().is_empty() {
        // The auto-connect gate requires a non-empty chat name.
        state.gui_state.user_name = "Autopilot".to_string();
    }
    state.gui_state.onboarding_complete = true;
    state.gui_state.showroom_active = false;
    state.gui_state.construction_active = false;
    state.gui_state.active_page = GuiPage::None;
    let key_prefix: String =
        state.gui_state.profile_public_key.chars().take(12).collect();
    log::info!(
        "Autopilot: entering world as '{}' (chat name '{}') on {} (identity {}...)",
        state.gui_state.character_name,
        state.gui_state.user_name,
        state.gui_state.server_url,
        key_prefix
    );
    let _ = std::fs::write(
        DONE_PATH,
        serde_json::json!({
            "ok": true,
            "server_url": state.gui_state.server_url,
            "user_name": state.gui_state.user_name,
            "character_name": state.gui_state.character_name,
            "public_key_prefix": key_prefix,
        })
        .to_string(),
    );
}

use glam::Vec3;
use crate::engine::frame_lock::*;
use crate::engine::ipc_parse::*;
use crate::engine::state::*;
use crate::gui::screen_surface::LoadState;
use crate::gui::{GuiPage, GuiState};

// ── VEGETATION DETERMINISM PINS (2026-09-18) ─────────────────────────────────
//
// Why these exist: two boots of the SAME exe at `fuji-forest-ground` differ in
// 42 to 44 percent of pixels outside the HUD (mean channel delta 9 to 14),
// while the sky and bare-ground blocks of the same frames read 0.00. Every one
// of those pixels is foliage or its shadow. So no vegetation vantage could
// carry a pixel-identity proof at all, and the V1 near-tree increment had to
// rest its "the model set did not change" claim on counters and unit tests
// instead of on a capture (docs/design/frame-cost-arc.md, "V1 outcome").
//
// What actually moves the pixels, read out of `00-bindings-vertex.wgsl`:
//   * the WIND SPEED published to the sway uniform, which sets the static lean
//     (v^2), the gust-front travel speed and part of the sway amplitude; and
//   * the ANIMATION CLOCK `t` (= `camera.sun_color.w`), which every sway,
//     gust and flutter term is a sine of. This is app-start-relative, so it is
//     different at every boot even when the wind is identical.
//
// `weather: clear` already pins the wind to 4 m/s, so the clock is the larger
// half - which is why there are TWO pins here and not one. Both are dev knobs
// with no effect unless a showcase_request sets them.

/// Smallest wind speed the pin will publish, in m/s.
///
/// NOT zero, and this is the whole trap: the shader treats the published slot
/// as "unwritten / no publisher yet" when the speed is `<= 0.0` and substitutes
/// a 4 m/s fallback breeze (`00-bindings-vertex.wgsl`, the LIVE WIND INPUT
/// block). Publishing a literal 0 for `{"wind":"0"}` would therefore hand the
/// forest a BREEZE while the manifest, the log and the operator all believed
/// the air was still - the exact class of defect this file's pins exist to
/// close. 1e-4 m/s is 0.1 mm/s: it clears the guard and its static lean
/// (6e-4 * v^2) is 6e-12 of a tree height, i.e. nothing any capture can see.
pub(crate) const FOLIAGE_WIND_PIN_MIN: f32 = 1.0e-4;

/// Fallback wind direction when the pin is active and the weather's direction
/// is degenerate. The shader ALSO falls back to its 4 m/s breeze when the
/// published direction has `dot(w,w) < 0.25`, so a pinned speed with a zero
/// direction would be silently ignored the same way a pinned 0 would be.
const FOLIAGE_WIND_PIN_DIR: Vec3 = Vec3::new(0.86, 0.0, 0.32);

/// A showcase pin that is either a number or the word `auto` (release it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ShowcasePin {
    /// `"auto"`: hand the value back to the simulation.
    Auto,
    /// `"<number>"`: hold it here.
    Value(f32),
}

/// Pull one `"key":"value"` string out of a showcase request body.
///
/// The showcase handler has always done this with a hand-rolled scan rather
/// than a JSON parse (the request is a dev file drop, not a wire format). It
/// lives here as a named function so the chain from the dropped text to the
/// pinned value is testable end to end: with it inline in a closure, a
/// mistyped key would have been caught by nothing at all.
pub(crate) fn showcase_value(text: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\"");
    let at = text.find(&k)? + k.len();
    let rest = &text[at..];
    let q0 = rest.find('"')? + 1;
    let q1 = rest[q0..].find('"')? + q0;
    Some(rest[q0..q1].to_string())
}

/// Parse one pin value. `None` for anything unparseable, so a typo leaves the
/// existing state alone instead of silently resetting it to a default.
pub(crate) fn parse_showcase_pin(raw: &str) -> Option<ShowcasePin> {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("auto") {
        return Some(ShowcasePin::Auto);
    }
    t.parse::<f32>().ok().filter(|v| v.is_finite()).map(ShowcasePin::Value)
}

/// THE ONE PLACE the foliage wind reaches the shader, in pure form.
///
/// `renderer.foliage_wind` is poked into BOTH camera buffers (the colour pass
/// and the shadow pass) at offset 576, and the vertex shader's wind branch is
/// shared by every swaying material: procedural plants (type 20), cluster cards
/// (21), baked bark (22), grass strands (23) and opted-in photoscans (19). So
/// pinning here pins the near-tree sway and the grass sway together, which is
/// the requirement - a rig that stilled the trees but not the sward would have
/// bought nothing at `fuji-grass-underfoot`.
///
/// `prev` is the slot's current contents, so a degenerate weather direction
/// keeps the last good one exactly as the pre-pin code did (it only rewrote
/// `[3]` in that case).
///
/// Returns `[dir.x, dir.y, dir.z, speed]`, direction always unit length.
pub(crate) fn published_foliage_wind(
    pin: Option<f32>,
    weather_dir: Vec3,
    weather_speed: f32,
    prev: [f32; 4],
) -> [f32; 4] {
    let speed = match pin {
        // Clamped up off zero, never down to it: see FOLIAGE_WIND_PIN_MIN.
        Some(p) => p.max(FOLIAGE_WIND_PIN_MIN),
        None => weather_speed,
    };
    // Direction, in order of preference: the weather's, then whatever was last
    // published, then the constant. The last two matter only because the
    // shader ALSO falls back to its 4 m/s breeze on a short direction vector,
    // so a pin must never publish one - a pinned calm undone by a degenerate
    // direction would be a knob that silently does the opposite of what it says.
    let dir = normalised_or(weather_dir, || {
        normalised_or(Vec3::new(prev[0], prev[1], prev[2]), || FOLIAGE_WIND_PIN_DIR)
    });
    [dir.x, dir.y, dir.z, speed]
}

/// Normalise `v`, or evaluate `fallback` when it is too short to normalise
/// safely. The 1e-3 threshold is the same one the pre-pin publish site used.
fn normalised_or(v: Vec3, fallback: impl FnOnce() -> Vec3) -> Vec3 {
    let len = v.length();
    if len > 1.0e-3 {
        v / len
    } else {
        fallback()
    }
}

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
    // Delegates to the named, unit-tested extractor above; the closure stays so
    // the ~60 call sites below read unchanged.
    let grab = |key: &str| -> Option<String> { showcase_value(&text, key) };
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
    // Optional "time_scale":"0" sets the game clock SPEED (v0.1287, the rig
    // clock freeze): 0 holds the clock still so the planet does not spin
    // and the sun does not move between a park and its capture. Two
    // down-look captures in one sweep came out rotated about the nadir
    // (2026-09-05) because the 20-minute day turns the planet 0.3 degrees
    // per second under a world-fixed camera. Same request channel as the
    // hour (the TimeSystem's own accumulator is authoritative).
    if let Some(sc) = grab("time_scale").and_then(|t| t.parse::<f32>().ok()) {
        if let Some(req) = state
            .data_store
            .get::<std::sync::Mutex<Option<f32>>>("time_set_scale_request")
        {
            if let Ok(mut r) = req.lock() {
                *r = Some(sc.max(0.0));
            }
        }
    }
    // Optional "wind":"0" pins the wind speed the VEGETATION sees, in m/s;
    // "wind":"auto" hands it back to the weather. See published_foliage_wind
    // above for the zero trap. Pins the near-tree sway and the grass sway
    // together, because both read the one uniform this sets.
    //
    // NOT a whole-weather wind pin: the ocean, the clouds and the HUD keep
    // reading the simulated wind. This is deliberately the narrow knob the
    // rig needs (a deterministic sway input), not a second weather system.
    if let Some(w) = grab("wind") {
        match parse_showcase_pin(&w) {
            Some(ShowcasePin::Auto) => {
                state.foliage_wind_override = None;
                log::info!("Showcase: wind -> auto (vegetation follows the weather again)");
            }
            Some(ShowcasePin::Value(v)) => {
                let v = v.clamp(0.0, 60.0);
                state.foliage_wind_override = Some(v);
                log::info!("Showcase: wind -> {v} m/s pinned (vegetation sway only)");
            }
            None => log::warn!("Showcase: wind \"{w}\" is not a number or \"auto\" - pin unchanged"),
        }
    }
    // Optional "anim_clock":"300" freezes the CELESTIAL-pass animation clock at
    // that many seconds; "auto" returns it live.
    //
    // This is the pin that actually makes a forest capture comparable per
    // pixel, and `wind` alone is not enough for that: the sway keeps a
    // wind-INDEPENDENT breathe term (`sway_amp = h * (0.020 + ...)`, about
    // 0.44 m at the tip of a 22 m fir, at 0.9 Hz) and the leaf flutter keeps a
    // floor of `clamp(wind_v/6, 0.35, 3)`, both of them sines of this clock.
    // Pin it and the whole vertex-side animation of the pass holds still -
    // foliage sway, ocean wave phase and cloud advection alike, in the colour
    // pass AND in the shadow pass, since both stamp it from the same value.
    //
    // Use it for identity proofs, NOT for a normal sweep: a frozen clock makes
    // every "does this move" gate (fuji-grass-underfoot's GRASS MOVES IN WIND,
    // the ladder's temporal pairs) unfalsifiable.
    if let Some(c) = grab("anim_clock") {
        match parse_showcase_pin(&c) {
            Some(ShowcasePin::Auto) => {
                state.anim_clock_pin = None;
                log::info!("Showcase: anim_clock -> auto (live)");
            }
            Some(ShowcasePin::Value(v)) => {
                let v = v.max(0.0);
                state.anim_clock_pin = Some(v);
                log::info!("Showcase: anim_clock -> {v} s pinned (sway + waves + cloud advection frozen)");
            }
            None => log::warn!("Showcase: anim_clock \"{c}\" is not a number or \"auto\" - pin unchanged"),
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
        // 0..12: channels 10/11/12 are the far rung's profile share / level /
        // fraction (increment 4); the F10 panel clamps to the same range.
        state.gui_state.cloud_dev_map_diag = (d as i32).clamp(0, 12);
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
    // {"cloud_hv_km":"2.0"}: height-varying warp amplitude in km (0 = default).
    if let Some(m) = grab("cloud_hv_km").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_hv_km = m;
    }
    // {"cloud_sigma_mul":"3"}: extinction multiplier (0 = off).
    if let Some(m) = grab("cloud_sigma_mul").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_sigma_mul = m;
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
    // Component bisect (v0.1279): {"cloud_no_detail":"1"} etc. turn ONE
    // noise-path density term off.
    if let Some(t) = grab("cloud_no_detail") { state.gui_state.cloud_dev_no_detail = t == "1"; }
    if let Some(t) = grab("cloud_no_puff") { state.gui_state.cloud_dev_no_puff = t == "1"; }
    if let Some(t) = grab("cloud_no_cell") { state.gui_state.cloud_dev_no_cell = t == "1"; }
    if let Some(t) = grab("cloud_no_fray") { state.gui_state.cloud_dev_no_fray = t == "1"; }
    if let Some(t) = grab("cloud_no_bdrop") {
        state.gui_state.cloud_dev_no_bdrop = t == "1";
    }
    // {"cloud_sharp_base":"1"}: base envelope 0.5% of the band instead of 3%.
    if let Some(t) = grab("cloud_sharp_base") {
        state.gui_state.cloud_dev_sharp_base = t == "1";
    }
    // {"cloud_relief_fade":"1"}: relief AO weighted by eye transmittance.
    if let Some(t) = grab("cloud_relief_fade") {
        state.gui_state.cloud_dev_relief_fade = t == "1";
    }
    // {"cloud_deep_rung":"1"}: skip the first three sun rungs for deep samples.
    if let Some(t) = grab("cloud_deep_rung") {
        state.gui_state.cloud_dev_deep_rung = t == "1";
    }
    // {"cloud_checker":"1"}: synthetic 0.5 km checker density, the projection test.
    if let Some(t) = grab("cloud_checker") {
        state.gui_state.cloud_dev_checker = t == "1";
    }
    // {"cloud_ms":"1"}: increment A, the in-cloud light; {"cloud_ms_gain":"1.5"}.
    if let Some(t) = grab("cloud_ms") {
        state.gui_state.cloud_dev_ms = t == "1";
    }
    // {"cloud_int_sat":"1"}: increment C, interior saturation of the built bodies.
    if let Some(m) = grab("cloud_int_sat").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_int_sat = m.clamp(0.0, 1.0);
    }
    // {"cloud_step_eco":"1"}: perf increment 2, step economy strength 0..1.
    if let Some(m) = grab("cloud_step_eco").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_step_eco = m.clamp(0.0, 1.0);
    }
    // {"cloud_body_cache":"1"}: perf increment 3, the per-ray body cluster cache.
    if let Some(t) = grab("cloud_body_cache") {
        state.gui_state.cloud_dev_body_cache = t == "1";
    }
    // {"cloud_field":"1"}: increment B 2.1, the three-octave domain warp.
    if let Some(t) = grab("cloud_field") {
        state.gui_state.cloud_dev_field = t == "1";
    }
    // {"cloud_light":"1"}: performance plan increment 1, the sun-shadow
    // cache (planet-fixed slice atlas, one tap per sample); "0" = the
    // 12-rung ladder per pixel, the A/B twin.
    if let Some(t) = grab("cloud_light") {
        state.gui_state.cloud_dev_light = t == "1";
    }
    // {"cloud_profile":"0|1|hard|ref|L0".."L5"}: performance plan increment
    // 4, the far rung (the planet-fixed cloud PROFILE read beyond the
    // footprint band). "0" = the point-sampled field, bit-identical (the
    // A/B twin); "1" = automatic level by footprint, blended; "hard" = the
    // hard-switch prove-red (knob 8); "ref" = the slow reference bake (knob
    // 9); "L0".."L5" = that level forced on every sample (knobs 2..7).
    if let Some(t) = grab("cloud_profile") {
        let knob = match t.trim().to_ascii_lowercase().as_str() {
            "0" | "off" => 0,
            "1" | "on" => 1,
            "hard" | "8" => 8,
            "ref" | "9" => 9,
            s if s.starts_with('l') && s.len() == 2 && s[1..].parse::<i32>().ok().is_some_and(|l| (0..=5).contains(&l)) => {
                2 + s[1..].parse::<i32>().unwrap_or(0)
            }
            s if s.parse::<i32>().ok().is_some_and(|k| (0..=9).contains(&k)) => s.parse::<i32>().unwrap_or(0),
            // An unknown value ("L6", "auto", a typo in a fixture) must be
            // LOUD: silently landing on knob 0 would run the A/B twin while
            // the sweep manifest says the cell was a profile cell (a gate
            // that cannot fail). The knob stays 0 and the log says so.
            s => {
                log::warn!("[showcase] cloud_profile: unknown value {s:?} (want 0|1|hard|ref|L0..L5), knob stays 0");
                0
            }
        };
        state.gui_state.cloud_dev_profile_knob = knob;
    }
    // {"cloud_top_bound":"0|1"}: D3 (2026-09-06), the built-body TOP BOUND
    // dev bit (flags-pad bit 12 of light2_color.w). "1" = the march finds
    // thin built clouds from above (the body publishes the vertical gap to
    // its admitted density region as a from-above SDF bound, and the step
    // economy's in-cloud floor is capped at a quarter of the found cloud's
    // height); "0" = today's 928 m comb, the A/B twin. Independent of
    // `cloud_profile`: the gate cells prof-vert-250/60-r1-{prod,ref,fix}
    // run it at knob 0. Sticky across cells like every showcase pin, so
    // every far-rung cell pins it explicitly.
    if let Some(t) = grab("cloud_top_bound") {
        // This bit is the ARM SELECTOR of the D3 gate, so an unknown value
        // ("true", "on", a typo in a fixture) must be LOUD, exactly like
        // `cloud_profile` above: silently landing on OFF would run the prod
        // arm while the sweep manifest says the cell was the fix arm (a gate
        // that cannot fail). The bit stays off and the log says so; the 1 Hz
        // `[CloudProfile] ... top_bound=` line in run.log records the arm.
        let t = t.trim();
        let on = t == "1";
        if !on && t != "0" {
            log::warn!("[showcase] cloud_top_bound: unknown value {t:?} (want 0|1), staying off");
        }
        state.gui_state.cloud_dev_top_bound = on;
    }
    if let Some(m) = grab("cloud_ms_gain").and_then(|t| t.parse::<f32>().ok()) {
        state.gui_state.cloud_dev_ms_gain = m;
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
    // The live camera, with the projection aspect of the capture. A COPY,
    // so the live camera is never touched (before `render_view_onto`
    // existed this path set and restored `state.camera.aspect` around the
    // passes; the copy is the same thing with nothing to restore).
    let mut camera = state.camera.clone();
    camera.aspect = w as f32 / h.max(1) as f32;
    render_view_onto(state, &camera, &capture_view, (w, h), lists, ViewPasses::Everything);

    let result = state.renderer.read_texture_to_png(
        &capture_tex,
        w,
        h,
        std::path::Path::new(out_path),
    );
    result.map(|()| (w, h))
}

/// Which passes `render_view_onto` runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewPasses {
    /// Every pass the live frame draws, in the live order: sky, planet and
    /// clouds, celestial lines, god rays, SSAO, scene, transparent, overlay,
    /// ring lines. The hi-res screenshot: what the player sees, at any size.
    Everything,
    /// The sky and the scene objects only: stars, then the opaque,
    /// transparent and overlay lists and the ring lines. No planet/cloud
    /// pass, no celestial lines, no god rays, no SSAO. This is what a CAMERA
    /// SCREEN renders (in-world screens, rung 3), for two reasons that are
    /// both about the cloud pass, not the camera: (1) the cloud renderer
    /// keeps per-frame temporal history (screen-space reprojection, a
    /// re-anchor order, a translation baseline) fitted to the PLAYER's
    /// camera, and running it from another pose at 10 Hz would poison that
    /// history for the live frame (the resample and reprojection orders are
    /// `take()`n by whichever celestial pass runs first in a frame, and a
    /// camera screen renders BEFORE the live frame's own pass); (2) it is
    /// the frame's most expensive pass by an order of magnitude. A camera
    /// post inside the homestead sees the station and a star sky through
    /// any window; a planet in a camera's window is a later rung (it needs
    /// a second temporal history keyed per camera).
    SceneOnly,
}

/// THE DAYLIGHT GATE for the star pass (v0.1059), shared by the live frame
/// and every off-screen view (`render_view_onto`). Inside an atmosphere
/// with the sun more than about 6 degrees up, every star is washed out by
/// the sky drawn over it, so the 16.8 M-point star draw is pure waste, and
/// on a view that draws NO sky over the stars (a camera screen,
/// `ViewPasses::SceneOnly`) it is worse than waste: a camera looking out a
/// window in daylight would show stars. Sun below that still draws the full
/// sky, so dusk, dawn and night are untouched, and so is space (no
/// frame-locked body = no atmosphere to hide behind). Read from the same
/// state fields whichever caller asks, so the live frame and a camera can
/// never disagree about whether it is day.
pub(crate) fn sky_daylight(state: &EngineState) -> bool {
    state
        .frame_lock_body
        .as_deref()
        .and_then(|b| state.planet_defs.get(b))
        .map(|d| {
            let alt = state.frame_lock_anchor.length() - d.radius;
            let up = state.frame_lock_anchor.normalize_or_zero();
            let to_sun = (state.sun_world_pos - state.ship_world_pos).normalize_or_zero();
            let sun_up = up.dot(to_sun);
            alt < 120_000.0 && sun_up > 0.10
        })
        .unwrap_or(false)
}

/// Render ONE view of the world from `camera` into `target` at `size`
/// pixels: the passes `passes` names, in the live frame's order, against
/// the draw lists `lists` (this frame's, borrowed) and the same world state
/// the live frame uses (sun, anchors, cloud clock, the daylight gate). Shared
/// by the hi-res screenshot (which reads the target back to a PNG) and the
/// camera screen provider (which renders straight into a screen's surface
/// texture). The passes bind a depth buffer of `size` for the duration, the
/// renderer's spare one (`Renderer::begin_view_depth`), and the window's own
/// depth buffer is parked untouched and put back before returning, so the
/// next live pass binds exactly the buffer it had, whatever happens in
/// between, and a 10 Hz camera costs no depth allocations at steady state.
///
/// `target` must be a render attachment in the scene's swapchain format
/// (`Renderer::surface_format`): the scene pipelines were built for that
/// format and can draw into no other. The caller checks `world_loaded`.
///
/// Frame-of-reference note for callers that run BEFORE the live scene pass
/// (the camera screens): the draw lists are still in the HOME frame at that
/// point (lib.rs adds `station_off` in place further down the frame), so a
/// camera pose in the home frame is the right input; the live camera's own
/// `effective_position` is in the render frame and would be off by the
/// station offset when the station is not at the render origin.
pub(crate) fn render_view_onto(
    state: &mut EngineState,
    camera: &crate::renderer::camera::Camera,
    target: &wgpu::TextureView,
    size: (u32, u32),
    lists: &SceneDrawLists,
    passes: ViewPasses,
) {
    let (w, h) = size;
    // Whose frame these passes are, for the cost keys. A camera screen's
    // 10 Hz re-render publishes under its own `gpu.screen_*` ids so the
    // Performance page shows the wall as its own number; the hi-res
    // screenshot keeps the MAIN ids on purpose: it is a one-off render of
    // the player's own view (its other passes, celestial and god rays, are
    // main-keyed already), it decays out of the pie within a second, and
    // keying it as a "screen" would misattribute a screenshot to the
    // camera wall.
    let who = match passes {
        ViewPasses::Everything => crate::renderer::frame_costs::SceneView::Main,
        ViewPasses::SceneOnly => crate::renderer::frame_costs::SceneView::Screen,
    };
    // The view's own depth buffer goes in; the window's is parked, not
    // recreated (see `Renderer::begin_view_depth`).
    state.renderer.begin_view_depth(w, h);
    // The same daylight the live frame computes for its star pass: a camera
    // draws no sky over its stars, so without this a daytime window would
    // show stars (the rung-3 reviewer's finding).
    let daylight = sky_daylight(state);

    // Pass 1: sky -- galaxy glow, stars, halos, constellations -- exactly as
    // the live frame draws it (clear to black + star renderer). If the star
    // renderer is somehow absent the clear still runs, so the target is
    // never undefined.
    {
        // `gpu.screen_sky` / `cpu.screen_sky` for a camera screen, `gpu.stars`
        // / `cpu.stars` for the screenshot (see `who` above). The CPU stage
        // is the submission twin the no-timestamp fallback reads.
        let (sky_gpu, sky_cpu) = who.sky_ids();
        let _cost = crate::renderer::frame_costs::stage(sky_cpu);
        if let Some(ref star_r) = state.star_renderer {
            star_r.update_camera(
                &state.renderer.queue,
                camera,
                crate::station::render_to_world_rot(state.station_ride, state.station_world_rot).as_quat(),
            );
        }
        let mut encoder = state.renderer.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("View Star Encoder") },
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("View Star Pass"),
                timestamp_writes: state.renderer.pass_timer(sky_gpu),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
                // The live frame's daylight gate, not a hard-coded "night".
                star_r.render_pass(&mut pass, daylight);
            }
        }
        state.renderer.queue.submit(std::iter::once(encoder.finish()));
    }

    if passes == ViewPasses::Everything {
        // Passes 1.5 .. 2.7: identical order + inputs to the live frame
        // render (see the in-game branch above `match scene_result`). The
        // sun direction is recomputed from the same state fields the live
        // path uses.
        let sun_dir_f = {
            let d = (state.sun_world_pos - state.ship_world_pos).normalize_or_zero();
            Vec3::new(d.x as f32, d.y as f32, d.z as f32)
        };
        state.renderer.render_celestial_onto(
            camera,
            lists.celestial,
            lists.celestial_transparent,
            sun_dir_f,
            // Same clock as the live path: cloud decks drift with time, and
            // a capture should freeze the exact frame the player is looking
            // at. That includes the `anim_clock` pin - this is the SECOND
            // caller of render_celestial_onto (the hi-res offscreen capture
            // and the live-broadcast pump), and a pin honoured by only one of
            // the two would mean a rig's own screenshot rendered a swaying
            // canopy while the window it came from stood still.
            state
                .anim_clock_pin
                .unwrap_or_else(|| state.start_time.elapsed().as_secs_f32()),
            cloud_ground_params(state),
            ground_anchor(state),
            ocean_anchor256(state),
            target,
        );
        state.renderer.draw_celestial_lines_onto(camera, lists.orbit_lines, target);
        // God rays in captures too (v0.895), same slot as the live path, so
        // screenshots show exactly what the player sees.
        state.renderer.render_godrays_onto(camera, sun_dir_f, target, godray_scale(state));
        state.renderer.render_ssao_onto(camera, target);
    }
    state.renderer.render_scene_onto(camera, lists.opaque, target, who);
    state.renderer.render_transparent_onto(camera, lists.transparent, target, who);
    // Same `who` for the overlay and the ring lines: a camera screen's draws
    // publish under `gpu.screen_overlay` / `gpu.screen_lines`, not the live
    // frame's ids (the 2026-09-18 review found them summed into the Main ones).
    state.renderer.render_overlay_onto(camera, lists.overlay, target, who);
    state.renderer.draw_lines_onto(camera, lists.ring_lines, target, who);

    // Put the window's depth buffer back BEFORE returning, or the next live
    // pass would bind a mismatched one. The view-sized buffer is kept for
    // the camera screens (the same size again in 100 ms) and dropped after a
    // one-off screenshot (an 8K depth buffer must not linger all session).
    state.renderer.end_view_depth(passes == ViewPasses::SceneOnly);
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

/// Cloud PROFILE atlas dump (increment 4, the far rung, dev forensics):
/// drop `debug/cloud_profile_dump_request.json` (any content) while the
/// game runs and every slice of the profile atlas is written as raw RGBA8
/// PNGs under `debug/`: the 54 window slices
/// (`cloud_profile_L<level>_s<slice>.png`), the three global slices
/// (`cloud_profile_global_<0|1|c>.png`) and the calibration table
/// (`cloud_profile_calib.png`), then `debug/cloud_profile_done.json`
/// reports the count (or the error). `scripts/cloud-profile-compare.js`
/// diffs two dumps (the reference bake against the analytic one, G1/G4).
pub(crate) fn poll_cloud_profile_dump_request(state: &mut EngineState) {
    const REQUEST_PATH: &str = "debug/cloud_profile_dump_request.json";
    const DONE_PATH: &str = "debug/cloud_profile_done.json";
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let _ = std::fs::remove_file(REQUEST_PATH);
    let dir = std::path::Path::new("debug");
    let done = match state.renderer.dump_cloud_profile_pngs(dir) {
        Ok(files) => {
            let pads = state
                .renderer
                .cloud_profile_cache
                .as_ref()
                .map(|pc| pc.state.pads())
                .unwrap_or([0.0; 4]);
            serde_json::json!({
                "ok": true,
                "dir": "debug",
                "files": files,
                "knob": state.renderer.cloud_profile_knob,
                // D3 dev bit, so a dump records which arm it came from.
                "top_bound": state.renderer.cloud_top_bound,
                "ground_i0": pads[0],
                "ground_j0": pads[1],
                // D3: the flags the SHADER saw, i.e. the cache's validity
                // bits with the top-bound bit ORed in exactly as the upload
                // site (renderer/mod.rs, the pad write at offset 240) does
                // it. The raw cache value would read 0 for a frame whose pad
                // carried 4096, and a dump must not disagree with the GPU.
                "flags": crate::renderer::cloud_temporal::cloud_fr_flags_with_top_bound(
                    pads[3] as u32,
                    state.renderer.cloud_top_bound,
                ),
            })
        }
        Err(e) => serde_json::json!({"ok": false, "error": e}),
    };
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(DONE_PATH, done.to_string());
}

/// In-world screen dev IPC (in-world screens rung 1; permanent tooling, the
/// same file-drop pattern as the screenshot command). Drop
/// `debug/screen_request.json`:
///
/// ```json
/// {"screen": "wall_screen_1", "action": "hover|click|scroll|text|snapshot|find|link|wait_ready|status",
///  "uv": [0.5, 0.2], "dy": 0, "text": "", "index": 0}
/// {"screen": "wall_screen_1", "find": {"text": "Home"}}
/// {"screen": "wall_screen_3", "link": {"index": 0}}
/// {"screen": "wall_screen_5", "video_open": "C:/Users/me/Videos/holiday.mp4"}
/// ```
///
/// `video_open` hands the path to a video screen's provider exactly as its
/// own Open button would (probe, cache, convert with ffmpeg). A FOLDER is
/// taken as a video disc (or the drive holding one): its main title is
/// chosen and converted as one film, `src/media/dvd.rs`. `status` is
/// a `snapshot` without the PNG, for polling the provider's fields (a
/// conversion's `media.transcoding_pct`).
///
/// and the engine performs that synthetic event on the named surface THROUGH
/// THE SAME EVENT API the look ray uses (`ScreenCore::pointer_moved`,
/// `ScreenSurface::button`, `scroll`, `text`), never a side path, then writes
/// `debug/screen_done.json`:
///
/// ```json
/// {"ok": true, "source": "page:inventory", "kind": "page", "wants_keyboard": false,
///  "hover_widget": true, "cursor_icon": "Default", "png": "debug/screen_wall_screen_1_3.png"}
/// ```
///
/// plus whatever the surface's provider reports through `status()` (a web
/// screen adds `url`, `title` and `status`: fetching / ready / error / off).
///
/// `snapshot` reads the surface texture back to that PNG (rows padded to 256
/// and unpadded, like the UI snapshot rig); `png` is absent for the other
/// actions. `find` answers with where a drawn text is (`found`, `uv`,
/// `rect_px`, `text`, `matches`) so a rig can click a widget by its label.
/// `link` clicks the Nth link the provider drew, mapped from its rect's
/// centre to a uv and sent as a normal click (hover, press, release, one
/// per frame). `wait_ready` completes only once the provider reports ready
/// or failed, bounded by `screens::WAIT_READY_LIMIT` (a timeout is
/// `ok: false`). This is how the runtime verifier
/// (`scripts/verify-screens.js`) proves a screen works with no human at the
/// keyboard. The request file is consumed even on error; a failure writes
/// `{"ok": false, "error": ...}`, including when the request's screen
/// disappears mid-flight (a placement rebuild, or a surface index that is
/// gone): both drop paths go through `Screens::abandon_ipc`, which clears
/// the record and its payload and names the cause, so a rig never waits
/// out its own timeout on a request the engine dropped.
///
/// Three functions because a click spans three frames (`screens::IpcStage`)
/// and the answer must describe the frame the LAST event landed in:
/// `poll_screen_request` (update phase) parses and stores the request;
/// `advance_screen_request` (after `screens::update`, so the look ray cannot
/// overwrite the event) queues this frame's event; `complete_screen_request`
/// (after `screens::frame_surfaces`) writes the done file from the freshly
/// drawn surface once the stage is `Complete`.
pub(crate) fn poll_screen_request(state: &mut EngineState) {
    const REQUEST_PATH: &str = "debug/screen_request.json";
    if state.screens.ipc.is_some() || !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let parsed = std::fs::read_to_string(REQUEST_PATH)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    // Consumed either way (same rule as the other debug polls).
    let _ = std::fs::remove_file(REQUEST_PATH);
    let fail = |msg: String| {
        log::warn!("Screen request: {msg}");
        write_screen_done(serde_json::json!({"ok": false, "error": msg}));
    };
    let Some(v) = parsed else {
        fail("malformed JSON".to_string());
        return;
    };
    if !state.world_loaded {
        fail("3D world not loaded".to_string());
        return;
    }
    let id = v.get("screen").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let Some(surface) = state.screens.surface_index(&id) else {
        let known: Vec<&str> = state.screens.surfaces.iter().map(|s| s.core.id.as_str()).collect();
        fail(format!("no screen named {id:?}; placed screens: {known:?}"));
        return;
    };
    let mut action = v.get("action").and_then(|s| s.as_str()).unwrap_or("snapshot").to_string();
    let mut text = v.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let mut link_index = v.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
    // The object forms name the verb by their key, so a request reads as
    // what it does: {"find": {"text": "Home"}} and {"link": {"index": 0}}.
    if let Some(f) = v.get("find").and_then(|f| f.as_object()) {
        action = "find".to_string();
        text = f.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
    }
    if let Some(l) = v.get("link").and_then(|l| l.as_object()) {
        action = "link".to_string();
        link_index = l.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
    }
    // {"video_open": "<absolute path>"}: open a file on a video screen
    // exactly as its Open button would (conversion included); the path
    // rides in the text payload. `status` is a bare read of the provider's
    // fields (a `snapshot` without the PNG), for polling a conversion.
    if let Some(p) = v.get("video_open").and_then(|p| p.as_str()) {
        action = "video_open".to_string();
        text = p.to_string();
    }
    if !matches!(
        action.as_str(),
        "hover" | "click" | "scroll" | "text" | "snapshot" | "find" | "link" | "wait_ready" | "video_open" | "status"
    ) {
        fail(format!(
            "unknown action {action:?} (hover|click|scroll|text|snapshot|find|link|wait_ready|video_open|status)"
        ));
        return;
    }
    if action == "video_open" && text.trim().is_empty() {
        fail("video_open needs a file path".to_string());
        return;
    }
    if action == "find" && text.trim().is_empty() {
        fail("find needs a non-empty \"text\"".to_string());
        return;
    }
    let uv = v
        .get("uv")
        .and_then(|a| a.as_array())
        .and_then(|a| Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)))
        .unwrap_or((0.5, 0.5));
    let dy = v.get("dy").and_then(|d| d.as_f64()).unwrap_or(0.0) as f32;
    let snapshot = action == "snapshot";
    state.screens.ipc = Some(crate::engine::screens::ScreenIpc {
        surface,
        find_text: if action == "find" { text.clone() } else { String::new() },
        action,
        uv,
        stage: crate::engine::screens::IpcStage::Queue,
        snapshot,
        link_index,
        started: None,
    });
    // Stash the scalar payloads on the pending record's owner: dy and text
    // are only needed once, at queue time, so they ride in a side slot.
    state.screens.ipc_payload = Some((dy, text));
}

/// Queue the pending request's event on its surface. Runs right after
/// `screens::update` so the synthetic pointer wins the frame over the look
/// ray (which also skips a surface with an IPC in flight).
pub(crate) fn advance_screen_request(state: &mut EngineState) {
    let Some(ipc) = state.screens.ipc.as_mut() else { return };
    let (dy, text) = state.screens.ipc_payload.clone().unwrap_or((0.0, String::new()));
    let Some(s) = state.screens.surfaces.get_mut(ipc.surface) else {
        // The surface index is gone (the placements were rebuilt between
        // poll and advance): answer with a failure naming the cause rather
        // than dropping the request silently and leaving a rig to wait out
        // its own timeout. The helper clears the record and its payload.
        if let Some(done) = state.screens.abandon_ipc("its surface index no longer exists") {
            write_screen_done(done);
        }
        return;
    };
    use crate::engine::screens::IpcStage;
    // A verb that cannot be queued (no such link) answers right here; the
    // message is carried out of the borrow and written below.
    let mut refused: Option<String> = None;
    match ipc.stage {
        IpcStage::Queue => {
            match ipc.action.as_str() {
                "hover" => s.core.pointer_moved(ipc.uv),
                // A click is hover, press, release on three frames: the
                // sequence `screen_click_through_uv_toggles_the_inventory_home_header`
                // proved, because a re-interacted row registers nothing on
                // a same-frame move + press.
                "click" => {
                    s.core.pointer_moved(ipc.uv);
                    ipc.stage = IpcStage::Press;
                    return;
                }
                "link" => {
                    // The Nth link rect the provider drew last frame, mapped
                    // to a uv, then the SAME hover/press/release the look ray
                    // would send. Nothing reaches the view except through the
                    // event API.
                    let rects = s.provider().map(|p| p.link_rects()).unwrap_or_default();
                    match crate::engine::screens::link_uv(&rects, ipc.link_index, s.size()) {
                        Some(uv) => {
                            ipc.uv = uv;
                            s.core.pointer_moved(uv);
                            ipc.stage = IpcStage::Press;
                            return;
                        }
                        None => {
                            refused = Some(format!(
                                "no link {} on screen: the content drew {} link(s) on the surface",
                                ipc.link_index,
                                rects.len()
                            ));
                        }
                    }
                }
                "scroll" => s.core.scroll(ipc.uv, dy),
                "text" => {
                    s.core.pointer_moved(ipc.uv);
                    s.core.text(&text);
                }
                "find" => s.core.find_text(&ipc.find_text),
                "wait_ready" => {
                    ipc.started = Some(std::time::Instant::now());
                    ipc.stage = IpcStage::Waiting;
                    return;
                }
                "video_open" => {
                    // The provider's own open path: probe, cache, convert
                    // (`ScreenProvider::open_media`); a screen that plays
                    // no media says so instead of swallowing the request.
                    let taken = s.provider_mut().map_or(false, |p| p.open_media(std::path::Path::new(&text)));
                    if !taken {
                        refused = Some(format!(
                            "video_open: screen {:?} ({}) plays no media; only a video: screen does",
                            s.core.id,
                            s.core.source.kind()
                        ));
                    }
                }
                _ => {} // snapshot / status: nothing to queue, just draw and report
            }
            ipc.stage = IpcStage::Complete;
        }
        IpcStage::Press => {
            // The surface's own entry point, so the provider hears the
            // press too (same as the look ray's `route_button`).
            s.button(ipc.uv, true);
            ipc.stage = IpcStage::Release;
        }
        IpcStage::Release => {
            s.button(ipc.uv, false);
            ipc.stage = IpcStage::Complete;
        }
        IpcStage::Complete | IpcStage::Waiting => {}
    }
    if let Some(msg) = refused {
        log::warn!("Screen request: {msg}");
        state.screens.ipc = None;
        state.screens.ipc_payload = None;
        write_screen_done(serde_json::json!({"ok": false, "error": msg}));
    }
}

/// The fields every done file carries, plus whatever the provider's
/// `status()` reports (a web view's url, title and fetch state), merged at
/// the top level so the rig reads `done.status` and `done.url` directly.
fn screen_done_base(s: &crate::gui::screen_surface::ScreenSurface, action: &str, focused: bool) -> serde_json::Value {
    let mut done = serde_json::json!({
        "ok": true,
        "screen": s.core.id,
        "source": s.core.source_id,
        "kind": s.core.source.kind(),
        "action": action,
        "wants_keyboard": s.core.wants_keyboard(),
        "hover_widget": s.core.hover_widget(),
        "cursor_icon": format!("{:?}", s.core.cursor_icon()),
        "focused": focused,
    });
    // The provider's own fields ride along (a stream's connection state
    // and frame count, a camera's render count), so the rig can wait for
    // "connected" or "renders > 0" on a wall the same way it waits for a
    // page. A provider key never shadows the fixed keys above, and a
    // null-valued one is left out (a success must never carry
    // `"error": null`; see `merge_provider_status`).
    if let Some(p) = s.provider() {
        merge_provider_status(&mut done, &p.status());
    }
    done
}

/// Write the done file once the surface has drawn the frame the last event
/// landed in (`IpcStage::Complete`), taking the snapshot first when asked;
/// a `wait_ready` (`IpcStage::Waiting`) is polled here every frame instead.
pub(crate) fn complete_screen_request(state: &mut EngineState) {
    use crate::engine::screens::IpcStage;
    let Some(ipc) = state.screens.ipc.as_ref() else { return };
    if !matches!(ipc.stage, IpcStage::Complete | IpcStage::Waiting) {
        return;
    }
    // `wait_ready` polls the provider every frame until it is ready,
    // failed, or the bounded wait runs out. While it waits the request
    // stays in flight, which keeps the surface framed (so a web view keeps
    // polling its fetch) and the look ray off its pointer.
    if ipc.stage == IpcStage::Waiting {
        let Some(s) = state.screens.surfaces.get(ipc.surface) else {
            // One shape for every "cannot finish" (see `Screens::abandon_ipc`).
            if let Some(done) = state.screens.abandon_ipc("its screen vanished while wait_ready was polling") {
                write_screen_done(done);
            }
            return;
        };
        let load = s.provider().map(|p| p.load_state()).unwrap_or(LoadState::Static);
        let elapsed = ipc.started.map(|t| t.elapsed()).unwrap_or_default();
        match crate::engine::screens::wait_ready_outcome(&load, elapsed) {
            None => return,
            Some(Ok(())) => {
                let ipc = state.screens.ipc.take().expect("checked above");
                state.screens.ipc_payload = None;
                let s = &state.screens.surfaces[ipc.surface];
                let mut done = screen_done_base(s, &ipc.action, state.screens.focused == Some(ipc.surface));
                done["waited_ms"] = serde_json::json!(elapsed.as_millis() as u64);
                write_screen_done(done);
            }
            Some(Err(msg)) => {
                let ipc = state.screens.ipc.take().expect("checked above");
                state.screens.ipc_payload = None;
                let s = &state.screens.surfaces[ipc.surface];
                // The provider's status rides along even on failure, so the
                // rig can print WHAT it was waiting on.
                let mut done = screen_done_base(s, &ipc.action, state.screens.focused == Some(ipc.surface));
                done["ok"] = serde_json::json!(false);
                done["error"] = serde_json::json!(msg);
                done["waited_ms"] = serde_json::json!(elapsed.as_millis() as u64);
                log::warn!("Screen request: {}", done["error"]);
                write_screen_done(done);
            }
        }
        return;
    }
    // The surface must still exist to be described; if it vanished between
    // the event and this frame, abandon with the cause (one shape for every
    // "cannot finish", see `Screens::abandon_ipc`).
    if state.screens.surfaces.get(ipc.surface).is_none() {
        if let Some(done) = state.screens.abandon_ipc("its screen vanished before the done file could be written") {
            write_screen_done(done);
        }
        return;
    }
    let ipc = state.screens.ipc.take().expect("checked above");
    state.screens.ipc_payload = None;
    let focused = state.screens.focused == Some(ipc.surface);
    let s = state.screens.surfaces.get_mut(ipc.surface).expect("existence checked just above");
    let mut done = screen_done_base(s, &ipc.action, focused);
    if matches!(ipc.action.as_str(), "hover" | "click" | "scroll" | "text" | "link") {
        // Where the event landed, so the evidence says which point was
        // clicked (a `link` resolves its uv from the rect only at queue time).
        done["uv"] = serde_json::json!([ipc.uv.0, ipc.uv.1]);
    }
    if ipc.action == "find" {
        // Answered from the frame just drawn (see `ScreenCore::find_text`).
        // `found: false` is a successful answer ("nothing by that name is
        // on the surface"), not an error; the rig decides what it means.
        let (w, h) = s.size();
        match s.core.take_found_text().flatten() {
            Some(f) => {
                let c = f.rect.center();
                done["found"] = serde_json::json!(true);
                done["text"] = serde_json::json!(f.text);
                done["matches"] = serde_json::json!(f.matches);
                done["uv"] = serde_json::json!([c.x / w as f32, c.y / h as f32]);
                done["rect_px"] = serde_json::json!([f.rect.min.x, f.rect.min.y, f.rect.width(), f.rect.height()]);
            }
            None => {
                done["found"] = serde_json::json!(false);
                done["text"] = serde_json::json!(ipc.find_text);
                done["matches"] = serde_json::json!(0);
            }
        }
    }
    if ipc.snapshot {
        state.screens.snapshot_counter += 1;
        let path = format!("debug/screen_{}_{}.png", s.core.id, state.screens.snapshot_counter);
        let (w, h) = s.size();
        let pixels = s.read_rgba(&state.renderer.device, &state.renderer.queue);
        let _ = std::fs::create_dir_all("debug");
        match image::RgbaImage::from_raw(w, h, pixels) {
            Some(img) => match img.save(&path) {
                Ok(()) => {
                    done["png"] = serde_json::json!(path);
                }
                Err(e) => {
                    done = serde_json::json!({"ok": false, "error": format!("png save failed: {e}")});
                }
            },
            None => {
                done = serde_json::json!({"ok": false, "error": "readback size mismatch"});
            }
        }
    }
    write_screen_done(done);
}

/// Write `debug/screen_done.json`. `pub(crate)` because `screens::sync_screens`
/// writes the abandonment failure through here too (see `Screens::abandon_ipc`).
pub(crate) fn write_screen_done(done: serde_json::Value) {
    const DONE_PATH: &str = "debug/screen_done.json";
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(DONE_PATH, done.to_string());
}

/// Where a main-UI dev IPC click is in its three-frame sequence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum UiIpcStage {
    /// The pointer move was queued; the press goes on the next frame.
    Press,
    /// The press was queued; the release goes on the next frame.
    Release,
    /// Every event landed (or the verb needed none): answer after this
    /// frame's egui pass.
    Complete,
}

/// One main-UI dev IPC request in flight (`debug/ui_request.json`,
/// 2026-09-18, permanent tooling like `debug/screen_request.json`). Lives on
/// `EngineState::ui_ipc`; one at a time.
#[derive(Debug)]
pub(crate) struct UiIpc {
    pub(crate) action: String,
    /// The pointer verbs' position: window PIXELS as requested, and the
    /// egui POINTS they map to (the pixels-per-point of the live context).
    pub(crate) pos_px: Option<(f32, f32)>,
    pub(crate) pos_points: Option<egui::Pos2>,
    /// The `GuiState` sidebar flag the done file reports before and after.
    pub(crate) flag: Option<String>,
    pub(crate) flag_before: Option<serde_json::Value>,
    pub(crate) stage: UiIpcStage,
    /// "escape": whether the rule closed the sidebar (its return value).
    pub(crate) closed: Option<bool>,
    /// "find": the drawn text to look for, and the answer once
    /// `ui_request_scan_shapes` has seen this frame's shapes (outer None =
    /// not scanned yet, inner None = not drawn).
    pub(crate) find_text: Option<String>,
    pub(crate) found: Option<Option<crate::gui::screen_surface::FoundText>>,
}

/// Main-UI dev IPC, the sibling of `poll_screen_request` for the MAIN egui
/// context rather than a wall screen's. Drop `debug/ui_request.json` while
/// the game runs:
///
/// ```json
/// {"action": "f10", "flag": "show_cloud_dev_panel"}
/// {"action": "escape"}
/// {"action": "state", "flag": "cloud_dev_dither_off"}
/// {"action": "pointer_move", "pos": [200, 300]}
/// {"action": "click", "pos": [200, 300], "flag": "cloud_dev_dither_off"}
/// {"action": "find", "text": "Depth dither"}
/// ```
///
/// `find` answers where a drawn text is on the MAIN frame (`found`, `text`,
/// `matches`, `rect_px`, `pos_px` = the rect's centre in window pixels, the
/// value a following `click` takes as its `pos`), through the same
/// `find_text_in_shapes` the screens' `find` uses: exact match first, then
/// a prefix, then a substring, first in drawing order within a tier, with
/// text clipped out of a scroll area skipped. The scan runs on this frame's
/// shapes in lib.rs (`ui_request_scan_shapes`, between `ctx.run` and
/// tessellation). This is the verb that lets a rig click a named toggle
/// without guessing a pixel from a screenshot.
///
/// `f10` and `escape` call `engine::input::toggle_cloud_dev_panel` and
/// `engine::input::escape_closes_cloud_dev`, the SAME functions the keys
/// run, so a rig proves the rules the operator's keys use. What they do not
/// exercise is the winit layer between the key and those functions (the
/// modifier trackers, the keybind-capture gate, the Escape ordering against
/// the modal and screen-focus gates): that is what the headless harness and
/// the operator's own key are for. `pointer_move` and `click` push
/// `egui::Event::PointerMoved` / `PointerButton` into the main context's
/// pending input (`egui_state.egui_input_mut().events`), so egui's own
/// hit-testing runs against the real panel; a click is move, press, release
/// on three consecutive frames, the sequence the headless click tests
/// proved. `pos` is in window pixels (what a screenshot shows), converted
/// here with the live pixels-per-point. `state` changes nothing and just
/// reports. The optional `flag` names any `GuiState` `cloud_dev_*` field (or
/// `show_cloud_dev_panel` / `cloud_dev_collapsed`), reported before and
/// after through `engine::input::cloud_dev_flag`.
///
/// Every request is consumed, even a bad one, and answered in
/// `debug/ui_request_done.json` (see `complete_ui_request` for the fields; a
/// bad request writes `{"ok": false, "error"}` at once). Runs in the update
/// phase, BEFORE `take_egui_input`, so a queued event lands in this frame's
/// egui pass; a click in flight is advanced here too, one event per frame.
pub(crate) fn poll_ui_request(state: &mut EngineState) {
    const REQUEST_PATH: &str = "debug/ui_request.json";
    // A click in flight: press, then release, one per frame, each landing in
    // the egui pass that follows this call.
    if let Some(ipc) = state.ui_ipc.as_mut() {
        let button = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        match (ipc.stage, ipc.pos_points) {
            (UiIpcStage::Press, Some(pos)) => {
                state.egui_state.egui_input_mut().events.push(button(pos, true));
                ipc.stage = UiIpcStage::Release;
            }
            (UiIpcStage::Release, Some(pos)) => {
                state.egui_state.egui_input_mut().events.push(button(pos, false));
                ipc.stage = UiIpcStage::Complete;
            }
            // Complete waits for `complete_ui_request` after the frame. A
            // pointer stage with no position cannot happen (the parse below
            // refuses it), but if it did the request must not hang a rig.
            (UiIpcStage::Complete, _) => {}
            (_, None) => ipc.stage = UiIpcStage::Complete,
        }
        return;
    }
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let parsed = std::fs::read_to_string(REQUEST_PATH)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    // Consumed either way (same rule as the other debug polls).
    let _ = std::fs::remove_file(REQUEST_PATH);
    let fail = |msg: String| {
        log::warn!("UI request: {msg}");
        write_ui_done(serde_json::json!({"ok": false, "error": msg}));
    };
    let Some(v) = parsed else {
        fail("malformed JSON".to_string());
        return;
    };
    let Some(action) = v.get("action").and_then(|s| s.as_str()).map(|s| s.to_string()) else {
        fail("missing \"action\" (f10|escape|state|pointer_move|click|find)".to_string());
        return;
    };
    let find_text = v.get("text").and_then(|t| t.as_str()).map(|t| t.trim().to_string());
    if action == "find" && find_text.as_deref().map_or(true, |t| t.is_empty()) {
        fail("find needs a non-empty \"text\"".to_string());
        return;
    }
    // The flag is validated up front so a typo answers at once instead of
    // reporting nulls after a three-frame click.
    let flag = v.get("flag").and_then(|f| f.as_str()).map(|f| f.to_string());
    let flag_before = match flag.as_deref() {
        Some(f) => match crate::engine::input::cloud_dev_flag(&state.gui_state, f) {
            Some(val) => Some(val),
            None => {
                fail(format!(
                    "unknown flag {f:?}: name a GuiState cloud_dev_* field, show_cloud_dev_panel or cloud_dev_collapsed"
                ));
                return;
            }
        },
        None => None,
    };
    // Window pixels -> egui points, using the live context's scale so a
    // rig can take its coordinates straight from a screenshot.
    let pos_px = v
        .get("pos")
        .and_then(|a| a.as_array())
        .and_then(|a| Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
    let ppp = state.egui_ctx.pixels_per_point();
    let pos_points = pos_px.map(|(x, y)| egui::pos2(x / ppp, y / ppp));
    let needs_pos = matches!(action.as_str(), "pointer_move" | "click");
    if needs_pos && pos_points.is_none() {
        fail(format!("{action:?} needs \"pos\": [x, y] in window pixels"));
        return;
    }
    let mut closed = None;
    let stage = match action.as_str() {
        "f10" => {
            crate::engine::input::toggle_cloud_dev_panel(&mut state.gui_state);
            UiIpcStage::Complete
        }
        "escape" => {
            closed = Some(crate::engine::input::escape_closes_cloud_dev(&mut state.gui_state));
            UiIpcStage::Complete
        }
        // Answered from this frame's shapes by `ui_request_scan_shapes`.
        "state" | "find" => UiIpcStage::Complete,
        "pointer_move" | "click" => {
            let pos = pos_points.expect("checked above");
            state.egui_state.egui_input_mut().events.push(egui::Event::PointerMoved(pos));
            // The winit-side pixel cache too, so the in-world screens' free
            // cursor ray (screens::pointing_ray) agrees with egui about
            // where the pointer is.
            if let Some(px) = pos_px {
                state.cursor_pos = px;
            }
            if action == "click" {
                UiIpcStage::Press
            } else {
                UiIpcStage::Complete
            }
        }
        other => {
            fail(format!("unknown action {other:?} (f10|escape|state|pointer_move|click|find)"));
            return;
        }
    };
    let find_text = if action == "find" { find_text } else { None };
    state.ui_ipc = Some(UiIpc { action, pos_px, pos_points, flag, flag_before, stage, closed, find_text, found: None });
}

/// Answer a pending `find` from the MAIN egui frame's shapes. lib.rs calls
/// this right after `egui_ctx.run` and before tessellation (which consumes
/// the shapes), every frame; it is a no-op unless a `find` is pending, so
/// it costs nothing on a normal frame. The lookup rule is the screens'
/// `find_text_in_shapes`, so both IPCs answer the same way.
pub(crate) fn ui_request_scan_shapes(state: &mut EngineState, shapes: &[egui::epaint::ClippedShape]) {
    let Some(ipc) = state.ui_ipc.as_mut() else { return };
    let Some(text) = ipc.find_text.as_deref() else { return };
    ipc.found = Some(crate::gui::screen_surface::find_text_in_shapes(shapes, text));
}

/// Answer the main-UI dev IPC request once its last event has landed: runs
/// after the egui pass, the present, and the post-frame `reconcile_cursor`,
/// so every field describes the frame the rig asked about with the cursor
/// as the authority left it. Fields:
///
/// - `ok`, `action`; `error` on a failure.
/// - `cursor_free`: the winit side (`EngineState::cursor_free`, what the
///   window was told); `gui_cursor_free`: the GuiState mirror the sidebar
///   reads; `cursor_want_free`: what `reconcile_cursor`'s predicate wants
///   this frame; `background_no_cursor`: true in a script-launched rig,
///   where the window is never touched and so the first two stay false
///   whatever the third says (read `cursor_want_free` there).
/// - `cloud_dev_sidebar_expanded`, `show_cloud_dev_panel`, `cloud_dev_collapsed`.
/// - `active_page` (the `GuiPage` variant name), `world_loaded`.
/// - `wants_pointer_input`, `is_pointer_over_area`: egui's own answers as
///   of the frame that just ran (over the sidebar both are true; over the
///   world with nothing under the pointer both are false).
/// - `flag`, `flag_before`, `flag_after` when a flag was named.
/// - `pos_px`, `pos_points`, `pixels_per_point` for the pointer verbs.
/// - `closed` for "escape": whether the rule closed the sidebar.
pub(crate) fn complete_ui_request(state: &mut EngineState) {
    let Some(ipc) = state.ui_ipc.as_ref() else { return };
    if ipc.stage != UiIpcStage::Complete {
        return;
    }
    let ipc = state.ui_ipc.take().expect("checked above");
    let gui = &state.gui_state;
    let mut done = serde_json::json!({
        "ok": true,
        "action": ipc.action,
        "cursor_free": state.cursor_free,
        "gui_cursor_free": gui.cursor_free,
        "cursor_want_free": crate::engine::input::cursor_want_free(state),
        "background_no_cursor": state.background_no_cursor,
        "cloud_dev_sidebar_expanded": gui.cloud_dev_sidebar_expanded(),
        "show_cloud_dev_panel": gui.show_cloud_dev_panel,
        "cloud_dev_collapsed": gui.cloud_dev_collapsed,
        "active_page": format!("{:?}", gui.active_page),
        "world_loaded": state.world_loaded,
        "wants_pointer_input": state.egui_ctx.wants_pointer_input(),
        "is_pointer_over_area": state.egui_ctx.is_pointer_over_area(),
        "pixels_per_point": state.egui_ctx.pixels_per_point(),
    });
    if let Some(flag) = ipc.flag.as_deref() {
        done["flag"] = serde_json::json!(flag);
        done["flag_before"] = ipc.flag_before.clone().unwrap_or(serde_json::Value::Null);
        done["flag_after"] = crate::engine::input::cloud_dev_flag(gui, flag).unwrap_or(serde_json::Value::Null);
    }
    if let (Some(px), Some(pt)) = (ipc.pos_px, ipc.pos_points) {
        done["pos_px"] = serde_json::json!([px.0, px.1]);
        done["pos_points"] = serde_json::json!([pt.x, pt.y]);
    }
    if let Some(closed) = ipc.closed {
        done["closed"] = serde_json::json!(closed);
    }
    if ipc.action == "find" {
        // Points -> window pixels, so `pos_px` can go straight into a
        // `click` request. `found: false` is an answer, not an error.
        let ppp = state.egui_ctx.pixels_per_point();
        done["query"] = serde_json::json!(ipc.find_text.clone().unwrap_or_default());
        match ipc.found.flatten() {
            Some(f) => {
                let r = f.rect;
                let c = r.center();
                done["found"] = serde_json::json!(true);
                done["text"] = serde_json::json!(f.text);
                done["matches"] = serde_json::json!(f.matches);
                done["rect_points"] = serde_json::json!([r.min.x, r.min.y, r.max.x, r.max.y]);
                done["rect_px"] = serde_json::json!([r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp]);
                done["pos_points"] = serde_json::json!([c.x, c.y]);
                done["pos_px"] = serde_json::json!([c.x * ppp, c.y * ppp]);
            }
            None => {
                done["found"] = serde_json::json!(false);
                done["matches"] = serde_json::json!(0);
            }
        }
    }
    write_ui_done(done);
}

/// Write `debug/ui_request_done.json`.
fn write_ui_done(done: serde_json::Value) {
    const DONE_PATH: &str = "debug/ui_request_done.json";
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
        // Optional "screen": park FACING a placed in-world screen instead
        // of over the deck: `{"station":"home","screen":"wall_screen_3"}`
        // puts the camera `distance_m` (default 2, inside the 3.5 m input
        // reach) straight out from the display's centre, at its height,
        // looking back at it. The pose is computed from the screen's own
        // quad, so it is exact for any screen in any room and never needs
        // a hand-typed coordinate. Resolved BEFORE any state changes so an
        // unknown id fails cleanly with the placed ids listed.
        let screen_pose: Option<(Vec3, Vec3)> = match v.get("screen").and_then(|s| s.as_str()) {
            None => None,
            Some(id) => match state.screens.quads.iter().find(|q| q.id == id) {
                Some(q) => {
                    let distance = v.get("distance_m").and_then(|d| d.as_f64()).unwrap_or(2.0).max(0.3) as f32;
                    let centre = q.geom.origin + (q.geom.u_axis + q.geom.v_axis) * 0.5;
                    let n = q.geom.normal.normalize_or_zero();
                    Some((centre + n * distance, -n))
                }
                None => {
                    let known: Vec<&str> = state.screens.quads.iter().map(|q| q.id.as_str()).collect();
                    fail(format!("no screen named {id:?}; placed screens: {known:?}"));
                    return;
                }
            },
        };
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
        // A deliberately FIXED pose: over the deck by default (the whole
        // point is that several captures differ only by the clock, so the
        // camera must not vary by so much as a pixel between them), or in
        // front of the named screen. Screen quads live in the home frame;
        // the camera is in render space, hence `+ station_off` (zero while
        // riding the station, kept for correctness).
        let hull_top = state.homestead_bounds.map(|(_, mx)| mx.y).unwrap_or(20.0);
        let (position, look) = match screen_pose {
            Some((p, look)) => (p + state.station_off, glam::DVec3::new(look.x as f64, look.y as f64, look.z as f64)),
            None => (Vec3::new(0.0, hull_top + 14.0, 34.0), glam::DVec3::new(0.0, -0.34, -1.0).normalize()),
        };
        state.camera.position = position;
        state.camera.clear_surface();
        let (yaw, pitch) = crate::dev_travel::look_angles(look);
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
        let mut done = serde_json::json!({"ok": true, "station": "home"});
        if let Some(id) = v.get("screen").and_then(|s| s.as_str()) {
            done["screen"] = serde_json::json!(id);
            done["position"] = serde_json::json!([position.x, position.y, position.z]);
            done["look"] = serde_json::json!([look.x, look.y, look.z]);
            log::info!("Camera request: parked aboard the home station facing screen {id:?} at {position:?}");
        } else {
            log::info!("Camera request: parked aboard the home station");
        }
        let _ = std::fs::write(DONE_PATH, done.to_string());
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

// ── Rig determinism pins: the tests ──────────────────────────────────────────
//
// These pin the two rules that are easy to get silently wrong and impossible to
// notice from a capture:
//   1. a pinned 0 must NOT be published as a literal 0, because the shader
//      reads that as "no publisher" and hands back a 4 m/s breeze - the pin
//      would then do the OPPOSITE of what it says, with nothing in the log,
//      the manifest or the picture to say so; and
//   2. "auto" must genuinely release the pin, or every later vantage in the
//      sweep inherits it (showcase pins are sticky across cells - the diag
//      channels needed an explicit per-vantage reset for the same reason).
// Both were verified red by inverting the rule before shipping.
#[cfg(test)]
mod showcase_pin_tests {
    use super::*;

    /// A plain unit wind direction, the shape WeatherSystem always publishes
    /// (`Vec3::new(angle.cos(), 0.0, angle.sin()).normalize()`).
    fn east() -> Vec3 {
        Vec3::new(1.0, 0.0, 0.0)
    }

    #[test]
    fn showcase_pin_parses_numbers_and_auto_and_rejects_junk() {
        assert_eq!(parse_showcase_pin("auto"), Some(ShowcasePin::Auto));
        assert_eq!(parse_showcase_pin("AUTO"), Some(ShowcasePin::Auto));
        assert_eq!(parse_showcase_pin(" auto "), Some(ShowcasePin::Auto));
        assert_eq!(parse_showcase_pin("0"), Some(ShowcasePin::Value(0.0)));
        assert_eq!(parse_showcase_pin("8.5"), Some(ShowcasePin::Value(8.5)));
        assert_eq!(parse_showcase_pin("300"), Some(ShowcasePin::Value(300.0)));
        // A typo must leave the pin ALONE rather than resetting it: a silent
        // mid-sweep state change is exactly what nothing ever prints.
        assert_eq!(parse_showcase_pin("clam"), None);
        assert_eq!(parse_showcase_pin(""), None);
        // NaN / inf would poison the uniform and every sine downstream of it.
        assert_eq!(parse_showcase_pin("NaN"), None);
        assert_eq!(parse_showcase_pin("inf"), None);
    }

    #[test]
    fn the_request_body_the_rig_writes_reaches_the_pins_by_their_real_key_names() {
        // Covers the one link a pure-function test would otherwise miss: the
        // KEY NAMES. A vantage that writes {"wind":"0"} and a handler that
        // reads "winds" would leave every test above green and every capture
        // silently unpinned. These are the exact strings tests/visual/
        // vantages.json and probe-sweep.js write.
        let body = r#"{"time":"2.9","weather":"clear","wind":"0","anim_clock":"300"}"#;
        assert_eq!(showcase_value(body, "wind").as_deref(), Some("0"));
        assert_eq!(showcase_value(body, "anim_clock").as_deref(), Some("300"));
        assert_eq!(
            parse_showcase_pin(&showcase_value(body, "wind").unwrap()),
            Some(ShowcasePin::Value(0.0))
        );
        assert_eq!(
            parse_showcase_pin(&showcase_value(body, "anim_clock").unwrap()),
            Some(ShowcasePin::Value(300.0))
        );
        // The release form probe-sweep sends for every unpinned vantage.
        let release = r#"{"wind":"auto","anim_clock":"auto"}"#;
        assert_eq!(parse_showcase_pin(&showcase_value(release, "wind").unwrap()), Some(ShowcasePin::Auto));
        assert_eq!(
            parse_showcase_pin(&showcase_value(release, "anim_clock").unwrap()),
            Some(ShowcasePin::Auto)
        );
        // An absent key must read as absent, not as some default: that is what
        // makes "the handler leaves the pin alone" mean anything.
        assert_eq!(showcase_value(r#"{"time":"2.9"}"#, "wind"), None);
    }

    #[test]
    fn unpinned_wind_publishes_the_weather_value_unchanged() {
        let prev = [0.86, 0.0, 0.32, 4.0];
        let out = published_foliage_wind(None, east(), 12.5, prev);
        assert_eq!(out[3], 12.5, "no pin means the weather wind reaches the shader");
        assert!((Vec3::new(out[0], out[1], out[2]).length() - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn a_pinned_zero_is_never_published_as_a_literal_zero() {
        // THE TRAP, from `00-bindings-vertex.wgsl`:
        //   if (dot(wind_w, wind_w) < 0.25 || wind_v <= 0.0) {
        //       wind_w = vec3(0.86, 0.0, 0.32); wind_v = 4.0;
        //   }
        // A published 0 therefore comes back out of the shader as a 4 m/s
        // breeze. The pin has to clear that guard from above, never sit on it.
        let out = published_foliage_wind(Some(0.0), east(), 4.0, [0.86, 0.0, 0.32, 4.0]);
        assert!(out[3] > 0.0, "a published speed of 0 trips the shader's fallback breeze");
        assert_eq!(out[3], FOLIAGE_WIND_PIN_MIN);
        // Still arithmetically still: the static lean is 6e-4 * v^2 of a tree
        // height, so at 1e-4 m/s it is 6e-12 - nothing a capture can resolve.
        assert!(6.0e-4 * out[3] * out[3] < 1.0e-9);
    }

    #[test]
    fn a_pinned_zero_keeps_the_direction_long_enough_to_clear_the_guard() {
        // The shader has a SECOND fallback, on the direction: dot(w,w) < 0.25.
        // A pinned speed with a short direction vector would be discarded the
        // same way a pinned 0 is, so the published direction must always be
        // unit - including when the weather hands us a degenerate one.
        let out = published_foliage_wind(Some(0.0), Vec3::ZERO, 4.0, [0.86, 0.0, 0.32, 4.0]);
        let d = Vec3::new(out[0], out[1], out[2]);
        assert!(d.dot(d) >= 0.25, "published direction must clear the shader's dot(w,w) guard");
        assert!((d.length() - 1.0).abs() < 1.0e-5);
        assert_eq!(out[3], FOLIAGE_WIND_PIN_MIN);
    }

    #[test]
    fn a_pinned_speed_overrides_the_weather_in_both_directions() {
        // Calmer than the weather...
        let calm = published_foliage_wind(Some(1.0), east(), 18.0, [1.0, 0.0, 0.0, 18.0]);
        assert_eq!(calm[3], 1.0);
        // ...and stormier, so the rig can photograph a gale on a clear day.
        let gale = published_foliage_wind(Some(18.0), east(), 2.0, [1.0, 0.0, 0.0, 2.0]);
        assert_eq!(gale[3], 18.0);
    }

    #[test]
    fn auto_restores_the_weather_value() {
        let pinned = published_foliage_wind(Some(0.0), east(), 7.0, [1.0, 0.0, 0.0, 7.0]);
        assert_eq!(pinned[3], FOLIAGE_WIND_PIN_MIN);
        let released = published_foliage_wind(None, east(), 7.0, pinned);
        assert_eq!(released[3], 7.0);
    }

    #[test]
    fn a_degenerate_weather_direction_keeps_the_last_published_one() {
        // Pre-pin behaviour, preserved exactly: the old site rewrote only the
        // speed when the weather direction could not be normalised.
        let prev = [0.0, 0.0, 1.0, 4.0];
        let out = published_foliage_wind(None, Vec3::ZERO, 9.0, prev);
        assert_eq!([out[0], out[1], out[2]], [prev[0], prev[1], prev[2]]);
        assert_eq!(out[3], 9.0);
    }

    #[test]
    fn the_wind_pin_cannot_freeze_a_canopy_on_its_own() {
        // The honest limit of the wind pin, kept as an executable note because
        // it is the whole reason `anim_clock` exists beside it.
        //
        // The sway amplitude in `00-bindings-vertex.wgsl` is
        //     sway_amp = h * (0.020 + 0.55 * hn * lean_frac)
        // and `lean_frac` is the only wind-dependent term. At a pinned zero the
        // 0.020 breathe survives: 0.44 m at the tip of a 22 m fir, oscillating
        // at `sway_hz = 0.9 + 0.02 * wind_v`, i.e. on the animation clock. So
        // a wind pin alone leaves a forest moving between two boots, which is
        // exactly what 42 to 44 percent of differing pixels looked like.
        let h = 22.0_f32; // a full-height fir vertex, metres
        let hn = 1.0_f32; // cantilever profile at the tip
        let amp = |v: f32| h * (0.020 + 0.55 * hn * (6.0e-4 * v * v).min(0.30));
        let pinned = published_foliage_wind(Some(0.0), east(), 4.0, [1.0, 0.0, 0.0, 4.0])[3];
        assert!(
            amp(pinned) > 0.4,
            "a pinned-calm canopy still breathes ~0.44 m at the tip: only anim_clock stops it"
        );
        // And the pin DOES remove the wind-driven part, which is why it is
        // still worth having: at 4 m/s the amplitude is measurably larger.
        assert!(amp(4.0) > amp(pinned));
    }
}

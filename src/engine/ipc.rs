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

/// The longest reconnect hold the `drop_link` showcase verb takes, in seconds.
/// A longer ask is held at this. Two minutes is already past the relay's 90 s
/// reconnect grace, so nothing a rig needs is lost, and a stray digit can no
/// longer leave a dev game cut off for hours with nothing on screen saying so
/// (the final review of ship homes increment 2).
pub(crate) const DROP_LINK_MAX_HOLD_SECS: f32 = 120.0;

/// The reconnect hold a `drop_link` showcase value asks for, in seconds: never
/// below the backoff's first rung and never above DROP_LINK_MAX_HOLD_SECS. None
/// for a value that is not a finite number (the verb then does nothing).
pub(crate) fn drop_link_hold(raw: &str) -> Option<f32> {
    let hold = raw.trim().parse::<f32>().ok().filter(|s| s.is_finite())?;
    Some(hold.clamp(crate::net::ws_client::RECONNECT_DELAY_INITIAL_SECS, DROP_LINK_MAX_HOLD_SECS))
}

/// What the showcase `place` verb asks for (ship homes increment 5): a blueprint, a floor point in
/// ship metres, and its quarter turns.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlaceSpec {
    pub blueprint: String,
    pub x: f32,
    pub z: f32,
    pub turns: u8,
}

/// Read a `place` verb's value, `"<blueprint>@<x>,<z>,<turns>"` (`wood_foundation@48,135,0`): a
/// blueprint id, two finite numbers and a turn from 0 to 3. None for anything else, so a typo
/// builds nothing.
pub(crate) fn parse_place(raw: &str) -> Option<PlaceSpec> {
    let (blueprint, rest) = raw.trim().split_once('@')?;
    let blueprint = blueprint.trim();
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    let [x, z, turns] = parts.as_slice() else { return None };
    let x = x.parse::<f32>().ok().filter(|v| v.is_finite())?;
    let z = z.parse::<f32>().ok().filter(|v| v.is_finite())?;
    let turns = turns.parse::<u8>().ok().filter(|t| *t < 4)?;
    (!blueprint.is_empty()).then(|| PlaceSpec { blueprint: blueprint.to_string(), x, z, turns })
}

/// Read a `take_down` verb's value: the relay's id of a piece the server keeps.
pub(crate) fn parse_take_down(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

/// What a showcase `hull` pin asks for: "0" hides the ship's hull, "1" shows it (the H key's
/// toggle, `GuiState::show_hull`). None for anything else, so a typo leaves the hull as it is.
pub(crate) fn hull_pin(raw: &str) -> Option<bool> {
    match raw.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
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

/// What a showcase `fov` pin holds: `auto` releases it (None), a number is
/// the lens in vertical degrees, held between 10 (below that the frame is a
/// telescope, and nothing in the world is built to be seen through one) and
/// 120.
pub(crate) fn fov_pin_from(pin: ShowcasePin) -> Option<f32> {
    match pin {
        ShowcasePin::Auto => None,
        ShowcasePin::Value(v) => Some(v.clamp(10.0, 120.0)),
    }
}

/// The camera's vertical fov: the showcase lens while one is pinned, else
/// the Settings value under the 60..120 clamp (a corrupt `"fov": 0` in a
/// config must never collapse the projection). The one rule for both
/// writers, the showcase verb and lib.rs's settings_dirty block, so a
/// Settings apply cannot drop a pinned lens mid-shot.
pub(crate) fn effective_fov(fov_pin: Option<f32>, settings_fov: f32) -> f32 {
    fov_pin.unwrap_or(settings_fov.clamp(60.0, 120.0))
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

/// data/world/showcase.ron shape (v0.863 perpetual showcase). Lives with the grow
/// machines' food model, which reads the same crops to compute each machine's figure.
pub(crate) use crate::systems::grow_machines::ShowcaseCfg;

/// Whether the showcase may plant the garden now, or why not (`auto_seed_showcase`'s first
/// check, kept pure so it is tested without a window). A free, ripe garden is a free resource,
/// so it is planted only while free resources are on: a play mode that gives them (Creative and
/// Dev) with the Inventory page's Creative toggle on, which Normal pins off (first-hour audit
/// 2026-10-04, Missing stakes 2: it replanted the whole garden whenever it was empty, in every
/// mode, so a Normal player never had to plant or wait for anything). The probe rigs keep their
/// garden vantages: a rig is a Dev-mode sandbox. A garden that grows is never replanted.
pub(crate) fn showcase_gate(
    world: &hecs::World,
    play_mode: crate::config::PlayMode,
    creative: bool,
) -> Result<(), &'static str> {
    if !(play_mode.allows(crate::config::Capability::FreeResources) && creative) {
        return Err("free resources are off (Normal mode, or the Creative toggle), so the garden is the player's to plant");
    }
    if world.query::<&crate::ecs::components::CropInstance>().iter().next().is_some() {
        return Err("crops already present");
    }
    Ok(())
}

/// Perpetual showcase auto-seed (v0.863). Operator: "just preload everything
/// with plants at different stages... a perpetual showcase." Whenever the
/// world holds ZERO crops (fresh boot, empty save, everything harvested),
/// replant every grow surface at staggered growth stages: each tower column
/// gets its config's curated plantings (or a whole-tower override, e.g. the
/// ntower_0 strawberry hero), each bed/field/rack gets its mapped crop.
/// Crops persist in the save now, so this fires only on a genuinely empty
/// garden. Called every frame; the empty check makes it ~free. Only while
/// free resources are on (Creative and Dev): see `showcase_gate`.
pub(crate) fn auto_seed_showcase(state: &mut EngineState) {
    // One-shot guard diagnostics: when seeding is NOT happening, say why
    // once, so an empty garden is never a silent mystery.
    static DIAGNOSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let diag = |why: &str| {
        if !DIAGNOSED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            log::info!("[Showcase] auto-seed idle: {why}");
        }
    };
    if let Err(why) = showcase_gate(
        &state.game_world.world,
        state.gui_state.settings.play_mode,
        state.gui_state.creative_mode,
    ) {
        diag(why);
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
    // A garden day: 24 hours of the one game clock (farming's day, 2026-09-27).
    const DAY: f64 = crate::systems::time::EARTH_DAY_S;

    // Collect the spawn list first (no world borrow while iterating data).
    let mut to_spawn: Vec<crate::ecs::components::CropInstance> = Vec::new();
    let mut stagger = |list: &mut Vec<crate::ecs::components::CropInstance>,
                       plant_id: &str,
                       grow_id: &str,
                       slot: u32,
                       frac: f32| {
        let Some(def) = reg.get(plant_id) else { return };
        let stages = def.stages();
        // Its stage and its age on the game clock, which agree: there is no
        // separate growth speed (the one clock, 2026-09-27).
        list.push(crate::ecs::components::CropInstance {
            crop_def_id: plant_id.to_string(),
            growth_stage: crate::systems::farming::stage_from_progress(frac, &stages).to_string(),
            planted_at: elapsed - def.growth_days as f64 * DAY * frac as f64,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some(grow_id.to_string()),
            tower_slot: Some(slot),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        });
    };
    // Beds sow one crop per plot (the grow medium's `plots`, 2026-09-26),
    // staggered across all the machines of a type through six stages.
    let mut sown_of_type: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
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
            // One crop per cup, the plantings cycled: the same rule the grow
            // machines' food figure counts (grow_machines::tower_cup_crops).
            let cups = crate::systems::grow_machines::tower_cup_crops(&cycle, slots);
            for (slot, plant) in (0u32..).zip(&cups) {
                let frac = slot as f32 / (slots.max(2) - 1) as f32;
                stagger(&mut to_spawn, plant, &g.id, slot, frac);
            }
        } else if let Some(plant) = cfg.bed_crops.get(&g.ty) {
            let plots = state.gui_state.grow_media.iter().find(|m| m.matches(&g.ty)).map_or(1, |m| m.plots.max(1));
            for u in 0..plots {
                let k = sown_of_type.entry(g.ty.as_str()).or_insert(0);
                let frac = (*k % 6) as f32 / 5.0;
                *k += 1;
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
    // The `hold` verb's keys come up when their time is up: every frame, so
    // before the no-request early return (engine/rig_walk.rs).
    crate::engine::rig_walk::tick(state);
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
    // {"build":"wood_wall@0,-2;roof@0,0","build_at":"23.0005,14"} (2026-09-27,
    // BUG-102): stand built pieces up finished around a ground point (the
    // lat, lon on the body the camera is locked to, or the crosshair), in the
    // real build site and with the real placement, so a rig capture can see
    // pieces built on a planet. `id@dx,dz,turns` items, `;`-joined; see
    // engine::planet_build::dev_build. Pair it with a vantage's
    // `post_showcase` (probe-sweep.js): the ground must have streamed in.
    if let Some(spec) = grab("build") {
        let note = crate::engine::planet_build::dev_build(state, &spec, grab("build_at").as_deref());
        log::info!("Showcase: build -> {note}");
    }
    // {"stand":"dx,h,dz,heading,pitch","stand_at":"lat,lon"} (2026-09-27): put
    // the eye h metres over the ground at dx, dz from that point, in its build
    // site, looking along heading (degrees from north) and pitch, so a capture
    // can stand INSIDE a hut the build verb stood up; the camera park cannot
    // (it parks tens of metres up). See engine::planet_build::dev_stand. Send
    // it as a vantage's `final_showcase` (probe-sweep.js), after the re-park.
    if let Some(spec) = grab("stand") {
        let note = crate::engine::planet_build::dev_stand(state, &spec, grab("stand_at").as_deref());
        log::info!("Showcase: stand -> {note}");
    }
    // {"walk":"1"} (2026-09-28): fly mode OFF, gravity on, the way a player
    // on foot is. Every camera park sets fly mode, and fly mode suspends the
    // survival rules, so until this a capture on a planet showed the HUD's
    // fly-mode line ("Indoors 21C, still air") and never the outdoor
    // weather line, the shelter note or what the air feels like. Send it in
    // a vantage's final_showcase with a stand (probe-sweep.js), so the eye is
    // already on the ground when gravity comes back.
    if grab("walk").as_deref() == Some("1") {
        state.gui_state.dev_fly_mode = false;
        state.gui_state.dev_hover = false;
        state.controller.fly_mode = false;
        log::info!("Showcase: walk -> fly mode off, on foot");
    }
    // {"hold":"forward,sprint","hold_s":"40"} (BUG-156, 2026-10-05): press
    // those movement keys for that long, through the controller's own action
    // path, so a capture can ARRIVE somewhere on foot the way a player does
    // rather than by teleport (engine/rig_walk.rs). Send it after a stand and
    // a walk, in the same request or a later one.
    if let Some(spec) = grab("hold") {
        let secs = grab("hold_s").and_then(|s| s.parse::<f32>().ok()).unwrap_or(5.0);
        let note = match crate::engine::rig_walk::parse_keys(&spec) {
            Some(keys) => crate::engine::rig_walk::hold(state, keys, secs),
            None => format!("hold wants movement keys (forward, back, left, right, jump, sprint), not {spec:?}"),
        };
        log::info!("Showcase: hold -> {note}");
    }
    // {"tree_ground":"1"} (BUG-156, 2026-10-05): write where every near tree's
    // base and the eye stand against the ground drawn under them, the finest
    // ground and the surface the harvest sampled, to debug/tree_ground.json
    // (engine/tree_ground.rs). The probe rig's `ground_probe` check reads it.
    if grab("tree_ground").as_deref() == Some("1") {
        let note = crate::engine::tree_ground::write_report(state);
        log::info!("Showcase: tree_ground -> {note}");
    }
    // {"solo":"1"} / {"solo":"0"} (2026-10-03, ship homes 1b): step out of the shared world
    // and back in, the switch the launcher's offline-home pick and Dev travel flip
    // (`copresence_solo`; lib.rs sends game_leave, then joins again once it clears). The
    // relay spawns the returning player afresh, at their door, wherever the game stands:
    // verify-copresence --plots uses it to check the game then stands where the relay holds
    // it (engine/home_plot.rs `stand_where_held`, the second review's freeze).
    match grab("solo").as_deref() {
        Some("1") => {
            state.gui_state.copresence_solo = true;
            log::info!("Showcase: solo -> stepping out of the shared world");
        }
        Some("0") => {
            state.gui_state.copresence_solo = false;
            log::info!("Showcase: solo off -> joining the shared world again");
        }
        _ => {}
    }
    // {"drop_link":"12"} (2026-10-04, ship homes 2 review, finding 2): drop the active server
    // connection the way a network outage does, with no game_leave, and hold the reconnect for
    // that many seconds (the backoff's own timer), at most DROP_LINK_MAX_HOLD_SECS (120 s; a
    // longer ask is held at that, `drop_link_hold`). The game leaves the shared world on its side at
    // once (a guest's home comes back onto the ship, `home_plot::forget_shared_world`), the relay
    // keeps the figure for its 90 s grace, and the reconnect's welcome says `rejoin`.
    // verify-copresence's guest leg walks the game into the home that came back, then judges the
    // welcome stands it off the plot again. Permanent dev tooling.
    if let Some(hold) = grab("drop_link").as_deref().and_then(drop_link_hold) {
        if let Some(ws) = state.gui_state.ws_client.as_mut() {
            ws.disconnect();
        }
        state.gui_state.ws_reconnect_delay = hold;
        log::info!("Showcase: drop_link -> the connection dropped (no game_leave); reconnecting in {hold} s");
    }
    // {"die":"<cause>"} (2026-10-04, the Death setting): the player dies of that cause, through
    // the slot every death goes through, so the death screen comes up and, in Realistic, the
    // backpack is left in a pack where they stand (engine/death_pack.rs). With the respawn
    // verb below, a rig can die, respawn and walk back to the pack. Permanent dev tooling.
    if let Some(cause) = grab("die") {
        let note = crate::engine::death_pack::dev_die(state, &cause);
        log::info!("Showcase: die -> {note}");
    }
    // {"respawn":"1"} (2026-10-03, ship homes 1b, the third review): press the death screen's
    // Respawn button (`pending_respawn`): the player goes to their Respawn point, and in the
    // shared world steps out and joins again so the relay stands them there too
    // (engine/home_plot.rs `respawn_through_relay`). verify-copresence --plots walks the game
    // more than 100 m from its door first, then judges it stands where the relay respawned it.
    if grab("respawn").as_deref() == Some("1") {
        state.gui_state.pending_respawn = true;
        log::info!("Showcase: respawn -> the Respawn button");
    }
    // {"walk_to":"x,y,z,yaw,pitch,speed"} (2026-10-04, ship homes increment 4): walk the camera
    // there in a straight line at that many metres a second, the way a person walks it with the
    // mouse and W: it turns to face the way it goes, walks, and at the point turns to yaw and
    // pitch (BUG-165, engine/move_check.rs `walk_step`; it used to hold yaw and pitch the whole
    // way, so it walked backwards); the probe's `moves.walking` stays true until that last turn
    // is done. The relay's speed check corrects a jump nobody could make, which the `cam` verb's
    // teleport is past a few metres, so verify-copresence walks the game through the ship with
    // this instead (it moved it in 40 m teleports while the relay's rule was 100 m per update).
    // It advances only while the game is in the shared world (engine/move_check.rs
    // `walk_tick`). Permanent dev tooling.
    if let Some(spec) = grab("walk_to") {
        match crate::engine::move_check::parse_walk(&spec) {
            Some(w) => {
                state.moves.walk = Some(w);
                state.gui_state.active_page = crate::gui::GuiPage::None;
                log::info!("Showcase: walk_to -> walking to {:?} at {} m/s", w.to, w.speed);
            }
            None => log::warn!("Showcase: walk_to wants \"x,y,z,yaw,pitch,speed\" with a speed above 0, not {spec:?}"),
        }
    }
    // {"build_editor":"1"} / {"build_editor":"0"} (2026-10-04, ship homes 1b, round 5): open or
    // shut the construction editor the way the B key does (engine/editor.rs
    // `toggle_build_editor`, under the B key's own conditions: the world view, no showroom).
    // verify-copresence --plots walks the game more than 100 m from its build spot, opens and
    // shuts the editor, and judges it still stands where the relay holds it
    // (engine/home_plot.rs `editor_close_spot`).
    if let Some(want) = grab("build_editor").as_deref().and_then(|v| match v {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }) {
        let free = state.gui_state.active_page == crate::gui::GuiPage::None && !state.gui_state.showroom_active;
        if want != state.gui_state.construction_active && free {
            crate::engine::editor::toggle_build_editor(state);
        }
        log::info!(
            "Showcase: build_editor {} -> the editor is {}",
            if want { "open" } else { "shut" },
            if state.gui_state.construction_active { "open" } else { "shut" }
        );
    }
    // {"stock":"1"} (ship homes increment 5, 2026-10-05): the Crafting page's "Dev: stock all
    // materials" (a stack of every recipe input into the pack), under the button's own rule, the
    // Dev mode with dev cheats on. verify-copresence --build stocks the game before it places.
    // Permanent dev tooling.
    if grab("stock").as_deref() == Some("1") {
        if state.gui_state.dev_cheats_active(&state.theme) {
            state.gui_state.dev_stock_materials = true;
            log::info!("Showcase: stock -> a stack of every recipe input into the pack");
        } else {
            log::warn!("Showcase: stock refused: like the Crafting page's button, it is the Dev mode's, with dev cheats on");
        }
    }
    // {"place":"<blueprint>@<x>,<z>,<turns>"} (ship homes increment 5): what E does with that
    // piece in hand aimed at that floor point (ship metres), through E's own gate
    // (engine/build_place.rs `place_at`): kept by the server on the player's own plot, refused at
    // the crosshair elsewhere (the sentence goes up as a notice), nothing left in hand after it.
    // Permanent dev tooling.
    if let Some(spec) = grab("place") {
        match parse_place(&spec) {
            Some(p) => {
                let note = crate::engine::build_place::place_at(state, &p.blueprint, p.x, p.z, p.turns);
                log::info!("Showcase: place -> {note}");
            }
            None => log::warn!("Showcase: place wants \"<blueprint>@<x>,<z>,<turns>\" with turns 0 to 3, not {spec:?}"),
        }
    }
    // {"take_down":"<piece_id>"} (ship homes increment 5): what F does at that piece the server
    // keeps (engine/build_place.rs `take_down_piece`): asked of the relay when it would allow it,
    // the reason on screen when not. Permanent dev tooling.
    if let Some(spec) = grab("take_down") {
        match parse_take_down(&spec) {
            Some(id) => {
                let note = crate::engine::build_place::take_down_piece(state, id);
                log::info!("Showcase: take_down -> {note}");
            }
            None => log::warn!("Showcase: take_down wants the id of a piece the server keeps, not {spec:?}"),
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
    // Optional "time_scale":"0" holds the game clock at that SPEED (v0.1287,
    // the rig clock freeze): 0 holds the clock still so the planet does not
    // spin and the sun does not move between a park and its capture. Two
    // down-look captures in one sweep came out rotated about the nadir
    // (2026-09-05) because a fast day turned the planet 0.3 degrees per
    // second under a world-fixed camera. A hold over the time-speed setting
    // (time::request_speed_hold); the TimeSystem's accumulator is authoritative.
    if let Some(sc) = grab("time_scale").and_then(|t| t.parse::<f32>().ok()) {
        crate::systems::time::request_speed_hold(&state.data_store, Some(sc.max(0.0)));
    }
    // Optional "fov":"40" sets the camera's vertical field of view in degrees;
    // "fov":"auto" puts the Settings value back (2026-10-04, the clip maker's
    // lens, scripts/make-clips.js). A far mountain needs a longer lens than
    // the 90 degree play default: Mount Rainier seen from Silverdale, 113 km
    // off, stands 1.7 degrees over the horizon, about 20 px of a 1080 px
    // frame at 90 degrees. Held until "auto" (state.fov_pin): lib.rs's
    // Settings apply goes through effective_fov too, so a config save in the
    // middle of a shot keeps the lens. The terrain LOD reads the camera's fov
    // (px_per_rad in lib.rs), so a long lens also asks for the finer far
    // patches it needs.
    if let Some(f) = grab("fov") {
        match parse_showcase_pin(&f) {
            Some(pin) => {
                state.fov_pin = fov_pin_from(pin);
                state.camera.fov_degrees = effective_fov(state.fov_pin, state.gui_state.settings.fov);
                log::info!("Showcase: fov -> {} deg", state.camera.fov_degrees);
            }
            None => log::warn!("Showcase: fov \"{f}\" is not a number or \"auto\" - lens unchanged"),
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
    // {"aurora":"0"} skips the fullscreen emission pass (the aurora) and
    // nothing else; "1" restores it. A measuring instrument: a fixture and its
    // aurora-OFF twin differ ONLY by the light the aurora delivers, which is
    // what scripts/aurora-gate.js subtracts (in linear light) to prove the
    // cloud deck no longer dims it (PRIORITIES 1b).
    if let Some(t) = grab("aurora") {
        state.renderer.emission.off = t == "0";
        log::info!(
            "Showcase: aurora -> {}",
            if state.renderer.emission.off { "OFF (emission pass skipped)" } else { "on" }
        );
    }
    // {"room_gi":"0"} switches room GI off (2026-09-27, docs/design/room-gi.md):
    // the room table says "off", every interior fragment keeps the old 0.005
    // ambient floor, and the probe update is not dispatched. "1" turns it back
    // on (the probes resume from where they stood). The same-boot A/B for both
    // the look and the cost: a capture and its room-GI-off twin differ ONLY by
    // the indirect light the probes add, and gpu.room_probes plus the sampling
    // share of gpu.scene is what it costs.
    if let Some(t) = grab("room_gi") {
        state.renderer.room_gi.off = t == "0";
        log::info!(
            "Showcase: room_gi -> {} ({} probes)",
            if state.renderer.room_gi.off { "OFF (the old ambient floor)" } else { "on" },
            state.renderer.room_gi.probe_count()
        );
    }
    // {"hull":"0"} hides the ship's hull and "1" shows it again (2026-10-05): what the H key and
    // Settings > Graphics > "Show hull (H)" do (`GuiState::show_hull`, never saved), so a rig sees
    // the ship the way a player who pressed H does. The hull's plating covers every plot but the
    // home's own (its glass roof cuts a hole), so the homes along First Street are only in view
    // with it hidden (the ship-first-street vantage). Sticky until "1": the rigs send "1" before
    // every other vantage (scripts/lib/showcase-pins.js).
    if let Some(t) = grab("hull") {
        match hull_pin(&t) {
            Some(shown) => {
                state.gui_state.show_hull = shown;
                log::info!("Showcase: hull -> {}", if shown { "shown" } else { "HIDDEN (as the H key)" });
            }
            None => log::warn!("Showcase: hull \"{t}\" is not 0 or 1 - the hull is left as it is"),
        }
    }
    // {"pipe_marking":"full"} draws the pipes' marker bands as the scheme's whole marker,
    // "simplified" as one band of the main colour, "auto" hands the choice back to Settings >
    // Gameplay > Pipe markings (2026-10-04, engine::pipe_markers). A PIN over the setting, never
    // a write to it, so a rig capture of each mode leaves the player's config alone. Sticky until
    // "auto": a vantage that cares pins its own mode, and the rigs send "auto" before every other
    // vantage (scripts/lib/showcase-pins.js). The pipes rebuild on the next frame.
    if let Some(t) = grab("pipe_marking") {
        use crate::ship::pipe_marking::MarkingMode;
        state.pipe_markers.pin = match t.as_str() {
            "full" => Some(MarkingMode::Full),
            "simplified" => Some(MarkingMode::Simplified),
            _ => None,
        };
        log::info!("Showcase: pipe_marking -> {:?} (None = Settings > Gameplay)", state.pipe_markers.pin);
    }
    // {"sun_shadows":"0"} switches the sun's shadow maps off, "1" on and
    // "auto" hands them back to Settings > Planets (2026-09-27,
    // docs/design/sun-cascades.md increment 0). A PIN over the setting, not a
    // write to it, so a capture and its sun-shadows-off twin differ ONLY by
    // what the maps take away (and gpu.shadow plus gpu.shadow_near is what
    // they cost), while the player's own config is never touched.
    if let Some(t) = grab("sun_shadows") {
        state.renderer.sun_cascades.shadows_pin = match t.as_str() {
            "auto" => None,
            v => Some(v != "0"),
        };
        log::info!("Showcase: sun_shadows -> {:?} (None = Settings > Planets)", state.renderer.sun_cascades.shadows_pin);
    }
    // {"room_gi_vis":"1"} makes every room run DDGI's Chebyshev visibility
    // test, which rung 1 skips because inside a room's own box it is an
    // identity; "0" restores the default. For measuring what the test costs,
    // and for seeing that it changes nothing until rung 2 traces contents.
    if let Some(t) = grab("room_gi_vis") {
        state.renderer.room_gi.force_visibility = t == "1";
        log::info!("Showcase: room_gi_vis -> {}", state.renderer.room_gi.force_visibility);
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
    // {"scene_format":"display"|"hdr"}: the HDR scene target's increment-3
    // A/B switch (renderer/scene_format_ab.rs). "display" rebuilds the scene
    // target and every scene pipeline in the display's 8-bit format (the
    // increments 1 and 2 path, where every pass quantised its own write);
    // "hdr" (the default) rebuilds them in Rgba16Float. The star sky is
    // built for the old format, so it is rebuilt here too, and a sky still
    // being built by the boot thread for the old format is dropped (the
    // next world entry builds one synchronously). Blocks this frame for the
    // megashader compile; the rig waits.
    if let Some(t) = grab("scene_format") {
        let t = t.trim().to_ascii_lowercase();
        let hdr = match t.as_str() {
            "hdr" | "float" | "1" => Some(true),
            "display" | "8bit" | "0" => Some(false),
            // An unknown value must be LOUD (an A/B arm that silently ran the
            // other arm is a gate that cannot fail).
            other => {
                log::warn!("[showcase] scene_format: unknown value {other:?} (want display|hdr), unchanged");
                None
            }
        };
        if let Some(hdr) = hdr {
            if state.renderer.switch_scene_format(hdr) {
                state.star_preload_rx = None;
                if state.world_loaded {
                    crate::engine::world_load::build_star_sky(state);
                }
            }
            log::info!("Showcase: scene_format -> {:?}", state.renderer.scene_format());
        }
    }
    // {"present_dither":"0"} turns off the one dither in the present pass
    // (HDR scene target, increment 4); "1" (the default) turns it back on.
    // The high-frequency gates (grain, speckle, comb, glint autocorrelation,
    // the pure-black census) pin it off so they read the scene, not the
    // dither; the rig resets it to "1" on every cell that does not pin it.
    if let Some(t) = grab("present_dither") {
        let t = t.trim();
        let on = t != "0";
        if on && t != "1" {
            log::warn!("[showcase] present_dither: unknown value {t:?} (want 0|1), dither stays on");
        }
        state.renderer.set_present_dither(on);
        log::info!("Showcase: present_dither -> {}", state.renderer.present_dither());
    }
    // {"present_direct":"1"} draws the scene straight into the display (the
    // pre-2026-09-27 path, no scene target, no present pass); "0" restores
    // the default. The HDR scene target's same-boot A/B switch of
    // increments 1 and 2: it proves the target changes no pixel and reads
    // what the present pass costs (renderer/scene_target.rs). It needs the
    // scene format to be the display's, which since increment 3 means
    // {"scene_format":"display"} first; otherwise it is ignored, and the log
    // says so here.
    if let Some(t) = grab("present_direct") {
        state.renderer.present_direct = t == "1";
        let live = state.renderer.scene_format() == state.renderer.surface_format(); // display-format: the switch needs the formats equal
        log::info!(
            "Showcase: present_direct -> {} ({})",
            state.renderer.present_direct,
            if live { "in force" } else { "IGNORED: the scene format is not the display format" }
        );
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
                // {"weather_wind":"0"} beside it pins the weather's own wind,
                // m/s (2026-09-29): a calm clear night is the case the body's
                // "feels" figure is most different from the air.
                let wind = grab("weather_wind").and_then(|x| x.parse::<f32>().ok()).map(|v| v.clamp(0.0, 60.0));
                c.manual = Some(crate::systems::weather::ManualWeather {
                    condition: cond,
                    intensity,
                    wind_speed: wind.unwrap_or(if intensity > 0.8 { 18.0 } else { 4.0 }),
                });
                c.retrigger = true;
            }
        }
        state.gui_state.weather_manual = true;
        state.gui_state.weather_pick_condition = cond;
        state.gui_state.weather_pick_intensity = intensity;
        // Pinning a condition places it HERE. Without this the pin would adopt
        // whatever anchor the previous condition left behind.
        state.weather_anchor = None;
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
        // Where the camera was when THIS picture was taken, in the home's
        // own coordinates, and whether it was riding the station (BUG-132).
        // probe-sweep compares it with a station vantage's requested pose,
        // so a camera that drifted or let go of the home during the settle
        // fails the vantage instead of passing as "captured". Off the
        // station the home-frame position is still well defined (the hull is
        // never rotated in render space), just far from anything.
        let home = camera_home_position(state.camera.position, state.station_off);
        obj.insert("camera_home".to_string(), serde_json::json!([home.x, home.y, home.z]));
        obj.insert(
            "camera_yaw_pitch".to_string(),
            serde_json::json!([state.camera.yaw, state.camera.pitch]),
        );
        obj.insert("station_ride".to_string(), serde_json::json!(state.station_ride));
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
/// re-run the normal scene passes into a scratch target in the scene format
/// and present it into the capture target (`render_view_onto`), read the
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
            daylight_over_anchor(
                state.frame_lock_anchor,
                state.current_spin,
                d.radius,
                state.sun_world_pos - state.ship_world_pos,
            )
        })
        .unwrap_or(false)
}

/// The daylight gate's own arithmetic, in pure form. `anchor_local` is the
/// frame lock's anchor, in the locked body's UNROTATED frame
/// (`dev_travel::frame_lock_capture`); `spin` is the body's turn
/// (`state.current_spin`, the same one the frame lock rides);
/// `to_sun_world` points from the camera to the sun in the WORLD frame.
pub(crate) fn daylight_over_anchor(anchor_local: glam::DVec3, spin: f64, radius: f64, to_sun_world: glam::DVec3) -> bool {
    let (sun_up, alt) = sun_over_anchor(anchor_local, spin, radius, to_sun_world);
    alt < 120_000.0 && sun_up > 0.10
}

/// The sine of the sun's elevation over the frame lock's ground point, and the
/// camera's height over the body's sphere: the numbers both the daylight gate
/// and the twilight fades read.
pub(crate) fn sun_over_anchor(anchor_local: glam::DVec3, spin: f64, radius: f64, to_sun_world: glam::DVec3) -> (f64, f64) {
    let alt = anchor_local.length() - radius;
    // The ground's up in the WORLD frame, where the sun is: the anchor turned
    // by the spin, exactly as frame_lock_ship_pos places the camera. Using
    // the unturned anchor judged the sun over another longitude, the same
    // one at every hour, so at Silverdale on 2026-10-04 the star pass was
    // skipped all night (BUG-138, daylight_gate_tests).
    let up = (glam::DQuat::from_rotation_y(spin) * anchor_local).normalize_or_zero();
    (up.dot(to_sun_world.normalize_or_zero()), alt)
}

/// What twilight leaves of each sky layer for the camera this frame
/// (`renderer::sky_frame::twilight_fades`): everything at night and away
/// from any body, the Milky Way first and the brightest stars last as the
/// sun comes up. Read from the same state as [`sky_daylight`], so the fades
/// and the gate always agree about the sun.
pub(crate) fn star_fades(state: &EngineState) -> crate::renderer::sky_frame::SkyFades {
    state
        .frame_lock_body
        .as_deref()
        .and_then(|b| state.planet_defs.get(b))
        .map(|d| {
            let (sun_up, alt) = sun_over_anchor(
                state.frame_lock_anchor,
                state.current_spin,
                d.radius,
                state.sun_world_pos - state.ship_world_pos,
            );
            crate::renderer::sky_frame::twilight_fades(sun_up.clamp(-1.0, 1.0).asin().to_degrees(), alt)
        })
        .unwrap_or(crate::renderer::sky_frame::SkyFades::FULL)
}

/// The star camera's turn (`StarRenderer::update_camera`'s `sky_rot`): render
/// frame to world (the hull frame aboard the homestead,
/// `station::render_to_world_rot`), then world to the star catalogue's
/// equatorial axes for today's date (`sky_frame::equatorial_to_world`, which
/// sets the world sun on the real sun's right ascension, BUG-139). The one
/// rule for the live frame and every off-screen view, so a camera screen and
/// the player can never see two different skies.
pub(crate) fn sky_rotation(state: &EngineState) -> glam::Quat {
    let days_since_j2000 = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0)
        / 86_400.0;
    let to_catalogue = crate::renderer::sky_frame::equatorial_to_world(state.sun_world_pos, days_since_j2000).inverse();
    (to_catalogue * crate::station::render_to_world_rot(state.station_ride, state.station_world_rot)).as_quat()
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
/// `target` must be a render attachment in the DISPLAY format
/// (`Renderer::surface_format`): the passes draw into a view-sized scratch
/// target in the scene format (`Renderer::begin_view_scene`), and the present
/// pass, which is built for the display format, writes `target` at the end,
/// exactly as the live frame presents its scene target to the swapchain
/// (renderer/scene_target.rs, HDR scene target increment 2, 2026-09-27). The
/// scratch is kept for the camera screens and dropped after a screenshot,
/// the same rule as the depth buffer. The caller checks `world_loaded`.
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
    // And its own colour target: every pass below draws into the scratch
    // (`target` from here on), and `end_view_scene` presents it into the
    // caller's display target (`display`).
    let display = target;
    let scratch = state.renderer.begin_view_scene(w, h, display);
    let target = &scratch;
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
            star_r.update_camera(&state.renderer.queue, camera, sky_rotation(state), star_fades(state));
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

    // Present the scratch into the caller's target, timed as this view's
    // present (`gpu.present` for the screenshot, `gpu.screen_present` for a
    // camera screen); the scratch is kept or dropped by the depth rule.
    state.renderer.end_view_scene(display, who, passes == ViewPasses::SceneOnly);

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

/// Where the camera request writes its answer. A station park writes it a
/// frame or two late, from `advance_station_park`, hence shared.
const CAMERA_DONE_PATH: &str = "debug/camera_done.json";

/// A station park waiting to report (BUG-132, 2026-10-03).
///
/// The `{"station":"home",...}` camera request places the camera at once, but
/// writes `debug/camera_done.json` only after the station block in lib.rs has
/// ridden that camera through the clock change the same request asked for.
/// The done file then says where the camera IS, in the home's own coordinates,
/// read on a later frame: a measurement, not an echo of the request. That is
/// what lets scripts/probe-sweep.js fail a vantage whose camera went somewhere
/// else (scripts/lib/station-park-check.js). Before this, the done file
/// repeated the verb's own arithmetic, so a park tens of thousands of km out
/// in space reported success.
pub(crate) struct StationPark {
    /// The home-frame eye position the verb placed the camera at.
    pub(crate) requested: Vec3,
    /// The look the verb set, (yaw, pitch), in the home frame.
    pub(crate) requested_yaw_pitch: (f32, f32),
    /// camera_done.json as the verb built it; the measured fields are added
    /// when the park reports.
    pub(crate) done: serde_json::Value,
    /// Station-block passes since the park (see `station_park_ready`).
    pub(crate) frames: u32,
}

/// How many station-block passes a park waits for the TimeSystem to take its
/// hour before reporting anyway (with `clock_settled: false`). The hour is
/// taken by the very next frame's system tick, so a park normally reports on
/// its second pass; this cap only matters if the clock never ticks at all.
pub(crate) const STATION_PARK_MAX_FRAMES: u32 = 120;

/// Is a pending station park ready to report, after `frames` station-block
/// passes, with the hour request still waiting (`clock_pending`)? Returns
/// `Some(clock_settled)` when ready and `None` to keep waiting.
///
/// The order inside one frame is what makes this right. The station block
/// propagates the orbit BEFORE the system tick that takes the hour request,
/// so the first pass after a park still rides the OLD clock. Only a pass that
/// begins with the request already taken has carried the station to where the
/// requested hour puts it (a jump of thousands of km for a few hours of a
/// synchronous orbit), and only a reading taken after that pass proves the
/// park survived the jump. A park that asked for no hour is ready after one
/// pass.
pub(crate) fn station_park_ready(frames: u32, clock_pending: bool) -> Option<bool> {
    if !clock_pending {
        Some(true)
    } else if frames >= STATION_PARK_MAX_FRAMES {
        Some(false)
    } else {
        None
    }
}

/// Where a station park puts the camera, in render space, for a position in
/// the home's own coordinates.
///
/// Render space is the ship frame, and the home is drawn at
/// `station_world_pos - ship_world_pos` in it (`station_off`). The park has
/// just set `ship_world_pos = station_world_pos`, so the offset of the frame
/// the camera now rides is ZERO and the home frame IS render space.
///
/// BUG-132 was adding the PREVIOUS frame's `station_off` here instead. That
/// value belonged to the frame the camera was leaving: after probe-sweep's
/// warm-up it was the whole distance from Earth's surface to the home, so the
/// camera landed tens of thousands of km out, the next frame read that as
/// having left the station, let go of it, and the capture showed empty space.
/// Computed from the two positions rather than written as a bare `home` so the
/// rule stays true if a park ever rides some other frame.
pub(crate) fn station_park_render_pos(
    home: Vec3,
    ship_world_pos: glam::DVec3,
    station_world_pos: glam::DVec3,
) -> Vec3 {
    let off = station_world_pos - ship_world_pos;
    home + Vec3::new(off.x as f32, off.y as f32, off.z as f32)
}

/// The camera's position in the home's own coordinates: its render position
/// minus `station_off`. True aboard and off the station alike, because the
/// hull is never rotated in render space: riding rotates the WORLD into the
/// hull frame instead (`station::hull_frame_rot`), so the home is always
/// drawn at home-local + `station_off`.
pub(crate) fn camera_home_position(camera_pos: Vec3, station_off: Vec3) -> Vec3 {
    camera_pos - station_off
}

/// Report a pending station park once the clock it asked for has moved the
/// station (see `StationPark` and `station_park_ready`). Called by lib.rs once
/// a frame, right after the station block. Reads the camera, never moves it:
/// if the park did not hold, the done file must say so, not hide it.
pub(crate) fn advance_station_park(state: &mut EngineState) {
    let Some(park) = state.station_park.as_mut() else {
        return;
    };
    park.frames += 1;
    let clock_pending = state
        .data_store
        .get::<std::sync::Mutex<Option<f32>>>("time_set_hour_request")
        .and_then(|m| m.lock().ok().map(|r| r.is_some()))
        .unwrap_or(false);
    let Some(clock_settled) = station_park_ready(park.frames, clock_pending) else {
        return;
    };
    let Some(park) = state.station_park.take() else {
        return;
    };
    let home = camera_home_position(state.camera.position, state.station_off);
    let miss_m = (home - park.requested).length();
    let mut done = park.done;
    done["position"] = serde_json::json!([home.x, home.y, home.z]);
    done["requested"] = serde_json::json!([park.requested.x, park.requested.y, park.requested.z]);
    done["yaw_pitch"] = serde_json::json!([state.camera.yaw, state.camera.pitch]);
    done["requested_yaw_pitch"] =
        serde_json::json!([park.requested_yaw_pitch.0, park.requested_yaw_pitch.1]);
    done["error_m"] = serde_json::json!(miss_m);
    done["station_ride"] = serde_json::json!(state.station_ride);
    done["clock_settled"] = serde_json::json!(clock_settled);
    done["frames"] = serde_json::json!(park.frames);
    if miss_m > 0.05 || !state.station_ride {
        log::warn!(
            "Camera request: station park MISSED: camera at home-frame {home:?}, asked for {:?} ({miss_m:.2} m off), riding {}",
            park.requested,
            state.station_ride
        );
    } else {
        log::info!(
            "Camera request: station park holds at home-frame {home:?} after {} frame(s) (clock settled: {clock_settled})",
            park.frames
        );
    }
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(CAMERA_DONE_PATH, done.to_string());
}

pub(crate) fn poll_camera_request(state: &mut EngineState) {
    const REQUEST_PATH: &str = "debug/camera_request.json";
    const DONE_PATH: &str = CAMERA_DONE_PATH;
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let parsed = std::fs::read_to_string(REQUEST_PATH)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    // Consumed either way (same rule as the other debug polls).
    let _ = std::fs::remove_file(REQUEST_PATH);
    // A station park still waiting to report belongs to the request this one
    // replaces. Drop it: reporting it later would overwrite THIS request's
    // camera_done with a reading of a park nobody is waiting for.
    if let Some(old) = state.station_park.take() {
        log::warn!(
            "Camera request: a new request replaced a station park still waiting to report ({} frame(s) in)",
            old.frames
        );
    }
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
        // The home's offset in the frame the camera now rides, recomputed
        // HERE rather than left at last frame's value (BUG-132). Last frame's
        // station_off belonged to the frame the camera was leaving (on Earth,
        // after probe-sweep's warm-up), so anything this frame that still
        // reads it (lights, labels, room GI after this poll) would place the
        // home in the wrong spot. Riding, it is zero: ship == station.
        let off = state.station_world_pos - state.ship_world_pos;
        state.station_off = Vec3::new(off.x as f32, off.y as f32, off.z as f32);
        if state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson {
            state
                .camera
                .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        }
        // A deliberately FIXED pose: over the deck by default (the whole
        // point is that several captures differ only by the clock, so the
        // camera must not vary by so much as a pixel between them), or in
        // front of the named screen. Both are HOME-FRAME positions (screen
        // quads live in the home frame); `station_park_render_pos` below turns
        // the chosen one into render space with the offset of the frame the
        // camera now rides.
        let hull_top = state.homestead_bounds.map(|(_, mx)| mx.y).unwrap_or(20.0);
        let (home_pos, look) = match screen_pose {
            Some((p, look)) => (p, glam::DVec3::new(look.x as f64, look.y as f64, look.z as f64)),
            None => (Vec3::new(0.0, hull_top + 14.0, 34.0), glam::DVec3::new(0.0, -0.34, -1.0).normalize()),
        };
        // Optional "pose" (2026-09-27, docs/design/sun-cascades.md increment
        // 0): `{"station":"home","pose":"x,y,z,yaw,pitch","time":20.15}` parks
        // at a HOME-FRAME pose instead, the same five numbers
        // scripts/home-vantages.json and the showcase `cam` verb use (metres in
        // the home zone's coordinates, y at eye height; yaw 0 looks north).
        // This is what lets a probe-sweep vantage hold a room pose at a pinned
        // clock: photograph-home.js cannot pin the clock, because its `cam`
        // pose is not re-glued to the station when the hour moves the orbit.
        let pose = match v.get("pose").and_then(|s| s.as_str()).map(crate::engine::ipc_parse::parse_pose5) {
            None => None,
            Some(Some(p)) => Some(p),
            Some(None) => {
                fail("pose must be \"x,y,z,yaw,pitch\" (five numbers)".to_string());
                return;
            }
        };
        let (home_pos, yaw_pitch) = match pose {
            Some(([x, y, z], yaw, pitch)) => (Vec3::new(x, y, z), Some((yaw, pitch))),
            None => (home_pos, None),
        };
        // Into render space with the offset of the station frame the camera
        // now rides (zero), never last frame's station_off (BUG-132).
        //
        // The clock jump this request may also ask for is safe from here on:
        // the station block in lib.rs rides by ABSOLUTE pose
        // (ship_world_pos = the station's new position, every frame), and its
        // departure test measures the camera against where the station was
        // when the frame last synced to it, which is exactly where this park
        // put it. So however far the requested hour carries the orbit, the
        // camera keeps its home-frame position; `advance_station_park` reads
        // it back after that jump and reports what it finds.
        let position = station_park_render_pos(home_pos, state.ship_world_pos, state.station_world_pos);
        state.camera.position = position;
        state.camera.clear_surface();
        let (yaw, pitch) = yaw_pitch.unwrap_or_else(|| crate::dev_travel::look_angles(look));
        state.camera.yaw = yaw;
        state.camera.pitch = pitch;
        if yaw_pitch.is_some() {
            // Teleport the player body too, as the showcase `cam` verb does:
            // in first person the camera derives from it.
            for (_e, (t, _c)) in state
                .game_world
                .world
                .query_mut::<(&mut crate::ecs::components::Transform, &crate::ecs::components::Controllable)>()
            {
                t.position = position;
            }
        }
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
        // camera_done is NOT written here. `position`, `yaw_pitch`,
        // `requested`, `station_ride` and `error_m` are filled in by
        // `advance_station_park` from a reading of the camera taken after
        // the station block has ridden it through the requested clock change
        // (normally two frames from now), so the done file is evidence of
        // where the camera went rather than a copy of where it was sent.
        let mut done = serde_json::json!({"ok": true, "station": "home"});
        if let Some(id) = v.get("screen").and_then(|s| s.as_str()) {
            done["screen"] = serde_json::json!(id);
            done["look"] = serde_json::json!([look.x, look.y, look.z]);
            log::info!("Camera request: parked aboard the home station facing screen {id:?} at home-frame {home_pos:?}");
        } else if let Some((yaw, pitch)) = yaw_pitch {
            log::info!("Camera request: parked aboard the home station at home-frame pose {home_pos:?} yaw {yaw} pitch {pitch}");
        } else {
            log::info!("Camera request: parked aboard the home station at home-frame {home_pos:?}");
        }
        state.station_park = Some(StationPark {
            requested: home_pos,
            requested_yaw_pitch: (yaw, pitch),
            done,
            frames: 0,
        });
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
            // Dev teleport: re-place the weather system here. The anchor is sticky by
            // design so a storm stays put as you fly through it (BUG-080), but a teleport
            // is not flying, and leaving the storm at the old spawn point is how a storm
            // fixture ends up measuring clear sky under a Storm HUD.
            state.weather_anchor = None;
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
        // Dev teleport: re-place the weather system here. The anchor is sticky by
        // design so a storm stays put as you fly through it (BUG-080), but a teleport
        // is not flying, and leaving the storm at the old spawn point is how a storm
        // fixture ends up measuring clear sky under a Storm HUD.
        state.weather_anchor = None;
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
/// `"enter": false` stops before step 4: the game stays on the main menu, connected, the
/// way a returning player's game sits there after its auto-connect identified it. A rig
/// then waits for the handshake (the recorder probe's `ws_identified`) and presses the
/// menu's own Enter World button (the ui request), so the join gate runs on the frame the
/// page changes, BEFORE the world has loaded: the path a real player takes, which the
/// all-in-one autopilot never ran (ship homes 1b, round 4 review: the first join of that
/// path named no ship and was refused). verify-copresence.js --plots, game-first.
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
    // A rig, not a new player: no first-entry controls hint over its captures
    // (gui/first_steps.rs, first-hour audit 2026-10-04).
    state.gui_state.controls_hint_shown = true;
    state.gui_state.showroom_active = false;
    state.gui_state.construction_active = false;
    let enter = req.get("enter").and_then(|v| v.as_bool()).unwrap_or(true);
    state.gui_state.active_page = if enter { GuiPage::None } else { GuiPage::MainMenu };
    let key_prefix: String =
        state.gui_state.profile_public_key.chars().take(12).collect();
    log::info!(
        "Autopilot: {} as '{}' (chat name '{}') on {} (identity {}...)",
        if enter { "entering the world" } else { "connecting from the main menu" },
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
            "entered": enter,
        })
        .to_string(),
    );
}

// ── Remote-player recorder (2026-10-03, permanent dev tooling) ───────────────
//
// WHY: other players are drawn by net::sync's snapshot interpolation, and its
// unit tests feed it made-up deliveries. Nothing measured what a REAL game
// draws when a real second player walks past, so a figure that stop-goes,
// slides backwards or never appears could pass every test. This records the
// drawn result from inside the running game, so a rig can judge it
// (scripts/verify-copresence.js, `just verify-copresence`).
//
// Drop `debug/remote_players_request.json` = `{"seconds": N}` while the game
// runs. From that frame on, for N seconds of the frame clock, every frame
// records each remote player's DRAWN position: the Transform net::sync wrote
// on that frame, which is exactly what the render pass builds the figure from
// (lib.rs, "Remote players"). Then `debug/remote_players_done.json` holds all
// of it. `{"seconds": 0}` answers on the same frame with one frame: a cheap
// "are we in the shared world yet" probe.
//
// THE FRAME CLOCK: each frame's `t` is the sum of the real frame steps
// (`clock_dt`, uncapped) since the recording began. That is the same step
// net::sync's own clock advances by (`set_clock_step(clock_dt)`), so
// "distance drawn between two frames / their t difference" is the speed the
// figure was really drawn at, whatever the frame rate did. A rig must use
// these times, never an assumed 1/60.
//
// The request is consumed either way (the house rule for every debug poll);
// a new request while one runs restarts the recording.

/// Longest recording the request accepts, seconds of the frame clock.
const REMOTE_RECORD_MAX_S: f64 = 120.0;
/// Most frames one recording keeps (a bound for a pathological frame rate,
/// not a limit anyone should meet: 120 s at 30 fps is 3,600).
const REMOTE_RECORD_MAX_FRAMES: usize = 20_000;
/// Most player rows one recording keeps across all its frames, so a busy
/// world cannot grow it without bound (each frame holds every remote
/// player): 3,600 frames of 50 players.
const REMOTE_RECORD_MAX_ROWS: usize = 180_000;

/// One recording in flight.
struct RemoteRecording {
    /// Seconds of frame clock asked for.
    seconds: f64,
    /// Frame clock since the recording began, seconds (sum of `clock_dt`).
    clock: f64,
    /// Wall clock at the start, for a cross-check against the frame clock.
    started: std::time::Instant,
    frames: Vec<serde_json::Value>,
    /// Player rows kept so far, across all frames.
    rows: usize,
    camera_start: serde_json::Value,
    truncated: bool,
}

/// The recording in flight, if any. A static rather than an EngineState field
/// so this dev tool lives in this one file.
static REMOTE_RECORDING: std::sync::Mutex<Option<RemoteRecording>> = std::sync::Mutex::new(None);

/// Read the `seconds` out of a request body: a number from 0 to
/// `REMOTE_RECORD_MAX_S`, or an error saying what was wrong.
pub(crate) fn parse_remote_players_request(text: &str) -> Result<f64, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    let s = v
        .get("seconds")
        .and_then(|s| s.as_f64())
        .ok_or_else(|| "missing \"seconds\" (a number of seconds to record)".to_string())?;
    if !s.is_finite() || s < 0.0 {
        return Err(format!("\"seconds\" must be 0 or more, got {s}"));
    }
    Ok(s.min(REMOTE_RECORD_MAX_S))
}

/// Every remote player as drawn right now: id, name, the DRAWN position (the
/// Transform, not the update it is walking toward), and what its jitter
/// buffer was doing (Waiting / Interpolating / Extrapolating / Holding).
pub(crate) fn remote_players_json(world: &hecs::World) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = world
        .query::<(
            &crate::ecs::components::Transform,
            &crate::net::sync::RemotePlayer,
            Option<&crate::net::sync::SnapshotBuffer>,
        )>()
        .iter()
        .map(|(_e, (t, r, buf))| {
            serde_json::json!({
                "id": r.player_id,
                "name": r.name,
                "pos": [t.position.x, t.position.y, t.position.z],
                "phase": buf.map(|b| format!("{:?}", b.phase)),
            })
        })
        .collect();
    // Stable order, so two frames list the same player in the same place.
    out.sort_by_key(|p| p.get("id").and_then(|i| i.as_u64()).unwrap_or(0));
    out
}

/// Every crew member as drawn right now (ship homes increment 3): id, name, the DRAWN position
/// (the Transform net::sync wrote this frame, which the render pass builds the amber figure
/// from, lib.rs "Crew"), their chore label and whether they are at work. The --plots rig judges
/// that no crew figure is ever drawn on a plot and that the crew are seen in the Commons
/// (scripts/lib/copresence-judge.js `judgeCrew`).
pub(crate) fn remote_crew_json(world: &hecs::World) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = world
        .query::<(&crate::ecs::components::Transform, &crate::net::sync::RemoteNpc)>()
        .iter()
        .map(|(_e, (t, n))| {
            serde_json::json!({
                "id": n.entity_id,
                "name": n.name,
                "pos": [t.position.x, t.position.y, t.position.z],
                "activity": n.activity,
                "working": n.working,
            })
        })
        .collect();
    out.sort_by_key(|c| c.get("id").and_then(|i| i.as_u64()).unwrap_or(0));
    out
}

fn camera_json(state: &EngineState) -> serde_json::Value {
    let c = &state.camera;
    serde_json::json!({
        "pos": [c.position.x, c.position.y, c.position.z],
        "yaw": c.yaw,
        "pitch": c.pitch,
    })
}

/// Called once a frame (lib.rs, beside the other dev polls, AFTER the
/// co-presence block has ticked net::sync for this frame). `clock_dt` is the
/// frame's real, uncapped step.
pub(crate) fn poll_remote_players_request(state: &mut EngineState, clock_dt: f32) {
    const REQUEST_PATH: &str = "debug/remote_players_request.json";
    const DONE_PATH: &str = "debug/remote_players_done.json";
    let Ok(mut slot) = REMOTE_RECORDING.lock() else { return };
    if std::path::Path::new(REQUEST_PATH).exists() {
        let text = std::fs::read_to_string(REQUEST_PATH).unwrap_or_default();
        let _ = std::fs::remove_file(REQUEST_PATH);
        match parse_remote_players_request(&text) {
            Ok(seconds) => {
                if slot.is_some() {
                    log::info!("Remote-player recorder: a new request replaced the recording in flight");
                }
                log::info!("Remote-player recorder: recording {seconds:.1} s of drawn remote players");
                *slot = Some(RemoteRecording {
                    seconds,
                    clock: 0.0,
                    started: std::time::Instant::now(),
                    frames: Vec::new(),
                    rows: 0,
                    camera_start: camera_json(state),
                    truncated: false,
                });
            }
            Err(e) => {
                log::warn!("Remote-player recorder: {e}");
                write_done_atomically(DONE_PATH, &serde_json::json!({"ok": false, "error": e}));
                *slot = None;
                return;
            }
        }
    } else if let Some(rec) = slot.as_mut() {
        // Not the first frame: the frame clock moves on by this frame's step.
        rec.clock += clock_dt.max(0.0) as f64;
    }
    let Some(rec) = slot.as_mut() else { return };
    if rec.frames.len() < REMOTE_RECORD_MAX_FRAMES && rec.rows < REMOTE_RECORD_MAX_ROWS {
        let c = &state.camera;
        let players = remote_players_json(&state.game_world.world);
        let crew = remote_crew_json(&state.game_world.world);
        // The pieces the server keeps, with their boxes on the screen (ship homes increment 5).
        let shared = crate::engine::shared_build::recorder_rows(state);
        rec.rows += players.len() + crew.len() + shared.len();
        // The computer's own clock, ms since 1970: a rig compares it with
        // when its walker did things, independent of the frame clock above.
        let epoch_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        rec.frames.push(serde_json::json!({
            "t": rec.clock,
            "dt": clock_dt,
            "wall": rec.started.elapsed().as_secs_f64(),
            "epoch_ms": epoch_ms,
            "joined": state.game_joined,
            "cam": [c.position.x, c.position.y, c.position.z, c.yaw, c.pitch],
            "players": players,
            // The crew as drawn this frame (increment 3).
            "crew": crew,
            // Every piece the server keeps in this world this frame, its screen rect in the
            // window's pixels or null behind the camera (scripts/lib/shared-build-judge.js).
            "shared": shared,
        }));
    } else {
        rec.truncated = true;
    }
    if rec.clock < rec.seconds {
        return;
    }
    let Some(rec) = slot.take() else { return };
    // Increment 1b: the plot the home stands on (null for none) and every plot of the ship,
    // so a rig can check the camera and the other players against the plots
    // (scripts/verify-copresence.js --plots).
    let (home_plot, ship_plots) = crate::engine::home_plot::probe_json(state.gui_state.ship_structure.as_ref());
    let (screen_w, screen_h) = state.renderer.viewport_size();
    let done = serde_json::json!({
        "ok": true,
        "seconds": rec.seconds,
        "recorded_s": rec.clock,
        "wall_s": rec.started.elapsed().as_secs_f64(),
        "frame_count": rec.frames.len(),
        "truncated": rec.truncated,
        "world_loaded": state.world_loaded,
        "ws_identified": state.gui_state.ws_identified,
        "game_joined": state.game_joined,
        "copresence_active": state.gui_state.copresence_active,
        "home_plot": home_plot,
        "ship_plots": ship_plots,
        // Where the home's own things stand (Respawn point, hologram, showroom stage, animals,
        // plants): the --plots judge checks each is on the game's plot (home_plot.rs).
        "home_things": crate::engine::home_plot::home_things_json(state),
        "welcomed": state.game_welcomed,
        // Increment 2: the plot the world load built the home on (the remembered one, else the
        // default), what the last welcome did with the home ("stay", "move", "guest",
        // "refused"), and whether the home is put away (a guest) (engine/home_plot.rs).
        "boot_plot": state.boot_plot,
        "last_welcome": state.last_welcome,
        // The last welcome's `rejoin` (the relay found us still in the world: a reconnect inside
        // its grace), and the notices on screen now (each toast's text): verify-copresence's
        // guest leg judges its dropped connection and the build editor's refusal by them.
        "last_welcome_rejoin": state.last_welcome_rejoin,
        "notices": state.gui_state.toasts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
        "home_away": state.gui_state.ship_structure.as_ref().is_some_and(|s| s.home_is_away()),
        "copresence_refused": state.copresence_refused.is_some(),
        // The sentence the HUD shows while it holds (home_plot.rs `refuse_shared_world`), so a
        // rig can name the refusal it hit.
        "copresence_refused_note": state.gui_state.copresence_refused_note,
        // The construction editor's camera is up (the showcase `build_editor` verb opens it;
        // verify-copresence --plots waits for it to open and shut).
        "build_editor": state.construction_cam_active,
        // Increment 4: the corrections the relay sent and this game applied, and the rig's walk in
        // progress (engine/move_check.rs); whether the camera stands inside the ship's bounds.
        "moves": crate::engine::move_check::probe_json(state),
        // Every transit link the ship has (its teleporter pairs, src/ship/transit.rs): the rig
        // walks onto the home's own west pad and judges where it lands (the increment 4 review, R1).
        "transit": crate::engine::move_check::transit_probe_json(state),
        "aboard_ship": state.aboard_bounds.is_some_and(|b| crate::ship::ship_space::in_box(&b, state.camera.position)),
        // Increment 5: the pieces the server keeps as this game has them, what this player may
        // build, what waits for the relay (engine/shared_build.rs `probe_json`), and the window's
        // size in pixels, the space of every frame's `shared` rects and of a screenshot.
        "shared_build": crate::engine::shared_build::probe_json(state),
        "screen_px": [screen_w, screen_h],
        "camera_start": rec.camera_start,
        "camera_end": camera_json(state),
        "frames": rec.frames,
    });
    write_done_atomically(DONE_PATH, &done);
    log::info!(
        "Remote-player recorder: wrote {} frames ({:.2} s) to {DONE_PATH}",
        rec.frames.len(),
        rec.clock
    );
}

// ── Door points (ship homes increment 2, "Meet in the Commons") ───────────────
//
// Drop `debug/door_points_request.json` (any content) while the game runs. The same frame
// writes `debug/door_points_done.json`: `{"ok": true, "ship_hash", "places": [...], "doors":
// [...]}` (src/ship/door_points.rs), the ship's places (each shared zone, each plot with its
// door) and every corridor's door points, from the Rust corridor geometry. A rig
// (scripts/verify-copresence.js, scripts/lib/copresence-judge.js `doorRoute`) walks the game and
// its scripted player from a home's door into the Commons through them, so it never
// re-implements the corridor maths. `{"ok": false, "error"}` while no ship has assembled (the
// world has not loaded, or the legacy layout is showing). Permanent dev tooling.

/// What the door-points request answers (see above). Pure.
pub(crate) fn door_points_json(ship: Option<&crate::ship::ship_structure::ShipStructure>) -> serde_json::Value {
    let Some(ship) = ship else {
        return serde_json::json!({"ok": false, "error": "no ship has assembled (the world has not loaded, or the legacy layout is showing)"});
    };
    let mut v = serde_json::to_value(crate::ship::door_points::door_points(ship)).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(o) = v.as_object_mut() {
        o.insert("ok".into(), serde_json::json!(true));
    }
    v
}

/// Called once a frame (lib.rs, beside the other dev polls).
pub(crate) fn poll_door_points_request(state: &EngineState) {
    const REQUEST_PATH: &str = "debug/door_points_request.json";
    const DONE_PATH: &str = "debug/door_points_done.json";
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let _ = std::fs::remove_file(REQUEST_PATH);
    write_done_atomically(DONE_PATH, &door_points_json(state.gui_state.ship_structure.as_ref()));
    log::info!("Door points: wrote {DONE_PATH}");
}

/// Write a done file whole: to a temporary name, then renamed over the real
/// one, so a reader polling for it never parses half a file.
fn write_done_atomically(path: &str, body: &serde_json::Value) {
    let _ = std::fs::create_dir_all("debug");
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, body.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

#[cfg(test)]
mod station_park_tests {
    //! BUG-132: a `{"station":"home","pose":...}` park must land on its pose
    //! in the home's own coordinates, whatever frame the camera was in before.
    use super::*;
    use glam::DVec3;

    /// Where the home station was on the BUG-132 run (the position run.log
    /// printed, near enough), and a ship frame on Earth's surface where
    /// probe-sweep's warm-up had left the camera (lat 23, lon 13, 50 m).
    fn station() -> DVec3 {
        DVec3::new(3.26e7, -2.49e6, -2.69e7)
    }
    fn earth_ship() -> DVec3 {
        let r = 6.371e6 + 50.0;
        let (lat, lon) = (23.0f64.to_radians(), 13.0f64.to_radians());
        DVec3::new(r * lat.cos() * lon.cos(), r * lat.sin(), r * lat.cos() * lon.sin())
    }
    const POSE: Vec3 = Vec3::new(27.5, 26.0, 96.0); // home-overview-noon

    #[test]
    fn a_park_rides_the_station_so_its_pose_is_the_render_position() {
        // The verb has just set ship_world_pos = station_world_pos.
        let render = station_park_render_pos(POSE, station(), station());
        assert_eq!(render, POSE);
        // And reading it back in the home frame (station_off is zero while
        // riding) gives the pose exactly, not approximately.
        assert_eq!(camera_home_position(render, Vec3::ZERO), POSE);
    }

    #[test]
    fn last_frames_offset_is_the_bug() {
        // The old verb: the pose plus the station_off of the frame the camera
        // was LEAVING (the ship frame on Earth), placed into the frame it now
        // rides (station_off zero). That is the whole Earth-to-home distance
        // added to the pose: the camera ends up in empty space.
        let stale = station() - earth_ship();
        let old = POSE + Vec3::new(stale.x as f32, stale.y as f32, stale.z as f32);
        let miss = (camera_home_position(old, Vec3::ZERO) - POSE).length();
        assert!(miss > 1.0e7, "the old placement misses by {miss} m");
        // The new placement does not depend on where the camera came from.
        let new = station_park_render_pos(POSE, station(), station());
        assert_eq!(camera_home_position(new, Vec3::ZERO), POSE);
    }

    #[test]
    fn the_home_frame_reading_undoes_station_off_off_the_station_too() {
        // Off the station the home renders at home-local + station_off, so a
        // camera 2 m east of a home 1 km away reads 2 m east in the home frame.
        let off = Vec3::new(1000.0, -40.0, 250.0);
        assert_eq!(camera_home_position(off + Vec3::new(2.0, 0.0, 0.0), off), Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn a_park_reports_only_after_the_clock_has_moved() {
        // First station-block pass: the hour request is still waiting for
        // the system tick that comes after the block, so the pass rode the
        // OLD clock. Keep waiting.
        assert_eq!(station_park_ready(1, true), None);
        // Second pass: the tick took the hour, and this pass carried the
        // station to where that hour puts it. Report.
        assert_eq!(station_park_ready(2, false), Some(true));
        // A park that asked for no hour reports after one pass.
        assert_eq!(station_park_ready(1, false), Some(true));
        // A clock that never ticks cannot hold the report forever.
        assert_eq!(station_park_ready(STATION_PARK_MAX_FRAMES - 1, true), None);
        assert_eq!(station_park_ready(STATION_PARK_MAX_FRAMES, true), Some(false));
    }
}

#[cfg(test)]
mod remote_player_recorder_tests {
    use super::*;
    use crate::ecs::components::Transform;
    use crate::net::sync::{RemotePlayer, SnapshotBuffer};
    use glam::Quat;

    #[test]
    fn the_request_reads_seconds_and_refuses_junk() {
        assert_eq!(parse_remote_players_request(r#"{"seconds": 12.5}"#), Ok(12.5));
        assert_eq!(parse_remote_players_request(r#"{"seconds": 0}"#), Ok(0.0));
        // Capped, not refused: a long ask still records something useful.
        assert_eq!(parse_remote_players_request(r#"{"seconds": 9999}"#), Ok(REMOTE_RECORD_MAX_S));
        assert!(parse_remote_players_request(r#"{"seconds": -1}"#).is_err());
        assert!(parse_remote_players_request(r#"{"secs": 3}"#).is_err());
        assert!(parse_remote_players_request("not json").is_err());
    }

    /// The recorder reports where the figure is DRAWN (its Transform), not
    /// the update it is walking toward (`target_position`). A recorder that
    /// read the target would show every update as a clean step and could
    /// never see the figure stop and go between them.
    #[test]
    fn it_reports_the_drawn_position_not_the_target() {
        let mut world = hecs::World::new();
        let drawn = Vec3::new(1.0, 1.7, -2.0);
        let target = Vec3::new(9.0, 1.7, -2.0);
        world.spawn((
            Transform { position: drawn, rotation: Quat::IDENTITY, scale: Vec3::ONE },
            RemotePlayer {
                player_id: 7,
                name: "TestBotWalker".to_string(),
                look: None,
                last_position: drawn,
                target_position: target,
                last_rotation: Quat::IDENTITY,
                target_rotation: Quat::IDENTITY,
                velocity: Vec3::ZERO,
                interpolation_t: 0.5,
                last_update_time: 0.0,
            },
            SnapshotBuffer::new(target, Quat::IDENTITY, Vec3::ZERO, 0.0, 1.0),
        ));
        let players = remote_players_json(&world);
        assert_eq!(players.len(), 1);
        let p = &players[0];
        assert_eq!(p["id"], 7);
        assert_eq!(p["name"], "TestBotWalker");
        let pos: Vec<f64> = p["pos"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let want = [drawn.x as f64, drawn.y as f64, drawn.z as f64];
        assert!(
            pos.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-6),
            "drawn position {pos:?}, want {want:?} (the target is {target:?})"
        );
        assert_eq!(p["phase"], "Waiting");
    }

    /// The crew too (increment 3): each crew member where it is DRAWN (its Transform), with its
    /// chore label, and not where it walks to. The --plots rig judges every crew figure the
    /// game drew against the plots, so a recorder that read the target would miss a figure
    /// drawn on a plot while it walked there.
    ///
    /// Seen red 2026-10-04 with `remote_crew_json` reading `target_position`: "drawn position
    /// [70.0, 1.0, 24.0], want [66.0, 1.0, 24.0] (the target is Vec3(70.0, 1.0, 24.0))".
    #[test]
    fn it_reports_each_crew_member_where_drawn() {
        let mut world = hecs::World::new();
        let drawn = Vec3::new(66.0, 1.0, 24.0);
        let target = Vec3::new(70.0, 1.0, 24.0);
        world.spawn((
            Transform { position: drawn, rotation: Quat::IDENTITY, scale: Vec3::ONE },
            crate::net::sync::RemoteNpc {
                entity_id: 12,
                name: "CB-7".to_string(),
                activity: "Wiping down the tables".to_string(),
                working: false,
                role: String::new(),
                dialog: Vec::new(),
                greetings: Vec::new(),
                last_position: drawn,
                target_position: target,
                facing: crate::turning::Turn::default(),
                heading: 0.0,
                interpolation_t: 0.5,
            },
        ));
        let crew = remote_crew_json(&world);
        assert_eq!(crew.len(), 1);
        let c = &crew[0];
        assert_eq!(c["id"], 12);
        assert_eq!(c["name"], "CB-7");
        assert_eq!(c["activity"], "Wiping down the tables");
        let pos: Vec<f64> = c["pos"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let want = [drawn.x as f64, drawn.y as f64, drawn.z as f64];
        assert!(
            pos.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-6),
            "drawn position {pos:?}, want {want:?} (the target is {target:?})"
        );
        // A world with no crew records an empty list, not a missing one.
        assert_eq!(remote_crew_json(&hecs::World::new()), Vec::<serde_json::Value>::new());
    }
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

    /// The clip maker's lens (2026-10-04). A number is the lens, held to
    /// 10..120; `auto` is the Settings fov under lib.rs's own 60..120 clamp,
    /// so a corrupt `"fov": 0` in a config cannot black the frame out
    /// through this door either. The whole chain, from the dropped text.
    ///
    /// Red checks, run 2026-10-04: with the lens computed from the Settings
    /// value whatever the pin said, "a 40 degree lens is 40 degrees" failed
    /// (left 90, right 40); with `effective_fov` ignoring the pin (what
    /// lib.rs's Settings apply did), it failed again (left 90, right 40).
    /// That second check covers the rule, not the call: lib.rs's Settings
    /// apply could still skip `effective_fov` (the take that came out at 90
    /// degrees when a config save landed one frame after the pin) with this
    /// test green. The call site is held by tests/engine_wiring_lint.rs
    /// (sky_and_lens_call_sites_use_their_shared_rules), red-checked against
    /// exactly that revert.
    #[test]
    fn the_fov_pin_sets_the_lens_and_auto_gives_it_back() {
        let lens = |body: &str, settings: f32| {
            effective_fov(fov_pin_from(parse_showcase_pin(&showcase_value(body, "fov").unwrap()).unwrap()), settings)
        };
        assert_eq!(lens(r#"{"fov":"40"}"#, 90.0), 40.0, "a 40 degree lens is 40 degrees");
        assert_eq!(lens(r#"{"fov":"2"}"#, 90.0), 10.0, "held at 10 at the long end");
        assert_eq!(lens(r#"{"fov":"300"}"#, 90.0), 120.0, "held at 120 at the wide end");
        assert_eq!(lens(r#"{"fov":"auto"}"#, 75.0), 75.0, "auto is the Settings fov");
        assert_eq!(lens(r#"{"fov":"auto"}"#, 0.0), 60.0, "auto keeps lib.rs's clamp on a corrupt Settings fov");
        // The Settings apply (lib.rs, settings_dirty) asks the same function:
        // a pinned lens survives it, whatever the Settings fov is.
        assert_eq!(effective_fov(Some(45.0), 90.0), 45.0, "a Settings apply keeps the pinned lens");
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

    /// The hull pin (2026-10-05, the ship-first-street vantage): the body the vantage writes
    /// hides the hull, the release every other vantage is sent (scripts/lib/showcase-pins.js
    /// STICKY_PIN_RESETS, "1") shows it again, and a typo leaves it as it is. Seen red
    /// 2026-10-05 with `hull_pin` answering None for everything (the handler then never
    /// touches the hull): "the vantage's {\"hull\":\"0\"} hides the hull: None".
    #[test]
    fn the_hull_pin_hides_and_shows_the_hull_and_a_typo_changes_nothing() {
        let pin = |body: &str| showcase_value(body, "hull").and_then(|t| hull_pin(&t));
        assert_eq!(pin(r#"{"hull":"0","time":"20.15"}"#), Some(false), "the vantage's {{\"hull\":\"0\"}} hides the hull: {:?}", pin(r#"{"hull":"0"}"#));
        assert_eq!(pin(r#"{"hull":"1"}"#), Some(true), "the release shows it again");
        for typo in ["off", "", "2", "auto"] {
            assert_eq!(hull_pin(typo), None, "{typo:?} leaves the hull as it is");
        }
        assert_eq!(pin(r#"{"time":"20.15"}"#), None, "a vantage that names no hull leaves it alone");
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

#[cfg(test)]
mod daylight_gate_tests {
    use super::daylight_over_anchor;
    use glam::{DQuat, DVec3};

    const R: f64 = 6_371_000.0;
    /// The sun, far off along the world -X axis, as seen from near the origin.
    fn to_sun() -> DVec3 {
        DVec3::new(-1.5e11, 0.0, 0.0)
    }

    /// THE STARLESS NIGHT AT SILVERDALE (BUG-138, 2026-10-04, the landing hero shot).
    /// The frame lock keeps its anchor in the body's UNROTATED frame and puts
    /// the camera at `Ry(spin) * anchor` in the world, so the ground's real
    /// "up" is the anchor turned by the spin. The gate compared the unturned
    /// anchor with the world sun, so it judged the sun over the wrong
    /// longitude, by an angle that is the planet's whole turn: whether it
    /// called it day did not depend on the hour at all. At Silverdale it read
    /// "day" at 04:00 and skipped the star pass, and the night sky showed a
    /// dozen points and no Milky Way.
    ///
    /// Red check, run 2026-10-04 with the spin left out of `up` (the old
    /// gate): "a point turned to face the sun is in daylight, whatever its
    /// unturned direction says" failed. That `sky_daylight` and `star_fades`
    /// pass the live spin into this arithmetic is checked by
    /// tests/engine_wiring_lint.rs (sky_and_lens_call_sites_use_their_shared_rules).
    #[test]
    fn the_gate_reads_the_sun_over_the_turned_ground() {
        // On the equator, facing world +X before the turn.
        let anchor = DVec3::new(R + 2.0, 0.0, 0.0);
        // Half a turn: the ground now faces world -X, toward the sun.
        assert!(
            daylight_over_anchor(anchor, std::f64::consts::PI, R, to_sun()),
            "a point turned to face the sun is in daylight, whatever its unturned direction says"
        );
        // No turn: it faces away, which is night.
        assert!(!daylight_over_anchor(anchor, 0.0, R, to_sun()), "a point facing away from the sun is at night");
        // A quarter turn the other way round: the turn the frame lock applies
        // (`DQuat::from_rotation_y(spin)`, dev_travel::frame_lock_ship_pos),
        // not its inverse. Ry(+90 deg) takes +X to -Z, so a sun along -Z is up.
        let quarter = std::f64::consts::FRAC_PI_2;
        let turned = DQuat::from_rotation_y(quarter) * DVec3::X;
        assert!((turned - DVec3::new(0.0, 0.0, -1.0)).length() < 1e-12, "Ry(+90 deg) * X = {turned:?}");
        assert!(daylight_over_anchor(anchor, quarter, R, DVec3::new(0.0, 0.0, -1.5e11)));
        assert!(!daylight_over_anchor(anchor, -quarter, R, DVec3::new(0.0, 0.0, -1.5e11)));
    }

    /// Above 120 km the gate never reads day: space has no sky to hide the
    /// stars behind, whatever the sun does.
    #[test]
    fn the_gate_is_off_above_the_air() {
        let anchor = DVec3::new(R + 200_000.0, 0.0, 0.0);
        assert!(!daylight_over_anchor(anchor, std::f64::consts::PI, R, to_sun()));
    }
}

#[cfg(test)]
mod drop_link_tests {
    use super::*;

    /// The `drop_link` verb's reconnect hold is capped at DROP_LINK_MAX_HOLD_SECS (the final
    /// review of ship homes increment 2): a typo of an extra digit or two used to hold the
    /// connection down for hours, past the relay's 90 s grace, with the game showing nothing
    /// wrong. Below the backoff's first rung it takes the first rung, and a value that is not a
    /// finite number does nothing. Seen red 2026-10-04 with no cap (the verb as increment 2's
    /// review left it): "a hold past the cap is held at the cap: Some(1000000000.0)".
    #[test]
    fn a_drop_link_hold_is_capped_and_never_below_the_first_rung() {
        assert_eq!(drop_link_hold("12"), Some(12.0));
        assert_eq!(drop_link_hold(" 12 "), Some(12.0));
        assert_eq!(drop_link_hold("1e9"), Some(DROP_LINK_MAX_HOLD_SECS), "a hold past the cap is held at the cap: {:?}", drop_link_hold("1e9"));
        assert_eq!(drop_link_hold("120"), Some(120.0));
        assert_eq!(drop_link_hold("0"), Some(crate::net::ws_client::RECONNECT_DELAY_INITIAL_SECS));
        assert_eq!(drop_link_hold("-5"), Some(crate::net::ws_client::RECONNECT_DELAY_INITIAL_SECS));
        for junk in ["", "soon", "inf", "NaN"] {
            assert_eq!(drop_link_hold(junk), None, "'{junk}' is not a hold");
        }
    }
}

#[cfg(test)]
mod showcase_gate_tests {
    use super::showcase_gate;
    use crate::config::PlayMode;

    /// THE FREE SHOWCASE GARDEN IS A FREE-RESOURCES FEATURE (first-hour audit
    /// 2026-10-04, Missing stakes 2). It replanted the whole garden, ripe crops
    /// and all, every time it was empty, in every play mode: a Normal player
    /// never had to plant or wait for anything. Now it plants only while free
    /// resources are on (Creative and Dev, with the Inventory page's Creative
    /// toggle on), which keeps the probe rigs' garden vantages: the rig is a
    /// Dev-mode sandbox. A garden that already grows is never replanted.
    ///
    /// Seen red 2026-10-04 with the gate asking only whether crops grow (the
    /// code before the fix): "Normal mode does not plant the free garden / left: Ok(())".
    #[test]
    fn the_showcase_garden_is_planted_only_with_free_resources() {
        let empty = hecs::World::new();
        let normal = showcase_gate(&empty, PlayMode::Normal, false);
        assert!(normal.is_err(), "Normal mode does not plant the free garden / left: {normal:?}");
        // Normal pins the Creative toggle off, but a stale one must not plant either.
        assert!(showcase_gate(&empty, PlayMode::Normal, true).is_err());
        assert_eq!(showcase_gate(&empty, PlayMode::Dev, true), Ok(()), "the rig's Dev sandbox plants it");
        assert_eq!(showcase_gate(&empty, PlayMode::Creative, true), Ok(()));
        assert!(showcase_gate(&empty, PlayMode::Dev, false).is_err(), "Dev testing real consumption plants nothing free");

        let mut growing = hecs::World::new();
        growing.spawn((crate::ecs::components::CropInstance {
            crop_def_id: "wheat".into(),
            growth_stage: "seed".into(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: None,
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        },));
        assert_eq!(showcase_gate(&growing, PlayMode::Dev, true), Err("crops already present"));
    }
}

#[cfg(test)]
mod shared_build_ipc_tests {
    //! The dev IPC's side of the pieces the server keeps (ship homes increment 5, 2026-10-05):
    //! the showcase verbs scripts/verify-copresence.js --build sends, and what the recorder
    //! reports for scripts/lib/shared-build-judge.js.
    use super::*;
    use crate::engine::shared_build::{piece_rows, spawn_piece, ScreenView};
    use crate::ship::build_frames::BuildFrames;
    use crate::systems::construction::shared::Piece;
    use crate::systems::construction::BlueprintRegistry;
    use glam::Mat4;

    /// The body of `fn name` in this file, up to the first line that is a lone `}` (line endings
    /// as git checks the file out on Windows or Linux).
    fn body_of(name: &str) -> String {
        let src = include_str!("ipc.rs").replace("\r\n", "\n");
        let start = src.find(&format!("pub(crate) fn {name}(")).unwrap_or_else(|| panic!("fn {name} in ipc.rs"));
        src[start..start + src[start..].find("\n}\n").expect("its end")].to_string()
    }

    /// THE SHARED-BUILD VERBS PARSE, AND THE SHOWCASE POLL READS THEM. `place` takes a blueprint, a
    /// floor point in ship metres and a turn from 0 to 3, spaces allowed, and refuses anything else
    /// (a missing number, a fifth turn, no blueprint, junk, an infinity); `take_down` takes a piece
    /// id; the request bodies exactly as verify-copresence writes them reach each verb by its key;
    /// and `poll_showcase_request` reads all three.
    ///
    /// Seen red 2026-10-05 before the verbs (the three blocks not yet in `poll_showcase_request`):
    /// "the showcase poll never reads grab(\"stock\")".
    #[test]
    fn place_take_down_and_stock_verbs_parse() {
        let spec = |b: &str, x: f32, z: f32, turns: u8| Some(PlaceSpec { blueprint: b.into(), x, z, turns });
        assert_eq!(parse_place("wood_foundation@48,135,0"), spec("wood_foundation", 48.0, 135.0, 0));
        assert_eq!(parse_place(" wood_wall @ 30.5 , -2 , 3 "), spec("wood_wall", 30.5, -2.0, 3));
        for bad in ["wood_wall@1,2", "wood_wall@1,2,4", "@1,2,0", "wood_wall@x,2,0", "wood_wall@1,2,0,1", "wood_wall@inf,2,0", "wood_wall", ""] {
            assert_eq!(parse_place(bad), None, "{bad:?}");
        }
        assert_eq!(parse_take_down("12"), Some(12));
        assert_eq!(parse_take_down(" 7 "), Some(7));
        for bad in ["-1", "seven", "", "1.5"] {
            assert_eq!(parse_take_down(bad), None, "{bad:?}");
        }
        assert_eq!(showcase_value(r#"{"place":"wood_foundation@48,135,0"}"#, "place").as_deref(), Some("wood_foundation@48,135,0"));
        assert_eq!(showcase_value(r#"{"take_down":"12"}"#, "take_down").as_deref(), Some("12"));
        assert_eq!(showcase_value(r#"{"stock":"1"}"#, "stock").as_deref(), Some("1"));
        assert_eq!(showcase_value(r#"{"tower":"a","plant":"b"}"#, "place"), None, "plant is not place");
        let poll = body_of("poll_showcase_request");
        for verb in ["grab(\"stock\")", "grab(\"place\")", "grab(\"take_down\")"] {
            assert!(poll.contains(verb), "the showcase poll never reads {verb}");
        }
    }

    /// THE RECORDER REPORTS THE PIECES THE SERVER KEEPS, the rows the judge reads. In the Commons
    /// (corner (65, 0, 20)): a wall finished 6 m in front of a camera at the meeting pose looking
    /// at it, a foundation 2 s into its 5 s, and a wall behind the camera. Each row has exactly the
    /// fields the judge reads (and its screen rect, which the probe leaves out): where it is drawn
    /// from in ship metres, its turn and size, whose, finished or not, how far grown. The wall in
    /// front lies wholly inside a 1280 x 720 view and covers its middle; the one behind has no
    /// rect. And the recorder puts the rows into every frame, and the probe's `shared_build` and
    /// `screen_px` into its done JSON.
    ///
    /// Seen red 2026-10-05 before the recorder reported them (`"shared"` not yet in each recorded
    /// frame): "the recorder never writes \"shared\": shared".
    #[test]
    fn the_recorder_reports_shared_pieces() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let frames = BuildFrames::of_ship(&crate::ship::ship_structure::ShipStructure::ship_for_relay(&dir).unwrap());
        let commons = frames.get("zone:commons").unwrap();
        let t = 1_759_500_000.0;
        let at = |id: u64, bp: &str, local: [f32; 3], placed_at: f64| Piece {
            piece_id: id,
            blueprint_id: bp.into(),
            position: local,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: reg.get(bp).unwrap().size,
            placed_at,
            mine: id == 32,
        };
        let mut world = hecs::World::new();
        spawn_piece(&mut world, Some(&reg), commons, &at(31, "wood_wall", [11.0, 0.0, 50.0], t - 100.0), t);
        spawn_piece(&mut world, Some(&reg), commons, &at(32, "wood_foundation", [5.0, 0.0, 45.0], t - 2.0), t);
        spawn_piece(&mut world, Some(&reg), commons, &at(33, "wood_wall", [11.0, 0.0, 10.0], t - 100.0), t);
        // The meeting pose (76, 1.7, 64), looking along +z at the wall at z 70.
        let eye = Vec3::new(76.0, 1.7, 64.0);
        let view = ScreenView {
            view_proj: Mat4::perspective_rh(70f32.to_radians(), 1280.0 / 720.0, 0.05, 1000.0) * Mat4::look_at_rh(eye, eye + Vec3::Z, Vec3::Y),
            size: [1280.0, 720.0],
            offset: Vec3::ZERO,
        };
        let rows = piece_rows(&world, Some(&reg), Some(&view));
        assert_eq!(rows.len(), 3);
        let mut keys: Vec<&str> = rows[0].as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["blueprint_id", "built", "frame", "mine", "piece_id", "pos", "progress", "rect", "rot", "scale"]);
        let (wall, slab, behind) = (&rows[0], &rows[1], &rows[2]);
        assert_eq!((wall["piece_id"].as_u64(), wall["frame"].as_str(), wall["blueprint_id"].as_str()), (Some(31), Some("zone:commons"), Some("wood_wall")));
        assert_eq!(wall["pos"], serde_json::json!([76.0, 0.0, 70.0]), "ship metres: the Commons' corner added");
        assert_eq!(wall["rot"], serde_json::json!([0.0, 0.0, 0.0, 1.0]));
        assert_eq!((wall["built"].as_bool(), wall["mine"].as_bool(), wall["progress"].as_f64()), (Some(true), Some(false), Some(4.0)));
        let r: Vec<f64> = wall["rect"].as_array().expect("in front: a rect").iter().map(|v| v.as_f64().unwrap()).collect();
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= 1280.0 && r[3] <= 720.0 && r[0] < r[2] && r[1] < r[3], "wholly in view: {r:?}");
        assert!(r[0] < 640.0 && 640.0 < r[2] && r[1] < 360.0 && 360.0 < r[3], "over the middle of the view: {r:?}");
        assert_eq!((slab["built"].as_bool(), slab["mine"].as_bool()), (Some(false), Some(true)));
        assert!((slab["progress"].as_f64().unwrap() - 2.0).abs() < 1e-3, "2 s grown: {}", slab["progress"]);
        assert!(behind["rect"].is_null(), "behind the camera: no rect, {}", behind["rect"]);
        let probe = piece_rows(&world, Some(&reg), None);
        assert!(probe[0].get("rect").is_none(), "the probe's rows carry no rect");

        let rec = body_of("poll_remote_players_request");
        for wired in ["\"shared\": shared", "\"shared_build\": crate::engine::shared_build::probe_json(state)", "\"screen_px\": [screen_w, screen_h]"] {
            assert!(rec.contains(wired), "the recorder never writes {wired}");
        }
    }
}

//! LOW IMPACT: the game a verification rig boots while the operator is using his PC (2026-10-22).
//!
//! The operator, 2026-10-22: "I'd rather development testing continue if possible. Is there any
//! way we could minimize the impact of the game running in the background for all your testing
//! purposes? Like if you set all of the graphics to minimum/off. ... If certain parts of the app
//! aren't necessary to a test can unnecessary resources hogs be disabled? Like minimum graphics vs
//! max."
//!
//! `HUMANITY_LOW_IMPACT` (read ONCE, at the first question; a dev switch like HUMANITY_FRAME_COSTS,
//! set only by the rigs through scripts/lib/game-launch.js and never shown to players) runs the
//! game at its least cost on the graphics card and the processor WITHOUT changing what the game
//! does. Which rigs may ask for it, and when, is scripts/lib/rig-needs.js: a rig whose checks are
//! about behaviour asks for it whenever one of the operator's GPU-heavy programs runs; a rig that
//! judges the picture or times frames never does, and refuses to run in it.
//!
//! WHAT IT CHANGES, all of it drawing:
//!   1. The graphics settings of the `minimum` preset in data/gui/graphics_presets.json (marked
//!      `rig_only`, so Settings never lists it), laid OVER the config as it is read and NEVER
//!      saved: `lay_over_text` on the way in, `strip_for_save` on the way out, so a rig's sandbox
//!      config.json keeps its own settings for the next run, which may be a picture rig at full
//!      graphics. A setting someone changes during the run (the value no longer equals the
//!      preset's) is theirs, and is saved as they set it.
//!   2. The frame rate: VSync off and a cap of `fps` frames a second, focused or not (15 unless
//!      the value says `fps=N`), through the existing Settings > Graphics caps (lib.rs).
//!   3. The window: `window` pixels, not maximized (960 x 540 unless `window=keep`, which a rig
//!      that counts pixels in its screenshots asks for: its thresholds are sized to the window).
//!   4. Renderer switches that have no setting: the god-ray pass, the aurora's emission pass and
//!      the Sun's glare are off (`hold_renderer`).
//!   5. The showcase pins that would turn any of the above back on (`HELD_PINS`) are held: a rig
//!      that resets every sticky pin between vantages (scripts/lib/showcase-pins.js) cannot undo
//!      low impact by accident. Each held pin is logged.
//!
//! WHAT IT NEVER CHANGES: movement, physics, rooms, doors, elevators, falls (the walk is
//! frame-rate independent: substeps, BUG-207; a landing's speed comes from the height fallen,
//! physics::landing::landing_speed), chat, networking, saves, the terrain's shape and streaming,
//! the water's waves (`water_fft` is the float's twin, lib.rs), the trees that stand (the density
//! is kept: only how many are drawn as full 3D models goes down, and some still are, so the
//! textured-model path verify-runtime's forest vantage exists for is still exercised), the CPU
//! rain path (`gpu_particles` is kept: off moves rain to the processor), audio (a background
//! instance is silent already: launch_focus).
//!
//! THE PROOF IT RAN: `debug/low_impact.json`, written once at boot (`write_confirmation`), which
//! spawnGame waits for; a game that does not confirm is stopped rather than left running at full
//! graphics beside the operator's program (a build from before this switch would do exactly that).

use serde_json::{Map, Value};
use std::sync::{Mutex, OnceLock};

/// The environment variable. "1" (or "on", "true") asks for the defaults; a comma list of
/// `fps=N` and `window=WxH` / `window=keep` changes them. "", "0", "off" or "false" is off.
pub const ENV: &str = "HUMANITY_LOW_IMPACT";

/// The preset in data/gui/graphics_presets.json whose values low impact lays over the config.
pub const PRESET_ID: &str = "minimum";

/// The frame-rate cap when the value does not name one. Low enough that the GPU and CPU cost per
/// second is a fraction of an uncapped rig's; high enough that every rig's per-frame judgement
/// holds (copresence's steady-speed band is designed for 9 to 30 frames a second, and asks for 30
/// itself through scripts/lib/rig-needs.js).
pub const DEFAULT_FPS: u32 = 15;

/// The window when the value does not name one, in physical pixels.
pub const DEFAULT_WINDOW: (u32, u32) = (960, 540);

/// Showcase pins (engine::ipc poll_showcase_request) held while low impact is on: each one would
/// put back a cost low impact takes away.
pub const HELD_PINS: &[&str] = &[
    "room_gi",
    "bounced_light_detail",
    "sun_shadows",
    "godrays",
    "aurora",
    "sun_glare",
    "cloud_quality",
];

/// What low impact was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// The frame-rate cap, focused or not.
    pub fps: u32,
    /// The window size in physical pixels; None = keep the window as a normal launch makes it.
    pub window: Option<(u32, u32)>,
    /// The raw value, for the log and the confirmation.
    pub raw: String,
}

/// Parse the variable's value. Ok(None) = off. Err = a value no rig writes (the caller logs it
/// and runs the defaults: an unreadable ask is still an ask to be light).
pub fn parse(value: &str) -> Result<Option<Spec>, String> {
    let v = value.trim();
    if v.is_empty() || matches!(v.to_ascii_lowercase().as_str(), "0" | "off" | "false" | "no") {
        return Ok(None);
    }
    let mut spec = Spec { fps: DEFAULT_FPS, window: Some(DEFAULT_WINDOW), raw: v.to_string() };
    for tok in v.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let lower = tok.to_ascii_lowercase();
        if matches!(lower.as_str(), "1" | "on" | "true" | "yes") {
            continue;
        }
        if let Some(n) = lower.strip_prefix("fps=") {
            let n: u32 = n.parse().map_err(|_| format!("fps must be a whole number, got {n:?}"))?;
            if !(5..=60).contains(&n) {
                return Err(format!("fps must be 5 to 60, got {n}"));
            }
            spec.fps = n;
        } else if let Some(w) = lower.strip_prefix("window=") {
            if w == "keep" {
                spec.window = None;
            } else {
                let (a, b) = w.split_once('x').ok_or_else(|| format!("window must be WxH or keep, got {w:?}"))?;
                let (a, b): (u32, u32) = (
                    a.parse().map_err(|_| format!("window width {a:?} is not a number"))?,
                    b.parse().map_err(|_| format!("window height {b:?} is not a number"))?,
                );
                if !(320..=3840).contains(&a) || !(200..=2160).contains(&b) {
                    return Err(format!("window {a}x{b} is outside 320x200 to 3840x2160"));
                }
                spec.window = Some((a, b));
            }
        } else {
            return Err(format!("unknown part {tok:?} (want 1, fps=N, window=WxH or window=keep)"));
        }
    }
    Ok(Some(spec))
}

/// The spec this process runs with, read from the environment once. None = a normal game.
pub fn spec() -> Option<&'static Spec> {
    static SPEC: OnceLock<Option<Spec>> = OnceLock::new();
    SPEC.get_or_init(|| {
        let raw = std::env::var(ENV).unwrap_or_default();
        match parse(&raw) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[low-impact] {ENV}={raw:?}: {e}; running the defaults (fps {DEFAULT_FPS}, window {}x{})", DEFAULT_WINDOW.0, DEFAULT_WINDOW.1);
                Some(Spec { fps: DEFAULT_FPS, window: Some(DEFAULT_WINDOW), raw })
            }
        }
    })
    .as_ref()
}

/// Is low impact on in this process?
pub fn on() -> bool {
    spec().is_some()
}

/// The window low impact asks for, when it asks for one.
pub fn window() -> Option<(u32, u32)> {
    spec().and_then(|s| s.window)
}

/// Is `key` a showcase pin low impact holds right now?
pub fn holds_pin(key: &str) -> bool {
    on() && HELD_PINS.contains(&key)
}

/// The `minimum` preset's values, from the presets file's text. Its keys are AppConfig field
/// names (the presets' keys are SettingsState names, which match config.rs for every key here; a
/// test checks it). None when the file has no such preset.
pub fn preset_values(presets_json: &str) -> Option<Map<String, Value>> {
    let v: Value = serde_json::from_str(presets_json).ok()?;
    v.get("presets")?
        .as_array()?
        .iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some(PRESET_ID))?
        .get("values")?
        .as_object()
        .cloned()
}

/// Everything low impact lays over the config for `spec`: the preset's values, then the frame
/// keys (the spec's, so a preset cannot set a frame rate).
pub fn overlay_for(spec: &Spec, preset: Option<&Map<String, Value>>) -> Map<String, Value> {
    let mut m = preset.cloned().unwrap_or_default();
    m.insert("vsync".into(), Value::Bool(false));
    m.insert("fps_foreground_unlimited".into(), Value::Bool(false));
    m.insert("fps_foreground".into(), Value::from(spec.fps));
    m.insert("fps_background_sync".into(), Value::Bool(false));
    m.insert("fps_background".into(), Value::from(spec.fps));
    m
}

/// This process's overlay and whether its preset was found (None when low impact is off), built
/// once from the data dir's presets.
fn overlay_entry() -> Option<&'static (Map<String, Value>, bool)> {
    static OVERLAY: OnceLock<Option<(Map<String, Value>, bool)>> = OnceLock::new();
    OVERLAY
        .get_or_init(|| {
            let spec = spec()?;
            let path = crate::data_dir().join("gui").join("graphics_presets.json");
            let preset = std::fs::read_to_string(&path).ok().and_then(|t| preset_values(&t));
            if preset.is_none() {
                log::warn!(
                    "[low-impact] no \"{PRESET_ID}\" preset in {}: only the frame cap, the window and the renderer switches apply; the graphics settings stay as configured",
                    path.display()
                );
            }
            let found = preset.is_some();
            Some((overlay_for(spec, preset.as_ref()), found))
        })
        .as_ref()
}

/// This process's overlay (None when low impact is off).
fn overlay() -> Option<&'static Map<String, Value>> {
    overlay_entry().map(|(m, _)| m)
}

/// Whether the overlay found its preset (for the confirmation).
fn preset_found() -> bool {
    overlay_entry().is_some_and(|(_, found)| *found)
}

/// The config file's own values for the overlaid keys, as last read (None inside = the file did
/// not have the key): what `strip_for_save` puts back.
static ORIGINALS: Mutex<Option<Map<String, Value>>> = Mutex::new(None);

/// Lay `overlay` over a config object, returning the file's own values for those keys
/// (`Value::Null` marks a key the file did not have). Pure: the tested half of `lay_over_text`.
pub fn lay_over_value(cfg: &mut Map<String, Value>, overlay: &Map<String, Value>) -> Map<String, Value> {
    let mut originals = Map::new();
    for (k, v) in overlay {
        originals.insert(k.clone(), cfg.get(k).cloned().unwrap_or(Value::Null));
        cfg.insert(k.clone(), v.clone());
    }
    originals
}

/// Two JSON values the same setting? Numbers within float rounding (an f32 setting written back
/// is not bit-equal to the f64 the preset file holds).
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() <= 1e-6 * x.abs().max(y.abs()).max(1.0),
        _ => a == b,
    }
}

/// Undo `lay_over_value` on a config about to be saved: every key still at the overlay's value
/// goes back to the file's own (or leaves the file, when the file never had it). A key changed
/// since is somebody's own setting and stays. Pure: the tested half of `strip_for_save`.
pub fn strip_value(cfg: &mut Map<String, Value>, overlay: &Map<String, Value>, originals: Option<&Map<String, Value>>) {
    for (k, forced) in overlay {
        let Some(now) = cfg.get(k) else { continue };
        if !same(now, forced) {
            continue;
        }
        match originals.and_then(|o| o.get(k)) {
            Some(Value::Null) | None => {
                cfg.remove(k);
            }
            Some(orig) => {
                cfg.insert(k.clone(), orig.clone());
            }
        }
    }
}

/// config.rs `load` / `load_if_exists`: the config file's text with low impact laid over it, as
/// a JSON value for serde to read. None when low impact is off or the text is not an object (the
/// caller then parses the text as it always did).
pub fn lay_over_text(json: &str) -> Option<Value> {
    let overlay = overlay()?;
    let mut v: Value = serde_json::from_str(json).ok()?;
    let obj = v.as_object_mut()?;
    let originals = lay_over_value(obj, overlay);
    if let Ok(mut o) = ORIGINALS.lock() {
        *o = Some(originals);
    }
    Some(v)
}

/// lib.rs, a sandbox with no config.json yet: lay low impact over a config built from the
/// defaults (none of its keys came from a file, so none is ever saved back).
pub fn lay_over_config<T: serde::Serialize + serde::de::DeserializeOwned>(cfg: T) -> T {
    let Some(overlay) = overlay() else { return cfg };
    let Ok(mut v) = serde_json::to_value(&cfg) else { return cfg };
    let Some(obj) = v.as_object_mut() else { return cfg };
    let mut originals = lay_over_value(obj, overlay);
    for val in originals.values_mut() {
        *val = Value::Null;
    }
    if let Ok(mut o) = ORIGINALS.lock() {
        o.get_or_insert_with(Map::new).extend(originals);
    }
    serde_json::from_value(v).unwrap_or(cfg)
}

/// config.rs `save_at`: the config as it must be written, with low impact taken back out. None
/// when low impact is off (the caller writes the config as it always did).
pub fn strip_for_save<T: serde::Serialize>(cfg: &T) -> Option<Value> {
    let overlay = overlay()?;
    let mut v = serde_json::to_value(cfg).ok()?;
    let originals = ORIGINALS.lock().ok().and_then(|o| o.clone());
    strip_value(v.as_object_mut()?, overlay, originals.as_ref());
    Some(v)
}

/// The renderer switches that have no setting (point 4 at the top). Called once, right after the
/// renderer is made.
pub fn hold_renderer(r: &mut crate::renderer::Renderer) {
    if !on() {
        return;
    }
    r.godrays_pin = Some(false);
    r.emission.off = true;
    r.sun_glare_on = false;
    r.sun_cascades.shadows_pin = Some(false);
}

/// The one line run.log carries, and `debug/low_impact.json`, the confirmation spawnGame waits
/// for (scripts/lib/game-launch.js). Called once at boot, after the config is read.
pub fn write_confirmation() {
    let (Some(spec), Some(overlay)) = (spec(), overlay()) else { return };
    let window = match spec.window {
        Some((w, h)) => format!("{w}x{h}"),
        None => "keep".to_string(),
    };
    log::info!(
        "[low-impact] ON ({ENV}={:?}): {} fps, window {window}, {} settings laid over config.json and never saved ({}), renderer switches off (god rays, aurora, sun glare, sun shadows), showcase pins held: {}",
        spec.raw,
        spec.fps,
        overlay.len(),
        if preset_found() { format!("the \"{PRESET_ID}\" preset plus the frame keys") } else { "the frame keys only: NO preset found".to_string() },
        HELD_PINS.join(", ")
    );
    let body = serde_json::json!({
        "on": true,
        "value": spec.raw,
        "fps": spec.fps,
        "window": spec.window.map(|(w, h)| serde_json::json!([w, h])).unwrap_or(Value::from("keep")),
        "preset": PRESET_ID,
        "preset_found": preset_found(),
        "settings": Value::Object(overlay.clone()),
        "held_pins": HELD_PINS,
        "renderer_off": ["godrays", "aurora", "sun_glare", "sun_shadows"],
        "version": env!("CARGO_PKG_VERSION"),
    });
    let dir = std::path::Path::new("debug");
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("low_impact.json");
    let tmp = dir.join("low_impact.json.tmp");
    let ok = serde_json::to_vec_pretty(&body).ok().is_some_and(|bytes| std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &path).is_ok());
    if !ok {
        log::warn!("[low-impact] could not write {}: the rig that asked will stop this game", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_value_reads_as_the_rigs_write_it() {
        assert_eq!(parse("").unwrap(), None);
        assert_eq!(parse("0").unwrap(), None);
        assert_eq!(parse("off").unwrap(), None);
        let d = parse("1").unwrap().unwrap();
        assert_eq!((d.fps, d.window), (DEFAULT_FPS, Some(DEFAULT_WINDOW)));
        let k = parse("fps=30,window=keep").unwrap().unwrap();
        assert_eq!((k.fps, k.window), (30, None));
        let w = parse("1, window=1280x720").unwrap().unwrap();
        assert_eq!(w.window, Some((1280, 720)));
        assert!(parse("fps=200").is_err(), "a frame cap above 60 is not low impact");
        assert!(parse("window=10x10").is_err());
        assert!(parse("max").is_err(), "an unknown part is refused, not ignored");
    }

    /// Every value a rig asks for (scripts/lib/rig-needs.js RIGS lowImpact) is one this parse
    /// reads: a rig asking for something the engine rejects would run the defaults and its
    /// judgement (copresence's 30 fps, its full window) would be made on the wrong game.
    #[test]
    fn every_rigs_value_is_one_the_engine_reads() {
        let js = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/lib/rig-needs.js")).expect("rig-needs.js");
        let mut n = 0;
        for line in js.lines() {
            if let Some(rest) = line.trim().strip_prefix("lowImpact: \"") {
                let v = rest.split('"').next().unwrap_or_default();
                assert!(matches!(parse(v), Ok(Some(_))), "rig-needs.js asks for HUMANITY_LOW_IMPACT={v:?}, which the engine does not read: {:?}", parse(v));
                n += 1;
            }
        }
        assert!(n >= 5, "found only {n} lowImpact values in rig-needs.js: the scan no longer matches the file");
    }

    /// Every pin low impact holds is one the showcase poll reads through `grab`, which is where the
    /// hold lives (engine::ipc poll_showcase_request): a pin read some other way would slip past.
    #[test]
    fn every_held_pin_is_read_through_grab() {
        let src = include_str!("ipc.rs").replace("\r\n", "\n");
        let start = src.find("pub(crate) fn poll_showcase_request(").expect("the showcase poll");
        let body = &src[start..start + src[start..].find("\n}\n").expect("its end")];
        assert!(body.contains("crate::engine::low_impact::holds_pin(key)"), "the showcase poll's grab no longer holds low impact's pins");
        for k in HELD_PINS {
            assert!(body.contains(&format!("grab(\"{k}\")")), "the showcase poll does not read {k:?} through grab, so low impact cannot hold it");
        }
    }

    #[test]
    fn the_overlay_is_never_saved_but_a_change_made_during_the_run_is() {
        let spec = parse("fps=15").unwrap().unwrap();
        let preset: Map<String, Value> = serde_json::from_str(r#"{"cloud_quality":"low","ssao_strength":0.1,"grass_density":0.0}"#).unwrap();
        let overlay = overlay_for(&spec, Some(&preset));
        let mut cfg: Map<String, Value> =
            serde_json::from_str(r#"{"cloud_quality":"ultra","ssao_strength":1.0,"vsync":true,"fps_background":30,"fov":90.0}"#).unwrap();
        let originals = lay_over_value(&mut cfg, &overlay);
        assert_eq!(cfg["cloud_quality"], "low");
        assert_eq!(cfg["fps_background"], 15);
        assert_eq!(cfg["vsync"], false);
        assert_eq!(cfg["fov"], 90.0, "a key the overlay does not name is left alone");
        // The game writes its settings back: f32 0.1 is not bit-equal to the file's 0.1.
        cfg.insert("ssao_strength".into(), Value::from(0.1f32 as f64));
        // Somebody changed the clouds during the run: theirs, and saved.
        cfg.insert("grass_density".into(), Value::from(0.5));
        strip_value(&mut cfg, &overlay, Some(&originals));
        assert_eq!(cfg["cloud_quality"], "ultra", "the file's own value goes back");
        assert_eq!(cfg["ssao_strength"], 1.0, "a float at the overlay's value, through f32 rounding, goes back too");
        assert_eq!(cfg["vsync"], true);
        assert_eq!(cfg["fps_background"], 30);
        assert_eq!(cfg["grass_density"], 0.5, "a setting changed during the run is kept");
        assert!(!cfg.contains_key("fps_foreground"), "a key the file never had leaves it again");
    }

    #[test]
    fn the_minimum_preset_names_only_real_config_settings() {
        // RED if the preset is renamed, loses its rig_only mark, or names a key config.rs does not
        // have (a typo would silently leave that setting at full cost: serde ignores it).
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/gui/graphics_presets.json")).expect("presets file");
        let values = preset_values(&text).expect("the minimum preset");
        assert!(values.len() >= 10, "the minimum preset is nearly empty: {values:?}");
        let all: Value = serde_json::from_str(&text).unwrap();
        let p = all["presets"].as_array().unwrap().iter().find(|p| p["id"] == PRESET_ID).unwrap();
        assert_eq!(p["rig_only"], true, "the minimum preset must be rig_only (never listed in Settings)");
        let cfg = serde_json::to_value(crate::config::AppConfig::default()).unwrap();
        let known = cfg.as_object().unwrap();
        for k in values.keys() {
            assert!(known.contains_key(k), "the minimum preset names {k:?}, which config.json does not have");
        }
        for k in ["vsync", "fps_foreground", "fps_foreground_unlimited", "fps_background", "fps_background_sync"] {
            assert!(known.contains_key(k), "the frame key {k:?} is not a config.json setting");
        }
        // The rigs' showcase pin releases (scripts/lib/showcase-pins.js) that would put a cost back
        // are all held, or a sweep would undo low impact at its second vantage.
        let pins = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/lib/showcase-pins.js")).expect("showcase-pins.js");
        for k in ["aurora", "room_gi", "sun_shadows", "bounced_light_detail", "sun_glare", "godrays"] {
            assert!(pins.contains(&format!("  {k}: ")), "showcase-pins.js no longer releases {k:?}: update HELD_PINS and this list together");
            assert!(HELD_PINS.contains(&k), "the rigs release the {k:?} pin between vantages and low impact does not hold it");
        }
        // Nothing that changes what the game DOES (the top of this file).
        for k in ["water_fft", "tree_density", "gpu_particles", "planet_chunked", "planet_surface_detail", "terrain_builds_per_frame", "live_weather", "time_speed"] {
            assert!(!values.contains_key(k), "the minimum preset must not change {k:?}: it changes what the game does, not only what it draws");
        }
    }
}

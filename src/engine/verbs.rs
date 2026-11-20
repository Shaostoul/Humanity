//! The rigs' verbs as one table (the headless sim runner, increment 5, 2026-11-20;
//! docs/design/headless-sim-runner.md, section 2 "Inputs, one verb table, two transports").
//!
//! A rig drives the game with verbs: stand here, turn there, hold these keys for so long, set the
//! illness level, run a console line. Until now each verb was a block of `poll_showcase_request`
//! (engine/ipc.rs), reading the `debug/showcase_request.json` file drop, so the only way to send
//! one was a window and a file. This is the table both transports parse into:
//!
//! - the windowed file drop: `poll_showcase_request` takes each verb's key at the place in its
//!   long list where that verb's block stood (`take`), so a request that names several verbs
//!   still runs them in the order it always did, among the picture pins around them;
//! - the headless one: `HumanityOS --sim verbs -` (engine/sim_verbs.rs) reads JSON lines on stdin
//!   and runs a line's verbs through `apply_request`, in the same order (`KEYS`).
//!
//! A verb is read with the same scanner the file drop has always used (`ipc::showcase_value`), so a
//! request means the same thing on either transport. Only verbs that change the simulation are in
//! the table: the picture's pins (clouds, shadows, the HUD, the field of view, ...) stay in the
//! file drop, and so do the planet's `stand` and `build`, which read the ground the renderer drew
//! (increment 6 gives the planet's ground to the simulation). The headless transport refuses any
//! key the table does not hold and says why. `tests/sim_gpu_free_lint.rs` keeps this file free of
//! the renderer, the window and the wall clock; the poll's order is held by the test at the end.

use crate::engine::state::EngineState;
use glam::Vec3;
use humanity_core::input::bindings::GameAction;

/// Every verb's key, in the order `poll_showcase_request` takes them (and so the order a request's
/// verbs run in, on either transport). The test `the_showcase_poll_takes_every_verb_in_order`
/// reads the poll and holds it to this list.
pub(crate) const KEYS: &[&str] = &["walk", "illness", "gameplay", "face", "hold", "console", "time", "time_scale", "weather", "enter", "cam"];

/// The keys a verb reads beside its own (`hold_s` with `hold`, `weather_wind` with `weather`):
/// not verbs, and not refused by the headless transport.
pub(crate) const COMPANIONS: &[&str] = &["hold_s", "weather_wind"];

/// One verb, read from a request.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Verb {
    /// `{"walk":"1"}` (2026-09-28): fly mode off, gravity on, the way a player on foot is. Every
    /// camera park sets fly mode, and fly mode suspends the survival rules.
    OnFoot,
    /// `{"illness":"realistic"}` (2026-10-20, falls; Off 2026-11-11): Settings > Gameplay >
    /// Illness and injury, set the way its radio sets it (the setting and the save mark), so a rig
    /// can judge a fall in every level in one boot (scripts/verify-falls.js).
    Illness(String),
    /// `{"gameplay":"body_heat=off"}` (2026-11-11): any gameplay setting to any of its levels
    /// (data/gameplay/levels.json), set the way its radio sets it.
    Gameplay { id: String, level: String },
    /// `{"face":"yaw,pitch"}` (ship homes increment 7, 2026-10-21): the camera turned to that yaw
    /// and pitch (radians: yaw 0 north, +PI/2 east; pitch up positive) without moving the body, as
    /// a mouse turn does (scripts/verify-storeys.js, engine/storey_probe.rs). Before `hold`, so a
    /// turn and a hold sent together walk the new way.
    Face { yaw: f32, pitch: f32 },
    /// `{"hold":"forward,sprint","hold_s":"40"}` (BUG-156, 2026-10-05): those movement keys
    /// pressed for that many SIMULATION seconds through the controller's own action path
    /// (engine/rig_walk.rs), so a rig arrives somewhere on foot the way a player does.
    Hold { keys: Vec<GameAction>, secs: f32 },
    /// `{"console":"add_structure deck 1 0 2 0"}`: a line of the build editor's console on the live
    /// home, for this session only (`ipc::console_this_session`: never written to the data files
    /// or a save). verify-falls places the deck its walks aboard step off this way.
    Console(String),
    /// `{"time":"6.5"}`: the game clock to that hour of the current day, through the TimeSystem's
    /// request channel (its own accumulator is authoritative and re-exports every tick).
    TimeOfDay(f32),
    /// `{"time_scale":"0"}` (v0.1287, the rig clock freeze): the game clock held at that speed, a
    /// hold over the time-speed setting (`time::request_speed_hold`).
    TimeSpeed(f32),
    /// `{"weather":"rain","weather_wind":"0"}`: a weather condition pinned here, its wind with it
    /// (m/s, 0 to 60). A name the table does not know pins clear, as it always has.
    Weather { name: String, wind: Option<f32> },
    /// `{"enter":"default"}`: the Play button: the last WHO/WHERE pairing replayed into the world,
    /// the character picker when none is recorded yet.
    Enter,
    /// `{"cam":"x,y,z,yaw,pitch"}`: the body and the eye put there (in first person the camera
    /// follows the body, so the body is moved), the look set, and the world view shown.
    Place { pos: Vec3, yaw: f32, pitch: f32 },
}

/// What a verb's value said that the verb cannot take: the sentence for the log, after
/// "Showcase: ".
pub(crate) type Refusal = String;

impl Verb {
    /// Read verb `key` from a request through `value` (a request's `"key":"value"` scanner).
    /// `None` when the request does not name it, or names it with a value that has always meant
    /// "do nothing" (`walk` other than "1", a `cam` without five numbers, a `time` that is not a
    /// number); `Some(Err)` for a value the verb refuses, with the sentence the log has always said.
    pub(crate) fn parse(key: &str, value: &dyn Fn(&str) -> Option<String>) -> Option<Result<Verb, Refusal>> {
        let raw = value(key)?;
        Some(match key {
            "walk" => {
                if raw != "1" {
                    return None;
                }
                Ok(Verb::OnFoot)
            }
            "illness" => match crate::engine::body_probe::parse_illness_mode(&raw) {
                Some(level) => Ok(Verb::Illness(level)),
                None => Err(format!("illness wants off, forgiving or realistic, not {raw:?}")),
            },
            "gameplay" => {
                let (id, level) = raw.split_once('=').map_or((raw.trim(), ""), |(a, b)| (a.trim(), b.trim()));
                if crate::gui::gameplay_levels::setting(id).and_then(|s| s.level(level)).is_some() {
                    Ok(Verb::Gameplay { id: id.to_string(), level: level.to_string() })
                } else {
                    Err(format!("gameplay wants setting=level from data/gameplay/levels.json, not {raw:?}"))
                }
            }
            "face" => match crate::engine::storey_probe::parse_face(&raw) {
                Some((yaw, pitch)) => Ok(Verb::Face { yaw, pitch }),
                None => Err(format!("face wants \"yaw,pitch\" in radians, not {raw:?}")),
            },
            "hold" => {
                let secs = value("hold_s").and_then(|s| s.parse::<f32>().ok()).unwrap_or(5.0);
                match crate::engine::rig_walk::parse_keys(&raw) {
                    Some(keys) => Ok(Verb::Hold { keys, secs }),
                    None => Err(format!("hold -> hold wants movement keys (forward, back, left, right, jump, sprint), not {raw:?}")),
                }
            }
            "console" => Ok(Verb::Console(raw)),
            "time" => Ok(Verb::TimeOfDay(raw.parse::<f32>().ok()?)),
            "time_scale" => Ok(Verb::TimeSpeed(raw.parse::<f32>().ok()?)),
            "weather" => Ok(Verb::Weather { name: raw, wind: value("weather_wind").and_then(|x| x.parse::<f32>().ok()).map(|v| v.clamp(0.0, 60.0)) }),
            "enter" => Ok(Verb::Enter),
            "cam" => {
                let v: Vec<f32> = raw.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                if v.len() != 5 {
                    return None;
                }
                Ok(Verb::Place { pos: Vec3::new(v[0], v[1], v[2]), yaw: v[3], pitch: v[4] })
            }
            _ => return None,
        })
    }

    /// Run the verb on the engine. Returns the sentence the log says after "Showcase: ", or None
    /// for the verbs that have never said one.
    pub(crate) fn apply(self, state: &mut EngineState) -> Option<String> {
        match self {
            Verb::OnFoot => {
                state.gui_state.dev_fly_mode = false;
                state.gui_state.dev_hover = false;
                state.controller.fly_mode = false;
                Some("walk -> fly mode off, on foot".into())
            }
            Verb::Illness(level) => {
                set_gameplay_level(state, "illness", &level);
                Some(format!("illness -> {level}"))
            }
            Verb::Gameplay { id, level } => {
                set_gameplay_level(state, &id, &level);
                Some(format!("gameplay {id} -> {level}"))
            }
            Verb::Face { yaw, pitch } => {
                state.camera.yaw = yaw;
                state.camera.pitch = pitch;
                Some(format!("face -> yaw {yaw:.3}, pitch {pitch:.3}"))
            }
            Verb::Hold { keys, secs } => Some(format!("hold -> {}", crate::engine::rig_walk::hold(state, keys, secs))),
            Verb::Console(line) => {
                let (said, rebuild) = crate::engine::ipc::console_this_session(&mut state.gui_state, &line);
                if rebuild {
                    crate::engine::home_meshes::rebuild_homestead(state);
                }
                Some(format!("console {line} -> {}", said.trim_end().replace('\n', " | ")))
            }
            Verb::TimeOfDay(hours) => {
                if let Some(req) = state.data_store.get::<std::sync::Mutex<Option<f32>>>("time_set_hour_request") {
                    if let Ok(mut r) = req.lock() {
                        *r = Some(hours);
                    }
                }
                None
            }
            Verb::TimeSpeed(scale) => {
                humanity_systems::time::request_speed_hold(&state.data_store, Some(scale.max(0.0)));
                None
            }
            Verb::Weather { name, wind } => {
                use humanity_systems::weather::WeatherCondition as W;
                let (cond, intensity) = match name.to_ascii_lowercase().as_str() {
                    "clear" => (W::Clear, 0.0),
                    "cloudy" => (W::Cloudy, 0.4),
                    "rain" => (W::Rain, 0.9),
                    "storm" => (W::Storm, 1.0),
                    "snow" => (W::Snow, 0.7),
                    "fog" => (W::Fog, 1.0),
                    "sandstorm" => (W::Sandstorm, 1.0),
                    _ => (W::Clear, 0.0),
                };
                if let Some(m) = state.data_store.get::<std::sync::Mutex<humanity_systems::weather::WeatherControl>>("weather_control") {
                    if let Ok(mut c) = m.lock() {
                        // A calm clear night is the case the body's "feels" figure is most
                        // different from the air (2026-09-29), so the wind can be pinned too.
                        c.manual = Some(humanity_systems::weather::ManualWeather {
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
                // Pinning a condition places it HERE. Without this the pin would adopt whatever
                // anchor the previous condition left behind.
                state.weather_anchor = None;
                Some(format!("weather -> {name}"))
            }
            Verb::Enter => {
                if !state.gui_state.apply_last_pairing() {
                    state.gui_state.launcher_open_select = true;
                }
                state.gui_state.enter_world();
                None
            }
            Verb::Place { pos, yaw, pitch } => {
                // In first person the camera derives from the PLAYER each frame, so the player's
                // body is moved; yaw and pitch live on the camera itself (mouse deltas accumulate
                // onto them), so those stick when set directly.
                for (_e, (t, _c)) in state
                    .game_world
                    .world
                    .query_mut::<(&mut humanity_core::ecs::components::Transform, &humanity_core::ecs::components::Controllable)>()
                {
                    t.position = pos;
                }
                state.camera.position = pos;
                state.camera.yaw = yaw;
                state.camera.pitch = pitch;
                state.gui_state.enter_world();
                None
            }
        }
    }
}

/// Choose `level` for the gameplay setting `id` the way its radio does (the setting and the save
/// mark), for the verbs `illness` and `gameplay` and the file drop's `walk_pace` and
/// `animation_mode`.
pub(crate) fn set_gameplay_level(state: &mut EngineState, id: &str, level: &str) {
    if state.gui_state.settings.level(id) != level {
        state.gui_state.settings.set_level(id, level);
        state.gui_state.settings_dirty = true;
    }
}

/// The file drop's way in: read verb `key` through `value` and run it, logging what it said as the
/// showcase block it replaced did. `poll_showcase_request` calls this once per key of `KEYS`, at
/// the place that verb's block stood.
pub(crate) fn take(state: &mut EngineState, value: &dyn Fn(&str) -> Option<String>, key: &str) {
    match Verb::parse(key, value) {
        None => {}
        Some(Ok(verb)) => {
            if let Some(said) = verb.apply(state) {
                log::info!("Showcase: {said}");
            }
        }
        Some(Err(why)) => log::warn!("Showcase: {why}"),
    }
}

/// What one request's verbs did: each verb's sentence (or its key, for the verbs that say none),
/// and each refusal.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Applied {
    pub said: Vec<String>,
    pub refused: Vec<String>,
}

/// Every verb a request names, run in `KEYS` order: the headless transport's way in.
pub(crate) fn apply_request(state: &mut EngineState, value: &dyn Fn(&str) -> Option<String>) -> Applied {
    let mut out = Applied::default();
    for key in KEYS {
        match Verb::parse(key, value) {
            None => {}
            Some(Ok(verb)) => out.said.push(verb.apply(state).unwrap_or_else(|| format!("{key} -> done"))),
            Some(Err(why)) => out.refused.push(why),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(text: &str) -> impl Fn(&str) -> Option<String> + '_ {
        move |k: &str| crate::engine::ipc::showcase_value(text, k)
    }

    /// THE VERBS READ AS THE FILE DROP HAS ALWAYS READ THEM: the request bodies exactly as
    /// verify-storeys and verify-falls write them reach each verb, with its companion key, and the
    /// values that have always meant "do nothing" still mean it.
    #[test]
    fn the_rigs_requests_parse_into_verbs() {
        let r = r#"{"walk":"1","cam":"10.000,1.700,-4.500,1.5708,0.0000"}"#;
        assert_eq!(Verb::parse("walk", &request(r)), Some(Ok(Verb::OnFoot)));
        assert_eq!(Verb::parse("cam", &request(r)), Some(Ok(Verb::Place { pos: Vec3::new(10.0, 1.7, -4.5), yaw: 1.5708, pitch: 0.0 })));
        assert_eq!(Verb::parse("face", &request(r#"{"face":"0.5000,-0.2000"}"#)), Some(Ok(Verb::Face { yaw: 0.5, pitch: -0.2 })));
        assert_eq!(
            Verb::parse("hold", &request(r#"{"hold":"forward,sprint","hold_s":"1.600"}"#)),
            Some(Ok(Verb::Hold { keys: vec![GameAction::MoveForward, GameAction::Sprint], secs: 1.6 }))
        );
        assert_eq!(Verb::parse("hold", &request(r#"{"hold":"jump"}"#)), Some(Ok(Verb::Hold { keys: vec![GameAction::Jump], secs: 5.0 })), "five seconds when hold_s is missing");
        assert_eq!(Verb::parse("illness", &request(r#"{"illness":"Realistic"}"#)), Some(Ok(Verb::Illness("realistic".into()))));
        assert_eq!(Verb::parse("console", &request(r#"{"console":"rm_structure 3"}"#)), Some(Ok(Verb::Console("rm_structure 3".into()))));
        assert_eq!(Verb::parse("weather", &request(r#"{"weather":"rain","weather_wind":"99"}"#)), Some(Ok(Verb::Weather { name: "rain".into(), wind: Some(60.0) })));
        assert_eq!(Verb::parse("time_scale", &request(r#"{"time_scale":"0"}"#)), Some(Ok(Verb::TimeSpeed(0.0))));
        assert_eq!(Verb::parse("time", &request(r#"{"time_scale":"0"}"#)), None, "time_scale is not time");
        assert_eq!(Verb::parse("enter", &request(r#"{"enter":"default"}"#)), Some(Ok(Verb::Enter)));
        // What has always meant nothing still means nothing.
        assert_eq!(Verb::parse("walk", &request(r#"{"walk":"0"}"#)), None);
        assert_eq!(Verb::parse("cam", &request(r#"{"cam":"1,2,3"}"#)), None);
        assert_eq!(Verb::parse("time", &request(r#"{"time":"dawn"}"#)), None);
        assert_eq!(Verb::parse("face", &request(r#"{"walk":"1"}"#)), None, "a request that does not name a verb");
        // And what has always been refused is refused, with the sentence the log has always said.
        assert!(matches!(Verb::parse("illness", &request(r#"{"illness":"sometimes"}"#)), Some(Err(e)) if e.contains("off, forgiving or realistic")));
        assert!(matches!(Verb::parse("hold", &request(r#"{"hold":"interact"}"#)), Some(Err(e)) if e.contains("movement keys")));
        assert!(matches!(Verb::parse("face", &request(r#"{"face":"north"}"#)), Some(Err(_))));
        for key in KEYS {
            assert!(!COMPANIONS.contains(key), "{key} is both a verb and a companion");
        }
    }

    /// THE FILE DROP TAKES EVERY VERB OF THE TABLE, IN THE TABLE'S ORDER, AND NO LONGER RUNS ONE
    /// ITSELF. `poll_showcase_request` (engine/ipc.rs) calls `take` once for each key of `KEYS`, in
    /// that order, which is the order the headless transport runs a request's verbs in
    /// (`apply_request`); and it no longer reads any of those keys with its own `grab`, so a verb
    /// cannot be changed in one transport and not the other.
    ///
    /// Seen red 2026-11-20: with the poll's `take(.., "face")` moved after its `take(.., "hold")`,
    /// "the showcase poll takes `hold` before `face`: KEYS has face first".
    #[test]
    fn the_showcase_poll_takes_every_verb_in_order() {
        let ipc = include_str!("ipc.rs").replace("\r\n", "\n");
        let start = ipc.find("pub(crate) fn poll_showcase_request(").expect("the showcase poll");
        let poll = &ipc[start..start + ipc[start..].find("\n}\n").expect("its end")];
        let mut last = 0;
        let mut last_key = "";
        for key in KEYS {
            let call = format!("crate::engine::verbs::take(state, &grab, \"{key}\");");
            let at = poll.find(&call).unwrap_or_else(|| panic!("the showcase poll never takes `{key}` through the verb table: no `{call}`"));
            assert_eq!(poll.matches(&call).count(), 1, "the showcase poll takes `{key}` more than once");
            assert!(at > last, "the showcase poll takes `{key}` before `{last_key}`: KEYS has {last_key} first");
            last = at;
            last_key = key;
            assert!(!poll.contains(&format!("grab(\"{key}\")")), "the showcase poll reads `grab(\"{key}\")` itself: the verb runs outside the table");
        }
        for key in COMPANIONS {
            assert!(!poll.contains(&format!("grab(\"{key}\")")), "the showcase poll reads the companion `grab(\"{key}\")` itself");
        }
    }
}

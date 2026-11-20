//! `HumanityOS --sim verbs [-|FILE] [--data DIR]`: the rigs' verbs with no window, no event loop
//! and no wgpu Instance (the headless sim runner, increment 5, 2026-11-20;
//! docs/design/headless-sim-runner.md, section 2 "Inputs, one verb table, two transports").
//!
//! The engine boots as the windowed app boots it (`EngineState::boot(opts, None)`), enters the
//! homestead through the same `world_load::load_world` (its drawing skipped and named), and then
//! reads one JSON object a line, on stdin (`-`) or from FILE, answering each with one JSON line on
//! stdout. A line is one of:
//!
//! - `{"do":"verbs","verbs":{"walk":"1","cam":"x,y,z,yaw,pitch"}}`: the verbs a rig would drop in
//!   `debug/showcase_request.json`, read by the same scanner and run through the same table
//!   (engine/verbs.rs), in the same order. A key the table does not hold is refused, by name, with
//!   the reason (the picture's pins need the picture; the planet's `stand` and `build` need the
//!   planet's ground, increment 6), and nothing of that line runs.
//! - `{"do":"step","s":1.5}`: the world stepped on for that many simulation seconds, 1/60 s a step,
//!   the windowed frame's step (`frame_step::run`), the rigs' held keys let go when their time is up
//!   (`rig_walk::tick`, as the file drop's poll does after each frame).
//! - `{"do":"record_begin","s":2.2}` and `{"do":"record_end"}`: the recorder, as the windowed
//!   recorder (`debug/remote_players_request.json`) records: its first frame the state now, then a
//!   frame a step, until that many seconds; between the two lines a rig may send verbs and steps
//!   (a hold sent 0.1 s into a recording, as the windowed rig does). `record_end` steps on to the
//!   recording's end and answers its done JSON, in the windowed recorder's shape for every field
//!   the simulation has (`ipc::recorder_frame`, `ipc::recorder_done`), so the rigs' judges read a
//!   headless run unchanged. `{"do":"record","s":N}` is the two at once.
//! - `{"do":"quit"}` (or the input's end).
//!
//! The first line out, before any line is read, says the engine is in the world: `{"ok":true,
//! "hello":...}` with the data folder, the step and the verbs the table holds. The log goes to
//! stderr, the windowed log's lines (a rig keeps it as the run's run.log: its PANIC count, its
//! built-in data lines and its "Data directory" line read as a windowed run's do).
//!
//! Safety, as `--sim walk`: saves go to a scratch folder removed afterwards, no config is read or
//! written, and none of the app's outside services start (`BootOptions::services` false).

use crate::engine::frame_prelude::*;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// The simulation step, seconds: the windowed game's at 60 frames a second.
const STEP_S: f32 = 1.0 / 60.0;
/// The most one `step` line may ask for, simulation seconds (an hour of the world at 1/60 s is
/// 216,000 steps, about a minute and a half of wall time).
const MAX_STEP_S: f64 = 3600.0;
/// A recording's longest ask and most frames: the windowed recorder's.
const MAX_RECORD_S: f64 = crate::engine::ipc::REMOTE_RECORD_MAX_S;
const MAX_RECORD_FRAMES: usize = crate::engine::ipc::REMOTE_RECORD_MAX_FRAMES;

/// Verbs the table holds that a run with no GPU cannot take yet, and why.
const NOT_YET: &[(&str, &str)] = &[(
    "console",
    "the home's rebuild after a console edit builds its meshes and its walls together (home_meshes::rebuild_homestead); with no GPU its walls would not follow the edit, so the console waits for that rebuild's split (verify-falls aboard needs it)",
)];

/// One recording in flight: the windowed recorder's, on the step's clock.
struct Recording {
    seconds: f64,
    clock: f64,
    frames: Vec<serde_json::Value>,
    camera_start: serde_json::Value,
    truncated: bool,
    done: bool,
}

/// The engine booted with no GPU and the homestead entered: the transport's world.
pub(crate) struct Session {
    pub(crate) state: EngineState,
    frame: FrameVars,
    rec: Option<Recording>,
    saves: PathBuf,
    data_dir: PathBuf,
    entry_s: f64,
}

impl Session {
    /// Boot with no GPU on `data_dir`, saves in a scratch folder, and enter the homestead; the
    /// world then runs a second before the first line is read (the systems' first ticks, the
    /// doors and the air settled), as `--sim walk` does.
    pub(crate) fn boot(data_dir: &Path) -> Result<Session, String> {
        let saves = std::env::temp_dir().join(format!("humanityos-verbs-{}-{:?}", std::process::id(), std::thread::current().id()).replace(['(', ')'], ""));
        let _ = std::fs::remove_dir_all(&saves);
        std::fs::create_dir_all(&saves).map_err(|e| format!("{}: {e}", saves.display()))?;
        let t0 = std::time::Instant::now();
        let mut state = EngineState::boot(
            crate::engine::engine_boot::BootOptions {
                data_dir: data_dir.to_path_buf(),
                saves_dir: saves.clone(),
                config: None,
                services: false,
                boot_timer: humanity_core::boot_timing::BootTimer::new(t0),
                egui_ctx: egui::Context::default(),
            },
            None,
        );
        crate::engine::world_load::load_world(&mut state);
        // The person steps into the world (the systems run from the first step, BUG-253).
        state.gui_state.enter_world_if(true);
        if state.gui_state.ship_structure.is_none() {
            drop(state);
            let _ = std::fs::remove_dir_all(&saves);
            return Err("the world entered with no ship structure: no homestead aboard".into());
        }
        let mut s = Session { state, frame: FrameVars::new(std::time::Instant::now()), rec: None, saves, data_dir: data_dir.to_path_buf(), entry_s: 0.0 };
        s.step_for(1.0);
        s.entry_s = t0.elapsed().as_secs_f64();
        Ok(s)
    }

    /// The first line out: in the world, and what this transport takes.
    pub(crate) fn hello(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": true,
            "hello": "HumanityOS --sim verbs: in the homestead with no window and no GPU",
            "data_dir": self.data_dir.display().to_string(),
            "entry_s": self.entry_s,
            "step_s": STEP_S,
            "verbs": crate::engine::verbs::KEYS,
            "not_yet": NOT_YET.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            "not_drawn": self.state.gpu_dirty,
        })
    }

    /// One step of the world, in the windowed frame's order: the clock on, the step, the recorder's
    /// frame (the windowed recorder records in frame_dev_ipc, after the step), then the held keys'
    /// release (the file drop's poll, after that).
    fn step(&mut self) {
        self.state.clock.advance_by(STEP_S);
        crate::engine::frame_step::run(&mut self.state, &mut self.frame);
        if let Some(rec) = self.rec.as_mut().filter(|r| !r.done) {
            rec.clock += f64::from(STEP_S);
            push_frame(rec, &self.state, f64::from(STEP_S));
        }
        crate::engine::rig_walk::tick(&mut self.state);
    }

    /// Step for `s` simulation seconds (rounded to whole steps; none for 0).
    fn step_for(&mut self, s: f64) -> u64 {
        let n = (s.clamp(0.0, MAX_STEP_S) / f64::from(STEP_S)).round() as u64;
        for _ in 0..n {
            self.step();
        }
        n
    }

    /// Start a recording of `seconds`: its first frame is the world as it stands.
    fn record_begin(&mut self, seconds: f64) -> Result<(), String> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(format!("\"s\" must be 0 or more, got {seconds}"));
        }
        let mut rec = Recording {
            seconds: seconds.min(MAX_RECORD_S),
            clock: 0.0,
            frames: Vec::new(),
            camera_start: crate::engine::ipc::recorder_camera(&self.state),
            truncated: false,
            done: false,
        };
        push_frame(&mut rec, &self.state, 0.0);
        self.rec = Some(rec);
        Ok(())
    }

    /// Step on to the recording's end and answer its done JSON.
    fn record_end(&mut self) -> Result<serde_json::Value, String> {
        if self.rec.is_none() {
            return Err("record_end with no recording begun (record_begin first)".into());
        }
        while self.rec.as_ref().is_some_and(|r| !r.done) {
            self.step();
        }
        let rec = self.rec.take().ok_or("no recording")?;
        let mut done = crate::engine::ipc::recorder_done(&self.state);
        done.insert("ok".into(), serde_json::json!(true));
        done.insert("headless".into(), serde_json::json!(true));
        done.insert("seconds".into(), serde_json::json!(rec.seconds));
        done.insert("recorded_s".into(), serde_json::json!(rec.clock));
        done.insert("frame_count".into(), serde_json::json!(rec.frames.len()));
        done.insert("truncated".into(), serde_json::json!(rec.truncated));
        done.insert("camera_start".into(), rec.camera_start);
        done.insert("frames".into(), serde_json::Value::Array(rec.frames));
        Ok(serde_json::Value::Object(done))
    }

    /// Run one request's verbs, or refuse the whole line naming what the table does not hold.
    fn verbs(&mut self, verbs: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
        use crate::engine::verbs::{COMPANIONS, KEYS};
        let mut refused: Vec<String> = Vec::new();
        for key in verbs.keys() {
            if let Some((_, why)) = NOT_YET.iter().find(|(k, _)| *k == key.as_str()) {
                refused.push(format!("{key}: not with no GPU yet: {why}"));
            } else if !KEYS.contains(&key.as_str()) && !COMPANIONS.contains(&key.as_str()) {
                refused.push(format!(
                    "{key}: not a verb of the table (engine/verbs.rs): the picture's pins need the picture, and the planet's stand and build need the planet's ground (increment 6)"
                ));
            }
        }
        if !refused.is_empty() {
            return serde_json::json!({ "ok": false, "error": "refused: nothing of this line ran", "refused": refused });
        }
        // The scanner the file drop reads with, over the same text a rig would have dropped.
        let text = serde_json::Value::Object(verbs.clone()).to_string();
        let value = |k: &str| crate::engine::ipc::showcase_value(&text, k);
        let applied = crate::engine::verbs::apply_request(&mut self.state, &value);
        serde_json::json!({ "ok": applied.refused.is_empty(), "said": applied.said, "refused": applied.refused })
    }

    /// Answer one input line; the bool is whether the run ends.
    pub(crate) fn handle(&mut self, line: &str) -> (serde_json::Value, bool) {
        let err = |e: String| (serde_json::json!({ "ok": false, "error": e }), false);
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return err(format!("not a JSON line: {e}")),
        };
        let s = v.get("s").and_then(|s| s.as_f64());
        match v.get("do").and_then(|d| d.as_str()).unwrap_or("") {
            "verbs" => match v.get("verbs").and_then(|o| o.as_object()) {
                Some(o) => (self.verbs(o), false),
                None => err("\"verbs\" wants an object of verbs, as a showcase request holds them".into()),
            },
            "step" => match s {
                Some(s) if s.is_finite() && s >= 0.0 => {
                    let steps = self.step_for(s);
                    (serde_json::json!({ "ok": true, "steps": steps, "elapsed_s": self.state.clock.elapsed_s }), false)
                }
                _ => err("step wants \"s\", 0 or more simulation seconds".into()),
            },
            "record_begin" => match self.record_begin(s.unwrap_or(f64::NAN)) {
                Ok(()) => (serde_json::json!({ "ok": true }), false),
                Err(e) => err(e),
            },
            "record_end" => match self.record_end() {
                Ok(done) => (done, false),
                Err(e) => err(e),
            },
            "record" => match self.record_begin(s.unwrap_or(f64::NAN)).and_then(|()| self.record_end()) {
                Ok(done) => (done, false),
                Err(e) => err(e),
            },
            "quit" => (serde_json::json!({ "ok": true, "bye": true }), true),
            other => err(format!("\"do\": {other:?} is not one of verbs, step, record_begin, record_end, record, quit")),
        }
    }
}

impl Session {
    /// The run over: the engine dropped, then its scratch saves removed (after, so no file in
    /// them is still held open).
    pub(crate) fn finish(self) {
        let saves = self.saves.clone();
        drop(self);
        let _ = std::fs::remove_dir_all(&saves);
    }
}

/// The recorder's frame for the world now, into `rec` (held at the windowed recorder's cap).
fn push_frame(rec: &mut Recording, state: &EngineState, dt: f64) {
    if rec.frames.len() < MAX_RECORD_FRAMES {
        rec.frames.push(crate::engine::ipc::recorder_frame(state, rec.clock, dt as f32));
    } else {
        rec.truncated = true;
    }
    if rec.clock >= rec.seconds {
        rec.done = true;
    }
}

/// The windowed log's lines on stderr, and a panic said as the windowed log says it (a rig counts
/// its PANIC lines).
fn init_log() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).format_timestamp_millis().target(env_logger::Target::Stderr).try_init();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("PANIC: {info}");
        default_hook(info);
    }));
}

/// `--sim verbs` from the command line: boot, answer every line, and leave.
pub(crate) fn cli(args: &[String], found_data_dir: &Path) -> Result<(), String> {
    init_log();
    let i = args.iter().position(|a| a == "verbs").ok_or("--sim verbs")?;
    let source = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "-".into());
    let (data_dir, how) = match args.iter().position(|a| a == "--data").and_then(|j| args.get(j + 1)) {
        Some(d) => (PathBuf::from(d), "--data"),
        None => (found_data_dir.to_path_buf(), "found"),
    };
    if !data_dir.join("homes").join("homestead.ron").exists() {
        return Err(format!("{}: no homes/homestead.ron there; --data wants a whole data folder", data_dir.display()));
    }
    // The line the windowed log has (lib.rs `find_data_dir`), which a rig reads the folder from.
    log::info!("Data directory (sim verbs, {how}): {}", data_dir.display());
    let mut session = Session::boot(&data_dir)?;
    let out = std::io::stdout();
    let say = |v: &serde_json::Value| -> Result<(), String> {
        let mut o = out.lock();
        writeln!(o, "{v}").and_then(|()| o.flush()).map_err(|e| format!("stdout: {e}"))
    };
    say(&session.hello())?;
    let input: Box<dyn BufRead> = if source == "-" {
        Box::new(std::io::BufReader::new(std::io::stdin()))
    } else {
        Box::new(std::io::BufReader::new(std::fs::File::open(&source).map_err(|e| format!("{source}: {e}"))?))
    };
    for line in input.lines() {
        let line = line.map_err(|e| format!("reading {source}: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let (answer, quit) = session.handle(&line);
        say(&answer)?;
        if quit {
            break;
        }
    }
    session.finish();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data")
    }

    fn eye_y(frame: &serde_json::Value) -> f64 {
        frame["cam"][1].as_f64().expect("a frame's eye")
    }

    /// THE VERBS RUN WITH NO GPU, ON THE STEP'S CLOCK, AND THE RECORDER ANSWERS IN ITS OWN SHAPE.
    /// The engine booted with no window and no wgpu Instance, in the homestead; a first look (a
    /// recording of 0 s) carries what the rigs' judges read (the home's box in the home session,
    /// the storeys block, the falls block, the frame's eye, health and storey); a jump held 0.1 s
    /// into a 2 s recording, as the windowed rig sends it, lifts the eye and sets it down again, its
    /// key let go on the step's clock (nothing held after); a line naming a key the table does not
    /// hold is refused whole (the turn beside it does not run), and so is the console, which waits
    /// for the home's rebuild to split. And the recorder's home session reads the run's scratch
    /// saves: the first headless storeys run (2026-11-20) read the person's own offline_home.json
    /// from %APPDATA% at each of its 49 recordings, through the global saves folder.
    ///
    /// Seen red 2026-11-20 with `home_session_json` reading `persistence::saves_dir()` again: "the
    /// recorder read a save outside the run's scratch folder:
    /// %APPDATA%/HumanityOS/saves/offline_home.json (scratch %TEMP%/humanityos-verbs-...)".
    #[test]
    fn the_verbs_drive_the_homestead_with_no_gpu() {
        let mut s = Session::boot(&data_dir()).expect("the homestead entered with no GPU");
        assert!(s.state.gpu.is_none(), "no GPU");
        let hello = s.hello();
        assert_eq!(hello["ok"], true, "{hello}");
        assert!(hello["verbs"].as_array().is_some_and(|v| v.iter().any(|k| k == "hold")), "{hello}");

        let (look, _) = s.handle(r#"{"do":"record","s":0}"#);
        assert_eq!(look["ok"], true, "{look}");
        assert_eq!(look["frame_count"], 1, "a recording of 0 s is one frame");
        assert!(look["home_session"]["home"]["box"]["min"].is_array(), "the home's box: {}", look["home_session"]["home"]);
        // The home session reads the scratch saves, never the person's own (%APPDATA%).
        let own = look["home_session"]["own_save"]["path"].as_str().unwrap_or_default().replace('\\', "/");
        let scratch = s.saves.display().to_string().replace('\\', "/");
        assert!(own.starts_with(&scratch), "the recorder read a save outside the run's scratch folder: {own} (scratch {scratch})");
        for key in ["storeys", "falls", "camera_end", "notices"] {
            assert!(!look[key].is_null(), "the done JSON has no {key}");
        }
        let f0 = &look["frames"][0];
        for key in ["t", "dt", "cam", "health", "storey"] {
            assert!(f0.get(key).is_some(), "the frame has no {key}: {f0}");
        }
        assert!(f0["storey"]["floor"].is_number(), "standing on a floor: {}", f0["storey"]);

        // A jump, sent 0.1 s into its recording as the windowed rig sends a hold.
        let (begun, _) = s.handle(r#"{"do":"record_begin","s":2.0}"#);
        assert_eq!(begun["ok"], true, "{begun}");
        s.handle(r#"{"do":"step","s":0.1}"#);
        let (said, _) = s.handle(r#"{"do":"verbs","verbs":{"hold":"jump","hold_s":"0.100"}}"#);
        assert_eq!(said["ok"], true, "{said}");
        assert!(said["said"][0].as_str().is_some_and(|t| t.starts_with("hold -> holding [Jump] for 0.1 s")), "{said}");
        let (done, _) = s.handle(r#"{"do":"record_end"}"#);
        assert_eq!(done["ok"], true);
        let frames = done["frames"].as_array().expect("frames");
        assert!((120..=122).contains(&frames.len()), "2 s at 1/60 s: {} frames", frames.len());
        let start = eye_y(&frames[0]);
        let peak = frames.iter().map(eye_y).fold(f64::MIN, f64::max);
        let end = eye_y(frames.last().unwrap());
        assert!(peak > start + 0.2, "the jump lifted the eye: {start:.3} to a peak of {peak:.3}");
        assert!((end - start).abs() < 0.05, "and set it down again: {start:.3} to {end:.3}");
        assert!(s.state.rig_hold.is_none(), "the jump's key is still held after its 0.1 s");

        // Refused whole: a picture pin beside a turn, and the console.
        let yaw = s.state.camera.yaw;
        let (r, _) = s.handle(r#"{"do":"verbs","verbs":{"clouds":"0","face":"1.0,0"}}"#);
        assert_eq!(r["ok"], false, "{r}");
        assert!(r["refused"][0].as_str().is_some_and(|t| t.starts_with("clouds:")), "{r}");
        assert_eq!(s.state.camera.yaw, yaw, "a refused line turned the camera");
        let (r, _) = s.handle(r#"{"do":"verbs","verbs":{"console":"rm_structure 1"}}"#);
        assert!(r["refused"][0].as_str().is_some_and(|t| t.contains("not with no GPU yet")), "{r}");
        let (bad, _) = s.handle("not json");
        assert_eq!(bad["ok"], false);
        let (bye, quit) = s.handle(r#"{"do":"quit"}"#);
        assert!(quit && bye["ok"] == true);
        s.finish();
    }
}

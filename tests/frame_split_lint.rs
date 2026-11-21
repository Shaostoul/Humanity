//! The frame's split between the simulation's step and the drawing (the headless sim runner,
//! increment 4, 2026-11-15; docs/design/headless-sim-runner.md, section 5). The windowed frame
//! (src/lib.rs, `WindowEvent::RedrawRequested`) runs its stages in order; this lint holds three
//! things about them, so the step the headless runner takes is the step the windowed game takes:
//!
//! - **the step runs first, once**: every stage of the simulation's step (the clock, the walk,
//!   the collisions, the bridges, the systems, the settles, the doors) is called exactly once, and
//!   all of them before any stage that draws or brings input. A new stage must be named in
//!   `STAGES` with its kind, so nothing slips into the frame unclassified;
//! - **the step reads no view state**: the step's stage files name no renderer, window, egui,
//!   particle system, drawn ground, drawn trees or shader clock, and no wall clock (the step's time
//!   is `state.clock`, `frame_sim::FrameClock`; frame_clock.rs is where the windowed driver turns
//!   the wall clock into it). The known exceptions are counted in `STEP_READS`, each with the
//!   increment that removes it, and the count must match exactly, so the list only shrinks;
//! - **the drawing writes no sim state**: the view's stage files do not change the ECS world, the
//!   DataStore, the systems, the body's position or the controller. Stages that bring input to the
//!   next step (the network, the editor and showroom, the cards' buttons, the GUI, the dev IPC, the
//!   world's entry) are `Input` and may.
//!
//! The step's stages are called through `frame_step::run` (engine/frame_step.rs), the one step the
//! windowed frame and a headless run (engine/sim_walk.rs) both take; the lint reads its stage list
//! from there, in its place in the frame.
//!
//! What it cannot see: a stage file's helpers in other files (frame_sky.rs's frame_sky_body.rs, the
//! step's frame_lock.rs). The `gpu: Option<Gpu>` split (same increment) makes the compiler find a
//! GPU touch in sim code wherever it hides; this lint holds the order and the stage files' words.
//!
//! Seen red 2026-11-15: with frame_settles' call moved back after frame_hud_voice in lib.rs,
//! "the step's stage `frame_settles` runs after `frame_particles` (View)"; with
//! `let _ = state.renderer.surface_size();` planted in frame_walk_aboard.rs, "frame_walk_aboard.rs
//! reads `state.renderer` 1 time(s), allowed 0"; with `let _ = state.game_world.world.despawn(e);`
//! planted in frame_lists_home.rs, "frame_lists_home.rs (View) writes sim state:
//! `state.game_world.world.despawn(`"; with a call to an unnamed `crate::engine::frame_new::run(state);`
//! added to the arm, "the frame calls `frame_new` (lib.rs line N), which STAGES does not name"; and,
//! since the step moved into frame_step.rs, with `frame_settles::run` called from lib.rs after the
//! step as well, "the step's stage `frame_settles` is called 2 times".

use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// Part of the simulation's step: run once, before anything else, the same in both drivers.
    Step,
    /// Brings input to the next step (it may write sim state, as a key press or a click does).
    Input,
    /// Draws the step's results: reads sim state, never writes it.
    View,
}
use Kind::*;

/// Every stage the windowed frame calls (`crate::engine::<module>::<fn>(` in the RedrawRequested
/// arm), its kind, and why.
const STAGES: &[(&str, Kind, &str)] = &[
    ("frame_clock", Step, "the frame's clock, advanced once"),
    ("frame_station", Step, "the station's orbit, boarding and riding"),
    ("frame_move_setup", Step, "speed multipliers, carried load, dev fly"),
    ("frame_walk_aboard", Step, "the elevator, footing, ladders, the body moved"),
    ("frame_probe_hold", Step, "a rig's camera park held over the integrator"),
    ("frame_house_power", Step, "the house lights drive a power consumer"),
    ("frame_planet_walk", Step, "the frame lock and the walk on a planet"),
    ("frame_collide_drive", Step, "wall and door collision, driving"),
    ("frame_look_env", Step, "look targets, landing injuries, the body's environment"),
    ("frame_bridges", Step, "the bridges' slice: the GUI's requests into the world"),
    ("frame_bridges_npc", Step, "the dev teleport, spawns, the creature editor, the NPC card"),
    ("frame_ecs", Step, "the systems' slice"),
    ("frame_settles", Step, "the game rules after the tick: collect, melee, loot, build, trade, equip, respawn"),
    ("doors", Step, "the doors' easing"),
    ("frame_particles", View, "ambient particles"),
    ("frame_copresence", Input, "the shared world: the network's players, our position sent"),
    ("frame_save_editor", Input, "the showroom and the construction editor, tools that edit the world"),
    ("frame_lists_home", View, "the home's draw lists"),
    ("frame_build_ghosts", View, "the build ghost drawn"),
    ("frame_build_gizmos", View, "the editor's gizmos drawn"),
    ("frame_build_helpers", View, "the editor's helpers drawn"),
    ("frame_people", View, "people and the planet pins drawn"),
    ("frame_sky", View, "the sky, the bodies, the near trees"),
    ("frame_hud_voice", View, "the HUD and the voice meter"),
    ("frame_mirrors", View, "the inventory, skills and quests mirrored to the pages"),
    ("frame_mirrors_world", View, "the world mirrored to the pages"),
    ("frame_lists_world", View, "the world's draw lists"),
    ("frame_env_publish", View, "the environment published to the pages and the renderer"),
    ("frame_cards", Input, "the machine cards' buttons (their requests join frame_sim_requests.rs later, as the tank card's did)"),
    ("frame_dev_ipc", Input, "the dev IPC's requests"),
    ("frame_load_screens", View, "the loading screens"),
    ("frame_world_passes", View, "the scene passes"),
    ("frame_ui_run", Input, "the GUI: its pages set the requests the next step carries"),
    ("redraw", View, "when the next frame is drawn on a phone, and the frame count (engine/redraw.rs)"),
    ("frame_audio", View, "the frame's sounds"),
    ("frame_ui_paint", View, "the GUI painted"),
    ("frame_tail", Input, "present, the settings mirrored to the controller, the world's entry"),
    ("quit", Input, "the way out on a lost GPU: the world saved"),
    ("ipc", Input, "the rigs' verbs"),
    ("world_prep", Input, "the frame's end for the page frames' timing (its length and laps); the module also opens the world when Play waited for it"),
];

/// The step's known reads of view state and the wall clock: (stage file, word, count, why and
/// which increment removes it). A count must match exactly: lower it when a read goes.
/// (Increment 5, 2026-11-20, took the probe hold's wall-clock grace out: it is timed on
/// `state.clock` now, and so are a rig's held keys, engine/rig_walk.rs.)
const STEP_READS: &[(&str, &str, usize, &str)] = &[
    ("frame_planet_walk", "state.gpu", 3, "the sky pass's fill scale (set when there is a view) and the sea's crest height (the renderer's default without one); increment 6 (the planet's ground owned by the sim)"),
    ("frame_planet_walk", "state.start_time", 1, "the sea's waves as the shader draws them; increment 6"),
    ("frame_planet_walk", ".elapsed()", 1, "the same read: the shader's clock since the app started; increment 6"),
    ("frame_planet_walk", "state.planet_chunk_states", 1, "the ground the renderer drew last frame, walked on; increment 6"),
    ("frame_bridges", "state.planet_chunk_states", 1, "the ground the renderer drew last frame, for the body's environment; increment 6"),
    ("frame_ecs", "state.planet_chunk_states", 1, "the ground the renderer drew last frame, for a death's pack; increment 6"),
];

/// View state and the wall clock: what the step's stage files must not read.
const VIEW_READS: &[&str] = &[
    "state.renderer",
    "state.window",
    "state.egui_state",
    "state.egui_renderer",
    "state.gpu",
    "gpu!(",
    "gpu_ref!(",
    "state.particle_system",
    "state.planet_chunk_states",
    "state.star_renderer",
    "state.near_trees",
    "state.start_time",
    "Instant",
    "SystemTime",
    ".elapsed()",
];

/// Sim state written: what the view's stage files must not do.
const SIM_WRITES: &[&str] = &[
    "&mutstate.game_world",
    "state.game_world.world.spawn(",
    "state.game_world.world.despawn(",
    "state.game_world.world.insert(",
    "state.game_world.world.insert_one(",
    "state.game_world.world.remove(",
    "state.game_world.world.remove_one(",
    "state.game_world.world.query_mut",
    "state.game_world.world.get::<&mut",
    "state.data_store.insert(",
    "&mutstate.data_store",
    "state.system_runner",
    "state.camera.position=",
    "state.camera.position+=",
    "state.camera.position-=",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// A line's code: everything before a `//` comment.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// A file's code with comments and every whitespace character removed, so a chain split over
/// lines (`state\n    .game_world\n    .world\n    .get::<&mut X>`) reads as one.
fn squeezed(text: &str) -> String {
    text.lines().map(code_of).flat_map(|l| l.chars()).filter(|c| !c.is_whitespace()).collect()
}

/// How often `word` (squeezed) appears in `code`, as a whole word: not followed by an identifier
/// character (so `state.renderer` does not count `state.renderer_x`), and an assignment not `==`.
fn count(code: &str, word: &str) -> usize {
    let w: String = word.chars().filter(|c| !c.is_whitespace()).collect();
    let mut n = 0;
    let mut at = 0;
    while let Some(i) = code[at..].find(&w) {
        let end = at + i + w.len();
        let next = code[end..].chars().next();
        let word_end = w.ends_with(|c: char| c.is_alphanumeric() || c == '_');
        let ok = match next {
            Some(c) if word_end && (c.is_alphanumeric() || c == '_') => false,
            Some('=') if w.ends_with('=') => false,
            _ => true,
        };
        if ok {
            n += 1;
        }
        at = end;
    }
    n
}

/// The `crate::engine::<module>::<fn>(` calls in lines `from..to` of `text`, in order: (module, line).
fn stage_calls(text: &str, from: usize, to: usize) -> Vec<(String, usize)> {
    let mut calls = Vec::new();
    for (n, line) in text.lines().enumerate().take(to).skip(from) {
        let code = code_of(line);
        let mut rest = code;
        while let Some(i) = rest.find("crate::engine::") {
            let after = &rest[i + "crate::engine::".len()..];
            let module: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let tail = &after[module.len()..];
            // A stage is `module::function(`: a module, one function, a call.
            if let Some(f) = tail.strip_prefix("::") {
                let func: String = f.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                let lower = func.chars().next().is_some_and(|c| c.is_lowercase());
                if lower && f[func.len()..].starts_with('(') {
                    calls.push((module.clone(), n + 1));
                }
            }
            rest = &after[module.len()..];
        }
    }
    calls
}

/// The stages the RedrawRequested arm calls, in order: (module, where). The step's stages are
/// called through `frame_step::run` (engine/frame_step.rs, the step both drivers take), so its
/// call is replaced by the stages it calls, in its order.
fn frame_calls() -> Vec<(String, String)> {
    let lib = read("src/lib.rs");
    let start = lib.lines().position(|l| l.contains("WindowEvent::RedrawRequested =>")).expect("lib.rs has no RedrawRequested arm");
    let end = lib.lines().skip(start).position(|l| l.contains("fn device_event(")).map(|e| start + e).expect("lib.rs: no device_event after the frame");
    let step = read("src/engine/frame_step.rs");
    let mut calls = Vec::new();
    for (m, n) in stage_calls(&lib, start, end) {
        if m == "frame_step" {
            for (s, k) in stage_calls(&step, 0, usize::MAX) {
                calls.push((s, format!("frame_step.rs line {k}")));
            }
        } else {
            calls.push((m, format!("lib.rs line {n}")));
        }
    }
    calls
}

fn kind_of(module: &str) -> Option<Kind> {
    STAGES.iter().find(|s| s.0 == module).map(|s| s.1)
}

#[test]
fn every_stage_is_named_and_the_step_runs_first_once() {
    let calls = frame_calls();
    assert!(calls.len() > 20, "found only {} stage calls in lib.rs's frame: the parser lost them", calls.len());
    let mut problems = Vec::new();
    for (m, line) in &calls {
        if kind_of(m).is_none() {
            problems.push(format!("the frame calls `{m}` ({line}), which STAGES does not name: say whether it is the step, input or drawing"));
        }
    }
    for (m, _, _) in STAGES {
        if !calls.iter().any(|(c, _)| c == m) {
            problems.push(format!("STAGES names `{m}`, which lib.rs's frame no longer calls: take it out"));
        }
    }
    // Every step stage exactly once.
    for (m, kind, _) in STAGES {
        if *kind == Step {
            let n = calls.iter().filter(|(c, _)| c == m).count();
            if n != 1 {
                problems.push(format!("the step's stage `{m}` is called {n} times in the frame: once"));
            }
        }
    }
    // All of them before the first stage that is not the step's.
    if let Some(first_other) = calls.iter().position(|(m, _)| kind_of(m) != Some(Step)) {
        let (other, _) = &calls[first_other];
        for (m, line) in &calls[first_other..] {
            if kind_of(m) == Some(Step) {
                problems.push(format!(
                    "the step's stage `{m}` ({line}) runs after `{other}` ({:?}): the step runs first, whole, before anything draws or brings input",
                    kind_of(other).unwrap_or(View)
                ));
            }
        }
    }
    assert!(problems.is_empty(), "The frame's split (docs/design/headless-sim-runner.md, increment 4):\n  {}", problems.join("\n  "));
}

#[test]
fn the_step_reads_no_view_state_and_the_view_writes_no_sim_state() {
    let mut problems = Vec::new();
    for (m, kind, _) in STAGES {
        let rel = format!("src/engine/{m}.rs");
        let path = root().join(&rel);
        if !path.exists() {
            continue; // `quit` and `ipc` are files too; a missing one is caught by the compiler
        }
        let code = squeezed(&read(&rel));
        match kind {
            Step => {
                // frame_clock.rs is where the windowed driver reads the wall clock into the step's.
                for word in VIEW_READS {
                    if *m == "frame_clock" && matches!(*word, "Instant" | "SystemTime" | ".elapsed()") {
                        continue;
                    }
                    let n = count(&code, word);
                    let allowed = STEP_READS.iter().find(|r| r.0 == *m && r.1 == *word).map_or(0, |r| r.2);
                    if n != allowed {
                        problems.push(format!(
                            "{m}.rs reads `{word}` {n} time(s), allowed {allowed}{}",
                            if n < allowed { ": one went, so lower its count in STEP_READS" } else { ": the step takes plain values or the frame's clock (state.clock), never view state" }
                        ));
                    }
                }
            }
            View => {
                for word in SIM_WRITES {
                    if count(&code, word) > 0 {
                        problems.push(format!("{m}.rs (View) writes sim state: `{word}`; a game rule belongs in the step (frame_settles.rs), a request in frame_sim_requests.rs"));
                    }
                }
            }
            Input => {}
        }
    }
    for (m, word, _, _) in STEP_READS {
        if kind_of(m) != Some(Step) {
            problems.push(format!("STEP_READS names `{m}` ({word}), which is not one of the step's stages"));
        }
    }
    assert!(problems.is_empty(), "The frame's split (docs/design/headless-sim-runner.md, increment 4):\n  {}", problems.join("\n  "));
}

/// The headless drivers (engine/sim_walk.rs, the route walk of increment 4, and engine/sim_verbs.rs,
/// the verbs' transport of increment 5) boot the engine as the windowed app does, with no GPU, and
/// step the world through `frame_step::run`, the windowed frame's step, calling none of the step's
/// stages themselves: one step, two drivers.
///
/// Seen red 2026-11-15: with `crate::engine::frame_ecs::run(&mut self.state);` added after the
/// walk's `frame_step::run` call, "sim_walk.rs calls the step's stage `frame_ecs` itself".
#[test]
fn the_headless_walk_takes_the_windowed_frames_step() {
    let mut own = Vec::new();
    for rel in ["src/engine/sim_walk.rs", "src/engine/sim_verbs.rs"] {
        let name = rel.trim_start_matches("src/engine/");
        let text = read(rel);
        let code: Vec<&str> = text.lines().map(code_of).collect();
        assert!(
            code.iter().any(|l| l.contains("crate::engine::frame_step::run(")),
            "{name} no longer steps the world through `frame_step::run`: the headless run and the windowed frame would drift apart"
        );
        assert!(code.iter().any(|l| l.contains("EngineState::boot(")), "{name} no longer boots the engine through `EngineState::boot`");
        for (m, kind, _) in STAGES {
            if *kind == Step && code.iter().any(|l| l.contains(&format!("crate::engine::{m}::"))) {
                own.push(format!("{name} calls the step's stage `{m}` itself: step through `frame_step::run` only"));
            }
        }
    }
    assert!(own.is_empty(), "{}", own.join("\n"));
}

/// The verbs' two transports are one (increment 5): the windowed file drop and the headless
/// transport both run the verb table (engine/verbs.rs), both let a rig's held keys go through
/// `rig_walk::tick` after the step, and both write the recorder's frame and done JSON through the
/// same two functions (engine/ipc.rs `recorder_frame`, `recorder_done`), so a judge reads a headless
/// run as it reads a windowed one. And the held keys and the camera park's grace are timed on the
/// step's clock, not the wall clock: rig_walk.rs and frame_probe_hold.rs name no `Instant`.
///
/// Seen red 2026-11-20: with sim_verbs.rs's frames built by a `serde_json::json!` of its own in
/// place of `ipc::recorder_frame(`, "sim_verbs.rs does not write `crate::engine::ipc::recorder_frame(`";
/// and with `use std::time::Instant;` put back in rig_walk.rs, "src/engine/rig_walk.rs names the
/// wall clock (line 39)". Before the verbs moved, the first run failed "the showcase poll no longer
/// runs its verbs through the verb table".
#[test]
fn the_verbs_two_transports_run_one_table_one_release_and_one_recorder() {
    let code_lines = |rel: &str| read(rel).lines().map(|l| code_of(l).to_string()).collect::<Vec<_>>();
    let has = |lines: &[String], w: &str| lines.iter().any(|l| l.contains(w));
    let headless = code_lines("src/engine/sim_verbs.rs");
    for w in ["crate::engine::verbs::apply_request(", "crate::engine::rig_walk::tick(", "crate::engine::ipc::recorder_frame(", "crate::engine::ipc::recorder_done("] {
        assert!(has(&headless, w), "sim_verbs.rs does not write `{w}`: the headless transport would drift from the windowed file drop");
    }
    let ipc = read("src/engine/ipc.rs").replace("\r\n", "\n");
    let body = |name: &str| -> String {
        let start = ipc.find(&format!("pub(crate) fn {name}(")).unwrap_or_else(|| panic!("ipc.rs has no `{name}`"));
        ipc[start..start + ipc[start..].find("\n}\n").expect("its end")].to_string()
    };
    let poll = body("poll_showcase_request");
    assert!(poll.contains("crate::engine::rig_walk::tick(state);"), "the showcase poll no longer lets the held keys go through `rig_walk::tick`");
    assert!(poll.contains("crate::engine::verbs::take(state, &grab,"), "the showcase poll no longer runs its verbs through the verb table");
    let rec = body("poll_remote_players_request");
    for w in ["recorder_frame(state,", "recorder_done(state)"] {
        assert!(rec.contains(w), "the windowed recorder no longer builds its JSON through `{w}`: the two recorders would write different shapes");
    }
    for rel in ["src/engine/rig_walk.rs", "src/engine/frame_probe_hold.rs"] {
        for (n, l) in read(rel).lines().enumerate() {
            let c = code_of(l);
            assert!(!c.contains("Instant") && !c.contains(".elapsed()"), "{rel} names the wall clock (line {}): {}; a rig's timing is the step's clock (state.clock)", n + 1, l.trim());
        }
    }
}

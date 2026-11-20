//! The simulation side stays free of the GPU (the headless sim runner, increment 1, 2026-11-15;
//! docs/design/headless-sim-runner.md). `src/engine/sim_boot.rs` builds the world's simulation at
//! startup and `src/engine/world_sim.rs` builds the home's at world entry, both with no window and
//! no wgpu device, so the windowed game and the headless runner (and the lib test that builds the
//! homestead with no GPU, `world_sim_tests.rs`) run one and the same setup. This lint keeps it so:
//!
//! - no GPU, window or egui type in the sim files' code (comments may say what they are not), and
//!   no `EngineState`, which holds the renderer: a sim function that took it could not be called
//!   without a GPU;
//! - the design's "no `if headless` in sim code" rule, started: the word does not appear in their
//!   code at all (one setup, two drivers; the drivers differ, the sim does not);
//! - the windowed game calls them where it builds the world (lib.rs `resumed`, world_load.rs
//!   `load_world`), and lib.rs registers no system of its own, so there is no second setup to drift.
//!
//! - the frame's ECS stage steps the systems through `frame_sim::systems_slice` and ticks nothing
//!   itself (increment 2), so the soak and the windowed game run one step.
//!
//! Seen red 2026-11-15: with `use crate::engine::state::EngineState;` added to world_sim.rs it
//! failed "src/engine/world_sim.rs line 20 names `EngineState`"; with `if headless {}` added to
//! sim_boot.rs's `boot`, "src/engine/sim_boot.rs line 65 names `headless`". Increment 2's checks,
//! the same day: an `EngineState` field planted in frame_sim.rs's `SimParts` failed "frame_sim.rs
//! line 24 names `EngineState`", and a second `world_hold::tick(` planted in frame_ecs.rs failed
//! "frame_ecs.rs calls `world_hold::tick(` itself". Increment 4's (2026-11-15): with sim_home.rs's
//! `sim_boot::seed_home_storage(` call taken out, "sim_home.rs `enter` no longer calls
//! `sim_boot::seed_home_storage(`: the headless homestead would start unlike the windowed game".

use std::fs;
use std::path::PathBuf;

/// The simulation side's files: built with no GPU.
/// Increment 2 (2026-11-15) added the systems slice, `frame_sim.rs`, one step of the systems on
/// the sim's parts, and `sim_home.rs` and `sim_soak.rs`, the homestead entered and stepped and
/// the `--sim soak` run. Increment 3 (the same day) added the bridges' slice and the `FrameClock`
/// to `frame_sim.rs`, the GUI's requests it carries, `frame_sim_requests.rs`, and a person's
/// routine for the soak, `sim_routine.rs`.
const SIM_FILES: &[&str] = &[
    "src/engine/sim_boot.rs",
    "src/engine/world_sim.rs",
    "src/engine/world_sim_tests.rs",
    "src/engine/frame_sim.rs",
    "src/engine/frame_sim_tests.rs",
    "src/engine/frame_sim_requests.rs",
    "src/engine/sim_home.rs",
    "src/engine/sim_soak.rs",
    "src/engine/sim_routine.rs",
];

/// Words that mean the GPU, the window, the GUI's painter, or the state that holds them. Matched
/// as whole identifiers in code, comments stripped.
const FORBIDDEN: &[&str] = &[
    "wgpu",
    "winit",
    "egui",
    "egui_wgpu",
    "egui_winit",
    "renderer",
    "Renderer",
    "Window",
    "EngineState",
    "headless",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// A line's code: everything before a `//` comment (these files hold no `//` inside a string).
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// The identifiers in a piece of code.
fn idents(code: &str) -> Vec<&str> {
    code.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect()
}

#[test]
fn the_sim_side_names_no_gpu_type() {
    let mut found = Vec::new();
    for rel in SIM_FILES {
        for (n, line) in read(rel).lines().enumerate() {
            for word in idents(code_of(line)) {
                if FORBIDDEN.contains(&word) {
                    found.push(format!("{rel} line {} names `{word}`: {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "The simulation side must build with no GPU, window or egui (docs/design/headless-sim-runner.md): \
         move the drawing to world_load.rs or lib.rs, or pass the sim the plain data it needs.\n  {}",
        found.join("\n  ")
    );
}

#[test]
fn the_windowed_game_builds_its_world_through_the_sim_side() {
    // The windowed app boots the engine through `EngineState::boot` (increment 4, 2026-11-15:
    // engine/engine_boot.rs, moved out of lib.rs `resumed`), the boot a headless run takes too.
    let app = read("src/lib.rs");
    assert!(app.contains("EngineState::boot("), "lib.rs `resumed` no longer boots the engine through `EngineState::boot`: the windowed game and a headless run would boot differently");
    let lib = read("src/engine/engine_boot.rs");
    // Home storage at the start too (increment 4): both drivers seed it through the sim side.
    let home = read("src/engine/sim_home.rs");
    for call in ["sim_boot::boot(", "sim_boot::starter_vehicles(", "sim_boot::apply_home_save(", "sim_boot::seed_home_storage("] {
        assert!(lib.contains(call), "engine_boot.rs `EngineState::boot` no longer calls `{call}`: the windowed game's setup and the sim's would drift apart");
        let short = call.trim_start_matches("sim_boot::");
        assert!(home.contains(short), "sim_home.rs `enter` no longer calls `{call}`: the headless homestead would start unlike the windowed game");
    }
    let registers = lib.lines().chain(app.lines()).filter(|l| code_of(l).contains(".register(")).count();
    assert_eq!(registers, 0, "lib.rs or engine_boot.rs registers {registers} systems of its own: every system is registered in src/engine/sim_boot.rs");
    let world_load = read("src/engine/world_load.rs");
    assert!(
        world_load.contains("world_sim::build("),
        "world_load.rs `load_world` no longer builds the home's simulation through `world_sim::build`"
    );
    // The windowed frame steps the systems through the slice (increment 2), and the slice alone
    // ticks them: no second copy of the step in the frame's ECS stage.
    let frame_ecs = read("src/engine/frame_ecs.rs");
    assert!(
        frame_ecs.lines().any(|l| code_of(l).contains("frame_sim::systems_slice(")),
        "frame_ecs.rs no longer steps the systems through `frame_sim::systems_slice`: the windowed game and the soak would drift apart"
    );
    for word in ["world_hold::tick(", ".tick("] {
        assert!(
            !frame_ecs.lines().any(|l| code_of(l).contains(word)),
            "frame_ecs.rs calls `{word}` itself: the systems are ticked inside `frame_sim::systems_slice` only"
        );
    }
}

/// The step's files that take their time from the `FrameClock` (increment 3): no wall clock in
/// their code. (sim_soak.rs times its own run with `Instant`, the wall time it reports; that is the
/// driver's measurement, not the sim's time.) Increment 5 (2026-11-20) added the verb table, the
/// rigs' held keys and the camera park's grace: a rig's timing is the step's clock.
const CLOCKED_FILES: &[&str] = &[
    "src/engine/frame_sim.rs",
    "src/engine/frame_sim_requests.rs",
    "src/engine/sim_home.rs",
    "src/engine/sim_routine.rs",
    "src/engine/verbs.rs",
    "src/engine/rig_walk.rs",
    "src/engine/frame_probe_hold.rs",
];

/// The verb table (increment 5, 2026-11-20): the verbs both transports run, the windowed file drop
/// and the headless `--sim verbs`. It takes the engine (`EngineState`, whose GPU is optional since
/// increment 4), but never the GPU, the window or the GUI's painter: a verb that needed them could
/// not run headless, and the two transports would drift.
const VERB_FILES: &[&str] = &["src/engine/verbs.rs"];

/// Seen red 2026-11-20: with `let _ = egui::Context::default();` planted in verbs.rs's `take`,
/// "src/engine/verbs.rs line 253 names `egui`"; and with `use std::time::Instant;` put back in
/// rig_walk.rs, the clocked check "src/engine/rig_walk.rs line 39 reads the wall clock (`Instant`)".
#[test]
fn the_verb_table_names_no_gpu_type() {
    let mut found = Vec::new();
    for rel in VERB_FILES {
        for (n, line) in read(rel).lines().enumerate() {
            for word in idents(code_of(line)) {
                if word != "EngineState" && FORBIDDEN.contains(&word) {
                    found.push(format!("{rel} line {} names `{word}`: {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(found.is_empty(), "The verb table runs with no GPU, window or egui (docs/design/headless-sim-runner.md, increment 5):\n  {}", found.join("\n  "));
}

/// Increment 3: the windowed frame's bridges and the soak run one bridges' slice, before the
/// systems' slice, on one clock.
///
/// Seen red 2026-11-15: with `survival_env::publish(` called from frame_bridges.rs beside the slice,
/// "frame_bridges.rs publishes `survival_env::publish(` itself"; with `std::time::Instant::now()`
/// planted in sim_home.rs's `step`, "src/engine/sim_home.rs line 95 reads the wall clock
/// (`Instant`)"; with sim_home.rs's `bridges_slice(` call moved after `systems_slice(`, "sim_home.rs
/// steps the systems before the bridges".
#[test]
fn the_frame_and_the_soak_run_one_bridges_slice_on_one_clock() {
    let bridges = read("src/engine/frame_bridges.rs");
    assert!(
        bridges.lines().any(|l| code_of(l).contains("frame_sim::bridges_slice(")),
        "frame_bridges.rs no longer feeds the world through `frame_sim::bridges_slice`: the windowed game and the soak would drift apart"
    );
    // The world-feeding half is the slice's alone: none of it is done beside it in the frame.
    let npc = read("src/engine/frame_bridges_npc.rs");
    for word in ["survival_env::publish(", "publish_body_environment(", "drone::bridge_gui(", "\"consume_request\"", "\"drink_request\"", "\"plant_request\"", "\"craft_request\"", "\"inventory_transfer_ops\""] {
        for (name, text) in [("frame_bridges.rs", &bridges), ("frame_bridges_npc.rs", &npc)] {
            assert!(!text.lines().any(|l| code_of(l).contains(word)), "{name} publishes `{word}` itself: the bridges' world-feeding half runs in `frame_sim::bridges_slice` only");
        }
    }
    // The soak steps the bridges, then the systems, as the frame's stages run them.
    let home = read("src/engine/sim_home.rs");
    let at = |word: &str| home.lines().position(|l| code_of(l).contains(word));
    match (at("bridges_slice("), at("systems_slice(")) {
        (Some(b), Some(s)) => assert!(b < s, "sim_home.rs steps the systems before the bridges"),
        other => panic!("sim_home.rs does not run both slices: {other:?}"),
    }
    // One clock: the windowed frame advances it, the stages read it, and the step's files read no
    // wall clock of their own.
    let clock = read("src/engine/frame_clock.rs");
    assert!(clock.lines().any(|l| code_of(l).contains("state.clock.advance(")), "frame_clock.rs no longer advances the frame's `FrameClock`");
    let mut found = Vec::new();
    for rel in CLOCKED_FILES {
        for (n, line) in read(rel).lines().enumerate() {
            for word in idents(code_of(line)) {
                if word == "Instant" || word == "SystemTime" {
                    found.push(format!("{rel} line {} reads the wall clock (`{word}`): {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(found.is_empty(), "The step takes its time from the `FrameClock` (frame_sim.rs):\n  {}", found.join("\n  "));
}

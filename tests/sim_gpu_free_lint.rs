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
//! Seen red 2026-11-15: with `use crate::engine::state::EngineState;` added to world_sim.rs it
//! failed "src/engine/world_sim.rs line 20 names `EngineState`"; with `if headless {}` added to
//! sim_boot.rs's `boot`, "src/engine/sim_boot.rs line 65 names `headless`".

use std::fs;
use std::path::PathBuf;

/// The simulation side's files: built with no GPU.
const SIM_FILES: &[&str] = &["src/engine/sim_boot.rs", "src/engine/world_sim.rs", "src/engine/world_sim_tests.rs"];

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
    let lib = read("src/lib.rs");
    for call in ["sim_boot::boot(", "sim_boot::starter_vehicles(", "sim_boot::apply_home_save("] {
        assert!(lib.contains(call), "lib.rs `resumed` no longer calls `{call}`: the windowed game's setup and the sim's would drift apart");
    }
    let registers = lib.lines().filter(|l| code_of(l).contains(".register(")).count();
    assert_eq!(registers, 0, "lib.rs registers {registers} systems of its own: every system is registered in src/engine/sim_boot.rs");
    let world_load = read("src/engine/world_load.rs");
    assert!(
        world_load.contains("world_sim::build("),
        "world_load.rs `load_world` no longer builds the home's simulation through `world_sim::build`"
    );
}

//! The simulation draws its random numbers from the run's seed (BUG-333, 2026-11-20;
//! docs/design/headless-sim-runner.md, "Safety"). A run that sets its seed (`--sim soak`, the step
//! ladder, the lib tests that step one span two ways) must decide the same thing every time; a draw
//! from the thread's shared generator (`rand::random`, `rand::rng()`) is seeded afresh from the
//! operating system on every thread, so no seed reaches it. The routine soak's person fell ill from
//! raw rice in some runs and not others that way, and three runs of one exe ended day 6 at
//! hydration 85.3, 94.8 and 93.1.
//!
//! So in the systems' code (crates/humanity-systems/src, its test modules aside) and the engine's
//! simulation files:
//! - no draw from the thread's generator: each system draws through its own
//!   `humanity_systems::sim_seed::Stream`, salted per system;
//! - a generator started from the operating system's entropy (`from_os_rng`) only in a file that
//!   restarts it from the run's seed (`sim_seed::for_system`), as WeatherSystem and DisasterSystem
//!   do, or in sim_seed.rs itself (the windowed game's start, which sets no seed).
//!
//! Test modules are left out: a file's code is read up to its `#[cfg(test)]` module (these files
//! keep it at the end), and files named `*tests.rs` are skipped.
//!
//! Seen red 2026-11-20 against the code before the fix: "12 random draws the run's seed does not
//! reach", among them "crates/humanity-systems/src/food.rs line 751 draws from the thread's shared
//! generator (`rand::random`)" (the illness roll) and the same for food.rs's wounds, six in
//! farming/picking.rs, crafting/mod.rs, ai/mod.rs, combat/mod.rs and fire.rs. Its first cut-off
//! (read up to the first `#[cfg(test)]` module) missed crafting/mod.rs's draw, which sits after a
//! test module at line 187; the test modules are now skipped, not read up to.

use std::fs;
use std::path::{Path, PathBuf};

/// The engine's simulation files (tests/sim_gpu_free_lint.rs's list, its tests aside): the world
/// entry, the step and the soak.
const ENGINE_SIM_FILES: &[&str] = &[
    "src/engine/sim_boot.rs",
    "src/engine/world_sim.rs",
    "src/engine/frame_sim.rs",
    "src/engine/frame_sim_requests.rs",
    "src/engine/sim_home.rs",
    "src/engine/sim_soak.rs",
    "src/engine/sim_routine.rs",
];

/// Draws from the thread's shared generator, which no seed reaches.
const THREAD_DRAWS: &[&str] = &["rand::random", "rand::rng(", "thread_rng", "ThreadRng", "from_entropy"];

/// A generator started from the operating system's entropy.
const ENTROPY_START: &str = "from_os_rng";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A line's code: everything before a `//` comment.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Every `.rs` file under `dir`, relative to the repo root, `/`-separated.
fn rust_files(dir: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            let rel = p.strip_prefix(root()).expect("under the repo").to_string_lossy().replace('\\', "/");
            out.push(rel);
        }
    }
}

/// The file's lines of code, numbered from 1, its inline test modules left out: from a
/// `#[cfg(test)]` over `mod name {` to the `}` at that `mod` line's indent (a test module can sit
/// mid-file, crafting/mod.rs's recipe tests do). A `mod name;` declaration is that file's business.
fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut out = Vec::new();
    let mut skip_to_indent: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(at) = skip_to_indent {
            if line.trim() == "}" && indent(line) == at {
                skip_to_indent = None;
            }
            continue;
        }
        if line.trim() == "#[cfg(test)]" {
            let next = lines[i + 1..].iter().find(|l| !l.trim().is_empty() && !l.trim().starts_with("#["));
            if let Some(m) = next {
                let code = code_of(m).trim();
                let is_mod = code.starts_with("mod ") || code.starts_with("pub mod ") || code.starts_with("pub(crate) mod ");
                if is_mod && code.ends_with('{') {
                    skip_to_indent = Some(indent(m));
                    continue;
                }
            }
        }
        out.push((i + 1, code_of(line)));
    }
    out
}

/// The files this lint reads.
fn sim_files() -> Vec<String> {
    let mut files = Vec::new();
    rust_files(&root().join("crates/humanity-systems/src"), &mut files);
    files.retain(|f| !f.ends_with("tests.rs"));
    files.extend(ENGINE_SIM_FILES.iter().map(|s| s.to_string()));
    files
}

#[test]
fn the_simulation_draws_only_from_the_runs_seed() {
    let files = sim_files();
    assert!(files.len() > 50, "found only {} simulation files: the walk is broken", files.len());
    assert!(files.iter().any(|f| f.ends_with("humanity-systems/src/food.rs")), "food.rs is not among the files read");
    let mut found = Vec::new();
    for rel in &files {
        let text = fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let restarts_from_seed = rel.ends_with("/sim_seed.rs") || text.contains("sim_seed::for_system");
        for (n, code) in code_lines(&text) {
            for word in THREAD_DRAWS {
                if code.contains(word) {
                    found.push(format!(
                        "{rel} line {n} draws from the thread's shared generator (`{word}`), which no run's seed reaches: \
                         draw through the system's `sim_seed::Stream` instead: {}",
                        code.trim()
                    ));
                }
            }
            if code.contains(ENTROPY_START) && !restarts_from_seed {
                found.push(format!(
                    "{rel} line {n} starts a generator from the entropy and never from the run's seed \
                     (no `sim_seed::for_system` in the file): use a `sim_seed::Stream`: {}",
                    code.trim()
                ));
            }
        }
    }
    assert!(found.is_empty(), "{} random draws the run's seed does not reach:\n  {}", found.len(), found.join("\n  "));
}

/// The test modules are left out, and only they: a helper behind `#[cfg(test)]`, a `mod x;`
/// declaration and the code after a mid-file test module are read.
#[test]
fn the_lint_leaves_out_the_test_modules_only() {
    let text = "#[cfg(test)]\nmod away_tests;\nfn a() {}\n#[cfg(test)]\nfn helper() {}\nfn b() { rand::random::<f32>(); }\n#[cfg(test)]\nmod mid {\n    fn c() { rand::random::<f32>(); }\n    fn inner() {\n    }\n}\nfn d() {}\n#[cfg(test)]\nmod tests {\n    fn e() {}\n}\n";
    let lines = code_lines(text);
    let read = |name: &str| lines.iter().any(|(_, l)| l.contains(name));
    assert!(read("fn a()") && read("fn b()"), "code after a mod declaration or a cfg(test) helper was left out: {lines:?}");
    assert!(read("fn d()"), "code after a mid-file test module was left out: {lines:?}");
    assert!(!read("fn c()") && !read("fn e()"), "a test module was read: {lines:?}");
}

//! Tests reach the GPU only through src/renderer/test_gpu.rs (2026-11-14), and that helper asks
//! crates/humanity-core/src/test_gpu.rs first (2026-11-01).
//!
//! RULE 1 (2026-11-14): outside the helper and the renderer's own production sites, no Rust file
//! makes a `wgpu::Instance` (`Instance::default()`, `Instance::new(`) or calls `request_adapter(`
//! or `request_device(`. On 2026-11-14 the full lib-test run froze in run after run: every GPU
//! test made its own `Instance::default()` (in a debug build every backend, OpenGL and Vulkan
//! included) and dropped it, side by side, and a thread dump showed NVIDIA's OpenGL and Vulkan
//! driver deadlocked against the loader lock (src/renderer/test_gpu.rs tells it). The helper makes
//! ONE instance and adapter on the game's backend (DX12 on Windows), hands each test its device in
//! turn, and turns a GPU that stops answering into a failure naming the test. A test that opens
//! its own instance or device brings the deadlock back, so this fails on one.
//!
//! RULE 2 (2026-11-01): the helper asks `humanity_core::test_gpu::may_use()` before anything
//! touches the GPU: while one of the operator's GPU-heavy games runs, scripts/lib-tests.js sets
//! HUMANITY_TEST_GPU_BUSY and the GPU tests stand down (they hung the run for two hours beside
//! helldivers2.exe on 2026-11-01). Any other instance (production excepted) must still ask first in
//! its own function.
//!
//! The renderer's own device, the one production place that makes an instance and opens a
//! device, is listed below with its counts, so a new production site is added on purpose, with its
//! reason, and a removed one lowers the count.
//!
//! Seen red (2026-11-14): on main's tree before the helper (11 files: bc7, film_blur_tests,
//! gpu_costs, lattice_noise, particles_gpu, room_probes_gpu, scene_target_tests, shadow_gpu_tests,
//! shadow_transmit_tests, stream_capture, gui/ui_snapshots), and with a
//! `wgpu::Instance::new(..)` and `pollster::block_on(instance.request_adapter(&Default::default()))`
//! planted in a test of src/renderer/film_blur_tests.rs: rule 1 named "film_blur_tests.rs:238
//! Instance::new(" and "film_blur_tests.rs:239 request_adapter(", rule 2 "film_blur_tests.rs:238".
//!
//! Std-only, compiled standalone like the other lints (`just lints`):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/gpu_test_gate_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

/// Production code that opens the GPU: (path ending, instances, request_adapter calls,
/// request_device calls, why). Tests never go here.
const PRODUCTION: &[(&str, usize, usize, usize, &str)] = &[
    (
        "renderer/mod.rs",
        2,
        0,
        0,
        "the renderer's instance for the desktop window (Renderer::native_pages_gpu) and for the web canvas (new_wasm)",
    ),
    (
        "renderer/pages_gpu.rs",
        0,
        1,
        1,
        "the one adapter and device both instances use (PagesGpu::new, 2026-11-20: the pages' GPU, which the world renderer is built on)",
    ),
];

/// The one file tests open the GPU through.
const HELPER: &str = "src/renderer/test_gpu.rs";

/// The calls that create an instance.
const INSTANCE: &[&str] = &["Instance::default()", "Instance::new("];
/// The calls that open an adapter or a device.
const OPEN: &[&str] = &["request_adapter(", "request_device("];
/// The question a test asks first.
const GATE: &str = "test_gpu::may_use()";
/// How far above the instance the question may be, at most.
const LOOK_BACK: usize = 40;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() != "target" {
                rust_files(&path, out);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// The 0-based lines that hold one of `needles` in code (not in a `//` comment), each with the
/// needle found; a needle preceded by part of a longer name (`PatchInstance::new(`) is not one.
fn needle_lines(text: &str, needles: &[&'static str]) -> Vec<(usize, &'static str)> {
    let mut lines = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        let hit = needles.iter().find(|n| {
            code.match_indices(*n).any(|(at, _)| {
                let before = code[..at].chars().next_back();
                !matches!(before, Some(c) if c.is_alphanumeric() || c == '_')
            })
        });
        if let Some(n) = hit {
            lines.push((i, *n));
        }
    }
    lines
}

/// The 0-based lines that create an instance.
fn instance_lines(text: &str) -> Vec<usize> {
    needle_lines(text, INSTANCE).into_iter().map(|(i, _)| i).collect()
}

/// Does the function holding `line` ask the gate above it? Walks up to the function's own `fn`.
fn asks_first(lines: &[&str], line: usize) -> bool {
    for k in (line.saturating_sub(LOOK_BACK)..line).rev() {
        let l = lines[k];
        if l.contains(GATE) {
            return true;
        }
        let t = l.trim_start();
        let starts_fn = ["fn ", "pub fn ", "pub(crate) fn ", "pub(super) fn ", "async fn ", "pub(crate) async fn "]
            .iter()
            .any(|p| t.starts_with(p));
        if starts_fn {
            return false;
        }
    }
    false
}

/// What a scan of the source finds.
#[derive(Default)]
struct Scan {
    /// (file:line) of an instance with no gate above it (rule 2), outside production and the helper.
    unasked: Vec<String>,
    /// (file:line needle) of an instance or an open outside production and the helper (rule 1).
    outside_helper: Vec<String>,
    /// Per production file: (rel, instances, adapters, devices).
    production: Vec<(String, usize, usize, usize)>,
    /// The helper's text, when found.
    helper: Option<String>,
}

fn scan(repo: &Path) -> Scan {
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    rust_files(&repo.join("crates"), &mut files);
    let mut out = Scan::default();
    for f in files {
        let Ok(text) = fs::read_to_string(&f) else { continue };
        let rel = f.strip_prefix(repo).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        if rel == HELPER {
            out.helper = Some(text);
            continue;
        }
        let instances = instance_lines(&text);
        let opens = needle_lines(&text, OPEN);
        if instances.is_empty() && opens.is_empty() {
            continue;
        }
        if PRODUCTION.iter().any(|(end, ..)| rel.ends_with(end)) {
            let adapters = opens.iter().filter(|(_, n)| *n == "request_adapter(").count();
            out.production.push((rel, instances.len(), adapters, opens.len() - adapters));
            continue;
        }
        for (l, n) in needle_lines(&text, INSTANCE).into_iter().chain(opens) {
            out.outside_helper.push(format!("{rel}:{} {n}", l + 1));
        }
        let lines: Vec<&str> = text.lines().collect();
        for l in instances {
            if !asks_first(&lines, l) {
                out.unasked.push(format!("{rel}:{}", l + 1));
            }
        }
    }
    out
}

#[test]
fn tests_open_the_gpu_only_through_the_helper() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let s = scan(repo);
    assert!(
        s.outside_helper.is_empty(),
        "\n\nA wgpu instance, adapter or device is opened outside {HELPER}:\n  {}\n\n\
         Tests that made and dropped their own instances side by side deadlocked the GPU driver and froze \
         the whole lib-test run on 2026-11-14 ({HELPER} says how). Ask the helper instead:\n\
         \x20   let Some(gpu) = crate::renderer::test_gpu::open(Ask::new(\"my test\")) else {{ return }};\n\
         \x20   let (device, queue) = (&gpu.device, &gpu.queue);\n\
         (`Ask::features` for features the test needs, `Ask::adapter_resolution` for big textures.)\n\
         If this is production code (not a test), add its file to PRODUCTION in tests/gpu_test_gate_lint.rs \
         with the reason.\n",
        s.outside_helper.join("\n  ")
    );
}

#[test]
fn the_helper_asks_test_gpu_first() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let s = scan(repo);
    let text = s.helper.unwrap_or_else(|| panic!("{HELPER} is missing: the one way a test gets a GPU device"));
    let open_at = text.find("pub fn open(").unwrap_or_else(|| panic!("{HELPER} has no `pub fn open(`"));
    let body = &text[open_at..];
    let gate = body.find(GATE).unwrap_or(usize::MAX);
    let first_gpu = ["shared(", "take_turn("].iter().filter_map(|n| body.find(n)).min().unwrap_or(0);
    assert!(
        gate < first_gpu,
        "{HELPER}'s `open` must ask `humanity_core::test_gpu::may_use()` before it takes the turn or touches the GPU \
         (the operator's GPU game comes first: crates/humanity-core/src/test_gpu.rs)"
    );
    let code: String = text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
    assert!(
        !code.contains("Instance::default()") && !code.contains("Backends::GL") && !code.contains("Backends::all()"),
        "{HELPER} must open only the game's backends (DX12 on Windows), never OpenGL: its driver deadlocked the test program"
    );
}

#[test]
fn every_other_instance_asks_test_gpu_first() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let s = scan(repo);
    assert!(
        s.unasked.is_empty(),
        "\n\nA wgpu instance is created with no `humanity_core::test_gpu::may_use()` above it in the same function:\n  {}\n\n\
         While the operator's GPU game runs, a test that opens a device can hang the WHOLE lib-test run, \
         silently (crates/humanity-core/src/test_gpu.rs). Use {HELPER}, which asks first.\n",
        s.unasked.join("\n  ")
    );
}

#[test]
fn the_production_sites_are_exactly_the_listed_ones() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let s = scan(repo);
    for (end, instances, adapters, devices, why) in PRODUCTION {
        let found = s.production.iter().filter(|(rel, ..)| rel.ends_with(end)).fold((0, 0, 0), |a, (_, i, ad, d)| (a.0 + i, a.1 + ad, a.2 + d));
        assert_eq!(
            found,
            (*instances, *adapters, *devices),
            "{end} makes (instances, request_adapter, request_device) {found:?}, the list says {:?} ({why}). A new one in a \
             test there goes through {HELPER}; a removed one lowers the count in tests/gpu_test_gate_lint.rs.",
            (instances, adapters, devices)
        );
    }
}

#[test]
fn the_finder_sees_each_call_and_not_a_longer_name() {
    let text = "fn a() {\n    let i = wgpu::Instance::default();\n}\nfn b() {\n    PatchInstance::new([0]);\n    // wgpu::Instance::new( in a comment\n}\nfn c() {\n    if !humanity_core::test_gpu::may_use() {\n        return;\n    }\n    let i = wgpu::Instance::new(&d);\n}\n";
    let found = instance_lines(text);
    assert_eq!(found, vec![1, 11], "{found:?}");
    let lines: Vec<&str> = text.lines().collect();
    assert!(!asks_first(&lines, 1), "fn a never asks");
    assert!(asks_first(&lines, 11), "fn c asks first");
    // The question asked in the function ABOVE does not count for the next one.
    let two = "fn a() {\n    if !humanity_core::test_gpu::may_use() { return; }\n}\nfn b() {\n    let i = wgpu::Instance::default();\n}\n";
    let lines: Vec<&str> = two.lines().collect();
    assert!(!asks_first(&lines, 4));
    // Opening an adapter or a device, by method or by path; not in a comment, not a longer name.
    let opens = "let a = pollster::block_on(instance.request_adapter(&o));\n// adapter.request_device( in a comment\nlet d = wgpu::Adapter::request_device(&a, &desc, None);\nmy_request_device(x);\n";
    assert_eq!(needle_lines(opens, OPEN), vec![(0, "request_adapter("), (2, "request_device(")]);
}

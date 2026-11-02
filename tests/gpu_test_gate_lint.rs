//! Every test that opens a GPU device asks crates/humanity-core/src/test_gpu.rs first (2026-11-01).
//!
//! While one of the operator's GPU-heavy games runs, a test that opens a device can hang the
//! whole lib-test run SILENTLY (two hours beside helldivers2.exe on 2026-11-01, no 60-second
//! warning: crates/humanity-core/src/test_gpu.rs says why). scripts/lib-tests.js then sets HUMANITY_TEST_GPU_BUSY,
//! and `humanity_core::test_gpu::may_use()` answers no, but only for a test that asks BEFORE it creates
//! a `wgpu::Instance` (creating the instance already loads the driver). This lint finds every
//! `Instance::default()` and `Instance::new(` in the Rust source and fails on one with no
//! `test_gpu::may_use()` above it in the same function. The renderer's own device, the one
//! production place that makes an instance, is listed below with its count, so a new production
//! site is added on purpose, with its reason, and a removed one lowers the count.
//!
//! Std-only, compiled standalone like the other lints (`just lints`):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/gpu_test_gate_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

/// Production code that makes a wgpu instance: (path ending, how many, why). Tests never go here.
const PRODUCTION: &[(&str, usize, &str)] = &[(
    "renderer/mod.rs",
    2,
    "the renderer's instance for the desktop window (Renderer::new) and for the web canvas (new_wasm)",
)];

/// The calls that create an instance.
const NEEDLES: &[&str] = &["Instance::default()", "Instance::new("];
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

/// The 0-based lines that create an instance (a needle not preceded by part of a longer name,
/// so `PatchInstance::new(` is not one).
fn instance_lines(text: &str) -> Vec<usize> {
    let mut lines = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        let hit = NEEDLES.iter().any(|n| {
            code.match_indices(n).any(|(at, _)| {
                let before = code[..at].chars().next_back();
                !matches!(before, Some(c) if c.is_alphanumeric() || c == '_')
            })
        });
        if hit {
            lines.push(i);
        }
    }
    lines
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

/// (file, 1-based line) of each instance a test makes without asking, and the production
/// counts as found.
fn scan(repo: &Path) -> (Vec<String>, Vec<(String, usize)>) {
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    rust_files(&repo.join("crates"), &mut files);
    let mut unasked = Vec::new();
    let mut production = Vec::new();
    for f in files {
        let Ok(text) = fs::read_to_string(&f) else { continue };
        let found = instance_lines(&text);
        if found.is_empty() {
            continue;
        }
        let rel = f.strip_prefix(repo).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        if PRODUCTION.iter().any(|(end, _, _)| rel.ends_with(end)) {
            production.push((rel, found.len()));
            continue;
        }
        let lines: Vec<&str> = text.lines().collect();
        for l in found {
            if !asks_first(&lines, l) {
                unasked.push(format!("{rel}:{}", l + 1));
            }
        }
    }
    (unasked, production)
}

#[test]
fn every_test_that_opens_a_gpu_device_asks_test_gpu_first() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (unasked, _) = scan(repo);
    assert!(
        unasked.is_empty(),
        "\n\nA wgpu instance is created with no `humanity_core::test_gpu::may_use()` above it in the same function:\n  {}\n\n\
         While the operator's GPU game runs, a test that opens a device can hang the WHOLE lib-test run, \
         silently (crates/humanity-core/src/test_gpu.rs). Ask first, before the instance:\n\
         \x20   if !humanity_core::test_gpu::may_use() {{ return; }}   (or `return None;` in a device helper)\n\
         If this is production code (not a test), add its file to PRODUCTION in tests/gpu_test_gate_lint.rs \
         with the reason.\n",
        unasked.join("\n  ")
    );
}

#[test]
fn the_production_instances_are_exactly_the_listed_ones() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (_, production) = scan(repo);
    for (end, count, why) in PRODUCTION {
        let found: usize = production.iter().filter(|(rel, _)| rel.ends_with(end)).map(|(_, n)| n).sum();
        assert_eq!(
            found, *count,
            "{end} makes {found} wgpu instance(s), the list says {count} ({why}). A new one in a test there needs \
             `test_gpu::may_use()` and a test file of its own; a removed one lowers the count in tests/gpu_test_gate_lint.rs."
        );
    }
}

#[test]
fn the_finder_sees_a_bare_instance_and_not_a_longer_name() {
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
}

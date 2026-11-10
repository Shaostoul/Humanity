//! NO AI IN THE APP lint (2026-11-09; docs/design/embeddable-ai.md, section 12.3; rung 1).
//!
//! The operator, 2026-11-09: "AI needs to be optional, not required", and the model runtime is a
//! separate download too ("Agreed."): a person who never chooses AI has none of it on their machine.
//! So HumanityOS.exe carries no model and no model runtime; those live in the separate AI engine
//! (crates/humanity-ai-engine/, its own workspace, never a dependency of the app) and in downloads the
//! person chooses. This lint fails when that stops being true:
//!
//! 1. **The app's manifests**: the root Cargo.toml and every member of its `[workspace]` name no model
//!    runtime in any dependency table (llama.cpp, ggml, audio.cpp, tract, ONNX Runtime and its `ort`
//!    binding, candle, burn), the AI engine is not a member, and nothing depends on it.
//! 2. **The app's whole dependency tree**: the root Cargo.lock, which lists every crate any build of the
//!    workspace can compile, transitive ones included, names none of them either. (This works because
//!    the engine is outside the workspace: its runtimes go in its own Cargo.lock, never this one.)
//! 3. **What ships beside the app**: data/ and assets/ hold no model file (`.gguf`, `.onnx`,
//!    `.safetensors`, `.ggml`).
//!
//! Each scanner is run on a synthetic input first (a positive control), and the walks must find files,
//! so a reader that stopped seeing anything cannot pass by accident.
//!
//! SEEN RED 2026-11-09 before it was added to `just lints`, with a scratch dependency
//! `llama-cpp-2 = "0.1"` in the root Cargo.toml's [dependencies] and a scratch data/ai/scratch.gguf
//! (both removed after):
//!   the_app_names_no_model_runtime: "a model runtime in the app's manifests: Cargo.toml: llama-cpp-2"
//!   nothing_beside_the_app_is_a_model_file: "model files beside the app: data/ai/scratch.gguf"
//! (the lock half was seen red on its positive control, a synthetic lock naming `ort-sys`).
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/no_ai_in_app_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The AI engine's folder: a workspace of its own, never a member and never a dependency.
const ENGINE_DIR: &str = "crates/humanity-ai-engine";
const ENGINE_NAME: &str = "humanity-ai-engine";

/// Is this crate name a model runtime (or a binding to one)? Names compared with `_` as `-`.
fn is_model_runtime(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase().replace('_', "-");
    let is_or_starts = |base: &str| n == base || n.starts_with(&format!("{base}-"));
    n.contains("llama")
        || n.contains("ggml")
        || n.contains("audiocpp")
        || n.contains("audio-cpp")
        || n.contains("onnx")
        || is_or_starts("ort")
        || is_or_starts("tract")
        || is_or_starts("candle")
        || is_or_starts("burn")
}

/// The dependency names a Cargo.toml declares, in every dependency table (`[dependencies]`,
/// `[dev-dependencies]`, `[build-dependencies]`, their `target.'..'.` forms, `[workspace.dependencies]`,
/// and a dependency's own `[dependencies.x]` table), with the package a rename points at.
fn manifest_dependencies(toml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut table = String::new();
    for raw in toml.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            table = line.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            // `[dependencies.llama-cpp-2]`: the name is the table's last part.
            for kind in ["dependencies.", "dev-dependencies.", "build-dependencies."] {
                if let Some(i) = table.find(kind) {
                    out.push(table[i + kind.len()..].trim_matches('"').to_string());
                }
            }
            continue;
        }
        let in_deps = table.ends_with("dependencies");
        if !in_deps {
            if line.starts_with("package") && table.contains("dependencies.") {
                if let Some(v) = line.split('=').nth(1) {
                    out.push(v.trim().trim_matches('"').to_string());
                }
            }
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            let name = name.trim().trim_matches('"');
            if name.is_empty() || name.contains(' ') {
                continue;
            }
            out.push(name.to_string());
            // `foo = { package = "llama-cpp-2", ... }`: the real crate.
            if let Some(i) = value.find("package") {
                let rest = &value[i + "package".len()..];
                if let Some(q) = rest.split('"').nth(1) {
                    out.push(q.to_string());
                }
            }
        }
    }
    out
}

/// The `[workspace] members` of a Cargo.toml.
fn workspace_members(toml: &str) -> Vec<String> {
    let mut table = String::new();
    let mut collecting = false;
    let mut body = String::new();
    for raw in toml.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if !collecting && line.starts_with('[') && line.ends_with(']') {
            table = line.to_string();
            continue;
        }
        if table == "[workspace]" && !collecting && line.starts_with("members") {
            collecting = true;
        }
        if collecting {
            body.push_str(line);
            if line.contains(']') {
                break;
            }
        }
    }
    body.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

/// The package names a Cargo.lock lists.
fn lock_packages(lock: &str) -> Vec<String> {
    lock.lines().filter_map(|l| l.trim().strip_prefix("name = \"").and_then(|r| r.strip_suffix('"'))).map(str::to_string).collect()
}

/// Every file under `dir` (links not followed: a rig's data junction is not the repo's own).
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(meta) = fs::symlink_metadata(&p) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn is_model_file(p: &Path) -> bool {
    let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    matches!(ext.as_str(), "gguf" | "onnx" | "safetensors" | "ggml")
}

fn rel(p: &Path) -> String {
    p.strip_prefix(repo()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The scanners can fail: each finds what it must in a synthetic input, and passes over what it must.
#[test]
fn the_scanners_see_what_they_must() {
    for name in ["llama-cpp-2", "llama_cpp_sys_2", "ggml-sys", "audiocpp", "tract-onnx", "tract", "ort", "ort-sys", "onnxruntime", "candle-core", "burn", "burn-ndarray"] {
        assert!(is_model_runtime(name), "{name} is a model runtime");
    }
    for name in ["serde", "tokio", "orthogonal", "portable-atomic", "sort", "tractor", "candlelight", "sunburn", "humanity-ai-protocol", "windows"] {
        assert!(!is_model_runtime(name), "{name} is not");
    }
    let toml = "[workspace]\nmembers = [\".\", \"crates/a\",\n \"crates/b\"]\n\n[dependencies]\nserde = \"1\"\nbrain = { package = \"llama-cpp-2\", version = \"0.1\" }\n\n[target.'cfg(windows)'.dependencies]\nort = { version = \"2\" }\n\n[dev-dependencies.candle-core]\nversion = \"0.8\"\n";
    let deps = manifest_dependencies(toml);
    for want in ["serde", "brain", "llama-cpp-2", "ort", "candle-core"] {
        assert!(deps.iter().any(|d| d == want), "{want} read from the synthetic manifest: {deps:?}");
    }
    assert_eq!(workspace_members(toml), ["." , "crates/a", "crates/b"]);
    let lock = "[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"ort-sys\"\nversion = \"2.0.0\"\n";
    let bad: Vec<String> = lock_packages(lock).into_iter().filter(|n| is_model_runtime(n)).collect();
    assert_eq!(bad, ["ort-sys"], "the lock reader finds a runtime");
    assert!(is_model_file(Path::new("data/ai/x.GGUF")) && is_model_file(Path::new("m.onnx")) && !is_model_file(Path::new("data/ai/choices.ron")));
}

/// The app's manifests (the root and every member) name no model runtime, the engine is no member, and
/// nothing depends on it.
#[test]
fn the_app_names_no_model_runtime() {
    let root_toml = fs::read_to_string(repo().join("Cargo.toml")).expect("the root Cargo.toml");
    let members = workspace_members(&root_toml);
    assert!(members.len() > 3, "the root's [workspace] members were read: {members:?}");
    assert!(!members.iter().any(|m| m.trim_end_matches('/') == ENGINE_DIR), "the AI engine ({ENGINE_DIR}) is a member of the app's workspace: it must keep a workspace of its own");
    let mut found = Vec::new();
    let mut read = 0;
    for m in &members {
        let path = if m == "." { repo().join("Cargo.toml") } else { repo().join(m).join("Cargo.toml") };
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        read += 1;
        for d in manifest_dependencies(&text) {
            if is_model_runtime(&d) || d == ENGINE_NAME {
                found.push(format!("{}: {d}", rel(&path)));
            }
        }
    }
    assert_eq!(read, members.len());
    assert!(found.is_empty(), "a model runtime in the app's manifests: {}", found.join(", "));
}

/// The app's whole dependency tree, transitive crates included (the root Cargo.lock), holds no model
/// runtime and not the engine.
#[test]
fn the_apps_lock_holds_no_model_runtime() {
    let lock = fs::read_to_string(repo().join("Cargo.lock")).expect("the root Cargo.lock");
    let packages = lock_packages(&lock);
    assert!(packages.len() > 100 && packages.iter().any(|p| p == "humanity-ai-protocol"), "the lock was read ({} packages)", packages.len());
    let bad: Vec<&String> = packages.iter().filter(|p| is_model_runtime(p) || p.as_str() == ENGINE_NAME).collect();
    assert!(bad.is_empty(), "a model runtime in the app's dependency tree (Cargo.lock): {bad:?}");
}

/// data/ and assets/, which ship beside the app, hold no model file.
#[test]
fn nothing_beside_the_app_is_a_model_file() {
    let mut files = Vec::new();
    files_under(&repo().join("data"), &mut files);
    let data = files.len();
    files_under(&repo().join("assets"), &mut files);
    assert!(data > 100 && files.len() > data, "the walk saw data/ ({data} files) and assets/ ({} files)", files.len() - data);
    let models: Vec<String> = files.iter().filter(|p| is_model_file(p)).map(|p| rel(p)).collect();
    assert!(models.is_empty(), "model files beside the app: {}", models.join(", "));
}

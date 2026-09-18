//! THE RIG'S DETERMINISM PINS CANNOT BE BYPASSED.
//!
//! Two dev pins exist so a measurement can be a function of the build alone
//! (2026-09-18, docs/design/frame-cost-arc.md "V1 outcome"):
//!
//!   * `EngineState::foliage_wind_override`  - showcase `{"wind":"<m/s>"}`,
//!     the wind speed the vegetation sees, reaching the shader through
//!     `Renderer::foliage_wind`.
//!   * `EngineState::anim_clock_pin`         - showcase `{"anim_clock":"<s>"}`,
//!     the celestial-pass animation clock, reaching the shader as the
//!     `time_s` argument of `render_celestial_onto` (stamped into
//!     `sun_color.w` on BOTH the colour and the shadow camera buffers).
//!
//! Each reaches the GPU through exactly one kind of statement, and a SECOND
//! such statement that forgets the pin does not break anything visible: the
//! pin simply stops working on that path, silently, and the capture it was
//! taken for looks like a real one. That already nearly happened in the
//! increment that introduced the pins - `render_celestial_onto` has a second
//! caller in `src/engine/ipc.rs` (the hi-res offscreen capture and the
//! live-broadcast pump) whose comment claims "same clock as the live path",
//! which would have been false the moment a pin was set. A rig screenshot
//! would then have rendered a swaying canopy out of a window that was
//! standing still.
//!
//! So this lint, not a comment: every call site must carry the pin.
//!
//! Std-only, compiled standalone like the other lints (never links the native
//! bin, so it dodges the Windows LNK1318 PDB limit):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/rig_pin_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

/// Every .rs file under src/, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Strip a `//` line comment, taking care not to cut inside a string literal.
/// Comments matter here because the spans below are found by counting
/// parentheses, and prose in an argument list is common in this codebase.
fn strip_line_comment(line: &str) -> String {
    let b = line.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' if in_str => i += 1, // skip the escaped byte
            b'"' => in_str = !in_str,
            b'/' if !in_str && i + 1 < b.len() && b[i + 1] == b'/' => return line[..i].to_string(),
            _ => {}
        }
        i += 1;
    }
    line.to_string()
}

/// The argument list of the call that starts at `needle` in `text`, by
/// matching parentheses from the call's opening one. Returns the span text and
/// the 1-based line number of the call. Comments are stripped first so a `)`
/// in prose cannot end the span early.
fn call_spans(text: &str, needle: &str) -> Vec<(usize, String)> {
    // Rebuild the file with comments removed but line structure intact, so
    // line numbers still mean something.
    let clean: Vec<String> = text.lines().map(strip_line_comment).collect();
    let joined = clean.join("\n");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = joined[from..].find(needle) {
        let at = from + rel;
        from = at + needle.len();
        // Skip the DEFINITION, which is the one place the name appears
        // without being a call.
        let line_start = joined[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
        if joined[line_start..at].contains("fn ") {
            continue;
        }
        // Walk from the opening paren to its match.
        let Some(open) = joined[at..].find('(').map(|i| at + i) else { continue };
        let bytes = joined.as_bytes();
        let mut depth = 0i32;
        let mut end = open;
        for i in open..bytes.len() {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end <= open {
            continue;
        }
        let line_no = joined[..at].matches('\n').count() + 1;
        out.push((line_no, joined[open..=end].to_string()));
    }
    out
}

const CLOCK_CALL: &str = "render_celestial_onto";
const CLOCK_PIN: &str = "anim_clock_pin";
const WIND_FIELD: &str = ".foliage_wind =";
const WIND_FN: &str = "published_foliage_wind";

#[test]
fn every_celestial_pass_call_honours_the_animation_clock_pin() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);

    let mut calls = 0usize;
    let mut violations = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else { continue };
        if !text.contains(CLOCK_CALL) {
            continue;
        }
        let rel = path.strip_prefix(repo).unwrap_or(path).to_string_lossy().replace('\\', "/");
        for (line, span) in call_spans(&text, CLOCK_CALL) {
            calls += 1;
            if !span.contains(CLOCK_PIN) {
                violations.push(format!("{rel}:{line}"));
            }
        }
    }

    // A lint that finds nothing to check is a lint that cannot fail. If the
    // call is ever renamed, this fires instead of going quietly green.
    assert!(
        calls >= 2,
        "\n\nrig_pin_lint found {calls} call site(s) of `{CLOCK_CALL}`, expected at least 2 \
         (the live frame in src/lib.rs and the offscreen capture in src/engine/ipc.rs).\n\
         Either the function was renamed - update CLOCK_CALL in tests/rig_pin_lint.rs - or the \
         scanner stopped matching, in which case this lint has been silently checking nothing.\n"
    );

    assert!(
        violations.is_empty(),
        "\n\n{CLOCK_CALL} called WITHOUT the animation-clock pin at:\n  {}\n\n\
         That argument is the clock the vegetation sway, the ocean wave phase and the cloud \
         advection are all sines of (it lands in sun_color.w on both the colour and the shadow \
         camera buffers). A call site passing `start_time.elapsed()` directly keeps animating \
         while every other path is frozen, so a rig capture taken through it is NOT comparable \
         with one taken through the others - and nothing about the picture says so.\n\n\
         Fix: pass the pin, exactly as src/lib.rs does:\n\
         \x20   state.{CLOCK_PIN}.unwrap_or_else(|| state.start_time.elapsed().as_secs_f32())\n",
        violations.join("\n  ")
    );
}

#[test]
fn every_foliage_wind_publish_goes_through_the_pin_helper() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);

    let mut writes = 0usize;
    let mut violations = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else { continue };
        let rel = path.strip_prefix(repo).unwrap_or(path).to_string_lossy().replace('\\', "/");
        // The renderer OWNS the field: its declaration and its default sit in
        // src/renderer/mod.rs and are not publishes.
        if rel == "src/renderer/mod.rs" {
            continue;
        }
        for (i, raw) in text.lines().enumerate() {
            let line = strip_line_comment(raw);
            if !line.contains(WIND_FIELD) {
                continue;
            }
            writes += 1;
            // The helper may be named on this line or on one of the next few
            // (the call is usually wrapped across lines).
            let window: String = text.lines().skip(i).take(6).collect::<Vec<_>>().join("\n");
            if !window.contains(WIND_FN) {
                violations.push(format!("{rel}:{}", i + 1));
            }
        }
    }

    assert!(
        writes >= 1,
        "\n\nrig_pin_lint found no `{WIND_FIELD}` publish at all. Either the field was renamed \
         - update WIND_FIELD in tests/rig_pin_lint.rs - or this lint is checking nothing.\n"
    );

    assert!(
        violations.is_empty(),
        "\n\nRenderer::foliage_wind written WITHOUT the pin helper at:\n  {}\n\n\
         That slot is the ONE place the wind reaches the vertex shader, and both the near-tree \
         sway and the grass sway read it. A publish that skips \
         engine::ipc::{WIND_FN} ignores the rig's `{{\"wind\":\"0\"}}` pin, and - worse - can \
         publish a literal 0, which the shader reads as \"no publisher\" and replaces with a \
         4 m/s fallback breeze. The knob would then do the OPPOSITE of what it says, with \
         nothing in the log, the manifest or the picture to reveal it.\n\n\
         Fix: build the value with\n\
         \x20   crate::engine::ipc::{WIND_FN}(state.foliage_wind_override, dir, speed, prev)\n",
        violations.join("\n  ")
    );
}

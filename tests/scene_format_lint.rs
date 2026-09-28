//! SCENE FORMAT lint (HDR scene target, increment 1, 2026-09-27; the plan is
//! docs/design/hdr-scene-target.md).
//!
//! The scene is drawn into a scene target whose format is
//! `Renderer::scene_format()`, and a present pass copies it into the display
//! (the swapchain). In increments 1 and 2 the two formats were equal, so a
//! scene pipeline built for the DISPLAY format by mistake still worked and
//! nothing would have noticed; the lint was written then, while the mistake
//! was harmless. Since increment 3 the scene target is `Rgba16Float`, and such
//! a pipeline fails wgpu validation the first time it draws, which is a
//! world-entry panic (the v0.1029 and v0.782 incident classes: every static
//! check green, the app dead on boot).
//!
//! Three checks:
//! 1. Every call to a scene-pipeline builder passes a `scene_format`
//!    expression and never `surface_format` or `config.format`. That covers
//!    the shader hot reload, which used to read `self.config.format`. Each
//!    builder must be found at least once, so a rename cannot quietly turn a
//!    row of this list into a check of nothing.
//! 2. Every OTHER read of the display format (`.surface_format()` or
//!    `config.format`) carries an inline `// display-format: <why>` note on
//!    the same line: egui, the capture targets, the camera screens (which
//!    the present pass writes), the tree-card bake. A new scene builder that
//!    is not in list 1 cannot reach for the display format without writing
//!    down why.
//! 3. The present pass's bind group layout has exactly ONE
//!    `create_bind_group` site (the v0.1029 lesson: a layout with several
//!    creation sites is one missed edit away from a world-entry panic).
//!
//! Each check's scanner is run on a synthetic source first (a positive
//! control), so a scanner that matches nothing cannot pass by accident.
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/scene_format_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The calls that build a pipeline which draws INTO THE SCENE, by the text
/// that opens the call. Add a row when a new scene pass gets a builder.
const SCENE_BUILDERS: &[&str] = &[
    "Pipeline::new(",
    ".recreate_pipelines(",
    "build_particle_pipelines(",
    "build_line_pipeline(",
    "BloomPass::new(",
    "GodrayPass::new(",
    "SsaoPass::new(",
    "CloudCompositePass::new(",
    "StarRenderer::new(",
];

/// The two spellings of the display format.
const DISPLAY_READS: &[&str] = &[".surface_format()", "config.format"];

/// The note a display-format read must carry on its own line.
const MARKER: &str = "display-format:";

/// The file that owns the present pass.
const PRESENT_FILE: &str = "src/renderer/scene_target.rs";

/// Every `.rs` file under `src/`, except the relay (a headless server that
/// never draws anything).
fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().map_or(false, |n| n == "relay") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().map_or(false, |x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo().join("src"), &mut out);
    out.sort();
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(repo()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// (code, comment) halves of one line. Naive about `//` inside a string
/// literal, which is fine for the lines this lint reads.
fn split_comment(line: &str) -> (&str, &str) {
    match line.find("//") {
        Some(i) => (&line[..i], &line[i..]),
        None => (line, ""),
    }
}

/// Every CALL of `needle` in `src` (a definition `fn name(` is not a call;
/// `CloudPipeline::new(` is not `Pipeline::new(`), as (1-based line, the
/// argument text between the balanced parentheses with comments removed).
fn call_args(src: &str, needle: &str) -> Vec<(usize, String)> {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(off) = src[from..].find(needle) {
        let at = from + off;
        from = at + needle.len();
        let line_start = src[..at].rfind('\n').map_or(0, |i| i + 1);
        let line_prefix = &src[line_start..at];
        if line_prefix.contains("//") {
            continue; // inside a comment
        }
        if needle.starts_with(ident) {
            if src[..at].chars().next_back().map_or(false, ident) {
                continue; // a longer name that merely ends in this one
            }
            if line_prefix.trim_end().ends_with("fn") {
                continue; // the definition
            }
        }
        // Walk the balanced parentheses from the one that ends the needle.
        let open = at + needle.len() - 1;
        let mut depth = 0i32;
        let mut end = None;
        for (i, c) in src[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { continue };
        let args: String = src[open + 1..end].lines().map(|l| split_comment(l).0).collect::<Vec<_>>().join("\n");
        let line = src[..at].matches('\n').count() + 1;
        out.push((line, args));
    }
    out
}

/// A builder call's argument text is right when it names the scene format
/// and neither spelling of the display format.
fn builder_args_problem(args: &str) -> Option<&'static str> {
    if args.contains("surface_format") {
        Some("passes `surface_format` (the DISPLAY format)")
    } else if args.contains("config.format") {
        Some("passes `config.format` (the DISPLAY format)")
    } else if !args.contains("scene_format") {
        Some("passes no `scene_format` expression")
    } else {
        None
    }
}

/// Lines (1-based) of `src` that read the display format without the note.
fn unmarked_display_reads(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let (code, comment) = split_comment(line);
        if DISPLAY_READS.iter().any(|d| code.contains(d)) && !comment.contains(MARKER) {
            out.push((i + 1, line.trim().to_string()));
        }
    }
    out
}

/// `needle` occurrences on code (not comment) lines.
fn code_count(src: &str, needle: &str) -> usize {
    src.lines().map(|l| split_comment(l).0.matches(needle).count()).sum()
}

#[test]
fn scanners_catch_what_they_are_for() {
    // Check 1's scanner: a display format passed to a builder is caught, a
    // scene format is not, and neither a definition nor a longer name counts.
    let src = "fn build_line_pipeline(device: &D, scene_format: F) {}\n\
               let p = Pipeline::new(&device, surface_format, &shader,\n    &batch);\n\
               let q = CloudPipeline::new(&device, surface_format);\n\
               let r = pipeline::Pipeline::new(&device, self.scene_format(), &shader, &batch);\n\
               // Pipeline::new(&device, surface_format) in a comment\n";
    let calls = call_args(src, "Pipeline::new(");
    assert_eq!(calls.len(), 2, "two real calls, got {calls:?}");
    assert_eq!(calls[0].0, 2);
    assert!(builder_args_problem(&calls[0].1).is_some(), "surface_format must be flagged");
    assert!(builder_args_problem(&calls[1].1).is_none(), "scene_format() must pass");
    assert!(call_args(src, "build_line_pipeline(").is_empty(), "a definition is not a call");
    assert!(builder_args_problem("&device, self.config.format, &m").is_some());
    assert!(builder_args_problem("&device, format, &m").is_some(), "an unnamed format is not proof");
    // Check 2's scanner.
    let src = "let f = self.config.format;\n\
               let g = self.config.format; // display-format: the capture target\n\
               // config.format in prose\n\
               let e = Renderer::new(renderer.surface_format(), 1);\n";
    let hits = unmarked_display_reads(src);
    assert_eq!(hits.iter().map(|h| h.0).collect::<Vec<_>>(), vec![1, 4], "got {hits:?}");
    // Check 3's counter ignores comments.
    assert_eq!(code_count("x.create_bind_group(a);\n// create_bind_group( in prose\n", "create_bind_group("), 1);
}

#[test]
fn scene_pso_builders_take_the_scene_format() {
    let files: Vec<(String, String)> =
        rust_sources().into_iter().map(|p| (rel(&p), fs::read_to_string(&p).unwrap_or_default())).collect();
    assert!(files.len() > 100, "the source walk found only {} files", files.len());
    let mut problems = Vec::new();
    for needle in SCENE_BUILDERS {
        let mut found = 0;
        for (path, src) in &files {
            for (line, args) in call_args(src, needle) {
                found += 1;
                if let Some(p) = builder_args_problem(&args) {
                    problems.push(format!("  {path}:{line}: `{needle}...)` {p}"));
                }
            }
        }
        assert!(
            found > 0,
            "no call of `{needle}` was found in src/: the builder was renamed or removed, so update \
             SCENE_BUILDERS in tests/scene_format_lint.rs (a row that matches nothing checks nothing)"
        );
    }
    assert!(
        problems.is_empty(),
        "\n\nA SCENE PIPELINE IS BUILT FOR THE WRONG FORMAT:\n{}\n\n\
         Scene pipelines draw into the scene target, whose format is \
         `Renderer::scene_format()` (`scene_target::scene_format_for` at init). \
         Since increment 3 of docs/design/hdr-scene-target.md it is Rgba16Float, \
         so a pipeline built for the display format fails validation at world entry.\n",
        problems.join("\n")
    );
}

#[test]
fn every_display_format_read_says_why() {
    let mut problems = Vec::new();
    for p in rust_sources() {
        let src = fs::read_to_string(&p).unwrap_or_default();
        for (line, text) in unmarked_display_reads(&src) {
            problems.push(format!("  {}:{line}: {text}", rel(&p)));
        }
    }
    assert!(
        problems.is_empty(),
        "\n\nTHE DISPLAY FORMAT IS READ WITHOUT A REASON:\n{}\n\n\
         Pipelines that draw the scene must use `scene_format()`. If this read \
         is genuinely about the display (egui, a capture or camera-screen \
         target the present pass writes, a readback swizzle, the tree-card \
         bake), say so on the same line: `// display-format: <why>`.\n",
        problems.join("\n")
    );
}

#[test]
fn the_present_layout_has_one_bind_group_site() {
    let src = fs::read_to_string(repo().join(PRESENT_FILE)).unwrap_or_else(|e| panic!("read {PRESENT_FILE}: {e}"));
    assert_eq!(code_count(&src, "create_bind_group_layout("), 1, "{PRESENT_FILE} should build exactly one layout");
    // `create_bind_group(` also matches inside `create_bind_group_layout(`?
    // No: the layout call continues `_layout(`, not `(`.
    assert_eq!(
        code_count(&src, "create_bind_group("),
        1,
        "{PRESENT_FILE} must create the present bind group in ONE place (`PresentPass::bind`), \
         shared by init, resize and the view scratch"
    );
    // And nobody else builds one for this layout.
    for p in rust_sources() {
        let r = rel(&p);
        if r == PRESENT_FILE {
            continue;
        }
        let other = fs::read_to_string(&p).unwrap_or_default();
        assert!(
            !other.contains("Present BGL") && !other.contains("present_bind_group_layout"),
            "{r} reaches for the present pass's layout; bind through `SceneTarget::new` instead"
        );
    }
}

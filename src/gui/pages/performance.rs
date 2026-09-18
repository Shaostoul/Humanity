//! Performance page (resource budgets increment 1 - MEASURE).
//!
//! Four live pies - GPU, CPU, VRAM, RAM - over one registry of systems, so the
//! question "where does my machine go?" has an answer inside the app instead of
//! in a profiler. Read-only in this increment; increment 2 makes the slices
//! draggable ceilings and increment 3 adds the governor that enforces them
//! (docs/design/resource-budgets.md).
//!
//! WHERE IT LIVES: Platform > Performance. Platform is the app's-own-machinery
//! tab (Files, Bugs, Testing, Dev, Browser) and this is the page-sized sibling
//! of the F2 performance overlay, so it needs no new top-level page and no
//! `GuiPage` variant.
//!
//! WHERE THE NUMBERS COME FROM: `renderer::frame_costs`, a process-global store
//! the renderer writes each frame (GPU timestamp queries resolved a frame late,
//! CPU stage timers, tracked allocations, process working set). The page only
//! reads, formats, and calls `frame_costs::watch()` so the expensive inventory
//! sampling happens ONLY while this page is open.
//!
//! WHAT IS A DATA ROW: everything about a slice - its name, which pie it is on,
//! which measurements sum into it, whether it is a locked floor, and its
//! expected share - lives in `data/performance/budget_systems.ron`. Adding a
//! system to the pie is adding a row (infinite-of-X).

use egui::{RichText, ScrollArea};
use serde::Deserialize;

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, pie::{fmt_bytes, fmt_ms, PieSlice, PieSpec}};
use crate::gui::GuiState;
use crate::renderer::frame_costs::{self, GpuTiming, Snapshot};

/// One row of `data/performance/budget_systems.ron`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BudgetSystem {
    pub id: String,
    pub display: String,
    /// "gpu" | "cpu" | "vram" | "ram".
    pub category: String,
    /// Measurement ids summed into this slice (see `frame_costs`).
    pub sources: Vec<String>,
    /// A floor system the operator may not starve (enforced in increment 2).
    pub locked: bool,
    /// Expected share of its pie, 0..1.
    pub base_frac: f32,
    /// Theme token name for the slice colour.
    pub color: String,
    #[serde(default)]
    pub note: String,
}

/// The four pies, in the order they are drawn.
pub const CATEGORIES: [(&str, &str); 4] = [
    ("gpu", "GPU frame time"),
    ("cpu", "CPU frame time"),
    ("vram", "Graphics memory"),
    ("ram", "System memory"),
];

/// The registry, parsed once. Embedded at compile time like the other small
/// registries (`lod_registry`, lock types), so a stripped install still has it.
pub fn systems() -> &'static [BudgetSystem] {
    static REG: std::sync::OnceLock<Vec<BudgetSystem>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../../data/performance/budget_systems.ron");
        match ron::from_str::<Vec<BudgetSystem>>(SRC) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) | Err(_) => {
                log::error!(
                    "data/performance/budget_systems.ron missing or invalid; the \
                     Performance page will show no slices"
                );
                Vec::new()
            }
        }
    })
}

/// Resolve a registry colour name to a theme token. Unknown names fall back to
/// the muted text colour, which makes a typo visible without crashing.
pub fn token_color(theme: &Theme, name: &str) -> egui::Color32 {
    match name {
        "accent" => theme.accent(),
        "success" => theme.success(),
        "warning" => theme.warning(),
        "danger" => theme.danger(),
        "info" => theme.info(),
        "nav_reality" => theme.nav_reality(),
        "nav_sim" => theme.nav_sim(),
        "nav_tools" => theme.nav_tools(),
        "nav_dev" => theme.nav_dev(),
        "orbit_line" => theme.orbit_line(),
        "constellation_line" => theme.constellation_line(),
        "badge_donor" => theme.badge_donor(),
        "badge_verified" => theme.badge_verified(),
        _ => theme.text_muted(),
    }
}

/// Sum a row's sources out of the snapshot.
pub fn row_value(snap: &Snapshot, row: &BudgetSystem) -> f64 {
    row.sources.iter().map(|s| snap.value(s)).sum()
}

/// The legend text of a GPU row none of whose passes can be measured on this
/// adapter (no timestamp queries AND no `cpu.*` twin to read). Shown in the
/// value column in place of a "0.0 ms" that would read as free.
pub const NOT_MEASURABLE: &str = "not measurable on this adapter";

/// How many of a GPU row's sources are real measurements in this snapshot
/// (all of them with timestamp queries; in the CPU fallback only the ids with
/// a `cpu.*` twin). `(measurable, total)`.
pub fn row_measurable(snap: &Snapshot, row: &BudgetSystem) -> (usize, usize) {
    let total = row.sources.len();
    let measurable = row.sources.iter().filter(|s| snap.gpu_measurable(s)).count();
    (measurable, total)
}

/// The GPU pie's footer line about passes the timestamp ring refused this
/// frame, or an empty string when every pass was timed. A refused pass runs
/// untimed and looks free on the pie, so the count is said out loud with the
/// fix (`MAX_TIMED_PASSES` in `renderer/frame_costs.rs`).
pub fn untimed_note(untimed: u32) -> String {
    match untimed {
        0 => String::new(),
        1 => "1 pass untimed this frame: raise MAX_TIMED_PASSES.".to_string(),
        n => format!("{n} passes untimed this frame: raise MAX_TIMED_PASSES."),
    }
}

/// Build one pie from the registry rows of a category plus the live snapshot.
/// `remainder` (if any) is appended as a final slice, which is how the GPU pie
/// shows unmeasured time and the VRAM pie shows the driver's untracked memory.
pub fn build_spec(
    theme: &Theme,
    snap: &Snapshot,
    category: &str,
    title: &str,
) -> PieSpec {
    let is_bytes = category == "vram" || category == "ram";
    let fmt = |v: f64| if is_bytes { fmt_bytes(v) } else { fmt_ms(v) };
    let mut slices: Vec<PieSlice> = Vec::new();
    let mut tracked = 0.0;
    // GPU rows that cannot be measured at all on this adapter (named in the
    // footer), so the operator reads "not measurable", never a silent 0.
    let mut unmeasurable: Vec<String> = Vec::new();
    for row in systems().iter().filter(|r| r.category == category) {
        let v = row_value(snap, row);
        tracked += v;
        let mut display = fmt(v);
        let mut note = row.note.clone();
        if category == "gpu" && snap.gpu_timing == GpuTiming::CpuFallback {
            // In the CPU fallback a `gpu.x` source reads `cpu.x`; a pass with
            // no such twin contributes exactly 0 and would look free. Say so
            // on the row (all sources twin-less) or in its hover (some).
            let (m, total) = row_measurable(snap, row);
            if m == 0 {
                display = NOT_MEASURABLE.to_string();
                note = format!(
                    "{note}\nNot measurable here: this adapter has no GPU timestamp \
                     queries and none of this row's passes has a CPU-side stage to \
                     read instead."
                );
                unmeasurable.push(row.display.clone());
            } else if m < total {
                note = format!(
                    "{note}\nPartial on this adapter: {} of {total} passes have no \
                     CPU-side stage and read 0 here.",
                    total - m
                );
            }
        }
        // Zero rows are kept: a system that costs nothing right now is exactly
        // the information the operator wants when comparing settings, and the
        // legend is the registry's own list.
        slices.push(PieSlice {
            label: row.display.clone(),
            value: v,
            display,
            color: token_color(theme, &row.color),
            locked: row.locked,
            base_frac: row.base_frac,
            note,
        });
    }

    // The honest remainder.
    let (headline, footer) = match category {
        "gpu" => {
            let rest = (snap.frame_ms as f64 - tracked).max(0.0);
            slices.push(remainder_slice(
                theme,
                "Elsewhere (present, drivers, untimed passes)",
                rest,
                fmt(rest),
                "Frame time not accounted for by a timed pass: swapchain present, \
                 driver work, and passes submitted outside the renderer module.",
            ));
            let mut foot = match snap.gpu_timing {
                GpuTiming::Timestamps => {
                    "Measured with GPU timestamp queries, resolved one frame late."
                        .to_string()
                }
                GpuTiming::CpuFallback => {
                    let mut s = "This adapter has no GPU timestamp queries, so these are \
                                 CPU-side pass submission times, which do NOT include GPU \
                                 execution."
                        .to_string();
                    if !unmeasurable.is_empty() {
                        s.push_str(&format!(
                            " Not measurable on this adapter (no CPU-side stage): {}.",
                            unmeasurable.join(", ")
                        ));
                    }
                    s
                }
            };
            // A pass refused a timestamp slot ran untimed and is missing from
            // every row above; the ring's refusal count says so.
            let untimed = untimed_note(snap.gpu_untimed_passes);
            if !untimed.is_empty() {
                foot.push(' ');
                foot.push_str(&untimed);
            }
            (format!("{} per frame", fmt_ms(snap.frame_ms as f64)), foot)
        }
        "cpu" => {
            let rest = (snap.frame_ms as f64 - tracked).max(0.0);
            slices.push(remainder_slice(
                theme,
                "Elsewhere (engine, interface, waiting)",
                rest,
                fmt(rest),
                "Everything not yet given its own stage timer, plus any time spent \
                 waiting for the display.",
            ));
            (
                format!("{} per frame", fmt_ms(snap.frame_ms as f64)),
                "Stage timers measure wall time inside the frame loop. Rows with no \
                 timer yet read zero until their call site is wired."
                    .to_string(),
            )
        }
        "vram" => {
            let driver = snap.value("vram.driver_reserved");
            let rest = (driver - tracked).max(0.0);
            if driver > 0.0 {
                slices.push(remainder_slice(
                    theme,
                    "Untracked / driver",
                    rest,
                    fmt(rest),
                    "Graphics memory the driver holds that this inventory does not \
                     name yet: the star field, shader and pipeline objects, staging \
                     buffers, and allocator padding.",
                ));
            }
            let reserved = snap.value("vram.patch_arena_reserved");
            let foot = if driver > 0.0 {
                format!(
                    "Tracked allocations against the {} the graphics allocator reports. \
                     The terrain arena reserves {} up front.",
                    fmt_bytes(driver),
                    fmt_bytes(reserved)
                )
            } else {
                "Tracked allocations only: this backend does not report an allocator \
                 total, so untracked memory cannot be shown."
                    .to_string()
            };
            (
                format!("{} tracked", fmt_bytes(tracked)),
                foot,
            )
        }
        _ => (
            format!("{} in use", fmt_bytes(snap.value("ram.resident"))),
            if snap.value("ram.resident") > 0.0 {
                "Process memory read from the operating system once a second while \
                 this page is open."
                    .to_string()
            } else {
                "Process memory is not available on this platform.".to_string()
            },
        ),
    };

    PieSpec {
        title: title.to_string(),
        headline,
        slices,
        footer,
    }
}

/// The computed "everything else" wedge. Locked, because it is not a system
/// anyone can be given a budget.
fn remainder_slice(
    theme: &Theme,
    label: &str,
    value: f64,
    display: String,
    note: &str,
) -> PieSlice {
    PieSlice {
        label: label.to_string(),
        value,
        display,
        color: theme.text_muted(),
        locked: true,
        base_frac: 0.0,
        note: note.to_string(),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, _state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                draw_content(ui, theme);
            });
        });
}

/// The page body, split out so the headless snapshot test can render it
/// without a window.
pub fn draw_content(ui: &mut egui::Ui, theme: &Theme) {
    // Tell the engine somebody is looking: the memory inventory and the
    // working-set syscall only run while this is fresh.
    frame_costs::watch();
    let snap = frame_costs::snapshot();

    ui.label(
        RichText::new("Performance")
            .size(theme.font_size_title)
            .color(theme.text_primary()),
    );
    ui.label(
        RichText::new(
            "Where your machine goes, measured live. Each pie is one budget: a \
             slice can only grow by taking from its neighbours.",
        )
        .size(theme.font_size_small)
        .color(theme.text_muted()),
    );
    ui.label(
        RichText::new(
            "This page does not render the world, so the frame numbers below are \
             held at your LAST rendered world frame. Memory is still live.",
        )
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_sm);

    if !snap.has_frames {
        widgets::card(ui, theme, |ui| {
            ui.label(
                RichText::new("No frames measured yet.")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new(
                    "Enter the world once; the pies fill in as soon as the renderer \
                     draws a frame.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
        });
        ui.add_space(theme.spacing_sm);
    }

    let specs: Vec<PieSpec> = CATEGORIES
        .iter()
        .map(|(cat, title)| build_spec(theme, &snap, cat, title))
        .collect();
    widgets::pie::pie_grid(ui, theme, &specs);

    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new(
            "Read-only for now. Allocating a share to each system (dragging the \
             slices) arrives with the budget editor, and the governor that keeps \
             systems inside their share arrives after it.",
        )
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry must parse, cover all four pies, and carry sane rows. This
    /// is the `just validate-data` guard for the budget registry.
    #[test]
    fn budget_registry_parses_and_every_row_is_sane() {
        let rows = systems();
        assert!(
            rows.len() >= 15,
            "expected the full budget registry, got {} rows (did the RON fail to parse?)",
            rows.len()
        );
        let mut ids: Vec<&str> = Vec::new();
        for r in rows {
            assert!(!r.id.is_empty(), "a row is missing its id");
            assert!(!r.display.is_empty(), "row {} has no display name", r.id);
            assert!(
                CATEGORIES.iter().any(|(c, _)| *c == r.category),
                "row {} has unknown category {:?}",
                r.id,
                r.category
            );
            assert!(!r.sources.is_empty(), "row {} names no measurement sources", r.id);
            for s in &r.sources {
                let ok = s.starts_with("gpu.")
                    || s.starts_with("cpu.")
                    || s.starts_with("vram.")
                    || s.starts_with("ram.");
                assert!(ok, "row {} source {:?} has no known prefix", r.id, s);
            }
            assert!(
                (0.0..=1.0).contains(&r.base_frac),
                "row {} base_frac {} is not a fraction",
                r.id,
                r.base_frac
            );
            assert!(!ids.contains(&r.id.as_str()), "duplicate row id {}", r.id);
            ids.push(&r.id);
        }
        // Every pie must have at least two slices or it is not a budget.
        for (cat, _) in CATEGORIES.iter() {
            let n = rows.iter().filter(|r| r.category == *cat).count();
            assert!(n >= 2, "category {cat} has {n} rows; a pie needs at least two");
        }
    }

    /// Each pie's base_frac column should describe a whole budget: the expected
    /// shares must be in the same ballpark as 100%, or the "expected vs actual"
    /// hover text lies.
    #[test]
    fn expected_shares_add_up_per_pie() {
        for (cat, _) in CATEGORIES.iter() {
            let sum: f32 = systems()
                .iter()
                .filter(|r| r.category == *cat)
                .map(|r| r.base_frac)
                .sum();
            assert!(
                (0.85..=1.15).contains(&sum),
                "category {cat} expected shares sum to {sum:.2}, which is not a budget"
            );
        }
    }

    /// Every colour name in the registry must resolve to a real theme token,
    /// not silently fall back to the muted text colour.
    #[test]
    fn registry_colors_resolve_to_theme_tokens() {
        let theme = crate::gui::theme::load_theme();
        let muted = theme.text_muted();
        for r in systems() {
            let c = token_color(&theme, &r.color);
            assert!(
                c != muted || r.color == "text_muted",
                "row {} colour token {:?} is unknown (fell back to muted)",
                r.id,
                r.color
            );
        }
    }

    // ── The join between the registry and the engine, in BOTH directions ──
    //
    // The ids the engine records are read from the SOURCE TREE, not from a
    // hand-typed list: the previous version of this test carried its own
    // copy of the ids and drifted (it vouched for `cpu.mesh_upload`, which
    // nothing had recorded in months). A string literal shaped like
    // `gpu.x` / `cpu.x` / `vram.x` / `ram.x` in non-comment, non-test code
    // IS a measurement id: that is the whole convention of `frame_costs`.

    /// Files compiled only under `cfg(test)` by their parent module (the
    /// `#[cfg(test)] mod ui_snapshots;` line in gui/mod.rs), so nothing in
    /// them records at runtime; the seeded numbers they stage would
    /// otherwise read as recorders.
    const TEST_ONLY_FILES: &[&str] = &["src/gui/ui_snapshots.rs"];

    /// Ids the engine records that are DELIBERATELY not a pie row, each with
    /// its reason. A recorded id that is neither a row nor on this list
    /// fails `every_recorded_id_is_a_row_or_a_listed_remainder`.
    const REMAINDER_IDS: &[(&str, &str)] = &[
        (
            "cpu.frame_total",
            "the whole frame (RedrawRequested entry to the last submit); the CPU pie's \
             rows are its parts, so a row would count everything twice",
        ),
        (
            "cpu.chunk_veg_and_draws",
            "a bucket that CONTAINS cpu.near_tree_harvest and cpu.grass_harvest (both \
             rows); its own uncovered part is draw-list assembly, honest remainder",
        ),
        (
            "vram.patch_arena_reserved",
            "the arena's up-front reservation, shown in the VRAM footer, not a slice \
             (vram.patch_arena is the slice)",
        ),
        (
            "vram.driver_reserved",
            "the allocator's total, which the VRAM pie's untracked remainder is \
             computed against",
        ),
        (
            "vram.driver_allocated",
            "recorded for the HUMANITY_FRAME_COSTS JSON drop; the pie uses reserved",
        ),
    ];

    /// Ids the registry may name before (or after) their recorder exists:
    /// the page shows them at zero until the call site is wired. Each entry
    /// states its debt rather than hiding it.
    const PENDING_IDS: &[&str] = &[
        // Wired for timing in `renderer/bloom.rs` (`BloomPass::apply` takes
        // the slot pair) but DORMANT: nothing calls `apply` today, because
        // bloom_intensity defaults to 0 and the call site was removed with
        // it. The row stays so switching bloom back on needs no registry
        // edit; the source scan cannot see an id that only a doc comment
        // and a dead parameter carry.
        "gpu.bloom",
    ];

    /// One per registered ECS system (`cpu.system.<slug>`), interned at
    /// registration by `ecs::systems::SystemRunner` from the system's own
    /// name. Not rows: `cpu.systems` is their sum and a row per system would
    /// count the tick twice (see `ecs_tick_is_not_counted_twice_on_the_cpu_pie`).
    const ECS_FAMILY_PREFIX: &str = "cpu.system.";

    /// Every `.rs` file under `src/`, recursively.
    fn rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_sources(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    /// The file's code with comments and its trailing `#[cfg(test)] mod`
    /// region dropped (test modules sit at the bottom of a file by
    /// convention here, and a doc comment quoting `pass_timer("gpu.x")` is
    /// prose, not a recorder).
    fn code_only(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            if t == "#[cfg(test)]" {
                let next = lines[i + 1..].iter().find(|l| !l.trim().is_empty());
                if next.is_some_and(|l| l.trim_start().starts_with("mod ")) {
                    break;
                }
            }
            if t.starts_with("//") {
                continue;
            }
            // A trailing `// comment` after code is dropped too.
            let code = match line.find("//") {
                Some(ix) => &line[..ix],
                None => line,
            };
            out.push_str(code);
            out.push('\n');
        }
        out
    }

    /// Every id-shaped string literal in `code`: `"gpu.…"`, `"cpu.…"`,
    /// `"vram.…"`, `"ram.…"`. A literal ending in `.` is a PREFIX (the ECS
    /// family's `intern_id("cpu.system.", …)`), reported as such.
    fn ids_in(code: &str, out: &mut std::collections::BTreeSet<String>) {
        let mut i = 0;
        while let Some(off) = code[i..].find('"') {
            let start = i + off + 1;
            let Some(len) = code[start..].find('"') else { break };
            let lit = &code[start..start + len];
            i = start + len + 1;
            let shaped = ["gpu.", "cpu.", "vram.", "ram."].iter().any(|p| lit.starts_with(p))
                && lit.len() > 4
                && lit.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.');
            if shaped {
                out.insert(lit.to_string());
            }
        }
    }

    /// `frame_costs::GPU_IDS_WITHOUT_CPU_TWIN` is a list ABOUT ids (which
    /// passes have no CPU stage), not a recorder: its literals must not
    /// count as "the engine records this", or the dormant `gpu.bloom` it
    /// names would look recorded. The block from its declaration to the
    /// closing `];` is dropped before scanning.
    fn strip_twin_list(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut skipping = false;
        for line in src.lines() {
            if line.contains("GPU_IDS_WITHOUT_CPU_TWIN: &[&str] = &[") {
                skipping = true;
                continue;
            }
            if skipping {
                if line.trim() == "];" {
                    skipping = false;
                }
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// The set of ids the engine records, read from the source tree.
    fn recorded_ids() -> std::collections::BTreeSet<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        rust_sources(&root.join("src"), &mut files);
        assert!(files.len() > 100, "the source walk found only {} files", files.len());
        let mut ids = std::collections::BTreeSet::new();
        for f in files {
            let rel = f.strip_prefix(root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
            if TEST_ONLY_FILES.contains(&rel.as_str()) {
                continue;
            }
            let src = std::fs::read_to_string(&f).unwrap_or_default();
            ids_in(&code_only(&strip_twin_list(&src)), &mut ids);
        }
        ids
    }

    /// Every measurement id the registry names must be one the engine
    /// actually records (found in the source tree), or be on the explicit
    /// pending list; otherwise the row is decoration.
    #[test]
    fn registry_sources_are_ids_the_engine_records() {
        let recorded = recorded_ids();
        for r in systems() {
            for s in &r.sources {
                let known = recorded.contains(s)
                    || PENDING_IDS.contains(&s.as_str())
                    || s.starts_with(ECS_FAMILY_PREFIX);
                assert!(
                    known,
                    "row {} names measurement {:?}, which nothing in src/ records and \
                     which is not on the pending list",
                    r.id, s
                );
            }
        }
    }

    /// The other direction: every id the engine records is either a registry
    /// row's source or is explicitly listed as remainder with a reason. This
    /// is what keeps a new timestamp scope from silently folding into
    /// "Elsewhere" (the 2026-09-18 measurement found the four cloud passes,
    /// gpu.celestial_t and the screens' passes doing exactly that).
    #[test]
    fn every_recorded_id_is_a_row_or_a_listed_remainder() {
        let recorded = recorded_ids();
        let registered: std::collections::BTreeSet<&str> = systems()
            .iter()
            .flat_map(|r| r.sources.iter().map(|s| s.as_str()))
            .collect();
        let mut orphans = Vec::new();
        for id in &recorded {
            if id.ends_with('.') {
                // A prefix family: only the ECS one is known.
                assert_eq!(id, ECS_FAMILY_PREFIX, "unknown measurement prefix family {id:?}");
                continue;
            }
            let ok = registered.contains(id.as_str())
                || REMAINDER_IDS.iter().any(|(r, _)| r == id)
                || id.starts_with(ECS_FAMILY_PREFIX);
            if !ok {
                orphans.push(id.clone());
            }
        }
        assert!(
            orphans.is_empty(),
            "these measurement ids are recorded by the engine but are neither a row \
             in data/performance/budget_systems.ron nor listed in REMAINDER_IDS with \
             a reason, so they fold into \"Elsewhere\" unseen: {orphans:?}"
        );
        // And the remainder list may not go stale either: every entry must
        // still be recorded somewhere, or it is documenting a ghost.
        for (id, why) in REMAINDER_IDS {
            assert!(
                recorded.contains(*id),
                "REMAINDER_IDS lists {id:?} ({why}), but nothing in src/ records it any more"
            );
        }
        // The two lists must not overlap: an id is a row or a remainder.
        for (id, _) in REMAINDER_IDS {
            assert!(!registered.contains(id), "{id:?} is both a registry source and a listed remainder");
        }
    }

    /// The scanner itself: comments and test modules are dropped, prefixes
    /// are reported as prefixes, and lookalike strings are ignored.
    #[test]
    fn id_scanner_reads_code_not_prose() {
        let src = "\
// a doc line quoting pass_timer(\"gpu.prose\")\n\
let a = stage(\"cpu.real\"); // trailing pass_timer(\"gpu.trailing\")\n\
let b = intern_id(\"cpu.system.\", name);\n\
let c = \"not an id\";\n\
let d = \"gpu.Upper\";\n\
#[cfg(test)]\n\
mod tests {\n\
    let t = stage(\"cpu.test_only\");\n\
}\n";
        let mut ids = std::collections::BTreeSet::new();
        ids_in(&code_only(src), &mut ids);
        let got: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        assert_eq!(got, vec!["cpu.real", "cpu.system."]);

        // The twin list is ABOUT ids, not a recorder: its block is dropped,
        // the code around it is kept.
        let with_list = "\
let a = stage(\"cpu.before\");\n\
pub const GPU_IDS_WITHOUT_CPU_TWIN: &[&str] = &[\n\
    \"gpu.listed_only\",\n\
    // a comment inside the list\n\
    \"gpu.also_listed\",\n\
];\n\
let b = stage(\"cpu.after\");\n";
        let mut ids = std::collections::BTreeSet::new();
        ids_in(&code_only(&strip_twin_list(with_list)), &mut ids);
        let got: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        assert_eq!(got, vec!["cpu.after", "cpu.before"]);
    }

    /// A row may not name the ECS aggregate AND a per-system id in the same
    /// pie: the pie sums its rows, so counting a system twice would inflate its
    /// share and shrink the honest "Elsewhere" remainder.
    #[test]
    fn ecs_tick_is_not_counted_twice_on_the_cpu_pie() {
        let aggregate = systems()
            .iter()
            .any(|r| r.sources.iter().any(|s| s == "cpu.systems"));
        let per_system = systems()
            .iter()
            .any(|r| r.sources.iter().any(|s| s.starts_with("cpu.system.")));
        assert!(
            !(aggregate && per_system),
            "the CPU pie names both the ECS total (cpu.systems) and per-system \
             rows (cpu.system.*); pick one or the tick is counted twice"
        );
    }

    /// A pie built from an empty snapshot must still list every registry row
    /// (so the page is readable before world entry) and must not divide by zero.
    #[test]
    fn pies_build_from_an_empty_snapshot() {
        let theme = crate::gui::theme::load_theme();
        let snap = Snapshot {
            gpu: Vec::new(),
            cpu: Vec::new(),
            vram: Vec::new(),
            ram: Vec::new(),
            frame_ms: 0.0,
            gpu_timing: GpuTiming::CpuFallback,
            has_frames: false,
            gpu_untimed_passes: 0,
        };
        for (cat, title) in CATEGORIES.iter() {
            let spec = build_spec(&theme, &snap, cat, title);
            let rows = systems().iter().filter(|r| r.category == *cat).count();
            assert!(
                spec.slices.len() >= rows,
                "category {cat} lost rows when nothing was measured"
            );
            assert!(
                widgets::pie::fractions(&spec.slices).is_empty(),
                "an all-zero pie must report no fractions rather than NaN"
            );
        }
    }

    /// With real numbers in the store, a row must pick up exactly the sum of
    /// its sources - the join between the registry and the measurements.
    #[test]
    fn a_row_sums_its_sources() {
        let snap = Snapshot {
            gpu: vec![
                frame_costs::Entry { id: "gpu.particles", value: 1.5 },
                frame_costs::Entry { id: "gpu.gpu_particles", value: 2.0 },
            ],
            cpu: Vec::new(),
            vram: Vec::new(),
            ram: Vec::new(),
            frame_ms: 16.0,
            gpu_timing: GpuTiming::Timestamps,
            has_frames: true,
            gpu_untimed_passes: 0,
        };
        let row = systems()
            .iter()
            .find(|r| r.id == "particles")
            .expect("the particles row is in the registry");
        assert!((row_value(&snap, row) - 3.5).abs() < 1e-9);
    }

    /// A snapshot with the given GPU timing mode and no measurements.
    fn empty_snap(gpu_timing: GpuTiming, untimed: u32) -> Snapshot {
        Snapshot {
            gpu: Vec::new(),
            cpu: Vec::new(),
            vram: Vec::new(),
            ram: Vec::new(),
            frame_ms: 16.0,
            gpu_timing,
            has_frames: true,
            gpu_untimed_passes: untimed,
        }
    }

    /// On an adapter without timestamp queries, a GPU row none of whose passes
    /// has a `cpu.*` twin reads "not measurable on this adapter" on the row
    /// AND is named in the footer, instead of showing 0.0 ms (the Clouds row
    /// did exactly that, silently, before the 2026-09-18 review). A row with
    /// twins keeps its number; with timestamps nothing is annotated.
    #[test]
    fn fallback_marks_rows_without_a_cpu_twin_as_not_measurable() {
        let theme = crate::gui::theme::load_theme();
        let fallback = build_spec(&theme, &empty_snap(GpuTiming::CpuFallback, 0), "gpu", "GPU");
        let clouds = fallback.slices.iter().find(|s| s.label == "Clouds").expect("Clouds row");
        assert_eq!(clouds.display, NOT_MEASURABLE, "the Clouds passes have no CPU twin");
        assert!(clouds.note.contains("Not measurable here"), "hover: {}", clouds.note);
        assert!(
            fallback.footer.contains("Not measurable on this adapter") && fallback.footer.contains("Clouds"),
            "the footer must name the unmeasurable rows: {}",
            fallback.footer
        );
        // Partial rows keep a number and explain the gap in the hover.
        let world = fallback.slices.iter().find(|s| s.label == "World surface").unwrap();
        assert_ne!(world.display, NOT_MEASURABLE, "gpu.celestial has cpu.celestial");
        assert!(world.note.contains("Partial on this adapter"), "hover: {}", world.note);
        // Fully twinned rows are untouched.
        let screens = fallback.slices.iter().find(|s| s.label == "In-world screens").unwrap();
        assert!(!screens.note.contains("adapter"), "every screen pass has a twin: {}", screens.note);
        // With timestamps every row is a measurement.
        let real = build_spec(&theme, &empty_snap(GpuTiming::Timestamps, 0), "gpu", "GPU");
        assert!(real.slices.iter().all(|s| s.display != NOT_MEASURABLE));
        assert!(!real.footer.contains("Not measurable"));
    }

    /// Passes the timestamp ring refused are said out loud in the GPU footer
    /// with the fix, and nothing is said when every pass was timed.
    #[test]
    fn untimed_passes_are_named_in_the_gpu_footer() {
        let theme = crate::gui::theme::load_theme();
        let clean = build_spec(&theme, &empty_snap(GpuTiming::Timestamps, 0), "gpu", "GPU");
        assert!(!clean.footer.contains("untimed"), "{}", clean.footer);
        let dropped = build_spec(&theme, &empty_snap(GpuTiming::Timestamps, 3), "gpu", "GPU");
        assert!(
            dropped.footer.contains("3 passes untimed this frame: raise MAX_TIMED_PASSES"),
            "{}",
            dropped.footer
        );
        assert_eq!(untimed_note(1), "1 pass untimed this frame: raise MAX_TIMED_PASSES.");
        assert_eq!(untimed_note(0), "");
        // The CPU pie has no ring; its footer must not carry the note.
        let cpu = build_spec(&theme, &empty_snap(GpuTiming::Timestamps, 3), "cpu", "CPU");
        assert!(!cpu.footer.contains("untimed"));
    }

    /// `frame_costs::GPU_IDS_WITHOUT_CPU_TWIN` is what the page consults to
    /// say "not measurable"; it must equal the set the source tree implies:
    /// every `gpu.x` (recorded, or a registry source) for which no `cpu.x`
    /// is recorded anywhere. Add a `cpu.x` stage or a twin-less `gpu.x` pass
    /// and this names the drift in both directions.
    #[test]
    fn no_twin_list_matches_the_source_tree() {
        let recorded = recorded_ids();
        let mut gpu_ids: std::collections::BTreeSet<String> = recorded
            .iter()
            .filter(|id| id.starts_with("gpu.") && !id.ends_with('.'))
            .cloned()
            .collect();
        for r in systems() {
            for s in &r.sources {
                if s.starts_with("gpu.") {
                    gpu_ids.insert(s.clone());
                }
            }
        }
        let expected: std::collections::BTreeSet<String> = gpu_ids
            .into_iter()
            .filter(|g| !recorded.contains(&format!("cpu.{}", &g["gpu.".len()..])))
            .collect();
        let listed: std::collections::BTreeSet<String> = frame_costs::GPU_IDS_WITHOUT_CPU_TWIN
            .iter()
            .map(|s| s.to_string())
            .collect();
        let missing: Vec<&String> = expected.difference(&listed).collect();
        let stale: Vec<&String> = listed.difference(&expected).collect();
        assert!(
            missing.is_empty() && stale.is_empty(),
            "GPU_IDS_WITHOUT_CPU_TWIN drifted from the source tree.\n\
             gpu ids with no cpu twin that are NOT listed (the page would show 0 \
             instead of \"not measurable\"): {missing:?}\n\
             listed ids that now HAVE a twin or are not recorded: {stale:?}"
        );
    }
}

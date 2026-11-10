//! EVERY GRAPHICS CONTROL SAYS WHAT IT CHANGES AND WHAT IT COSTS (2026-11-10; CLAUDE.md, "Every
//! graphics setting says what it actually changes"; docs/design/graphics-settings.md). The rule's
//! ratchet: a control added to Settings > Graphics without its description and its cost fails here.

use std::collections::BTreeSet;
use std::path::Path;

use super::graphics::{registry, Cost};

/// The page's file, and the sections it hands to other files (each with the call that draws it).
const PAGE: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/gui/pages/settings/graphics.rs"));
const CHILDREN: &[(&str, &str, &str)] = &[
    ("furniture.rs", "furniture::draw_furniture(", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/gui/pages/settings/furniture.rs"))),
    ("garments.rs", "garments::draw_garment_textures(", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/gui/pages/settings/garments.rs"))),
];

/// The calls that draw a control a person can press, drag or type into (the hint lint's, as they
/// appear on this page).
const CONTROLS: &[&str] = &[
    "widgets::toggle(",
    "widgets::labeled_slider(",
    "widgets::labeled_slider_entry(",
    ".selectable_label(",
    ".button(",
    "Button::primary(",
    "Button::secondary(",
    "egui::Button::new(",
];

fn code(line: &str) -> &str {
    line.split("//").next().unwrap_or("")
}

fn is_control(line: &str) -> bool {
    let c = code(line);
    CONTROLS.iter().any(|k| c.contains(k))
}

/// The id a `graphics_hint(.., "id")` call on this line names.
fn hint_id(line: &str) -> Option<&str> {
    let c = code(line);
    let at = c.find("graphics_hint(")?;
    let rest = &c[at..];
    let q = rest.find('"')?;
    let end = rest[q + 1..].find('"')?;
    Some(&rest[q + 1..q + 1 + end])
}

/// Every `"id"` string literal in the engine's source (src/ and crates/), the registry and this test
/// aside: the ids the engine records its measurements under.
fn recorded_ids() -> BTreeSet<String> {
    fn walk(dir: &Path, out: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("graphics_tests.rs") {
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                // (The families are matched without writing "<family>." literals here: the
                // Performance page's own scan reads every such literal in src/ as a recorded id.)
                for part in text.split('"').skip(1).step_by(2) {
                    if let Some((family, rest)) = part.split_once('.') {
                        if matches!(family, "gpu" | "cpu" | "vram") && !rest.is_empty() {
                            out.insert(part.to_string());
                        }
                    }
                }
            }
        }
    }
    let root = Path::new(env!("HUMANITY_REPO_ROOT"));
    let mut out = BTreeSet::new();
    walk(&root.join("src"), &mut out);
    walk(&root.join("crates"), &mut out);
    out
}

/// EVERY GRAPHICS CONTROL STATES ITS COST: each control on the page (a switch, a slider, a choice, a
/// button) is followed, before the next subsection or the end of its function, by `graphics_hint`
/// naming a row of data/gui/graphics_settings.ron; each section drawn by another file is called and
/// names its rows with its own graphics_hint; every row is drawn by something, names at least one way its cost is
/// known, and has its words (name, hint, the reason of a Free cost, the plain name of each measured
/// id) in data/i18n/en.json; and every measured id is one the engine records (a misspelt id would
/// read 0 and look free).
///
/// Seen red 2026-11-10 against the page as it stood (moved verbatim out of settings.rs, its
/// descriptions written into the code and no cost on any control): "113 problems" (61 controls with
/// no description and cost after them, 52 rows drawn by nothing), the first "graphics.rs:181: a
/// control with no graphics_hint after it (its description and its cost on this computer)".
#[test]
fn every_graphics_control_states_its_cost() {
    let reg = registry();
    assert!(!reg.settings.is_empty(), "data/gui/graphics_settings.ron reads");
    let mut problems: Vec<String> = Vec::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut pending: Vec<usize> = Vec::new();
    for (n, line) in PAGE.lines().enumerate() {
        let n = n + 1;
        if line.trim_start().starts_with("//") {
            continue;
        }
        if is_control(line) {
            pending.push(n);
        }
        if let Some(id) = hint_id(line) {
            if reg.row(id).is_none() {
                problems.push(format!("graphics.rs:{n}: graphics_hint names \"{id}\", which data/gui/graphics_settings.ron has no row for"));
            }
            used.insert(id.to_string());
            pending.clear();
        }
        let boundary = code(line).contains("subsection_header(") || line.starts_with('}');
        if boundary {
            for p in pending.drain(..) {
                problems.push(format!("graphics.rs:{p}: a control with no graphics_hint after it (its description and its cost on this computer)"));
            }
        }
    }
    for p in pending {
        problems.push(format!("graphics.rs:{p}: a control with no graphics_hint after it"));
    }
    // A section drawn by another file: called from the page, and its rows named by its own
    // graphics_hint calls (its controls' figures are its own lines: memory per level, download sizes).
    for (file, call, src) in CHILDREN {
        if !PAGE.contains(call) {
            problems.push(format!("graphics.rs does not draw {file}'s section ({call}..)"));
        }
        let before = used.len();
        for (n, line) in src.lines().enumerate() {
            if let Some(id) = hint_id(line) {
                if reg.row(id).is_none() {
                    problems.push(format!("{file}:{}: graphics_hint names \"{id}\", which data/gui/graphics_settings.ron has no row for", n + 1));
                }
                used.insert(id.to_string());
            }
        }
        if used.len() == before {
            problems.push(format!("{file} draws no graphics_hint: its section says nothing of its cost"));
        }
    }
    let recorded = recorded_ids();
    for row in &reg.settings {
        if !used.contains(&row.id) {
            problems.push(format!("row {}: drawn by nothing on the page (no graphics_hint names it, and no section draws its hint)", row.id));
        }
        if row.cost.is_empty() {
            problems.push(format!("row {}: names no cost", row.id));
        }
        let words_key = row.words.clone().unwrap_or_else(|| format!("graphics.{}", row.id));
        for key in [words_key, row.hint_key()] {
            if !humanity_core::i18n::has(&key) {
                problems.push(format!("row {}: {key} is not in data/i18n/en.json", row.id));
            }
        }
        for c in &row.cost {
            match c {
                Cost::Gpu(ids) | Cost::Shares(ids) | Cost::Cpu(ids) | Cost::Memory(ids) => {
                    if ids.is_empty() {
                        problems.push(format!("row {}: a measured cost with no id", row.id));
                    }
                    for id in ids {
                        if !recorded.contains(id) {
                            problems.push(format!("row {}: {id} is recorded nowhere in the engine (it would read 0 and look free)", row.id));
                        }
                        match reg.passes.get(id) {
                            Some(k) if humanity_core::i18n::has(k) => {}
                            Some(k) => problems.push(format!("passes: {id}'s name {k} is not in data/i18n/en.json")),
                            None => problems.push(format!("row {}: {id} has no plain name in `passes`", row.id)),
                        }
                    }
                }
                Cost::Free(key) if !humanity_core::i18n::has(key) => problems.push(format!("row {}: its reason {key} is not in data/i18n/en.json", row.id)),
                _ => {}
            }
        }
    }
    assert!(problems.is_empty(), "{} problems:\n  {}", problems.len(), problems.join("\n  "));
}

/// THE COST LINE SAYS HOW IT KNOWS: measured costs read the snapshot (a pass that is off says it takes
/// no time, a pass on says its milliseconds), a pass this card cannot time says so instead of reading
/// 0, nothing is claimed before the world has drawn a frame, and the other kinds say what they are.
/// Seen red 2026-11-10 with the no-timer arm taken out (the cloud row on a card without timestamp
/// queries then claimed its passes were idle, as the Performance page's 0.0 ms once did): "a card
/// without timers is told so: Cost on this computer: the cloud passes is not running now, so it
/// takes no time (measured)."
#[test]
fn the_cost_line_says_how_it_knows() {
    use humanity_core::frame_costs::{Entry, GpuTiming, Snapshot};
    let reg = registry();
    let snap = |gpu: Vec<Entry>, timing: GpuTiming, has_frames: bool| Snapshot { gpu, cpu: vec![], vram: vec![], ram: vec![], frame_ms: 8.3, gpu_timing: timing, has_frames, gpu_untimed_passes: 0 };
    let shadows = reg.row("sun_shadows").expect("a row for the sun's shadows");
    let on = super::graphics::cost_line(reg, shadows, &snap(vec![Entry { id: "gpu.shadow", value: 1.25 }], GpuTiming::Timestamps, true));
    assert!(on.contains("1.25") && on.contains("measured"), "{on}");
    let off = super::graphics::cost_line(reg, shadows, &snap(vec![], GpuTiming::Timestamps, true));
    assert!(!off.contains("0.00"), "a pass that is off says so, not 0.00 ms: {off}");
    let clouds = reg.row("cloud_quality").expect("a row for the clouds");
    let blind = super::graphics::cost_line(reg, clouds, &snap(vec![], GpuTiming::CpuFallback, true));
    assert!(blind.contains("cannot be timed"), "a card without timers is told so: {blind}");
    let menu = super::graphics::cost_line(reg, shadows, &snap(vec![], GpuTiming::Timestamps, false));
    assert!(menu.contains("in the world"), "nothing claimed before the world has drawn a frame: {menu}");
    let vsync = super::graphics::cost_line(reg, reg.row("vsync").unwrap(), &snap(vec![], GpuTiming::Timestamps, true));
    assert!(vsync.contains("8.3") && vsync.contains("120"), "the frame's time and rate: {vsync}");
}

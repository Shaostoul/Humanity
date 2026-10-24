//! The How-to page's own tests: its words, and what it draws (headless, no GPU).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::gui::theme::load_theme;
use crate::gui::GuiState;
use crate::how_to::runs::{Run, RunStore};
use crate::how_to::view::View;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every quoted literal in `text` written as `f("key"` or `f('key'`, for each call name in `calls`.
fn called_keys(text: &str, calls: &[&str]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for call in calls {
        for quote in ['"', '\''] {
            let open = format!("{call}({quote}");
            let mut rest = text;
            while let Some(i) = rest.find(&open) {
                // Only a whole call name: not the tail of a longer identifier.
                let before = rest[..i].chars().last();
                let after = &rest[i + open.len()..];
                if let Some(end) = after.find(quote) {
                    if !before.map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false) {
                        out.insert(after[..end].to_string());
                    }
                }
                rest = &rest[i + open.len()..];
            }
        }
    }
    out
}

fn page_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let dir = repo().join("src/gui/pages/how_to");
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("rs") && p.file_name().and_then(|n| n.to_str()) != Some("tests.rs") {
            out.push((p.display().to_string(), std::fs::read_to_string(&p).unwrap()));
        }
    }
    out.push(("src/gui/pages/how_to.rs".into(), std::fs::read_to_string(repo().join("src/gui/pages/how_to.rs")).unwrap()));
    out
}

/// THE PAGE'S WORDS ARE ALL IN data/how_to/words.json, AND IT HAS NONE THAT NEITHER APP USES. Both
/// apps read the one file (the desktop's w("key"), the website's W('key')), so a key one app adds
/// and the file lacks shows as a bare key on the page, and a word left in the file after both apps
/// stopped using it is a translation nobody needs. A key is used when either app's source names it
/// in quotes (some are picked from a list, `("view_short", ...)`).
#[test]
fn the_pages_words_are_all_in_the_words_file() {
    let words = super::words::parse(&std::fs::read_to_string(repo().join("data/how_to/words.json")).unwrap());
    let web = std::fs::read_to_string(repo().join("web/pages/how-to-app.js")).unwrap();
    let mut called = BTreeSet::new();
    let mut quoted = String::new();
    for (_, text) in page_sources() {
        called.extend(called_keys(&text, &["w", "wf"]));
        quoted.push_str(&text);
    }
    called.extend(called_keys(&web, &["W", "WF"]));
    quoted.push_str(&web);
    // The nav button is the page's title, in escape_menu.rs.
    quoted.push_str(&std::fs::read_to_string(repo().join("src/gui/pages/escape_menu.rs")).unwrap());
    let missing: Vec<&String> = called.iter().filter(|k| !words.contains_key(*k)).collect();
    assert!(missing.is_empty(), "keys the page uses that data/how_to/words.json does not have: {missing:?}");
    let unused: Vec<&String> = words.keys().filter(|k| !quoted.contains(&format!("\"{k}\"")) && !quoted.contains(&format!("'{k}'"))).collect();
    assert!(unused.is_empty(), "words neither app uses (take them out of data/how_to/words.json): {unused:?}");
    assert!(called.len() > 30, "the scanner found the calls: {}", called.len());
}

fn ctx_and_state(view: View) -> (egui::Context, GuiState) {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let mut state = GuiState::default();
    state.how_to.store = RunStore::new(Path::new("target").join("no_such_how_to_runs_test"));
    state.how_to.load_now(&repo().join("data"));
    state.settings.how_to_view = view;
    (ctx, state)
}

fn drawn(ctx: &egui::Context, state: &mut GuiState) -> Vec<egui::epaint::ClippedShape> {
    let mut theme = load_theme();
    theme.apply_to_egui(ctx);
    let mut shapes = Vec::new();
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 6000.0))), ..Default::default() };
        shapes = ctx.run(input, |ctx| super::draw(ctx, &theme, state)).shapes;
    }
    shapes
}

fn has(shapes: &[egui::epaint::ClippedShape], text: &str) -> bool {
    crate::gui::screen_surface::find_text_in_shapes(shapes, text).is_some()
}

/// WHAT THE PAGE DRAWS FOR A LETHAL HOW-TO, IN BOTH VIEWS. The Stop and call box and every
/// serious or lethal safety line are on the page in Short as in Full; Short leaves out a step's
/// reasons and Full shows them.
#[test]
fn short_and_full_both_show_the_safety_and_only_full_the_reasons() {
    for view in [View::Short, View::Full] {
        let (ctx, mut state) = ctx_and_state(view);
        state.how_to.open("mix_ors_packet");
        let shapes = drawn(&ctx, &mut state);
        assert!(has(&shapes, "Stop and call someone if"), "{view:?}: the stop box");
        assert!(has(&shapes, "Use exactly the amount of water the packet says."), "{view:?}: Short shows the lethal line at the step");
        assert!(has(&shapes, "Can kill you if done wrong"), "{view:?}: the hazard in words");
        let reason = "Packets are made for different amounts of water";
        assert_eq!(has(&shapes, reason), view == View::Full, "{view:?}: a step's reason is Full's alone");
    }
}

/// A RUN IN PROGRESS SHOWS WHERE IT IS: Doing now in the rail, "Step n of m", the current step's
/// text, and the problem a No pointed at. Nothing is written: the run is made in memory.
#[test]
fn a_run_in_progress_shows_where_it_is() {
    let (ctx, mut state) = ctx_and_state(View::Full);
    let lib = state.how_to.lib.clone().unwrap();
    let l = lib.find("change_furnace_filter").unwrap();
    let mut run = Run::start(&l.how_to, &l.hash, 1_792_789_331_000);
    run.set_done("power_off", true, 1_792_789_340_000);
    run.answer(&l.how_to, "arrow", false, 1_792_789_350_000);
    state.how_to.runs = vec![run];
    state.how_to.open("change_furnace_filter");
    let shapes = drawn(&ctx, &mut state);
    assert!(has(&shapes, "Doing now"), "the rail lists the open run");
    assert!(has(&shapes, "Step 2 of"), "where the run is");
    assert!(has(&shapes, "No: see what to do"), "the check's answer");
    assert!(has(&shapes, "The arrow on the filter points away from the furnace."), "its problem");
}

/// The page's choices survive a restart: Short or Full, How this page works, the split.
#[test]
fn the_pages_settings_survive_a_restart() {
    let mut state = GuiState::default();
    state.settings.how_to_view = View::Short;
    state.settings.how_to_how_open = true;
    state.settings.how_to_layout = vec![0.3, 0.3, 0.4];
    let saved = serde_json::to_string(&crate::config::AppConfig::from_gui_state(&state)).unwrap();
    let back: crate::config::AppConfig = serde_json::from_str(&saved).unwrap();
    let mut fresh = GuiState::default();
    back.apply_to_gui_state(&mut fresh);
    assert_eq!(fresh.settings.how_to_view, View::Short);
    assert!(fresh.settings.how_to_how_open);
    assert_eq!(fresh.settings.how_to_layout, vec![0.3, 0.3, 0.4]);
}

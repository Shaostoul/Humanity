//! Every window fits the visible area of a phone (window_fit.rs, 2026-11-23). Each test was seen to
//! fail before it passed; what it saw red is in its comment.

use crate::gui::visible_area::{self, Covered};
use crate::gui::GuiState;

/// The operator's phone, in points (a OnePlus 9 Pro, 2026-11-23).
const PHONE: egui::Vec2 = egui::vec2(412.0, 915.0);

/// Android's status bar and navigation bar on that phone, as the frame loop takes them off the
/// screen (gui/visible_area.rs): the visible area is what is left between them.
const BARS: Covered = Covered { left: 0.0, top: 24.0, right: 0.0, bottom: 48.0 };

fn headless() -> (egui::Context, crate::gui::theme::Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

/// Run `frames` headless frames (no GPU) of `draw` on a `screen` with `covered` taken off it; the
/// visible area of the last frame.
fn run(ctx: &egui::Context, screen: egui::Vec2, covered: Covered, frames: usize, mut draw: impl FnMut(&egui::Context)) -> egui::Rect {
    let mut visible = egui::Rect::NOTHING;
    for _ in 0..frames {
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)), ..Default::default() };
        visible_area::apply(ctx, &mut input, covered);
        let _ = ctx.run(input, |ctx| {
            visible = ctx.screen_rect();
            draw(ctx);
        });
    }
    visible
}

/// The window `id`'s whole rectangle (its frame included) as egui placed it last frame.
fn window_rect(ctx: &egui::Context, id: &str) -> egui::Rect {
    ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap_or_else(|| panic!("the window {id:?} was drawn"))
}

/// `window` lies inside `visible` (half a point of rounding allowed) and, when it is centred,
/// stays centred in it.
fn assert_inside(what: &str, window: egui::Rect, visible: egui::Rect, centred: bool) {
    let inside = visible.expand(0.5).contains_rect(window);
    assert!(inside, "the {what} window {window:?} is not inside the visible area {visible:?}");
    if centred {
        let off = (window.center().x - visible.center().x).abs();
        assert!(off < 1.0, "the {what} window {window:?} is not centred in the visible area {visible:?} ({off} points off)");
    }
}

/// THE ATTACH WINDOW ON A PHONE (operator, 2026-11-23, his OnePlus 9 Pro with v0.1571.0: "I can't
/// upload images because the windows aren't scaling to the phone"; his screenshot shows "Attach
/// files: choose one or more" running off both edges). At 412 x 915 with the phone's bars, the
/// picker (the one the phone preview and every other picker on a phone still shows; Android's own
/// Attach uses the system's document picker, chat/attach_android.rs) lies inside the visible area
/// and stays centred in it.
///
/// Seen red 2026-11-23 before the fit (window_fit.rs): "the Attach window [[-211.9 99.8] -
/// [623.9 791.3]] is not inside the visible area [[0.0 24.0] - [412.0 867.0]]".
#[test]
fn the_attach_window_fits_a_phone() {
    use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerState, PICKER_ID};
    let (ctx, theme) = headless();
    let mut picker = FilePickerState::new(&[], 500 * 1024 * 1024).with_pick_verb("Attach").multiple();
    let visible = run(&ctx, PHONE, BARS, 4, |ctx| {
        file_picker_modal(ctx, &theme, &mut picker, "Attach files: choose one or more");
    });
    assert_inside("Attach", window_rect(&ctx, PICKER_ID), visible, true);
    // And with the keyboard up (the picker's path box typed in): it shrinks to what is left.
    let keyboard = Covered { bottom: 343.0, ..BARS };
    let visible = run(&ctx, PHONE, keyboard, 4, |ctx| {
        file_picker_modal(ctx, &theme, &mut picker, "Attach files: choose one or more");
    });
    assert_inside("Attach (keyboard up)", window_rect(&ctx, PICKER_ID), visible, false);
}

/// The passphrase window on a phone, the set-new form (the tallest of its forms, with its
/// requirements and the remember choice): inside the visible area with the bars, and with the
/// keyboard up, its body scrolling in the window rather than running past the keyboard.
///
/// Seen red 2026-11-23 before the fit, already with only the bars: "the passphrase window [[-10.6
/// 262.4] - [422.5 628.7]] is not inside the visible area [[0.0 24.0] - [412.0 867.0]]" (its own
/// width left out part of its frame).
#[test]
fn the_passphrase_window_fits_a_phone() {
    let (ctx, theme) = headless();
    let mut state = GuiState::default();
    state.passphrase_needed = true;
    state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
    let visible = run(&ctx, PHONE, BARS, 4, |ctx| crate::gui::widgets::passphrase_modal::draw(ctx, &theme, &mut state));
    assert_inside("passphrase", window_rect(&ctx, "Passphrase Required"), visible, true);
    let keyboard = Covered { bottom: 391.0, ..BARS };
    let visible = run(&ctx, PHONE, keyboard, 4, |ctx| crate::gui::widgets::passphrase_modal::draw(ctx, &theme, &mut state));
    assert_inside("passphrase (keyboard up)", window_rect(&ctx, "Passphrase Required"), visible, false);
}

/// A dialog (widgets/dialog.rs, which the settings and confirm dialogs use) with a wide row of
/// buttons and more lines than a phone shows: inside the visible area, centred, its end reachable
/// by scrolling inside the window.
///
/// Seen red 2026-11-23 before the fit: "the dialog window [[13.4 -281.8] - [398.6 1172.8]] is not
/// inside the visible area [[0.0 24.0] - [412.0 867.0]]".
#[test]
fn a_tall_wide_dialog_fits_a_phone() {
    let (ctx, theme) = headless();
    let mut open = true;
    let visible = run(&ctx, PHONE, BARS, 4, |ctx| {
        crate::gui::widgets::dialog(ctx, &theme, "fit_test_dialog", "A confirm dialog", &mut open, |ui| {
            ui.horizontal(|ui| {
                for word in ["Remove", "Keep", "Ask me later", "Tell me more", "Cancel"] {
                    let _ = ui.button(word);
                }
            });
            for i in 0..60 {
                ui.label(format!("Line {i} of a long confirmation"));
            }
        });
    });
    assert_inside("dialog", window_rect(&ctx, "fit_test_dialog"), visible, true);
}

// The first-run card with the recovery phrase showing (the onboarding window) is checked beside
// its other phone scenes: pages/main_menu_restore_tests.rs, the_recovery_phrase_card_fits_a_phone.

/// EVERY WINDOW IS SHOWN FITTED: after each `layers::window(` under src/gui, the window is shown
/// with `.show_fitted(`, never egui's own `.show(` (which leaves it as wide and tall as a desktop
/// made it). A new window that skips the fit fails here, naming its file.
///
/// Seen red 2026-11-23: the same check run over the tree before the call sites were switched found
/// 20 windows in 16 files shown with `.show(`, the first "src/gui/first_minutes.rs".
#[test]
fn every_window_is_shown_fitted() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let mut files = Vec::new();
    walk(&root.join("src/gui"), &mut files);
    let mut problems = Vec::new();
    let mut windows = 0;
    for f in files {
        let name = f.strip_prefix(root).unwrap_or(&f).display().to_string().replace('\\', "/");
        if name.ends_with("_tests.rs") || name.ends_with("/layers.rs") || name.ends_with("/window_fit.rs") {
            continue;
        }
        let src = std::fs::read_to_string(&f).unwrap();
        let mut at = 0;
        while let Some(i) = src[at..].find("layers::window(") {
            let start = at + i;
            at = start + 1;
            let line = &src[src[..start].rfind('\n').map_or(0, |n| n + 1)..start];
            if line.contains("//") {
                continue;
            }
            windows += 1;
            let rest = &src[start..];
            let plain = rest.find(".show(").unwrap_or(usize::MAX);
            let fitted = rest.find(".show_fitted(").unwrap_or(usize::MAX);
            if plain < fitted {
                problems.push(format!("{name}: a window made by layers::window is shown with .show( instead of .show_fitted("));
            }
        }
    }
    assert!(windows >= 20, "found only {windows} windows: has the way windows are made changed?");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

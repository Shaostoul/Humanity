//! Settings > Safety's content-policy section (safety_content_policy.rs): its button is a real,
//! clickable control (a synthetic pointer click through egui, headless, the way
//! gui/bottom_strip/mute_tests.rs clicks the warning bar), and the click opens the Library on the
//! policy the shipped data/library/ holds. The website's twin is
//! scripts/tests/content-policy-web.test.js.
//!
//! RED FIRST (2026-11-20): with CONTENT_POLICY_SLUG spelled "content_policy" (a file name's
//! underscore, not the slug), "the Library is open on the content policy" failed with
//! left: None, right: Some("content_policy"); with the click not acted on (`if pressed.inner` made
//! `if false && pressed.inner`), "the click opened the Library" failed with left: Settings, right: Library.

use std::cell::Cell;

use super::*;

thread_local! {
    /// Where the button was laid out on the last frame (a test has no cursor, and egui has no
    /// query by content).
    static BUTTON: Cell<Option<egui::Rect>> = const { Cell::new(None) };
}

/// Note where the button was laid out (called by the drawing code under `cfg(test)`).
pub(crate) fn record_button(rect: egui::Rect) {
    BUTTON.with(|b| b.set(Some(rect)));
}

const SCREEN: egui::Rect = egui::Rect { min: egui::pos2(0.0, 0.0), max: egui::pos2(1280.0, 720.0) };

/// One frame of the section alone, with these events.
fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) {
    let input = egui::RawInput { screen_rect: Some(SCREEN), events, ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw_section(ui, theme, state, theme.info()));
    });
}

#[test]
fn read_the_content_policy_is_a_real_click_that_opens_it_in_the_library() {
    let theme = crate::gui::theme::load_theme();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    theme.apply_to_egui(&ctx);

    let mut state = GuiState::default();
    state.onboarding_complete = true;
    state.active_page = GuiPage::Settings;
    state.library = crate::gui::load_library(std::path::Path::new("data")).sections;

    BUTTON.with(|b| b.set(None));
    for _ in 0..2 {
        frame(&ctx, &theme, &mut state, Vec::new());
    }
    let at = BUTTON.with(|b| b.get()).expect("the section drew its button").center();
    assert_eq!(state.active_page, GuiPage::Settings, "drawing alone opens nothing");

    let m = egui::Modifiers::default();
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(at)]);
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);

    assert_eq!(state.active_page, GuiPage::Library, "the click opened the Library");
    assert_eq!(
        crate::gui::pages::library::shown_doc(&state).as_deref(),
        Some(CONTENT_POLICY_SLUG),
        "the Library is open on the content policy"
    );
}

#[test]
fn the_sections_words_are_en_json_keys() {
    for key in ["content_policy.heading", "content_policy.body", "content_policy.open", "content_policy.open_hint"] {
        let words = humanity_core::i18n::t_dyn(key);
        assert_ne!(words, key, "data/i18n/en.json has no {key}");
    }
}

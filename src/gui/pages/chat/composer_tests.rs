//! The composer grows upward and stays on screen (operator, 2026-10-10: "when I type a long
//! message into the chat bar, instead of moving upwards as I add a new line, the text box expands
//! downwards and offscreen, which makes reviewing long posts impossible").

use super::*;

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), ..Default::default() };
    let _ = ctx.run(input, |ctx| draw(ctx, theme, state));
}

/// What of the composer can be seen and used: its rect within the scroll area's clip.
fn composer(ctx: &egui::Context) -> egui::Rect {
    ctx.read_response(egui::Id::new("chat_composer_input")).expect("the composer is drawn").interact_rect
}

/// A LONG POST GROWS THE COMPOSER UPWARD AND STAYS ON SCREEN: six lines raise the box's top and
/// keep its bottom on screen; two hundred lines stay on screen too, the box held to about the
/// web's 45% of the chat area with the text scrolling inside. Seen red 2026-10-10 with the bar
/// held at its old fixed 52 px: "and the box grew upward" (the box sat clipped at the window's
/// bottom edge, 12 px of it visible, the rest below the window).
#[test]
fn a_long_post_grows_the_composer_upward_and_stays_on_screen() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    for _ in 0..3 {
        frame(&ctx, &theme, &mut state);
    }
    let one = composer(&ctx);

    state.chat_input = (1..=6).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    for _ in 0..4 {
        frame(&ctx, &theme, &mut state);
    }
    let six = composer(&ctx);
    assert!(six.bottom() <= SCREEN.y + 0.5, "six lines stay on screen: {one:?} then {six:?}");
    assert!(six.top() < one.top() - 50.0, "and the box grew upward: {one:?} then {six:?}");

    state.chat_input = (1..=200).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    for _ in 0..4 {
        frame(&ctx, &theme, &mut state);
    }
    let long = composer(&ctx);
    assert!(long.bottom() <= SCREEN.y + 0.5, "two hundred lines stay on screen: {long:?}");
    assert!(long.height() <= SCREEN.y * 0.5, "the box is held near 45% of the chat area, scrolling inside: {long:?}");
}

/// ONE LONG LINE THAT WRAPS GROWS IT THE SAME WAY (the operator's screenshot, 2026-10-10: one
/// paragraph with no line breaks, wrapped to three rows, of which the third was below the window).
/// The box counts its wrapped rows, not its line breaks.
#[test]
fn one_long_wrapping_line_grows_the_composer_upward_too() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    for _ in 0..3 {
        frame(&ctx, &theme, &mut state);
    }
    let one = composer(&ctx);
    state.chat_input = "This is the typing bar going beyond its limit. You can see row 1 and 2 but, row 3 is hidden on the desktop app. ".repeat(6);
    assert!(!state.chat_input.contains('\n'), "one line, no breaks");
    for _ in 0..4 {
        frame(&ctx, &theme, &mut state);
    }
    let wrapped = composer(&ctx);
    assert!(wrapped.bottom() <= SCREEN.y + 0.5, "the wrapped paragraph stays on screen: {wrapped:?}");
    assert!(wrapped.top() < one.top() - 30.0, "and the box grew upward to show its rows: {one:?} then {wrapped:?}");
}

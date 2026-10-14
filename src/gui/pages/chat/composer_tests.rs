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

/// THE COMPOSER'S HINTS OPEN ABOVE IT (operator, 2026-10-11: the Attach hint, still up when the
/// file picker closed, covered the chat bar). Resting the pointer on each button to the right of
/// the text box in turn, the first hint that opens sits wholly above the box and does not
/// overlap it. Seen red 2026-10-11 with the hint above never shown (`show_tip_above` returning
/// at once): "no hint opened above the composer".
#[test]
fn the_composers_hints_open_above_it() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    for _ in 0..3 {
        frame(&ctx, &theme, &mut state);
    }
    let boxed = composer(&ctx);
    let y = boxed.center().y;
    let mut t = 0.0;
    let mut x = boxed.right() + 6.0;
    let mut found = None;
    while x < SCREEN.x - 4.0 && found.is_none() {
        // Move onto the spot, then rest there past the tooltip delay.
        for step in 0..4 {
            t += if step == 0 { 0.01 } else { 0.4 };
            let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), time: Some(t), ..Default::default() };
            if step == 0 {
                input.events.push(egui::Event::PointerMoved(egui::pos2(x, y)));
            }
            let _ = ctx.run(input, |ctx| draw(ctx, &theme, &mut state));
        }
        found = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(crate::gui::widgets::button::TIP_ABOVE_LAST)));
        x += 8.0;
    }
    let tip = found.expect("no hint opened above the composer");
    assert!(tip.bottom() <= boxed.top(), "the hint sits above the text box: hint {tip:?}, box {boxed:?}");
    assert!(!tip.intersects(boxed), "and does not overlap it: hint {tip:?}, box {boxed:?}");
}

/// THE BAR SETTLES AND SHRINKS BACK (operator, 2026-10-14: "as I type more than 4 rows the area
/// housing the text input box expands"). v0.1499.0 measured the bar's frame, which stretches to
/// fill the space reserved for it, so every frame read the reservation back as the bar's height:
/// a long post grew the bar until it filled 60% of the chat, and it never came back. Now the bar
/// measures what it drew. With the text unchanged the box stays put, and with the text cleared it
/// returns to where one line sat.
///
/// Seen red 2026-10-14 with the measurement taken from the space given (`ui.max_rect()`), the
/// v0.1499.0 behaviour: "the bar stopped growing once the text stopped changing".
#[test]
fn the_bar_settles_and_shrinks_back_when_the_text_does() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    for _ in 0..4 {
        frame(&ctx, &theme, &mut state);
    }
    let one = composer(&ctx);

    state.chat_input = (1..=6).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    for _ in 0..6 {
        frame(&ctx, &theme, &mut state);
    }
    let settled = composer(&ctx);
    // Every one of the six lines is in view: the box is about six times one line's height, not
    // held to the few lines its row had room for. Seen red 2026-10-14 with the box placed straight
    // in the row (v0.1500.0): "all six lines are in view: the text is [[249.0 705.0] - [833.8 793.0]],
    // the box shows [[249.0 705.0] - [833.8 772.0]]".
    let full = ctx.read_response(egui::Id::new("chat_composer_input")).expect("drawn").rect;
    assert!(settled.height() >= full.height() - 0.5, "all six lines are in view: the text is {full:?}, the box shows {settled:?}");
    for _ in 0..10 {
        frame(&ctx, &theme, &mut state);
    }
    let later = composer(&ctx);
    assert!((later.top() - settled.top()).abs() < 1.0, "the bar stopped growing once the text stopped changing: {settled:?} then {later:?}");

    state.chat_input.clear();
    for _ in 0..6 {
        frame(&ctx, &theme, &mut state);
    }
    let back = composer(&ctx);
    assert!((back.top() - one.top()).abs() < 1.0, "and with the text cleared it is back where one line sat: {one:?} then {back:?}");
}


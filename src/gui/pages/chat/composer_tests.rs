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
    state.chat_active_channel = "global".into();
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
    state.chat_active_channel = "global".into();
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
    state.chat_active_channel = "global".into();
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
    state.chat_active_channel = "global".into();
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


/// THE BOX YOU TYPE IN STANDS ABOVE THE UNFINISHED-SOFTWARE WARNING, WITH THE LINE ABOVE IT
/// (operator, 2026-11-15, v0.1561.0, on a Commons room: "I can't see the chat bar to type in for
/// some reason", and "the chat bar is even off screen when I mute the wip notice"). Drawn as the
/// app draws a frame: the warning first (gui/bottom_strip.rs, a bottom panel added before the page),
/// then the chat page, on #fire-disciples, a Commons room anyone can read, as an admin (so the line
/// above the box says only who can read it). The whole box, and the line, sit above the warning's
/// top edge; with the warning muted (not drawn), above the window's bottom edge.
///
/// The cause was not the warning: the bar recorded the text box's height as the scroll area's
/// `inner_rect` (egui's minimum scrolled height, 64 px) instead of the 21 px it drew, so the bar's
/// other parts read 64 px short. Seen red 2026-11-15 with that measurement back: "the whole box
/// stands above the warning (its top at 776): drawn [[249.0 786.0] - [827.5 807.0]]".
#[test]
fn the_box_stands_above_the_warning_with_its_line() {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.go(crate::gui::GuiPage::Chat, crate::gui::How::Replace);
    state.profile_public_key = "my_key".into();
    state.chat_active_channel = "commons:fire-disciples".into();
    state.chat_channels.push(crate::gui::ChatChannel {
        id: "fire-disciples".into(),
        name: "fire-disciples".into(),
        read_level: humanity_protocol::chat_access::ReadLevel::Anyone,
        ..Default::default()
    });
    state.chat_users.push(crate::gui::ChatUser { name: "me".into(), public_key: "my_key".into(), role: "admin".into(), status: "online".into() });
    // The warning's height is noted for the frame it drew in, so it is read inside the frame.
    let strip = std::cell::Cell::new(0.0_f32);
    let run = |ctx: &egui::Context, state: &mut GuiState| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), ..Default::default() };
        ctx.run(input, |ctx| {
            crate::gui::bottom_strip::draw_for_version(ctx, &theme, state, "0.1561.0");
            strip.set(crate::gui::bottom_strip::height(ctx));
            draw(ctx, &theme, state);
        })
    };
    for _ in 0..4 {
        run(&ctx, &mut state);
    }
    let out = run(&ctx, &mut state);
    let strip = strip.get();
    assert!(strip > 10.0, "the warning is drawn (a 0.x version): {strip}");
    let strip_top = SCREEN.y - strip;
    let line = crate::gui::screen_surface::find_text_in_shapes(&out.shapes, humanity_core::i18n::t("chat.access_read_anyone"))
        .expect("the line above the box says who can read the room");
    let r = ctx.read_response(egui::Id::new("chat_composer_input")).expect("the box is drawn");
    let seen = r.interact_rect;
    assert!(
        seen.height() >= r.rect.height() - 0.5 && r.rect.bottom() <= strip_top + 0.5,
        "the whole box stands above the warning (its top at {strip_top}): drawn {:?}, visible {seen:?}",
        r.rect
    );
    assert!(line.rect.bottom() <= r.rect.top() + 0.5, "the line sits above the box: {:?} / {:?}", line.rect, r.rect);

    // Muted: no warning, the chat to the window's bottom edge, the box still wholly on screen.
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    for _ in 0..5 {
        frame(&ctx, &theme, &mut state);
    }
    let r = ctx.read_response(egui::Id::new("chat_composer_input")).expect("the box is drawn");
    assert!(
        r.interact_rect.height() >= r.rect.height() - 0.5 && r.rect.bottom() <= SCREEN.y + 0.5,
        "with the warning muted the whole box is on screen: drawn {:?}, visible {:?}",
        r.rect,
        r.interact_rect
    );
}

/// THE TYPING MARKS ASK FOR THEIR OWN FRAMES (2026-11-20, Android rung A1): "Ann is typing…" leaves
/// three seconds after the last typing event, and a phone draws only when something asks
/// (engine/redraw.rs), so while the line shows the composer asks for a frame within half a second,
/// and once it has gone it asks for none of its own. Seen red 2026-11-20 with the composer's
/// `request_repaint_after` taken out: "while someone types, egui wants the next frame in
/// 18446744073709551615999 ms" (Duration::MAX: never, so the line would stay until the next touch).
#[test]
fn the_typing_marks_ask_for_frames_while_they_show() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let mut state = GuiState::default();
    state.chat_active_channel = "global".into();
    // A second between frames, so egui's own short animations have finished before anything is
    // judged: only a request that stays is left.
    let mut t = 0.0;
    let mut run = |state: &mut GuiState| {
        t += 1.0;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), time: Some(t), ..Default::default() };
        let out = ctx.run(input, |ctx| draw(ctx, &theme, state));
        out.viewport_output.get(&egui::ViewportId::ROOT).map_or(std::time::Duration::MAX, |v| v.repaint_delay)
    };
    for _ in 0..5 {
        run(&mut state);
    }
    let quiet = run(&mut state);
    assert!(quiet >= std::time::Duration::from_secs(1), "nobody typing: egui wants the next frame in {} ms", quiet.as_millis());
    state.chat_typing_users.insert("ann0".into(), ("Ann".into(), std::time::Instant::now()));
    // The line's arrival and departure change the layout, which egui settles over a frame or two of
    // its own: judged after them, so only the typing marks' own request is left.
    run(&mut state);
    run(&mut state);
    let typing = run(&mut state);
    assert!(typing <= std::time::Duration::from_millis(500), "while someone types, egui wants the next frame in {} ms", typing.as_millis());
    let long_ago = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(4)).expect("the clock is past four seconds");
    state.chat_typing_users.insert("ann0".into(), ("Ann".into(), long_ago));
    run(&mut state); // the line is pruned in this frame
    run(&mut state);
    run(&mut state);
    let gone = run(&mut state);
    assert!(state.chat_typing_users.is_empty(), "the old typing mark was pruned");
    assert!(gone >= std::time::Duration::from_secs(1), "after the line went, egui wants the next frame in {} ms", gone.as_millis());
}

// ── On a phone: above its navigation bar and keyboard (2026-11-22, gui/visible_area.rs) ──

use crate::gui::visible_area::{apply, Covered};

/// The operator's phone, in points (2026-11-22).
const PHONE: egui::Vec2 = egui::vec2(412.0, 915.0);

/// One headless frame (no GPU) of the Chat page laid out as on a phone, an open room (chat/phone.rs:
/// one column at a time; called directly, since the phone preview is a switch every test shares),
/// with `covered` taken off the screen as the frame loop does (nothing when `ignore`); the output.
fn chat_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, covered: Covered, ignore: bool) -> egui::FullOutput {
    let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, PHONE)), ..Default::default() };
    apply(ctx, &mut input, if ignore { Covered::default() } else { covered });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("chat_phone_shows_rooms"), false));
    ctx.run(input, |ctx| super::phone::columns(ctx, theme, state))
}

/// The chat's text box and its attach button with `covered` (or with it ignored).
fn chat_composer(covered: Covered, ignore: bool) -> (egui::Rect, egui::Rect) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = GuiState::default();
    state.chat_active_channel = "global".into();
    let mut out = None;
    for _ in 0..4 {
        out = Some(chat_frame(&ctx, &theme, &mut state, covered, ignore));
    }
    let out = out.expect("frames ran");
    let text_box = ctx
        .read_response(egui::Id::new(composer::INPUT_ID))
        .expect("the chat's text box was drawn")
        .rect;
    let attach = humanity_core::i18n::t("chat.attach");
    let found = crate::gui::screen_surface::find_text_in_shapes(&out.shapes, attach).filter(|f| f.text == attach);
    let attach = found
        .unwrap_or_else(|| {
            // What the bottom of the page drew instead, for the failure's words.
            let mut near = Vec::new();
            for cs in &out.shapes {
                if let egui::Shape::Text(t) = &cs.shape {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    if r.top() > text_box.top() - 80.0 {
                        near.push(format!("{:?} at {r:?} clip {:?}", t.galley.text(), cs.clip_rect));
                    }
                }
            }
            panic!("no {attach:?} button drawn; near the text box {text_box:?}: {near:#?}")
        })
        .rect;
    (text_box, attach)
}

/// On the phone the chat's text box and its attach button sit above the navigation bar's buttons
/// (the operator, 2026-11-22: "I can't click the attach button cause of the center button that
/// takes me back to my phones homepage"), and above the keyboard once it is up.
///
/// Seen red 2026-11-22 with the edges ignored (the app as it was): the control below, the text
/// box's bottom at the window's bottom, under the navigation bar.
#[test]
fn the_chat_composer_sits_above_the_phones_buttons_and_keyboard() {
    for (what, cover) in [
        ("the navigation bar", Covered { left: 0.0, top: 24.0, right: 0.0, bottom: 48.0 }),
        ("the keyboard", Covered { left: 0.0, top: 24.0, right: 0.0, bottom: 330.0 }),
    ] {
        let bottom = PHONE.y - cover.bottom;
        let (text_box, attach) = chat_composer(cover, false);
        assert!(text_box.bottom() <= bottom + 0.5, "the chat's text box at {text_box:?} is under {what} (from {bottom})");
        assert!(attach.bottom() <= bottom + 0.5, "the attach button at {attach:?} is under {what} (from {bottom})");
        assert!(text_box.top() >= cover.top && attach.top() >= cover.top, "under the status bar: {text_box:?} {attach:?}");
        // The control: ignored, the same edges leave the box under them.
        let (ignored, ignored_attach) = chat_composer(cover, true);
        assert!(
            ignored.bottom() > bottom || ignored_attach.bottom() > bottom,
            "the scene does not need the fix for {what}: ignored, the box sits at {ignored:?}"
        );
    }
}

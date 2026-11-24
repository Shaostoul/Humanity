//! The Chat page's first phone cleanups (2026-11-23; the full phone layout is planned in
//! docs/design/chat-layout.md, "The phone layout"). The operator, 2026-11-23, on his OnePlus 9 Pro: "the
//! chat page UI needs work ... the go live and open studio buttons could be hidden in the studio page
//! instead of also being in the chat page." Each test was seen to fail before it passed; what it saw
//! red is in its comment.

use super::*;

/// The operator's phone, in points.
const PHONE: egui::Vec2 = egui::vec2(412.0, 915.0);
const DESKTOP: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn headless() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

/// A few headless frames (no GPU) of `draw` on a `screen`; the last frame's output.
fn frames(ctx: &egui::Context, screen: egui::Vec2, mut draw: impl FnMut(&egui::Context)) -> egui::FullOutput {
    let mut out = None;
    for _ in 0..4 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)), ..Default::default() };
        out = Some(ctx.run(input, |ctx| draw(ctx)));
    }
    out.expect("frames ran")
}

/// Where the text exactly `text` is drawn, if it is.
fn drawn(out: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    crate::gui::screen_surface::find_text_in_shapes(&out.shapes, text).filter(|f| f.text == text).map(|f| f.rect)
}

/// Where a text beginning with `start` is drawn (as much of it as shows), if it is.
fn drawn_starting(out: &egui::FullOutput, start: &str) -> Option<egui::Rect> {
    fn walk(shape: &egui::Shape, start: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(t) if t.galley.text().starts_with(start) => Some(t.galley.rect.translate(t.pos.to_vec2())),
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, start)),
            _ => None,
        }
    }
    out.shapes.iter().find_map(|cs| walk(&cs.shape, start))
}

/// GO LIVE AND OPEN STUDIO ARE THE STUDIO PAGE'S: the Chat page draws neither, on a desktop or on a
/// phone (the rooms list or a room open; the page's `draw` is the same above its columns on both, so
/// the phone's columns are drawn directly, the phone preview being a switch every test shares). Going live, its status and the stream's address are on the
/// Studio page (pages/studio.rs); the streams live now are listed on the Watch page.
///
/// Seen red 2026-11-23 with a panel across the top of the page drawing the strip's Go Live button
/// as the strip did (`widgets::Button::primary("Go Live")`): "the Chat page on a desktop draws
/// \"Go Live\"".
#[test]
fn the_chat_page_has_no_go_live_or_open_studio() {
    let (ctx, theme) = headless();
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    let desktop = frames(&ctx, DESKTOP, |ctx| draw(ctx, &theme, &mut state));
    for word in ["Go Live", "Open Studio", "End Stream"] {
        assert!(drawn(&desktop, word).is_none(), "the Chat page on a desktop draws {word:?}");
    }
    for rooms in [true, false] {
        let (ctx, theme) = headless();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("chat_phone_shows_rooms"), rooms));
        let phone = frames(&ctx, PHONE, |ctx| {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("chat_phone_shows_rooms"), rooms));
            super::phone::columns(ctx, &theme, &mut state);
        });
        for word in ["Go Live", "Open Studio", "End Stream"] {
            assert!(drawn(&phone, word).is_none(), "the Chat page on a phone (rooms list {rooms}) draws {word:?}");
        }
    }
}

/// THE CHANNEL'S DESCRIPTION NEVER RUNS UNDER COPY LINK: on a 412-point phone, a room whose
/// description is longer than the header shows it cut short with an ellipsis before Copy link, and
/// Copy link stays whole on the screen.
///
/// Seen red 2026-11-23 with the description laid out at its full length (not cut short): "the
/// description [[133.2 51.5] - [888.9 67.5]] runs under Copy link [[318.4 46.0] - [387.0 68.0]]".
#[test]
fn the_channel_description_stops_before_copy_link() {
    let (ctx, theme) = headless();
    let mut state = GuiState::default();
    state.chat_channels = vec![crate::gui::state_types::ChatChannel {
        id: "general".into(),
        name: "general".into(),
        description: "Talk about anything here: introductions, questions, what you are growing this season, and the projects you are building with everyone".into(),
        ..Default::default()
    }];
    state.chat_active_channel = "general".into();
    let out = frames(&ctx, PHONE, |ctx| {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("chat_phone_shows_rooms"), false));
        super::phone::columns(ctx, &theme, &mut state);
    });
    let copy = humanity_core::i18n::t("chat.copy_link");
    let link = drawn(&out, copy).unwrap_or_else(|| panic!("{copy:?} is drawn"));
    let desc = drawn_starting(&out, "  |  Talk about anything").expect("the description is drawn");
    assert!(desc.right() <= link.left() + 0.5, "the description {desc:?} runs under Copy link {link:?}");
    assert!(link.right() <= PHONE.x + 0.5, "Copy link {link:?} runs off the {}-point screen", PHONE.x);
}

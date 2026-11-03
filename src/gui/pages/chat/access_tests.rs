//! The chat access levels in the desktop app's chat (2026-11-02): the line above the box you type
//! in says who can read the channel, its glossary word opens the Verified card, and a poster below
//! Verified has their links drawn as plain words.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use humanity_protocol::chat_access::{PostLevel, Rank, ReadLevel};

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 800.0);
const ME: &str = "my_key";

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)), events, ..Default::default() };
    ctx.run(input, |ctx| super::super::draw(ctx, theme, state))
}

/// The chat on #`channel` (reading at `read`, posting at `post`), signed in as a member whose role
/// is `role`.
fn chat_on(channel: &str, read: ReadLevel, post: PostLevel, role: &str) -> GuiState {
    let mut state = GuiState::default();
    state.profile_public_key = ME.into();
    state.chat_active_channel = channel.into();
    state.chat_channels.push(crate::gui::ChatChannel { id: channel.into(), name: channel.into(), read_level: read, post_level: post, ..Default::default() });
    state.chat_users.push(crate::gui::ChatUser { name: "me".into(), public_key: ME.into(), role: role.into(), status: "online".into() });
    state
}

/// THE WORDS FOR EACH LEVEL AND RANK: who reads it always, then why you cannot post, or the
/// one-link rule below Verified, or nothing.
#[test]
fn the_line_says_who_reads_and_what_you_may_post() {
    assert_eq!(audience_words(ReadLevel::Anyone, PostLevel::Members, Rank::Verified), (t("chat.access_read_anyone"), None));
    assert_eq!(audience_words(ReadLevel::Members, PostLevel::Members, Rank::Member), (t("chat.access_read_members"), Some(t_marked("chat.access_one_link"))));
    assert_eq!(audience_words(ReadLevel::Anyone, PostLevel::Mods, Rank::Member).1, Some(t_marked("chat.access_post_mods")));
    assert_eq!(audience_words(ReadLevel::Verified, PostLevel::Members, Rank::Member), (t("chat.access_read_verified"), Some(t_marked("chat.access_post_verified"))));
    assert_eq!(audience_words(ReadLevel::Anyone, PostLevel::Mods, Rank::Moderator).1, None);
}

/// THE LINE ABOVE THE BOX IS DRAWN AND ITS GLOSSARY WORD WORKS: on #general (Anyone) as an
/// unverified member, "Anyone can read this, even without signing up." is drawn above the box,
/// with the one-link line; resting the pointer on its word "verified" opens the definition card
/// (an egui tooltip layer), so the line is not only painted but answers the pointer
/// (memory feedback_ui_interactability). On #lounge (Members) it says only people on this server
/// read it; in a direct message there is no such line.
///
/// The line's level badge (a globe for Anyone) shows its hint, "Who can read: Anyone. Who can post:
/// Members.", when the pointer rests on it.
///
/// Seen red 2026-11-02, one break at a time: with the `draw_audience_line` call taken out of the
/// composer, "the line says who can read #general" (the expect panicked); with the one-link line
/// drawn as a plain label instead of `widgets::glossary_words`, "resting on \"verified\" opens its
/// card" (no tooltip layer); with the badge's hint call taken out, "resting on the badge shows who
/// can read and post".
#[test]
fn the_line_above_the_box_says_who_reads_and_its_word_opens_the_card() {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    ctx.style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    let theme = crate::gui::theme::load_theme();
    let mut state = chat_on("general", ReadLevel::Anyone, PostLevel::Members, "");
    for _ in 0..3 {
        frame(&ctx, &theme, &mut state, Vec::new());
    }
    let out = frame(&ctx, &theme, &mut state, Vec::new());
    let reads = find_text_in_shapes(&out.shapes, t("chat.access_read_anyone")).expect("the line says who can read #general");
    let composer = ctx.read_response(egui::Id::new("chat_composer_input")).expect("the composer is drawn").rect;
    assert!(reads.rect.bottom() <= composer.top(), "above the box you type in: {:?} / {composer:?}", reads.rect);
    let one_link = humanity_core::i18n::plain(t_marked("chat.access_one_link"));
    let link_line = find_text_in_shapes(&out.shapes, &one_link).expect("and the one-link rule for a member below Verified");

    // Rest on the word "verified" in it: the definition card opens.
    let before = &one_link[..one_link.find("verified").expect("the line names verified")];
    let before_w = ctx.fonts(|f| f.layout_no_wrap(before.into(), egui::FontId::proportional(theme.font_size_small), egui::Color32::WHITE).size().x);
    // On its first row (the line may wrap): half a row below its top.
    let on_word = egui::pos2(link_line.rect.min.x + before_w + 6.0, link_line.rect.top() + theme.font_size_small * 0.6);
    for _ in 0..3 {
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(on_word)]);
    }
    let card_up = ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Tooltip));
    assert!(card_up, "resting on \"verified\" opens its card");

    // Rest on the badge before the words: its hint says who can read and who can post.
    let ctx2 = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx2);
    ctx2.style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    let mut state2 = chat_on("general", ReadLevel::Anyone, PostLevel::Members, "");
    for _ in 0..3 {
        frame(&ctx2, &theme, &mut state2, Vec::new());
    }
    let out = frame(&ctx2, &theme, &mut state2, Vec::new());
    let reads = find_text_in_shapes(&out.shapes, t("chat.access_read_anyone")).expect("the line");
    let badge = ctx2.read_response(egui::Id::new(COMPOSER_BADGE_ID)).expect("the level badge is drawn on the line").rect;
    // On the words' row (a wrapped label's rect starts at the row's left edge, under the badge).
    assert!(reads.rect.y_range().contains(badge.center().y), "on the line: {badge:?} / {:?}", reads.rect);
    let on_badge = badge.center();
    // The chat page applies the theme's own style every frame, its tooltip delay included, so the
    // pointer rests long enough (frames of 1/60 s each) for an ordinary hover hint to open.
    let mut shown = None;
    for _ in 0..60 {
        shown = Some(frame(&ctx2, &theme, &mut state2, vec![egui::Event::PointerMoved(on_badge)]));
    }
    let hint = badge_hint(ReadLevel::Anyone, PostLevel::Members);
    assert_eq!(hint, "Who can read: Anyone. Who can post: Members.");
    assert!(find_text_in_shapes(&shown.unwrap().shapes, &hint).is_some(), "resting on the badge shows who can read and post");

    let mut members = chat_on("lounge", ReadLevel::Members, PostLevel::Members, "verified");
    frame(&ctx, &theme, &mut members, Vec::new());
    let out = frame(&ctx, &theme, &mut members, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, t("chat.access_read_members")).is_some(), "#lounge: only people on this server");
    assert!(find_text_in_shapes(&out.shapes, &one_link).is_none(), "a verified member has no one-link line");

    let mut dm = chat_on("general", ReadLevel::Anyone, PostLevel::Members, "");
    dm.chat_active_channel = "dm:someone".into();
    frame(&ctx, &theme, &mut dm, Vec::new());
    let out = frame(&ctx, &theme, &mut dm, Vec::new());
    assert!(find_text_in_shapes(&out.shapes, t("chat.access_read_anyone")).is_none(), "a direct message is no channel");
}

/// A POSTER BELOW VERIFIED HAS THEIR LINKS DRAWN AS PLAIN WORDS: their line is not one whose
/// links show; a verified poster's is; a bot's is; in a direct message everyone's is.
///
/// Seen red 2026-11-02 with `shows_links` asking as though every poster were Verified: "a member
/// below Verified: plain words".
#[test]
fn a_poster_below_verified_has_plain_links() {
    let mut state = chat_on("general", ReadLevel::Anyone, PostLevel::Members, "");
    state.chat_users.push(crate::gui::ChatUser { name: "vic".into(), public_key: "vic_key".into(), role: "verified".into(), status: String::new() });
    let line = |from: &str| ChatMessage { sender_key: from.into(), content: "see https://a.example".into(), ..Default::default() };
    assert!(!shows_links(&state, &line(ME)), "a member below Verified: plain words");
    assert!(!shows_links(&state, &line("stranger")), "someone the member list does not name: plain words");
    assert!(shows_links(&state, &line("vic_key")), "a verified member: links");
    assert!(shows_links(&state, &line("bot_deploy")), "the server's bot: links");
    state.chat_active_channel = "dm:vic_key".into();
    assert!(shows_links(&state, &line(ME)), "a direct message is not a channel");
}

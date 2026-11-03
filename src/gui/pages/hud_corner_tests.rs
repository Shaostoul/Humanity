// Child of hud.rs (#[path]): the HUD's bottom-left corner, where the room card and the in-game
// chat feed live (BUG-236, 2026-11-03).

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;

/// The console room's card as the world builds it (`RoomTypes::room_bounds`): its name, purpose
/// and in-room actions read from data/rooms.ron, around an eye at the origin.
fn console_room() -> humanity_ship::room_types::RoomBounds {
    let types = humanity_ship::room_types::RoomTypeRegistry::load(std::path::Path::new(concat!(env!("HUMANITY_REPO_ROOT"), "/data")));
    humanity_ship::room_types::RoomBounds {
        id: "console_room".into(),
        min: Vec3::splat(-3.0),
        max: Vec3::splat(3.0),
        display_name: types.name("console_room"),
        purpose: types.purpose("console_room"),
        actions: types.action_labels("console_room"),
        action_pages: Vec::new(),
        access: String::new(),
        fill: None,
    }
}

/// A player standing in the console room, the chat feed showing `messages` of #general.
fn in_the_console_room(messages: usize) -> GuiState {
    let mut gs = GuiState::default();
    gs.room_bounds = vec![console_room()];
    let channel = gs.chat_active_channel.clone();
    for i in 0..messages {
        gs.chat_messages.push(crate::gui::ChatMessage {
            channel: channel.clone(),
            sender_name: "Ben".into(),
            content: format!("message {i}: are you coming to the garden at noon?"),
            ..Default::default()
        });
    }
    gs
}

/// What one headless frame (no GPU) of the HUD on a screen of `size` points paints in the
/// bottom-left corner: the room card (its three lines, joined) and the chat feed's block (its
/// dark backing), each None when not drawn. The Font Size is the app's zoom (BUG-225,
/// gui/font_size.rs), so 200% on a 2560x1440 screen is this HUD on 1280x720 points.
fn corner(gs: &GuiState, size: Vec2) -> (Option<Rect>, Option<Rect>) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    // The settled HUD: egui fades a new Area in over `animation_time`, which would tint the
    // backing's colour on these first frames (the snapshot harness does the same).
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    let mut out = None;
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
        out = Some(ctx.run(input, |ctx| draw(ctx, &theme, gs, 0.0, Mat4::IDENTITY, Vec3::ZERO)));
    }
    let shapes = out.expect("a frame").shapes;
    let room = console_room();
    let mut card: Option<Rect> = None;
    for line in [room.display_name.clone(), room.purpose.clone(), room.actions[0].clone()] {
        if let Some(found) = find_text_in_shapes(&shapes, &line) {
            card = Some(card.map_or(found.rect, |c| c.union(found.rect)));
        }
    }
    let mut feed: Option<Rect> = None;
    fn backing(shape: &egui::Shape, feed: &mut Option<Rect>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| backing(s, feed)),
            egui::Shape::Rect(r) if r.fill == CHAT_FEED_BACKING => *feed = Some(r.rect),
            _ => {}
        }
    }
    shapes.iter().for_each(|cs| backing(&cs.shape, &mut feed));
    (card, feed)
}

/// THE ROOM CARD STANDS ON THE CHAT FEED (BUG-236, 2026-11-03). Both lived in the bottom-left
/// corner at fixed offsets from the screen's bottom, so "Console room", its purpose and its
/// "Here: ..." line were painted across the feed's status line and messages, at every Font Size
/// (both are measured in points). One bottom-left stack now: the feed is laid out first and
/// reports its block (`chat_feed_rect`), and the card stands on it, a gap above (`room_card_base`).
/// Checked on the operator's screen at 100% and 200% (2560x1440 and 1280x720 points) and in
/// 1920x1080 and 1280x720 windows at 200% (960x540 and 640x360 points), with an empty feed (its
/// one hint line), one, seven and twenty messages (the feed shows seven), the feed switched off
/// (the card goes down to the corner), and the chat panel open (the card is not drawn: the panel
/// covers the corner and the player is typing, not walking).
///
/// Seen red 2026-11-03 against the old layout (the feed's backing found by its colour, which the
/// old code painted the same): "100% (2560x1440), 0 messages: the room card [[14.8 1370.8] -
/// [472.0 1424.8]] crosses the chat feed's block [[8.0 1384.0] - [434.0 1430.0]]".
#[test]
fn the_room_card_and_the_chat_feed_never_overlap() {
    for (what, size) in [
        ("100% (2560x1440)", Vec2::new(2560.0, 1440.0)),
        ("200% (1280x720)", Vec2::new(1280.0, 720.0)),
        ("a 1920x1080 window at 200% (960x540)", Vec2::new(960.0, 540.0)),
        ("a 1280x720 window at 200% (640x360)", Vec2::new(640.0, 360.0)),
    ] {
        let screen = Rect::from_min_size(Pos2::ZERO, size);
        for messages in [0, 1, 7, 20] {
            let gs = in_the_console_room(messages);
            let (card, feed) = corner(&gs, size);
            let card = card.unwrap_or_else(|| panic!("{what}, {messages} messages: the room card is drawn"));
            let feed = feed.unwrap_or_else(|| panic!("{what}, {messages} messages: the chat feed is drawn"));
            assert!(
                !card.intersects(feed),
                "{what}, {messages} messages: the room card {card:?} crosses the chat feed's block {feed:?}"
            );
            assert!(card.max.y <= feed.min.y, "{what}, {messages} messages: the card stands above the feed");
            assert!(screen.contains_rect(card) && screen.contains_rect(feed), "{what}, {messages} messages: both on screen");
        }
        // The feed switched off (the in-world panel's Options tab): the card goes down to the corner.
        let mut off = in_the_console_room(3);
        off.hud_chat_feed_visible = false;
        let (card, feed) = corner(&off, size);
        let card = card.expect("the card without the feed");
        assert!(feed.is_none(), "{what}: no feed is drawn when it is switched off");
        assert!(card.max.y > size.y - 40.0, "{what}: with no feed the card sits in the corner, at {card:?}");
        // The chat panel open (Enter): no feed, and no card behind the panel.
        let mut typing = in_the_console_room(3);
        typing.chat_input_active = true;
        assert_eq!(corner(&typing, size), (None, None), "{what}: the chat panel open draws neither");
    }
}

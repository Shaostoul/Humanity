// Child of chat/person_controls.rs (#[path]): the profile card's "Your settings for <name>",
// drawn and clicked headless (docs/design/per-person-controls.md), and the card's name (BUG-244).
// Nothing here reaches a server: no socket is open, so a note to myself waits in its queue.
// Each test was seen failing once on purpose, recorded at the test.

use super::*;
use crate::gui::pages::chat::modals::{card_name, open_profile};
use crate::gui::{ChatDm, ChatUser};
use crate::net::dm_store::{DmStore, SentPass};
use crate::net::person_controls::Notices;
use crate::notifications::{self as n, Candidate, Level};

const ME: &str = "me00000000000000000000000000000000";
const ARIA: &str = "aria1111111111111111111111111111ffff";

/// Signed in as ME with a DM store in which Aria is a friend (a mutual follow holding a pass from
/// us), Aria in the member list.
fn app(tag: &str) -> GuiState {
    let mut gs = GuiState::default();
    gs.profile_public_key = ME.into();
    gs.server_url = "http://127.0.0.1:9".into();
    gs.chat_users = vec![
        ChatUser { name: "You".into(), public_key: ME.into(), role: String::new(), status: "online".into() },
        ChatUser { name: "Aria".into(), public_key: ARIA.into(), role: "verified".into(), status: "offline".into() },
    ];
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut store = DmStore::load(&[7u8; 32], ME, &format!("wss://pp-{tag}-{nanos}.example"));
    store.set_following(ARIA, true);
    store.set_follower(ARIA, true);
    store.record_pass_sent(ARIA, SentPass { serial: "a1".repeat(16), may: "invite,message,trade,voice_message".into() });
    gs.dm_store = Some(store);
    gs.chat_following_keys.insert(ARIA.into());
    gs.chat_followers.insert(ARIA.into());
    gs
}

fn tidy(gs: &GuiState) {
    if let Some(s) = gs.dm_store.as_ref() {
        s.remove_file_for_test();
    }
}

/// A context set up as the app's (its fonts and theme) with animations off: a window still fading
/// in takes no clicks, and a headless run has no clock to finish the fade.
fn card_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    crate::gui::theme::load_theme().apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    ctx
}

/// Every piece of text one frame drew, with where.
fn texts_of(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn walk(shape: &egui::Shape, texts: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => texts.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        walk(&clipped.shape, &mut texts);
    }
    texts
}

/// One frame of the profile card with `events`, and what it drew. A tall screen, so the whole card
/// fits without scrolling.
fn frame(ctx: &egui::Context, gs: &mut GuiState, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
    let theme = crate::gui::theme::load_theme();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 2400.0))),
        events,
        ..Default::default()
    };
    let out = ctx.run(input, |ctx| crate::gui::pages::chat::draw_user_modal(ctx, &theme, gs));
    texts_of(&out)
}

fn words(texts: &[(String, egui::Rect)]) -> Vec<String> {
    texts.iter().map(|(t, _)| t.clone()).collect()
}

/// The card as it settles: egui sizes a new window on its first frame without drawing it, so the
/// second frame is the one a person sees.
fn settled(ctx: &egui::Context, gs: &mut GuiState) -> Vec<(String, egui::Rect)> {
    for _ in 0..3 {
        frame(ctx, gs, Vec::new());
    }
    frame(ctx, gs, Vec::new())
}

/// Click the text `what` the last frame drew (the last one drawn, so a choice in an open list is
/// found over the box showing it): the pointer moves there, presses and releases, each in a frame
/// of its own (the canonical sequence, docs/design/ui-system.md; a press and release in one frame
/// is dropped by a control that re-interacts its rect, as a ComboBox does), then one more frame
/// lets the change show. Returns that last frame's text.
fn click(ctx: &egui::Context, gs: &mut GuiState, drawn: &[(String, egui::Rect)], what: &str) -> Vec<(String, egui::Rect)> {
    let at = drawn.iter().rev().find(|(t, _)| t == what).unwrap_or_else(|| panic!("{what:?} is drawn: {:?}", words(drawn))).1.center();
    let m = egui::Modifiers::default();
    frame(ctx, gs, vec![egui::Event::PointerMoved(at)]);
    frame(ctx, gs, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, gs, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    frame(ctx, gs, Vec::new())
}

/// A candidate from Aria of `kind`, carrying what the card chose for her as engine/notify.rs fills it
/// (the always-allow list, and her level from the record; that file's own test holds its candidates to
/// the same, `every_candidate_from_a_person_carries_their_level_from_the_card`).
fn from_aria(gs: &GuiState, kind: &str) -> Candidate {
    Candidate {
        kind: kind.into(),
        server: "http://127.0.0.1:9".into(),
        conversation: format!("dm:{ARIA}"),
        sender: "Aria".into(),
        text: if kind == "dm" { "hi".into() } else { String::new() },
        from_allowed: gs.settings.notifications.always_allows(ME, ARIA),
        from_level: gs.settings.person_controls.level_of(ME, ARIA).map(|l| Level::from_id(l.id())),
        ..Default::default()
    }
}

/// Quiet hours from 22:00 to 07:00, loaded, and 23:00 now: what THE rule says for `c`.
fn at_night(c: &Candidate) -> n::Outcome {
    let synced = n::Synced { dm: true, mentions: true, tasks: true, quiet_start: Some("22:00"), quiet_end: Some("07:00"), quiet_friends: false, quiet_mentions: false, loaded: true };
    let at = n::Moment { now: 1_000_000, minute: 23 * 60, focused: false, on_screen: false, last_notice: None };
    n::decide(n::data::get(), c, &synced, &crate::notifications::LocalSettings::default(), &at)
}

/// THE CARD NEVER DRAWS AN EMPTY NAME (BUG-244, the operator's screenshot: a "?" avatar and no
/// name). A DM row's View Profile set only the key, so the card drew no name at the start of a
/// run, or the last card's name. Opened with no name, the card finds it from the key: the DM
/// list's for someone not in the member list, the member list's when it has one, and the start of
/// the key for someone nobody named. Seen red 2026-11-03 with `open_profile` setting only the key
/// and `card_name` drawing the stored name, as the old DM row did: "the DM list's name, not the
/// last card's" got "Ben".
#[test]
fn the_card_never_draws_an_empty_name() {
    let mut gs = app("name");
    gs.chat_users.retain(|u| u.public_key != ARIA); // an offline DM partner the list does not hold
    gs.chat_dms = vec![ChatDm { user_name: "Aria".into(), user_key: ARIA.into(), last_message: String::new(), timestamp: String::new(), unread: false }];
    gs.chat_user_modal_name = "Ben".into(); // the last card opened
    open_profile(&mut gs, ARIA, "");
    assert_eq!(card_name(&gs), "Aria", "the DM list's name, not the last card's");
    let ctx = card_ctx();
    let drawn = words(&settled(&ctx, &mut gs));
    assert!(drawn.iter().any(|t| t == "Aria"), "the card draws it: {drawn:?}");
    assert!(drawn.iter().any(|t| t == "A"), "and its initial in the avatar, not a ?: {drawn:?}");

    gs.chat_users.push(ChatUser { name: "Aria R".into(), public_key: ARIA.into(), role: String::new(), status: "online".into() });
    assert_eq!(card_name(&gs), "Aria R", "the member list's name first");
    open_profile(&mut gs, "0123456789abcdef", "  ");
    assert_eq!(card_name(&gs), "01234567", "someone nobody named: the start of their key, never blank");
    open_profile(&mut gs, ARIA, "Aria (peer)");
    assert_eq!(card_name(&gs), "Aria (peer)", "an opener's own name is kept");
    tidy(&gs);
}

/// THE CARD DRAWS "YOUR SETTINGS FOR <NAME>" for someone else, each control with its words, the
/// friend's three ticks, and the line that says where the choices are kept; never on my own card.
/// Seen red 2026-11-03 with the section's call left out of `draw_user_modal`: "Your settings for
/// Aria is drawn" failed, the card drawing Report and Block straight after the actions.
#[test]
fn the_card_draws_your_settings_for_the_person() {
    let mut gs = app("section");
    open_profile(&mut gs, ARIA, "Aria");
    let ctx = card_ctx();
    let drawn = words(&settled(&ctx, &mut gs));
    for want in [
        "Your settings for Aria",
        "Notifications from Aria",
        "Everything",
        "Calls and messages come through quiet hours",
        "Files from Aria",
        "Use my setting",
        "What Aria may do",
        "Message",
        "Call",
        "Trade",
        "Kept on this device.",
    ] {
        assert!(drawn.iter().any(|t| t == want), "{want:?} is drawn: {drawn:?}");
    }
    // Someone who is not a friend: the line saying what applies, and no ticks.
    let stranger = "cy2222222222222222222222222222220000";
    open_profile(&mut gs, stranger, "Cy");
    let drawn = words(&settled(&ctx, &mut gs));
    assert!(drawn.iter().any(|t| t == "Your settings for Cy"), "{drawn:?}");
    assert!(drawn.iter().any(|t| t.starts_with("They reach you as Settings")), "{drawn:?}");
    assert!(!drawn.iter().any(|t| t == "What Cy may do"), "no ticks for someone who is not a friend: {drawn:?}");
    // My own card: nothing to set for myself.
    open_profile(&mut gs, ME, "You");
    let drawn = words(&settled(&ctx, &mut gs));
    assert!(!drawn.iter().any(|t| t.starts_with("Your settings for")), "{drawn:?}");
    tidy(&gs);
}

/// A CALL DURING QUIET HOURS RINGS ONLY ONCE THE CARD'S SWITCH IS ON: a click on the card's "Calls
/// and messages come through quiet hours" puts Aria on the always-allow list (the note to my other
/// devices waits, no server being connected), and THE rule then lets her call ring at 23:00;
/// another click takes her off, and the call waits again. Seen red 2026-11-03 with the switch's
/// `quiet_allow::set` left out of `draw`: "the click put her on the list" failed (the same click
/// test passing with it in place, so the click lands).
#[test]
fn a_call_during_quiet_hours_rings_only_once_the_cards_switch_is_on() {
    let mut gs = app("quiet");
    open_profile(&mut gs, ARIA, "Aria");
    let call = |gs: &GuiState| at_night(&from_aria(gs, "call")).why;
    assert_eq!(call(&gs), n::Why::QuietHours, "off: her call waits");
    let ctx = card_ctx();
    let drawn = settled(&ctx, &mut gs);
    let drawn = click(&ctx, &mut gs, &drawn, "Calls and messages come through quiet hours");
    assert!(crate::engine::quiet_allow::allows(&gs, ARIA), "the click put her on the list");
    assert_eq!(call(&gs), n::Why::Notify, "on: her call rings at night");
    assert_eq!(gs.settings.notifications.quiet_allow_unsent.len(), 1, "the note to my other devices waits for a server");
    click(&ctx, &mut gs, &drawn, "Calls and messages come through quiet hours");
    assert_eq!(call(&gs), n::Why::QuietHours, "off again: it waits");
    tidy(&gs);
}

/// A PERSON SET TO NOTHING MAKES NO NOTICE: on the card, Notifications from Aria, then Nothing; her
/// direct message then makes no notice (and her call none either), and the choice is kept for her
/// under my identity. Back to Everything, she notifies again and nothing is kept. Seen red
/// 2026-11-03 (when it built the candidate through engine/notify.rs) with `from_level` left out of
/// `dm` there: "nothing: her message makes no notice" answered notify.
#[test]
fn a_person_set_to_nothing_makes_no_notice() {
    let mut gs = app("nothing");
    open_profile(&mut gs, ARIA, "Aria");
    let by_day = |gs: &GuiState| {
        let c = from_aria(gs, "dm");
        let synced = n::Synced { dm: true, mentions: true, tasks: true, quiet_start: None, quiet_end: None, quiet_friends: false, quiet_mentions: false, loaded: true };
        let at = n::Moment { now: 1_000_000, minute: 600, focused: false, on_screen: false, last_notice: None };
        n::decide(n::data::get(), &c, &synced, &gs.settings.notifications, &at).why
    };
    assert_eq!(by_day(&gs), n::Why::Notify, "nothing chosen: her message notifies");
    let ctx = card_ctx();
    let drawn = settled(&ctx, &mut gs);
    let drawn = click(&ctx, &mut gs, &drawn, "Everything"); // opens the choices
    click(&ctx, &mut gs, &drawn, "Nothing");
    assert_eq!(gs.settings.person_controls.get(ME, ARIA).notices, Notices::Nothing, "kept for her under my identity");
    assert!(gs.settings_dirty, "and saved");
    assert_eq!(by_day(&gs), n::Why::PersonLevel, "nothing: her message makes no notice");
    assert_eq!(at_night(&from_aria(&gs, "call")).why, n::Why::PersonLevel, "nor her call");

    let drawn = settled(&ctx, &mut gs);
    let drawn = click(&ctx, &mut gs, &drawn, "Nothing");
    click(&ctx, &mut gs, &drawn, "Everything");
    assert_eq!(by_day(&gs), n::Why::Notify, "everything again: she notifies");
    assert!(gs.settings.person_controls.0.is_empty(), "and nothing is kept: {:?}", gs.settings.person_controls.0);
    tidy(&gs);
}

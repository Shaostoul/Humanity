//! Headless tests of the server strip (server_strip.rs, Media round 5's lane 1E): the pages drawn
//! through `dispatch::draw_tool_page` into an egui context with no window, clicked with synthetic
//! pointer input (docs/design/ui-system.md, "Interaction tests"), and what they drew or did read
//! back. Nothing here reaches the network: both servers are closed ports on this machine, every
//! switch goes to a parked link (no dial), and nothing writes the person's config.json. Each test
//! was seen to fail once against the break its comment names before it passed.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::{ChatChannel, ChatMessage, ChatServer};

/// The server the app is on, and a second saved one: closed ports on this machine, so anything
/// that does try to reach them fails at once.
const HOME: &str = "http://127.0.0.1:9";
const OTHER: &str = "http://127.0.0.1:19";

/// A context with the app's fonts and theme, animations off.
fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

/// One frame of `page`, drawn as the app draws it (the strip, then the page), with `events`.
fn frame(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState, page: GuiPage, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1400.0))),
        events,
        ..Default::default()
    };
    ctx.run(input, |ctx| {
        crate::gui::dispatch::draw_tool_page(ctx, page, theme, state);
    })
}

/// A few frames, so windows and areas stand still; the last one's output.
fn settled(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState, page: GuiPage) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, theme, state, page, Vec::new());
    }
    frame(ctx, theme, state, page, Vec::new())
}

/// Click the drawn text `text` on `page`: the pointer moved there, pressed, released, each on a
/// frame of its own.
fn click(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState, page: GuiPage, text: &str) {
    let out = settled(ctx, theme, state, page);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn on {page:?}")).rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, page, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, page, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, page, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

fn shows(out: &egui::FullOutput, text: &str) -> bool {
    find_text_in_shapes(&out.shapes, text).is_some()
}

fn saved(name: &str, url: &str) -> ChatServer {
    ChatServer { name: name.into(), channels: Vec::new(), voice_channels: Vec::new(), id: format!("srv_{url}"), url: url.into(), connected: false }
}

/// An unlocked app on HOME, with OTHER saved and its link parked (a message in its own #local
/// room, which it was last in), so a switch to OTHER brings it back instead of dialling.
fn two_servers() -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut s = GuiState::default();
    s.user_name = "Ada".into();
    s.profile_public_key = "me".into();
    s.private_key_bytes = Some(vec![7u8; 32]);
    s.chat_servers = vec![saved("Home server", HOME), saved("Second server", OTHER)];
    // OTHER first, as it was before the person came to HOME: then parked.
    s.server_url = OTHER.into();
    s.connected_server_url = OTHER.into();
    s.chat_channels.push(ChatChannel { id: "local".into(), name: "Second server".into(), local_only: true, ..Default::default() });
    s.chat_active_channel = "local".into();
    s.chat_messages.push(ChatMessage { sender_name: "Bela".into(), content: "hello from the second server".into(), channel: "local".into(), timestamp_ms: 1, ..Default::default() });
    s.park_active_connection();
    // HOME, active.
    s.server_url = HOME.into();
    s.connected_server_url = HOME.into();
    s.chat_messages.push(ChatMessage { sender_name: "Ada".into(), content: "hello from home".into(), channel: "general".into(), timestamp_ms: 2, ..Default::default() });
    s
}

/// What a switch leaves that the person can see or the app acts on, for comparing two switches.
fn after_switch(s: &GuiState) -> String {
    let mut parked: Vec<String> = s.connections.iter().map(|c| format!("{} {:?}", c.url, c.messages.iter().map(|m| m.content.clone()).collect::<Vec<_>>())).collect();
    parked.sort();
    format!(
        "on {} (field {}), room {}, messages {:?}, status {:?}, identified {}, parked {:?}",
        s.active_server(),
        s.server_url,
        s.chat_active_channel,
        s.chat_messages.iter().map(|m| m.content.clone()).collect::<Vec<_>>(),
        s.ws_status,
        s.ws_identified,
        parked
    )
}

/// The strip is on Media, Watch, Chat and Server Settings, drawn through `draw_tool_page`, and on
/// no other page; and it names the server, by its host when it has no name (a fresh app is on the
/// official server, saved under no name). Every page is drawn on an empty state, as the dispatch
/// table's own coverage test draws them.
///
/// Seen red 2026-10-12 with the strip drawn on every page (`shown_on` returning true): "GuiPage::
/// None: the strip is drawn on a page that is not one of the four" (the in-game page, whose
/// drawing stays in lib.rs, is the first in the list).
#[test]
fn the_strip_shows_on_the_four_pages_and_no_other() {
    crate::config::keep_saves_off_disk();
    let four = [GuiPage::Media, GuiPage::Watch, GuiPage::Chat, GuiPage::ServerSettings];
    for &page in crate::gui::dispatch::ALL_PAGES {
        let (ctx, mut theme) = ctx_and_theme();
        let mut state = GuiState::default();
        let mut out = None;
        for _ in 0..2 {
            out = Some(frame(&ctx, &mut theme, &mut state, page, Vec::new()));
        }
        let drawn = egui::containers::panel::PanelState::load(&ctx, Id::new(STRIP_ID)).is_some();
        let expected = four.contains(&page);
        assert_eq!(
            drawn,
            expected,
            "GuiPage::{page:?}: the strip is {}",
            if expected { "missing from one of the four pages" } else { "drawn on a page that is not one of the four" }
        );
        if expected {
            let out = out.expect("two frames ran");
            assert!(shows(&out, "united-humanity.us"), "GuiPage::{page:?}: the strip does not name the server's host");
            assert!(shows(&out, SWITCH), "GuiPage::{page:?}: the strip has no Switch");
        }
    }
    // The words: the server's own name and its host, two spaces apart, as on the website.
    assert_eq!(strip_label("United Humanity", "united-humanity.us"), "United Humanity  united-humanity.us");
    assert_eq!(strip_label("  ", "127.0.0.1:9"), "127.0.0.1:9");
    assert_eq!(host_of("https://united-humanity.us/"), "united-humanity.us");
    assert_eq!(host_of("http://127.0.0.1:9"), "127.0.0.1:9");
}

/// Switch lists the saved servers, and picking one parks the server the app is on (its socket and
/// its messages kept) and brings the picked one back as it was, in the room it was last in. The
/// switch also forgets the old server's answer about where you live (`home_answer`): lane 2B of
/// increment 6 found nothing cleared it, so Characters' card could show one server's home while
/// on another.
///
/// Seen red 2026-10-12 with `switch_to_server` not parking first: "the server switched from was
/// not parked with its messages" (left: [], the old server's messages were lost). The answer's
/// line seen red with the clear in `reset_per_server_transients` taken out: "the old server's
/// answer about where you live was kept".
#[test]
fn switching_from_the_strip_parks_and_unparks() {
    let mut state = two_servers();
    state.home_answer = Some(crate::ship::starts::HomeAnswer { plot: None, starts: Vec::new(), chose: false, refused: None, message: String::new() });
    let (ctx, mut theme) = ctx_and_theme();
    let page = GuiPage::ServerSettings;
    let out = settled(&ctx, &mut theme, &mut state, page);
    assert!(shows(&out, "Home server  127.0.0.1:9"), "the strip names the server the app is on");
    click(&ctx, &mut theme, &mut state, page, SWITCH);
    let out = settled(&ctx, &mut theme, &mut state, page);
    assert!(shows(&out, "Second server  127.0.0.1:19"), "Switch lists the saved servers");
    assert!(shows(&out, CURRENT), "the server the app is on is marked");
    click(&ctx, &mut theme, &mut state, page, "Second server");

    assert_eq!(state.active_server(), OTHER, "the pick did not switch the app to the second server");
    let home = state.connections.iter().find(|c| c.url == HOME);
    let kept: Vec<String> = home.map(|c| c.messages.iter().map(|m| m.content.clone()).collect()).unwrap_or_default();
    assert_eq!(kept, vec!["hello from home".to_string()], "the server switched from was not parked with its messages");
    assert!(state.connections.iter().all(|c| c.url != OTHER), "the second server's link is still parked");
    let messages: Vec<String> = state.chat_messages.iter().map(|m| m.content.clone()).collect();
    assert_eq!(messages, vec!["hello from the second server".to_string()], "the second server did not come back as it was");
    assert_eq!(state.chat_active_channel, "local", "the second server did not come back in its own room");
    assert!(state.home_answer.is_none(), "the old server's answer about where you live was kept");
    let out = settled(&ctx, &mut theme, &mut state, page);
    assert!(shows(&out, "Second server  127.0.0.1:19"), "the strip does not name the server switched to");
    assert!(!shows(&out, LIST_HEAD), "the list stayed open after the pick");
}

/// With an upload on its way to this server, a pick asks first, says what a switch does to it,
/// and switches only on the answer. The upload is a real one, started the way a person carries
/// one on: the Media page's Carry on, pressed on a record the server named (its worker dials a
/// closed port here and is stopped at the end).
///
/// Seen red 2026-10-12 with `pick` switching without asking: "the pick switched with an upload on
/// its way, without asking" (left: "http://127.0.0.1:19", right: "http://127.0.0.1:9").
#[test]
fn switching_with_uploads_running_asks_first() {
    let mut state = two_servers();
    // The Media page on HOME, signed in, its list as good as loaded (nothing is asked for), and
    // an upload the server named that did not finish.
    crate::gui::pages::media::sync_server(&mut state);
    state.media.requested = true;
    state.upload_token = Some((HOME.to_string(), "tok".to_string()));
    state.media.unfinished.push(crate::gui::pages::media::resumable::Unfinished {
        server: HOME.into(),
        key: "me".into(),
        id: "up1".into(),
        path: std::env::temp_dir().join("server-strip-test-no-such-file.webm"),
        name: "film.webm".into(),
        size: 4096,
        ..Default::default()
    });
    let (ctx, mut theme) = ctx_and_theme();
    click(&ctx, &mut theme, &mut state, GuiPage::Media, "Carry on");
    assert_eq!(uploads_on_their_way(&state), 1, "Carry on did not start the upload: {}", state.media.status);

    // Server Settings from here on: the Media page would read the upload's worker, which fails
    // against the closed port, and the upload would end before the question could be asked.
    let page = GuiPage::ServerSettings;
    click(&ctx, &mut theme, &mut state, page, SWITCH);
    click(&ctx, &mut theme, &mut state, page, "Second server");
    assert_eq!(state.active_server(), HOME, "the pick switched with an upload on its way, without asking");
    let out = settled(&ctx, &mut theme, &mut state, page);
    let question = uploads_question(1, "127.0.0.1:9");
    assert!(shows(&out, &question), "the question is not shown in its words: {question:?}");

    // Stay here keeps everything as it was.
    click(&ctx, &mut theme, &mut state, page, STAY);
    assert_eq!(state.active_server(), HOME, "Stay here switched");
    assert_eq!(uploads_on_their_way(&state), 1, "Stay here stopped the upload");
    assert!(!shows(&settled(&ctx, &mut theme, &mut state, page), &question), "the question stayed after Stay here");

    // Asked again, the switch goes on the answer.
    click(&ctx, &mut theme, &mut state, page, SWITCH);
    click(&ctx, &mut theme, &mut state, page, "Second server");
    click(&ctx, &mut theme, &mut state, page, "Switch to Second server");
    assert_eq!(state.active_server(), OTHER, "the answer to switch did not switch");

    // The Media page's own step when the server changed: the upload stops where it is, its
    // record kept for Carry on when the person comes back.
    assert!(crate::gui::pages::media::sync_server(&mut state), "the Media page saw no change of server");
    assert!(state.media.unfinished.iter().any(|u| u.id == "up1"), "the upload's record went with the switch");
    // The plural, for two or more.
    assert_eq!(
        uploads_question(2, "united-humanity.us"),
        "2 uploads are on their way to united-humanity.us. Switching stops them where they are; they carry on when you come back and press Carry on."
    );
}

/// Chat's server rows and the strip switch the same way, because they make the same switch: a
/// row clicked in Chat and the same server picked in the strip leave two identical apps. And the
/// Chat page holds no switch of its own any more: its cog, its row and its rooms each call
/// `switch_to_server`, and nothing in it parks a server and brings another back by itself (the
/// Connect button parks the server it leaves, which is a connect, not a switch).
///
/// Seen red 2026-10-12 with the row's block put back inline: "Chat's cog, row and rooms do not
/// each call the one switch" (left: 2, right: 3).
#[test]
fn the_chat_rows_and_the_strip_switch_the_same_way() {
    let mut by_row = two_servers();
    // A wide left panel, so the row shows the server's whole name ("S..." at the default width).
    by_row.chat_left_panel_width = 420.0;
    let (ctx, mut theme) = ctx_and_theme();
    let out = settled(&ctx, &mut theme, &mut by_row, GuiPage::Chat);
    assert!(shows(&out, "Second server"), "Chat lists the second server");
    click(&ctx, &mut theme, &mut by_row, GuiPage::Chat, "Second server");

    let mut by_strip = two_servers();
    let (ctx, mut theme) = ctx_and_theme();
    let page = GuiPage::ServerSettings;
    click(&ctx, &mut theme, &mut by_strip, page, SWITCH);
    click(&ctx, &mut theme, &mut by_strip, page, "Second server");

    assert_eq!(by_row.active_server(), OTHER, "the row did not switch");
    assert_eq!(after_switch(&by_strip), after_switch(&by_row), "the strip and Chat's row switched differently");

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let panel = std::fs::read_to_string(root.join("src/gui/pages/chat/left_panel.rs")).expect("read left_panel.rs");
    assert_eq!(
        panel.matches("state.switch_to_server(&server.url, &server.name);").count(),
        3,
        "Chat's cog, row and rooms do not each call the one switch"
    );
    assert_eq!(panel.matches("unpark_connection(").count(), 0, "a switch in Chat brings a server back by itself");
    assert_eq!(panel.matches("connect_with_kyber(").count(), 0, "a switch in Chat dials a server by itself");
    assert_eq!(panel.matches("park_active_connection()").count(), 1, "Chat parks a server outside its Connect button");
}

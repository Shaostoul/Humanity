//! Links to chat channels on the desktop (2026-11-03, chat/channel_link.rs). The operator: "Can we
//! make it so I can link people to specific channels like https://united-humanity.us/chat#global
//! or https://united-humanity.us/chat#fire-disciples", and "I think we need to also fix the default
//! channel to go to global". Drawn headlessly (no GPU, no network: the server the app is on has a
//! test client that counts as connected, the other's rooms are its parked connection's, and a
//! dialled server is a closed port on this machine), clicked with synthetic pointer input, and
//! what the app copied, opened or left for the browser read back from egui's output.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::pages::chat::{land_if_room_gone, left_panel, norm_server_url};
use crate::gui::theme::Theme;
use crate::gui::{ChatChannel, ChatServer, GuiPage, GuiState, ServerConnection};

/// Two saved servers with public names (nothing dials them: the active one has a test client,
/// the other is parked), and a third, a closed port on this machine, for a switch that dials.
const HOME: &str = "https://home.example";
const OTHER: &str = "https://other.example";
const CLOSED: &str = "http://127.0.0.1:9";

fn fixture() -> serde_json::Value {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/chat-links-v1.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("the shared fixture")).expect("it is JSON")
}

/// THE SHARED FIXTURE: every row of scripts/tests/fixtures/chat-links-v1.json, the answers the
/// website's web/shared/chat-links.js gives too (scripts/tests/chat-links-web.test.js, test 1).
///
/// Seen red 2026-11-03 with `find_id` matching ids only: "the channel \"fire disciples\" names"
/// (left: None, right: Some("fire-disciples")).
#[test]
fn the_desktop_link_rules_give_every_answer_of_the_shared_fixture() {
    let fx = fixture();
    let rows = |k: &str| fx[k].as_array().unwrap_or_else(|| panic!("the fixture has {k}")).clone();
    for r in rows("words") {
        let want = r[1].as_str().unwrap();
        assert_eq!(word_of(r[0].as_str().unwrap()).unwrap_or_default(), want, "the word of {:?}", r[0]);
    }
    let channels: Vec<(String, String)> = rows("channels").iter().map(|c| (c["id"].as_str().unwrap().into(), c["name"].as_str().unwrap().into())).collect();
    for r in rows("finds") {
        let got = find_id(r[0].as_str().unwrap(), channels.iter().map(|(i, n)| (i.as_str(), n.as_str())));
        assert_eq!(got, r[1].as_str(), "the channel {:?} names", r[0]);
    }
    for r in rows("links") {
        assert_eq!(link(r[0].as_str().unwrap(), r[1].as_str().unwrap()), r[2].as_str().unwrap(), "the address of {:?} on {:?}", r[1], r[0]);
    }
    for r in rows("public_hosts") {
        assert_eq!(is_public_host(r[0].as_str().unwrap()), r[1].as_bool().unwrap(), "is {:?} public", r[0]);
    }
    for r in rows("parses") {
        let want = if r[1].is_null() {
            None
        } else if let Some(key) = r[1]["dm"].as_str() {
            Some(ChatLink::Dm { origin: r[1]["origin"].as_str().unwrap().into(), key: key.into() })
        } else {
            Some(ChatLink::Channel { origin: r[1]["origin"].as_str().unwrap().into(), word: r[1]["channel"].as_str().unwrap().into() })
        };
        assert_eq!(parse(r[0].as_str().unwrap()), want, "what {:?} asks for", r[0]);
    }
    assert_eq!(server_origin("wss://Home.Example/ws").as_deref(), Some("https://home.example"), "a socket's address is its server's");
    assert_eq!(server_origin("united-humanity.us").as_deref(), Some("https://united-humanity.us"), "a bare host is https");
    assert_eq!(server_origin(""), None, "no server, no address");
}

/// HOME active (its link up, #global, #feedback and #fire-disciples shown as "Fire Disciples"),
/// OTHER saved with its link parked and its rooms opened in the sidebar (#local, #garden).
fn two_servers() -> GuiState {
    crate::config::keep_saves_off_disk();
    let mut s = GuiState::default();
    s.user_name = "Ada".into();
    s.profile_public_key = "me".into();
    s.private_key_bytes = Some(vec![7u8; 32]);
    let saved = |name: &str, url: &str| ChatServer { name: name.into(), channels: Vec::new(), voice_channels: Vec::new(), id: format!("srv_{url}"), url: url.into(), connected: true };
    s.chat_servers = vec![saved("Home server", HOME), saved("Second server", OTHER)];
    let room = |id: &str, name: &str| ChatChannel { id: id.into(), name: name.into(), ..Default::default() };
    s.server_url = HOME.into();
    s.connected_server_url = HOME.into();
    s.ws_client = Some(crate::net::ws_client::WsClient::recording().0);
    s.chat_channels = vec![room("global", "global"), room("feedback", "feedback"), room("fire-disciples", "Fire Disciples")];
    s.chat_active_channel = "global".into();
    s.connections.push(ServerConnection {
        url: OTHER.into(),
        display_url: OTHER.into(),
        identified: true,
        channels: vec![room("local", "local"), room("garden", "garden")],
        active_channel: "local".into(),
        ..Default::default()
    });
    s.chat_server_folds.insert(norm_server_url(OTHER), false);
    s
}

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

/// What a test draws: the chat's left panel, the whole Chat page, or the Server Settings page.
#[derive(Clone, Copy)]
enum View {
    Panel,
    Chat,
    Settings,
}

/// One frame of `view`, with `events`.
fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, view: View, events: Vec<egui::Event>) -> egui::FullOutput {
    let tall = if matches!(view, View::Settings) { 2000.0 } else { 900.0 };
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, tall))), events, ..Default::default() };
    ctx.run(input, |ctx| match view {
        View::Chat => crate::gui::pages::chat::draw(ctx, theme, state),
        View::Settings => crate::gui::pages::server_settings::draw(ctx, theme, state),
        View::Panel => {
            egui::CentralPanel::default().show(ctx, |ui| left_panel::draw_left_panel(ui, theme, state));
        }
    })
}

fn settled(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, page: View) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, theme, state, page, Vec::new());
    }
    frame(ctx, theme, state, page, Vec::new())
}

/// Press and release `button` on the drawn text `text`, each on a frame of its own; every frame's
/// output, in order.
fn press(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, page: View, text: &str, button: egui::PointerButton) -> Vec<egui::FullOutput> {
    let out = settled(ctx, theme, state, page);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect.center();
    press_at(ctx, theme, state, page, pos, button)
}

/// Press and release `button` at `pos`, as `press` does.
fn press_at(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, page: View, pos: egui::Pos2, button: egui::PointerButton) -> Vec<egui::FullOutput> {
    let m = egui::Modifiers::default();
    vec![
        frame(ctx, theme, state, page, vec![egui::Event::PointerMoved(pos)]),
        frame(ctx, theme, state, page, vec![egui::Event::PointerButton { pos, button, pressed: true, modifiers: m }]),
        frame(ctx, theme, state, page, vec![egui::Event::PointerButton { pos, button, pressed: false, modifiers: m }]),
        frame(ctx, theme, state, page, Vec::new()),
    ]
}

/// What the frames copied.
fn copied(outs: &[egui::FullOutput]) -> Vec<String> {
    outs.iter()
        .flat_map(|o| o.platform_output.commands.iter())
        .filter_map(|c| if let egui::OutputCommand::CopyText(t) = c { Some(t.clone()) } else { None })
        .collect()
}

/// COPY LINK ON A ROOM'S RIGHT-CLICK COPIES ITS ADDRESS ON ITS OWN SERVER, and the channel's header
/// has it too: a room on the server the app is on, a room on another saved server, and the open
/// channel's header each copy https://<that server>/chat#<id>, with the notice saying what was
/// copied; a room on a server with no public address says the link opens only here.
///
/// Seen red 2026-11-03 with the menu as it was (left_panel.rs at the merge of 85cd9778c):
/// "\"Copy link\" is drawn" failed on the first room's menu (its item said "Copy Channel Link"
/// and copied https://united-humanity.us/chat/feedback, from any server, and another server's rooms
/// had none); and with the header's button taken out, the same at the header.
#[test]
fn copy_link_copies_the_room_address_on_its_own_server() {
    let (ctx, theme) = ctx_and_theme();
    let copy = humanity_core::i18n::t("chat.copy_link");

    let mut state = two_servers();
    press(&ctx, &theme, &mut state, View::Panel, "# feedback", egui::PointerButton::Secondary);
    let outs = press(&ctx, &theme, &mut state, View::Panel, copy, egui::PointerButton::Primary);
    assert_eq!(copied(&outs), vec!["https://home.example/chat#feedback".to_string()], "the room's menu has Copy link, copying its address here");
    let said = humanity_core::i18n::tf("chat.copy_link_done", &[("link", &"https://home.example/chat#feedback")]);
    assert!(state.toasts.iter().any(|t| t.text == said), "and it says what was copied: {:?}", state.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());

    let mut state = two_servers();
    press(&ctx, &theme, &mut state, View::Panel, "# garden", egui::PointerButton::Secondary);
    let outs = press(&ctx, &theme, &mut state, View::Panel, copy, egui::PointerButton::Primary);
    assert_eq!(copied(&outs), vec!["https://other.example/chat#garden".to_string()], "a room on another server is copied with that server's address");
    assert_eq!(state.active_server(), norm_server_url(HOME), "and copying switches nothing");

    let mut state = two_servers();
    state.chat_active_channel = "fire-disciples".into();
    let outs = press(&ctx, &theme, &mut state, View::Chat, copy, egui::PointerButton::Primary);
    assert_eq!(copied(&outs), vec!["https://home.example/chat#fire-disciples".to_string()], "the header's Copy link copies the open channel");

    let mut state = two_servers();
    state.server_url = "http://127.0.0.1:3210".into();
    state.connected_server_url = state.server_url.clone();
    state.chat_servers[0].url = state.server_url.clone();
    let outs = press(&ctx, &theme, &mut state, View::Chat, copy, egui::PointerButton::Primary);
    assert_eq!(copied(&outs), vec!["http://127.0.0.1:3210/chat#global".to_string()], "a server with no public address is copied all the same");
    let said = humanity_core::i18n::tf("chat.copy_link_done_private", &[("link", &"http://127.0.0.1:3210/chat#global")]);
    assert!(state.toasts.iter().any(|t| t.text == said), "and says the link opens only here");
}

/// Click the cog on the server row of the server the app is on, which opens its menu: a painted
/// mark with no text, found from the row's ground (as wide as the list, a row tall) and the cog's
/// fixed place, 48 points in from the row's right edge (left_panel.rs `draw_server_row`).
fn open_server_menu(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let out = settled(ctx, theme, state, View::Panel);
    let name = find_text_in_shapes(&out.shapes, "Home server").expect("the server row is drawn").rect.center();
    let ground = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) if r.rect.contains(name) && r.rect.width() > 300.0 && r.rect.height() < 40.0 => Some(r.rect),
            _ => None,
        })
        .next()
        .expect("the server row has its own ground");
    press_at(ctx, theme, state, View::Panel, egui::pos2(ground.right() - 48.0, name.y), egui::PointerButton::Primary);
}

/// The server the app is on moved to an address with no public name (this computer).
fn on_this_computer(state: &mut GuiState) {
    state.server_url = "http://127.0.0.1:3210".into();
    state.connected_server_url = state.server_url.clone();
    state.chat_servers[0].url = state.server_url.clone();
}

/// BUG-245: THE INVITE LINK IS THE SERVER'S OWN CHAT ADDRESS. The server cog's Copy invite link and
/// Server Settings' invite link show and copy https://<the server the app is on>/chat, as Copy link
/// does for a room, with the notice saying what was copied; for a server with no public address
/// they copy it all the same and say the link opens only on this computer or its home network.
///
/// Seen red 2026-11-04 with both building the address as they did (the cog's
/// `format!("https://united-humanity.us/chat")`, Server Settings' `"https://united-humanity.us/chat"`),
/// their buttons already reading the new word: "the server cog's Copy invite link copies this
/// server's chat address" (left: ["https://united-humanity.us/chat"], right:
/// ["https://home.example/chat"]); and with the cog fixed and Server Settings not yet, "Server
/// Settings shows this server's chat address".
#[test]
fn the_invite_link_is_the_servers_own_chat_address() {
    let (ctx, theme) = ctx_and_theme();
    let copy = humanity_core::i18n::t("chat.copy_invite_link");

    let mut state = two_servers();
    open_server_menu(&ctx, &theme, &mut state);
    let outs = press(&ctx, &theme, &mut state, View::Panel, copy, egui::PointerButton::Primary);
    assert_eq!(copied(&outs), vec!["https://home.example/chat".to_string()], "the server cog's Copy invite link copies this server's chat address");
    let said = humanity_core::i18n::tf("chat.copy_link_done", &[("link", &"https://home.example/chat")]);
    assert!(state.toasts.iter().any(|t| t.text == said), "and says what was copied: {:?}", state.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());

    let mut state = two_servers();
    let out = settled(&ctx, &theme, &mut state, View::Settings);
    assert!(find_text_in_shapes(&out.shapes, "https://home.example/chat").is_some(), "Server Settings shows this server's chat address");
    let outs = press(&ctx, &theme, &mut state, View::Settings, copy, egui::PointerButton::Primary);
    assert_eq!(copied(&outs), vec!["https://home.example/chat".to_string()], "and its Copy invite link copies it");

    let private = humanity_core::i18n::t("chat.invite_private");
    let mut state = two_servers();
    on_this_computer(&mut state);
    let out = settled(&ctx, &theme, &mut state, View::Settings);
    assert!(find_text_in_shapes(&out.shapes, private).is_some(), "Server Settings says a server with no public address has a link that opens only here");
    open_server_menu(&ctx, &theme, &mut state);
    let outs = press(&ctx, &theme, &mut state, View::Panel, copy, egui::PointerButton::Primary);
    assert_eq!(copied(&outs), vec!["http://127.0.0.1:3210/chat".to_string()], "a server with no public address is copied all the same");
    let said = humanity_core::i18n::tf("chat.copy_link_done_private", &[("link", &"http://127.0.0.1:3210/chat")]);
    assert!(state.toasts.iter().any(|t| t.text == said), "and the notice says the link opens only here");
}

/// One frame in which something in the app asks to open `url` in the browser (as a chat message's
/// link does, chat.rs), then the end of the frame (lib.rs): the links left for the browser.
fn follow(ctx: &egui::Context, state: &mut GuiState, url: &str) -> Vec<String> {
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        ctx.open_url(egui::OpenUrl::new_tab(url));
        open_in_app(ctx, state);
    });
    out.platform_output.commands.iter().filter_map(|c| if let egui::OutputCommand::OpenUrl(o) = c { Some(o.url.clone()) } else { None }).collect()
}

/// A LINK TO A CHANNEL ON THE SERVER THE APP IS ON, OR ON A SAVED ONE, OPENS IT IN THE APP:
/// clicked on another page, the link's channel opens on the Chat page, by its id or its shown
/// name, and nothing goes to the browser; a saved server's link switches to that server first and
/// lands in its room; a link naming no channel says so and opens no other; a conversation link
/// opens the conversation; a link to a server not saved here, or followed with no identity
/// unlocked, goes to the browser as before.
///
/// Seen red 2026-11-03 with `open_in_app` returning at once (every link to the browser): "a link
/// to this server's channel stays in the app" (left: ["https://home.example/chat#Fire%20Disciples"],
/// right: []).
#[test]
fn a_link_to_this_or_a_saved_servers_channel_opens_in_the_app() {
    let ctx = egui::Context::default();
    let mut state = two_servers();
    state.active_page = GuiPage::Library;
    let left = follow(&ctx, &mut state, "https://home.example/chat#Fire%20Disciples");
    assert_eq!(left, Vec::<String>::new(), "a link to this server's channel stays in the app");
    assert_eq!(state.chat_active_channel, "fire-disciples", "its channel opens, found by its shown name");
    assert_eq!(state.active_page, GuiPage::Chat, "on the Chat page");

    assert!(follow(&ctx, &mut state, "https://home.example/chat#feedback").is_empty());
    assert_eq!(state.chat_active_channel, "feedback", "by its id");

    assert!(follow(&ctx, &mut state, "https://home.example/chat#nowhere").is_empty(), "a link naming no channel here is still this server's");
    assert_eq!(state.chat_active_channel, "feedback", "and opens no other channel");
    let said = humanity_core::i18n::tf("chat.link_no_channel", &[("name", &"nowhere")]);
    assert!(state.toasts.iter().any(|t| t.text == said), "it says so");

    let key = "ab".repeat(20);
    assert!(follow(&ctx, &mut state, &format!("https://home.example/chat#dm={key}")).is_empty());
    assert_eq!(state.chat_active_channel, format!("dm:{key}"), "a conversation link opens the conversation");

    assert!(follow(&ctx, &mut state, "https://other.example/chat#garden").is_empty(), "a saved server's link stays in the app");
    assert_eq!(state.active_server(), norm_server_url(OTHER), "it switches to that server");
    assert_eq!(state.chat_active_channel, "garden", "and lands in the link's room");

    let elsewhere = "https://public.guide/chat#global";
    assert_eq!(follow(&ctx, &mut state, elsewhere), vec![elsewhere.to_string()], "a server not saved here: the browser, as before");
    assert_eq!(follow(&ctx, &mut state, "https://other.example/files"), vec!["https://other.example/files".to_string()], "and any other page");
    state.private_key_bytes = None;
    assert_eq!(follow(&ctx, &mut state, "https://other.example/chat#local").len(), 1, "with no identity unlocked the browser opens it");
}

/// A LINK TO A SAVED SERVER THE APP HAS NO LINK TO YET waits for that server's channel list: the
/// switch dials it (a closed port here), and when its list arrives the link's room opens, matched
/// by its shown name, before the room-gone rule looks; a word naming no room there says so and the
/// app lands in #global. Another switch first forgets the wait.
///
/// Seen red 2026-11-03 with `find_id` matching ids only: "the waiting link opens its room once the
/// list is here" (left: "global", right: "garden-club").
#[test]
fn a_link_to_a_server_being_dialled_waits_for_its_channel_list() {
    let ctx = egui::Context::default();
    let mut state = two_servers();
    state.chat_servers.push(ChatServer { name: "Closed".into(), channels: Vec::new(), voice_channels: Vec::new(), id: format!("srv_{CLOSED}"), url: CLOSED.into(), connected: false });
    assert!(follow(&ctx, &mut state, &format!("{CLOSED}/chat#Garden%20Club")).is_empty());
    assert_eq!(state.active_server(), norm_server_url(CLOSED), "the saved server is switched to");
    assert_eq!(state.chat_link_pending.as_deref(), Some("Garden Club"), "and the link waits for its list");
    // Its channel list arrives (engine/frame_ws_poll.rs calls these two in this order).
    let room = |id: &str, name: &str| ChatChannel { id: id.into(), name: name.into(), ..Default::default() };
    state.chat_channels = vec![room("announcements", "announcements"), room("global", "global"), room("garden-club", "Garden Club")];
    open_pending(&mut state, 0.0);
    land_if_room_gone(&mut state);
    assert_eq!(state.chat_active_channel, "garden-club", "the waiting link opens its room once the list is here");
    assert!(state.chat_link_pending.is_none());

    state.chat_link_pending = Some("nowhere".into());
    state.chat_active_channel = "global".into();
    open_pending(&mut state, 0.0);
    assert_eq!(state.chat_active_channel, "global", "a word naming no room opens no other");
    state.chat_link_pending = Some("garden-club".into());
    state.switch_to_server(HOME, "Home server");
    assert!(state.chat_link_pending.is_none(), "a switch forgets a link waiting for another server's list");
}

/// THE DESKTOP'S DEFAULT IS #global at every entry point with no link and no saved room: the first
/// open, a switch to a server the app dials (its rooms not known yet), and (already held by
/// chat.rs's `a_room_that_is_gone_lands_in_global`) a room that is gone.
///
/// Seen red 2026-11-03, one break at a time: with `GuiState::default` opening "announcements",
/// "the first open is #global" (left: "announcements"); with the dialling switch landing in the room
/// it left, "a switch to a server being dialled lands in #global" (left: "feedback", right: "global").
#[test]
fn with_no_link_the_desktop_lands_in_global() {
    let main = humanity_protocol::chat_access::MAIN_CHANNEL;
    assert_eq!(main, "global");
    assert_eq!(GuiState::default().chat_active_channel, main, "the first open is #global");
    let mut state = two_servers();
    state.chat_active_channel = "feedback".into();
    state.switch_to_server(CLOSED, "Closed");
    assert_eq!(state.chat_active_channel, main, "a switch to a server being dialled lands in #global");
}

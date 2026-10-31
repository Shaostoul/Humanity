//! From gui/pages/chat.rs (`connect_form_tests`, BUG-160): the connect form's Server field, through
//! the engine's background connections (`bg_connections`).

//! BUG-160: the connect form's Server field is a draft that Connect dials, proven drawn, clicked
//! and typed into the way a person does it, and (its follow-up) the hold on that draft holds
//! nothing else: not the connection the person already has, nor one Connect parks. Here rather
//! than beside the form because pages/chat/left_panel.rs stands at its line budget
//! (tests/file_size_ratchet.rs). No test here reaches a server: every address is a loopback
//! port nothing listens on (`nowhere`), and Connect's save of the config is kept off the disk.

use crate::gui::pages::chat::*;
use crate::engine::bg_connections::{link_frame, redial_due};
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::pages::chat::left_panel::draw_left_panel;

/// One headless frame (no GPU) of the left panel, with `events`.
fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(320.0, 900.0))),
        events,
        ..Default::default()
    };
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw_left_panel(ui, theme, state));
    })
}

/// After a restart with no server, an address typed into the empty Server field is dialled
/// by Connect, not letter by letter by itself: the first letter made the address non-empty,
/// so the auto-connect dialled "h" and the background pump then started the saved servers.
/// The field's edit holds both until Connect (`hold_dialling_until_connect`; since BUG-160's
/// follow-up a hold of its own, not a Disconnect).
///
/// Seen red 2026-10-05 with B1's change in place and the edit holding nothing: "the first
/// letter typed after a restart with no server was dialled".
#[test]
fn typing_into_the_empty_server_field_waits_for_connect() {
    // A restart with no server: the config's empty address, loaded, and the identity
    // unlocked (`apply_pq_identity`, which every unlock runs).
    let cfg: crate::config::AppConfig =
        serde_json::from_str(r#"{"server_url":"","user_name":"Ada","completed_onboarding":true}"#).expect("the config loads");
    let mut state = GuiState::default();
    cfg.apply_to_gui_state(&mut state);
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.apply_pq_identity();
    assert!(!state.may_auto_connect(), "a restart with no server dialled one by itself");

    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    // Click the empty field (its suggestion is drawn inside it), then type one letter.
    let out = frame(&ctx, &theme, &mut state, Vec::new());
    let field = find_text_in_shapes(&out.shapes, crate::gui::OFFICIAL_SERVER).expect("the empty field is drawn");
    let (pos, m) = (field.rect.center(), egui::Modifiers::default());
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(pos)]);
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    frame(&ctx, &theme, &mut state, vec![egui::Event::Text("h".into())]);
    assert_eq!(state.server_url, "h", "the letter did not reach the field");
    assert!(!state.may_auto_connect(), "the first letter typed after a restart with no server was dialled");
    assert!(!state.may_dial_saved_servers(), "typing an address started the background links");
}

/// A context and the app's theme, set up as the app sets them.
fn headless() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

/// Press and release the pointer on what is drawn as `text`, the way a person clicks it.
fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = frame(ctx, theme, state, Vec::new());
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text} is not drawn")).rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

/// A key pressed in the field that has the keyboard.
fn press(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, key: egui::Key, modifiers: egui::Modifiers) {
    frame(ctx, theme, state, vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }]);
}

/// Click the Server field showing `from`, select what it holds and type `to` over it.
fn retype(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, from: &str, to: &str) {
    click(ctx, theme, state, from);
    press(ctx, theme, state, egui::Key::A, egui::Modifiers::COMMAND);
    frame(ctx, theme, state, vec![egui::Event::Text(to.into())]);
    assert_eq!(state.server_url, to, "the address did not reach the field");
}

/// `n` different addresses on this machine where nothing listens (ports opened together,
/// so they differ, and closed again): a socket dialled to one fails at once, and no test
/// here reaches a real server.
fn nowhere(n: usize) -> Vec<String> {
    let held: Vec<std::net::TcpListener> =
        (0..n).map(|_| std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port")).collect();
    held.iter().map(|l| format!("http://127.0.0.1:{}", l.local_addr().expect("its address").port())).collect()
}

/// A person with an unlocked identity whose connection to `address` is being opened: the
/// socket is dialled and not up yet, so the connect form is shown, as it is until the
/// socket opens and while a dropped one waits to reconnect.
fn opening(address: &str) -> GuiState {
    let mut state = GuiState::default();
    state.onboarding_complete = true;
    state.user_name = "Ada".to_string();
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.apply_pq_identity();
    state.server_url = address.to_string();
    state.connected_server_url = address.to_string();
    let socket = crate::net::ws_client::WsClient::connect_with_key_card(
        &derive_ws_url(address),
        &state.user_name,
        &state.profile_public_key,
        &state.own_key_card,
    );
    state.ws_client = Some(socket);
    state
}

/// The review of BUG-160's completion, item 3: with the person on server A, another address
/// typed into the Server field and Connect pressed, A is parked, and the typing hold went
/// with it: it was the Disconnect flag, and the background pump never re-arms or redials a
/// link with that flag set. So at A's next drop (every deploy restarts the relay) A was never
/// dialled again, and its chat and DMs stopped for the session. Run with the pump's own two
/// steps (engine/bg_connections.rs).
///
/// Seen red 2026-10-05 with the hold as on 347c8f77b: "A, parked by Connect, was never
/// redialled after its socket dropped".
#[test]
fn connecting_elsewhere_leaves_the_server_you_left_reconnecting_after_a_drop() {
    crate::config::keep_saves_off_disk();
    let places = nowhere(2);
    let (a, b) = (&places[0], &places[1]);
    let mut state = opening(a);
    let (ctx, theme) = headless();
    retype(&ctx, &theme, &mut state, a, b);
    click(&ctx, &theme, &mut state, "Connect");
    assert_eq!(state.connected_server_url, *b, "Connect did not dial the typed address");
    let parked = state.connections.iter_mut().find(|c| c.url == norm_server_url(a)).expect("A was parked, socket and all");
    // A's socket drops, as at a relay restart: the pump lets it go and counts down.
    parked.ws.as_mut().expect("A's socket went with it").disconnect();
    let redialled = (0..240).any(|_| {
        link_frame(parked, 0.5);
        redial_due(parked)
    });
    assert!(redialled, "A, parked by Connect, was never redialled after its socket dropped");
}

/// The same review, item 3, without a switch: an edit of the Server field while the
/// connection is up or coming up (here: a letter typed and deleted, so the field holds its
/// address again) set the Disconnect flag on that live connection, and at its next drop it
/// was not reconnected. Its socket's drop runs the pump's own teardown
/// (`active_socket_dropped`, engine/frame_ws_poll.rs).
///
/// Seen red 2026-10-05 with the hold as on 347c8f77b: "an edit of the Server field, typed and
/// deleted, left the connection with no reconnect after its drop".
#[test]
fn an_edit_typed_and_deleted_leaves_the_connection_reconnecting_after_a_drop() {
    let a = nowhere(1).remove(0);
    let mut state = opening(&a);
    let (ctx, theme) = headless();
    click(&ctx, &theme, &mut state, &a);
    frame(&ctx, &theme, &mut state, vec![egui::Event::Text("x".into())]);
    assert_ne!(state.server_url, a, "the letter did not reach the field");
    press(&ctx, &theme, &mut state, egui::Key::Backspace, egui::Modifiers::default());
    assert_eq!(state.server_url, a, "the letter was not deleted");
    state.active_socket_dropped();
    assert!(state.backoff_reconnect_runs(), "an edit of the Server field, typed and deleted, left the connection with no reconnect after its drop");
    assert_eq!(state.dial_address(), Some(a.as_str()), "the reconnect does not dial the connection's own server");
}

/// The same review, item 3, during a reconnect: the connection to A dropped and is counting
/// down to a redial when another address is typed into the field. The typed address waits for
/// Connect, and A is still redialled, to A (lib.rs dials `dial_address`), not to the address
/// being typed. When A's socket is up again the form is gone, and the field holds A's address,
/// which the app files A's messages under, instead of the abandoned draft.
///
/// Seen red 2026-10-05 with the hold as on 347c8f77b: "an address typed while A was
/// reconnecting stopped A's reconnect".
#[test]
fn an_address_typed_during_a_reconnect_waits_and_the_connection_still_reconnects() {
    let places = nowhere(2);
    let (a, b) = (&places[0], &places[1]);
    let mut state = opening(a);
    state.active_socket_dropped();
    assert!(state.backoff_reconnect_runs(), "the setup: A's reconnect is counting down");
    let (ctx, theme) = headless();
    retype(&ctx, &theme, &mut state, a, b);
    assert!(state.backoff_reconnect_runs(), "an address typed while A was reconnecting stopped A's reconnect");
    assert_eq!(state.dial_address(), Some(a.as_str()), "the reconnect would dial the address being typed");
    // A's socket opens again (engine/frame_ws_poll.rs runs this every frame it is up).
    state.active_socket_up();
    assert_eq!(state.server_url, *a, "with A up again the field still names the abandoned draft");
    assert!(!state.typed_address_held());
}

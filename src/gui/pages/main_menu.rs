//! Main menu / onboarding screen.
//!
//! First-run: walks user through welcome, server connection, identity setup.
//! Returning user: shows the main hub with quick-access buttons.

use egui::{Align2, Color32, RichText, Vec2};
use crate::gui::{connect_target, GuiPage, GuiState, OFFICIAL_SERVER, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;

#[cfg(test)]
thread_local! {
    /// Test-only stand-in for the build version. The menu prints it, so its
    /// snapshot changed on every release and could never be a baseline.
    /// `set_version_for_snapshot` pins it for the calling thread, the way
    /// `cosmos::set_clock_for_snapshot` pins that page's clock.
    static TEST_VERSION: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// Test-only: pin (or with `None`, release) the version this menu shows, for
/// the calling thread.
#[cfg(test)]
pub(crate) fn set_version_for_snapshot(version: Option<&'static str>) {
    TEST_VERSION.with(|c| c.set(version));
}

/// The version the menu prints: the build's own, except under a snapshot pin.
fn shown_version() -> &'static str {
    #[cfg(test)]
    if let Some(v) = TEST_VERSION.with(|c| c.get()) {
        return v;
    }
    VERSION
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Full-screen dark backdrop — derived from theme.bg_primary with 94% alpha
    // so the 3D world (if rendered behind) shows through faintly.
    let bg = theme.bg_primary();
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 240));

    // First-boot storage chooser (v0.707, operator design): on a truly fresh
    // machine (nothing beside the exe, nothing in the OS dir) the user picks
    // WHERE HumanityOS keeps their files BEFORE anything is written -- ahead
    // of identity creation, so the encrypted identity lands in the chosen
    // place. Existing installs never see this (they detect Installed /
    // LegacyBesideExe / Portable from what's already on disk).
    if crate::storage::mode() == crate::storage::StorageMode::Undecided
        && !state.onboarding_complete
    {
        draw_storage_chooser(ctx, theme);
        return;
    }

    if !state.onboarding_complete {
        draw_onboarding(ctx, theme, state);
    } else {
        draw_hub(ctx, theme, state);
    }
}

/// The first-boot "where should HumanityOS keep your files?" step. Two clear
/// choices; nothing is written to disk until one is made. GUI-first rule:
/// this is an in-app step, never an installer-only or CLI decision.
fn draw_storage_chooser(ctx: &egui::Context, theme: &Theme) {
    egui::Window::new("storage_chooser")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(520.0, 0.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                ui.label(RichText::new("Welcome to").size(16.0).color(theme.text_secondary()));
                ui.label(RichText::new("HumanityOS").size(32.0).color(theme.accent()));
                ui.add_space(18.0);
                ui.label(
                    RichText::new("Where should HumanityOS keep your files?")
                        .size(17.0)
                        .strong()
                        .color(theme.text_primary()),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Saves, settings, your identity, and editable game data.")
                        .size(13.0)
                        .color(theme.text_muted()),
                );
                ui.add_space(18.0);

                if widgets::Button::primary("My user folder (recommended)").show(ui, theme) {
                    crate::storage::choose_installed();
                }
                ui.label(
                    RichText::new(
                        "Kept safe in your user profile. The app can be moved,\n\
                         updated, or deleted without losing anything.",
                    )
                    .size(12.5)
                    .color(theme.text_secondary()),
                );
                ui.add_space(14.0);

                if widgets::Button::secondary("Next to the app (portable)").show(ui, theme) {
                    crate::storage::choose_portable();
                }
                ui.label(
                    RichText::new(
                        "Everything stays in this folder, so a USB or external\n\
                         drive carries the whole thing between computers.\n\
                         Best if the app is in its own folder.",
                    )
                    .size(12.5)
                    .color(theme.text_secondary()),
                );
                ui.add_space(22.0);
            });
        });
}

/// First-run onboarding flow.
fn draw_onboarding(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("onboarding")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        // Width fixed, height from the content (the storage chooser's
        // pattern). A fixed 520 px height clipped the identity step, which is
        // taller once the 24 words are showing: the Back button under Finish
        // Setup was cut in half (2026-09-27 snapshot review).
        .fixed_size(Vec2::new(500.0, 0.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            match state.onboarding_step {
                0 => draw_step_welcome(ui, theme, state),
                1 => draw_step_server(ui, theme, state),
                2 => draw_step_identity(ui, theme, state),
                _ => draw_step_ready(ui, theme, state),
            }
        });
}

/// Step 0: Welcome
fn draw_step_welcome(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.vertical_centered(|ui| {
        ui.add_space(30.0);
        ui.label(RichText::new("Welcome to").size(18.0).color(theme.text_secondary()));
        ui.add_space(4.0);
        ui.label(RichText::new("HumanityOS").size(36.0).color(theme.accent()));
        ui.add_space(8.0);
        ui.label(RichText::new("End poverty. Unite humanity.").size(14.0).color(theme.text_secondary()));
        ui.add_space(30.0);

        ui.label(RichText::new(
            "A free platform for communication, survival education,\n\
             resource management, and 3D simulation."
        ).size(14.0).color(theme.text_primary()));

        ui.add_space(12.0);

        ui.label(RichText::new(
            "Your identity is a post-quantum cryptographic key.\n\
             No sign-up, no passwords stored on any server, no tracking. You own your data."
        ).size(13.0).color(theme.text_muted()));

        ui.add_space(30.0);

        if widgets::primary_button(ui, theme, "   Get Started   ") {
            state.onboarding_step = 1;
        }
        ui.add_space(8.0);
        if ui.small_button("Skip setup (offline mode)").clicked() {
            skip_setup(state);
            crate::config::AppConfig::from_gui_state(state).save();
        }

        ui.add_space(16.0);
        ui.label(RichText::new(format!("v{}", shown_version())).size(11.0).color(theme.text_muted()));
    });
}

/// `<server_url>/health` -- the same endpoint every relay instance exposes
/// (`GET /health`, see src/relay/mod.rs), used here purely as a lightweight
/// reachability probe. Mirrors `chat::derive_ws_url`'s normalization but
/// keeps the http(s) scheme instead of converting to ws(s).
fn derive_health_url(url: &str) -> String {
    let base = url.trim_end_matches('/');
    if base.ends_with("/health") {
        base.to_string()
    } else {
        format!("{base}/health")
    }
}

/// The server step's Connect, as the Chat page's: it checks the server the Chat page's Connect
/// dials for this field (`connect_target`, the official server for an empty field), and the
/// field then holds it, so the ready step names that server and the app dials it once setup is
/// done; the address is chosen now, no longer being typed. Returns the address of its
/// `/health`. Before BUG-160's follow-up an empty field was checked as "/health", which fails,
/// while the step said the official server was the default.
fn choose_server_to_check(state: &mut GuiState) -> String {
    state.server_url = connect_target(&state.server_url).to_string();
    state.server_field_draft = false;
    derive_health_url(&state.server_url)
}

/// What the server step says under its field: the empty field's suggestion is the official
/// server, which Connect uses, the way the Chat page's connect form shows it (`connect_target`).
/// It used to say "Default: united-humanity.us", and a new player who cleared the field and
/// pressed Skip ended with no server, as an empty field means (BUG-160).
fn empty_field_note() -> String {
    let host = OFFICIAL_SERVER.trim_start_matches("https://");
    format!("With the field empty, Connect uses {host}, the official community server.")
}

#[cfg(test)]
thread_local! {
    /// Test-only: the `/health` addresses the server step's Connect checked on this thread. A
    /// test build records them instead of sending the request, so a test can press Connect
    /// without reaching any server (BUG-160 was rigs reaching the live one).
    static CHECKS_STARTED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Poll the in-flight reachability check (if any) started by the "Connect"
/// button below, applying its result to `server_connected`/
/// `server_check_error` once it arrives. A no-op while idle or still
/// checking. Extracted so the receive-and-apply logic is unit-testable
/// without a real network call or a real egui frame.
fn poll_server_check(state: &mut GuiState) {
    let Some(rx) = state.server_check_rx.as_ref() else { return };
    match rx.try_recv() {
        Ok(Ok(())) => {
            state.server_connected = true;
            state.server_check_error.clear();
            state.server_check_rx = None;
        }
        Ok(Err(e)) => {
            state.server_connected = false;
            state.server_check_error = e;
            state.server_check_rx = None;
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {} // still checking
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            state.server_connected = false;
            state.server_check_error = "Check failed unexpectedly (no response).".to_string();
            state.server_check_rx = None;
        }
    }
}

/// Step 1: Server connection
fn draw_step_server(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    poll_server_check(state);
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(RichText::new("Connect to a Server").size(24.0).color(theme.accent()));
        ui.add_space(8.0);
        ui.label(RichText::new(
            "Servers host communities. You can join any server\n\
             or run your own. This step is optional."
        ).size(13.0).color(theme.text_secondary()));
        ui.add_space(24.0);
    });

    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new("Server URL:").size(14.0).color(theme.text_primary()));
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        let response = ui.add_sized(
            Vec2::new(380.0, 30.0),
            egui::TextEdit::singleline(&mut state.server_url)
                .hint_text(OFFICIAL_SERVER),
        );
        if response.changed() {
            // The URL changed -- any in-flight or previous check result is
            // for a different address now, so drop it rather than apply a
            // stale outcome to the newly-typed URL.
            state.server_connected = false;
            state.server_check_error.clear();
            state.server_check_rx = None;
            // As on the Chat page, an address typed here is dialled by Connect, never by
            // itself once setup is done (BUG-160): Skip leaves it undialled.
            state.hold_dialling_until_connect();
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(empty_field_note()).size(11.0).color(theme.text_muted()));
    });

    ui.add_space(16.0);

    if state.server_connected {
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("Reachable!").size(14.0).color(theme.success()));
        });
    } else if state.server_check_rx.is_some() {
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("Checking...").size(14.0).color(theme.text_muted()));
        });
    } else if !state.server_check_error.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new(&state.server_check_error).size(12.0).color(theme.danger()));
        });
    }

    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        if !state.server_connected {
            let checking = state.server_check_rx.is_some();
            if widgets::primary_button(ui, theme, if checking { "  Checking...  " } else { "  Connect  " }) && !checking {
                // A real lightweight reachability probe (GET .../health, the
                // same endpoint every relay exposes) on a background thread --
                // see poll_server_check's doc comment for why this isn't the
                // full WS identify handshake (that genuinely can't happen
                // until onboarding completes and identity exists).
                let (tx, rx) = std::sync::mpsc::channel();
                state.server_check_rx = Some(rx);
                state.server_check_error.clear();
                let health_url = choose_server_to_check(state);
                #[cfg(test)]
                CHECKS_STARTED.with(|checks| checks.borrow_mut().push(health_url.clone()));
                if !cfg!(test) {
                    std::thread::spawn(move || {
                        let result = ureq::get(&health_url)
                            .call()
                            .map(|_| ())
                            .map_err(|e| format!("Could not reach {health_url}: {e}"));
                        let _ = tx.send(result);
                    });
                }
            }
        } else {
            if widgets::primary_button(ui, theme, "  Continue  ") {
                state.onboarding_step = 2;
            }
        }
        ui.add_space(8.0);
        if ui.small_button("Skip (stay offline)").clicked() {
            skip_server_step(state);
        }
        ui.add_space(4.0);
        if ui.small_button("Back").clicked() {
            state.onboarding_step = 0;
        }
    });
}

#[cfg(test)]
mod server_check_tests {
    use super::{derive_health_url, poll_server_check};
    use crate::gui::GuiState;

    #[test]
    fn derive_health_url_appends_the_endpoint() {
        assert_eq!(derive_health_url("https://united-humanity.us"), "https://united-humanity.us/health");
        assert_eq!(derive_health_url("https://united-humanity.us/"), "https://united-humanity.us/health");
    }

    #[test]
    fn derive_health_url_is_idempotent() {
        // Must not double-append if it's somehow already there.
        assert_eq!(
            derive_health_url("https://united-humanity.us/health"),
            "https://united-humanity.us/health"
        );
    }

    #[test]
    fn poll_is_a_no_op_when_idle() {
        let mut state = GuiState::default();
        assert!(state.server_check_rx.is_none());
        poll_server_check(&mut state);
        assert!(!state.server_connected);
        assert!(state.server_check_error.is_empty());
    }

    #[test]
    fn poll_applies_a_success_result() {
        let mut state = GuiState::default();
        let (tx, rx) = std::sync::mpsc::channel();
        state.server_check_rx = Some(rx);
        state.server_check_error = "stale error from a previous check".to_string();
        tx.send(Ok(())).unwrap();
        poll_server_check(&mut state);
        assert!(state.server_connected);
        assert!(state.server_check_error.is_empty(), "a fresh success must clear a stale error");
        assert!(state.server_check_rx.is_none(), "the receiver is consumed once the result lands");
    }

    #[test]
    fn poll_applies_a_failure_result_without_faking_connected() {
        let mut state = GuiState::default();
        let (tx, rx) = std::sync::mpsc::channel();
        state.server_check_rx = Some(rx);
        tx.send(Err("Could not reach https://bad.example/health: timeout".to_string())).unwrap();
        poll_server_check(&mut state);
        assert!(!state.server_connected, "a failed reachability check must never flip server_connected true");
        assert_eq!(state.server_check_error, "Could not reach https://bad.example/health: timeout");
        assert!(state.server_check_rx.is_none());
    }

    #[test]
    fn poll_leaves_state_untouched_while_still_checking() {
        let mut state = GuiState::default();
        let (_tx, rx) = std::sync::mpsc::channel(); // sender kept alive, nothing sent yet
        state.server_check_rx = Some(rx);
        poll_server_check(&mut state);
        assert!(!state.server_connected);
        assert!(state.server_check_rx.is_some(), "still checking -- must not clear the receiver early");
    }

    #[test]
    fn poll_handles_a_dropped_sender_without_fabricating_success() {
        let mut state = GuiState::default();
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        state.server_check_rx = Some(rx);
        drop(tx); // simulates the background thread dying without sending
        poll_server_check(&mut state);
        assert!(!state.server_connected, "a dead checker thread must never be reported as reachable");
        assert!(!state.server_check_error.is_empty());
        assert!(state.server_check_rx.is_none());
    }
}

/// BUG-160's follow-up (the review of its completion, item 10): the onboarding's server step
/// keeps the Chat page's rule for its field. An empty field is no server, its suggestion is the
/// official server, Connect checks and takes `connect_target`, and an address typed and never
/// connected is not dialled by itself. Drawn, clicked and typed into headlessly; Connect's
/// request is recorded, never sent.
#[cfg(test)]
mod server_step_tests {
    use super::{draw_step_server, empty_field_note, finish_onboarding, skip_server_step, CHECKS_STARTED};
    use crate::gui::screen_surface::find_text_in_shapes;
    use crate::gui::theme::Theme;
    use crate::gui::{GuiState, OFFICIAL_SERVER};

    fn headless() -> (egui::Context, Theme) {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        (ctx, theme)
    }

    /// One headless frame (no GPU) of the server step, with `events`.
    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_step_server(ui, theme, state));
        })
    }

    /// Where the text drawn is exactly `text`. The button reads "  Connect  ", and
    /// `find_text_in_shapes` trims what it looks for, so it finds the heading "Connect to a
    /// Server" first.
    fn exactly(shapes: &[egui::epaint::ClippedShape], text: &str) -> Option<egui::Rect> {
        fn walk(shape: &egui::Shape, text: &str, found: &mut Option<egui::Rect>) {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == text => *found = Some(t.galley.rect.translate(t.pos.to_vec2())),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, text, found)),
                _ => {}
            }
        }
        let mut found = None;
        shapes.iter().for_each(|cs| walk(&cs.shape, text, &mut found));
        found
    }

    /// Press and release the pointer at `pos`, the way a person clicks.
    fn click_at(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, pos: egui::Pos2) {
        let m = egui::Modifiers::default();
        frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }

    /// A new player who cleared the field presses Connect: the step checks the official server,
    /// the server the Chat page's Connect dials for an empty field (`connect_target`), and the
    /// field then holds it, so the ready step names it and the app dials it once setup is done.
    ///
    /// Seen red 2026-10-05 with the step's Connect as on 347c8f77b: "the server step's Connect
    /// checked [\"/health\"] for an empty field".
    #[test]
    fn the_server_steps_connect_with_an_empty_field_uses_the_official_server() {
        CHECKS_STARTED.with(|checks| checks.borrow_mut().clear());
        let mut state = GuiState::default();
        state.server_url.clear();
        let (ctx, theme) = headless();
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        let connect = exactly(&out.shapes, "  Connect  ").expect("the Connect button is drawn");
        click_at(&ctx, &theme, &mut state, connect.center());
        let checked = CHECKS_STARTED.with(|checks| checks.borrow().clone());
        assert_eq!(checked, vec![format!("{OFFICIAL_SERVER}/health")], "the server step's Connect checked {checked:?} for an empty field");
        assert_eq!(state.server_url, OFFICIAL_SERVER, "the field does not hold the server Connect checked");
    }

    /// The step says what an empty field means the way the Chat page's connect form shows it:
    /// the official server is the empty field's suggestion, which Connect uses. It said
    /// "Default: united-humanity.us", and a new player who cleared the field and pressed Skip
    /// ended with no server, which is what an empty field means (BUG-160).
    ///
    /// Seen red 2026-10-05 with the step as on 347c8f77b: "the step still calls the official
    /// server a default".
    #[test]
    fn the_server_step_says_what_an_empty_field_means_as_the_chat_page_does() {
        let mut state = GuiState::default();
        state.server_url.clear();
        let (ctx, theme) = headless();
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        assert!(find_text_in_shapes(&out.shapes, "Default:").is_none(), "the step still calls the official server a default");
        assert!(exactly(&out.shapes, OFFICIAL_SERVER).is_some(), "the empty field does not suggest the official server");
        assert!(exactly(&out.shapes, &empty_field_note()).is_some(), "the step does not say what Connect does with an empty field");
        assert!(empty_field_note().contains(OFFICIAL_SERVER.trim_start_matches("https://")), "the note does not name the official server");
    }

    /// The Chat page's typing hold, on this step too: an address typed here and not checked with
    /// Connect is not dialled by itself once setup is done (the button says "stay offline"), as an
    /// address typed into the Chat page's field waits for its Connect. The address the step
    /// starts with, never edited, is dialled as before: chat connects by itself (first-hour audit).
    ///
    /// Seen red 2026-10-05 with the step's field as on 347c8f77b: "an address typed on the server
    /// step and skipped was dialled once setup was done".
    #[test]
    fn an_address_typed_on_the_server_step_and_skipped_is_not_dialled() {
        let skip_and_finish = |state: &mut GuiState| {
            skip_server_step(state);
            state.user_name = "Ada".to_string();
            state.private_key_bytes = Some(vec![7u8; 32]);
            finish_onboarding(state);
        };
        let mut untouched = GuiState::default();
        skip_and_finish(&mut untouched);
        assert!(untouched.may_auto_connect(), "the setup itself must allow a connect");

        let mut state = GuiState::default();
        let (ctx, theme) = headless();
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        let field = exactly(&out.shapes, OFFICIAL_SERVER).expect("the field holds the official server");
        click_at(&ctx, &theme, &mut state, field.center());
        let select_all = egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
        frame(&ctx, &theme, &mut state, vec![select_all]);
        frame(&ctx, &theme, &mut state, vec![egui::Event::Text("https://my.server".into())]);
        assert_eq!(state.server_url, "https://my.server", "the address did not reach the field");
        skip_and_finish(&mut state);
        assert!(!state.may_auto_connect(), "an address typed on the server step and skipped was dialled once setup was done");
    }
}

/// Step 2: Identity / display name
/// A note indented 40 px like the rest of the identity step, WRAPPING inside
/// the window with the same 40 px margin on the right. A plain label in a
/// `ui.horizontal` does not wrap: the long recovery-phrase warning stretched the
/// 500 px window to about 975 px (2026-09-27 snapshot review).
fn indented_note(ui: &mut egui::Ui, text: RichText) {
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        let w = (ui.available_width() - 40.0).max(120.0);
        ui.scope(|ui| {
            ui.set_max_width(w);
            ui.add(egui::Label::new(text).wrap());
        });
    });
}

fn draw_step_identity(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(RichText::new("Your Identity").size(24.0).color(theme.accent()));
        ui.add_space(8.0);
        ui.label(RichText::new(
            "Pick the name people will see. There is nothing to sign\n\
             up for: the app creates a secret key that stays on your device."
        ).size(13.0).color(theme.text_secondary()));
        ui.add_space(16.0);
    });

    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new("Display Name:").size(14.0).color(theme.text_primary()));
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.add_sized(
            Vec2::new(380.0, 30.0),
            egui::TextEdit::singleline(&mut state.user_name)
                .hint_text("Enter your name"),
        );
    });

    ui.add_space(12.0);

    // ── Generate a New Identity ──
    // The onboarding header promises the PQ identity is "generated
    // automatically" — but nothing actually created it (Finish Setup
    // only advanced the step), so new users ended up identity-less.
    // This button is that missing primitive.
    if state.private_key_bytes.is_none() {
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            if widgets::primary_button(ui, theme, "  Generate New Identity  ") {
                let seed = crate::net::identity::generate_new_seed();
                state.private_key_bytes = Some(seed);
                state.apply_pq_identity(); // derive Dilithium+Kyber + connect
                state.identity_recovered = true;
                // Show the 24 recovery words RIGHT HERE (below) instead of
                // sending a brand-new user hunting through Settings for the
                // only backup of their account (first-run friction pass).
                state.settings.seed_phrase_visible = true;
                state.settings.seed_phrase_recovery_status =
                    "Identity created. Your 24 recovery words are below -- write them down before you continue.".to_string();
                state.passphrase_needed = true;
                state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
            }
        });
        ui.add_space(4.0);
        indented_note(
            ui,
            RichText::new("Creates a fresh 24-word recovery phrase (your only backup). Or recover an existing one below.")
                .size(11.0)
                .color(theme.text_secondary()),
        );
        ui.add_space(8.0);
    }

    // ── First-run seed backup, shown IN PLACE (the 24-word moment is the
    // scariest step in the whole funnel for a non-technical person -- the
    // words appear here with plain instructions the moment the identity is
    // created, not behind a Settings page they have never opened) ──
    if state.settings.seed_phrase_visible {
        if let Some(words) = state
            .private_key_bytes
            .as_ref()
            .and_then(|s| crate::net::identity::mnemonic_from_seed(s))
        {
            ui.add_space(10.0);
            indented_note(
                ui,
                RichText::new("Write these 24 words on paper, in this order. They ARE your account.")
                    .size(13.0)
                    .strong()
                    .color(theme.warning()),
            );
            indented_note(
                ui,
                RichText::new(
                    "Anyone who has them can be you, and if you lose them nobody can reset or \
                     recover your account -- not even us. Paper beats a screenshot: photos get \
                     synced, hacked, and lost.",
                )
                .size(11.0)
                .color(theme.text_secondary()),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(40.0);
                egui::Grid::new("onboarding_seed_words").num_columns(4).spacing([18.0, 4.0]).show(ui, |ui| {
                    for (i, w) in words.split_whitespace().enumerate() {
                        ui.label(
                            RichText::new(format!("{:>2}. {w}", i + 1))
                                .monospace()
                                .size(13.0)
                                .color(theme.text_primary()),
                        );
                        if (i + 1) % 4 == 0 {
                            ui.end_row();
                        }
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(40.0);
                if widgets::secondary_button(ui, theme, "I wrote them down -- hide the words") {
                    state.settings.seed_phrase_visible = false;
                    state.settings.seed_phrase_recovery_status =
                        "Good. Keep that paper somewhere safe -- you can see the words again any time in Settings -> Identity.".to_string();
                }
            });
            ui.add_space(8.0);
        }
    }

    // ── Restore from Recovery Phrase ──
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        if ui.small_button(
            if state.settings.seed_phrase_show_recover { "Cancel recovery" } else { "Restore an existing account from its recovery phrase" }
        ).clicked() {
            state.settings.seed_phrase_show_recover = !state.settings.seed_phrase_show_recover;
            state.settings.seed_phrase_recovery_status.clear();
        }
    });

    if state.settings.seed_phrase_show_recover {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.vertical(|ui| {
                ui.add(egui::TextEdit::multiline(&mut state.settings.seed_phrase_input)
                    .desired_width(380.0)
                    .desired_rows(2)
                    .hint_text("Enter your 24-word recovery phrase"));
                ui.add_space(4.0);
                if widgets::primary_button(ui, theme, "Recover") {
                    let phrase = state.settings.seed_phrase_input.trim().to_string();
                    match crate::net::identity::derive_keypair_from_mnemonic(&phrase) {
                        Ok((_ed25519_hex, privkey_bytes)) => {
                            // Full-PQ: the chat identity is Dilithium3 (derived
                            // from the SAME seed, byte-identical to web). The
                            // Ed25519 hex is kept only as the Solana wallet.
                            match crate::net::identity::derive_pq_identity(&privkey_bytes) {
                                Ok(pq) => {
                                    state.settings.seed_phrase_recovery_status = format!(
                                        "Recovered: {}...{}",
                                        &pq.dilithium_hex[..8],
                                        &pq.dilithium_hex[pq.dilithium_hex.len()-8..]
                                    );
                                    state.private_key_bytes = Some(privkey_bytes);
                                    // Canonical: derive Dilithium+Kyber and
                                    // force the reconnect that advertises
                                    // kyber_public (same path as unlock).
                                    state.apply_pq_identity();
                                    state.identity_recovered = true;
                                    state.settings.seed_phrase_input.clear();
                                    state.settings.seed_phrase_show_recover = false;
                                    // Prompt for a passphrase to encrypt the seed.
                                    state.passphrase_needed = true;
                                    state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
                                }
                                Err(e) => {
                                    state.settings.seed_phrase_recovery_status =
                                        format!("Error deriving PQ identity: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            state.settings.seed_phrase_recovery_status = format!("Error: {}", e);
                        }
                    }
                }
                if !state.settings.seed_phrase_recovery_status.is_empty() {
                    let color = if state.settings.seed_phrase_recovery_status.starts_with("Error") {
                        theme.danger()
                    } else {
                        theme.success()
                    };
                    ui.label(RichText::new(&state.settings.seed_phrase_recovery_status).color(color).size(11.0));
                }
            });
        });
    }

    // Show current public key if set
    if !state.profile_public_key.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            let key = &state.profile_public_key;
            let display = if key.len() > 16 {
                format!("Identity: {}...{}", &key[..8], &key[key.len()-8..])
            } else {
                format!("Identity: {}", key)
            };
            ui.label(RichText::new(display).size(11.0).color(theme.success()));
        });
    }

    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        let has_identity = state.private_key_bytes.is_some();
        if has_identity {
            if widgets::primary_button(ui, theme, "  Finish Setup  ") {
                // Step F's recovery-phrase guard on the display name (it is sent whenever the app
                // connects): holding the phrase, setup stops there and the field goes back to the
                // name last saved.
                let typed = state.user_name.clone();
                if crate::engine::warnings::guard_stops(state, &[&typed]) {
                    state.user_name = crate::config::AppConfig::load().user_name;
                } else {
                    state.onboarding_step = 3;
                }
            }
        } else {
            // Cannot finish without an identity — that was the ghost-
            // account bug. Force Generate or Recover first.
            ui.label(RichText::new("Generate or recover an identity above to continue.")
                .size(12.0).color(theme.warning()));
        }
        ui.add_space(4.0);
        if ui.small_button("Back").clicked() {
            state.onboarding_step = 1;
        }
    });
}

/// Step 3: Ready
fn draw_step_ready(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new("You're Ready!").size(28.0).color(theme.accent()));
        ui.add_space(16.0);

        let name_display = if state.user_name.is_empty() {
            "Explorer".to_string()
        } else {
            state.user_name.clone()
        };
        ui.label(RichText::new(format!("Welcome, {}.", name_display)).size(16.0).color(theme.text_primary()));

        ui.add_space(8.0);

        let server_status = if state.server_connected {
            format!("Connected to {}", state.server_url)
        } else {
            "Offline mode".to_string()
        };
        ui.label(RichText::new(server_status).size(13.0).color(theme.text_secondary()));
        ui.add_space(8.0);
        ui.label(RichText::new(where_you_start(state)).size(13.0).color(theme.text_secondary()));

        ui.add_space(40.0);

        if widgets::primary_button(ui, theme, "  Enter HumanityOS  ") {
            finish_onboarding(state);
            crate::config::AppConfig::from_gui_state(state).save();
        }

        ui.add_space(30.0);
        ui.label(RichText::new(
            "Press Escape anytime to open the menu.\n\
             Press Enter to toggle chat."
        ).size(12.0).color(theme.text_muted()));
    });
}

/// The welcome step's "Skip setup (offline mode)": onboarding done with no identity, into the
/// player's own home, alone (`GuiState::start_at_home_alone`), so an identity made later in
/// Settings does not quietly join a shared world either. The caller saves the config.
pub(crate) fn skip_setup(state: &mut GuiState) {
    state.onboarding_complete = true;
    state.start_at_home_alone();
    state.active_page = GuiPage::None;
}

/// The server step's "Skip (stay offline)": the player chose not to use the server, so a
/// reachability check that already succeeded (or is still running) is dropped, and Enter does
/// not take it for a choice of that server's world (`finish_onboarding`).
pub(crate) fn skip_server_step(state: &mut GuiState) {
    state.server_connected = false;
    state.server_check_rx = None;
    state.server_check_error.clear();
    state.onboarding_step = 2;
}

/// The ready step's "Enter HumanityOS" (first-hour audit 2026-10-04, Blocker 1). A player who
/// connected to a server on the server step and pressed Continue chose it: Chat opens, and that
/// server's shared world is where their character goes, as before. Everyone else starts in
/// their own home, alone (`GuiState::start_at_home_alone`): chat still connects by itself, but
/// joining a server's WORLD is one deliberate pick in Characters. The caller saves the config
/// (a test never writes the real one).
pub(crate) fn finish_onboarding(state: &mut GuiState) {
    state.onboarding_complete = true;
    if state.server_connected {
        state.active_page = GuiPage::Chat;
    } else {
        state.start_at_home_alone();
        state.active_page = GuiPage::None; // Enter the 3D world
    }
}

/// Where Enter puts the player, said on the ready step before they press it (first-hour audit
/// 2026-10-04, Blocker 1: the shared world used to be joined without a word).
fn where_you_start(state: &GuiState) -> &'static str {
    if state.server_connected {
        "Your character will play in this server's shared world."
    } else {
        "You start in your own home, playing alone. To play with\n\
         others, open Characters and pick a server under Open Net."
    }
}

/// Returning user hub (after onboarding is complete).
fn draw_hub(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("hub_menu")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(360.0, 380.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("HumanityOS").size(theme.font_size_title).color(theme.accent()));
                ui.add_space(4.0);
                ui.label(RichText::new("End poverty. Unite humanity.").size(theme.font_size_body).color(theme.text_secondary()));
                ui.add_space(8.0);

                let status = if state.server_connected { "Online" } else { "Offline" };
                ui.label(RichText::new(status).size(12.0).color(theme.text_muted()));

                ui.add_space(24.0);

                if widgets::primary_button(ui, theme, "  Enter World  ") {
                    state.active_page = GuiPage::None;
                }
                ui.add_space(6.0);
                if widgets::secondary_button(ui, theme, "  Chat  ") {
                    state.active_page = GuiPage::Chat;
                }
                ui.add_space(6.0);
                if widgets::secondary_button(ui, theme, "  Settings  ") {
                    state.active_page = GuiPage::Settings;
                }
                ui.add_space(6.0);
                if widgets::danger_button(ui, theme, "  Quit  ") {
                    state.quit_requested = true;
                }

                ui.add_space(20.0);
                ui.label(RichText::new(format!("v{}", shown_version())).size(theme.font_size_small).color(theme.text_muted()));
            });
        });
}

/// The first session starts alone in the player's own home, and a relaunch keeps it there
/// (first-hour audit 2026-10-04, docs/design/first-hour-audit-2026-10-04.md: Blocker 1 and
/// Friction 10). A new player who typed a name and was online used to be put in the live
/// shared world a few seconds after Enter: chat connects to united-humanity.us by itself, and
/// with `copresence_solo` false (its default, never saved) the join gate joined its world,
/// on a 72x clock and possibly as a guest with no home. Only the WORLD join changes here;
/// chat still connects as it always has.
#[cfg(test)]
mod first_session_tests {
    use super::{finish_onboarding, skip_server_step, skip_setup};
    use crate::config::AppConfig;
    use crate::engine::home_plot::{join_step, JoinGate, JoinStep};
    use crate::gui::{GuiPage, GuiState, LauncherWhere};

    /// The join gate once the player stands in the loaded world, connected and identified,
    /// aboard: everything a join waits for except the player's own solo choice.
    fn in_world_gate(solo: bool) -> JoinGate {
        JoinGate { in_world: true, joined: false, identified: true, solo, aboard: true, refused_here: false, world_loaded: true, has_ship: true }
    }

    /// The app closed and opened again: the config written from `state`, read back as JSON,
    /// applied to a fresh state the way boot applies it (lib.rs, `apply_to_gui_state`).
    fn relaunch(state: &GuiState) -> GuiState {
        let json = serde_json::to_string(&AppConfig::from_gui_state(state)).expect("config serializes");
        let back: AppConfig = serde_json::from_str(&json).expect("config parses");
        let mut fresh = GuiState::default();
        back.apply_to_gui_state(&mut fresh);
        fresh
    }

    /// "Enter HumanityOS" after skipping the server step: into the world, alone, in the
    /// player's own home, and that home is the pairing Play repeats.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed: "a new player is joined to the shared world without
    /// being asked: copresence_solo is false".
    #[test]
    fn entering_from_onboarding_starts_alone_in_your_own_home() {
        let mut state = GuiState::default();
        assert!(!state.server_connected, "the setup: no server chosen on the server step");
        finish_onboarding(&mut state);
        assert!(state.onboarding_complete);
        assert_eq!(state.active_page, GuiPage::None, "Enter goes into the world");
        assert!(state.copresence_solo, "a new player is joined to the shared world without being asked: copresence_solo is false");
        assert_eq!(join_step(&in_world_gate(state.copresence_solo)), JoinStep::Wait, "the join gate joins the shared world");
        assert_eq!(state.launcher_last_world, "home:", "no pairing recorded, so Play opens the picker: {:?}", state.launcher_last_world);
        assert_eq!(state.launcher_where_kind, LauncherWhere::Home);
        // And it outlives the session.
        let back = relaunch(&state);
        assert!(back.copresence_solo, "after a relaunch the game joins the shared world");
    }

    /// The welcome step's "Skip setup (offline mode)" goes into the world with no identity;
    /// one made later in Settings must not quietly join the shared world either.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed: "skipping setup leaves the shared world one identity
    /// away: copresence_solo is false".
    #[test]
    fn skipping_setup_starts_alone_in_your_own_home() {
        let mut state = GuiState::default();
        skip_setup(&mut state);
        assert!(state.onboarding_complete);
        assert_eq!(state.active_page, GuiPage::None);
        assert!(state.copresence_solo, "skipping setup leaves the shared world one identity away: copresence_solo is false");
        assert_eq!(state.launcher_last_world, "home:", "{:?}", state.launcher_last_world);
    }

    /// "Skip (stay offline)" after a successful Connect: the player chose not to use the
    /// server, so Enter must not treat the earlier reachability check as a choice of its world.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed: "Skip then Enter lands on Chat, not in the world".
    #[test]
    fn skip_stay_offline_after_connecting_still_starts_alone() {
        let mut state = GuiState::default();
        state.onboarding_step = 1;
        state.server_connected = true; // Connect reached the server
        skip_server_step(&mut state);
        assert_eq!(state.onboarding_step, 2, "Skip moves on to the identity step");
        finish_onboarding(&mut state);
        assert_eq!(state.active_page, GuiPage::None, "Skip then Enter lands on {:?}, not in the world", state.active_page);
        assert!(state.copresence_solo, "Skip (stay offline) after a Connect joins the shared world: copresence_solo is false");
        assert_eq!(state.launcher_last_world, "home:");
    }

    /// The one exception: the player pressed Connect, reached the server and pressed
    /// Continue. They chose that server, so its shared world stays theirs (and Enter opens
    /// Chat, as before); no home pairing is recorded over that choice. Green on 8e400d7ed
    /// too: it guards the exception, so the fix cannot overreach. Broken on purpose 2026-10-04
    /// with the home start applied on this path as well: "a player who chose a server is kept
    /// out of its world".
    #[test]
    fn connecting_and_continuing_keeps_the_servers_shared_world() {
        let mut state = GuiState::default();
        state.server_connected = true;
        finish_onboarding(&mut state);
        assert_eq!(state.active_page, GuiPage::Chat);
        assert!(!state.copresence_solo, "a player who chose a server is kept out of its world");
        assert_eq!(join_step(&in_world_gate(state.copresence_solo)), JoinStep::Join);
        assert!(state.launcher_last_world.is_empty(), "a home pairing was recorded over the server choice: {:?}", state.launcher_last_world);
    }

    /// RELAUNCH (Friction 10): the player's last pairing was their own home. After a restart
    /// Esc (or the hub's Enter World) drops into the world: the join gate must keep out of the
    /// shared world, and Play must go straight home, never through the picker or onto the server.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed (boot never reading the solo start back from the
    /// pairing): "after a relaunch, Esc into the world joins the shared world: copresence_solo
    /// is false".
    #[test]
    fn a_relaunch_keeps_a_home_player_out_of_the_shared_world() {
        let mut state = GuiState::default();
        state.onboarding_complete = true;
        state.launcher_where_kind = LauncherWhere::Home;
        state.copresence_solo = true;
        state.record_pairing();
        let mut back = relaunch(&state);
        assert_eq!(back.launcher_last_world, "home:", "the pairing is in the config");
        assert!(back.copresence_solo, "after a relaunch, Esc into the world joins the shared world: copresence_solo is false");
        assert_eq!(join_step(&in_world_gate(back.copresence_solo)), JoinStep::Wait);
        // Play: the recorded pairing, not the picker, and still alone.
        assert!(back.apply_last_pairing(), "Play opens the picker");
        assert!(back.copresence_solo, "Play joins the shared world");
        assert_eq!(back.launcher_where_kind, LauncherWhere::Home);
    }

    /// The other pairings keep what they always did: a server pairing is that server's shared
    /// world, and no pairing at all (a config from before pairings, a rig's fresh folder) keeps
    /// the shared default the copresence rigs rely on. Green on 8e400d7ed too: a guard, so
    /// the solo start read back at boot stays confined to home pairings. Broken on purpose
    /// 2026-10-04 with solo read back for anything but a server pairing: "no pairing comes back
    /// solo, which would keep every rig out of the shared world".
    #[test]
    fn a_relaunch_after_a_server_or_no_pairing_keeps_the_shared_world() {
        let mut state = GuiState::default();
        state.launcher_where_kind = LauncherWhere::Server;
        state.launcher_selected_server = Some("__connected__".to_string());
        state.record_pairing();
        let back = relaunch(&state);
        assert_eq!(back.launcher_last_world, "server:__connected__");
        assert!(!back.copresence_solo, "a server pairing comes back solo");
        let back = relaunch(&GuiState::default());
        assert!(back.launcher_last_world.is_empty());
        assert!(!back.copresence_solo, "no pairing comes back solo, which would keep every rig out of the shared world");
    }
}

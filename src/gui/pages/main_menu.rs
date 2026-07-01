//! Main menu / onboarding screen.
//!
//! First-run: walks user through welcome, server connection, identity setup.
//! Returning user: shows the main hub with quick-access buttons.

use egui::{Align2, Color32, RichText, Vec2};
use crate::gui::{GuiPage, GuiState, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Full-screen dark backdrop — derived from theme.bg_primary with 94% alpha
    // so the 3D world (if rendered behind) shows through faintly.
    let bg = theme.bg_primary();
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 240));

    if !state.onboarding_complete {
        draw_onboarding(ctx, theme, state);
    } else {
        draw_hub(ctx, theme, state);
    }
}

/// First-run onboarding flow.
fn draw_onboarding(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("onboarding")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(500.0, 520.0))
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
             No accounts, no passwords, no tracking. You own your data."
        ).size(13.0).color(theme.text_muted()));

        ui.add_space(30.0);

        if widgets::primary_button(ui, theme, "   Get Started   ") {
            state.onboarding_step = 1;
        }
        ui.add_space(8.0);
        if ui.small_button("Skip setup (offline mode)").clicked() {
            state.onboarding_complete = true;
            state.active_page = GuiPage::None;
            crate::config::AppConfig::from_gui_state(state).save();
        }

        ui.add_space(16.0);
        ui.label(RichText::new(format!("v{}", VERSION)).size(11.0).color(theme.text_muted()));
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
                .hint_text("https://united-humanity.us"),
        );
        if response.changed() {
            // The URL changed -- any in-flight or previous check result is
            // for a different address now, so drop it rather than apply a
            // stale outcome to the newly-typed URL.
            state.server_connected = false;
            state.server_check_error.clear();
            state.server_check_rx = None;
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(
            "Default: united-humanity.us (the official community server)"
        ).size(11.0).color(theme.text_muted()));
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
                let health_url = derive_health_url(&state.server_url);
                std::thread::spawn(move || {
                    let result = ureq::get(&health_url)
                        .call()
                        .map(|_| ())
                        .map_err(|e| format!("Could not reach {health_url}: {e}"));
                    let _ = tx.send(result);
                });
            }
        } else {
            if widgets::primary_button(ui, theme, "  Continue  ") {
                state.onboarding_step = 2;
            }
        }
        ui.add_space(8.0);
        if ui.small_button("Skip (stay offline)").clicked() {
            state.onboarding_step = 2;
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

/// Step 2: Identity / display name
fn draw_step_identity(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(RichText::new("Your Identity").size(24.0).color(theme.accent()));
        ui.add_space(8.0);
        ui.label(RichText::new(
            "Choose a display name. Your post-quantum cryptographic\n\
             identity (Dilithium3 key) is generated automatically."
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
                state.settings.seed_phrase_recovery_status =
                    "Identity created. Back up your 24-word seed phrase in Settings → Identity now, it is the ONLY way to restore this account.".to_string();
                state.passphrase_needed = true;
                state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("Creates a fresh 24-word seed (your only backup). Or recover an existing one below.").size(11.0).color(theme.text_secondary()));
        });
        ui.add_space(8.0);
    }

    // ── Recover from Seed Phrase ──
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        if ui.small_button(
            if state.settings.seed_phrase_show_recover { "Cancel recovery" } else { "Recover existing identity from seed phrase" }
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
                    .hint_text("Enter your 24-word seed phrase"));
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
                state.onboarding_step = 3;
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

        ui.add_space(40.0);

        if widgets::primary_button(ui, theme, "  Enter HumanityOS  ") {
            state.onboarding_complete = true;
            // Default to chat page (connected) or game (offline)
            if state.server_connected {
                state.active_page = GuiPage::Chat;
            } else {
                state.active_page = GuiPage::None; // Enter the 3D world
            }
            crate::config::AppConfig::from_gui_state(state).save();
        }

        ui.add_space(30.0);
        ui.label(RichText::new(
            "Press Escape anytime to open the menu.\n\
             Press Enter to toggle chat."
        ).size(12.0).color(theme.text_muted()));
    });
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
                ui.label(RichText::new(format!("v{}", VERSION)).size(theme.font_size_small).color(theme.text_muted()));
            });
        });
}

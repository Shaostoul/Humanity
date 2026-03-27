//! Main menu / onboarding screen.
//!
//! First-run: walks user through welcome, server connection, identity setup.
//! Returning user: shows the main hub with quick-access buttons.

use egui::{Align2, Color32, RichText, Vec2};
use crate::gui::{GuiPage, GuiState, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Full-screen dark backdrop
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(10, 10, 14, 240));

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
        .frame(egui::Frame::window(&ctx.style()).fill(Color32::from_rgb(18, 18, 24)))
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
            "Your identity is a cryptographic key. No accounts,\n\
             no passwords, no tracking. You own your data."
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

/// Step 1: Server connection
fn draw_step_server(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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
            state.server_connected = false;
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
            ui.label(RichText::new("Connected!").size(14.0).color(Color32::from_rgb(46, 204, 113)));
        });
    }

    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        if !state.server_connected {
            if widgets::primary_button(ui, theme, "  Connect  ") {
                // TODO: actually connect via WebSocket
                state.server_connected = true;
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

/// Step 2: Identity / display name
fn draw_step_identity(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(RichText::new("Your Identity").size(24.0).color(theme.accent()));
        ui.add_space(8.0);
        ui.label(RichText::new(
            "Choose a display name. Your cryptographic identity\n\
             (Ed25519 key) is generated automatically."
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

    // Context mode selection
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new("Default Mode:").size(14.0).color(theme.text_primary()));
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.radio_value(&mut state.context_real, true, "Real (life tools, real notifications)");
    });
    ui.horizontal(|ui| {
        ui.add_space(40.0);
        ui.radio_value(&mut state.context_real, false, "Sim (game mode, simulation notifications)");
    });

    ui.add_space(12.0);

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
                        Ok((pubkey_hex, privkey_bytes)) => {
                            state.settings.seed_phrase_recovery_status = format!(
                                "Recovered: {}...{}", &pubkey_hex[..8], &pubkey_hex[pubkey_hex.len()-8..]
                            );
                            state.profile_public_key = pubkey_hex;
                            state.private_key_bytes = Some(privkey_bytes);
                            state.identity_recovered = true;
                            state.settings.seed_phrase_input.clear();
                            state.settings.seed_phrase_show_recover = false;
                            // Prompt for passphrase to encrypt the recovered key
                            state.passphrase_needed = true;
                            state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
                        }
                        Err(e) => {
                            state.settings.seed_phrase_recovery_status = format!("Error: {}", e);
                        }
                    }
                }
                if !state.settings.seed_phrase_recovery_status.is_empty() {
                    let color = if state.settings.seed_phrase_recovery_status.starts_with("Error") {
                        Color32::from_rgb(231, 76, 60)
                    } else {
                        Color32::from_rgb(46, 204, 113)
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
            ui.label(RichText::new(display).size(11.0).color(Color32::from_rgb(46, 204, 113)));
        });
    }

    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        if widgets::primary_button(ui, theme, "  Finish Setup  ") {
            state.onboarding_step = 3;
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

        let mode = if state.context_real { "Real" } else { "Sim" };
        let server_status = if state.server_connected {
            format!("Connected to {}", state.server_url)
        } else {
            "Offline mode".to_string()
        };
        ui.label(RichText::new(format!("Mode: {} | {}", mode, server_status)).size(13.0).color(theme.text_secondary()));

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
        .frame(egui::Frame::window(&ctx.style()).fill(Color32::from_rgb(18, 18, 24)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("HumanityOS").size(theme.font_size_title).color(theme.accent()));
                ui.add_space(4.0);
                ui.label(RichText::new("End poverty. Unite humanity.").size(theme.font_size_body).color(theme.text_secondary()));
                ui.add_space(8.0);

                let mode = if state.context_real { "Real" } else { "Sim" };
                let status = if state.server_connected { "Online" } else { "Offline" };
                ui.label(RichText::new(format!("{} | {}", mode, status)).size(12.0).color(theme.text_muted()));

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

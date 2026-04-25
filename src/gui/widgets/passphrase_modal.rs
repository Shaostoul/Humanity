//! Passphrase entry modal overlay.
//!
//! Shows when:
//! - Migration from plaintext key (SetNew): user must choose a passphrase
//! - Startup with encrypted key (Unlock): user must enter passphrase
//! - Changing passphrase (Change): user enters old + new passphrase

use egui::{Align2, Color32, Frame, RichText, Rounding, Stroke, Vec2};
use crate::gui::{GuiState, PassphraseMode};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Semi-transparent backdrop
    egui::Area::new(egui::Id::new("passphrase_backdrop"))
        .fixed_pos([0.0, 0.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        });

    egui::Window::new("Passphrase Required")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(400.0, 0.0))
        .frame(Frame::window(&ctx.style())
            .fill(Color32::from_rgb(28, 28, 36))
            .rounding(Rounding::same(8))
            .stroke(Stroke::new(1.0, theme.accent()))
            .inner_margin(20.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            match state.passphrase_mode {
                PassphraseMode::SetNew => draw_set_new(ui, theme, state),
                PassphraseMode::Unlock => draw_unlock(ui, theme, state),
                PassphraseMode::Change => draw_change(ui, theme, state),
            }
        });
}

fn draw_set_new(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Encrypt Your Private Key")
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        "Choose a passphrase to encrypt your private key. \
         This passphrase will be required each time you start the app. \
         Your key will never be stored in plaintext again.")
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new("Passphrase:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.passphrase_input)
        .password(true)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Confirm Passphrase:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.passphrase_confirm)
        .password(true)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_md);

    if !state.passphrase_status.is_empty() {
        ui.label(RichText::new(&state.passphrase_status)
            .color(theme.danger())
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "Encrypt Key") {
            if state.passphrase_input.is_empty() {
                state.passphrase_status = "Passphrase cannot be empty.".to_string();
            } else if state.passphrase_input != state.passphrase_confirm {
                state.passphrase_status = "Passphrases do not match.".to_string();
            } else if let Some(ref key_bytes) = state.private_key_bytes.clone() {
                match crate::config::encrypt_private_key(key_bytes, &state.passphrase_input) {
                    Ok((encrypted, salt)) => {
                        state.encrypted_private_key = encrypted;
                        state.key_salt = salt;
                        state.passphrase_needed = false;
                        state.passphrase_input.clear();
                        state.passphrase_confirm.clear();
                        state.passphrase_status.clear();
                        // Save config (now with encrypted key, no plaintext)
                        crate::config::AppConfig::from_gui_state(state).save();
                        log::info!("Private key encrypted and saved successfully");
                    }
                    Err(e) => {
                        state.passphrase_status = format!("Encryption failed: {}", e);
                    }
                }
            } else {
                state.passphrase_status = "No private key available to encrypt.".to_string();
            }
        }

        if widgets::secondary_button(ui, theme, "Skip (key unavailable)") {
            // User skips: key stays in memory but is not persisted encrypted
            state.passphrase_needed = false;
            state.passphrase_input.clear();
            state.passphrase_confirm.clear();
            state.passphrase_status.clear();
        }
    });
}

fn draw_unlock(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Unlock Private Key")
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        "Enter your passphrase to decrypt your private key. \
         Without it, signing and wallet features will be unavailable.")
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new("Passphrase:").color(theme.text_secondary()));
    let response = ui.add(egui::TextEdit::singleline(&mut state.passphrase_input)
        .password(true)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_md);

    if !state.passphrase_status.is_empty() {
        ui.label(RichText::new(&state.passphrase_status)
            .color(theme.danger())
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "Unlock") || enter_pressed {
            match crate::config::decrypt_private_key(
                &state.encrypted_private_key,
                &state.key_salt,
                &state.passphrase_input,
            ) {
                Ok(key_bytes) => {
                    state.private_key_bytes = Some(key_bytes);
                    state.passphrase_needed = false;
                    state.passphrase_input.clear();
                    state.passphrase_status.clear();
                    log::info!("Private key unlocked successfully");
                }
                Err(e) => {
                    state.passphrase_status = format!("Wrong passphrase: {}", e);
                }
            }
        }

        if widgets::secondary_button(ui, theme, "Skip (limited mode)") {
            // User skips: no private key available, chat-only mode
            state.passphrase_needed = false;
            state.passphrase_input.clear();
            state.passphrase_status.clear();
            log::info!("User skipped passphrase; private key unavailable");
        }
    });
}

fn draw_change(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Change Passphrase")
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new("Current Passphrase:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.passphrase_old_input)
        .password(true)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("New Passphrase:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.passphrase_input)
        .password(true)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Confirm New Passphrase:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.passphrase_confirm)
        .password(true)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_md);

    if !state.passphrase_status.is_empty() {
        let color = if state.passphrase_status.starts_with("Passphrase changed") {
            Color32::from_rgb(46, 204, 113)
        } else {
            theme.danger()
        };
        ui.label(RichText::new(&state.passphrase_status)
            .color(color)
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "Change Passphrase") {
            if state.passphrase_input.is_empty() {
                state.passphrase_status = "New passphrase cannot be empty.".to_string();
            } else if state.passphrase_input != state.passphrase_confirm {
                state.passphrase_status = "New passphrases do not match.".to_string();
            } else {
                // First decrypt with old passphrase
                match crate::config::decrypt_private_key(
                    &state.encrypted_private_key,
                    &state.key_salt,
                    &state.passphrase_old_input,
                ) {
                    Ok(key_bytes) => {
                        // Re-encrypt with new passphrase
                        match crate::config::encrypt_private_key(&key_bytes, &state.passphrase_input) {
                            Ok((encrypted, salt)) => {
                                state.encrypted_private_key = encrypted;
                                state.key_salt = salt;
                                state.private_key_bytes = Some(key_bytes);
                                state.passphrase_needed = false;
                                state.passphrase_old_input.clear();
                                state.passphrase_input.clear();
                                state.passphrase_confirm.clear();
                                state.passphrase_status = "Passphrase changed successfully!".to_string();
                                crate::config::AppConfig::from_gui_state(state).save();
                                log::info!("Passphrase changed successfully");
                            }
                            Err(e) => {
                                state.passphrase_status = format!("Re-encryption failed: {}", e);
                            }
                        }
                    }
                    Err(_) => {
                        state.passphrase_status = "Current passphrase is incorrect.".to_string();
                    }
                }
            }
        }

        if widgets::secondary_button(ui, theme, "Cancel") {
            state.passphrase_needed = false;
            state.passphrase_old_input.clear();
            state.passphrase_input.clear();
            state.passphrase_confirm.clear();
            state.passphrase_status.clear();
        }
    });
}

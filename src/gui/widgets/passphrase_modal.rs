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
                PassphraseMode::PinSetup => draw_pin_setup(ui, theme, state),
                PassphraseMode::PinUnlock => draw_pin_unlock(ui, theme, state),
                PassphraseMode::PinChange => draw_pin_change(ui, theme, state),
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
                    Ok((encrypted, salt, iterations)) => {
                        state.encrypted_private_key = encrypted;
                        state.key_salt = salt;
                        state.key_iterations = iterations;
                        state.passphrase_needed = false;
                        state.passphrase_input.clear();
                        state.passphrase_confirm.clear();
                        state.passphrase_status.clear();
                        // Save config (now with encrypted key, no plaintext)
                        crate::config::AppConfig::from_gui_state(state).save();
                        log::info!(
                            "Private key encrypted and saved successfully ({} PBKDF2 iters)",
                            iterations
                        );
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

    ui.add_space(theme.spacing_sm);

    // v0.278.0: auto-unlock opt-in. Default off — user has to consciously
    // tick this to stash the seed in the OS keychain. Honors the rule
    // that auto-unlock is always opt-in, never magic.
    ui.checkbox(
        &mut state.remember_on_device,
        RichText::new("Remember on this device (OS keychain)")
            .color(theme.text_secondary())
            .size(theme.font_size_small),
    );
    ui.label(RichText::new(
        "Skips the passphrase prompt next time. Encrypted by your OS account; \
         clear it from Settings → Security.")
        .size(theme.font_size_small)
        .color(theme.text_muted()));

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
                state.key_iterations,
            ) {
                Ok(key_bytes) => {
                    // Silent migration: if the vault was written with the
                    // legacy 100_000-iter count, re-encrypt at the new
                    // 600_000 count using the same passphrase the user
                    // just proved they know. Persist immediately so the
                    // next unlock pays the new (higher) cost — exactly
                    // once per vault. Failures here are non-fatal: the
                    // unlock itself already succeeded, the user is in;
                    // we log and move on. Worst case: we retry the
                    // migration on the next unlock.
                    if state.key_iterations < crate::config::PBKDF2_ITERATIONS_NEW {
                        match crate::config::encrypt_private_key(
                            &key_bytes,
                            &state.passphrase_input,
                        ) {
                            Ok((new_encrypted, new_salt, new_iters)) => {
                                let old_iters = state.key_iterations;
                                state.encrypted_private_key = new_encrypted;
                                state.key_salt = new_salt;
                                state.key_iterations = new_iters;
                                crate::config::AppConfig::from_gui_state(state).save();
                                log::info!(
                                    "Vault PBKDF2 silently upgraded: {} -> {} iters",
                                    old_iters, new_iters,
                                );
                            }
                            Err(e) => {
                                log::warn!(
                                    "Vault PBKDF2 upgrade failed (re-encrypt): {}. \
                                     Continuing on legacy iter count; will retry next unlock.",
                                    e
                                );
                            }
                        }
                    }

                    // v0.278.0: if the user ticked "Remember on this device",
                    // stash the freshly-unlocked seed in the OS keychain BEFORE
                    // we move it into state. Best-effort: a keychain failure
                    // just logs and falls back to AlwaysPrompt — the unlock
                    // itself already succeeded, the user is in. Worst case:
                    // they'll be re-prompted next launch and can re-tick the
                    // checkbox.
                    if state.remember_on_device {
                        if key_bytes.len() != 32 {
                            log::warn!("Auto-unlock stash skipped: seed is {} bytes, expected 32", key_bytes.len());
                        } else {
                            let mut seed_arr = [0u8; 32];
                            seed_arr.copy_from_slice(&key_bytes);
                            // Identity = the Dilithium hex once apply_pq_identity
                            // has run — but that hasn't happened yet here, so
                            // use the Ed25519 public key hex (`profile_public_key`)
                            // as the keychain account. The startup load path
                            // uses the SAME field (`public_key_hex` in config),
                            // so the lookup matches.
                            let identity = state.profile_public_key.clone();
                            if !identity.is_empty() {
                                match crate::auto_unlock::keychain_stash(
                                    crate::auto_unlock::KeychainSlot::Seed,
                                    &identity,
                                    &seed_arr,
                                ) {
                                    Ok(()) => {
                                        state.auto_unlock_mode = crate::auto_unlock::AutoUnlockMode::Keychain;
                                        log::info!("Auto-unlock: seed stashed in OS keychain; mode -> Keychain");
                                    }
                                    Err(e) => {
                                        log::warn!("Auto-unlock stash FAILED ({}). Mode stays AlwaysPrompt.", e);
                                    }
                                }
                            } else {
                                log::warn!("Auto-unlock stash skipped: no profile_public_key in state");
                            }
                        }
                        state.remember_on_device = false; // one-shot per modal
                    }

                    state.private_key_bytes = Some(key_bytes);
                    state.passphrase_needed = false;
                    state.passphrase_input.clear();
                    state.passphrase_status.clear();
                    log::info!("Private key unlocked successfully");
                    // Full-PQ: derive Dilithium+Kyber from the now-unlocked
                    // seed and reconnect so we advertise kyber_public —
                    // otherwise DMs are impossible (no_own_key / no peer key).
                    state.apply_pq_identity();
                    // Persist (auto_unlock_mode + any silent re-encrypt above)
                    crate::config::AppConfig::from_gui_state(state).save();
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
                // First decrypt with old passphrase + the vault's stored
                // iter count (legacy 100k or new 600k — both work)
                match crate::config::decrypt_private_key(
                    &state.encrypted_private_key,
                    &state.key_salt,
                    &state.passphrase_old_input,
                    state.key_iterations,
                ) {
                    Ok(key_bytes) => {
                        // Re-encrypt with new passphrase. Always lands at
                        // the new (600k) iter count: change-passphrase IS
                        // a re-encrypt, so it's a natural migration point.
                        match crate::config::encrypt_private_key(&key_bytes, &state.passphrase_input) {
                            Ok((encrypted, salt, iterations)) => {
                                state.encrypted_private_key = encrypted;
                                state.key_salt = salt;
                                state.key_iterations = iterations;
                                state.private_key_bytes = Some(key_bytes);
                                state.passphrase_needed = false;
                                state.passphrase_old_input.clear();
                                state.passphrase_input.clear();
                                state.passphrase_confirm.clear();
                                state.passphrase_status = "Passphrase changed successfully!".to_string();
                                crate::config::AppConfig::from_gui_state(state).save();
                                log::info!(
                                    "Passphrase changed successfully ({} PBKDF2 iters)",
                                    iterations
                                );
                                state.apply_pq_identity();
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

// ── v0.278.0 PIN flows ──────────────────────────────────────────────────
//
// All three PIN flows share the same shape as their passphrase siblings:
// PinSetup mirrors SetNew, PinUnlock mirrors Unlock, PinChange mirrors
// Change. The differences are (a) digit-only input + length cap, (b) the
// keychain-stored device_key as the second half of the AES-GCM key
// derivation, (c) the user's PIN-encrypted seed lives in
// AppConfig.pin_encrypted_seed / pin_salt (the passphrase vault is left
// intact — they coexist as alternate unlock paths).

/// Common helper: load the device_key from the OS keychain for the
/// current identity. Returns None when the entry is genuinely absent
/// (caller's UI surface should display "PIN setup gone — re-run setup
/// or use passphrase"); Err on platform failure (logged + None).
fn load_device_key(state: &GuiState) -> Option<[u8; 32]> {
    if state.profile_public_key.is_empty() {
        log::warn!("PIN flow: no profile_public_key in state");
        return None;
    }
    match crate::auto_unlock::keychain_load(
        crate::auto_unlock::KeychainSlot::DeviceKey,
        &state.profile_public_key,
    ) {
        Ok(opt) => opt,
        Err(e) => {
            log::warn!("PIN flow: keychain load failed: {}", e);
            None
        }
    }
}

fn draw_pin_setup(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Set a Quick PIN")
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        "Choose a 4-12 digit PIN for faster unlock on this device. \
         Your full passphrase remains your recovery option. \
         If you forget the PIN, you can always recover with the passphrase.")
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new("PIN (digits only):").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.pin_input)
        .password(true)
        .char_limit(crate::auto_unlock::PIN_MAX_LEN)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Confirm PIN:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.pin_confirm)
        .password(true)
        .char_limit(crate::auto_unlock::PIN_MAX_LEN)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_md);

    if !state.pin_status.is_empty() {
        ui.label(RichText::new(&state.pin_status)
            .color(theme.danger())
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "Set PIN") {
            // Validation
            if let Err(msg) = crate::auto_unlock::validate_pin(&state.pin_input) {
                state.pin_status = msg;
                return;
            }
            if state.pin_input != state.pin_confirm {
                state.pin_status = "PINs do not match.".to_string();
                return;
            }
            // Must already have the seed in memory (set up post-unlock
            // or right after fresh identity generation).
            let seed_bytes = match &state.private_key_bytes {
                Some(b) if b.len() == 32 => {
                    let mut s = [0u8; 32];
                    s.copy_from_slice(b);
                    s
                }
                _ => {
                    state.pin_status = "Unlock with your passphrase first, then set a PIN.".to_string();
                    return;
                }
            };
            if state.profile_public_key.is_empty() {
                state.pin_status = "No identity loaded — generate or recover first.".to_string();
                return;
            }

            // Generate fresh device_key, stash to keychain.
            let device_key = match crate::auto_unlock::generate_device_key() {
                Ok(k) => k,
                Err(e) => { state.pin_status = format!("RNG failed: {}", e); return; }
            };
            if let Err(e) = crate::auto_unlock::keychain_stash(
                crate::auto_unlock::KeychainSlot::DeviceKey,
                &state.profile_public_key,
                &device_key,
            ) {
                state.pin_status = format!("Keychain unavailable: {}", e);
                return;
            }

            // Encrypt seed with PIN + device_key.
            match crate::auto_unlock::encrypt_seed_with_pin(&seed_bytes, &state.pin_input, &device_key) {
                Ok((enc, salt)) => {
                    state.pin_encrypted_seed = enc;
                    state.pin_salt = salt;
                    state.auto_unlock_mode = crate::auto_unlock::AutoUnlockMode::KeychainPin;
                    state.passphrase_needed = false;
                    state.pin_input.clear();
                    state.pin_confirm.clear();
                    state.pin_status.clear();
                    crate::config::AppConfig::from_gui_state(state).save();
                    log::info!("PIN set; auto-unlock mode -> KeychainPin");
                }
                Err(e) => {
                    state.pin_status = format!("PIN encrypt failed: {}", e);
                }
            }
        }

        if widgets::secondary_button(ui, theme, "Cancel") {
            state.passphrase_needed = false;
            state.pin_input.clear();
            state.pin_confirm.clear();
            state.pin_status.clear();
        }
    });
}

fn draw_pin_unlock(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Enter PIN")
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        "Type your PIN to unlock. If you've forgotten it, switch to \
         'Use passphrase instead' and unlock with your 24-word seed phrase.")
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new("PIN:").color(theme.text_secondary()));
    let response = ui.add(egui::TextEdit::singleline(&mut state.pin_input)
        .password(true)
        .char_limit(crate::auto_unlock::PIN_MAX_LEN)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_md);

    if !state.pin_status.is_empty() {
        ui.label(RichText::new(&state.pin_status)
            .color(theme.danger())
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "Unlock") || enter_pressed {
            let device_key = match load_device_key(state) {
                Some(k) => k,
                None => {
                    state.pin_status = "PIN setup gone (keychain cleared). Use passphrase to re-unlock.".to_string();
                    return;
                }
            };
            match crate::auto_unlock::decrypt_seed_with_pin(
                &state.pin_encrypted_seed,
                &state.pin_salt,
                &state.pin_input,
                &device_key,
            ) {
                Ok(seed) => {
                    state.private_key_bytes = Some(seed.to_vec());
                    state.passphrase_needed = false;
                    state.pin_input.clear();
                    state.pin_status.clear();
                    state.apply_pq_identity();
                    log::info!("Unlocked via PIN");
                }
                Err(_) => {
                    // Deliberately generic — see decrypt_seed_with_pin
                    // docstring on why we don't distinguish bad-PIN from
                    // corrupted-blob.
                    state.pin_status = "Wrong PIN.".to_string();
                }
            }
        }

        if widgets::secondary_button(ui, theme, "Use passphrase instead") {
            state.passphrase_mode = PassphraseMode::Unlock;
            state.pin_input.clear();
            state.pin_status.clear();
        }
    });
}

fn draw_pin_change(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Change PIN")
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new("Current PIN:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.pin_old_input)
        .password(true)
        .char_limit(crate::auto_unlock::PIN_MAX_LEN)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("New PIN:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.pin_input)
        .password(true)
        .char_limit(crate::auto_unlock::PIN_MAX_LEN)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Confirm New PIN:").color(theme.text_secondary()));
    ui.add(egui::TextEdit::singleline(&mut state.pin_confirm)
        .password(true)
        .char_limit(crate::auto_unlock::PIN_MAX_LEN)
        .desired_width(ui.available_width()));

    ui.add_space(theme.spacing_md);

    if !state.pin_status.is_empty() {
        let ok = state.pin_status.starts_with("PIN changed");
        let color = if ok { egui::Color32::from_rgb(46, 204, 113) } else { theme.danger() };
        ui.label(RichText::new(&state.pin_status)
            .color(color)
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, "Change PIN") {
            if let Err(msg) = crate::auto_unlock::validate_pin(&state.pin_input) {
                state.pin_status = msg;
                return;
            }
            if state.pin_input != state.pin_confirm {
                state.pin_status = "New PINs do not match.".to_string();
                return;
            }
            let device_key = match load_device_key(state) {
                Some(k) => k,
                None => {
                    state.pin_status = "PIN setup gone — set up a new PIN from Settings.".to_string();
                    return;
                }
            };
            // Verify old PIN by decrypting existing blob.
            let seed = match crate::auto_unlock::decrypt_seed_with_pin(
                &state.pin_encrypted_seed, &state.pin_salt,
                &state.pin_old_input, &device_key,
            ) {
                Ok(s) => s,
                Err(_) => { state.pin_status = "Current PIN is incorrect.".to_string(); return; }
            };
            // Re-encrypt with new PIN (same device_key — no churn in keychain).
            match crate::auto_unlock::encrypt_seed_with_pin(&seed, &state.pin_input, &device_key) {
                Ok((enc, salt)) => {
                    state.pin_encrypted_seed = enc;
                    state.pin_salt = salt;
                    state.pin_old_input.clear();
                    state.pin_input.clear();
                    state.pin_confirm.clear();
                    state.pin_status = "PIN changed successfully.".to_string();
                    crate::config::AppConfig::from_gui_state(state).save();
                    log::info!("PIN changed");
                }
                Err(e) => {
                    state.pin_status = format!("Re-encrypt failed: {}", e);
                }
            }
        }

        if widgets::secondary_button(ui, theme, "Cancel") {
            state.passphrase_needed = false;
            state.pin_old_input.clear();
            state.pin_input.clear();
            state.pin_confirm.clear();
            state.pin_status.clear();
        }
    });
}

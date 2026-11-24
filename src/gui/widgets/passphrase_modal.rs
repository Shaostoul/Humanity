//! Passphrase entry modal overlay.
//!
//! Shows when:
//! - Migration from plaintext key (SetNew): user must choose a passphrase
//! - Startup with encrypted key (Unlock): user must enter passphrase
//! - Changing passphrase (Change): user enters old + new passphrase

use egui::{Align2, Frame, RichText, CornerRadius, Stroke};
use crate::gui::{GuiState, PassphraseMode};
use crate::gui::theme::Theme;
use crate::gui::layers;
use crate::gui::widgets;
use humanity_core::i18n::{t, tf};
use crate::gui::widgets::window_fit::Fitted as _; // every window fits a phone (2026-11-23)

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // A full-screen popup (gui/layers.rs). Unlocking, setting a first passphrase or PIN asks for a
    // decision: nothing but its own buttons ends it, so Back and Escape leave the page under it
    // where it is. Changing one may be put off: Back and Escape are its Cancel.
    let close = match state.passphrase_mode {
        PassphraseMode::Change | PassphraseMode::PinChange => layers::Close::Keys,
        _ => layers::Close::Decide,
    };
    // bg_modal IS the modal-scrim token (a black at ~70% alpha by default).
    let shade = layers::shade(ctx, egui::Id::new(PASSPHRASE), close, Theme::c32(&theme.bg_modal));
    if shade.wants_close() {
        state.passphrase_needed = false;
        return;
    }

    // 400 points, or the visible width less the margins on a phone (2026-11-22: 400 and the
    // 20-point margins ran past a 412-point phone's sides).
    let modal_w = (ctx.screen_rect().width() - 56.0).clamp(200.0, 400.0);
    let body_id = egui::Id::new(PASSPHRASE).with("body");
    let body_max_h = (ctx.screen_rect().height() - 56.0).max(120.0);
    let body_h = crate::gui::visible_area::body_height(ctx, body_id, body_max_h);
    // On a phone or any small screen the window's body is fitted and scrolls by itself
    // (widgets/window_fit.rs, 2026-11-23); its own scrolling body stays for a desktop.
    let fitted = crate::gui::widgets::window_fit::small(ctx);
    let window = layers::window(ctx, egui::Id::new(PASSPHRASE), t("passphrase.window_title"), layers::Layer::PopupWindow)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        // As tall as its contents, scrolling when that is more than a phone shows above its
        // keyboard (gui/visible_area.rs).
        .fixed_size(egui::vec2(modal_w, if fitted { 0.0 } else { body_h }))
        .frame(Frame::window(&ctx.style())
            .fill(theme.bg_card())
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0_f32, theme.accent()))
            .inner_margin(20.0))
        .show_fitted(ctx, |ui| {
            let mut forms = |ui: &mut egui::Ui| match state.passphrase_mode {
                PassphraseMode::SetNew => draw_set_new(ui, theme, state),
                PassphraseMode::Unlock => draw_unlock(ui, theme, state),
                PassphraseMode::Change => draw_change(ui, theme, state),
                PassphraseMode::PinSetup => draw_pin_setup(ui, theme, state),
                PassphraseMode::PinUnlock => draw_pin_unlock(ui, theme, state),
                PassphraseMode::PinChange => draw_pin_change(ui, theme, state),
            };
            if fitted {
                forms(ui);
            } else {
                crate::gui::visible_area::scrolling_body(ui, body_id, body_h, body_max_h, forms);
            }
        });
    if let Some(w) = window {
        layers::seat(ctx, &shade, w.response.layer_id);
    }
}

/// The passphrase popup and its window, by id (the window's title was its id before).
const PASSPHRASE: &str = "Passphrase Required";

fn draw_set_new(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("passphrase.set_new_title"))
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        t("passphrase.set_new_intro"))
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new(t("passphrase.passphrase_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.passphrase_input", &mut state.passphrase_input).show(ui, theme);

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("passphrase.confirm_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.passphrase_confirm", &mut state.passphrase_confirm).show(ui, theme);

    ui.add_space(theme.spacing_md);

    if !state.passphrase_status.is_empty() {
        ui.label(RichText::new(&state.passphrase_status)
            .color(theme.danger())
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, t("passphrase.encrypt_key")) {
            if state.passphrase_input.is_empty() {
                state.passphrase_status = t("passphrase.empty").to_string();
            } else if state.passphrase_input != state.passphrase_confirm {
                state.passphrase_status = t("passphrase.no_match").to_string();
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
                        state.passphrase_status = tf("passphrase.encrypt_failed", &[("error", &e)]);
                    }
                }
            } else {
                state.passphrase_status = t("passphrase.no_key").to_string();
            }
        }

        // The words say what a skip costs: the key stays in memory only, and the saved settings
        // never hold a key that is not encrypted (config.rs, from_gui_state), so a new identity
        // skipped here is gone when the app closes unless its words were written down.
        if widgets::secondary_button(ui, theme, humanity_core::i18n::t("passphrase.skip_not_kept")) {
            // User skips: key stays in memory but is not persisted encrypted
            state.passphrase_needed = false;
            state.passphrase_input.clear();
            state.passphrase_confirm.clear();
            state.passphrase_status.clear();
        }
    });
}

fn draw_unlock(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("passphrase.unlock_title"))
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        t("passphrase.unlock_intro"))
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new(t("passphrase.passphrase_label")).color(theme.text_secondary()));
    let response = widgets::secret_field::SecretField::new("passphrase_modal.passphrase_input", &mut state.passphrase_input).show(ui, theme);

    ui.add_space(theme.spacing_sm);

    // v0.278.0: auto-unlock opt-in. Default off — user has to consciously
    // tick this to stash the seed in the OS keychain. Honors the rule
    // that auto-unlock is always opt-in, never magic.
    ui.checkbox(
        &mut state.remember_on_device,
        RichText::new(t("passphrase.remember_label"))
            .color(theme.text_secondary())
            .size(theme.font_size_small),
    );
    ui.label(RichText::new(
        t("passphrase.remember_note"))
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

    // Drain a finished background unlock. The slow part (600k-iter PBKDF2 decrypt
    // + an optional legacy re-encrypt) runs on a worker thread so the UI never
    // freezes on the click; the cheap post-steps run here on the main thread.
    if state.passphrase_unlocking {
        let drained = state.passphrase_unlock_rx.as_ref().map(|rx| rx.try_recv());
        match drained {
            Some(Ok(outcome)) => {
                state.passphrase_unlock_rx = None;
                state.passphrase_unlocking = false;
                let (remember, now) = (state.remember_on_device, ui.ctx().input(|i| i.time));
                apply_unlock_outcome(state, outcome, remember, now);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                state.passphrase_unlock_rx = None;
                state.passphrase_unlocking = false;
                state.passphrase_status =
                    t("passphrase.unlock_worker_stopped").to_string();
            }
            _ => {
                // Still running — keep repainting so we keep draining the rx.
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
            }
        }
    }

    ui.horizontal(|ui| {
        if state.passphrase_unlocking {
            // Mid-unlock: spinner instead of the button, so the click can't
            // re-fire and the user sees the work is happening.
            ui.add(egui::Spinner::new());
            ui.label(RichText::new(t("passphrase.unlocking"))
                .color(theme.text_secondary())
                .size(theme.font_size_small));
        } else {
            if widgets::primary_button(ui, theme, t("passphrase.unlock")) || enter_pressed {
                // Kick the slow PBKDF2 (decrypt + the legacy 100k→600k
                // re-encrypt, which is ALSO a 600k PBKDF2) onto a worker thread.
                // The result is drained next frame by the block above.
                let encrypted = state.encrypted_private_key.clone();
                let salt = state.key_salt.clone();
                let pass = state.passphrase_input.clone();
                let iters = state.key_iterations;
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let result = crate::config::decrypt_private_key(&encrypted, &salt, &pass, iters)
                        .map(|key_bytes| {
                            let reenc = if iters < crate::config::PBKDF2_ITERATIONS_NEW {
                                // Re-encrypt off-thread too (it's the second heavy
                                // PBKDF2). None on failure → keep the legacy blob,
                                // retry on next unlock (matches the old behavior).
                                crate::config::encrypt_private_key(&key_bytes, &pass).ok()
                            } else {
                                None
                            };
                            (key_bytes, reenc)
                        });
                    let _ = tx.send(result);
                });
                state.passphrase_unlock_rx = Some(rx);
                state.passphrase_unlocking = true;
                state.passphrase_status.clear();
            }

            if widgets::secondary_button(ui, theme, t("passphrase.skip_limited")) {
                // User skips: no private key available, chat-only mode
                skip_unlock(state);
            }
        }
    });
}

/// Apply a finished background unlock to state (main thread). The slow PBKDF2
/// already ran in the worker; this does the cheap post-steps: persist a legacy-
/// vault re-encrypt, optionally stash the seed in the OS keychain, set the
/// unlocked seed, derive the PQ identity, and save. Mirrors the old inline
/// post-decrypt logic exactly — only WHERE the PBKDF2 runs changed. Settings, Account's
/// Reveal recovery phrase gate on a locked identity ends the same way (BUG-213).
///
/// `remember` is the dialog's own "Remember on this device" tick, passed by the dialog and
/// `false` from everywhere else, so an unlock that never showed the tick never remembers the seed
/// (the second review: a tick left from a skipped dialog stored the seed on a later Reveal).
/// `now` is the egui time, for the notice a failed remember shows.
pub(crate) fn apply_unlock_outcome(
    state: &mut GuiState,
    outcome: Result<(Vec<u8>, Option<(String, String, u32)>), String>,
    remember: bool,
    now: f64,
) {
    match outcome {
        Ok((key_bytes, reenc)) => {
            // Silent PBKDF2 upgrade (computed in the worker): persist the new
            // blob/salt/iters so the next unlock pays the higher count once.
            if let Some((new_encrypted, new_salt, new_iters)) = reenc {
                let old_iters = state.key_iterations;
                state.encrypted_private_key = new_encrypted;
                state.key_salt = new_salt;
                state.key_iterations = new_iters;
                log::info!("Vault PBKDF2 silently upgraded: {} -> {} iters", old_iters, new_iters);
            }

            // "Remember on this device": stash the freshly-unlocked seed in the key store BEFORE
            // moving it into state (remember_after_unlock). A failure is said and the mode stays
            // as it was (the unlock succeeded).
            if remember {
                remember_after_unlock(&mut humanity_core::auto_unlock::ThisDevice, state, &key_bytes, now);
            }
            state.remember_on_device = false; // one-shot per modal

            state.private_key_bytes = Some(key_bytes);
            state.passphrase_needed = false;
            state.passphrase_input.clear();
            state.passphrase_status.clear();
            log::info!("Private key unlocked successfully");
            // Full-PQ: derive Dilithium+Kyber from the now-unlocked seed +
            // reconnect so we send our key card (else DMs are impossible).
            state.apply_pq_identity();
            // Persist (auto_unlock_mode + any silent re-encrypt above).
            crate::config::AppConfig::from_gui_state(state).save();
        }
        Err(e) => {
            state.passphrase_status = tf("passphrase.wrong", &[("error", &e)]);
        }
    }
}

fn draw_change(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("passphrase.change_title"))
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new(t("passphrase.current_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.passphrase_old_input", &mut state.passphrase_old_input).show(ui, theme);

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("passphrase.new_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.passphrase_input", &mut state.passphrase_input).show(ui, theme);

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("passphrase.confirm_new_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.passphrase_confirm", &mut state.passphrase_confirm).show(ui, theme);

    ui.add_space(theme.spacing_md);

    if !state.passphrase_status.is_empty() {
        let color = if state.passphrase_status == t("passphrase.changed") {
            theme.success()
        } else {
            theme.danger()
        };
        ui.label(RichText::new(&state.passphrase_status)
            .color(color)
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, t("passphrase.change_title")) {
            if state.passphrase_input.is_empty() {
                state.passphrase_status = t("passphrase.new_empty").to_string();
            } else if state.passphrase_input != state.passphrase_confirm {
                state.passphrase_status = t("passphrase.new_no_match").to_string();
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
                                state.passphrase_status = t("passphrase.changed").to_string();
                                crate::config::AppConfig::from_gui_state(state).save();
                                log::info!(
                                    "Passphrase changed successfully ({} PBKDF2 iters)",
                                    iterations
                                );
                                state.apply_pq_identity();
                            }
                            Err(e) => {
                                state.passphrase_status = tf("passphrase.reencrypt_failed", &[("error", &e)]);
                            }
                        }
                    }
                    Err(_) => {
                        state.passphrase_status = t("passphrase.current_wrong").to_string();
                    }
                }
            }
        }

        if widgets::secondary_button(ui, theme, t("passphrase.cancel")) {
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

/// Switch Unlock on App Launch to `mode`: the one way every switch goes, from Settings and from
/// the unlock and PIN dialogs, on a desktop and on a phone (BUG-335: Remember on this device, then
/// Quick PIN, left the seed remembered). The key store keeps `secret` for the new mode and
/// forgets every other mode's (`auto_unlock::remember`); the PIN blob is `pin_blob` for the Quick
/// PIN and cleared otherwise; the config is saved. When the new secret could not be kept nothing
/// changes; when it was kept but another mode's could not be forgotten the switch still happens
/// (the new mode works) and the screen says so. `Err` is the words' key for the screen, also kept
/// in `state.unlock_alert` for Settings (cleared by a switch that fully happens); the details go
/// to the log only (a phone's can hold a Java stack trace).
pub(crate) fn set_unlock_mode(
    state: &mut GuiState,
    mode: humanity_core::auto_unlock::AutoUnlockMode,
    secret: Option<&[u8; 32]>,
    pin_blob: Option<(String, String)>,
) -> Result<(), &'static str> {
    set_unlock_mode_in(&mut humanity_core::auto_unlock::ThisDevice, state, mode, secret, pin_blob)
}

/// [`set_unlock_mode`] with the key store named, so the tests can stand one in.
pub(crate) fn set_unlock_mode_in(
    store: &mut impl humanity_core::auto_unlock::SecretStore,
    state: &mut GuiState,
    mode: humanity_core::auto_unlock::AutoUnlockMode,
    secret: Option<&[u8; 32]>,
    pin_blob: Option<(String, String)>,
) -> Result<(), &'static str> {
    use humanity_core::auto_unlock::{self as au, AutoUnlockMode, RememberError};
    // An empty identity is `remember`'s to judge: nothing to forget is not a failure, so Always
    // ask always works; a remembering mode is refused there (the second review).
    let identity = state.profile_public_key.clone();
    let result = au::remember(store, mode, &identity, secret);
    if let Err(e) = &result {
        log::warn!("unlock on launch: switching to {mode:?}: {}", e.details());
        if matches!(e, RememberError::NotKept(_)) {
            state.unlock_alert = Some(e.words_key());
            return Err(e.words_key());
        }
    }
    let (enc, salt) = if mode == AutoUnlockMode::KeychainPin { pin_blob.unwrap_or_default() } else { Default::default() };
    state.pin_encrypted_seed = enc;
    state.pin_salt = salt;
    state.auto_unlock_mode = mode;
    crate::config::AppConfig::from_gui_state(state).save();
    log::info!("unlock on launch: mode -> {mode:?}");
    let result = result.map_err(|e| e.words_key());
    state.unlock_alert = result.err();
    result
}

/// Settings' Try again beside a forget that failed (and choosing Always ask again): forget what
/// the modes not chosen still keep. Nothing new is kept, so no secret is needed, whichever mode
/// is chosen. The outcome lands in `state.unlock_alert` as a switch's does.
pub(crate) fn forget_other_modes(state: &mut GuiState) -> Result<(), &'static str> {
    forget_other_modes_in(&mut humanity_core::auto_unlock::ThisDevice, state)
}

/// [`forget_other_modes`] with the key store named.
pub(crate) fn forget_other_modes_in(store: &mut impl humanity_core::auto_unlock::SecretStore, state: &mut GuiState) -> Result<(), &'static str> {
    let identity = state.profile_public_key.clone();
    let result = humanity_core::auto_unlock::forget_others(store, state.auto_unlock_mode, &identity).map_err(|e| {
        log::warn!("unlock on launch: forgetting again: {}", e.details());
        e.words_key()
    });
    state.unlock_alert = result.err();
    result
}

/// "Skip (limited mode)" in the unlock dialog: it closes with the key still locked, and its
/// Remember tick goes with it, so no later unlock remembers the seed unasked (the second review:
/// the tick survived, and Account's Reveal recovery phrase then stored the seed).
pub(crate) fn skip_unlock(state: &mut GuiState) {
    state.passphrase_needed = false;
    humanity_core::platform::key_wrap::wipe_string(&mut state.passphrase_input);
    state.passphrase_status.clear();
    state.remember_on_device = false;
    log::info!("User skipped passphrase; private key unavailable");
}

/// The unlock dialog's "Remember on this device", once the passphrase has opened the key: the
/// seed kept through the one switch every mode change takes (a Quick PIN set up before is
/// forgotten with it). The unlock itself succeeded whatever happens here; a remember that did not
/// fully happen is said as a sentence by key, as a notice now (the dialog closes) and in Settings'
/// alert (`set_unlock_mode_in` keeps it), never only as a log line (the second review).
pub(crate) fn remember_after_unlock(store: &mut impl humanity_core::auto_unlock::SecretStore, state: &mut GuiState, key_bytes: &[u8], now: f64) {
    let result = if key_bytes.len() != 32 {
        log::warn!("Auto-unlock stash skipped: seed is {} bytes, expected 32", key_bytes.len());
        state.unlock_alert = Some("unlock.stash_failed");
        Err("unlock.stash_failed")
    } else {
        let mut seed = [0u8; 32];
        seed.copy_from_slice(key_bytes);
        let result = set_unlock_mode_in(store, state, humanity_core::auto_unlock::AutoUnlockMode::Keychain, Some(&seed), None);
        humanity_core::platform::key_wrap::wipe(&mut seed);
        result
    };
    if let Err(key) = result {
        log::warn!("Auto-unlock: Remember on this device did not complete ({key})");
        state.notice(humanity_core::i18n::t(key), now);
    }
}

/// Common helper: load the device_key from the OS keychain for the
/// current identity. Returns None when the entry is genuinely absent
/// (caller's UI surface should display "PIN setup gone, re-run setup
/// or use passphrase"); Err on platform failure (logged + None).
fn load_device_key(state: &GuiState) -> Option<[u8; 32]> {
    if state.profile_public_key.is_empty() {
        log::warn!("PIN flow: no profile_public_key in state");
        return None;
    }
    match humanity_core::auto_unlock::keychain_load(
        humanity_core::auto_unlock::KeychainSlot::DeviceKey,
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
    ui.label(RichText::new(t("passphrase.pin_setup_title"))
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        t("passphrase.pin_setup_intro"))
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new(t("passphrase.pin_label_digits")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.pin_input", &mut state.pin_input).char_limit(humanity_core::auto_unlock::PIN_MAX_LEN).show(ui, theme);

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("passphrase.pin_confirm_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.pin_confirm", &mut state.pin_confirm).char_limit(humanity_core::auto_unlock::PIN_MAX_LEN).show(ui, theme);

    ui.add_space(theme.spacing_md);

    if !state.pin_status.is_empty() {
        ui.label(RichText::new(&state.pin_status)
            .color(theme.danger())
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, t("passphrase.pin_set")) {
            // Validation
            if let Err(msg) = humanity_core::auto_unlock::validate_pin(&state.pin_input) {
                state.pin_status = msg;
                return;
            }
            if state.pin_input != state.pin_confirm {
                state.pin_status = t("passphrase.pin_no_match").to_string();
                return;
            }
            // Checked before the seed is copied, so no early return leaves a copy unwiped (the
            // second review's C).
            if state.profile_public_key.is_empty() {
                state.pin_status = t("passphrase.pin_no_identity").to_string();
                return;
            }
            // Must already have the seed in memory (set up post-unlock
            // or right after fresh identity generation).
            let mut seed_bytes = match &state.private_key_bytes {
                Some(b) if b.len() == 32 => {
                    let mut s = [0u8; 32];
                    s.copy_from_slice(b);
                    s
                }
                _ => {
                    state.pin_status = t("passphrase.pin_unlock_first").to_string();
                    return;
                }
            };

            // A fresh device key; the seed sealed with the PIN and it (nothing is kept yet).
            let mut device_key = match humanity_core::auto_unlock::generate_device_key() {
                Ok(k) => k,
                Err(e) => {
                    humanity_core::platform::key_wrap::wipe(&mut seed_bytes);
                    state.pin_status = tf("passphrase.pin_rng_failed", &[("error", &e)]);
                    return;
                }
            };
            let encrypted = humanity_core::auto_unlock::encrypt_seed_with_pin(&seed_bytes, &state.pin_input, &device_key);
            humanity_core::platform::key_wrap::wipe(&mut seed_bytes);
            match encrypted {
                // Then the one switch every mode change takes: the device key kept, the seed
                // Remember on this device kept forgotten (BUG-335), the blob and the mode saved.
                Ok(blob) => match set_unlock_mode(state, humanity_core::auto_unlock::AutoUnlockMode::KeychainPin, Some(&device_key), Some(blob)) {
                    Ok(()) => {
                        state.passphrase_needed = false;
                        wipe_pin_boxes(state);
                        state.pin_status.clear();
                    }
                    Err(key) if key == "unlock.forget_failed" => {
                        // The PIN works; something kept before could not be forgotten: said here.
                        wipe_pin_boxes(state);
                        state.pin_status = humanity_core::i18n::t(key).to_string();
                    }
                    Err(key) => state.pin_status = humanity_core::i18n::t(key).to_string(),
                },
                Err(e) => {
                    state.pin_status = tf("passphrase.pin_encrypt_failed", &[("error", &e)]);
                }
            }
            humanity_core::platform::key_wrap::wipe(&mut device_key);
        }

        if widgets::secondary_button(ui, theme, t("passphrase.cancel")) {
            state.passphrase_needed = false;
            wipe_pin_boxes(state);
            state.pin_status.clear();
        }
    });
}

fn draw_pin_unlock(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("passphrase.pin_unlock_title"))
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(
        t("passphrase.pin_unlock_intro"))
        .size(theme.font_size_small)
        .color(theme.text_secondary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new(t("passphrase.pin_label")).color(theme.text_secondary()));
    let response = widgets::secret_field::SecretField::new("passphrase_modal.pin_input", &mut state.pin_input).char_limit(humanity_core::auto_unlock::PIN_MAX_LEN).show(ui, theme);

    ui.add_space(theme.spacing_md);

    if !state.pin_status.is_empty() {
        ui.label(RichText::new(&state.pin_status)
            .color(theme.danger())
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

    // Drain a finished background PIN unlock (the PBKDF2 in decrypt_seed_with_pin
    // ran off the UI thread, same as the passphrase path).
    if state.pin_unlocking {
        let drained = state.pin_unlock_rx.as_ref().map(|rx| rx.try_recv());
        match drained {
            Some(Ok(Ok(seed))) => {
                state.pin_unlock_rx = None;
                state.pin_unlocking = false;
                state.private_key_bytes = Some(seed);
                state.passphrase_needed = false;
                wipe_pin_boxes(state);
                state.pin_status.clear();
                state.apply_pq_identity();
                log::info!("Unlocked via PIN");
            }
            Some(Ok(Err(_))) => {
                state.pin_unlock_rx = None;
                state.pin_unlocking = false;
                // Deliberately generic — see decrypt_seed_with_pin docstring on
                // why we don't distinguish bad-PIN from corrupted-blob.
                state.pin_status = t("passphrase.pin_wrong").to_string();
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                state.pin_unlock_rx = None;
                state.pin_unlocking = false;
                state.pin_status = t("passphrase.pin_worker_stopped").to_string();
            }
            _ => {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
            }
        }
    }

    ui.horizontal(|ui| {
        if state.pin_unlocking {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new(t("passphrase.unlocking"))
                .color(theme.text_secondary())
                .size(theme.font_size_small));
        } else {
            if widgets::primary_button(ui, theme, t("passphrase.unlock")) || enter_pressed {
                let mut device_key = match load_device_key(state) {
                    Some(k) => k,
                    None => {
                        state.pin_status = t("passphrase.pin_gone_unlock").to_string();
                        return;
                    }
                };
                // Run the PBKDF2 (decrypt_seed_with_pin) on a worker thread. Every plaintext copy
                // is wiped once used (the second review's C): the worker's device key and PIN
                // after the decrypt, the 32-byte seed once it is in the Vec the app keeps, and
                // this thread's device key (an array is Copy, so the worker got its own) at once.
                let enc = state.pin_encrypted_seed.clone();
                let salt = state.pin_salt.clone();
                let pin = state.pin_input.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let (mut device_key, mut pin) = (device_key, pin);
                    let result = humanity_core::auto_unlock::decrypt_seed_with_pin(&enc, &salt, &pin, &device_key)
                        .map(|mut seed| {
                            let kept = seed.to_vec();
                            humanity_core::platform::key_wrap::wipe(&mut seed);
                            kept
                        })
                        .map_err(|e| e.to_string());
                    humanity_core::platform::key_wrap::wipe(&mut device_key);
                    humanity_core::platform::key_wrap::wipe_string(&mut pin);
                    let _ = tx.send(result);
                });
                humanity_core::platform::key_wrap::wipe(&mut device_key);
                state.pin_unlock_rx = Some(rx);
                state.pin_unlocking = true;
                state.pin_status.clear();
            }

            if widgets::secondary_button(ui, theme, t("passphrase.use_passphrase")) {
                state.passphrase_mode = PassphraseMode::Unlock;
                wipe_pin_boxes(state);
                state.pin_status.clear();
            }
        }
    });
}

fn draw_pin_change(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("passphrase.pin_change_title"))
        .size(theme.font_size_heading)
        .color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new(t("passphrase.pin_current_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.pin_old_input", &mut state.pin_old_input).char_limit(humanity_core::auto_unlock::PIN_MAX_LEN).show(ui, theme);

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("passphrase.pin_new_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.pin_input", &mut state.pin_input).char_limit(humanity_core::auto_unlock::PIN_MAX_LEN).show(ui, theme);

    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("passphrase.pin_confirm_new_label")).color(theme.text_secondary()));
    widgets::secret_field::SecretField::new("passphrase_modal.pin_confirm", &mut state.pin_confirm).char_limit(humanity_core::auto_unlock::PIN_MAX_LEN).show(ui, theme);

    ui.add_space(theme.spacing_md);

    if !state.pin_status.is_empty() {
        let ok = state.pin_status == t("passphrase.pin_changed");
        let color = if ok { theme.success() } else { theme.danger() };
        ui.label(RichText::new(&state.pin_status)
            .color(color)
            .size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
    }

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, theme, t("passphrase.pin_change_title")) {
            if let Err(msg) = humanity_core::auto_unlock::validate_pin(&state.pin_input) {
                state.pin_status = msg;
                return;
            }
            if state.pin_input != state.pin_confirm {
                state.pin_status = t("passphrase.pin_new_no_match").to_string();
                return;
            }
            let Some(mut device_key) = load_device_key(state) else {
                state.pin_status = t("passphrase.pin_gone_settings").to_string();
                return;
            };
            // Verify the old PIN by opening the existing blob, then re-seal with the new one
            // (same device key, no churn in the key store). The device key and the seed are wiped
            // on every way out, a wrong PIN included (the second review's C).
            let resealed = match humanity_core::auto_unlock::decrypt_seed_with_pin(
                &state.pin_encrypted_seed, &state.pin_salt,
                &state.pin_old_input, &device_key,
            ) {
                Ok(mut seed) => {
                    let sealed = humanity_core::auto_unlock::encrypt_seed_with_pin(&seed, &state.pin_input, &device_key);
                    humanity_core::platform::key_wrap::wipe(&mut seed);
                    Some(sealed)
                }
                Err(_) => None,
            };
            humanity_core::platform::key_wrap::wipe(&mut device_key);
            match resealed {
                None => state.pin_status = t("passphrase.pin_current_wrong").to_string(),
                Some(Ok((enc, salt))) => {
                    state.pin_encrypted_seed = enc;
                    state.pin_salt = salt;
                    wipe_pin_boxes(state);
                    state.pin_status = t("passphrase.pin_changed").to_string();
                    crate::config::AppConfig::from_gui_state(state).save();
                    log::info!("PIN changed");
                }
                Some(Err(e)) => {
                    state.pin_status = tf("passphrase.pin_reencrypt_failed", &[("error", &e)]);
                }
            }
        }

        if widgets::secondary_button(ui, theme, t("passphrase.cancel")) {
            state.passphrase_needed = false;
            wipe_pin_boxes(state);
            state.pin_status.clear();
        }
    });
}

/// The PIN boxes emptied with their bytes zeroed (`key_wrap::wipe_string`), not just cleared.
fn wipe_pin_boxes(state: &mut GuiState) {
    use humanity_core::platform::key_wrap::wipe_string;
    wipe_string(&mut state.pin_old_input);
    wipe_string(&mut state.pin_input);
    wipe_string(&mut state.pin_confirm);
}

#[cfg(test)]
#[path = "passphrase_modal_tests.rs"]
mod tests;

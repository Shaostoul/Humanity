//! Settings panel with sidebar navigation and category content panels.
//!
//! Categories: Account, Appearance, Notifications, Wallet, Audio,
//! Graphics, Controls, Privacy, Data, Updates.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiState, SettingsCategory, WalletNetwork, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::updater::{UpdateChannel, UpdateState};

// Full-PQ: the web "import ECDH key" flow is gone. DMs are pure Kyber768
// derived from the BIP39 seed — identical on web and native with no manual
// key import, ever (that import existed only to bridge the old random
// per-browser ECDH key, the very thing that caused the cross-client bug).

/// Styling params for inline sliders (captured before mutable theme borrows).
struct SliderStyle {
    track_color: Color32,
    track_h: f32,
    thumb_r: f32,
    accent: Color32,
    accent_hover: Color32,
    font_sm: f32,
}

impl SliderStyle {
    fn from_theme(theme: &Theme) -> Self {
        Self {
            track_color: theme.slider_track(),
            track_h: theme.slider_track_height,
            thumb_r: theme.slider_thumb_radius,
            accent: theme.accent(),
            accent_hover: theme.accent_hover(),
            font_sm: theme.font_size_small,
        }
    }
}

/// Inline slider for the Widgets section where theme fields are mutably borrowed.
/// Pre-captured styling avoids borrow conflicts with &mut theme.field.
fn styled_slider(
    ui: &mut egui::Ui,
    style: &SliderStyle,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label_color: Color32,
) -> bool {
    let min = *range.start();
    let max = *range.end();
    let mut changed = false;
    ui.horizontal(|ui| {
        // Fixed-width label column so every slider track starts at the SAME x.
        // The old 120px was too narrow for labels like "Settings Label Width" or
        // "Button Padding H" — they overflowed and pushed their slider right (the
        // "stepping" the operator flagged). 170px fits the longest, so all the
        // tracks align into one clean column.
        ui.allocate_ui_with_layout(
            Vec2::new(170.0, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new(label).color(label_color)); },
        );
        let desired_width = ui.available_width().min(200.0);
        let widget_height = style.thumb_r * 2.0 + 4.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(desired_width, widget_height),
            egui::Sense::click_and_drag(),
        );
        let old_val = *value;
        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                *value = min + t * (max - min);
            }
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let cy = rect.center().y;
            let t = if (max - min).abs() < f32::EPSILON { 0.5 } else { (*value - min) / (max - min) };
            let tx = rect.left() + t * rect.width();
            let tr = Rounding::same((style.track_h / 2.0) as u8);
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(rect.left(), cy - style.track_h / 2.0), egui::pos2(rect.right(), cy + style.track_h / 2.0)),
                tr, style.track_color,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(rect.left(), cy - style.track_h / 2.0), egui::pos2(tx, cy + style.track_h / 2.0)),
                tr, style.accent,
            );
            let tc = if response.hovered() || response.dragged() { style.accent_hover } else { style.accent };
            painter.circle_filled(egui::pos2(tx, cy), style.thumb_r, tc);
        }
        changed = (*value - old_val).abs() > f32::EPSILON;
        let vt = if max <= 4.0 { format!("{:.1}", *value) } else { format!("{:.0}", *value) };
        ui.label(RichText::new(vt).color(label_color).size(style.font_sm));
    });
    changed
}

pub fn draw(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    // Left sidebar: Table of Contents with jump links
    egui::SidePanel::left("settings_sidebar")
        .default_width(180.0)
        .min_width(140.0)
        .max_width(240.0)
        .frame(Frame::none()
            .fill(theme.bg_sidebar())
            .inner_margin(egui::Margin::symmetric(8, 12))
            .stroke(Stroke::new(1.0, theme.border())))
        .show(ctx, |ui| {
            ui.label(RichText::new("Settings").size(theme.font_size_heading).color(theme.text_primary()));
            ui.add_space(theme.spacing_md);

            let categories = [
                ("Account", SettingsCategory::Account),
                ("Appearance", SettingsCategory::Appearance),
                ("Animations", SettingsCategory::Animations),
                ("Widgets", SettingsCategory::Widgets),
                ("Notifications", SettingsCategory::Notifications),
                ("Wallet", SettingsCategory::Wallet),
                ("Audio", SettingsCategory::Audio),
                ("Graphics", SettingsCategory::Graphics),
                ("Gameplay", SettingsCategory::Gameplay),
                ("Controls", SettingsCategory::Controls),
                ("Privacy", SettingsCategory::Privacy),
                ("Data", SettingsCategory::Data),
                ("Updates", SettingsCategory::Updates),
            ];

            for (label, cat) in &categories {
                let is_active = state.settings.category == *cat;
                let text_color = if is_active { Color32::WHITE } else { theme.text_muted() };
                let bg = if is_active {
                    Color32::from_rgba_unmultiplied(237, 140, 36, 30)
                } else {
                    Color32::TRANSPARENT
                };

                let btn = egui::Button::new(
                    RichText::new(*label).size(theme.font_size_body).color(text_color),
                )
                .fill(bg)
                .stroke(if is_active {
                    Stroke::new(1.0, theme.accent())
                } else {
                    Stroke::NONE
                })
                .rounding(Rounding::same(4))
                .min_size(Vec2::new(ui.available_width(), 28.0));

                if ui.add(btn).clicked() {
                    state.settings.scroll_to_section = Some(*cat);
                }
            }
        });

    // Right content area: all sections in one infinite scroll
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .id_salt("settings_scroll")
                .show(ui, |ui| {
                    let visible_top = ui.clip_rect().top();
                    let mut section_rects: Vec<(SettingsCategory, egui::Rect)> = Vec::new();
                    let categories_order = [
                        SettingsCategory::Account,
                        SettingsCategory::Appearance,
                        SettingsCategory::Animations,
                        SettingsCategory::Widgets,
                        SettingsCategory::Notifications,
                        SettingsCategory::Wallet,
                        SettingsCategory::Audio,
                        SettingsCategory::Graphics,
                        SettingsCategory::Gameplay,
                        SettingsCategory::Controls,
                        SettingsCategory::Privacy,
                        SettingsCategory::Data,
                        SettingsCategory::Updates,
                    ];

                    for (i, cat) in categories_order.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(theme.spacing_xl);
                            ui.separator();
                            ui.add_space(theme.spacing_md);
                        }

                        // Section heading
                        let heading_text = match cat {
                            SettingsCategory::Account => "Account",
                            SettingsCategory::Appearance => "Appearance",
                            SettingsCategory::Animations => "Animations",
                            SettingsCategory::Widgets => "Widgets",
                            SettingsCategory::Notifications => "Notifications",
                            SettingsCategory::Wallet => "Wallet",
                            SettingsCategory::Audio => "Audio",
                            SettingsCategory::Graphics => "Graphics",
                            SettingsCategory::Gameplay => "Gameplay",
                            SettingsCategory::Controls => "Controls",
                            SettingsCategory::Privacy => "Privacy",
                            SettingsCategory::Data => "Data",
                            SettingsCategory::Updates => "Updates",
                        };
                        let heading_response = ui.label(
                            RichText::new(heading_text)
                                .size(theme.font_size_title)
                                .color(theme.text_primary()),
                        );
                        section_rects.push((*cat, heading_response.rect));
                        ui.add_space(theme.spacing_md);

                        // Draw section content
                        match cat {
                            SettingsCategory::Account => draw_account_content(ui, theme, state),
                            SettingsCategory::Appearance => draw_appearance_content(ui, theme, state),
                            SettingsCategory::Animations => draw_animations_content(ui, theme, state),
                            SettingsCategory::Widgets => draw_widgets_content(ui, theme, state),
                            SettingsCategory::Notifications => draw_notifications_content(ui, theme, state),
                            SettingsCategory::Wallet => draw_wallet_content(ui, theme, state),
                            SettingsCategory::Audio => draw_audio_content(ui, theme, state),
                            SettingsCategory::Graphics => draw_graphics_content(ui, theme, state),
                            SettingsCategory::Gameplay => draw_gameplay_content(ui, theme, state),
                            SettingsCategory::Controls => draw_controls_content(ui, theme, state),
                            SettingsCategory::Privacy => draw_privacy_content(ui, theme, state),
                            SettingsCategory::Data => draw_data_content(ui, theme, state),
                            SettingsCategory::Updates => draw_updates_content(ui, theme, state),
                        }
                    }

                    // Handle scroll-to-section
                    if let Some(target) = state.settings.scroll_to_section.take() {
                        for (cat, rect) in &section_rects {
                            if *cat == target {
                                ui.scroll_to_rect(*rect, Some(egui::Align::TOP));
                                break;
                            }
                        }
                    }

                    // Track which section is currently visible for TOC highlight
                    let mut active_section = SettingsCategory::Account;
                    for (cat, rect) in &section_rects {
                        if rect.top() <= visible_top + 60.0 {
                            active_section = *cat;
                        }
                    }
                    state.settings.category = active_section;
                });
        });
}

/// Build a scannable QR texture for a device-link payload. Black/white are
/// intentional (a QR must be high-contrast to scan regardless of app theme, so
/// these are not theme tokens). Matrix from the `qrcode` crate, painted into an
/// egui texture with a 4-module quiet zone. Returns None if encoding fails
/// (payload too large for any QR version, which the ~250-char backup never is).
fn build_link_qr_texture(ctx: &egui::Context, payload: &str) -> Option<egui::TextureHandle> {
    let code = qrcode::QrCode::new(payload.as_bytes()).ok()?;
    let width = code.width();
    let colors = code.to_colors();
    let quiet = 4usize; // standard QR quiet zone (modules)
    let scale = 6usize; // pixels per module
    let side = (width + quiet * 2) * scale;
    let mut pixels = vec![egui::Color32::WHITE; side * side];
    for my in 0..width {
        for mx in 0..width {
            // Color::select(dark, light) -> returns the first value when Dark.
            if colors[my * width + mx].select(true, false) {
                let x0 = (mx + quiet) * scale;
                let y0 = (my + quiet) * scale;
                for dy in 0..scale {
                    let row = (y0 + dy) * side + x0;
                    for dx in 0..scale {
                        pixels[row + dx] = egui::Color32::BLACK;
                    }
                }
            }
        }
    }
    let image = egui::ColorImage { size: [side, side], pixels };
    Some(ctx.load_texture("link_device_qr", image, egui::TextureOptions::NEAREST))
}

pub(crate) fn draw_account_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, "Display name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.user_name).desired_width(200.0));
        });

        widgets::form_row(ui, theme, "Boot page", |ui| {
            let current_label = crate::gui::BOOT_PAGE_OPTIONS.iter()
                .find(|(p, _)| *p == state.default_page)
                .map(|(_, l)| *l)
                .unwrap_or("Onboarding");
            egui::ComboBox::from_id_salt("boot_page_combo")
                .selected_text(current_label)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for &(page, label) in crate::gui::BOOT_PAGE_OPTIONS {
                        if ui.selectable_value(&mut state.default_page, page, label).changed() {
                            crate::config::AppConfig::from_gui_state(state).save();
                        }
                    }
                });
        });

        widgets::form_row(ui, theme, "Public key", |ui| {
            let key_display = if state.profile_public_key.is_empty() {
                "No key generated".to_string()
            } else if state.profile_public_key.len() > 16 {
                format!("{}…{}", &state.profile_public_key[..8], &state.profile_public_key[state.profile_public_key.len()-8..])
            } else {
                state.profile_public_key.clone()
            };
            ui.label(RichText::new(&key_display).color(theme.text_muted()).size(theme.font_size_small));
            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Copy") {
                ui.ctx().copy_text(state.profile_public_key.clone());
            }
        });

        ui.add_space(theme.spacing_md);

        // Full-PQ: there is no DM-key UI. The Kyber768 DM key (and the
        // Dilithium3 identity) derive deterministically from the seed
        // phrase below — identical on every device, nothing to copy or
        // import. The old "ECDH public / Import JSON" panel was removed.

        // Identity & seed phrase
        ui.label(RichText::new("Identity & Seed Phrase").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        if state.private_key_bytes.is_none() {
            // No identity on this device — let the user CREATE one. This
            // is the primitive native was missing entirely (web had it):
            // without it a first-time native user can never get a seed.
            ui.label(RichText::new("No identity on this device yet. Generate one (creates a fresh 24-word seed, your only backup), or recover an existing seed below.").color(theme.text_muted()).size(theme.font_size_small));
            ui.add_space(theme.spacing_xs);
            if widgets::primary_button(ui, theme, "  Generate New Identity  ") {
                let seed = crate::net::identity::generate_new_seed();
                state.private_key_bytes = Some(seed);
                // Derive Dilithium+Kyber + reconnect to advertise it.
                state.apply_pq_identity();
                state.settings.seed_phrase_visible = true;
                state.settings.seed_phrase_recovery_status =
                    "New identity generated. WRITE DOWN the 24 words below, they are the ONLY way to recover this account.".to_string();
                // Prompt to encrypt the new seed with a passphrase.
                state.passphrase_needed = true;
                state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
            }
            if !state.settings.seed_phrase_recovery_status.is_empty() {
                ui.add_space(theme.spacing_xs);
                ui.label(RichText::new(&state.settings.seed_phrase_recovery_status).color(theme.success()).size(theme.font_size_small));
            }
        } else {
            ui.label(RichText::new("Your 24-word seed phrase backs up your identity and wallet. Anyone with it controls your account, never share it.").color(theme.text_muted()).size(theme.font_size_small));
            ui.add_space(theme.spacing_xs);

            // Passphrase-gated reveal (v0.356). The seed is your account's master
            // key, so showing it now requires RE-ENTERING the vault passphrase —
            // it can no longer be revealed from an unlocked-but-unattended screen
            // with one click. Re-locks on Lock / restart. If no passphrase vault
            // exists yet (a just-generated seed not encrypted yet), fall back to
            // the plain show/hide so the user can still write down their words.
            let enc = state.encrypted_private_key.clone();
            let salt = state.key_salt.clone();
            let iters = state.key_iterations;
            let reveal = if !enc.is_empty() && !salt.is_empty() {
                let lock = state.section_locks.entry("seed_phrase".to_string()).or_default();
                widgets::lockable_gate(ui, theme, lock, "Reveal seed phrase", |pass| {
                    crate::config::decrypt_private_key(&enc, &salt, pass, iters).is_ok()
                })
            } else {
                if widgets::secondary_button(ui, theme, if state.settings.seed_phrase_visible { "Hide Seed Phrase" } else { "Show Seed Phrase" }) {
                    state.settings.seed_phrase_visible = !state.settings.seed_phrase_visible;
                }
                state.settings.seed_phrase_visible
            };
            if reveal {
                ui.add_space(theme.spacing_xs);
                // Render the REAL phrase from the in-memory seed (this was
                // previously a stub that always said "not generated yet").
                let phrase = state.private_key_bytes.as_ref()
                    .and_then(|s| crate::net::identity::mnemonic_from_seed(s))
                    .unwrap_or_else(|| "(cannot render, key is not a 32-byte BIP39 seed)".to_string());
                egui::Frame::none()
                    .fill(Color32::from_rgb(40, 30, 20))
                    .rounding(Rounding::same(4))
                    .inner_margin(8.0)
                    .stroke(Stroke::new(1.0, theme.warning()))
                    .show(ui, |ui| {
                        ui.label(RichText::new(&phrase).color(theme.warning()).size(theme.font_size_small));
                        ui.add_space(theme.spacing_xs);
                        if widgets::secondary_button(ui, theme, "Copy") {
                            ui.ctx().copy_text(phrase.clone());
                        }
                    });
            }

            // ── Link a Device (v0.838) ──
            // Standalone + discoverable. In v0.837 this QR lived INSIDE the
            // seed-words reveal above, so it was invisible until you unlocked the
            // seed and nobody found it. It is now its own labelled action with its
            // OWN passphrase gate (the QR encodes your seed, so it must still be
            // gated). A phone scans it to bring this identity onto it.
            ui.add_space(theme.spacing_lg);
            ui.label(RichText::new("Link a Device").color(theme.text_secondary()).strong());
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new("Show a QR another device can scan to bring this identity onto it (on that device: chat > your identity > \"Link this device to me\" > \"Scan a QR code\"). The QR contains your seed, so unlocking is required.").color(theme.text_muted()).size(theme.font_size_small));
            ui.add_space(theme.spacing_xs);
            let qr_reveal = if !enc.is_empty() && !salt.is_empty() {
                let lock = state.section_locks.entry("link_device_qr".to_string()).or_default();
                widgets::lockable_gate(ui, theme, lock, "Show device-link QR", |pass| {
                    crate::config::decrypt_private_key(&enc, &salt, pass, iters).is_ok()
                })
            } else {
                if widgets::secondary_button(ui, theme, if state.link_device_qr_show { "Hide device-link QR" } else { "Show device-link QR (scan from a phone)" }) {
                    state.link_device_qr_show = !state.link_device_qr_show;
                    if !state.link_device_qr_show { state.link_device_qr = None; }
                }
                state.link_device_qr_show
            };
            if qr_reveal {
                // Encode a fragment URL (not raw JSON): a system camera then
                // NAVIGATES to the chat page instead of searching the seed. See
                // net::identity::device_link_url.
                let payload = state.private_key_bytes.as_ref()
                    .and_then(|s| crate::net::identity::device_link_url(s, &state.user_name));
                match payload {
                    Some(payload) => {
                        let stale = match &state.link_device_qr {
                            Some((p, _)) => *p != payload,
                            None => true,
                        };
                        if stale {
                            state.link_device_qr = build_link_qr_texture(ui.ctx(), &payload)
                                .map(|tex| (payload.clone(), tex));
                        }
                        if let Some((_, tex)) = &state.link_device_qr {
                            ui.add_space(theme.spacing_xs);
                            ui.image(egui::load::SizedTexture::from_handle(tex));
                            ui.add_space(theme.spacing_xs);
                            ui.label(RichText::new("Anyone who scans this becomes you. Only show it to your own devices, in private.").color(theme.warning()).size(theme.font_size_small));
                        } else {
                            ui.label(RichText::new("(Could not build the QR code.)").color(theme.text_muted()).size(theme.font_size_small));
                        }
                    }
                    None => {
                        ui.label(RichText::new("(Cannot build QR: key is not a 32-byte BIP39 seed.)").color(theme.text_muted()).size(theme.font_size_small));
                    }
                }
            }

            // ── Replace Identity (v0.842) ──
            // Generate a fresh identity that REPLACES the current one. The
            // "Generate New Identity" button (above) only renders when the device
            // has NO identity yet, so once you have one -- which is everyone after
            // first run -- there was no in-app way to ROTATE, e.g. away from a
            // compromised/exposed key. Two-click confirm since it replaces the key.
            ui.add_space(theme.spacing_lg);
            ui.label(RichText::new("Replace Identity").color(theme.text_secondary()).strong());
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new("Generate a brand-new identity (new seed + keys) on this device, replacing the current one -- for rotating away from a compromised or exposed key. Back up your current seed above first if you still need it.").color(theme.text_muted()).size(theme.font_size_small));
            ui.add_space(theme.spacing_xs);
            let regen_id = egui::Id::new("regen_identity_confirm");
            let regen_confirming = ui.ctx().data(|d| d.get_temp::<bool>(regen_id).unwrap_or(false));
            if !regen_confirming {
                if widgets::secondary_button(ui, theme, "Generate New Identity (replace current)") {
                    ui.ctx().data_mut(|d| d.insert_temp(regen_id, true));
                }
            } else {
                ui.label(RichText::new("This permanently replaces the identity on THIS device. Your current seed is gone unless you backed it up. Continue?").color(theme.warning()).size(theme.font_size_small));
                ui.add_space(theme.spacing_xs);
                if widgets::primary_button(ui, theme, "  Yes, generate a new identity  ") {
                    let seed = crate::net::identity::generate_new_seed();
                    state.private_key_bytes = Some(seed);
                    state.apply_pq_identity();
                    state.settings.seed_phrase_visible = true;
                    state.settings.seed_phrase_recovery_status =
                        "New identity generated. WRITE DOWN the 24 words above -- they are the ONLY backup.".to_string();
                    state.passphrase_needed = true;
                    state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
                    // Drop the cached device-link QR so it rebuilds for the NEW identity.
                    state.link_device_qr = None;
                    state.link_device_qr_show = false;
                    ui.ctx().data_mut(|d| d.insert_temp(regen_id, false));
                }
                if widgets::secondary_button(ui, theme, "Cancel") {
                    ui.ctx().data_mut(|d| d.insert_temp(regen_id, false));
                }
            }
        }

        ui.add_space(theme.spacing_lg);

        // ── Recover from Seed Phrase ──
        ui.label(RichText::new("Recover Identity from Seed Phrase").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Paste your 24-word seed phrase to restore your identity from the website or another device.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);

        if widgets::secondary_button(ui, theme, if state.settings.seed_phrase_show_recover { "Cancel Recovery" } else { "Recover from Seed Phrase" }) {
            state.settings.seed_phrase_show_recover = !state.settings.seed_phrase_show_recover;
            state.settings.seed_phrase_recovery_status.clear();
        }

        if state.settings.seed_phrase_show_recover {
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new("Enter your 24-word seed phrase:").color(theme.text_secondary()).size(theme.font_size_small));
            ui.add_space(theme.spacing_xs);

            ui.add(egui::TextEdit::multiline(&mut state.settings.seed_phrase_input)
                .desired_width(ui.available_width())
                .desired_rows(3)
                .hint_text("word1 word2 word3 ... (24 words)"));

            ui.add_space(theme.spacing_sm);

            if widgets::primary_button(ui, theme, "  Recover Identity  ") {
                let phrase = state.settings.seed_phrase_input.trim().to_string();
                match crate::net::identity::derive_keypair_from_mnemonic(&phrase) {
                    Ok((_ed25519_hex, privkey_bytes)) => {
                        // Full-PQ: the chat identity is Dilithium3 (NOT the
                        // Ed25519 hex). apply_pq_identity() derives it +
                        // Kyber from the seed and forces the reconnect.
                        state.private_key_bytes = Some(privkey_bytes);
                        if let Some(ref mut ws) = state.ws_client {
                            ws.disconnect();
                        }
                        state.apply_pq_identity();
                        state.settings.seed_phrase_recovery_status = format!(
                            "Identity recovered! {}…{}",
                            &state.profile_public_key[..8.min(state.profile_public_key.len())],
                            &state.profile_public_key[state.profile_public_key.len().saturating_sub(8)..]
                        );
                        state.ws_status = "Reconnecting with recovered identity...".to_string();
                        state.identity_recovered = true;
                        state.history_fetched = false;
                        // Prompt for passphrase to encrypt the recovered key
                        state.passphrase_needed = true;
                        state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
                        // Clear the input
                        state.settings.seed_phrase_input.clear();
                        state.settings.seed_phrase_show_recover = false;
                    }
                    Err(e) => {
                        state.settings.seed_phrase_recovery_status = format!("Error: {}", e);
                    }
                }
            }

            if !state.settings.seed_phrase_recovery_status.is_empty() {
                ui.add_space(theme.spacing_xs);
                let color = if state.settings.seed_phrase_recovery_status.starts_with("Error") {
                    theme.danger()
                } else {
                    Color32::from_rgb(46, 204, 113)
                };
                ui.label(RichText::new(&state.settings.seed_phrase_recovery_status).color(color).size(theme.font_size_small));
            }
        }
    });

    ui.add_space(theme.spacing_md);

    // Change Passphrase section
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Key Encryption").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        if !state.encrypted_private_key.is_empty() && state.private_key_bytes.is_some() {
            // Key is encrypted AND unlocked
            ui.label(RichText::new("Your private key is encrypted and unlocked.")
                .color(theme.text_muted()).size(theme.font_size_small));
            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Change Passphrase") {
                state.passphrase_needed = true;
                state.passphrase_mode = crate::gui::PassphraseMode::Change;
                state.passphrase_status.clear();
            }
        } else if !state.encrypted_private_key.is_empty() && state.private_key_bytes.is_none() {
            // Key is encrypted but NOT unlocked (limited mode)
            ui.label(RichText::new("Your private key is locked. Unlock to enable signing and wallet features.")
                .color(theme.warning()).size(theme.font_size_small));
            ui.add_space(theme.spacing_sm);
            if widgets::primary_button(ui, theme, "Unlock Key") {
                state.passphrase_needed = true;
                state.passphrase_mode = crate::gui::PassphraseMode::Unlock;
                state.passphrase_status.clear();
            }
        } else if state.private_key_bytes.is_some() {
            ui.label(RichText::new("Your private key is not encrypted. Set a passphrase to protect it.")
                .color(theme.warning()).size(theme.font_size_small));
            ui.add_space(theme.spacing_sm);
            if widgets::primary_button(ui, theme, "Set Passphrase") {
                state.passphrase_needed = true;
                state.passphrase_mode = crate::gui::PassphraseMode::SetNew;
                state.passphrase_status.clear();
            }
        } else {
            ui.label(RichText::new("No private key loaded.")
                .color(theme.text_muted()).size(theme.font_size_small));
        }
    });

    // ── v0.278.0: Auto-unlock on app launch ────────────────────────────
    // Three opt-in modes that coexist alongside the always-available
    // passphrase. UI mirrors the auto_unlock::AutoUnlockMode enum.
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Unlock on App Launch").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(
            "Skip typing your passphrase every launch. Your passphrase remains the recovery option in all modes, these are just shortcuts.")
            .color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_sm);

        let current = state.auto_unlock_mode;
        use crate::auto_unlock::AutoUnlockMode;

        // Mode 1: Always prompt
        let mut sel_always = current == AutoUnlockMode::AlwaysPrompt;
        if ui.radio_value(&mut sel_always, true, "Always ask for passphrase").changed() && sel_always {
            // Switching INTO AlwaysPrompt: clear keychain entries + PIN
            // blob so we don't leave secrets on disk/keychain the user
            // thinks they revoked.
            let identity = state.profile_public_key.clone();
            if !identity.is_empty() {
                let _ = crate::auto_unlock::keychain_clear(crate::auto_unlock::KeychainSlot::Seed, &identity);
                let _ = crate::auto_unlock::keychain_clear(crate::auto_unlock::KeychainSlot::DeviceKey, &identity);
            }
            state.pin_encrypted_seed.clear();
            state.pin_salt.clear();
            state.auto_unlock_mode = AutoUnlockMode::AlwaysPrompt;
            crate::config::AppConfig::from_gui_state(state).save();
        }
        ui.label(RichText::new("Most secure. Use on shared or public machines.")
            .color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);

        // Mode 2: Remember on this device (Keychain)
        let mut sel_keychain = current == AutoUnlockMode::Keychain;
        let key_locked = state.private_key_bytes.is_none();
        let resp = ui.add_enabled(
            !key_locked, // can only enable Keychain when seed is in memory
            egui::RadioButton::new(sel_keychain, "Remember on this device"),
        );
        if resp.clicked() && !sel_keychain {
            // Switching INTO Keychain. Need the seed in memory; stash it.
            if let Some(ref kb) = state.private_key_bytes {
                if kb.len() == 32 && !state.profile_public_key.is_empty() {
                    let mut seed = [0u8; 32];
                    seed.copy_from_slice(kb);
                    match crate::auto_unlock::keychain_stash(
                        crate::auto_unlock::KeychainSlot::Seed,
                        &state.profile_public_key,
                        &seed,
                    ) {
                        Ok(()) => {
                            // Clear KeychainPin remnants if user switched
                            // from KeychainPin → Keychain
                            let _ = crate::auto_unlock::keychain_clear(
                                crate::auto_unlock::KeychainSlot::DeviceKey,
                                &state.profile_public_key,
                            );
                            state.pin_encrypted_seed.clear();
                            state.pin_salt.clear();
                            state.auto_unlock_mode = AutoUnlockMode::Keychain;
                            crate::config::AppConfig::from_gui_state(state).save();
                            sel_keychain = true;
                        }
                        Err(e) => {
                            log::warn!("Keychain stash failed from Settings: {}", e);
                        }
                    }
                }
            }
        }
        let _ = sel_keychain; // explicit ack the radio bool is now-or-never
        if key_locked && current != AutoUnlockMode::Keychain {
            ui.label(RichText::new("(Unlock with passphrase first to enable.)")
                .color(theme.text_muted()).size(theme.font_size_small));
        } else {
            ui.label(RichText::new(
                "OS keychain (Windows Credential Manager / macOS Keychain) holds your seed. Silent unlock on launch.")
                .color(theme.text_muted()).size(theme.font_size_small));
        }
        ui.add_space(theme.spacing_xs);

        // Mode 3: Quick PIN
        let mut sel_pin = current == AutoUnlockMode::KeychainPin;
        let resp_pin = ui.add_enabled(
            !key_locked,
            egui::RadioButton::new(sel_pin, "Quick PIN (4-12 digits)"),
        );
        if resp_pin.clicked() && !sel_pin {
            // Switching INTO KeychainPin: open the PinSetup modal so
            // the user can pick a PIN. Mode flips only after a
            // successful setup (the modal's "Set PIN" handler).
            state.passphrase_needed = true;
            state.passphrase_mode = crate::gui::PassphraseMode::PinSetup;
            state.pin_status.clear();
            state.pin_input.clear();
            state.pin_confirm.clear();
        }
        let _ = sel_pin;
        if key_locked && current != AutoUnlockMode::KeychainPin {
            ui.label(RichText::new("(Unlock with passphrase first to enable.)")
                .color(theme.text_muted()).size(theme.font_size_small));
        } else {
            ui.label(RichText::new(
                "Small barrier against opportunistic OS-account access. PIN protects a device key kept in the OS keychain.")
                .color(theme.text_muted()).size(theme.font_size_small));
        }

        ui.add_space(theme.spacing_sm);

        // PIN management buttons, only relevant in KeychainPin mode
        if current == AutoUnlockMode::KeychainPin {
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, theme, "Change PIN") {
                    state.passphrase_needed = true;
                    state.passphrase_mode = crate::gui::PassphraseMode::PinChange;
                    state.pin_status.clear();
                    state.pin_old_input.clear();
                    state.pin_input.clear();
                    state.pin_confirm.clear();
                }
            });
        }
    });

    ui.add_space(theme.spacing_md);

    // Donation Addresses section (admin/owner)
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Donation Addresses").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Configure donation addresses shown on the Donate page. Supports any cryptocurrency or URL.")
            .color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_sm);

        // Legacy fields (kept for backward compatibility)
        widgets::form_row(ui, theme, "Solana (SOL)", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.donate_solana_address)
                .desired_width(280.0)
                .hint_text("Base58 Solana address"));
        });

        widgets::form_row(ui, theme, "Bitcoin (BTC)", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.donate_btc_address)
                .desired_width(280.0)
                .hint_text("Bitcoin address (bc1...)"));
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // Dynamic addresses list
        ui.label(RichText::new("Additional Addresses").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        let mut remove_idx: Option<usize> = None;
        let mut swap_up_idx: Option<usize> = None;

        for (i, addr) in state.donate_addresses.iter_mut().enumerate() {
            let frame = egui::Frame::none()
                .fill(theme.bg_secondary())
                .rounding(egui::Rounding::same(4))
                .inner_margin(8.0);

            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}.", i + 1)).color(theme.text_muted()).size(theme.font_size_small));

                    ui.vertical(|ui| {
                        widgets::form_row(ui, theme, "Network", |ui| {
                            ui.add(egui::TextEdit::singleline(&mut addr.network)
                                .desired_width(150.0)
                                .hint_text("e.g. Ethereum (ETH)"));
                        });
                        widgets::form_row(ui, theme, "Value", |ui| {
                            ui.add(egui::TextEdit::singleline(&mut addr.value)
                                .desired_width(250.0)
                                .hint_text("Address or URL"));
                        });
                        widgets::form_row(ui, theme, "Label", |ui| {
                            ui.add(egui::TextEdit::singleline(&mut addr.label)
                                .desired_width(200.0)
                                .hint_text("Short description"));
                        });
                        widgets::form_row(ui, theme, "Type", |ui| {
                            egui::ComboBox::from_id_salt(format!("donate_type_{}", i))
                                .selected_text(&addr.addr_type)
                                .width(100.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut addr.addr_type, "address".into(), "address");
                                    ui.selectable_value(&mut addr.addr_type, "url".into(), "url");
                                });
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").clicked() {
                            remove_idx = Some(i);
                        }
                        if i > 0 && ui.small_button("Up").clicked() {
                            swap_up_idx = Some(i);
                        }
                    });
                });
            });
            ui.add_space(theme.section_gap);
        }

        // Process removals and reordering
        if let Some(idx) = remove_idx {
            state.donate_addresses.remove(idx);
        }
        if let Some(idx) = swap_up_idx {
            state.donate_addresses.swap(idx, idx - 1);
        }

        ui.add_space(theme.spacing_sm);

        // Add new address form
        ui.label(RichText::new("Add Address").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
        widgets::form_row(ui, theme, "Network", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.donate_new_network)
                .desired_width(150.0)
                .hint_text("e.g. Monero (XMR)"));
        });
        widgets::form_row(ui, theme, "Value", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.donate_new_value)
                .desired_width(250.0)
                .hint_text("Address or URL"));
        });
        widgets::form_row(ui, theme, "Label", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.donate_new_label)
                .desired_width(200.0)
                .hint_text("Short description"));
        });
        widgets::form_row(ui, theme, "Type", |ui| {
            egui::ComboBox::from_id_salt("donate_new_type")
                .selected_text(&state.donate_new_type)
                .width(100.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.donate_new_type, "address".into(), "address");
                    ui.selectable_value(&mut state.donate_new_type, "url".into(), "url");
                });
        });

        ui.add_space(theme.spacing_xs);
        if widgets::secondary_button(ui, theme, "Add Address") && !state.donate_new_network.is_empty() {
            state.donate_addresses.push(crate::gui::DonateAddress {
                network: state.donate_new_network.clone(),
                addr_type: state.donate_new_type.clone(),
                value: state.donate_new_value.clone(),
                label: state.donate_new_label.clone(),
            });
            state.donate_new_network.clear();
            state.donate_new_value.clear();
            state.donate_new_label.clear();
            state.donate_new_type = "address".into();
        }

        ui.add_space(theme.spacing_sm);

        if widgets::secondary_button(ui, theme, "Save All Addresses") {
            crate::config::AppConfig::from_gui_state(state).save();
        }
    });
}

pub(crate) fn draw_appearance_content(ui: &mut egui::Ui, theme: &mut Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        if widgets::toggle(ui, theme, "Dark Mode", &mut state.settings.dark_mode) {
            state.settings_dirty = true;
        }

        ui.add_space(theme.spacing_sm);

        if widgets::labeled_slider(ui, theme, "Font Size", &mut state.settings.font_size, 10.0..=24.0) {
            state.settings_dirty = true;
        }

        ui.add_space(theme.spacing_sm);

        // Chat timestamp display format (operator request). Applies app-wide and
        // instantly — re-formats already-shown messages too. All UTC.
        widgets::form_row(ui, theme, "Timestamp format", |ui| {
            let mut current = crate::gui::pages::chat::timestamp_format();
            let before = current;
            egui::ComboBox::from_id_salt("timestamp_format_combo")
                .selected_text(current.label())
                .width(320.0)
                .show_ui(ui, |ui| {
                    for fmt in crate::gui::pages::chat::TimestampFormat::ALL {
                        ui.selectable_value(&mut current, fmt, fmt.label());
                    }
                });
            if current != before {
                crate::gui::pages::chat::set_timestamp_format(current);
                crate::config::AppConfig::from_gui_state(state).save();
                // Re-format already-rendered messages so the change is instant.
                for m in state.chat_messages.iter_mut() {
                    if m.timestamp_ms > 0 {
                        m.timestamp = crate::gui::pages::chat::format_timestamp(m.timestamp_ms);
                    }
                }
            }
        });
    });

    ui.add_space(theme.spacing_md);

    // ── Theme Colors: live-edit every color token; saves to data/gui/theme.ron ──
    let mut any_color_changed = false;
    let card_bg = theme.bg_card();
    let card_border = theme.border();
    let card_radius = theme.border_radius;
    let card_padding = theme.card_padding;
    let label_color = theme.text_secondary();
    let header_color = theme.text_primary();
    let heading_size = theme.font_size_heading;

    egui::Frame::none()
        .fill(card_bg)
        .rounding(Rounding::same(card_radius as u8))
        .inner_margin(card_padding)
        .stroke(Stroke::new(1.0, card_border))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Theme Colors")
                    .size(heading_size)
                    .color(header_color)
                    .strong(),
            );
            ui.label(
                RichText::new(
                    "Live-edit every color token. Click a swatch to open the picker. \
                     Changes apply instantly across every page; click Save to persist.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            ui.columns(2, |cols| {
                let labels_left = [
                    ("Background (primary)", &mut theme.bg_primary as *mut _),
                    ("Background (secondary)", &mut theme.bg_secondary as *mut _),
                    ("Background (tertiary)", &mut theme.bg_tertiary as *mut _),
                    ("Background (card)", &mut theme.bg_card as *mut _),
                    ("Row stripe (odd rows)", &mut theme.row_stripe as *mut _),
                    ("Background (modal overlay)", &mut theme.bg_modal as *mut _),
                    ("Accent", &mut theme.accent as *mut _),
                    ("Accent (hover)", &mut theme.accent_hover as *mut _),
                    ("Accent (pressed)", &mut theme.accent_pressed as *mut _),
                    ("Text on accent", &mut theme.text_on_accent as *mut _),
                    ("Border", &mut theme.border as *mut _),
                    ("Border (focus)", &mut theme.border_focus as *mut _),
                ];
                for (label, ptr) in labels_left {
                    let ui_l = &mut cols[0];
                    // SAFETY: we hold &mut theme; the pointer is valid for the
                    // duration of this scope and not aliased.
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_l, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
                let labels_right = [
                    ("Text (primary)", &mut theme.text_primary as *mut _),
                    ("Text (secondary)", &mut theme.text_secondary as *mut _),
                    ("Text (muted)", &mut theme.text_muted as *mut _),
                    ("Success", &mut theme.success as *mut _),
                    ("Warning", &mut theme.warning as *mut _),
                    ("Danger", &mut theme.danger as *mut _),
                    ("Info", &mut theme.info as *mut _),
                    ("Sky: orbit lines", &mut theme.orbit_line as *mut _),
                    ("Sky: constellation lines", &mut theme.constellation_line as *mut _),
                    ("Badge: admin", &mut theme.badge_admin as *mut _),
                    ("Badge: mod", &mut theme.badge_mod as *mut _),
                    ("Badge: verified", &mut theme.badge_verified as *mut _),
                ];
                for (label, ptr) in labels_right {
                    let ui_r = &mut cols[1];
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_r, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
            });

            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Panel & chat-section colors")
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new("Tints for the side panels and the DM/Group/Server lanes in the chat 3-panel layout.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            ui.columns(2, |cols| {
                let labels_left = [
                    ("Panel background",          &mut theme.bg_panel as *mut _),
                    ("Sidebar background",        &mut theme.bg_sidebar as *mut _),
                    ("Sidebar (dark)",            &mut theme.bg_sidebar_dark as *mut _),
                    ("DM lane background",        &mut theme.dm_bg as *mut _),
                    ("DM row background",         &mut theme.dm_row_bg as *mut _),
                    ("DM row (hover)",            &mut theme.dm_row_hover as *mut _),
                    ("Group lane background",     &mut theme.group_bg as *mut _),
                    ("Group row background",      &mut theme.group_row_bg as *mut _),
                ];
                for (label, ptr) in labels_left {
                    let ui_l = &mut cols[0];
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_l, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
                let labels_right = [
                    ("Group row (hover)",         &mut theme.group_row_hover as *mut _),
                    ("Server lane background",    &mut theme.server_bg as *mut _),
                    ("Server row background",     &mut theme.server_row_bg as *mut _),
                    ("Server row (hover)",        &mut theme.server_row_hover as *mut _),
                    ("Slider track",              &mut theme.slider_track as *mut _),
                    ("Badge: donor",              &mut theme.badge_donor as *mut _),
                    ("Badge: live",               &mut theme.badge_live as *mut _),
                ];
                for (label, ptr) in labels_right {
                    let ui_r = &mut cols[1];
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_r, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
            });

            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Nav category colors")
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new("Top-tier categories in the two-tier nav (Reality / Sim / Tools / Settings) and the legacy single-row nav groups (red / green / blue).")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            ui.columns(2, |cols| {
                let labels_left = [
                    ("Nav: Reality (red)",   &mut theme.nav_reality as *mut _),
                    ("Nav: Sim (purple)",    &mut theme.nav_sim as *mut _),
                    ("Nav: Tools (blue)",    &mut theme.nav_tools as *mut _),
                    ("Nav: Settings (gray)", &mut theme.nav_settings as *mut _),
                    ("Nav: Dev (amber)",     &mut theme.nav_dev as *mut _),
                ];
                for (label, ptr) in labels_left {
                    let ui_l = &mut cols[0];
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_l, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
                let labels_right = [
                    ("Nav legacy: red group",   &mut theme.nav_legacy_red as *mut _),
                    ("Nav legacy: green group", &mut theme.nav_legacy_green as *mut _),
                    ("Nav legacy: blue group",  &mut theme.nav_legacy_blue as *mut _),
                ];
                for (label, ptr) in labels_right {
                    let ui_r = &mut cols[1];
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_r, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
            });

            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Studio source colors")
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new("Fill colors for each source type on the Broadcasting Studio's Program/Preview canvases, plus the source outline/label, AFK timer, and audio-meter trough.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            ui.columns(2, |cols| {
                let labels_left = [
                    ("Studio: camera source",     &mut theme.studio_source_camera as *mut _),
                    ("Studio: screen source",     &mut theme.studio_source_screen as *mut _),
                    ("Studio: microphone source", &mut theme.studio_source_microphone as *mut _),
                    ("Studio: chat overlay",      &mut theme.studio_source_chat as *mut _),
                    ("Studio: image source",      &mut theme.studio_source_image as *mut _),
                    ("Studio: text source",       &mut theme.studio_source_text as *mut _),
                ];
                for (label, ptr) in labels_left {
                    let ui_l = &mut cols[0];
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_l, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
                let labels_right = [
                    ("Studio: timer source",  &mut theme.studio_source_timer as *mut _),
                    ("Studio: source outline", &mut theme.studio_source_border as *mut _),
                    ("Studio: source label",  &mut theme.studio_source_label as *mut _),
                    ("Studio: AFK timer",     &mut theme.studio_afk as *mut _),
                    ("Studio: meter trough",  &mut theme.studio_meter_bg as *mut _),
                ];
                for (label, ptr) in labels_right {
                    let ui_r = &mut cols[1];
                    let color_tuple = unsafe { &mut *ptr };
                    if color_row(ui_r, label, color_tuple, label_color) {
                        any_color_changed = true;
                    }
                }
            });

            ui.add_space(theme.spacing_md);
            ui.horizontal(|ui| {
                if widgets::Button::primary("Save Theme").show(ui, theme) {
                    theme.save();
                }
                if widgets::Button::secondary("Reset Colors").show(ui, theme) {
                    theme.reset_color_defaults();
                    any_color_changed = true;
                }
            });
        });

    if any_color_changed {
        // Apply visuals immediately so the rest of the UI re-renders with new colors.
        theme.apply_to_egui(ui.ctx());
    }
}

/// One row of the color-picker grid: a swatch button (left) followed by the
/// label (right). Swatches first means they all align in a clean column
/// regardless of label length. Returns true if the color changed.
fn color_row(
    ui: &mut egui::Ui,
    label: &str,
    color_tuple: &mut (f32, f32, f32, f32),
    label_color: Color32,
) -> bool {
    let mut rgba = [color_tuple.0, color_tuple.1, color_tuple.2, color_tuple.3];
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
            color_tuple.0 = rgba[0];
            color_tuple.1 = rgba[1];
            color_tuple.2 = rgba[2];
            color_tuple.3 = rgba[3];
            changed = true;
        }
        ui.add_space(8.0);
        ui.label(RichText::new(label).color(label_color).size(13.0));
    });
    changed
}

/// Animation customization (v0.177.0). Master switch + per-element style
/// pickers (RGB cycle / solid / pulse / off) and speed multipliers.
/// Replaces the formerly-hardcoded RGB-cycle and red-pulse behaviors so
/// users can pick what they want — accessibility users get a "off" option
/// for reduced motion, action gamers can pick yellow-pulse over red, etc.
pub(crate) fn draw_animations_content(ui: &mut egui::Ui, theme: &mut Theme, state: &mut GuiState) {
    use crate::gui::theme::attack as atk;
    let mut changed = false;

    // Snapshot styling values up-front so we can borrow theme mutably
    // for the field references inside the cards.
    let card_bg = theme.bg_card();
    let card_border = theme.border();
    let card_radius = theme.border_radius;
    let card_padding = theme.card_padding;
    let body_size = theme.font_size_body;
    let small_size = theme.font_size_small;
    let xs = theme.spacing_xs;
    let md = theme.spacing_md;
    let text_primary = theme.text_primary();
    let text_muted = theme.text_muted();

    let frame = || {
        egui::Frame::none()
            .fill(card_bg)
            .rounding(Rounding::same(card_radius as u8))
            .inner_margin(card_padding)
            .stroke(Stroke::new(1.0, card_border))
    };

    // Snapshot the editable token values into locals so we can pass
    // `&Theme` (immutable, for styling) and `&mut local` to widgets
    // simultaneously. Write back whatever changed at the end.
    let mut anim_enabled = theme.animations_enabled;
    let mut sep_anim = theme.nav_separator_animation;
    let mut sep_speed = theme.nav_separator_animation_speed;
    let mut border_anim = theme.nav_active_border_animation;
    let mut atk_style = theme.attack_indicator_style;
    let mut atk_speed = theme.attack_indicator_speed;
    let mut dev_visible = theme.nav_dev_visible;
    let mut cheats_on = theme.cheats_enabled;

    // ── Master switch ──
    frame().show(ui, |ui| {
        ui.label(RichText::new("Master switch").size(body_size).color(text_primary).strong());
        ui.label(RichText::new(
            "Off freezes every animation, RGB cycles hold their last frame, \
             attack pulses become a solid danger color. Use for reduced-motion \
             accessibility or to focus while you work."
        ).size(small_size).color(text_muted));
        if widgets::toggle(ui, theme, "Animations enabled", &mut anim_enabled) {
            changed = true;
        }
    });
    ui.add_space(md);

    // ── Nav separator style + speed ──
    frame().show(ui, |ui| {
        ui.label(RichText::new("Nav separator (colored line under the top + sub menus)")
            .size(body_size).color(text_primary).strong());
        ui.add_space(xs);
        if anim_style_picker(ui, theme, "Style", &mut sep_anim) { changed = true; }
        if widgets::labeled_slider(ui, theme, "Speed", &mut sep_speed, 0.0..=3.0) {
            changed = true;
        }
    });
    ui.add_space(md);

    // ── Active button border style ──
    frame().show(ui, |ui| {
        ui.label(RichText::new("Active button border (current page / category highlight)")
            .size(body_size).color(text_primary).strong());
        ui.add_space(xs);
        if anim_style_picker(ui, theme, "Style", &mut border_anim) { changed = true; }
    });
    ui.add_space(md);

    // ── Attack indicator style + speed + test button ──
    frame().show(ui, |ui| {
        ui.label(RichText::new("Attack indicator (in-menu alert when you take damage)")
            .size(body_size).color(text_primary).strong());
        ui.label(RichText::new(
            "Most games only play sound when you're hit while in menus. \
             This gives you a visual too, pick a style."
        ).size(small_size).color(text_muted));
        ui.add_space(xs);
        let attack_options = [
            (atk::NONE,         "None (sound only)"),
            (atk::PULSE_RED,    "Pulse red"),
            (atk::PULSE_YELLOW, "Pulse yellow"),
            (atk::FLASH_WHITE,  "Flash white"),
            (atk::BORDER_ONLY,  "Solid (no motion)"),
        ];
        if u8_radio_picker(ui, theme, "Style", &mut atk_style, &attack_options) {
            changed = true;
        }
        if widgets::labeled_slider(ui, theme, "Speed", &mut atk_speed, 0.0..=3.0) {
            changed = true;
        }
        if widgets::Button::secondary("Test attack pulse for 3s").show(ui, theme) {
            state.attack_pulse_active = true;
            state.attack_pulse_last_hit_at = ui.ctx().input(|i| i.time);
        }
    });

    // ── Developer mode (lives here for borrow-checker geometry; will
    // move to its own Settings → Developer section at v1.0). ──
    frame().show(ui, |ui| {
        ui.label(RichText::new("Developer mode").size(body_size).color(text_primary).strong());
        ui.label(RichText::new(
            "Show the Dev top-tier category in the nav bar (Testing / Bugs / \
             Agents / AI Usage / Files). On by default during the development \
             period; turn off if you want a cleaner production-style nav."
        ).size(small_size).color(text_muted));
        if widgets::toggle(ui, theme, "Show Dev menu", &mut dev_visible) {
            changed = true;
        }
    });

    // ── Developer cheats (the "Dev:" provisioning buttons across the app) ──
    frame().show(ui, |ui| {
        ui.label(RichText::new("Developer cheats").size(body_size).color(text_primary).strong());
        ui.label(RichText::new(
            "Show the in-app cheat buttons (stock all materials, stock seeds, \
             grow all crops, max skills) that let you test every loop instantly. \
             Since the play-mode system (task #50) these ALSO require the Dev \
             play mode (Settings > Gameplay); this switch is the extra \
             kill-switch for a clean demo on a Dev-mode install."
        ).size(small_size).color(text_muted));
        if widgets::toggle(ui, theme, "Enable dev cheats", &mut cheats_on) {
            changed = true;
        }
    });

    // Write back any edits.
    theme.animations_enabled = anim_enabled;
    theme.nav_separator_animation = sep_anim;
    theme.nav_separator_animation_speed = sep_speed;
    theme.nav_active_border_animation = border_anim;
    theme.attack_indicator_style = atk_style;
    theme.attack_indicator_speed = atk_speed;
    theme.nav_dev_visible = dev_visible;
    theme.cheats_enabled = cheats_on;

    // Auto-clear the test attack pulse after 3 seconds.
    if state.attack_pulse_active {
        let now = ui.ctx().input(|i| i.time);
        if now - state.attack_pulse_last_hit_at > 3.0 {
            state.attack_pulse_active = false;
        }
        ui.ctx().request_repaint();
    }

    ui.add_space(md);
    if widgets::Button::primary("Save Animations").show(ui, theme) {
        theme.save();
    }

    if changed {
        state.settings_dirty = true;
    }
}

/// Radio-button-ish picker for the standard nav animation style enum
/// (off / solid / rgb_cycle / pulse). Returns true if value changed.
fn anim_style_picker(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &mut u8) -> bool {
    use crate::gui::theme::anim;
    let options = [
        (anim::OFF,       "Off"),
        (anim::SOLID,     "Solid"),
        (anim::RGB_CYCLE, "RGB cycle"),
        (anim::PULSE,     "Pulse"),
    ];
    u8_radio_picker(ui, theme, label, value, &options)
}

/// Generic horizontal-radio picker for a u8 enum token. Each option is
/// rendered as a small toggle button; clicking sets the value. Returns
/// true if value changed.
fn u8_radio_picker(
    ui: &mut egui::Ui,
    theme: &Theme,
    label: &str,
    value: &mut u8,
    options: &[(u8, &str)],
) -> bool {
    let mut changed = false;
    widgets::settings_row(ui, theme, label, |ui| {
        for (code, name) in options {
            let active = *value == *code;
            if widgets::Button::secondary(*name).active(active).show(ui, theme) {
                if !active {
                    *value = *code;
                    changed = true;
                }
            }
        }
    });
    changed
}

pub(crate) fn draw_widgets_content(ui: &mut egui::Ui, theme: &mut Theme, state: &mut GuiState) {
    // Capture card styling values before mutable borrows
    let card_bg = theme.bg_card();
    let card_border = theme.border();
    let card_radius = theme.border_radius;
    let card_padding = theme.card_padding;
    let spacing_sm = theme.spacing_sm;
    let spacing_md = theme.spacing_md;
    let heading_sz = theme.font_size_heading;

    let label_color = Color32::from_rgb(136, 136, 148);
    let text_color = Color32::from_rgb(232, 232, 234);
    let ss = SliderStyle::from_theme(theme);

    // Two-column layout: sliders on left, live preview on right
    ui.columns(2, |cols| {
        // ── LEFT COLUMN: sliders ──
        let ui = &mut cols[0];
        let mut any_changed = false;

        let make_card = |ui: &mut egui::Ui, title: &str, content: &mut dyn FnMut(&mut egui::Ui)| {
            egui::Frame::none()
                .fill(card_bg)
                .rounding(Rounding::same(card_radius as u8))
                .inner_margin(card_padding)
                .stroke(Stroke::new(1.0, card_border))
                .show(ui, |ui| {
                    ui.label(RichText::new(title).strong().color(text_color));
                    ui.add_space(4.0);
                    content(ui);
                });
        };

        // Sizing card
        make_card(ui, "Sizing", &mut |ui| {
            any_changed |= styled_slider(ui, &ss, "Icon Size", &mut theme.icon_size, 8.0..=64.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Icon Small", &mut theme.icon_small, 8.0..=32.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Avatar Size", &mut theme.avatar_size, 16.0..=64.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Avatar Gap", &mut theme.avatar_gap, 0.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Pill Radius", &mut theme.pill_radius, 0.0..=20.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Row Height", &mut theme.row_height, 12.0..=48.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Header Height", &mut theme.header_height, 16.0..=64.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Button Height", &mut theme.button_height, 16.0..=48.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Input Height", &mut theme.input_height, 16.0..=48.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Status Dot", &mut theme.status_dot_size, 2.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Checkbox Size", &mut theme.checkbox_size, 10.0..=28.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Stat Name Width", &mut theme.stat_name_width, 40.0..=160.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Stat Value Width", &mut theme.stat_value_width, 40.0..=160.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Status Bar Width", &mut theme.status_bar_width, 80.0..=400.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Status Bar Height", &mut theme.status_bar_height, 2.0..=20.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Compact Button Height", &mut theme.compact_button_height, 12.0..=36.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Cell Narrow Width", &mut theme.cell_narrow_width, 30.0..=120.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Cell Short Width", &mut theme.cell_short_width, 40.0..=180.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Cell Name Width", &mut theme.cell_name_width, 80.0..=300.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Sidebar Width", &mut theme.sidebar_width, 150.0..=400.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Settings Label Width", &mut theme.settings_label_width, 100.0..=300.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Modal Width", &mut theme.modal_width, 300.0..=800.0, label_color);
        });
        ui.add_space(spacing_sm);

        // Spacing card
        make_card(ui, "Spacing", &mut |ui| {
            any_changed |= styled_slider(ui, &ss, "Row Gap", &mut theme.row_gap, 0.0..=8.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Section Gap", &mut theme.section_gap, 0.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Item Padding", &mut theme.item_padding, 0.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Panel Margin", &mut theme.panel_margin, 0.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Card Padding", &mut theme.card_padding, 0.0..=32.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Button Padding H", &mut theme.button_padding_h, 0.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Button Padding V", &mut theme.button_pad_y, 0.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Spacing XS", &mut theme.spacing_xs, 0.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Spacing SM", &mut theme.spacing_sm, 0.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Spacing MD", &mut theme.spacing_md, 0.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Spacing LG", &mut theme.spacing_lg, 0.0..=32.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Spacing XL", &mut theme.spacing_xl, 0.0..=48.0, label_color);
        });
        ui.add_space(spacing_sm);

        // Fonts card
        make_card(ui, "Fonts", &mut |ui| {
            any_changed |= styled_slider(ui, &ss, "Small Font", &mut theme.small_size, 8.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Body Font", &mut theme.body_size, 10.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Name Font", &mut theme.name_size, 10.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Heading Size", &mut theme.heading_size, 12.0..=32.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Title Size", &mut theme.title_size, 14.0..=48.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Font Small", &mut theme.font_size_small, 8.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Font Body", &mut theme.font_size_body, 10.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Font Heading", &mut theme.font_size_heading, 12.0..=32.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Font Title", &mut theme.font_size_title, 14.0..=48.0, label_color);
        });
        ui.add_space(spacing_sm);

        // Borders & Radii card
        make_card(ui, "Borders & Radii", &mut |ui| {
            any_changed |= styled_slider(ui, &ss, "Border Width", &mut theme.border_width, 0.0..=4.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Border Radius", &mut theme.border_radius, 0.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Border Radius LG", &mut theme.border_radius_lg, 0.0..=24.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Widget Radius", &mut theme.border_radius_widget, 0.0..=12.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Badge Radius", &mut theme.badge_radius, 0.0..=12.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Badge Pad H", &mut theme.badge_padding_h, 0.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Badge Pad V", &mut theme.badge_padding_v, 0.0..=8.0, label_color);
        });
        ui.add_space(spacing_sm);

        // Slider & Checkbox card
        make_card(ui, "Controls", &mut |ui| {
            any_changed |= styled_slider(ui, &ss, "Slider Track H", &mut theme.slider_track_height, 1.0..=12.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Slider Thumb R", &mut theme.slider_thumb_radius, 3.0..=16.0, label_color);
        });
        ui.add_space(spacing_sm);

        // Nav card (v0.176.0): RGB separator height + active/hover border
        // widths used by the two-tier and legacy nav. Editing these
        // re-styles the nav immediately.
        make_card(ui, "Nav", &mut |ui| {
            any_changed |= styled_slider(ui, &ss, "Separator Height", &mut theme.nav_separator_height, 1.0..=10.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Active Border Width", &mut theme.nav_active_border_width, 1.0..=5.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Hover Border Width", &mut theme.nav_hover_border_width, 1.0..=5.0, label_color);
        });

        ui.add_space(spacing_sm);

        // Save / Reset buttons
        ui.horizontal(|ui| {
            if widgets::primary_button(ui, theme, "Save Theme") {
                theme.save();
            }
            if widgets::secondary_button(ui, theme, "Reset to Defaults") {
                theme.reset_widget_defaults();
                any_changed = true;
            }
        });

        if any_changed {
            state.settings_dirty = true;
        }

        // ── RIGHT COLUMN: live preview ──
        let ui = &mut cols[1];

        egui::Frame::none()
            .fill(theme.bg_panel())
            .rounding(Rounding::same(4))
            .inner_margin(8.0)
            .stroke(Stroke::new(1.0, Color32::from_rgb(42, 42, 53)))
            .show(ui, |ui| {
                ui.label(RichText::new("Live Preview").size(heading_sz).color(text_color));
                ui.add_space(spacing_sm);

                // Sample message row (uses actual widget)
                ui.label(RichText::new("Message Row").size(theme.small_size).color(label_color).strong());
                ui.add_space(theme.row_gap);
                crate::gui::widgets::row::message_row(
                    ui,
                    theme,
                    'A',
                    Color32::from_rgb(52, 152, 219),
                    "Alice",
                    "12:34 PM",
                    "This is a sample message to preview how the row widget looks with the current theme settings.",
                    true,
                    Color32::from_rgb(26, 26, 34),
                    false,
                    0.0,
                    0.0, // pill_width = 0 → preview keeps inline timestamp
                    &[], // no mention highlighting in the theme preview
                &[], // no markdown/link spans in the theme preview
                );
                ui.add_space(theme.section_gap);
                // Continuation row
                crate::gui::widgets::row::message_row(
                    ui,
                    theme,
                    'A',
                    Color32::from_rgb(52, 152, 219),
                    "Alice",
                    "",
                    "A continuation message from the same user.",
                    false,
                    Color32::from_rgb(30, 30, 38),
                    false,
                    0.0,
                    0.0, // pill_width = 0
                    &[], // no mention highlighting in the theme preview
                &[], // no markdown/link spans in the theme preview
                );

                ui.add_space(spacing_md);

                // Sample channel list item
                ui.label(RichText::new("Channel List Item").size(theme.small_size).color(label_color).strong());
                ui.add_space(theme.row_gap);
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), theme.row_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let full_rect = ui.max_rect();
                        let hover = ui.rect_contains_pointer(full_rect);
                        let fill = if hover {
                            Color32::from_rgb(35, 35, 50)
                        } else {
                            Color32::from_rgb(20, 20, 55)
                        };
                        ui.painter().rect_filled(full_rect, 0.0, fill);
                        ui.add_space(theme.item_padding * 2.0);
                        ui.label(
                            RichText::new("# general")
                                .size(theme.body_size)
                                .color(text_color),
                        );
                    },
                );

                ui.add_space(spacing_md);

                // Sample user list items
                ui.label(RichText::new("User List Item").size(theme.small_size).color(label_color).strong());
                ui.add_space(theme.row_gap);
                ui.horizontal(|ui| {
                    ui.add_space(theme.item_padding);
                    let dot_sz = theme.status_dot_size;
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, Color32::from_rgb(51, 191, 77));
                    ui.label(
                        RichText::new("Bob")
                            .size(theme.body_size)
                            .color(text_color),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_space(theme.item_padding);
                    let dot_sz = theme.status_dot_size;
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, Color32::from_rgb(100, 100, 100));
                    ui.label(
                        RichText::new("Charlie")
                            .size(theme.body_size)
                            .color(Color32::from_rgb(106, 106, 117)),
                    );
                });
            });
    });
}

pub(crate) fn draw_notifications_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        widgets::toggle(ui, theme, "Direct Messages", &mut state.settings.notify_dm);
        widgets::toggle(ui, theme, "Mentions", &mut state.settings.notify_mentions);
        widgets::toggle(ui, theme, "Task Updates", &mut state.settings.notify_tasks);

        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Do Not Disturb").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        widgets::form_row(ui, theme, "Quiet hours start", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.settings.dnd_start)
                .desired_width(80.0)
                .hint_text("22:00"));
        });
        widgets::form_row(ui, theme, "Quiet hours end", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.settings.dnd_end)
                .desired_width(80.0)
                .hint_text("08:00"));
        });
    });
}

pub(crate) fn draw_wallet_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        // Solana address
        ui.horizontal(|ui| {
            ui.label(RichText::new("Solana Address:").color(theme.text_secondary()));
            let addr = if state.wallet_address.is_empty() {
                "Not generated".to_string()
            } else if state.wallet_address.len() > 12 {
                format!("{}...{}", &state.wallet_address[..6], &state.wallet_address[state.wallet_address.len()-6..])
            } else {
                state.wallet_address.clone()
            };
            ui.label(RichText::new(&addr).color(theme.text_muted()).size(theme.font_size_small));
            if !state.wallet_address.is_empty() {
                if widgets::secondary_button(ui, theme, "Copy") {
                    ui.ctx().copy_text(state.wallet_address.clone());
                }
            }
        });

        ui.add_space(theme.spacing_md);

        // Network selector
        ui.label(RichText::new("Network").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        let mut net = state.settings.wallet_network;
        let mut changed = false;
        ui.horizontal(|ui| {
            for n in [WalletNetwork::Mainnet, WalletNetwork::Devnet, WalletNetwork::Testnet] {
                let is_sel = net == n;
                let text_color = if is_sel { theme.text_on_accent() } else { theme.text_secondary() };
                let fill = if is_sel { theme.accent() } else { Color32::TRANSPARENT };
                let btn = egui::Button::new(RichText::new(n.label()).color(text_color).size(theme.font_size_body))
                    .fill(fill)
                    .rounding(Rounding::same(4));
                if ui.add(btn).clicked() && !is_sel {
                    net = n;
                    changed = true;
                }
            }
        });
        if changed {
            state.settings.wallet_network = net;
            state.settings_dirty = true;
        }

        ui.add_space(theme.spacing_md);

        widgets::form_row(ui, theme, "Custom RPC URL", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.settings.custom_rpc_url)
                .desired_width(280.0)
                .hint_text("https://..."));
        });
    });
}

pub(crate) fn draw_audio_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        if widgets::labeled_slider(ui, theme, "Master Volume", &mut state.settings.master_volume, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        if widgets::labeled_slider(ui, theme, "Music Volume", &mut state.settings.music_volume, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        if widgets::labeled_slider(ui, theme, "SFX Volume", &mut state.settings.sfx_volume, 0.0..=1.0) {
            state.settings_dirty = true;
        }
    });
    // Voice (v0.485). Device selectors + a mic loopback test (toggle) with a live
    // level meter, so you can confirm capture + playback and pick devices. The
    // full in-app voice transport is being built in phases.
    widgets::card(ui, theme, |ui| {
        widgets::section_header(ui, theme, "Voice");
        // Enumerate audio devices once (cpal enumeration is slow, and the active
        // test repaints at 60fps, so never enumerate per frame). Refresh on demand.
        if !state.audio_devices_loaded {
            state.audio_input_devices = crate::net::voice::list_input_devices();
            state.audio_output_devices = crate::net::voice::list_output_devices();
            state.audio_devices_loaded = true;
        }
        let in_devs = state.audio_input_devices.clone();
        let out_devs = state.audio_output_devices.clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Input (microphone)").size(theme.font_size_small).color(theme.text_secondary()));
            egui::ComboBox::from_id_salt("audio_in_dev")
                .selected_text(if state.audio_input_device.is_empty() { "System default".to_string() } else { state.audio_input_device.clone() })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.audio_input_device, String::new(), "System default");
                    for d in &in_devs {
                        ui.selectable_value(&mut state.audio_input_device, d.clone(), d);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Output (speakers)").size(theme.font_size_small).color(theme.text_secondary()));
            egui::ComboBox::from_id_salt("audio_out_dev")
                .selected_text(if state.audio_output_device.is_empty() { "System default".to_string() } else { state.audio_output_device.clone() })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.audio_output_device, String::new(), "System default");
                    for d in &out_devs {
                        ui.selectable_value(&mut state.audio_output_device, d.clone(), d);
                    }
                });
        });
        if widgets::Button::ghost("Refresh devices").show(ui, theme) {
            state.audio_devices_loaded = false;
        }

        // ── Input processing (v0.488): gain, noise filter, transmit mode ──
        ui.add_space(theme.spacing_sm);
        // Mic gain, 0-200% (100% = unchanged). Stored as a 0.0..=2.0 multiplier.
        let mut gain_pct = state.voice_gain * 100.0;
        if widgets::labeled_slider(ui, theme, "Mic gain %", &mut gain_pct, 0.0..=200.0) {
            state.voice_gain = (gain_pct / 100.0).clamp(0.0, 2.0);
            state.settings_dirty = true;
        }

        // Noise filter mode.
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Noise filter").size(theme.font_size_small).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            for m in crate::config::VoiceFilterMode::ALL {
                let selected = state.voice_filter_mode == m;
                if ui.selectable_label(selected, m.label()).clicked() && !selected {
                    state.voice_filter_mode = m;
                    state.settings_dirty = true;
                }
            }
        });
        ui.label(RichText::new(state.voice_filter_mode.hint()).size(theme.font_size_small).color(theme.text_muted()));

        // Transmit mode.
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Transmit mode").size(theme.font_size_small).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            for m in crate::config::VoiceTransmitMode::ALL {
                let selected = state.voice_transmit_mode == m;
                if ui.selectable_label(selected, m.label()).clicked() && !selected {
                    state.voice_transmit_mode = m;
                    state.settings_dirty = true;
                }
            }
        });
        ui.label(RichText::new(state.voice_transmit_mode.hint()).size(theme.font_size_small).color(theme.text_muted()));

        // Push key binding (push-to-talk / push-to-mute only). The actual key
        // capture happens in the raw winit handler (so it can bind CapsLock and
        // any key, and read them in-game); clicking here just arms it.
        if state.voice_transmit_mode.uses_key() {
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Push key").size(theme.font_size_small).color(theme.text_secondary()));
                let lbl = if state.voice_binding_key {
                    "Press any key (Esc cancels)...".to_string()
                } else if state.voice_ptt_key.is_empty() {
                    "Unbound".to_string()
                } else {
                    crate::config::pretty_ptt_key_name(&state.voice_ptt_key)
                };
                if ui.selectable_label(state.voice_binding_key, lbl).clicked() {
                    state.voice_binding_key = !state.voice_binding_key;
                }
            });
            if state.voice_ptt_key == "CapsLock" {
                ui.label(
                    RichText::new("Heads up: CapsLock also toggles caps each push. Rebind if that bothers you.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
        }

        // Activation threshold (voice-activated only). Stored 0.0..=1.0, shown as %.
        if state.voice_transmit_mode == crate::config::VoiceTransmitMode::VoiceActivated {
            let mut vad_pct = state.voice_vad_threshold * 100.0;
            if widgets::labeled_slider(ui, theme, "Activation threshold %", &mut vad_pct, 0.0..=30.0) {
                state.voice_vad_threshold = (vad_pct / 100.0).clamp(0.0, 1.0);
                state.settings_dirty = true;
            }
        }

        ui.add_space(theme.spacing_sm);
        widgets::body_hint(
            ui, theme,
            "Test microphone plays your own mic back to you so you can confirm capture and \
             playback (use headphones to avoid feedback). It stays on until you stop it. Gain, \
             filter, and transmit mode all apply live while the test runs.",
        );
        ui.add_space(theme.spacing_xs);

        // Toggle button. While active it gets an animated RGB border (same channeling
        // color as the nav) so it is unmistakably live, and the section repaints so
        // the meter stays live.
        let active = state.mic_test_active;
        let label = if active { "Stop test" } else { "Test microphone" };
        let btn = ui.horizontal(|ui| widgets::Button::secondary(label).active(active).show(ui, theme));
        if active {
            let time = ui.ctx().input(|i| i.time) as f32;
            let col = crate::gui::pages::escape_menu::channeling_color(theme, time, false, theme.accent());
            ui.painter().rect_stroke(
                btn.response.rect.expand(2.0),
                egui::Rounding::same(theme.border_radius as u8),
                egui::Stroke::new(2.0, col),
                egui::StrokeKind::Outside,
            );
            ui.ctx().request_repaint();
        }
        if btn.inner {
            state.mic_test_active = !state.mic_test_active;
        }

        // Live mic level meter + status.
        ui.add_space(theme.spacing_xs);
        let lvl = state.mic_meter.clamp(0.0, 1.0);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(280.0), 10.0),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, egui::Rounding::same(2), theme.bg_card());
        if lvl > 0.001 {
            let fill = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * lvl, rect.height()));
            let col = if lvl > 0.7 { theme.danger() } else { theme.success() };
            ui.painter().rect_filled(fill, egui::Rounding::same(2), col);
        }
        let status = crate::net::voice::mic_status();
        if !status.is_empty() {
            ui.label(RichText::new(status).size(theme.font_size_small).color(theme.text_secondary()));
        }
        // While the test runs, show whether the transmit gate is open right now,
        // so push-to-talk / voice-activated modes are visibly working.
        if state.mic_test_active {
            let (txt, col) = if crate::net::voice::is_transmitting() {
                ("Transmitting", theme.success())
            } else {
                ("Silent (transmit gate closed)", theme.text_muted())
            };
            ui.label(RichText::new(txt).size(theme.font_size_small).color(col));
        }
    });
}

pub(crate) fn draw_graphics_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        // Window presentation mode (v0.454). Default = Windowed fullscreen (maximized, title
        // bar + taskbar still visible). Selecting a mode applies it immediately.
        ui.label(RichText::new("Window mode").color(theme.text_secondary()).strong());
        ui.horizontal_wrapped(|ui| {
            for mode in crate::config::WindowMode::ALL {
                let selected = state.settings.window_mode == mode;
                if ui.selectable_label(selected, mode.label()).clicked() && !selected {
                    state.settings.window_mode = mode;
                    state.settings_dirty = true;
                }
            }
        });
        ui.label(RichText::new("Windowed fullscreen keeps the title bar + taskbar. Borderless drops the title bar. Exclusive is true fullscreen.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_sm);
        if widgets::toggle(ui, theme, "VSync", &mut state.settings.vsync) {
            state.settings_dirty = true;
        }
        if widgets::labeled_slider(ui, theme, "FOV", &mut state.settings.fov, 60.0..=120.0) {
            state.settings_dirty = true;
        }
        if widgets::labeled_slider(ui, theme, "Render Distance", &mut state.settings.render_distance, 50.0..=2000.0) {
            state.settings_dirty = true;
        }

        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Planets").color(theme.text_secondary()).strong());
        ui.label(RichText::new("Sky planets subdivide as they grow on screen: one more detail level each time a body's projected size doubles past the pixel threshold. Changes apply live.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
        // Procedural fractal surfaces (oceans, continents, polar caps) vs the
        // old flat single-color spheres. Data lives in data/planets/<id>.ron.
        if widgets::toggle(ui, theme, "Procedural surfaces", &mut state.settings.planet_detail) {
            state.settings_dirty = true;
        }
        if widgets::labeled_slider(ui, theme, "LOD pixel threshold", &mut state.settings.planet_lod_px, 4.0..=64.0) {
            state.settings_dirty = true;
        }
        // Ceiling raised 7 -> 9 (2026-07-11) for FTL close approaches; the
        // top levels only trigger when one planet fills the screen (see
        // terrain::planet::MAX_SKY_SUBDIVISION for the face/memory table).
        if widgets::labeled_slider(ui, theme, "Max subdivision level", &mut state.settings.planet_max_subdiv, 0.0..=9.0) {
            state.settings_dirty = true;
        }
        ui.label(RichText::new("Levels 8-9 add real close-range detail but build big meshes; lower this if a close planet flyby stutters.").color(theme.text_muted()).size(theme.font_size_small));
        // Chunked planetary LOD (2026-07-11): quadtree surface patches that
        // follow the camera once a planet fills the screen, replacing the
        // heavy uniform level 8-9 spheres near heightmap planets (Earth).
        if widgets::toggle(ui, theme, "Chunked surface detail", &mut state.settings.planet_chunked) {
            state.settings_dirty = true;
        }
        ui.label(RichText::new("Near a planet with real elevation data, surface detail streams in around the camera (~54 m triangles) instead of remeshing the whole globe. Turn off to fall back to uniform spheres.").color(theme.text_muted()).size(theme.font_size_small));
        // Analytic scattering atmosphere (v0.807): per-pixel single
        // scattering on the planet air shells. Off = the pre-v0.807 fresnel
        // tint, kept forever-dev style as the A/B reference + a safety hatch
        // for GPUs that dislike the math. Applies live: the material is
        // rebuilt (cached per mode) the next time the shell draws.
        if widgets::toggle(ui, theme, "Scattering atmosphere", &mut state.settings.planet_atmo_scatter) {
            state.settings_dirty = true;
        }
        ui.label(RichText::new("Physically shaded planet air: blue limb from orbit, warm terminator, pale horizon from inside the atmosphere. Turn off for the simple tinted-shell look.").color(theme.text_muted()).size(theme.font_size_small));
        // Animated procedural cloud deck (clouds increment 1): a second
        // translucent shell under the atmosphere, on planets whose RON
        // declares cloud_coverage. Applies live: off skips the draw next
        // frame; on reuses the cached material.
        if widgets::toggle(ui, theme, "Cloud layer", &mut state.settings.planet_clouds) {
            state.settings_dirty = true;
        }
        ui.label(RichText::new("Drifting sun-lit clouds on worlds that have them (Earth). Turn off for bare surfaces or on very old GPUs.").color(theme.text_muted()).size(theme.font_size_small));
        // Close-range surface detail (v0.816): animated ocean waves + land
        // micro-texture on planets with real imagery. Applies live: the sky
        // loop rewrites the material flag every frame.
        if widgets::toggle(ui, theme, "Surface detail", &mut state.settings.planet_surface_detail) {
            state.settings_dirty = true;
        }
        ui.label(RichText::new("Up close, oceans get moving waves and sun sparkle and land keeps revealing texture as you descend. The view from orbit is identical either way. Turn off on very old GPUs.").color(theme.text_muted()).size(theme.font_size_small));
        // Cloud quality ladder (clouds increment 3). Applies live: the cloud
        // material is cached per (body, quality), so flipping tiers rebuilds
        // it the next frame the deck draws.
        if state.settings.planet_clouds {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Cloud quality").color(theme.text_secondary()));
                for (val, label) in [
                    ("low", "Low"),
                    ("medium", "Medium"),
                    ("high", "High"),
                ] {
                    let sel = state.settings.cloud_quality == val;
                    if ui.selectable_label(sel, RichText::new(label).size(theme.font_size_small)).clicked() && !sel {
                        state.settings.cloud_quality = val.to_string();
                        state.settings_dirty = true;
                    }
                }
            });
            ui.label(RichText::new("High raymarches real 3D cloud shapes with sunlight scattering (puffy towers, dark bases). Medium is the lighter layered march; Low is a flat painted deck for weak GPUs.").color(theme.text_muted()).size(theme.font_size_small));
        }

        // ── Sky / map lines (v0.786, operator sky settings) ──
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Sky / map lines").color(theme.text_secondary()).strong());
        ui.label(
            RichText::new(
                "The line overlays in the night sky. Colors live in Appearance \
                 (Sky: orbit lines / Sky: constellation lines). Vessel orbits, \
                 collision-course flags, and selected-object modes arrive as \
                 those systems come online.",
            )
            .color(theme.text_muted())
            .size(theme.font_size_small),
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Orbit rings").color(theme.text_secondary()));
            for (val, label) in [
                ("off", "Off"),
                ("planets", "Planets"),
                ("planets_moons", "Planets + moons"),
            ] {
                let sel = state.settings.sky_orbit_mode == val;
                if ui.selectable_label(sel, RichText::new(label).size(theme.font_size_small)).clicked() && !sel {
                    state.settings.sky_orbit_mode = val.to_string();
                    state.settings_dirty = true;
                }
            }
        });
        if widgets::toggle(ui, theme, "Constellation figures", &mut state.settings.sky_constellations) {
            state.settings_dirty = true;
        }
        // Milky Way glow (2026-07-10): the baked all-sky texture of real
        // integrated catalog starlight (data/galaxy_glow.png), drawn behind
        // the star points. Both controls apply live: the toggle skips the
        // render pass, the intensity is a shader uniform.
        if widgets::toggle(ui, theme, "Milky Way glow", &mut state.settings.sky_milkyway_glow) {
            state.settings_dirty = true;
        }
        if state.settings.sky_milkyway_glow {
            if widgets::labeled_slider(ui, theme, "Glow intensity", &mut state.settings.sky_milkyway_intensity, 0.0..=2.0) {
                state.settings_dirty = true;
            }
        }
        ui.label(
            RichText::new("The galaxy band baked from the real star catalog's integrated light. Changes apply live.")
                .color(theme.text_muted())
                .size(theme.font_size_small),
        );
        // Glow texture tier (2026-07-11): Standard ships with the app; Ultra
        // is a one-time download fetched exactly like the star catalog tiers
        // below (background thread, progress bar, retry on FAILED). The
        // chooser only offers Ultra once the file is installed; while absent
        // the Download button stands in for it, and finishing a download
        // selects Ultra automatically.
        if state.settings.sky_milkyway_glow {
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Glow texture:")
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                );
                let std_sel = state.settings.sky_glow_tier != "ultra";
                if ui
                    .selectable_label(
                        std_sel,
                        RichText::new("Standard (8192, included)").size(theme.font_size_small),
                    )
                    .clicked()
                    && !std_sel
                {
                    state.settings.sky_glow_tier = "standard".to_string();
                    state.settings_dirty = true;
                }
                match state.galaxy_glow_installed {
                    Some(bytes) => {
                        let ultra_sel = state.settings.sky_glow_tier == "ultra";
                        if ui
                            .selectable_label(
                                ultra_sel,
                                RichText::new(format!(
                                    "Ultra (16384, {} MB installed)",
                                    bytes / 1_048_576
                                ))
                                .size(theme.font_size_small),
                            )
                            .clicked()
                            && !ultra_sel
                        {
                            state.settings.sky_glow_tier = "ultra".to_string();
                            state.settings_dirty = true;
                        }
                    }
                    None => {
                        // Same one-download-at-a-time rule as the catalog
                        // buttons: disabled while ACTIVELY transferring, a
                        // FAILED attempt re-enables (retry replaces the dead
                        // handle in lib.rs).
                        let downloading = state
                            .galaxy_glow_dl
                            .as_ref()
                            .and_then(|p| p.lock().ok().map(|g| !g.2.starts_with("FAILED")))
                            .unwrap_or(false);
                        if ui
                            .add_enabled(
                                !downloading,
                                egui::Button::new("Download ultra glow (16384, 99 MB)"),
                            )
                            .clicked()
                        {
                            state.galaxy_glow_download = true;
                        }
                    }
                }
            });
            if let Some(dl) = &state.galaxy_glow_dl {
                if let Ok(g) = dl.lock() {
                    let (done, total, ref status) = *g;
                    let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                    ui.add(egui::ProgressBar::new(frac).text(format!(
                        "Ultra glow: {} ({} / {} MB)",
                        status,
                        done / 1_048_576,
                        total.max(1) / 1_048_576
                    )));
                }
            }
            if state.galaxy_glow_installed.is_some() && state.galaxy_glow_dl.is_none() {
                if ui.button("Remove ultra glow texture").clicked() {
                    state.galaxy_glow_remove = true;
                }
            }
            ui.label(
                RichText::new("Ultra is a sharper 16384x8192 bake of the same catalog light. Uses about 512 MB of GPU memory; applies next time you enter the world.")
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
            );
        }
        // Star halos (2026-07-11): soft photographic glow + a faint 4-point
        // diffraction cross on the ~50 brightest stars (mag <= 2), drawn
        // additively over the star points. A plain visibility flag on the
        // star renderer - applies live, nothing to rebuild.
        if widgets::toggle(ui, theme, "Star halos", &mut state.settings.sky_star_halos) {
            state.settings_dirty = true;
        }
        ui.label(
            RichText::new("A soft long-exposure-photo glow around the brightest stars (Sirius, Vega, Rigel...). Applies live.")
                .color(theme.text_muted())
                .size(theme.font_size_small),
        );

        // ── Star catalog (v0.800 rung 2; 2026-07-11 rung 4: 3-tier chooser) ──
        // Standard ships with the app; Extended (ATHYG, 36 MB) and Ultra
        // (Gaia G<14, ~350 MB) are one-time downloads from GitHub release
        // assets, dropped beside stars.bin. The loader prefers the biggest
        // installed catalog on the next world entry.
        {
            use crate::renderer::stars::StarCatalogTier;
            ui.add_space(theme.spacing_md);
            ui.label(RichText::new("Star catalog").color(theme.text_secondary()).strong());
            // Which tier actually renders: mirror of StarCatalog::load's
            // prefer order (biggest installed wins).
            let active = StarCatalogTier::ALL
                .iter()
                .rev()
                .find(|t| state.star_catalog_installed[t.index()].is_some())
                .map(|t| t.label())
                .unwrap_or("Standard");
            ui.label(
                RichText::new(format!("Active: {} (the biggest installed catalog wins)", active))
                    .color(theme.text_secondary())
                    .size(theme.font_size_small),
            );
            ui.add_space(theme.spacing_xs);

            // Render-tier CEILING (2026-07-12 dev tooling): caps which catalog
            // actually LOADS, independent of what is downloaded. "Auto" keeps
            // the biggest-installed-wins default; "Standard" forces the fast
            // 120k catalog (big win when doing planet/dev work with the Ultra
            // catalog installed). Applies next world entry. The env var
            // HUMANITY_STAR_TIER overrides this for scripted/verify boots.
            ui.label(RichText::new("Render tier").size(theme.font_size_small).color(theme.text_secondary()));
            ui.horizontal_wrapped(|ui| {
                for (val, lbl) in [
                    ("auto", "Auto"),
                    ("standard", "Standard (fast)"),
                    ("extended", "Extended"),
                    ("ultra", "Ultra"),
                ] {
                    let selected = state.settings.star_catalog_tier == val;
                    if ui.selectable_label(selected, lbl).clicked() && !selected {
                        state.settings.star_catalog_tier = val.to_string();
                        state.settings_dirty = true;
                    }
                }
            });
            ui.label(
                RichText::new("Caps which catalog loads. Auto uses the biggest installed; Standard forces the fast 120k catalog. Applies next world entry.")
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
            );
            ui.add_space(theme.spacing_xs);

            // Standard tier: always installed, nothing to download or remove.
            ui.label(
                RichText::new("Standard: 120,000 nearby stars (HYG). Ships with the app.")
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
            );
            ui.add_space(theme.spacing_xs);

            // One download slot: every Download button disables while a
            // transfer is ACTIVELY running; a FAILED attempt re-enables them
            // (the retry click replaces the dead handle in lib.rs).
            let downloading = state
                .star_catalog_dl
                .as_ref()
                .and_then(|(_, p)| p.lock().ok().map(|g| !g.2.starts_with("FAILED")))
                .unwrap_or(false);
            for tier in StarCatalogTier::ALL {
                match state.star_catalog_installed[tier.index()] {
                    Some(bytes) => {
                        ui.label(
                            RichText::new(format!(
                                "{}: {} ({} MB installed)",
                                tier.label(),
                                tier.blurb(),
                                bytes / 1_048_576
                            ))
                            .color(theme.text_muted())
                            .size(theme.font_size_small),
                        );
                        if ui
                            .button(format!("Remove {} catalog", tier.label().to_lowercase()))
                            .clicked()
                        {
                            state.star_catalog_remove = Some(tier);
                        }
                    }
                    None => {
                        ui.label(
                            RichText::new(format!("{}: {}", tier.label(), tier.blurb()))
                                .color(theme.text_muted())
                                .size(theme.font_size_small),
                        );
                        if ui
                            .add_enabled(
                                !downloading,
                                egui::Button::new(format!(
                                    "Download {} catalog ({})",
                                    tier.label().to_lowercase(),
                                    tier.size_hint()
                                )),
                            )
                            .clicked()
                        {
                            state.star_catalog_download = Some(tier);
                        }
                    }
                }
                ui.add_space(theme.spacing_xs);
            }
            if let Some((tier, dl)) = &state.star_catalog_dl {
                if let Ok(g) = dl.lock() {
                    let (done, total, ref status) = *g;
                    let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                    ui.add(egui::ProgressBar::new(frac).text(format!(
                        "{}: {} ({} / {} MB)",
                        tier.label(),
                        status,
                        done / 1_048_576,
                        total.max(1) / 1_048_576
                    )));
                }
            }
            ui.label(
                RichText::new("Catalog changes apply next time you enter the world.")
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
            );
        }

        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Machine label distances (m)").color(theme.text_secondary()).strong());
        ui.label(RichText::new("How close to show a machine's dot / name / info card. Hold Tab in-game to triple these and see through walls.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
        // These live on GuiState (session-tunable); the defaults (21 / 13 / 8) are the
        // saved-feel values. Not persisted to settings yet.
        widgets::labeled_slider(ui, theme, "Dot", &mut state.machine_label_dot_dist, 2.0..=60.0);
        widgets::labeled_slider(ui, theme, "Name", &mut state.machine_label_name_dist, 1.0..=40.0);
        widgets::labeled_slider(ui, theme, "Info card", &mut state.machine_label_card_dist, 1.0..=30.0);

        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Home").color(theme.text_secondary()).strong());
        // Construction mode (v0.453): the home roof. Off by default so the sky shows through
        // the open top; on seals it. Also toggled with the R key in-world.
        widgets::toggle(ui, theme, "Show roof (R)", &mut state.show_roof);
        ui.label(RichText::new("Off shows the sky (stars + the real solar system) through the open top; on seals the home for an interior / atmosphere look.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_xs);
        // Hull wrap (ship-superstructure increment D): the generated exterior shell around the
        // zone cluster (data/blueprints/hull_profile.ron). Default ON; also toggled with H.
        widgets::toggle(ui, theme, "Show hull (H)", &mut state.show_hull);
        ui.label(RichText::new("The generated exterior hull around the ship's zones (open above glass roofs, so gardens keep their starlight). Off for unobstructed interior or top-down build views.").color(theme.text_muted()).size(theme.font_size_small));
    });
}

/// Settings > Gameplay (v0.791): survival tuning + which home design loads.
/// Born from an operator field report ("disable the wolves... extend the
/// dehydration time. I keep getting killed and its annoying").
pub(crate) fn draw_gameplay_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Play mode (task #50): Normal | Creative | Dev ──
    // The one ladder every cheat/scope gate hangs off; see
    // crate::config::PlayMode for the full design + tested truth table.
    // Applies LIVE: the gates (construction scope, Dev page, creative flag,
    // fly/FTL) read the mode per frame, no world reload needed.
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Play mode").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(
                "Who gets which powers. Applies immediately: building scope, \
                 the Dev page, and free materials all follow the mode. When \
                 the mode is not Normal, a CREATIVE / DEV tag shows on the \
                 HUD so screenshots stay honest.",
            )
            .color(theme.text_muted())
            .size(theme.font_size_small),
        );
        ui.add_space(theme.spacing_sm);
        for mode in crate::config::PlayMode::ALL {
            let selected = state.settings.play_mode == mode;
            if ui.radio(selected, RichText::new(mode.label()).color(theme.text_primary())).clicked()
                && !selected
            {
                state.settings.play_mode = mode;
                // The mode PRESETS the creative (free resources) flag right
                // now; the per-frame bridge in lib.rs keeps Normal honest even
                // if something else flips the flag later. Deliberately does
                // NOT touch the Vitals drain slider below -- vitals stay a
                // separate knob you pair with Creative if you want needs
                // paused (per the mode's own description).
                state.creative_mode =
                    mode.allows(crate::config::Capability::FreeResources);
                state.settings_dirty = true; // persists play_mode to config.json
            }
            ui.label(
                RichText::new(mode.hint())
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
            );
            ui.add_space(theme.spacing_xs);
        }
        // Multiplayer honesty note (task #50): in a shared world the relay is
        // the authority on shared state, so Dev tools keep working for now;
        // per-player server-enforced permissions are the follow-up when real
        // players arrive. The HUD tag is force-shown so nobody can pass off a
        // Dev-mode screenshot as survival play.
        if state.copresence_active {
            ui.label(
                RichText::new(
                    "You are in a shared world: the mode tag stays visible on \
                     the HUD, and the server remains the authority on shared \
                     state.",
                )
                .color(theme.warning())
                .size(theme.font_size_small),
            );
        }
    });
    ui.add_space(theme.spacing_md);
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Survival").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        if widgets::toggle(ui, theme, "Hostile wildlife", &mut state.settings.hostile_wildlife) {
            state.settings_dirty = true;
        }
        ui.label(
            RichText::new(
                "Wolf packs and other predators in the wilds. Off removes them \
                 immediately; turning it on repopulates next time you enter the \
                 world. The Dev spawn page can always place any creature.",
            )
            .color(theme.text_muted())
            .size(theme.font_size_small),
        );
        ui.add_space(theme.spacing_sm);
        if widgets::labeled_slider(ui, theme, "Vitals drain", &mut state.settings.vitals_drain, 0.0..=3.0) {
            state.settings_dirty = true;
        }
        ui.label(
            RichText::new(
                "How fast hunger, thirst, and energy fall. 1.0 = normal (about \
                 half an hour from full to empty), 0 = survival needs paused.",
            )
            .color(theme.text_muted())
            .size(theme.font_size_small),
        );

        ui.add_space(theme.spacing_lg);
        // Household size (2026-07-01, moved here from Data in v0.791): which home design
        // data/machines/*.ron loads. Two real, fully-authored designs exist -- the default
        // family-scale home.ron and a one-person self-sufficient design (home_solo.ron,
        // see docs/design/homestead-solo-design.md) sized to real one-person kWh/L/kcal
        // needs. GUI-first per the project's own rule.
        ui.label(RichText::new("Home Design").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new("Which pre-built homestead loads. Takes effect next time you enter the world (restart HumanityOS to apply immediately).")
                .color(theme.text_muted())
                .size(theme.font_size_small),
        );
        ui.add_space(theme.spacing_sm);
        let mut is_family = state.settings.home_variant != "home_solo";
        let mut is_solo = state.settings.home_variant == "home_solo";
        if ui.radio_value(&mut is_family, true, "Family (default) -- 3-person self-sufficient design").changed() && is_family {
            state.settings.home_variant = "home".to_string();
            state.settings_dirty = true;
        }
        if ui.radio_value(&mut is_solo, true, "Solo -- 1-person self-sufficient design").changed() && is_solo {
            state.settings.home_variant = "home_solo".to_string();
            state.settings_dirty = true;
        }
    });
}

pub(crate) fn draw_controls_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        // Range max 1.0 keeps the slider in the usable band AND selects the widget's
        // 2-decimal display (max <= 1.0), so a low value like 0.11 is visible and tunable
        // instead of rounding to "0.1". 1.0 here is a fast 0.01 rad per mouse-pixel.
        if widgets::labeled_slider(ui, theme, "Mouse Sensitivity", &mut state.settings.mouse_sensitivity, 0.02..=1.0) {
            state.settings_dirty = true;
        }
        if widgets::toggle(ui, theme, "Invert Y-Axis", &mut state.settings.invert_y) {
            state.settings_dirty = true;
        }

        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Keybinds").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        let keybinds = [
            ("Move Forward", "W"),
            ("Move Back", "S"),
            ("Move Left", "A"),
            ("Move Right", "D"),
            ("Jump", "Space"),
            ("Sprint", "Shift"),
            ("Interact", "E"),
            ("Inventory", "I"),
            ("Reveal labels (hold)", "Tab"),
            ("Map", "M"),
            ("Escape Menu", "Esc"),
        ];
        // Three LEFT-aligned columns [Action | Primary | Secondary] in a Grid so
        // each key sits right next to its action — was label-left / key-far-right
        // (right_to_left layout), which the operator flagged as hard to match up.
        // Secondary is a placeholder (", ") until per-action secondary bindings
        // are wired through the input map.
        egui::Grid::new("keybinds_grid")
            .num_columns(3)
            .spacing([24.0, theme.row_gap])
            .show(ui, |ui| {
                ui.label(RichText::new("Action").color(theme.text_muted()).size(theme.font_size_small));
                ui.label(RichText::new("Primary").color(theme.text_muted()).size(theme.font_size_small));
                ui.label(RichText::new("Secondary").color(theme.text_muted()).size(theme.font_size_small));
                ui.end_row();
                for (action, key) in &keybinds {
                    ui.label(RichText::new(*action).color(theme.text_secondary()));
                    egui::Frame::none()
                        .fill(Color32::from_rgb(40, 40, 50))
                        .rounding(Rounding::same(3))
                        .inner_margin(Vec2::new(8.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(*key).color(theme.text_primary()).size(theme.font_size_small).strong());
                        });
                    ui.label(RichText::new("(none)").color(theme.text_muted()).size(theme.font_size_small));
                    ui.end_row();
                }
            });
    });
}

pub(crate) fn draw_privacy_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        widgets::toggle(ui, theme, "Profile Visible to Others", &mut state.settings.profile_visible);
        widgets::toggle(ui, theme, "Show Online Status", &mut state.settings.online_status_visible);
    });
}

/// Open a folder (or a file's parent folder) in the OS file manager.
/// Windows explorer / macOS open / Linux xdg-open; spawn-and-forget.
fn open_in_file_manager(path: &std::path::Path) {
    let dir = if path.is_file() { path.parent().unwrap_or(path) } else { path };
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
}

/// One "label: path [Open]" row for the Storage section. The Open button
/// only renders when the path exists (nothing to show otherwise).
fn storage_path_row(ui: &mut egui::Ui, theme: &Theme, label: &str, path: &std::path::Path) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{label}:"))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        ui.label(
            RichText::new(path.display().to_string())
                .size(theme.font_size_small)
                .color(theme.text_primary()),
        );
        if path.exists() && widgets::compact_button(ui, theme, "Open", widgets::ButtonVariant::Secondary) {
            open_in_file_manager(path);
        }
    });
}

pub(crate) fn draw_data_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        // Where your files live (v0.741, GUI-first + the v0.707 storage-chooser
        // follow-up): show the ACTIVE storage mode + every real path, each with
        // an Open button, so nobody needs a terminal (or a doc) to find their
        // saves, identity, or modding data.
        ui.label(RichText::new("Storage").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        let (mode_name, mode_note) = match crate::storage::mode() {
            crate::storage::StorageMode::Portable => (
                "Portable",
                "Everything lives beside the app (USB-drive friendly). Delete portable.txt next to the exe and restart to switch to per-user storage.",
            ),
            crate::storage::StorageMode::Installed => (
                "Per-user",
                "Your files live in your user folder, so they survive app updates and moves.",
            ),
            crate::storage::StorageMode::LegacyBesideExe => (
                "Beside the app (legacy)",
                "A data folder sits next to the exe from an earlier setup; files stay there so nothing moves out from under you.",
            ),
            crate::storage::StorageMode::Undecided => (
                "Not chosen yet",
                "The first-boot storage chooser runs before any files are written.",
            ),
        };
        ui.label(RichText::new(format!("Mode: {mode_name}")).color(theme.text_primary()));
        ui.label(
            RichText::new(mode_note)
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);
        if let Some(p) = crate::storage::writable_data_dir() {
            storage_path_row(ui, theme, "Game data (modding)", &p);
        }
        storage_path_row(ui, theme, "Saves", &crate::persistence::saves_dir());
        storage_path_row(ui, theme, "Settings + identity", &crate::config::AppConfig::config_path());

        // Move-my-files (v0.742, the second v0.707 follow-up): switch modes
        // WITH the file migration, in-app. Copy-first, commit-last, originals
        // kept — see storage.rs's migration safety contract. Two-click confirm;
        // the result line persists in egui temp memory until the next attempt.
        let migrate_result_id = egui::Id::new("storage_migrate_result");
        let confirm_id = egui::Id::new("storage_migrate_confirm");
        // ONE migration per session (adversarial-review hardening): after a
        // successful move the switch button is replaced by the restart note,
        // so the mode can't be toggled back and forth against half-reloaded
        // session state. The flag is session-memory; a restart clears it.
        let migrated_id = egui::Id::new("storage_migrated_this_session");
        let migrated = ui.data_mut(|d| d.get_temp::<bool>(migrated_id).unwrap_or(false));
        let target: Option<(&str, &str)> = match crate::storage::mode() {
            crate::storage::StorageMode::Installed
            | crate::storage::StorageMode::LegacyBesideExe => Some((
                "Switch to portable storage",
                "Copies your files (identity, saves, game data, logs) next to the app so the folder travels between machines. Your current files stay where they are as a backup.",
            )),
            crate::storage::StorageMode::Portable => Some((
                "Switch to per-user storage",
                "Copies your files into your user folder so they survive app moves. The app-side copies stay as a backup.",
            )),
            crate::storage::StorageMode::Undecided => None,
        };
        if migrated {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Files moved. Restart HumanityOS to finish switching storage modes.")
                    .size(theme.font_size_small)
                    .color(theme.accent()),
            );
        } else if let Some((label, note)) = target {
            ui.add_space(theme.spacing_sm);
            let confirming = ui.data_mut(|d| d.get_temp::<bool>(confirm_id).unwrap_or(false));
            if confirming {
                ui.label(
                    RichText::new(note).size(theme.font_size_small).color(theme.text_muted()),
                );
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, theme, "Yes, move my files") {
                        let result = match crate::storage::mode() {
                            crate::storage::StorageMode::Portable => {
                                crate::storage::migrate_to_per_user()
                            }
                            _ => crate::storage::migrate_to_portable(),
                        };
                        let line = match result {
                            Ok(msg) => {
                                ui.data_mut(|d| d.insert_temp(migrated_id, true));
                                msg
                            }
                            Err(e) => format!("Nothing was changed: {e}"),
                        };
                        ui.data_mut(|d| {
                            d.insert_temp(migrate_result_id, line);
                            d.insert_temp(confirm_id, false);
                        });
                    }
                    if widgets::secondary_button(ui, theme, "Cancel") {
                        ui.data_mut(|d| d.insert_temp(confirm_id, false));
                    }
                });
            } else if widgets::secondary_button(ui, theme, label) {
                ui.data_mut(|d| d.insert_temp(confirm_id, true));
            }
            if let Some(line) = ui.data_mut(|d| d.get_temp::<String>(migrate_result_id)) {
                if !line.is_empty() {
                    ui.add_space(theme.spacing_xs);
                    let failed = line.starts_with("Nothing was changed");
                    ui.label(
                        RichText::new(line)
                            .size(theme.font_size_small)
                            .color(if failed { theme.danger() } else { theme.success() }),
                    );
                }
            }
        }

        ui.add_space(theme.spacing_lg);
        // (Home Design moved to Settings > Gameplay in v0.791.)

        ui.label(RichText::new("Export & Backup").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Export your data for backup or migration.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_sm);

        ui.horizontal(|ui| {
            let _ = widgets::secondary_button(ui, theme, "Export Profile Data");
            let _ = widgets::secondary_button(ui, theme, "Export Save Data");
        });

        ui.add_space(theme.spacing_lg);

        ui.label(RichText::new("Cache").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Clear cached data to free disk space.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_sm);
        let _ = widgets::secondary_button(ui, theme, "Clear Cache");

        ui.add_space(theme.spacing_lg);

        ui.label(RichText::new("Danger Zone").color(theme.danger()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Permanently delete your account and all associated data.").color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_sm);
        let _ = widgets::danger_button(ui, theme, "Delete Account");
    });
}

pub(crate) fn draw_updates_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        // Current version
        ui.label(RichText::new(format!("Current Version: v{}", VERSION)).strong());
        ui.add_space(theme.spacing_sm);

        // Update channel
        ui.label(RichText::new("Update Channel").color(theme.text_secondary()));
        let mut is_latest = state.updater.channel == UpdateChannel::AlwaysLatest;
        let mut is_disabled = state.updater.channel == UpdateChannel::Disabled;

        if ui.radio_value(&mut is_latest, true, "Always Latest (recommended)").changed() && is_latest {
            state.updater.channel = UpdateChannel::AlwaysLatest;
        }
        if ui.radio_value(&mut is_disabled, true, "Disabled (never check)").changed() && is_disabled {
            state.updater.channel = UpdateChannel::Disabled;
        }

        ui.add_space(theme.spacing_md);

        // Status
        let status_text = match &state.updater.state {
            UpdateState::Idle => "Not checked yet".to_string(),
            UpdateState::Checking => "Checking for updates...".to_string(),
            UpdateState::UpToDate => "You're on the latest version".to_string(),
            UpdateState::Available { version, .. } => format!("Update available: {}", version),
            UpdateState::Downloading { version, progress } => {
                format!("Downloading {}: {:.0}%", version, progress * 100.0)
            }
            UpdateState::Ready { version, .. } => format!("{} ready. Restart to apply.", version),
            UpdateState::Error(e) => format!("Error: {}", e),
        };
        ui.label(RichText::new(&status_text).color(
            match &state.updater.state {
                UpdateState::Available { .. } => theme.accent(),
                UpdateState::Error(_) => theme.danger(),
                UpdateState::Ready { .. } => theme.success(),
                _ => theme.text_secondary(),
            }
        ));

        ui.add_space(theme.spacing_sm);

        // Action buttons
        ui.horizontal(|ui| {
            if widgets::primary_button(ui, theme, "Check Now") {
                state.updater.check_now();
            }

            if let UpdateState::Available { version, .. } = &state.updater.state {
                let ver = version.clone();
                if widgets::primary_button(ui, theme, "Download & Install") {
                    state.updater.download_version(&ver);
                }
            }

            if let UpdateState::Ready { .. } = &state.updater.state {
                if widgets::primary_button(ui, theme, "Restart to Apply") {
                    // Read the restart target from restart_target.txt (written
                    // before the binary swap) to get the correct exe path.
                    let target = crate::updater::read_restart_target(&state.updater.exe_path);
                    crate::debug::push_debug(format!("Updater: restart target = {}", target.display()));
                    log::info!("Restarting from: {}", target.display());

                    #[cfg(target_os = "windows")]
                    {
                        // Use a batch script to wait for this process to exit
                        // before launching the new binary. This avoids the race
                        // where the old process hasn't fully released the exe.
                        match crate::updater::create_restart_script(&target) {
                            Ok(bat) => {
                                crate::debug::push_debug(format!("Updater: launching restart script {}", bat.display()));
                                use std::os::windows::process::CommandExt;
                                let _ = std::process::Command::new("cmd")
                                    .args(["/C", &bat.to_string_lossy()])
                                    .creation_flags(0x00000008) // DETACHED_PROCESS
                                    .spawn();
                            }
                            Err(e) => {
                                // Fallback: try direct spawn if batch script fails
                                crate::debug::push_debug(format!("Updater: batch script failed ({}), trying direct spawn", e));
                                log::warn!("Updater: batch script failed: {}", e);
                                let _ = std::process::Command::new(&target).spawn();
                            }
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let _ = std::process::Command::new(&target).spawn();
                    }

                    state.quit_requested = true;
                }
            }
        });

        // Download progress bar
        if let UpdateState::Downloading { progress, .. } = &state.updater.state {
            ui.add_space(theme.spacing_sm);
            widgets::progress_bar(ui, theme, *progress, Some("Downloading..."));
        }

        // Release notes
        if let UpdateState::Available { ref release_notes, .. } = &state.updater.state {
            if !release_notes.is_empty() {
                ui.add_space(theme.spacing_md);
                ui.label(RichText::new("Release Notes").color(theme.text_secondary()).strong());
                ui.add_space(theme.spacing_xs);
                egui::Frame::none()
                    .fill(Color32::from_rgb(30, 30, 38))
                    .rounding(Rounding::same(4))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(release_notes).color(theme.text_muted()).size(theme.font_size_small));
                    });
            }
        }
    });

    ui.add_space(theme.spacing_md);

    // Version picker
    widgets::card_with_header(ui, theme, "Available Versions", |ui| {
        let versions = state.updater.available_versions();
        if versions.is_empty() {
            ui.label(RichText::new("Check for updates to see available versions.").color(theme.text_muted()));
        } else {
            for (tag, date, is_current) in &versions {
                ui.horizontal(|ui| {
                    let label = if *is_current {
                        RichText::new(format!("{} (current)", tag)).strong().color(theme.success())
                    } else {
                        RichText::new(tag).color(theme.text_primary())
                    };
                    ui.label(label);
                    ui.label(RichText::new(date).small().color(theme.text_muted()));

                    if !is_current {
                        let tag_clone = tag.clone();
                        if ui.small_button("Install").clicked() {
                            state.updater.download_version(&tag_clone);
                        }
                    }
                });
            }
        }
    });
}

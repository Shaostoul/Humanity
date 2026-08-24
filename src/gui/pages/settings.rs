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
        // Width alone is not enough: without pinning min == max, egui advances
        // the cursor by only the label's TEXT width, so short labels pulled
        // their track left and long ones pushed it right - the stair-stepping
        // the operator flagged twice (second time specifically here in Widgets,
        // which had this duplicate of the settings_row bug).
        ui.allocate_ui_with_layout(
            Vec2::new(170.0, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(170.0);
                ui.set_max_width(170.0);
                ui.label(RichText::new(label).color(label_color));
            },
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

            // The nav mirrors the section tints on the right: each item wears
            // its own section colour, and the active one gets the SAME animated
            // border as the main header nav (operator, 2026-08-12), driven by
            // the shared `channeling_color` (RGB cycle / solid / pulse / off,
            // set in Animations).
            let time = ui.ctx().input(|i| i.time) as f32;
            let attack_pulse = state.attack_pulse_active;
            for (label, cat) in &categories {
                let is_active = state.settings.category == *cat;
                let accent = section_accent(*cat, theme);
                // Inactive items wear their section colour; the active one goes
                // bright white so the animated border is the highlight.
                let text_color = if is_active { Color32::WHITE } else { accent };
                let a = accent;
                let bg = if is_active {
                    Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 34)
                } else {
                    Color32::TRANSPARENT
                };
                let stroke = if is_active {
                    // Keep the border animating even while the menu is idle.
                    ui.ctx().request_repaint();
                    Stroke::new(
                        theme.nav_active_border_width,
                        crate::gui::pages::escape_menu::channeling_color(theme, time, attack_pulse, accent),
                    )
                } else {
                    Stroke::NONE
                };

                let btn = egui::Button::new(
                    RichText::new(*label).size(theme.font_size_body).color(text_color),
                )
                .fill(bg)
                .stroke(stroke)
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
                            ui.add_space(theme.section_gap);
                        }

                        // Section heading text
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

                        // Each section sits in a faint accent-tinted band with a
                        // coloured edge and a coloured title, so a long scroll
                        // reads as distinct areas instead of one wall of dark
                        // cards (operator, 2026-08-12). Accents come from
                        // existing theme tokens (see `section_accent`), so they
                        // stay editable and adjacent sections differ at a glance.
                        let accent = section_accent(*cat, theme);
                        let tint = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 12);
                        let edge = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 110);
                        let mut heading_rect = egui::Rect::NOTHING;
                        Frame::none()
                            .fill(tint)
                            .rounding(Rounding::same(theme.border_radius as u8))
                            .stroke(Stroke::new(1.0, edge))
                            .inner_margin(theme.card_padding)
                            .show(ui, |ui| {
                                let hr = ui.label(
                                    RichText::new(heading_text)
                                        .size(theme.font_size_title)
                                        .color(accent)
                                        .strong(),
                                );
                                heading_rect = hr.rect;
                                ui.add_space(theme.spacing_md);

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
                            });
                        section_rects.push((*cat, heading_rect));
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

/// A faint accent per settings section (operator, 2026-08-12: "add a variety
/// of ever so faint colored backgrounds to the sections to more easily
/// differentiate what we're looking at").
///
/// Colours are drawn from the EXISTING theme tokens rather than new literals,
/// so every one stays editable in Appearance and the whole scheme restyles
/// from `theme.ron`. They are also chosen so that adjacent sections in the
/// scroll order differ (blue, purple, amber, blue, red, green, ...).
fn section_accent(cat: SettingsCategory, theme: &Theme) -> Color32 {
    match cat {
        SettingsCategory::Account => theme.info(),
        SettingsCategory::Appearance => theme.nav_sim(),
        SettingsCategory::Animations => theme.nav_dev(),
        SettingsCategory::Widgets => theme.nav_tools(),
        SettingsCategory::Notifications => theme.danger(),
        SettingsCategory::Wallet => theme.success(),
        SettingsCategory::Audio => theme.info(),
        SettingsCategory::Graphics => theme.nav_sim(),
        SettingsCategory::Gameplay => theme.success(),
        SettingsCategory::Controls => theme.nav_tools(),
        SettingsCategory::Privacy => theme.warning(),
        SettingsCategory::Data => theme.nav_settings(),
        SettingsCategory::Updates => theme.accent(),
    }
}

pub(crate) fn draw_account_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let accent = section_accent(SettingsCategory::Account, theme);

    // ── Your data: export + erase (sovereignty, 2026-08-23) ──
    widgets::subsection_header(ui, theme, accent, "Your data", "");
    widgets::card(ui, theme, |ui| {
        widgets::body_hint(
            ui, theme,
            "Download everything the connected server stores about you, or erase it \
             all, self-service, no admin needed. Data on this device is yours and stays.",
        );
        let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
        ui.horizontal(|ui| {
            if connected && widgets::secondary_button(ui, theme, "Export my data") {
                if let Some(ref client) = state.ws_client {
                    client.send(&serde_json::json!({ "type": "account_export" }).to_string());
                }
                state.account_export_status = "Requested; writing the export file when it arrives.".to_string();
            }
            if !connected {
                ui.label(
                    egui::RichText::new("Connect to a server to export or erase.")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            }
        });
        if !state.account_export_status.is_empty() {
            ui.label(
                egui::RichText::new(&state.account_export_status)
                    .color(theme.text_secondary())
                    .size(theme.font_size_small),
            );
        }
        ui.add_space(theme.spacing_sm);
        widgets::body_hint(
            ui, theme,
            "Erase: removes your messages, uploads, profile, follows, mailbox, and \
             membership from this server, permanently. Type your display name exactly \
             to arm the button.",
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.account_delete_confirm_input)
                    .hint_text("type your name to confirm")
                    .desired_width(180.0),
            );
            let armed = connected
                && !state.account_delete_confirm_input.trim().is_empty()
                && state.account_delete_confirm_input.trim() == state.user_name;
            if armed
                && widgets::Button::danger("Erase my account on this server").show(ui, theme)
            {
                if let Some(ref client) = state.ws_client {
                    client.send(&serde_json::json!({
                        "type": "account_delete",
                        "confirm_name": state.account_delete_confirm_input.trim(),
                    }).to_string());
                }
                state.account_delete_confirm_input.clear();
            }
        });
    });
    ui.add_space(theme.spacing_md);

    // ── Identity: who you are (name, key, seed phrase, device linking) ──
    widgets::subsection_header(ui, theme, accent, "Identity", "");
    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, "Display name", |ui| {
            let resp = ui.add(egui::TextEdit::singleline(&mut state.user_name).desired_width(200.0));
            // Commit on Enter or the Save button. WITHOUT this the name was
            // session-only: it never saved to config (so a restart bounced to a
            // DesktopUser_NNNN default) and never re-registered with the relay, so
            // the name never actually stuck (operator-reported, 2026-07-13).
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let save = widgets::secondary_button(ui, theme, "Save");
            if enter || save {
                let cleaned: String = state.user_name.trim().chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .take(24)
                    .collect();
                if !cleaned.is_empty() {
                    state.user_name = cleaned;
                    // Persist immediately, then reconnect so the relay
                    // re-registers this name to our key.
                    crate::config::AppConfig::from_gui_state(state).save();
                    state.apply_pq_identity();
                }
            }
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
                // Seed-phrase box: a warm wash of the warning token (it holds the
                // single most dangerous string in the app), derived from the token
                // via alpha rather than a hardcoded brown.
                let w = theme.warning();
                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(w.r(), w.g(), w.b(), 40))
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
                    theme.success()
                };
                ui.label(RichText::new(&state.settings.seed_phrase_recovery_status).color(color).size(theme.font_size_small));
            }
        }
    });

    ui.add_space(theme.spacing_md);

    // ── Security: how the key is protected at rest and at launch ──
    widgets::subsection_header(ui, theme, accent, "Security", "");

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

    // ── Donation addresses (admin/owner): the header carries the title, so
    // the card no longer repeats it inside ──
    widgets::subsection_header(ui, theme, accent, "Donation addresses", "");
    widgets::card(ui, theme, |ui| {
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
    // Read the hint-display mode ONCE up front; every `setting_hint` call below
    // uses it. Captured (Copy) so it does not re-borrow `state` per call.
    let hint = state.settings.hint_display;
    let accent = section_accent(SettingsCategory::Appearance, theme);
    // ── Basics: the everyday knobs (theme, text size, help text, timestamps) ──
    widgets::subsection_header(ui, theme, accent, "Basics", "");
    widgets::card(ui, theme, |ui| {
        if widgets::toggle(ui, theme, "Dark Mode", &mut state.settings.dark_mode) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Dark backgrounds with light text (easier on the eyes at night). Off = light theme.");

        if widgets::labeled_slider(ui, theme, "Font Size", &mut state.settings.font_size, 10.0..=24.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Base text size in points. Higher = larger, easier-to-read text that fits less on screen. Per-element sizes live under Widgets > Fonts.");

        // The hint-display mode itself (Full / Hover / Off) — controls how every
        // description on this page, including the ones right here, is shown.
        if hint_display_picker(ui, theme, &mut state.settings.hint_display) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How each setting's help text on this page is shown. Full keeps every description visible and tucked under its control; Hover hides them behind a small (?) you point at; Off removes them entirely.");

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

    // ── Theme colors: live-edit every color token; saves to data/gui/theme.ron.
    // The subsection header above the frame carries the title now. ──
    widgets::subsection_header(ui, theme, accent, "Theme colors", "");
    let mut any_color_changed = false;
    let card_bg = theme.bg_card();
    let card_border = theme.border();
    let card_radius = theme.border_radius;
    let card_padding = theme.card_padding;
    let label_color = theme.text_secondary();

    egui::Frame::none()
        .fill(card_bg)
        .rounding(Rounding::same(card_radius as u8))
        .inner_margin(card_padding)
        .stroke(Stroke::new(1.0, card_border))
        .show(ui, |ui| {
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
                    ("DM accent (bar + header)",  &mut theme.dm_accent as *mut _),
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
                    ("Group accent (header)",     &mut theme.group_accent as *mut _),
                    ("Scratchpad accent",         &mut theme.scratchpad_accent as *mut _),
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
                let now = ui.ctx().input(|i| i.time);
                if widgets::Button::primary("Save Theme").show(ui, theme) {
                    theme.save();
                    state.toast("Theme saved", crate::gui::ToastKind::Success, now);
                }
                if widgets::Button::secondary("Reset Colors").show(ui, theme) {
                    theme.reset_color_defaults();
                    any_color_changed = true;
                    state.toast("Colors reset to defaults", crate::gui::ToastKind::Info, now);
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
    // Copy the description-display mode into a local so the per-control hints
    // below don't re-borrow `state` inside the card closures (which already
    // capture it), and so `theme` stays free of any live mutable borrow.
    let hint = state.settings.hint_display;

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
        widgets::setting_hint(ui, theme, hint, "1.0 = normal. Higher = faster color movement; 0 holds it still.");
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
        widgets::setting_hint(ui, theme, hint, "1.0 = normal. Higher = faster, more urgent pulsing; lower = a slow fade.");
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
        let now = ui.ctx().input(|i| i.time);
        state.toast("Animation settings saved", crate::gui::ToastKind::Success, now);
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

/// Segmented picker for the per-setting description mode (Full / Hover / Off).
/// Same look as `u8_radio_picker`; returns true if the value changed. Kept
/// generic over `HintDisplay::ALL` so a new mode appears here automatically.
fn hint_display_picker(
    ui: &mut egui::Ui,
    theme: &Theme,
    value: &mut crate::gui::HintDisplay,
) -> bool {
    let mut changed = false;
    widgets::settings_row(ui, theme, "Setting descriptions", |ui| {
        for mode in crate::gui::HintDisplay::ALL {
            let active = *value == mode;
            if widgets::Button::secondary(mode.label()).active(active).show(ui, theme) {
                if !active {
                    *value = mode;
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

    let label_color = theme.text_muted();
    let text_color = theme.text_primary();
    // Preview-only surfaces, all read from tokens so the live preview shows the
    // user's ACTUAL theme rather than a frozen snapshot of some old palette.
    let preview_avatar = widgets::swatch_color("Alice");
    let preview_row_bg = theme.bg_card();
    let preview_row_bg_alt = theme.bg_tertiary();
    let preview_offline = theme.text_muted();
    let ss = SliderStyle::from_theme(theme);

    // Two-column layout: sliders on left, live preview on right
    ui.columns(2, |cols| {
        // ── LEFT COLUMN: sliders ──
        let ui = &mut cols[0];
        let mut any_changed = false;

        ui.label(RichText::new("All values are in pixels (fonts in points). Drag a slider and watch the Live Preview on the right update; click Save Theme to keep the look.")
            .size(ss.font_sm).color(label_color));
        ui.add_space(spacing_sm);

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
            let now = ui.ctx().input(|i| i.time);
            if widgets::primary_button(ui, theme, "Save Theme") {
                theme.save();
                state.toast("Theme saved", crate::gui::ToastKind::Success, now);
            }
            if widgets::secondary_button(ui, theme, "Reset to Defaults") {
                theme.reset_widget_defaults();
                any_changed = true;
                state.toast("Widget styles reset to defaults", crate::gui::ToastKind::Info, now);
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
            .stroke(Stroke::new(1.0, theme.border()))
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
                    preview_avatar,
                    "Alice",
                    "12:34 PM",
                    "This is a sample message to preview how the row widget looks with the current theme settings.",
                    true,
                    preview_row_bg,
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
                    preview_avatar,
                    "Alice",
                    "",
                    "A continuation message from the same user.",
                    false,
                    preview_row_bg_alt,
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
                        // Mirrors the real chat server-lane row (chat.rs), so the
                        // preview moves when the user edits those lane tokens.
                        let fill = if hover {
                            theme.server_row_hover()
                        } else {
                            theme.server_row_bg()
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
                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, theme.success());
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
                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, preview_offline);
                    ui.label(
                        RichText::new("Charlie")
                            .size(theme.body_size)
                            .color(preview_offline),
                    );
                });
            });
    });
}

pub(crate) fn draw_notifications_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // v0.980 (the 2026-07-20 dead-fields audit): this card used to edit
    // Settings fields NOTHING read - the real notification prefs live on the
    // RELAY (notification_prefs table), mirrored in state.notif_* and edited
    // by the chat DM cog. The card now edits that same live state and syncs
    // it to the server, making Settings and the cog two views of one truth.
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    // Lazy fetch, same rule as the DM cog: the first time this card renders
    // while connected, pull the stored prefs so toggles show reality.
    if connected && !state.notif_prefs_loaded {
        if let Some(ref client) = state.ws_client {
            client.send(&serde_json::json!({ "type": "get_notification_prefs" }).to_string());
            state.notif_prefs_loaded = true;
        }
    }
    let hint = state.settings.hint_display;
    let mut changed = false;
    widgets::card(ui, theme, |ui| {
        changed |= widgets::toggle(ui, theme, "Direct Messages", &mut state.notif_dm_enabled);
        changed |= widgets::toggle(ui, theme, "Mentions", &mut state.notif_mentions_enabled);
        changed |= widgets::toggle(ui, theme, "Task Updates", &mut state.notif_tasks_enabled);
        widgets::setting_hint(ui, theme, hint, "Which events notify you: private messages, someone naming you in chat, and changes to tasks you are on.");

        ui.label(RichText::new("Do Not Disturb").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        // The live fields are Option<String>; edit through plain buffers and
        // store trimmed non-empty values back as Some.
        let mut dnd_start = state.notif_dnd_start.clone().unwrap_or_default();
        let mut dnd_end = state.notif_dnd_end.clone().unwrap_or_default();
        widgets::form_row(ui, theme, "Quiet hours start", |ui| {
            if ui
                .add(egui::TextEdit::singleline(&mut dnd_start).desired_width(80.0).hint_text("22:00"))
                .lost_focus()
            {
                changed = true;
            }
        });
        widgets::form_row(ui, theme, "Quiet hours end", |ui| {
            if ui
                .add(egui::TextEdit::singleline(&mut dnd_end).desired_width(80.0).hint_text("08:00"))
                .lost_focus()
            {
                changed = true;
            }
        });
        state.notif_dnd_start = Some(dnd_start.trim().to_string()).filter(|s| !s.is_empty());
        state.notif_dnd_end = Some(dnd_end.trim().to_string()).filter(|s| !s.is_empty());
        widgets::setting_hint(ui, theme, hint, "Notifications stay silent between these times. 24-hour clock, e.g. 22:00 to 08:00 keeps nights quiet.");

        if !connected {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new("Sign in to chat to sync these to the server - they apply across your devices once saved.").color(theme.text_muted()).size(theme.font_size_small));
        }
    });
    if changed && connected {
        if let Some(ref client) = state.ws_client {
            client.send(
                &serde_json::json!({
                    "type": "update_notification_prefs",
                    "dm": state.notif_dm_enabled,
                    "mentions": state.notif_mentions_enabled,
                    "tasks": state.notif_tasks_enabled,
                    "dnd_start": state.notif_dnd_start,
                    "dnd_end": state.notif_dnd_end,
                })
                .to_string(),
            );
        }
    }
}

pub(crate) fn draw_wallet_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
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

        // v0.980 (dead-fields audit): this selector used to edit a Settings
        // field nothing read; the Wallet PAGE drives its own live
        // state.wallet_network. Both selectors now edit that one live field.
        // (The dead "Custom RPC URL" row is gone outright - no code ever
        // consumed it; it returns when the native wallet actually makes RPC
        // calls.)
        ui.horizontal(|ui| {
            for n in [WalletNetwork::Mainnet, WalletNetwork::Devnet, WalletNetwork::Testnet] {
                let is_sel = state.wallet_network == n;
                let text_color = if is_sel { theme.text_on_accent() } else { theme.text_secondary() };
                let fill = if is_sel { theme.accent() } else { Color32::TRANSPARENT };
                let btn = egui::Button::new(RichText::new(n.label()).color(text_color).size(theme.font_size_body))
                    .fill(fill)
                    .rounding(Rounding::same(4));
                if ui.add(btn).clicked() && !is_sel {
                    state.wallet_network = n;
                }
            }
        });
        widgets::setting_hint(ui, theme, hint, "Mainnet is the real Solana network where coins have value. Devnet and Testnet are free practice networks for trying things safely.");
    });
}

pub(crate) fn draw_audio_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    let accent = section_accent(SettingsCategory::Audio, theme);
    widgets::subsection_header(ui, theme, accent, "Volumes", "");
    widgets::card(ui, theme, |ui| {
        if widgets::labeled_slider(ui, theme, "Master Volume", &mut state.settings.master_volume, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Overall loudness of everything the app plays. Far left = silent, far right = full volume.");
        if widgets::labeled_slider(ui, theme, "Music Volume", &mut state.settings.music_volume, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Background music only. Scales together with Master Volume.");
        if widgets::labeled_slider(ui, theme, "SFX Volume", &mut state.settings.sfx_volume, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Game sound effects like footsteps and machines. Scales together with Master Volume. Interface clicks have their own control below.");
        // Interface sounds (v0.1112): button clicks and other UI feedback ride
        // their own "ui" bus, so they can be quieted or silenced without
        // touching game SFX. The toggle is the master switch; the slider is the
        // level. Answers "clicking makes a sound even if I don't click anything
        // is fairly loud" - the click is now gentle by default and fully
        // adjustable here.
        if widgets::toggle(ui, theme, "Interface sounds", &mut state.settings.ui_sounds_enabled) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Play a soft click when you press a button or toggle. Turn off for silent interface.");
        if widgets::labeled_slider(ui, theme, "UI Volume", &mut state.settings.ui_volume, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Loudness of interface clicks only. Separate from game sound effects; scales together with Master Volume.");
    });
    // Voice (v0.485). Device selectors + a mic loopback test (toggle) with a live
    // level meter, so you can confirm capture + playback and pick devices. The
    // full in-app voice transport is being built in phases.
    widgets::subsection_header(ui, theme, accent, "Voice", "");
    widgets::card(ui, theme, |ui| {
        // Enumerate audio devices once (cpal enumeration is slow, and the active
        // test repaints at 60fps, so never enumerate per frame). Refresh on demand.
        if !state.audio_devices_loaded {
            state.audio_input_devices = crate::net::voice::list_input_devices();
            state.audio_output_devices = crate::net::voice::list_output_devices();
            state.audio_devices_loaded = true;
        }
        let in_devs = state.audio_input_devices.clone();
        let out_devs = state.audio_output_devices.clone();
        // Mark settings dirty on a device pick so the choice persists to
        // config.json (the combos alone never set the dirty flag, so a
        // device selection silently failed to save; wiring-audit fix).
        let mut dev_changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Input (microphone)").size(theme.font_size_small).color(theme.text_secondary()));
            egui::ComboBox::from_id_salt("audio_in_dev")
                .selected_text(if state.audio_input_device.is_empty() { "System default".to_string() } else { state.audio_input_device.clone() })
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut state.audio_input_device, String::new(), "System default").changed() {
                        dev_changed = true;
                    }
                    for d in &in_devs {
                        if ui.selectable_value(&mut state.audio_input_device, d.clone(), d).changed() {
                            dev_changed = true;
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Output (speakers)").size(theme.font_size_small).color(theme.text_secondary()));
            egui::ComboBox::from_id_salt("audio_out_dev")
                .selected_text(if state.audio_output_device.is_empty() { "System default".to_string() } else { state.audio_output_device.clone() })
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut state.audio_output_device, String::new(), "System default").changed() {
                        dev_changed = true;
                    }
                    for d in &out_devs {
                        if ui.selectable_value(&mut state.audio_output_device, d.clone(), d).changed() {
                            dev_changed = true;
                        }
                    }
                });
        });
        if dev_changed {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Which microphone and speakers voice uses. A change takes effect the next time a call or mic test starts.");
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
        widgets::setting_hint(ui, theme, hint, "How loud your microphone sounds to others. 100% = unchanged; raise it if people say you are quiet, lower it if your voice crackles or distorts.");

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
        widgets::setting_hint(ui, theme, hint, state.voice_filter_mode.hint());

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
        widgets::setting_hint(ui, theme, hint, state.voice_transmit_mode.hint());

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
                widgets::setting_hint(ui, theme, hint, "Heads up: CapsLock also toggles caps each push. Rebind if that bothers you.");
            }
        }

        // Activation threshold (voice-activated only). Stored 0.0..=1.0, shown as %.
        if state.voice_transmit_mode == crate::config::VoiceTransmitMode::VoiceActivated {
            let mut vad_pct = state.voice_vad_threshold * 100.0;
            if widgets::labeled_slider(ui, theme, "Activation threshold %", &mut vad_pct, 0.0..=30.0) {
                state.voice_vad_threshold = (vad_pct / 100.0).clamp(0.0, 1.0);
                state.settings_dirty = true;
            }
            widgets::setting_hint(ui, theme, hint, "How loud you must be before your mic opens. Lower = opens at a whisper but may pick up background noise; higher = needs a clearer, louder voice.");
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
    let hint = state.settings.hint_display;
    // The section accent drives every subsection header below, so the whole
    // ladder (section title > subsection > group label) wears one colour.
    let accent = section_accent(SettingsCategory::Graphics, theme);
    widgets::card(ui, theme, |ui| {
        // ── General: universal traits (window, frame pacing, camera) ──
        widgets::subsection_header(ui, theme, accent, "General", "");
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
        widgets::setting_hint(ui, theme, hint, "Windowed fullscreen keeps the title bar + taskbar. Borderless drops the title bar. Exclusive is true fullscreen.");
        if widgets::toggle(ui, theme, "VSync", &mut state.settings.vsync) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Syncs frames to your monitor to stop image tearing. On = smoother, frame rate capped at your monitor's refresh rate; off = uncapped FPS but frames can visibly tear.");
        // Frame-rate caps (v0.1016, operator request: "set background and
        // foreground FPS"). Foreground = window focused; background =
        // alt-tabbed away (e.g. chatting on the website while the game
        // runs). The pacing itself lives in the redraw loop.
        if widgets::toggle(ui, theme, "Unlimited foreground FPS", &mut state.settings.fps_foreground_unlimited) {
            state.settings_dirty = true;
        }
        if !state.settings.fps_foreground_unlimited {
            let mut v = state.settings.fps_foreground as f32;
            if widgets::labeled_slider(ui, theme, "Foreground FPS cap", &mut v, 10.0..=240.0) {
                state.settings.fps_foreground = v.round() as u32;
                state.settings_dirty = true;
            }
        }
        widgets::setting_hint(ui, theme, hint, "Highest frame rate while the game window is active. Unlimited = as fast as VSync and your GPU allow. A lower cap saves power and heat.");
        if widgets::toggle(ui, theme, "Background FPS matches foreground", &mut state.settings.fps_background_sync) {
            state.settings_dirty = true;
        }
        if !state.settings.fps_background_sync {
            let mut v = state.settings.fps_background as f32;
            if widgets::labeled_slider(ui, theme, "Background FPS cap", &mut v, 5.0..=240.0) {
                state.settings.fps_background = v.round() as u32;
                state.settings_dirty = true;
            }
        }
        widgets::setting_hint(ui, theme, hint, "Frame rate while the game is in the background (alt-tabbed). A low cap keeps the world simulating while freeing your GPU and CPU for whatever you switched to.");
        if widgets::labeled_slider(ui, theme, "FOV", &mut state.settings.fov, 60.0..=120.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Field of view: how wide the camera sees, in degrees. Higher = see more around you with a mild fisheye stretch; lower = zoomed in. 90 suits most screens.");
        if widgets::labeled_slider(ui, theme, "Render Distance", &mut state.settings.render_distance, 50.0..=2000.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How far away objects still draw, in meters. Higher = see further but more GPU work / lower FPS.");

        widgets::subsection_header(
            ui, theme, accent,
            "Planets seen from space",
            "Shapes how worlds look from orbit; does nothing while you are on the ground.",
        );
        widgets::setting_hint(ui, theme, hint, "Sky planets subdivide as they grow on screen: one more detail level each time a body's projected size doubles past the pixel threshold. Changes apply live.");
        // Procedural fractal surfaces (oceans, continents, polar caps) vs the
        // old flat single-color spheres. Data lives in data/planets/<id>.ron.
        if widgets::toggle(ui, theme, "Procedural surfaces", &mut state.settings.planet_detail) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Distant planets get oceans, continents, and polar caps instead of flat single-color spheres. Small GPU cost; off is fastest.");
        if widgets::labeled_slider(ui, theme, "LOD pixel threshold (distant planets)", &mut state.settings.planet_lod_px, 4.0..=64.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How many pixels wide a planet must look on screen before it earns its next round of detail. LOWER = planets sharpen while still far away (more GPU); higher = they stay simple longer (faster).");
        // Ceiling raised 7 -> 9 (2026-07-11) for FTL close approaches; the
        // top levels only trigger when one planet fills the screen (see
        // terrain::planet::MAX_SKY_SUBDIVISION for the face/memory table).
        if widgets::labeled_slider(ui, theme, "Max subdivision level (distant planets)", &mut state.settings.planet_max_subdiv, 0.0..=9.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Levels 8-9 add real close-range detail but build big meshes; lower this if a close planet flyby stutters.");
        widgets::subsection_header(
            ui, theme, accent,
            "Ground detail (on a planet)",
            "Drives terrain while you are on or near a surface.",
        );
        // Chunked planetary LOD (2026-07-11): quadtree surface patches that
        // follow the camera once a planet fills the screen, replacing the
        // heavy uniform level 8-9 spheres near heightmap planets (Earth).
        if widgets::toggle(ui, theme, "Chunked surface detail", &mut state.settings.planet_chunked) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Near a planet with real elevation data, surface detail streams in around the camera (down to ~7 m triangles with the tile tier) instead of remeshing the whole globe. Turn off to fall back to uniform spheres.");
        // Geomorph crossfades (v0.920): LOD swaps dissolve instead of pop.
        if widgets::toggle(ui, theme, "Smooth detail transitions", &mut state.settings.terrain_lod_fade) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "When ground detail changes level, the old and new versions crossfade for a third of a second instead of swapping instantly. Costs almost nothing; turn off only to compare or debug.");
        // Planet LOD knobs (v0.873, operator: "I want to see more real
        // terrain further away from me... add settings for all these
        // variables"). All three apply live next frame. (The old space-vs-
        // ground positional hint that sat here is retired: the two
        // subsection headers above now say it structurally.)
        if widgets::labeled_slider(ui, theme, "Terrain sharpness (px per triangle)", &mut state.settings.terrain_split_px, 2.0..=24.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Patches split until triangles are about this many pixels on screen. LOWER = sharper terrain further away (more patches, more GPU).");
        if widgets::labeled_slider(ui, theme, "Terrain patch budget", &mut state.settings.terrain_patch_budget, 256.0..=12288.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Most ground pieces the terrain may keep loaded at once. Higher = detail holds across more of the horizon (more memory + GPU); lower = distant ground goes soft sooner but runs lighter.");
        if widgets::labeled_slider(ui, theme, "Detail draw distance", &mut state.settings.terrain_detail_distance, 0.5..=3.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How far fine surface detail (rock grain, waves, micro texture) stays visible. Higher = crisper distant terrain, more GPU.");
        if widgets::labeled_slider(ui, theme, "Terrain stream speed (builds per frame)", &mut state.settings.terrain_builds_per_frame, 6.0..=64.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How fast terrain refines during a descent. Higher = quicker sharpening, a few ms per frame while streaming.");
        if widgets::toggle(ui, theme, "Tiled light lists (EXPERIMENTAL, higher light counts)", &mut state.settings.lights_tiled) {
            state.settings_dirty = true;
        }
        // ── Detail distances by item type (v0.965, operator: "a settings
        // section dedicated to the LODs of everything organized by type of
        // item") ── every row a live control; categories whose ladder
        // stages have not shipped yet are listed muted rather than given
        // lying sliders (honest-UI rule). Rows and labels come from the
        // LOD category registry (data/vegetation/lod_categories.ron).
        // Vegetation LOD ladder (v0.923, operator: per-stage distance
        // sliders "like LOD0, LOD1, LOD2"). Stage 1 = full 3D models,
        // stage 2 = silhouette cards, then bare terrain. Billboard mid-stage
        // + grass/shrub categories are the next rungs (rungs 2b-2d).
        widgets::subsection_header(ui, theme, accent, "Detail distances by item type", "");
        widgets::setting_hint(ui, theme, hint, "How far each kind of thing keeps its detail. Planet terrain has its own sliders above; more types appear here as their detail stages ship.");
        // v0.1109 (operator: "sliders or maybe just number selectors so we can
        // go to any value ... then I wouldn't have to ask you to increase the
        // ceiling"): every distance here is a slider PLUS a number box. The
        // slider covers the band worth dragging; the box goes all the way to
        // the engine ceiling, so an experiment past the band is a typed number
        // rather than a code edit. Ceilings live in one place
        // (config::TREE_MODEL_MAX_M and friends) precisely so the box, the
        // saved config and the engine's own clamp cannot drift apart again.
        widgets::setting_hint(ui, theme, hint, "Each distance below is a slider plus a number box. Drag the slider for the everyday range, or click the number and type to go past it, right up to the engine's own limit. Values that cost a lot say so underneath.");
        if !VEG_AWAITING_WIRING.is_empty() {
            // Better a blunt notice than a control that quietly does nothing.
            // This block deletes itself the moment the engine reads them; a
            // test fails if it is left standing after that.
            let names: Vec<&str> = VEG_AWAITING_WIRING.iter().map(|(_, label)| *label).collect();
            ui.label(RichText::new(format!(
                "NOT CONNECTED YET: {}. These save and reload correctly and the cost estimates below are real, but the renderer does not read them yet, so moving them will not change what you see.",
                names.join(", ")
            )).color(theme.warning()).size(theme.font_size_small));
        }
        let tree_label = crate::lod_registry::category("tree").map(|c| c.label.as_str()).unwrap_or("Trees");
        if widgets::labeled_slider_entry(ui, theme, &format!("{tree_label}: 3D models within (m)"), &mut state.settings.tree_model_distance, 0.0..=400.0, crate::config::TREE_MODEL_MAX_M, 2.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "The closest, prettiest tree stage: real photoscanned trees stand within this range. 0 turns the stage off (silhouettes only). COST: each model is 120,000 to 190,000 triangles, and doubling this radius quadruples how many trees fall inside it, so this is the most expensive control on the page. It is bounded by the draw budget below, not by itself.");
        if widgets::labeled_slider_entry(ui, theme, &format!("{tree_label}: 3D models drawn at once"), &mut state.settings.near_tree_budget, 32.0..=1024.0, crate::config::NEAR_TREE_BUDGET_MAX, 4.0) {
            state.settings_dirty = true;
        }
        {
            // The honest bit: the budget, not the distance, is what usually
            // ends the model stage, and the engine handles running out
            // GRACEFULLY (it tracks how far models actually reached and hands
            // over to cards exactly there). Say that plainly so a distance that
            // "did nothing" is understood rather than reported as a bug.
            let budget = state.settings.near_tree_budget.max(1.0);
            let tris_m = budget * 155_000.0 / 1.0e6;
            widgets::setting_hint(ui, theme, hint, &format!(
                "How many of those 3D models may be drawn at the same time, nearest to you first. This is the real ceiling on the model stage: if the budget runs out before the distance does, the models form a tight ring around you and silhouette cards carry everything past it. Raise this and the distance together. COST: about {tris_m:.1} million triangles at this budget."
            ));
        }
        if widgets::labeled_slider_entry(ui, theme, &format!("{tree_label}: silhouettes out to (m)"), &mut state.settings.veg_tree_card_m, 100.0..=3000.0, crate::config::TREE_CARD_MAX_M, 10.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "The far tree stage: flat silhouette cards carry the forest from the 3D-model range out to this distance, then trees stop drawing. COST: cards are cheap per tree but there are a lot of them, so this spends fill rate, roughly with the square of the distance.");
        {
            // THE REQUESTED NUMBER NEXT TO THE REAL ONE (v0.1111). Cards are
            // baked into terrain patches at the 215 m detail level, so this
            // slider can only reveal trees as far as the terrain LOD actually
            // built that level - which depends on 'Terrain sharpness' and
            // 'Terrain patch budget', not on this control. The old warning
            // here fired on a fixed 3 km, which was neither the real limit nor
            // even a constant one: at settings that ask for detail the LEAF
            // BUDGET ends the descent first and the reach comes out SHORTER
            // than the shipped 3 km, so the slider could quietly over-promise
            // by kilometres with the warning still silent. The engine measures
            // the reach every selection; print it.
            let requested = state.settings.veg_tree_card_m;
            let reach = crate::terrain::far_trees::measured_card_reach_m();
            if reach.is_finite() {
                let effective =
                    crate::terrain::far_trees::effective_card_far_m(requested, reach);
                widgets::setting_hint(ui, theme, hint, &format!(
                    "Effective right now: {effective:.0} m of the {requested:.0} m asked for. Ground carrying silhouette cards reaches {reach:.0} m from where you are standing, measured on the terrain actually drawn this frame."
                ));
                if effective < requested - 1.0 {
                    ui.label(RichText::new(format!(
                        "ASKING FOR MORE THAN THE GROUND CAN CARRY. Everything between {effective:.0} m and {requested:.0} m gets trees requested and has no ground built at the 215 m detail level to stand them on, so it reads as open field until you walk into it. To buy real distance instead, lower 'Terrain sharpness (px per triangle)' and raise 'Terrain patch budget' until the reach above grows."
                    )).color(theme.warning()).size(theme.font_size_small));
                }
            } else {
                widgets::setting_hint(ui, theme, hint, "Effective right now: not measured yet. The real limit is how far the terrain LOD builds ground at the 215 m detail level, and that is measured while you are standing on a planet; open this page in the world to see it.");
            }
        }
        // TREES, GRASS COVER and GRASS DETAIL are three separate controls
        // (v0.1106). They used to be one "vegetation" slider, which meant a
        // player who wanted thick grass under thin forest could not ask for it,
        // and turning quality down secretly stripped ground cover.
        if widgets::labeled_slider(ui, theme, "Trees: forest density", &mut state.settings.tree_density, 0.1..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How many trees grow per patch of land. 1.0 is dense forest, 0.6 is open woodland. Rebuilds terrain as you move, so the change appears patch by patch.");
        // GRASS DRAW DISTANCE (v0.1109). The operator's headline ask: "I would
        // like to see how the game performs when I extend the grass to render
        // further away." Raising this stretches the density ramp's last leg, so
        // it does not only add a thin outer fringe, it also thickens the middle
        // distance (the fade now has further to travel). Everything under it is
        // a live estimate rather than a static sentence, because the two costs
        // that matter (instance count and harvest walk) both depend on the
        // COVERAGE slider below as well as on this one.
        if widgets::labeled_slider_entry(ui, theme, "Grass: draw distance (m)", &mut state.settings.grass_far_m, 12.0..=80.0, crate::config::GRASS_FAR_MAX_M, 0.5) {
            state.settings_dirty = true;
        }
        {
            let far = state.settings.grass_far_m;
            let cover = state.settings.grass_density;
            let drawn = grass_drawn_estimate(far, cover);
            let harvest = grass_harvest_estimate(far, cover);
            let walk = grass_harvest_walk_multiple(far);
            let cap = state.settings.grass_harvest_cap;
            widgets::setting_hint(ui, theme, hint, &format!(
                "How far grass reaches before it fades out completely. The 6 m and 12 m density steps stay put, so raising this stretches the fade rather than sliding the whole field outward, which thickens the middle distance too. COST: it buys area, so doubling the distance costs roughly two and a half times as many tufts near the default and closer to four times once you are out past 100 m. At {far:.0} m and this ground-cover setting: about {} tufts drawn (roughly {} triangles at full blade detail), harvested from about {} candidates. The harvest runs on the frame thread and its ground walk is a plain disc, so it is exactly {walk:.1}x the cost it has at the default 22 m; that shows up as a hitch every few metres of walking rather than as a lower frame rate.",
                thousands(drawn),
                thousands(drawn * 90.0),
                thousands(harvest),
            ));
            if harvest >= cap {
                // The defect class this whole increment is guarding against: a
                // control that looks like it worked. The harvest walks
                // nearest-first and BREAKS at the cap, so the field ends in a
                // hard circle instead of fading.
                ui.label(RichText::new(format!(
                    "TOO FAR FOR THE CURRENT INSTANCE CAP. About {} tufts are needed but the cap below stops the harvest at {}. The harvest fills nearest-first and then stops, so the grass will end in a hard circle instead of fading out. Raise the instance cap, lower the ground cover, or pull this back to about {:.0} m.",
                    thousands(harvest),
                    thousands(cap),
                    grass_far_within_cap(cap, cover),
                )).color(theme.warning()).size(theme.font_size_small));
            }
        }
        if widgets::labeled_slider_entry(ui, theme, "Grass: instance cap", &mut state.settings.grass_harvest_cap, 50_000.0..=500_000.0, crate::config::GRASS_HARVEST_CAP_MAX, 500.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "The hard ceiling on how many tufts one harvest may produce. It exists so a bad combination of distance and ground cover cannot lock the game up; it is exposed so raising it is your decision rather than a code change. COST: memory and harvest time, both straight-line with the number. Grass stops looking better long before this stops rising.");
        if widgets::labeled_slider(ui, theme, "Grass: ground cover", &mut state.settings.grass_density, 0.1..=3.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How much grass is on the ground. 1.0 is a real lawn or pasture; 3.0 is deep meadow you wade through; below 0.5 the ground starts showing between tufts. This is about how the world LOOKS, not how fast it runs.");
        if widgets::labeled_slider(ui, theme, "Grass: blade detail", &mut state.settings.grass_detail, 0.1..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How finely each tuft of grass is modelled. Turn this DOWN for frames: blades get fewer but wider, so the ground keeps exactly as much grass on it and only the close-up sharpness changes.");
        if widgets::labeled_slider(ui, theme, "Water: wave mesh detail (14-20)", &mut state.settings.water_detail_depth, 14.0..=20.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How finely the water surface is meshed near you: 17 = ~5 m wave vertices, 20 = ~0.6 m (every ripple is real geometry). Only the closest water refines, so the open ocean costs the same at any setting.");
        for cat in crate::lod_registry::categories() {
            if cat.id == "tree" || cat.id == "water" {
                continue; // live sliders above own these
            }
            // v0.971: types whose live controls sit elsewhere say WHERE;
            // types with no shipped stages say so. Nothing is omitted and
            // nothing gets a dead slider (honest-UI rule).
            let line = if cat.controls_note.is_empty() {
                format!("{}: detail stages not shipped yet", cat.label)
            } else {
                format!("{}: {}", cat.label, cat.controls_note)
            };
            ui.label(
                RichText::new(line)
                    .color(theme.text_muted())
                    .size(theme.font_size_small)
                    .italics(),
            );
        }
        widgets::subsection_header(ui, theme, accent, "Light and sky", "");
        // Lighting passes (v0.907): the three surface-lighting features
        // gained user controls. All apply live next frame.
        if widgets::toggle(ui, theme, "Sun shadows", &mut state.settings.sun_shadows) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Terrain, plants, and structures cast real shadows from the sun. Off = flatter light, a little more FPS.");
        if widgets::labeled_slider(ui, theme, "Shadow strength", &mut state.settings.shadow_strength, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How dark a full shadow gets. 1 = realistic (no direct sun in shadow, only sky light, which reads cool and blue). Lower leaks warm sunlight into shadows and flattens the scene.");
        if widgets::labeled_slider(ui, theme, "Aerial haze strength", &mut state.settings.aerial_strength, 0.0..=2.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How strongly distant hills and sea fade into the sky, like real air does. 0 = crystal-clear air (off), 1 = earthlike, 2 = misty. Almost free on the GPU.");
        if widgets::labeled_slider(ui, theme, "God ray intensity", &mut state.settings.godray_intensity, 0.0..=1.5) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Visible light shafts when facing the sun through clouds or terrain gaps. 0 turns the pass off.");
        if widgets::labeled_slider(ui, theme, "Ambient occlusion", &mut state.settings.ssao_strength, 0.0..=1.5) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Soft contact shading in crevices and where objects meet the ground. 0 turns the pass off.");
        // Analytic scattering atmosphere (v0.807): per-pixel single
        // scattering on the planet air shells. Off = the pre-v0.807 fresnel
        // tint, kept forever-dev style as the A/B reference + a safety hatch
        // for GPUs that dislike the math. Applies live: the material is
        // rebuilt (cached per mode) the next time the shell draws.
        if widgets::toggle(ui, theme, "Scattering atmosphere", &mut state.settings.planet_atmo_scatter) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Physically shaded planet air: blue limb from orbit, warm terminator, pale horizon from inside the atmosphere. Turn off for the simple tinted-shell look.");
        // Animated procedural cloud deck (clouds increment 1): a second
        // translucent shell under the atmosphere, on planets whose RON
        // declares cloud_coverage. Applies live: off skips the draw next
        // frame; on reuses the cached material.
        if widgets::toggle(ui, theme, "Cloud layer", &mut state.settings.planet_clouds) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Drifting sun-lit clouds on worlds that have them (Earth). Turn off for bare surfaces or on very old GPUs.");
        // Live weather (v0.874): NASA GIBS MODIS cloud fraction placed on
        // the game sky. Fetcher spawns once per session; the toggle gates
        // both the spawn and per-frame uploads (turning it off mid-session
        // freezes the last map until restart -- fine, it is real data).
        if state.settings.planet_clouds {
            if widgets::toggle(ui, theme, "Live weather (Earth)", &mut state.settings.live_weather) {
                state.settings_dirty = true;
            }
            widgets::setting_hint(ui, theme, hint, "Places the game's clouds where real clouds are right now, from NASA's daily satellite cloud map. Needs internet once; the last map is kept for offline play. Off = purely procedural skies.");
        }
        // Cloud quality ladder (clouds increment 3). Applies live: the cloud
        // material is cached per (body, quality), so flipping tiers rebuilds
        // it the next frame the deck draws. Sits beside the other cloud
        // controls so the conditional cloud group reads as one block.
        if state.settings.planet_clouds {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Cloud quality").color(theme.text_secondary()));
                for (val, label) in [
                    ("low", "Low"),
                    ("medium", "Medium"),
                    ("high", "High"),
                    ("ultra", "Ultra"),
                ] {
                    let sel = state.settings.cloud_quality == val;
                    if ui.selectable_label(sel, RichText::new(label).size(theme.font_size_small)).clicked() && !sel {
                        state.settings.cloud_quality = val.to_string();
                        state.settings_dirty = true;
                    }
                }
            });
            widgets::setting_hint(ui, theme, hint, "Ultra builds cloud bodies from primitives (flat bases, cauliflower crowns, real size variety) instead of carving them from noise - in development. High raymarches 3D cloud shapes with sunlight scattering. Medium is the lighter layered march; Low is a flat painted deck for weak GPUs.");
        }
        // Close-range surface detail (v0.816): animated ocean waves + land
        // micro-texture on planets with real imagery. Applies live: the sky
        // loop rewrites the material flag every frame.
        if widgets::toggle(ui, theme, "Surface detail", &mut state.settings.planet_surface_detail) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Up close, oceans get moving waves and sun sparkle and land keeps revealing texture as you descend. The view from orbit is identical either way. Turn off on very old GPUs.");
        // Underwater clarity (v0.1054). Deliberately a slider rather than a
        // toggle: the operator wants the physical fade AND the ability to keep
        // seeing far underwater for exploration, and the honest way to offer
        // both is to let them pick where on that line to sit. Lives under
        // Light and sky because it is literally how light dies in water, and
        // it is a shipped control, not an experimental one.
        if widgets::labeled_slider(
            ui,
            theme,
            "Underwater clarity",
            &mut state.settings.water_clarity,
            0.0..=1.0,
        ) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How far you can see underwater. 0 is physical: real seawater absorbs red within a few metres, then green, so the world goes blue and then black as you descend. 1 keeps the old unlimited visibility, which is far better for finding places like Challenger Deep.");

        // ── Sky / map lines (v0.786, operator sky settings) ──
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Sky / map lines").color(theme.text_secondary()).strong());
        widgets::setting_hint(
            ui,
            theme,
            hint,
            "The line overlays in the night sky. Colors live in Appearance \
             (Sky: orbit lines / Sky: constellation lines). Vessel orbits, \
             collision-course flags, and selected-object modes arrive as \
             those systems come online.",
        );
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
        widgets::setting_hint(ui, theme, hint, "The soft glowing band of our galaxy behind the stars, baked from the real star catalog. Intensity: 0 = invisible, 1 = natural, 2 = doubled brightness. Changes apply live.");
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
                    if status.starts_with("FAILED") {
                        // Render a failure as a plain danger-colored line, NOT white text
                        // on the filled orange progress bar (which is hard to read,
                        // especially with astigmatism). Operator feedback 2026-07-15.
                        ui.label(
                            RichText::new(format!("Ultra glow download {}", status.to_lowercase()))
                                .color(theme.danger())
                                .size(theme.font_size_small),
                        );
                    } else {
                        let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                        ui.add(egui::ProgressBar::new(frac).text(format!(
                            "Ultra glow: {} ({} / {} MB)",
                            status,
                            done / 1_048_576,
                            total.max(1) / 1_048_576
                        )));
                    }
                }
            }
            if state.galaxy_glow_installed.is_some() && state.galaxy_glow_dl.is_none() {
                if ui.button("Remove ultra glow texture").clicked() {
                    state.galaxy_glow_remove = true;
                }
            }
            widgets::setting_hint(ui, theme, hint, "Ultra is a sharper 16384x8192 bake of the same catalog light. Uses about 512 MB of GPU memory; applies next time you enter the world.");
        }
        // Star halos (2026-07-11): soft photographic glow + a faint 4-point
        // diffraction cross on the ~50 brightest stars (mag <= 2), drawn
        // additively over the star points. A plain visibility flag on the
        // star renderer - applies live, nothing to rebuild.
        if widgets::toggle(ui, theme, "Star halos", &mut state.settings.sky_star_halos) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "A soft long-exposure-photo glow around the brightest stars (Sirius, Vega, Rigel...). Applies live.");

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
            widgets::setting_hint(ui, theme, hint, "Caps which catalog loads. Auto uses the biggest installed; Standard forces the fast 120k catalog. Applies next world entry.");
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

        // ── Experimental: default-off features kept for A/B and early access ──
        widgets::subsection_header(ui, theme, accent, "Experimental", "");
        // FFT ocean (v0.1029, water-fft.md increment 1). Applies live: the
        // spectrum builds on first use; the mode flag rides the per-frame
        // uniform so geometry and buoyancy flip together.
        if widgets::toggle(ui, theme, "FFT ocean (experimental)", &mut state.settings.water_fft) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Replaces the hand-tuned chop waves with a real oceanographic wave spectrum (thousands of simultaneous waves). Early version: same energy, richer structure. Off = the shipped wave look.");
        // GPU particle simulation (v0.1068). Same shape as the FFT-ocean
        // toggle: experimental, default off, CPU path stays as the fallback.
        if widgets::toggle(ui, theme, "GPU particles (experimental)", &mut state.settings.gpu_particles) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Simulates rain and snow entirely on the GPU: the pool lives in video memory and the CPU submits one dispatch instead of moving every particle across the bus. The CPU path was measured bandwidth-bound at ~17-20 ns per particle per frame, which capped it near 160k. Off = that shipped CPU path.");
        // Far-tree card sheet (v0.1022, default off since v0.1029): kept
        // for A/B against the upcoming impostor system.
        if widgets::toggle(ui, theme, "Far tree sheet (experimental)", &mut state.settings.far_tree_sheet) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Draws distant forests as coarse canopy cards out to the horizon. Known issue: reads as dark squares from high altitude, which is why it is off by default until the proper long-range tree system lands.");

        // ── Machine labels and Home view ──
        widgets::subsection_header(ui, theme, accent, "Machine labels and Home view", "");
        ui.label(RichText::new("Machine label distances (m)").color(theme.text_secondary()).strong());
        widgets::setting_hint(ui, theme, hint, "How close (in meters) you must be before a machine shows its dot / name / info card. Higher = labels appear from further away, busier screen. Hold Tab in-game to triple these and see through walls. Session-only for now: they reset to defaults on restart.");
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
        widgets::setting_hint(ui, theme, hint, "Off shows the sky (stars + the real solar system) through the open top; on seals the home for an interior / atmosphere look.");
        // Hull wrap (ship-superstructure increment D): the generated exterior shell around the
        // zone cluster (data/blueprints/hull_profile.ron). Default ON; also toggled with H.
        widgets::toggle(ui, theme, "Show hull (H)", &mut state.show_hull);
        widgets::setting_hint(ui, theme, hint, "The generated exterior hull around the ship's zones (open above glass roofs, so gardens keep their starlight). Off for unobstructed interior or top-down build views.");
    });
}

/// Settings > Gameplay (v0.791): survival tuning + which home design loads.
/// Born from an operator field report ("disable the wolves... extend the
/// dehydration time. I keep getting killed and its annoying").
pub(crate) fn draw_gameplay_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    // ── Play mode (task #50): Normal | Creative | Dev ──
    // The one ladder every cheat/scope gate hangs off; see
    // crate::config::PlayMode for the full design + tested truth table.
    // Applies LIVE: the gates (construction scope, Dev page, creative flag,
    // fly/FTL) read the mode per frame, no world reload needed.
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Play mode").color(theme.text_secondary()).strong());
        widgets::setting_hint(
            ui,
            theme,
            hint,
            "Who gets which powers. Applies immediately: building scope, \
             the Dev page, and free materials all follow the mode. When \
             the mode is not Normal, a CREATIVE / DEV tag shows on the \
             HUD so screenshots stay honest.",
        );
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
            widgets::setting_hint(ui, theme, hint, mode.hint());
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
        widgets::setting_hint(
            ui,
            theme,
            hint,
            "Wolf packs and other predators in the wilds. Off removes them \
             immediately; turning it on repopulates next time you enter the \
             world. The Dev spawn page can always place any creature.",
        );
        if widgets::labeled_slider(ui, theme, "Vitals drain", &mut state.settings.vitals_drain, 0.0..=3.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(
            ui,
            theme,
            hint,
            "How fast hunger, thirst, and energy fall. 1.0 = normal (about \
             half an hour from full to empty), 0 = survival needs paused.",
        );

        ui.add_space(theme.spacing_lg);
        // Household size (2026-07-01, moved here from Data in v0.791): which home design
        // data/machines/*.ron loads. Two real, fully-authored designs exist -- the default
        // family-scale home.ron and a one-person self-sufficient design (home_solo.ron,
        // see docs/design/homestead-solo-design.md) sized to real one-person kWh/L/kcal
        // needs. GUI-first per the project's own rule.
        ui.label(RichText::new("Home Design").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        widgets::setting_hint(ui, theme, hint, "Which pre-built homestead loads. Takes effect next time you enter the world (restart HumanityOS to apply immediately).");
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
    use crate::input::bindings as kb;
    let hint = state.settings.hint_display;
    widgets::card(ui, theme, |ui| {
        // Range max 1.0 keeps the slider in the usable band AND selects the widget's
        // 2-decimal display (max <= 1.0), so a low value like 0.11 is visible and tunable
        // instead of rounding to "0.1". 1.0 here is a fast 0.01 rad per mouse-pixel.
        if widgets::labeled_slider(ui, theme, "Mouse Sensitivity", &mut state.settings.mouse_sensitivity, 0.02..=1.0) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "How fast the camera turns when you move the mouse. Lower = steadier, more precise aim; higher = faster turns.");
        if widgets::toggle(ui, theme, "Invert Y-Axis", &mut state.settings.invert_y) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "On = pushing the mouse forward looks down, like an aircraft stick. Off = forward looks up.");

        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Keybinds").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        widgets::setting_hint(
            ui,
            theme,
            hint,
            "Click a key to rebind it: press the new key to bind, Esc cancels, Delete clears \
             the slot. Each key can drive only one action; binding a taken key asks before \
             moving it.",
        );

        // One-line status from the last capture attempt ("Function keys are
        // reserved...", the voice-key overlap note). Cleared by the next
        // successful capture or cancel.
        if !state.keybind_status.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(state.keybind_status.clone())
                    .size(theme.font_size_small)
                    .color(theme.warning()),
            );
        }

        // Conflict confirm (the capture found the key on another action):
        // name both sides and let the user move the key or keep it.
        if let Some((c_action, c_secondary, c_key, c_holder)) = state.keybind_conflict.clone() {
            ui.add_space(theme.spacing_xs);
            egui::Frame::none()
                .fill(theme.bg_tertiary())
                .rounding(Rounding::same(4))
                .inner_margin(Vec2::new(10.0, 8.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} is already bound to {}.",
                            kb::pretty_key_name(&c_key),
                            kb::Keybinds::info(c_holder).label
                        ))
                        .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        let move_label = format!(
                            "Move it to {}",
                            kb::Keybinds::info(c_action).label
                        );
                        if widgets::compact_button(ui, theme, &move_label, widgets::ButtonVariant::Primary) {
                            state.keybinds.force_bind(c_action, c_secondary, &c_key);
                            state.keybind_conflict = None;
                            state.settings_dirty = true;
                        }
                        if widgets::compact_button(ui, theme, "Cancel", widgets::ButtonVariant::Secondary) {
                            state.keybind_conflict = None;
                        }
                    });
                });
        }

        // The bind grid, grouped by category. Four LEFT-aligned columns
        // [Action | Primary | Secondary | Reset] so each key sits right next
        // to its action (the far-right layout was flagged hard to match up).
        // The key cells are BUTTONS: clicking one arms capture for that slot.
        for cat in kb::CATEGORIES {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new(*cat)
                    .color(theme.text_muted())
                    .size(theme.font_size_small)
                    .strong(),
            );
            egui::Grid::new(format!("keybinds_grid_{cat}"))
                .num_columns(4)
                .spacing([24.0, theme.row_gap])
                .show(ui, |ui| {
                    ui.label(RichText::new("Action").color(theme.text_muted()).size(theme.font_size_small));
                    ui.label(RichText::new("Primary").color(theme.text_muted()).size(theme.font_size_small));
                    ui.label(RichText::new("Secondary").color(theme.text_muted()).size(theme.font_size_small));
                    ui.label(RichText::new("").size(theme.font_size_small));
                    ui.end_row();
                    for info in kb::ACTIONS.iter().filter(|i| i.category == *cat) {
                        ui.label(RichText::new(info.label).color(theme.text_secondary()));
                        for secondary in [false, true] {
                            let capturing =
                                state.keybind_capture == Some((info.action, secondary));
                            let (p, s) = state.keybinds.pair(info.action);
                            let raw = if secondary { s } else { p };
                            let text = if capturing {
                                "Press a key...".to_string()
                            } else {
                                kb::pretty_key_name(raw)
                            };
                            let (fill, color) = if capturing {
                                (theme.accent(), theme.text_on_accent())
                            } else if raw.is_empty() {
                                (theme.bg_tertiary(), theme.text_muted())
                            } else {
                                (theme.bg_tertiary(), theme.text_primary())
                            };
                            let btn = egui::Button::new(
                                RichText::new(text)
                                    .color(color)
                                    .size(theme.font_size_small)
                                    .strong(),
                            )
                            .fill(fill)
                            .rounding(Rounding::same(3));
                            let resp = ui.add(btn);
                            // Only a real POINTER click toggles the cell. egui
                            // also reports clicked() when Space/Enter activates
                            // the focused button, but while capturing those
                            // keystrokes are capture INPUT (the raw handler
                            // binds them), not a toggle request; without this
                            // gate, binding Space would instantly re-arm the
                            // cell it just closed.
                            if resp.clicked() && ui.input(|i| i.pointer.any_click()) {
                                if capturing {
                                    // Clicking the armed cell disarms it.
                                    state.keybind_capture = None;
                                } else {
                                    state.keybind_capture = Some((info.action, secondary));
                                    state.keybind_conflict = None;
                                    state.keybind_status.clear();
                                }
                            }
                            if capturing {
                                // Keep egui focus off the armed cell so the
                                // captured key can never double as a button
                                // activation.
                                resp.surrender_focus();
                            }
                        }
                        if state.keybinds.is_default(info.action) {
                            // Blank cell keeps the grid aligned.
                            ui.label(RichText::new("").size(theme.font_size_small));
                        } else if widgets::compact_button(ui, theme, "Reset", widgets::ButtonVariant::Secondary) {
                            state.keybinds.reset(info.action);
                            state.keybind_capture = None;
                            state.settings_dirty = true;
                        }
                        ui.end_row();
                    }
                });
        }

        ui.add_space(theme.spacing_sm);
        if widgets::Button::ghost("Reset all binds to defaults").show(ui, theme) {
            state.keybinds.reset_all();
            state.keybind_capture = None;
            state.keybind_conflict = None;
            state.keybind_status.clear();
            state.settings_dirty = true;
        }
    });

    // Fixed shortcuts: real, discoverable, deliberately not rebindable (dev
    // and diagnostic surface, shortcut-modifier combos, capture controls).
    // Sourced from input::bindings::FIXED_BINDS so this list and the audit
    // stay one source of truth.
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Fixed keys").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        widgets::setting_hint(
            ui,
            theme,
            hint,
            "These shortcuts are built in and cannot be rebound yet. Hold F1 in any screen to \
             see the keys that work there.",
        );
        egui::Grid::new("fixed_keys_grid")
            .num_columns(2)
            .spacing([24.0, theme.row_gap])
            .show(ui, |ui| {
                for (keys, what) in kb::FIXED_BINDS {
                    egui::Frame::none()
                        .fill(theme.bg_tertiary())
                        .rounding(Rounding::same(3))
                        .inner_margin(Vec2::new(8.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(*keys)
                                    .color(theme.text_primary())
                                    .size(theme.font_size_small)
                                    .strong(),
                            );
                        });
                    ui.label(
                        RichText::new(*what)
                            .color(theme.text_secondary())
                            .size(theme.font_size_small),
                    );
                    ui.end_row();
                }
            });
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(
                "Voice push-to-talk key: set under Settings > Audio > Voice (default CapsLock).",
            )
            .size(theme.font_size_small)
            .color(theme.text_muted()),
        );
    });
}

pub(crate) fn draw_privacy_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    // Privacy tiers (2026-08-23): the preset chooser + the server-enforced
    // presence switch live in pages::privacy (shared with the first-connect
    // modal). The old standalone "Show Online Status" toggle merged into it
    // — it used to be written to config and read by NOTHING; the same field
    // now drives the relay's privacy_update.
    widgets::card(ui, theme, |ui| {
        crate::gui::pages::privacy::draw_privacy_content(ui, theme, state);
    });
    ui.add_space(theme.spacing_sm);
    widgets::card(ui, theme, |ui| {
        if widgets::toggle(ui, theme, "Profile Visible to Others", &mut state.settings.profile_visible) {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, "Whether other players can open and view your profile page.");
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
    let hint = state.settings.hint_display;
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
        widgets::setting_hint(ui, theme, hint, "Export your data for backup or migration. (These buttons are not wired up yet; use the Open buttons above to copy files by hand.)");

        ui.horizontal(|ui| {
            let _ = widgets::secondary_button(ui, theme, "Export Profile Data");
            let _ = widgets::secondary_button(ui, theme, "Export Save Data");
        });

        ui.add_space(theme.spacing_lg);

        ui.label(RichText::new("Cache").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        widgets::setting_hint(ui, theme, hint, "Clear cached data to free disk space. (Not wired up yet.)");
        let _ = widgets::secondary_button(ui, theme, "Clear Cache");

        ui.add_space(theme.spacing_lg);

        ui.label(RichText::new("Danger Zone").color(theme.danger()).strong());
        ui.add_space(theme.spacing_xs);
        widgets::setting_hint(ui, theme, hint, "Permanently delete your account and all associated data. (Not wired up yet; deleting your identity means removing your seed and data folders, see the paths above.)");
        let _ = widgets::danger_button(ui, theme, "Delete Account");
    });
}

pub(crate) fn draw_updates_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
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
        widgets::setting_hint(ui, theme, hint, "Always Latest looks for new releases at launch and on Check Now. Disabled stops all checking. This choice lasts for the current session only; it returns to Always Latest on restart.");

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
                    .fill(theme.bg_sidebar_dark())
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

// Vegetation LOD cost model (v0.1109).
//
// Every control in "Detail distances by item type" trades frames for view
// distance, and the operator changes them specifically to measure that trade
// (2026-08-03: "I would like to see how the game performs when I extend the
// grass to render further away"). A slider with no cost attached makes that a
// guessing game, and worse, two of these ranges run into HARD ENGINE CAPS that
// truncate silently. So the page prints an estimate and warns before the cap,
// and these are the functions behind it.
//
// The model is not a guess: it integrates the SAME piecewise-linear density
// ramp the harvest uses (terrain::grass::grass_ramp_at) over the disc, and it
// is calibrated against the two numbers the shipped code already reports.
// See `cost_model_matches_the_shipped_measurements` below.

// ── What the engine actually honours, right now ────────────────────────────
// A control that moves, saves, reloads and changes NOTHING is the worst
// outcome available here, and it is the outcome this repo keeps producing: the
// tree-model slider stopped at 300 m while the engine allowed 400, and nobody
// noticed for a year. The Settings page therefore states, in the UI, exactly
// where the engine's own limit sits, and the constants below are checked
// against `src/lib.rs` by `the_page_tells_the_truth_about_what_the_engine_reads`
// so they cannot rot the moment the engine changes.
//
// These live in settings.rs rather than config.rs on purpose: config.rs owns
// what the app WANTS to allow (the ceilings), this owns what the engine
// currently DOES. When the two agree, everything below disappears.

/// Vegetation LOD settings the engine does not read at all yet, with the UI
/// label to name in the warning. They save and reload correctly; they simply do
/// not reach the renderer until their consumer lands in `src/lib.rs` and
/// `src/terrain/grass.rs`. Empty this list in the same change that wires them.
const VEG_AWAITING_WIRING: &[(&str, &str)] = &[];


/// Realised filler-class population as a fraction of the tussock population.
///
/// `GRASS_FILLER_FRACTION` (9.5/45) is the NOMINAL ratio, but the filler class
/// rides the COMPLEMENT of the clump field and that complement averages 0.615
/// over the field, so the count that actually lands is the product. Both
/// numbers are stated in the doc comments on `terrain::grass`
/// (`GRASS_FILLER_LAI_SHARE` names the 0.615; `GRASS_FILLER_FRACTION` states
/// the realised result is "near 13% of the tussock population"). This is a UI
/// estimate, so approximating the field mean by its documented constant is
/// fine; the calibration test below is what keeps it honest.
const GRASS_CLUMP_COMPLEMENT_MEAN: f32 = 0.615;

/// Peak-density-equivalent ground AREA in m^2 under a three-anchor grass
/// density ramp: full density inside `near`, falling linearly to
/// `GRASS_MID_FRACTION` of peak at `mid`, then linearly to zero at `far`.
///
/// Multiply by a peak density (tillers per m^2) to get an instance count.
/// Closed form because the ramp is piecewise linear, which makes this exact
/// rather than a sampled sum.
pub fn grass_ramp_area_m2(near: f32, mid: f32, far: f32) -> f32 {
    let frac = crate::terrain::grass::GRASS_MID_FRACTION;
    let n = near.max(0.0);
    let m = mid.max(n + 0.001);
    let f = far.max(m + 0.001);
    let tau = std::f32::consts::TAU;
    // Inner disc, all at peak.
    let a1 = std::f32::consts::PI * n * n;
    // near..mid, density = 1 + s(d - n): integral of that times 2*pi*d.
    let s = (frac - 1.0) / (m - n);
    let a2 = tau * ((1.0 - s * n) * (m * m - n * n) / 2.0 + s * (m * m * m - n * n * n) / 3.0);
    // mid..far, density = frac + s2(d - m), reaching zero at f.
    let s2 = -frac / (f - m);
    let a3 = tau * ((frac - s2 * m) * (f * f - m * m) / 2.0 + s2 * (f * f * f - m * m * m) / 3.0);
    a1 + a2 + a3
}

/// Peak tiller density (per m^2 of ground) at a given COVERAGE setting.
///
/// Mirrors `terrain::grass::grass_peak_per_m2` but takes coverage as an
/// argument instead of reading the live atomic, so the Settings page can cost
/// a value the player is still dragging (the atomic is only written by the
/// render loop, one frame later).
pub fn grass_peak_at_cover(cover: f32) -> f32 {
    use crate::terrain::grass::{
        GRASS_FILLER_LAI_SHARE, GRASS_LEAF_AREA_UNIT, GRASS_MEAN_H2_M2, GRASS_TARGET_LAI,
    };
    GRASS_TARGET_LAI * cover
        / (GRASS_LEAF_AREA_UNIT * GRASS_MEAN_H2_M2 * (1.0 + GRASS_FILLER_LAI_SHARE))
}

/// Estimated grass instances DRAWN at a given draw distance and coverage:
/// tussocks plus the realised filler class.
pub fn grass_drawn_estimate(far_m: f32, cover: f32) -> f32 {
    use crate::terrain::grass::{GRASS_FILLER_FRACTION, GRASS_MID_M, GRASS_NEAR_M};
    let filler = 1.0 + GRASS_FILLER_FRACTION * GRASS_CLUMP_COMPLEMENT_MEAN;
    grass_peak_at_cover(cover) * filler * grass_ramp_area_m2(GRASS_NEAR_M, GRASS_MID_M, far_m)
}

/// Estimated grass instances the CPU HARVEST emits, which is the number the
/// instance cap actually bounds.
///
/// The harvest is a SUPERSET: it keeps every tiller that any camera within
/// `GRASS_HARVEST_MARGIN_M` of the harvest centre could want, so the ramp is
/// SHIFTED outward by the margin (all three anchors move), not stretched.
/// Getting that distinction right is worth roughly a factor of two: at the
/// shipped settings the stretched model predicts ~18k and the shifted model
/// ~31k, and the engine logs ~31k.
pub fn grass_harvest_estimate(far_m: f32, cover: f32) -> f32 {
    use crate::terrain::grass::{
        GRASS_FILLER_FRACTION, GRASS_HARVEST_MARGIN_M, GRASS_MID_M, GRASS_NEAR_M,
    };
    let margin = GRASS_HARVEST_MARGIN_M as f32;
    let filler = 1.0 + GRASS_FILLER_FRACTION * GRASS_CLUMP_COMPLEMENT_MEAN;
    grass_peak_at_cover(cover)
        * filler
        * grass_ramp_area_m2(GRASS_NEAR_M + margin, GRASS_MID_M + margin, far_m + margin)
}

/// Relative cost of the harvest's CELL WALK against the shipped 22 m default.
///
/// The walk covers a disc of radius `far_m + margin`, so its cost is that
/// radius squared. Reported as a multiple because the absolute figure depends
/// on the machine, but the shipped one is measured (about 16 ms at Fuji), and
/// the walk runs INLINE on the frame thread once per few metres of movement,
/// which is why a big number here reads as a stutter rather than a lower FPS.
pub fn grass_harvest_walk_multiple(far_m: f32) -> f32 {
    let margin = crate::terrain::grass::GRASS_HARVEST_MARGIN_M as f32;
    let base = crate::terrain::grass::GRASS_FAR_M + margin;
    let r = far_m.max(0.0) + margin;
    (r * r) / (base * base).max(1.0)
}

/// The largest grass draw distance whose harvest still fits under `cap` at the
/// given coverage, in metres.
///
/// This is the number the warning actually needs: telling somebody "too far"
/// without telling them how far is fine is half an answer. Bisection rather
/// than an inverse because the ramp integral is a cubic in the far distance and
/// a closed-form root brings a branch-and-sign problem for no benefit; twenty
/// halvings of a 250 m range settle to well under a millimetre and this runs
/// only while the warning is on screen.
pub fn grass_far_within_cap(cap: f32, cover: f32) -> f32 {
    let mid_m = crate::terrain::grass::GRASS_MID_M;
    let mut lo = mid_m + 1.0;
    let mut hi = crate::config::GRASS_FAR_MAX_M;
    if grass_harvest_estimate(lo, cover) >= cap {
        return lo;
    }
    if grass_harvest_estimate(hi, cover) < cap {
        return hi;
    }
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        if grass_harvest_estimate(mid, cover) < cap {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Format an instance count for a help line: 12,755 reads better than 12755
/// and much better than 1.2755e4.
fn thousands(v: f32) -> String {
    let n = v.max(0.0).round() as u64;
    let s = n.to_string();
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in b.iter().enumerate() {
        if i > 0 && (b.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

#[cfg(test)]
mod veg_lod_range_tests {
    use super::*;
    use crate::config;
    use crate::terrain::grass;

    /// The cost model has to predict the two numbers the shipped code already
    /// reports, or every warning built on it is decoration.
    ///
    /// Both references come from the engine's own comments/logs at the shipped
    /// settings (22 m ramp, coverage 1.0): `grass.rs` states the ramp draws
    /// 12,755 TUSSOCKS, and it states the harvest superset is about 31,000
    /// INSTANCES. Those are different populations on purpose (drawn tussocks
    /// vs harvested tussocks + filler), which is exactly the pair that catches
    /// a model that got the filler class or the margin shift wrong.
    #[test]
    fn cost_model_matches_the_shipped_measurements() {
        let cover = 1.0;
        // Peak density: the file derives 27.0 per m^2 at coverage 1.0.
        let peak = grass_peak_at_cover(cover);
        assert!(
            (peak - 27.0).abs() < 0.2,
            "peak density {peak} should be the derived 27.0/m^2"
        );
        // Drawn TUSSOCKS = peak * ramp area, no filler.
        let tussocks =
            peak * grass_ramp_area_m2(grass::GRASS_NEAR_M, grass::GRASS_MID_M, grass::GRASS_FAR_M);
        assert!(
            (tussocks - 12_755.0).abs() / 12_755.0 < 0.02,
            "drawn tussock estimate {tussocks} should be within 2% of the \
             12,755 stated in terrain::grass"
        );
        // Harvested SUPERSET, tussocks + filler, ramp shifted by the margin.
        let harvest = grass_harvest_estimate(grass::GRASS_FAR_M, cover);
        assert!(
            (harvest - 31_000.0).abs() / 31_000.0 < 0.10,
            "harvest estimate {harvest} should be within 10% of the ~31,000 \
             superset terrain::grass reports at the shipped settings"
        );
    }

    /// The whole point of the increment: every new range must reach values the
    /// shipped constants cannot express. A control that can only say what the
    /// code already said is a control that does nothing.
    #[test]
    fn every_range_reaches_past_the_shipped_constant() {
        assert!(
            config::GRASS_FAR_MAX_M > grass::GRASS_FAR_M,
            "grass draw distance must reach past the shipped {} m ramp end, \
             otherwise 'extend the grass further' is unreachable",
            grass::GRASS_FAR_M
        );
        // And past it by enough to be an experiment, not a rounding error: the
        // area cost model is only interesting once the radius has doubled.
        assert!(
            config::GRASS_FAR_MAX_M >= grass::GRASS_FAR_M * 4.0,
            "grass draw distance ceiling {} m barely moves off the shipped {} m",
            config::GRASS_FAR_MAX_M,
            grass::GRASS_FAR_M
        );
        assert!(
            config::NEAR_TREE_BUDGET_MAX > config::NEAR_TREE_BUDGET_DEFAULT,
            "the near-tree draw budget ceiling must exceed the shipped 256, or \
             raising the tree distance still just packs the same models tighter"
        );
        assert!(
            config::GRASS_HARVEST_CAP_MAX > config::GRASS_HARVEST_CAP_DEFAULT,
            "the grass instance cap must be raisable past the shipped 200,000, \
             or the operator still has to ask for a code edit to lift it"
        );
        // The two PRE-EXISTING controls had ceilings below their own code
        // clamps: the tree-model slider stopped at 300 m while config and
        // lib.rs both allowed 400 m.
        assert!(
            config::TREE_MODEL_MAX_M > 400.0,
            "tree model distance ceiling must exceed the old 400 m code clamp"
        );
        assert!(
            config::TREE_CARD_MAX_M > 3000.0,
            "tree card ceiling must exceed the old 3000 m code clamp"
        );
    }

    /// A gate that cannot fire is not a gate. The instance-cap warning has to
    /// be REACHABLE inside the range the operator can actually set, and it has
    /// to be QUIET at the shipped defaults, or it is either decoration or
    /// noise. This asserts both ends.
    #[test]
    fn the_instance_cap_warning_is_reachable_and_quiet_by_default() {
        let cap = config::GRASS_HARVEST_CAP_DEFAULT;
        // Quiet where it ships.
        let shipped = grass_harvest_estimate(grass::GRASS_FAR_M, 1.0);
        assert!(
            shipped < cap * 0.5,
            "the shipped settings ({shipped} instances) must sit well under the \
             {cap} cap, or the warning cries wolf on a fresh install"
        );
        // Reachable: at deep-meadow coverage the cap binds well inside the
        // range the number box accepts. Find where.
        let mut binds_at = None;
        let mut d = grass::GRASS_FAR_M;
        while d <= config::GRASS_FAR_MAX_M {
            if grass_harvest_estimate(d, 3.0) >= cap {
                binds_at = Some(d);
                break;
            }
            d += 1.0;
        }
        let binds_at = binds_at.expect(
            "the 200,000 instance cap must be reachable by raising the grass \
             draw distance inside its range, otherwise the warning is dead code",
        );
        assert!(
            binds_at < config::GRASS_FAR_MAX_M * 0.5,
            "the cap should bind in the middle of the usable range, not at its \
             extreme; measured {binds_at} m at coverage 3.0"
        );
    }

    /// Raising a ceiling is worthless if the value cannot survive a restart:
    /// `apply_to_gui_state` used to clamp to local literals (400 m, 3000 m), which
    /// silently reverted anything typed above them on the next launch.
    #[test]
    fn values_past_the_old_clamps_survive_a_config_round_trip() {
        let mut cfg = config::AppConfig::default();
        cfg.tree_model_distance = 1200.0;
        cfg.veg_tree_card_m = 6000.0;
        cfg.grass_far_m = 120.0;
        cfg.near_tree_budget = 2048.0;
        cfg.grass_harvest_cap = 900_000.0;
        let mut state = crate::gui::GuiState::default();
        cfg.apply_to_gui_state(&mut state);
        assert_eq!(state.settings.tree_model_distance, 1200.0);
        assert_eq!(state.settings.veg_tree_card_m, 6000.0);
        assert_eq!(state.settings.grass_far_m, 120.0);
        assert_eq!(state.settings.near_tree_budget, 2048.0);
        assert_eq!(state.settings.grass_harvest_cap, 900_000.0);
        // And the ceilings still bite: nothing is unbounded.
        let mut wild = config::AppConfig::default();
        wild.grass_far_m = 1.0e9;
        wild.apply_to_gui_state(&mut state);
        assert_eq!(state.settings.grass_far_m, config::GRASS_FAR_MAX_M);
    }

    /// The help text makes two SEPARATE cost claims and they scale
    /// differently, which is exactly the sort of thing that gets flattened
    /// into one wrong sentence:
    ///
    /// - the HARVEST WALK covers a plain disc, so it is exactly quadratic;
    /// - the INSTANCE COUNT integrates a ramp that falls to zero at the far
    ///   edge, so a doubling costs about 2.6x at 30 to 60 m and only
    ///   approaches 4x once the outer leg dominates the disc.
    ///
    /// The first draft of the help text said "square of the number" for both.
    /// This test is why it does not any more.
    #[test]
    fn grass_cost_is_quadratic_in_the_walk_and_approaches_it_in_the_count() {
        // The walk: exactly quadratic, and exactly 1.0x where it ships.
        let walk = grass_harvest_walk_multiple(grass::GRASS_FAR_M);
        assert!(
            (walk - 1.0).abs() < 1.0e-3,
            "the walk multiple must read 1.0x at the shipped distance, got {walk}"
        );
        let margin = grass::GRASS_HARVEST_MARGIN_M as f32;
        let w1 = grass_harvest_walk_multiple(60.0 - margin);
        let w2 = grass_harvest_walk_multiple(120.0 - margin);
        assert!(
            (w2 / w1 - 4.0).abs() < 0.01,
            "the harvest walk is a disc and must be exactly quadratic, got {}x",
            w2 / w1
        );
        // The count: clearly superlinear near the default, and closer to
        // quadratic further out.
        let near = grass_drawn_estimate(60.0, 1.0) / grass_drawn_estimate(30.0, 1.0);
        assert!(
            near > 2.2 && near < 3.0,
            "doubling 30 m to 60 m should cost about 2.6x, measured {near}x"
        );
        let far = grass_drawn_estimate(200.0, 1.0) / grass_drawn_estimate(100.0, 1.0);
        assert!(
            far > near && far > 3.4 && far < 4.0,
            "far out the count should approach the disc's 4x, measured {far}x"
        );
    }

    /// The warning tells the operator a distance that WILL fit. That advice has
    /// to be true on both sides: fitting at the number given, and not fitting a
    /// hair past it (otherwise it is needlessly pessimistic advice, which is
    /// how a control quietly loses range again).
    #[test]
    fn the_suggested_distance_is_the_largest_one_that_fits() {
        for cover in [0.5f32, 1.0, 2.0, 3.0] {
            let cap = config::GRASS_HARVEST_CAP_DEFAULT;
            let d = grass_far_within_cap(cap, cover);
            assert!(
                grass_harvest_estimate(d, cover) <= cap * 1.001,
                "suggested {d} m does not actually fit under the cap at cover {cover}"
            );
            if d < config::GRASS_FAR_MAX_M - 0.01 {
                assert!(
                    grass_harvest_estimate(d + 0.5, cover) > cap,
                    "suggested {d} m at cover {cover} is pessimistic: half a \
                     metre further still fits"
                );
            }
        }
    }

    /// The page makes three claims about the ENGINE, and all three are the
    /// kind that rot silently:
    ///
    /// 1. that certain settings are not read by the renderer yet;
    /// 2. that `tree_model_distance` is clamped at 400 m before use;
    /// 3. that `veg_tree_card_m` is clamped at 3000 m before use.
    ///
    /// Each is checked against the actual text of `src/lib.rs`, in BOTH
    /// directions. Wire a setting up and forget to delete its warning, and this
    /// fails because the page is now lying the other way. That two-sided check
    /// is the point: a one-sided one would let the notice outlive its reason,
    /// which is how "known issue" comments become folklore.
    #[test]
    fn the_page_tells_the_truth_about_what_the_engine_reads() {
        let lib = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
        )
        .expect("read src/lib.rs");
        let grass_src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/terrain/grass.rs"),
        )
        .expect("read src/terrain/grass.rs");
        let engine = format!("{lib}\n{grass_src}");

        // 1. Every setting on the "not connected" list must really be absent
        //    from the engine, and every setting NOT on it must really be there.
        for field in ["grass_far_m", "grass_harvest_cap", "near_tree_budget"] {
            let consumed = engine.contains(&format!("settings.{field}"));
            let listed = VEG_AWAITING_WIRING.iter().any(|(f, _)| *f == field);
            assert_ne!(
                consumed, listed,
                "`{field}`: the engine {} it, but the Settings page says it {}. \
                 Either wire it up or fix VEG_AWAITING_WIRING; a control that \
                 silently does nothing is the defect this list exists to prevent.",
                if consumed { "reads" } else { "does not read" },
                if listed { "is not connected" } else { "is connected" },
            );
        }

        // 2 + 3. The engine clamps used to be LOWER than what the page
        //    offered, so the page carried a warning and this test pinned the
        //    two together. v0.1108.2 wired the ceilings to one source
        //    (`config::TREE_MODEL_MAX_M` / `TREE_CARD_MAX_M`, read by both the
        //    control and `src/lib.rs`), which makes the warning unreachable by
        //    construction - so both it and its assertions are gone rather than
        //    left as text that can never fire. What replaces them is the
        //    stronger claim below: the engine must clamp against the SHARED
        //    constant, not a literal that can drift away from it again.
        for (field, konst) in [
            ("tree_model_distance", "TREE_MODEL_MAX_M"),
            ("veg_tree_card_m", "TREE_CARD_MAX_M"),
        ] {
            assert!(
                lib.contains(&format!("crate::config::{konst}")),
                "src/lib.rs no longer clamps {field} against config::{konst}.                  If it went back to a literal, the control's ceiling and the                  renderer's can drift apart again - which is exactly how the                  300-400 m band of this slider was unreachable for releases                  without anyone noticing."
            );
        }
    }

    /// The card row prints an EFFECTIVE distance; the renderer is supposed to
    /// clamp the cutoff to the same number, from the same helper, in
    /// `src/lib.rs`. Those live two files apart, so pin them together.
    ///
    /// WHAT THIS CATCHES: the half-wiring. Publishing the measurement without
    /// applying it leaves the page honest and the picture unchanged; applying
    /// it without publishing leaves the clamp reading a stale sentinel. Either
    /// one fails here.
    ///
    /// WHAT IT DOES NOT: it is both-or-NEITHER, so it cannot prove the wiring
    /// landed in the first place. It passes on a tree where `src/lib.rs` has
    /// not been touched yet, which is exactly the state this test was written
    /// in - the page then reports "not measured yet" rather than lying.
    #[test]
    fn the_measured_card_reach_is_wired_at_both_ends_or_neither() {
        let lib = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
        )
        .expect("read src/lib.rs");
        let publishes = lib.contains("far_trees::publish_card_reach_m");
        let clamps = lib.contains("far_trees::effective_card_far_m");
        assert_eq!(
            publishes, clamps,
            "src/lib.rs {} the measured tree-card reach but {} it. Both halves \
             or neither: the Settings row shows what the clamp is supposed to \
             apply, so a one-sided wiring makes the page describe a cutoff the \
             renderer is not using.",
            if publishes { "publishes" } else { "does not publish" },
            if clamps { "applies" } else { "does not apply" },
        );
    }

    #[test]
    fn thousands_separates_groups_of_three() {
        assert_eq!(thousands(0.0), "0");
        assert_eq!(thousands(999.0), "999");
        assert_eq!(thousands(12_755.0), "12,755");
        assert_eq!(thousands(200_000.0), "200,000");
        assert_eq!(thousands(1_234_567.0), "1,234,567");
    }
}

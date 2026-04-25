//! Settings panel with sidebar navigation and category content panels.
//!
//! Categories: Account, Appearance, Notifications, Wallet, Audio,
//! Graphics, Controls, Privacy, Data, Updates.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiState, SettingsCategory, WalletNetwork, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::updater::{UpdateChannel, UpdateState};

/// Parse the web client's ECDH backup JSON and convert to native format.
/// Accepts either:
/// - The full JSON: {"publicKeyRaw": "...", "privateKeyPkcs8": "..."}
/// - Just the PKCS8 base64 string alone
/// Returns (private_hex_32bytes, public_base64_65bytes).
fn try_import_ecdh(input: &str) -> Result<(String, String), String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Empty input".into());
    }

    // Try parsing as JSON first
    let pkcs8_b64 = if input.starts_with('{') {
        let val: serde_json::Value = serde_json::from_str(input)
            .map_err(|e| format!("JSON parse: {}", e))?;
        val.get("privateKeyPkcs8")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'privateKeyPkcs8' field".to_string())?
            .to_string()
    } else {
        input.to_string()
    };

    let keypair = crate::net::dm_crypto::DmKeypair::from_pkcs8_base64(&pkcs8_b64)?;
    let priv_hex = hex::encode(keypair.secret_bytes());
    let pub_b64 = keypair.public_base64();
    Ok((priv_hex, pub_b64))
}

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
        ui.allocate_ui_with_layout(
            Vec2::new(120.0, ui.spacing().interact_size.y),
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
                ("Widgets", SettingsCategory::Widgets),
                ("Notifications", SettingsCategory::Notifications),
                ("Wallet", SettingsCategory::Wallet),
                ("Audio", SettingsCategory::Audio),
                ("Graphics", SettingsCategory::Graphics),
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
                        SettingsCategory::Widgets,
                        SettingsCategory::Notifications,
                        SettingsCategory::Wallet,
                        SettingsCategory::Audio,
                        SettingsCategory::Graphics,
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
                            SettingsCategory::Widgets => "Widgets",
                            SettingsCategory::Notifications => "Notifications",
                            SettingsCategory::Wallet => "Wallet",
                            SettingsCategory::Audio => "Audio",
                            SettingsCategory::Graphics => "Graphics",
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
                            SettingsCategory::Widgets => draw_widgets_content(ui, theme, state),
                            SettingsCategory::Notifications => draw_notifications_content(ui, theme, state),
                            SettingsCategory::Wallet => draw_wallet_content(ui, theme, state),
                            SettingsCategory::Audio => draw_audio_content(ui, theme, state),
                            SettingsCategory::Graphics => draw_graphics_content(ui, theme, state),
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

fn draw_account_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Display Name:").color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.user_name).desired_width(200.0));
        });

        ui.add_space(theme.spacing_sm);

        // Public key
        ui.horizontal(|ui| {
            ui.label(RichText::new("Public Key:").color(theme.text_secondary()));
            let key_display = if state.profile_public_key.is_empty() {
                "No key generated".to_string()
            } else if state.profile_public_key.len() > 16 {
                format!("{}...{}", &state.profile_public_key[..8], &state.profile_public_key[state.profile_public_key.len()-8..])
            } else {
                state.profile_public_key.clone()
            };
            ui.label(RichText::new(&key_display).color(theme.text_muted()).size(theme.font_size_small));
            if widgets::secondary_button(ui, theme, "Copy") {
                ui.ctx().copy_text(state.profile_public_key.clone());
            }
        });

        ui.add_space(theme.spacing_md);

        // ECDH key (for E2E encrypted DMs)
        ui.label(RichText::new("DM Encryption Key (ECDH P-256)").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.label(RichText::new("ECDH Public:").color(theme.text_secondary()));
            let display = if state.ecdh_public_b64.is_empty() {
                "(not set)".to_string()
            } else if state.ecdh_public_b64.len() > 20 {
                format!("{}...", &state.ecdh_public_b64[..20])
            } else {
                state.ecdh_public_b64.clone()
            };
            ui.label(RichText::new(&display).color(theme.text_muted()).size(theme.font_size_small));
            if !state.ecdh_public_b64.is_empty() && widgets::secondary_button(ui, theme, "Copy Public") {
                ui.ctx().copy_text(state.ecdh_public_b64.clone());
            }
        });
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new("To read DMs sent by the web client, import the web ECDH key. In your browser console on united-humanity.us, run: localStorage.getItem('humanity_ecdh_backup')")
                .color(theme.text_muted())
                .size(theme.font_size_small)
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Import (JSON from browser):").color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.ecdh_import_input)
                .desired_width(260.0)
                .password(true)
                .hint_text("{\"publicKeyRaw\":...}"));
            if widgets::primary_button(ui, theme, "Import") {
                match try_import_ecdh(&state.ecdh_import_input) {
                    Ok((priv_hex, pub_b64)) => {
                        state.ecdh_private_hex = priv_hex;
                        state.ecdh_public_b64 = pub_b64;
                        state.ecdh_import_input.clear();
                        state.ecdh_import_status = "Imported successfully. Reconnect to use.".to_string();
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                    Err(e) => {
                        state.ecdh_import_status = format!("Import failed: {}", e);
                    }
                }
            }
        });
        if !state.ecdh_import_status.is_empty() {
            ui.label(RichText::new(&state.ecdh_import_status).color(theme.text_muted()).size(theme.font_size_small));
        }

        ui.add_space(theme.spacing_md);

        // Seed phrase backup
        ui.label(RichText::new("Seed Phrase Backup").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Your 24-word seed phrase backs up your identity and wallet.").color(theme.text_muted()).size(theme.font_size_small));

        ui.add_space(theme.spacing_xs);
        if widgets::secondary_button(ui, theme, if state.settings.seed_phrase_visible { "Hide Seed Phrase" } else { "Show Seed Phrase" }) {
            state.settings.seed_phrase_visible = !state.settings.seed_phrase_visible;
        }

        if state.settings.seed_phrase_visible {
            ui.add_space(theme.spacing_xs);
            egui::Frame::none()
                .fill(Color32::from_rgb(40, 30, 20))
                .rounding(Rounding::same(4))
                .inner_margin(8.0)
                .stroke(Stroke::new(1.0, theme.warning()))
                .show(ui, |ui| {
                    ui.label(RichText::new("No seed phrase generated yet.").color(theme.warning()).size(theme.font_size_small));
                });
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
                    Ok((pubkey_hex, privkey_bytes)) => {
                        state.settings.seed_phrase_recovery_status = format!("Identity recovered! Public key: {}...{}", &pubkey_hex[..8], &pubkey_hex[pubkey_hex.len()-8..]);
                        state.profile_public_key = pubkey_hex;
                        state.private_key_bytes = Some(privkey_bytes);
                        // Disconnect existing connection so auto-connect uses new identity
                        if let Some(ref mut ws) = state.ws_client {
                            ws.disconnect();
                        }
                        state.ws_client = None;
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

    ui.add_space(theme.spacing_md);

    // Donation Addresses section (admin/owner)
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new("Donation Addresses").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Configure donation addresses shown on the Donate page. Supports any cryptocurrency or URL.")
            .color(theme.text_muted()).size(theme.font_size_small));
        ui.add_space(theme.spacing_sm);

        // Legacy fields (kept for backward compatibility)
        ui.horizontal(|ui| {
            ui.label(RichText::new("Solana (SOL):").color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.donate_solana_address)
                .desired_width(300.0)
                .hint_text("Base58 Solana address"));
        });

        ui.add_space(theme.spacing_xs);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Bitcoin (BTC):").color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.donate_btc_address)
                .desired_width(300.0)
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
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Network:").color(theme.text_muted()).size(theme.font_size_small));
                            ui.add(egui::TextEdit::singleline(&mut addr.network)
                                .desired_width(150.0)
                                .hint_text("e.g. Ethereum (ETH)"));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Value:").color(theme.text_muted()).size(theme.font_size_small));
                            ui.add(egui::TextEdit::singleline(&mut addr.value)
                                .desired_width(250.0)
                                .hint_text("Address or URL"));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Label:").color(theme.text_muted()).size(theme.font_size_small));
                            ui.add(egui::TextEdit::singleline(&mut addr.label)
                                .desired_width(200.0)
                                .hint_text("Short description"));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Type:").color(theme.text_muted()).size(theme.font_size_small));
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
        ui.horizontal(|ui| {
            ui.label(RichText::new("Network:").color(theme.text_muted()).size(theme.font_size_small));
            ui.add(egui::TextEdit::singleline(&mut state.donate_new_network)
                .desired_width(150.0)
                .hint_text("e.g. Monero (XMR)"));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Value:").color(theme.text_muted()).size(theme.font_size_small));
            ui.add(egui::TextEdit::singleline(&mut state.donate_new_value)
                .desired_width(250.0)
                .hint_text("Address or URL"));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Label:").color(theme.text_muted()).size(theme.font_size_small));
            ui.add(egui::TextEdit::singleline(&mut state.donate_new_label)
                .desired_width(200.0)
                .hint_text("Short description"));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Type:").color(theme.text_muted()).size(theme.font_size_small));
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

fn draw_appearance_content(ui: &mut egui::Ui, theme: &mut Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        if widgets::toggle(ui, theme, "Dark Mode", &mut state.settings.dark_mode) {
            state.settings_dirty = true;
        }

        ui.add_space(theme.spacing_sm);

        if widgets::labeled_slider(ui, theme, "Font Size", &mut state.settings.font_size, 10.0..=24.0) {
            state.settings_dirty = true;
        }
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

/// One row of the color-picker grid: a label and a swatch button that opens
/// egui's color picker. Returns true if the color changed.
fn color_row(
    ui: &mut egui::Ui,
    label: &str,
    color_tuple: &mut (f32, f32, f32, f32),
    label_color: Color32,
) -> bool {
    let mut rgba = [color_tuple.0, color_tuple.1, color_tuple.2, color_tuple.3];
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(label_color).size(13.0));
        ui.add_space(4.0);
        if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
            color_tuple.0 = rgba[0];
            color_tuple.1 = rgba[1];
            color_tuple.2 = rgba[2];
            color_tuple.3 = rgba[3];
            changed = true;
        }
    });
    changed
}

fn draw_widgets_content(ui: &mut egui::Ui, theme: &mut Theme, state: &mut GuiState) {
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
            any_changed |= styled_slider(ui, &ss, "Row Height", &mut theme.row_height, 12.0..=48.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Header Height", &mut theme.header_height, 16.0..=64.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Button Height", &mut theme.button_height, 16.0..=48.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Input Height", &mut theme.input_height, 16.0..=48.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Status Dot", &mut theme.status_dot_size, 2.0..=16.0, label_color);
            any_changed |= styled_slider(ui, &ss, "Checkbox Size", &mut theme.checkbox_size, 10.0..=28.0, label_color);
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

fn draw_notifications_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        widgets::toggle(ui, theme, "Direct Messages", &mut state.settings.notify_dm);
        widgets::toggle(ui, theme, "Mentions", &mut state.settings.notify_mentions);
        widgets::toggle(ui, theme, "Task Updates", &mut state.settings.notify_tasks);

        ui.add_space(theme.spacing_md);
        ui.label(RichText::new("Do Not Disturb").color(theme.text_secondary()).strong());
        ui.add_space(theme.spacing_xs);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Start:").color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.settings.dnd_start)
                .desired_width(80.0)
                .hint_text("22:00"));
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new("End:").color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.settings.dnd_end)
                .desired_width(80.0)
                .hint_text("08:00"));
        });
    });
}

fn draw_wallet_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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

        // Custom RPC
        ui.horizontal(|ui| {
            ui.label(RichText::new("Custom RPC URL:").color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.settings.custom_rpc_url)
                .desired_width(300.0)
                .hint_text("https://..."));
        });
    });
}

fn draw_audio_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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
}

fn draw_graphics_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        if widgets::toggle(ui, theme, "Fullscreen", &mut state.settings.fullscreen) {
            state.settings_dirty = true;
        }
        if widgets::toggle(ui, theme, "VSync", &mut state.settings.vsync) {
            state.settings_dirty = true;
        }
        if widgets::labeled_slider(ui, theme, "FOV", &mut state.settings.fov, 60.0..=120.0) {
            state.settings_dirty = true;
        }
        if widgets::labeled_slider(ui, theme, "Render Distance", &mut state.settings.render_distance, 50.0..=2000.0) {
            state.settings_dirty = true;
        }
    });
}

fn draw_controls_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        if widgets::labeled_slider(ui, theme, "Mouse Sensitivity", &mut state.settings.mouse_sensitivity, 0.01..=10.0) {
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
            ("Map", "M"),
            ("Escape Menu", "Esc"),
        ];
        for (action, key) in &keybinds {
            ui.horizontal(|ui| {
                ui.label(RichText::new(*action).color(theme.text_secondary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(40, 40, 50))
                        .rounding(Rounding::same(3))
                        .inner_margin(Vec2::new(8.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(*key).color(theme.text_primary()).size(theme.font_size_small).strong());
                        });
                });
            });
        }
    });
}

fn draw_privacy_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
        widgets::toggle(ui, theme, "Profile Visible to Others", &mut state.settings.profile_visible);
        widgets::toggle(ui, theme, "Show Online Status", &mut state.settings.online_status_visible);
    });
}

fn draw_data_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::card(ui, theme, |ui| {
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

fn draw_updates_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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

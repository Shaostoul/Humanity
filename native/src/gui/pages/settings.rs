//! Settings panel with sidebar navigation and category content panels.
//!
//! Categories: Account, Appearance, Notifications, Wallet, Audio,
//! Graphics, Controls, Privacy, Data, Updates.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiState, SettingsCategory, WalletNetwork, VERSION};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::updater::{UpdateChannel, UpdateState};

pub fn draw(ctx: &egui::Context, theme: &mut Theme, state: &mut GuiState) {
    // Left sidebar with category list
    egui::SidePanel::left("settings_sidebar")
        .default_width(180.0)
        .min_width(140.0)
        .max_width(240.0)
        .frame(Frame::none()
            .fill(Color32::from_rgb(22, 22, 28))
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
                    state.settings.category = *cat;
                }
            }
        });

    // Right content area
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                match state.settings.category {
                    SettingsCategory::Account => draw_account(ui, theme, state),
                    SettingsCategory::Appearance => draw_appearance(ui, theme, state),
                    SettingsCategory::Widgets => draw_widgets(ui, theme, state),
                    SettingsCategory::Notifications => draw_notifications(ui, theme, state),
                    SettingsCategory::Wallet => draw_wallet(ui, theme, state),
                    SettingsCategory::Audio => draw_audio(ui, theme, state),
                    SettingsCategory::Graphics => draw_graphics(ui, theme, state),
                    SettingsCategory::Controls => draw_controls(ui, theme, state),
                    SettingsCategory::Privacy => draw_privacy(ui, theme, state),
                    SettingsCategory::Data => draw_data(ui, theme, state),
                    SettingsCategory::Updates => draw_updates(ui, theme, state),
                }
            });
        });
}

fn draw_account(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Account").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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
                        // Save config with new identity
                        crate::config::AppConfig::from_gui_state(state).save();
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
}

fn draw_appearance(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Appearance").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        if widgets::toggle(ui, theme, "Dark Mode", &mut state.settings.dark_mode) {
            state.settings_dirty = true;
        }

        ui.add_space(theme.spacing_sm);

        if widgets::labeled_slider(ui, theme, "Font Size", &mut state.settings.font_size, 10.0..=24.0) {
            state.settings_dirty = true;
        }
    });
}

fn draw_widgets(ui: &mut egui::Ui, theme: &mut Theme, state: &mut GuiState) {
    ui.label(RichText::new("Widgets").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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

    // Two-column layout: sliders on left, live preview on right
    ui.columns(2, |cols| {
        // ── LEFT COLUMN: sliders ──
        let ui = &mut cols[0];
        let mut any_changed = false;

        // Sizing card (inline Frame to avoid borrow conflict with widgets::card)
        egui::Frame::none()
            .fill(card_bg)
            .rounding(Rounding::same(card_radius as u8))
            .inner_margin(card_padding)
            .stroke(Stroke::new(1.0, card_border))
            .show(ui, |ui| {
                ui.label(RichText::new("Sizing").strong().color(text_color));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Icon Size").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.icon_size, 16.0..=64.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Row Height").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.row_height, 14.0..=32.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Row Gap").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.row_gap, 0.0..=8.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Header Height").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.header_height, 24.0..=64.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Border Width").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.border_width, 0.0..=4.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Status Dot").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.status_dot_size, 4.0..=16.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Panel Margin").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.panel_margin, 0.0..=16.0).show_value(true)).changed();
                });
            });

        ui.add_space(spacing_sm);

        // Fonts card
        egui::Frame::none()
            .fill(card_bg)
            .rounding(Rounding::same(card_radius as u8))
            .inner_margin(card_padding)
            .stroke(Stroke::new(1.0, card_border))
            .show(ui, |ui| {
                ui.label(RichText::new("Fonts").strong().color(text_color));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Name Font").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.name_size, 10.0..=24.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Body Font").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.body_size, 10.0..=24.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Small Font").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.small_size, 8.0..=16.0).show_value(true)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Border Radius").color(label_color));
                    any_changed |= ui.add(egui::Slider::new(&mut theme.border_radius_widget, 0.0..=12.0).show_value(true)).changed();
                });
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
            .fill(Color32::from_rgb(20, 20, 25))
            .rounding(Rounding::same(4))
            .inner_margin(8.0)
            .stroke(Stroke::new(1.0, Color32::from_rgb(42, 42, 53)))
            .show(ui, |ui| {
                ui.label(RichText::new("Live Preview").size(heading_sz).color(text_color));
                ui.add_space(spacing_sm);

                // Sample message row (uses actual widget)
                ui.label(RichText::new("Message Row").size(theme.small_size).color(label_color).strong());
                ui.add_space(2.0);
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
                ui.add_space(4.0);
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
                ui.add_space(2.0);
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
                ui.add_space(2.0);
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

fn draw_notifications(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Notifications").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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

fn draw_wallet(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Wallet").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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

fn draw_audio(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Audio").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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

fn draw_graphics(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Graphics").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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

fn draw_controls(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Controls").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        if widgets::labeled_slider(ui, theme, "Mouse Sensitivity", &mut state.settings.mouse_sensitivity, 0.5..=10.0) {
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

fn draw_privacy(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Privacy").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        widgets::toggle(ui, theme, "Profile Visible to Others", &mut state.settings.profile_visible);
        widgets::toggle(ui, theme, "Show Online Status", &mut state.settings.online_status_visible);
    });
}

fn draw_data(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Data").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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

fn draw_updates(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Updates").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

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

//! 3-panel chat page matching the website layout.
//!
//! LEFT:   Server info, channel list, voice channels
//! MIDDLE: Message feed with input bar
//! RIGHT:  Online users list

use egui::{Color32, Frame, RichText, ScrollArea, Stroke, Vec2};
use crate::gui::{ChatMessage, GuiState};
use crate::gui::theme::Theme;

/// Maximum messages kept in the local chat buffer.
const MAX_MESSAGES: usize = 200;

/// Left panel width in points.
const LEFT_PANEL_WIDTH: f32 = 220.0;
/// Right panel width in points.
const RIGHT_PANEL_WIDTH: f32 = 220.0;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // ── LEFT PANEL: Channels ──
    egui::SidePanel::left("chat_left_panel")
        .exact_width(LEFT_PANEL_WIDTH)
        .resizable(false)
        .frame(Frame::none().fill(Color32::from_rgb(30, 30, 36)).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_left_panel(ui, theme, state);
        });

    // ── RIGHT PANEL: Users ──
    egui::SidePanel::right("chat_right_panel")
        .exact_width(RIGHT_PANEL_WIDTH)
        .resizable(false)
        .frame(Frame::none().fill(Color32::from_rgb(30, 30, 36)).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_right_panel(ui, theme, state);
        });

    // ── MIDDLE PANEL: Messages ──
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_center_panel(ui, theme, state);
        });
}

// ─────────────────────────────── LEFT PANEL ───────────────────────────────

fn draw_left_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Server header
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), 48.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new("HumanityOS")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
        },
    );

    // Connection status
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let (dot_color, status_text) = if state.ws_client.as_ref().map_or(false, |c| c.is_connected()) {
            let online_count = state.chat_users.len();
            (theme.success(), format!("{} online", online_count))
        } else {
            (theme.danger(), state.ws_status.clone())
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, dot_color);
        ui.label(RichText::new(status_text).size(theme.font_size_small).color(theme.text_muted()));
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Text Channels ──
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new("TEXT CHANNELS")
                .size(theme.font_size_small - 1.0)
                .color(theme.text_muted())
                .strong(),
        );
    });
    ui.add_space(4.0);

    let active = state.chat_active_channel.clone();
    for ch in state.chat_channels.clone() {
        if ch.category != "Voice" {
            let is_active = ch.id == active;
            let bg = if is_active {
                Color32::from_rgb(50, 50, 58)
            } else {
                Color32::TRANSPARENT
            };
            let text_color = if is_active {
                theme.text_primary()
            } else {
                theme.text_secondary()
            };

            let response = ui
                .allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 30.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let full_rect = ui.max_rect();
                        ui.painter().rect_filled(full_rect, 4.0, bg);
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(format!("# {}", ch.name))
                                .size(theme.font_size_body)
                                .color(text_color),
                        );
                    },
                )
                .response;

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                state.chat_active_channel = ch.id.clone();
            }
        }
    }

    ui.add_space(12.0);

    // ── Voice Channels ──
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new("VOICE CHANNELS")
                .size(theme.font_size_small - 1.0)
                .color(theme.text_muted())
                .strong(),
        );
    });
    ui.add_space(4.0);

    for label in &["Lounge", "Dev Talk"] {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("  {}", label))
                    .size(theme.font_size_body)
                    .color(theme.text_muted()),
            );
        });
    }

    // ── Bottom: Connect / Add Server ──
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        ui.add_space(8.0);

        if state.ws_client.is_none() {
            // Show server URL input if empty
            if state.server_url.is_empty() {
                state.server_url = "https://united-humanity.us".to_string();
            }

            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Server:").size(theme.font_size_small).color(theme.text_muted()));
            });
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let input = egui::TextEdit::singleline(&mut state.server_url)
                    .desired_width(LEFT_PANEL_WIDTH - 16.0)
                    .font(egui::TextStyle::Small);
                ui.add(input);
            });
            ui.add_space(4.0);

            // Name input if empty
            if state.user_name.is_empty() {
                state.user_name = "Desktop User".to_string();
            }
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Name:").size(theme.font_size_small).color(theme.text_muted()));
            });
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let input = egui::TextEdit::singleline(&mut state.user_name)
                    .desired_width(LEFT_PANEL_WIDTH - 16.0)
                    .font(egui::TextStyle::Small);
                ui.add(input);
            });
            ui.add_space(4.0);

            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Connect")
                            .size(theme.font_size_body)
                            .color(theme.text_on_accent()),
                    )
                    .fill(theme.accent())
                    .min_size(Vec2::new(LEFT_PANEL_WIDTH - 24.0, 32.0)),
                )
                .clicked()
            {
                // Convert https:// URL to wss:// WebSocket URL
                let ws_url = derive_ws_url(&state.server_url);
                let name = state.user_name.clone();
                let pubkey = if state.profile_public_key.is_empty() {
                    generate_random_hex_key()
                } else {
                    state.profile_public_key.clone()
                };
                log::info!("Connecting to {} as {} (key: {})", ws_url, name, &pubkey[..8]);
                state.ws_client = Some(crate::net::ws_client::WsClient::connect(
                    &ws_url, &name, &pubkey,
                ));
                state.ws_status = "Connecting...".to_string();

                // Save config so it persists
                crate::config::AppConfig::from_gui_state(state).save();
            }
        } else if state.ws_client.as_ref().map_or(false, |c| c.is_connected()) {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Disconnect")
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    )
                    .fill(Color32::from_rgb(60, 30, 30))
                    .min_size(Vec2::new(LEFT_PANEL_WIDTH - 24.0, 32.0)),
                )
                .clicked()
            {
                if let Some(ref mut client) = state.ws_client {
                    client.disconnect();
                }
                state.ws_client = None;
                state.ws_status = "Disconnected".to_string();
                state.chat_users.clear();
            }
        }

        ui.add_space(4.0);
    });
}

// ─────────────────────────────── RIGHT PANEL ──────────────────────────────

fn draw_right_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Header
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), 48.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new("People & Streams")
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
        },
    );

    ui.separator();
    ui.add_space(4.0);

    // Online count
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new(format!("ONLINE  {}", state.chat_users.len()))
                .size(theme.font_size_small - 1.0)
                .color(theme.text_muted())
                .strong(),
        );
    });
    ui.add_space(4.0);

    // User list
    ScrollArea::vertical()
        .id_salt("chat_users_scroll")
        .show(ui, |ui| {
            for user in &state.chat_users {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);

                    // Online dot
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                    let dot_color = match user.status.as_str() {
                        "away" => theme.warning(),
                        "busy" | "dnd" => theme.danger(),
                        _ => theme.success(),
                    };
                    ui.painter().circle_filled(rect.center(), 4.0, dot_color);

                    // Name
                    ui.label(
                        RichText::new(&user.name)
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    );

                    // Role badge
                    if !user.role.is_empty() && user.role != "member" {
                        let badge_color = match user.role.as_str() {
                            "admin" => Theme::c32(&theme.badge_admin),
                            "moderator" | "mod" => Theme::c32(&theme.badge_mod),
                            _ => theme.text_muted(),
                        };
                        ui.label(
                            RichText::new(&user.role)
                                .size(theme.font_size_small - 1.0)
                                .color(badge_color)
                                .strong(),
                        );
                    }
                });
            }

            if state.chat_users.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("No users online")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });
            }
        });
}

// ─────────────────────────────── CENTER PANEL ─────────────────────────────

fn draw_center_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Channel header ──
    Frame::none()
        .fill(Color32::from_rgb(25, 25, 30))
        .inner_margin(egui::Margin::symmetric(16, 10))
        .stroke(Stroke::new(1.0, Color32::from_rgb(40, 40, 48)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("# {}", state.chat_active_channel))
                        .size(theme.font_size_heading)
                        .color(theme.text_primary())
                        .strong(),
                );

                // Find description for current channel
                let desc = state
                    .chat_channels
                    .iter()
                    .find(|c| c.id == state.chat_active_channel)
                    .map(|c| c.description.as_str())
                    .unwrap_or("");
                if !desc.is_empty() {
                    ui.label(
                        RichText::new(format!("  |  {}", desc))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                }
            });
        });

    // ── Message area ──
    let active_channel = state.chat_active_channel.clone();
    let input_height = 52.0;

    let available = ui.available_rect_before_wrap();
    let messages_rect = egui::Rect::from_min_size(
        available.min,
        Vec2::new(available.width(), available.height() - input_height),
    );

    ui.allocate_ui_at_rect(messages_rect, |ui| {
        ScrollArea::vertical()
            .id_salt("chat_messages_scroll")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);

                let filtered: Vec<&ChatMessage> = state
                    .chat_messages
                    .iter()
                    .filter(|m| m.channel == active_channel)
                    .collect();

                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(format!("Welcome to #{}", active_channel))
                                .size(theme.font_size_title)
                                .color(theme.text_primary()),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("No messages yet. Say something!")
                                .size(theme.font_size_body)
                                .color(theme.text_muted()),
                        );
                    });
                }

                let mut last_sender = String::new();
                for msg in &filtered {
                    let new_group = msg.sender_name != last_sender;
                    last_sender = msg.sender_name.clone();

                    if new_group {
                        ui.add_space(8.0);
                    }

                    ui.horizontal(|ui| {
                        ui.add_space(16.0);

                        if new_group {
                            // Avatar placeholder: colored circle from name hash
                            let color = name_color(&msg.sender_name);
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 14.0, color);
                            let initial = msg
                                .sender_name
                                .chars()
                                .next()
                                .unwrap_or('?')
                                .to_uppercase()
                                .to_string();
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                initial,
                                egui::FontId::proportional(14.0),
                                Color32::WHITE,
                            );
                        } else {
                            // Indent to align with messages in the same group
                            ui.add_space(36.0);
                        }

                        ui.vertical(|ui| {
                            if new_group {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&msg.sender_name)
                                            .size(theme.font_size_body)
                                            .color(theme.text_primary())
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(&msg.timestamp)
                                            .size(theme.font_size_small - 1.0)
                                            .color(theme.text_muted()),
                                    );
                                });
                            }
                            ui.label(
                                RichText::new(&msg.content)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            );
                        });
                    });
                }

                ui.add_space(8.0);
            });
    });

    // ── Input bar ──
    let input_rect = egui::Rect::from_min_size(
        egui::pos2(available.min.x, available.max.y - input_height),
        Vec2::new(available.width(), input_height),
    );

    ui.allocate_ui_at_rect(input_rect, |ui| {
        Frame::none()
            .fill(Color32::from_rgb(25, 25, 30))
            .inner_margin(egui::Margin::symmetric(16, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut state.chat_input)
                            .desired_width(ui.available_width() - 70.0)
                            .hint_text(format!("Message #{}", state.chat_active_channel)),
                    );

                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let send_clicked = ui
                        .add(
                            egui::Button::new(
                                RichText::new("Send")
                                    .size(theme.font_size_body)
                                    .color(theme.text_on_accent()),
                            )
                            .fill(theme.accent())
                            .min_size(Vec2::new(56.0, 28.0)),
                        )
                        .clicked();

                    if (enter_pressed || send_clicked) && !state.chat_input.trim().is_empty() {
                        let content = state.chat_input.trim().to_string();
                        let channel = state.chat_active_channel.clone();

                        // Send via WebSocket if connected
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;

                                let chat_msg = serde_json::json!({
                                    "type": "chat",
                                    "from": state.profile_public_key,
                                    "content": content,
                                    "timestamp": ts,
                                    "channel": channel,
                                });
                                client.send(&chat_msg.to_string());
                            }
                        }

                        // Also store locally (so the user sees their own message immediately)
                        let now = chrono_now_str();
                        state.chat_messages.push(ChatMessage {
                            sender_name: if state.user_name.is_empty() {
                                "You".to_string()
                            } else {
                                state.user_name.clone()
                            },
                            sender_key: state.profile_public_key.clone(),
                            content,
                            timestamp: now,
                            channel,
                        });

                        // Bound the buffer
                        while state.chat_messages.len() > MAX_MESSAGES {
                            state.chat_messages.remove(0);
                        }

                        state.chat_input.clear();
                        response.request_focus();
                    }

                    // Re-focus on enter
                    if enter_pressed {
                        response.request_focus();
                    }
                });
            });
    });
}

// ─────────────────────────────── Helpers ───────────────────────────────────

/// Convert an HTTPS URL to a WSS URL for the relay.
pub fn derive_ws_url(url: &str) -> String {
    let base = url
        .trim_end_matches('/')
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    if base.ends_with("/ws") {
        base
    } else {
        format!("{}/ws", base)
    }
}

/// Generate a random 64-char hex string to use as a placeholder public key.
pub fn generate_random_hex_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Derive a consistent color from a username (for avatar circles).
fn name_color(name: &str) -> Color32 {
    let hash: u32 = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (hash % 360) as f32;
    // HSL to RGB (simplified, saturation=0.5, lightness=0.45)
    let s = 0.5_f32;
    let l = 0.45_f32;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Return a human-readable timestamp string for "now".
fn chrono_now_str() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    format!("{:02}:{:02} UTC", hours, minutes)
}

/// Format a Unix-millis timestamp to HH:MM UTC.
pub fn format_timestamp(ts: u64) -> String {
    let total_secs = ts / 1000;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    format!("{:02}:{:02} UTC", hours, minutes)
}

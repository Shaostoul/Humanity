//! 3-panel chat page matching the website layout.
//!
//! LEFT:   Collapsible DMs (red), Groups (green), Servers (blue), Connection settings
//! MIDDLE: Channel header, message feed, input bar
//! RIGHT:  Friends list, Server members list

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{ChatMessage, ChatUser, GuiState};
use crate::gui::theme::Theme;

/// Maximum messages kept in the local chat buffer.
const MAX_MESSAGES: usize = 200;

/// Minimum panel width in points.
const MIN_PANEL_WIDTH: f32 = 150.0;
/// Maximum panel width in points.
const MAX_PANEL_WIDTH: f32 = 400.0;

// ── Section tint colors (matching website rgba values) ──

const DM_BG: Color32 = Color32::from_rgba_premultiplied(45, 15, 15, 255);
const DM_ROW_BG: Color32 = Color32::from_rgba_premultiplied(55, 20, 20, 255);
const DM_ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(70, 25, 25, 255);

const GROUP_BG: Color32 = Color32::from_rgba_premultiplied(15, 45, 15, 255);
const GROUP_ROW_BG: Color32 = Color32::from_rgba_premultiplied(20, 55, 20, 255);
const GROUP_ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(25, 70, 25, 255);

const SERVER_BG: Color32 = Color32::from_rgba_premultiplied(15, 15, 45, 255);
const SERVER_ROW_BG: Color32 = Color32::from_rgba_premultiplied(20, 20, 55, 255);
const SERVER_ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(25, 25, 70, 255);

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // ── LEFT PANEL ──
    let left_panel = egui::SidePanel::left("chat_left_panel")
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(0.0))
        .width_range(MIN_PANEL_WIDTH..=MAX_PANEL_WIDTH);
    let left_panel = if state.chat_left_panel_locked {
        left_panel.exact_width(state.chat_left_panel_width).resizable(false)
    } else {
        left_panel.default_width(state.chat_left_panel_width).resizable(true)
    };
    let mut left_lock_toggled = false;
    let left_response = left_panel.show(ctx, |ui| {
        if draw_panel_lock_button(ui, theme, state.chat_left_panel_locked) {
            left_lock_toggled = true;
        }
        draw_left_panel(ui, theme, state);
    });
    if left_lock_toggled {
        state.chat_left_panel_locked = !state.chat_left_panel_locked;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // Track the actual rendered width so it persists
    if !state.chat_left_panel_locked {
        state.chat_left_panel_width = left_response.response.rect.width();
    }

    // ── RIGHT PANEL ──
    let right_panel = egui::SidePanel::right("chat_right_panel")
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(0.0))
        .width_range(MIN_PANEL_WIDTH..=MAX_PANEL_WIDTH);
    let right_panel = if state.chat_right_panel_locked {
        right_panel.exact_width(state.chat_right_panel_width).resizable(false)
    } else {
        right_panel.default_width(state.chat_right_panel_width).resizable(true)
    };
    let mut right_lock_toggled = false;
    let right_response = right_panel.show(ctx, |ui| {
        if draw_panel_lock_button(ui, theme, state.chat_right_panel_locked) {
            right_lock_toggled = true;
        }
        draw_right_panel(ui, theme, state);
    });
    if right_lock_toggled {
        state.chat_right_panel_locked = !state.chat_right_panel_locked;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // Track the actual rendered width so it persists
    if !state.chat_right_panel_locked {
        state.chat_right_panel_width = right_response.response.rect.width();
    }

    // ── CENTER PANEL ──
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(Color32::from_rgb(20, 20, 25)).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_center_panel(ui, theme, state);
        });

    // ── USER PROFILE MODAL ──
    if state.chat_user_modal_open {
        draw_user_modal(ctx, theme, state);
    }

    // ── CREATE CHANNEL MODAL ──
    if state.show_create_channel_modal {
        draw_create_channel_modal(ctx, theme, state);
    }

    // ── EDIT CHANNEL MODAL ──
    if state.show_channel_edit_modal {
        draw_edit_channel_modal(ctx, theme, state);
    }

    // ── CREATE GROUP MODAL ──
    if state.show_create_group_modal {
        draw_create_group_modal(ctx, theme, state);
    }

    // ── JOIN GROUP MODAL ──
    if state.show_join_group_modal {
        draw_join_group_modal(ctx, theme, state);
    }
}

// ─────────────────────────────── LEFT PANEL ───────────────────────────────

fn draw_left_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Connection status bar ──
    let is_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    if is_connected {
        // Connected: green dot + server name + online count + disconnect button
        Frame::NONE
            .fill(Color32::from_rgb(25, 30, 25))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let dot_sz = theme.status_dot_size;
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, theme.success());
                    let online_count = state.chat_users.iter().filter(|u| u.status != "offline").count();
                    ui.label(
                        RichText::new(format!("{} ({} online)", server_display_name(&state.server_url), online_count))
                            .size(theme.small_size)
                            .color(theme.text_primary()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(
                            RichText::new("X")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        ).fill(Color32::TRANSPARENT).frame(false)).on_hover_text("Disconnect").clicked() {
                            if let Some(ref mut client) = state.ws_client {
                                client.disconnect();
                            }
                            state.ws_client = None;
                            state.ws_status = "Disconnected".to_string();
                            state.ws_manually_disconnected = true;
                            state.ws_reconnect_timer = 0.0;
                            state.ws_reconnect_delay = 5.0;
                            state.ws_reconnect_attempts = 0;
                            state.chat_users.clear();
                        }
                    });
                });
            });
    } else {
        // Disconnected: red dot + "Not connected" + server/name inputs + Connect button
        Frame::NONE
            .fill(Color32::from_rgb(40, 30, 30))
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let dot_sz = theme.status_dot_size;
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, theme.danger());
                    ui.label(
                        RichText::new("Not connected")
                            .size(theme.font_size_body)
                            .color(theme.danger())
                            .strong(),
                    );
                });
                ui.add_space(4.0);

                if state.server_url.is_empty() {
                    state.server_url = "https://united-humanity.us".to_string();
                }
                if state.user_name.is_empty() {
                    state.user_name = "Desktop User".to_string();
                }

                ui.label(RichText::new("Server:").size(theme.font_size_small).color(theme.text_muted()));
                ui.add(
                    egui::TextEdit::singleline(&mut state.server_url)
                        .desired_width(ui.available_width() - 24.0)
                        .font(egui::TextStyle::Small),
                );
                ui.add_space(2.0);
                ui.label(RichText::new("Name:").size(theme.font_size_small).color(theme.text_muted()));
                ui.add(
                    egui::TextEdit::singleline(&mut state.user_name)
                        .desired_width(ui.available_width() - 24.0)
                        .font(egui::TextStyle::Small),
                );
                ui.add_space(4.0);

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Connect")
                                .size(theme.font_size_body)
                                .color(theme.text_on_accent()),
                        )
                        .fill(theme.accent())
                        .min_size(Vec2::new(ui.available_width() - 32.0, 32.0)),
                    )
                    .clicked()
                {
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
                    state.ws_manually_disconnected = false;
                    state.ws_reconnect_timer = 0.0;
                    state.ws_reconnect_delay = 5.0;
                    state.ws_reconnect_attempts = 0;
                    crate::config::AppConfig::from_gui_state(state).save();
                }
            });
    }

    ui.add_space(2.0);

    // ── Scrollable section area ──
    ScrollArea::vertical()
        .id_salt("chat_left_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── DMs Section (red tint) ──
            draw_dm_section(ui, theme, state);

            // ── Groups Section (green tint) ──
            draw_groups_section(ui, theme, state);

            // ── Servers Section (blue tint) ──
            draw_servers_section(ui, theme, state);
        });
}

// ── DMs Section ──

fn draw_dm_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_dm_collapsed;
    let dm_count = state.chat_dms.len();

    if tinted_section_header(ui, &format!("DMs ({})", dm_count), collapsed, DM_BG) {
        state.chat_dm_collapsed = !state.chat_dm_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        Frame::NONE
            .fill(DM_BG)
            .inner_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if state.chat_dms.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("No conversations yet")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                    ui.add_space(4.0);
                }

                let dms = state.chat_dms.clone();
                for dm in &dms {
                    let response = ui
                        .allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), theme.row_height),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let full_rect = ui.max_rect();
                                let bg = if ui.rect_contains_pointer(full_rect) {
                                    DM_ROW_HOVER
                                } else {
                                    DM_ROW_BG
                                };
                                ui.painter().rect_filled(full_rect, 0.0, bg);

                                ui.add_space(theme.item_padding);

                                // Unread dot
                                if dm.unread {
                                    let dot_sz = theme.status_dot_size * 0.75;
                                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, theme.accent());
                                }

                                ui.label(
                                    RichText::new(&dm.user_name)
                                        .size(theme.body_size)
                                        .color(if dm.unread { theme.text_primary() } else { theme.text_secondary() }),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(theme.item_padding);
                                    ui.label(
                                        RichText::new(&dm.timestamp)
                                            .size(theme.small_size)
                                            .color(theme.text_muted()),
                                    );
                                });
                            },
                        )
                        .response;

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
    }
}

// ── Groups Section ──

fn draw_groups_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_groups_collapsed;
    let group_count = state.chat_groups.len();

    if tinted_section_header(ui, &format!("Groups ({})", group_count), collapsed, GROUP_BG) {
        state.chat_groups_collapsed = !state.chat_groups_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        Frame::NONE
            .fill(GROUP_BG)
            .inner_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if state.chat_groups.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("No groups yet")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                    ui.add_space(4.0);
                }

                let groups = state.chat_groups.clone();
                for group in &groups {
                    let response = ui
                        .allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), theme.row_height),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let full_rect = ui.max_rect();
                                let bg = if ui.rect_contains_pointer(full_rect) {
                                    GROUP_ROW_HOVER
                                } else {
                                    GROUP_ROW_BG
                                };
                                ui.painter().rect_filled(full_rect, 0.0, bg);
                                ui.add_space(theme.item_padding);
                                ui.label(
                                    RichText::new(&group.name)
                                        .size(theme.body_size)
                                        .color(theme.text_primary()),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(theme.item_padding);
                                    ui.label(
                                        RichText::new(format!("{} members", group.member_count))
                                            .size(theme.small_size)
                                            .color(theme.text_muted()),
                                    );
                                });
                            },
                        )
                        .response;

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                // Create/Join group buttons
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    if ui.add(egui::Button::new(
                        RichText::new("+ Create Group").size(theme.font_size_small).color(theme.text_secondary()),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        state.show_create_group_modal = true;
                        state.new_group_name.clear();
                    }
                    if ui.add(egui::Button::new(
                        RichText::new("Join Group").size(theme.font_size_small).color(theme.text_secondary()),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        state.show_join_group_modal = true;
                        state.join_group_invite_code.clear();
                    }
                });
                ui.add_space(2.0);
            });
    }
}

// ── Servers Section ──

fn draw_servers_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_servers_collapsed;

    // Build a virtual server from the current connection
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let virtual_server_count = if connected { 1 } else { 0 } + state.chat_servers.len();

    if tinted_section_header(ui, &format!("Servers ({})", virtual_server_count), collapsed, SERVER_BG) {
        state.chat_servers_collapsed = !state.chat_servers_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        Frame::NONE
            .fill(SERVER_BG)
            .inner_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                // Current connected server (virtual entry)
                if connected {
                    // Server name header
                    ui.horizontal(|ui| {
                        ui.add_space(theme.item_padding);
                        let dot_sz = theme.status_dot_size;
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), dot_sz / 2.0, theme.success());
                        ui.label(
                            RichText::new(server_display_name(&state.server_url))
                                .size(theme.body_size)
                                .color(theme.text_primary())
                                .strong(),
                        );
                    });
                    ui.add_space(2.0);

                    // Merged channels: each row shows # name with voice Join/Leave on the right
                    let active = state.chat_active_channel.clone();
                    let channels = state.chat_channels.clone();
                    let is_channel_admin = {
                        let vr = viewer_role(state);
                        vr == "admin" || vr == "moderator" || vr == "mod"
                    };

                    // Track which channel index had a voice toggle click
                    let mut voice_toggle_idx: Option<(usize, bool)> = None;
                    // Track which channel had a gear icon click
                    let mut gear_click_id: Option<String> = None;

                    let ctx_time = ui.ctx().input(|i| i.time);
                    for (idx, ch) in channels.iter().enumerate() {
                        let is_active = ch.id == active;
                        let accent = theme.accent();
                        let bg = if is_active {
                            Color32::from_rgb(
                                accent.r() / 5 + 15,
                                accent.g() / 5 + 15,
                                accent.b() / 5 + 15,
                            )
                        } else {
                            SERVER_ROW_BG
                        };

                        // Check if the edit modal is open for THIS channel (for RGB border on cog)
                        let edit_modal_for_this = state.show_channel_edit_modal
                            && state.edit_channel_id == ch.id;

                        let response = ui
                            .allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), theme.row_height),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.set_min_height(theme.row_height);
                                    let full_rect = ui.max_rect();
                                    let hover = ui.rect_contains_pointer(full_rect);
                                    let fill = if hover && !is_active { SERVER_ROW_HOVER } else { bg };
                                    ui.painter().rect_filled(full_rect, 0.0, fill);
                                    // Accent left border on active channel
                                    if is_active {
                                        let bar = egui::Rect::from_min_size(
                                            full_rect.min,
                                            Vec2::new(3.0, full_rect.height()),
                                        );
                                        ui.painter().rect_filled(bar, 0.0, accent);
                                    }
                                    ui.add_space(theme.item_padding * 2.0);
                                    let text_color = if is_active {
                                        theme.text_primary()
                                    } else {
                                        theme.text_secondary()
                                    };

                                    // Green speaker icon if voice is active on this channel
                                    if ch.voice_joined {
                                        ui.label(
                                            RichText::new("\u{1F50A}")
                                                .size(theme.body_size - 2.0)
                                                .color(theme.success()),
                                        );
                                    }

                                    ui.label(
                                        RichText::new(format!("# {}", ch.name))
                                            .size(theme.body_size)
                                            .color(text_color),
                                    );

                                    // Voice Join/Leave button + gear icon on the right
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.add_space(theme.item_padding);

                                        // Mic icon (only if voice_enabled)
                                        if ch.voice_enabled {
                                            if ch.voice_joined {
                                                let mic_resp = ui.add(egui::Button::new(
                                                    RichText::new("Leave")
                                                        .size(theme.font_size_small - 1.0)
                                                        .color(theme.danger()),
                                                ).fill(Color32::TRANSPARENT));
                                                if mic_resp.clicked() {
                                                    voice_toggle_idx = Some((idx, false));
                                                }
                                            } else {
                                                let mic_color = if false { Color32::WHITE } else { Color32::from_rgb(120, 120, 130) };
                                                let mic_resp = ui.add(egui::Button::new(
                                                    RichText::new("\u{1F3A4}")
                                                        .size(theme.font_size_small)
                                                        .color(mic_color),
                                                ).fill(Color32::TRANSPARENT).frame(false));
                                                // Hover effect for mic icon
                                                if mic_resp.hovered() {
                                                    ui.painter().rect_stroke(
                                                        mic_resp.rect,
                                                        Rounding::same(3),
                                                        Stroke::new(1.0, Color32::from_rgb(52, 152, 219)),
                                                        egui::StrokeKind::Outside,
                                                    );
                                                    // Re-draw with white color on hover
                                                    ui.painter().text(
                                                        mic_resp.rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "\u{1F3A4}",
                                                        egui::FontId::proportional(theme.font_size_small),
                                                        Color32::WHITE,
                                                    );
                                                }
                                                if mic_resp.clicked() {
                                                    voice_toggle_idx = Some((idx, true));
                                                }
                                            }
                                        }

                                        // Gear icon for admin/mod to edit channel
                                        if is_channel_admin {
                                            let gear_color = if edit_modal_for_this {
                                                Color32::WHITE
                                            } else {
                                                Color32::from_rgb(120, 120, 130)
                                            };
                                            let gear_resp = ui.add(egui::Button::new(
                                                RichText::new("\u{2699}")
                                                    .size(12.0)
                                                    .color(gear_color),
                                            ).fill(Color32::TRANSPARENT).frame(false));

                                            // Hover effect for cog icon
                                            if gear_resp.hovered() && !edit_modal_for_this {
                                                ui.painter().rect_stroke(
                                                    gear_resp.rect,
                                                    Rounding::same(3),
                                                    Stroke::new(1.0, Color32::from_rgb(52, 152, 219)),
                                                    egui::StrokeKind::Outside,
                                                );
                                                ui.painter().text(
                                                    gear_resp.rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "\u{2699}",
                                                    egui::FontId::proportional(12.0),
                                                    Color32::WHITE,
                                                );
                                            }

                                            // RGB animated border when edit modal is open for this channel
                                            if edit_modal_for_this {
                                                let rgb_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
                                                ui.painter().rect_stroke(
                                                    gear_resp.rect,
                                                    Rounding::same(3),
                                                    Stroke::new(1.5, rgb_color),
                                                    egui::StrokeKind::Outside,
                                                );
                                                ui.ctx().request_repaint();
                                            }

                                            if gear_resp.clicked() {
                                                gear_click_id = Some(ch.id.clone());
                                            }
                                        }
                                    });
                                },
                            )
                            .response;

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if response.clicked() && voice_toggle_idx.is_none() && gear_click_id.is_none() {
                            state.chat_active_channel = ch.id.clone();
                            state.chat_messages.clear();
                            state.history_fetched = false;
                        }
                    }

                    // Apply gear click (open edit modal for that channel)
                    if let Some(ch_id) = gear_click_id {
                        if let Some(ch) = channels.iter().find(|c| c.id == ch_id) {
                            state.show_channel_edit_modal = true;
                            state.edit_channel_id = ch.id.clone();
                            state.edit_channel_name = ch.name.clone();
                            state.edit_channel_description = ch.description.clone();
                            state.edit_channel_confirm_delete = false;
                        }
                    }

                    // Apply voice toggle after the loop
                    if let Some((idx, joining)) = voice_toggle_idx {
                        if let Some(ch) = state.chat_channels.get_mut(idx) {
                            let ch_name = ch.name.clone();
                            ch.voice_joined = joining;
                            let msg_type = if joining { "voice_join" } else { "voice_leave" };
                            log::info!("Voice {} requested: {}", msg_type, ch_name);
                            crate::debug::push_debug(format!("Voice: {} for channel '{}'", msg_type, ch_name));
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": msg_type,
                                        "channel": ch_name,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    }

                    // + Create Channel (admin/mod only)
                    if is_channel_admin {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.add_space(theme.item_padding * 2.0);
                            if ui.add(egui::Button::new(
                                RichText::new("+ Create Channel").size(theme.font_size_small).color(theme.text_muted()),
                            ).fill(Color32::TRANSPARENT)).clicked() {
                                state.show_create_channel_modal = true;
                                state.new_channel_name.clear();
                                state.new_channel_description.clear();
                            }
                        });
                    }

                    ui.add_space(2.0);
                }

                // Additional servers from chat_servers list
                for server in state.chat_servers.clone().iter() {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(&server.name)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                    });
                    for ch in &server.channels {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new(format!("# {}", ch.name))
                                    .size(theme.font_size_body)
                                    .color(theme.text_secondary()),
                            );
                        });
                    }
                }
            });
    }

    // + Add Server button (outside the server card)
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.add(egui::Button::new(
            RichText::new("+ Add Server").size(theme.font_size_small).color(theme.text_secondary()),
        ).fill(Color32::TRANSPARENT)).clicked() {
            // placeholder
        }
    });
    ui.add_space(2.0);
}

// ─────────────────────────────── RIGHT PANEL ──────────────────────────────

fn draw_right_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ScrollArea::vertical()
        .id_salt("chat_right_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── Friends Section ──
            draw_friends_section(ui, theme, state);

            ui.add_space(4.0);

            // ── Server Members Section ──
            draw_members_section(ui, theme, state);
        });
}

/// Shared user row renderer for both friends and members lists.
/// Ensures consistent spacing, hover, click handling, and layout.
fn draw_user_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    name: &str,
    public_key: &str,
    role: &str,
    status: &str,
    state: &mut GuiState,
    ctx_time: f64,
) {
    let is_modal_target = state.chat_user_modal_open
        && state.chat_user_modal_key == public_key;

    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(ui.available_width(), theme.row_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let full_rect = ui.max_rect();
                let hovered = ui.rect_contains_pointer(full_rect);
                let bg = if hovered {
                    Color32::from_rgb(45, 45, 55)
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect_filled(full_rect, 0.0, bg);

                // RGB channeling border when this user's modal is open
                if is_modal_target {
                    let border_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
                    ui.painter().rect_stroke(
                        full_rect,
                        2.0,
                        egui::Stroke::new(1.5, border_color),
                        egui::epaint::StrokeKind::Inside,
                    );
                    ui.ctx().request_repaint();
                }

                ui.add_space(theme.item_padding);

                // Online/offline dot
                let dot_color = match status {
                    "offline" => Color32::from_rgb(100, 100, 100),
                    "away" => theme.warning(),
                    "busy" | "dnd" => theme.danger(),
                    _ => theme.success(),
                };
                let dot_sz = theme.status_dot_size;
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), dot_sz / 2.0, dot_color);

                // Name color: muted if offline
                let nc = if status == "offline" {
                    theme.text_muted()
                } else {
                    theme.text_primary()
                };
                ui.label(
                    RichText::new(name)
                        .size(theme.body_size)
                        .color(nc),
                );

                // Role badges
                draw_role_badges(ui, theme, role);
            },
        )
        .response;

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        state.chat_user_modal_open = true;
        state.chat_user_modal_name = name.to_string();
        state.chat_user_modal_key = public_key.to_string();
    }
}

fn draw_friends_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_friends_collapsed;
    let friend_count = state.chat_friends.len();

    if section_header(ui, &format!("Friends ({})", friend_count), collapsed, Color32::from_rgb(35, 35, 42)) {
        state.chat_friends_collapsed = !state.chat_friends_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        ui.spacing_mut().item_spacing.y = theme.row_gap;

        if state.chat_friends.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("No friends added yet")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(4.0);
        }

        let ctx_time = ui.ctx().input(|i| i.time);
        let friends = state.chat_friends.clone();
        for friend in &friends {
            draw_user_row(ui, theme, &friend.name, &friend.public_key, &friend.role, &friend.status, state, ctx_time);
        }
    }
}

fn draw_members_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_members_collapsed;
    let member_count = state.chat_users.len();
    let server_name = server_display_name(&state.server_url);

    if section_header(ui, &format!("{} ({})", server_name, member_count), collapsed, Color32::from_rgb(35, 35, 42)) {
        state.chat_members_collapsed = !state.chat_members_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        ui.spacing_mut().item_spacing.y = theme.row_gap;

        if state.chat_users.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("No users online")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(4.0);
        }

        // Sort: online first, then alphabetical
        let mut users = state.chat_users.clone();
        users.sort_by(|a, b| {
            let a_online = a.status != "offline";
            let b_online = b.status != "offline";
            b_online.cmp(&a_online).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let ctx_time = ui.ctx().input(|i| i.time);
        for user in &users {
            draw_user_row(ui, theme, &user.name, &user.public_key, &user.role, &user.status, state, ctx_time);
        }
    }
}

// ─────────────────────────────── CENTER PANEL ─────────────────────────────

fn draw_center_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Channel header ──
    Frame::NONE
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

                // Track alternating user colors
                let mut last_sender = String::new();
                let mut sender_parity = false; // toggles each time sender changes
                let bg_even = Color32::from_rgb(8, 8, 10);
                let bg_odd = Color32::from_rgb(16, 16, 20);
                let ctx_time = ui.ctx().input(|i| i.time);

                // Remove default item spacing so rows sit flush
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                for msg in &filtered {
                    let show_header = msg.sender_name != last_sender;
                    if show_header {
                        sender_parity = !sender_parity;
                    }
                    last_sender = msg.sender_name.clone();

                    let row_bg = if sender_parity { bg_even } else { bg_odd };
                    let icon_color = name_color(&msg.sender_name);
                    let icon_letter = msg.sender_name.chars().next().unwrap_or('?');
                    let channeling = state.chat_user_modal_open
                        && msg.sender_key == state.chat_user_modal_key;
                    let response = crate::gui::widgets::row::message_row(
                        ui,
                        theme,
                        icon_letter,
                        icon_color,
                        &msg.sender_name,
                        &msg.timestamp,
                        &msg.content,
                        show_header,
                        row_bg,
                        channeling,
                        ctx_time,
                    );
                    if response.clicked() && show_header {
                        state.chat_user_modal_open = true;
                        state.chat_user_modal_name = msg.sender_name.clone();
                        state.chat_user_modal_key = msg.sender_key.clone();
                    }
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
        Frame::NONE
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

                                // Resolve display name: prefer user_name, fall back to peer list, then "Anonymous"
                                let display_name = if !state.user_name.is_empty() {
                                    state.user_name.clone()
                                } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                                    if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "Anonymous".to_string() }
                                } else {
                                    "Anonymous".to_string()
                                };

                                let chat_msg = serde_json::json!({
                                    "type": "chat",
                                    "from": state.profile_public_key,
                                    "from_name": display_name,
                                    "content": content,
                                    "timestamp": ts,
                                    "channel": channel,
                                });
                                let json_str = chat_msg.to_string();
                                crate::debug::push_debug(format!("WS >>> {}", json_str));
                                client.send(&json_str);

                                // Track timestamp for dedup when server echoes it back
                                state.chat_sent_timestamps.push(ts);
                                // Keep only last 20 timestamps
                                if state.chat_sent_timestamps.len() > 20 {
                                    state.chat_sent_timestamps.remove(0);
                                }
                            }
                        }

                        // Store locally so user sees their own message immediately
                        let now = chrono_now_str();
                        let local_name = if !state.user_name.is_empty() {
                            state.user_name.clone()
                        } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                            if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "You".to_string() }
                        } else {
                            "You".to_string()
                        };
                        state.chat_messages.push(ChatMessage {
                            sender_name: local_name,
                            sender_key: state.profile_public_key.clone(),
                            content,
                            timestamp: now,
                            channel,
                        });

                        while state.chat_messages.len() > MAX_MESSAGES {
                            state.chat_messages.remove(0);
                        }

                        state.chat_input.clear();
                        response.request_focus();
                    }

                    if enter_pressed {
                        response.request_focus();
                    }
                });
            });
    });
}

// ─────────────────────────────── User Profile Modal ────────────────────────

fn draw_user_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.chat_user_modal_open;
    egui::Window::new("User Profile")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(320.0, 0.0))
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            let name = state.chat_user_modal_name.clone();
            let key = state.chat_user_modal_key.clone();

            // Avatar circle with initial
            ui.vertical_centered(|ui| {
                let icon_color = name_color(&name);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 30.0, icon_color);
                let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &initial,
                    egui::FontId::proportional(28.0),
                    Color32::WHITE,
                );
            });

            ui.add_space(8.0);

            // Display name (bold) + role badge
            let user_role = state.chat_users.iter()
                .find(|u| u.public_key == key)
                .map(|u| u.role.clone())
                .unwrap_or_default();

            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&name)
                            .size(theme.font_size_heading)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    if !user_role.is_empty() && user_role != "member" {
                        draw_role_badges(ui, theme, &user_role);
                    }
                });
            });

            ui.add_space(4.0);

            // Online/offline status
            let user_status = state.chat_users.iter()
                .find(|u| u.public_key == key)
                .map(|u| u.status.clone())
                .unwrap_or_else(|| "offline".to_string());
            ui.vertical_centered(|ui| {
                let (dot_color, status_text) = match user_status.as_str() {
                    "offline" => (Color32::from_rgb(100, 100, 100), "Offline"),
                    "away" => (theme.warning(), "Away"),
                    "busy" | "dnd" => (theme.danger(), "Do Not Disturb"),
                    _ => (theme.success(), "Online"),
                };
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, dot_color);
                    ui.label(
                        RichText::new(status_text)
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Public key (truncated) with Copy button
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Key:")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                let display_key = if key.len() > 16 {
                    format!("{}...{}", &key[..8], &key[key.len()-8..])
                } else {
                    key.clone()
                };
                ui.label(
                    RichText::new(&display_key)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
                if ui.add(egui::Button::new(
                    RichText::new("Copy").size(theme.font_size_small - 1.0).color(theme.text_muted()),
                ).fill(Color32::from_rgb(45, 45, 55))).clicked() {
                    ui.ctx().copy_text(key.clone());
                }
            });

            ui.add_space(12.0);

            // Determine relationship state
            let is_following = state.chat_friends.iter().any(|f| f.public_key == key);

            // Check if target user is streaming (simple heuristic: check chat_users for streaming status)
            // For now we check if the user has "streaming" in their status or role metadata
            let is_streaming = state.chat_users.iter()
                .find(|u| u.public_key == key)
                .map(|u| u.status == "streaming")
                .unwrap_or(false);

            // Action buttons
            let btn_width = 140.0;
            ui.vertical_centered(|ui| {
                // Row 1: Send DM (always) + Follow/Unfollow toggle
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Button::new(
                            RichText::new("Send DM")
                                .size(theme.font_size_body)
                                .color(theme.text_primary()),
                        )
                        .fill(Color32::from_rgb(45, 45, 55))
                        .min_size(Vec2::new(btn_width, 30.0)),
                    ).clicked() {
                        // Placeholder for DM functionality
                    }

                    ui.add_space(4.0);

                    if is_following {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Unfollow")
                                    .size(theme.font_size_body)
                                    .color(theme.text_secondary()),
                            )
                            .fill(Color32::from_rgb(50, 35, 35))
                            .min_size(Vec2::new(btn_width, 30.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "unfollow",
                                        "target": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                            // Remove from local friends list immediately
                            state.chat_friends.retain(|f| f.public_key != key);
                        }
                    } else {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Follow")
                                    .size(theme.font_size_body)
                                    .color(theme.text_on_accent()),
                            )
                            .fill(theme.accent())
                            .min_size(Vec2::new(btn_width, 30.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "follow",
                                        "target": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    }
                });

                // Watch Stream (only if user is streaming)
                if is_streaming {
                    ui.add_space(4.0);
                    if ui.add(
                        egui::Button::new(
                            RichText::new("Watch Stream")
                                .size(theme.font_size_body)
                                .color(theme.text_on_accent()),
                        )
                        .fill(Color32::from_rgb(100, 50, 150))
                        .min_size(Vec2::new(btn_width * 2.0 + 4.0, 30.0)),
                    ).clicked() {
                        // Placeholder for stream watching
                    }
                }

                // Moderator section (only if viewer is mod or admin)
                let my_role = viewer_role(state);
                let is_mod = my_role == "moderator" || my_role == "mod" || my_role == "admin";
                let is_admin = my_role == "admin";

                if is_mod {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Moderation")
                            .size(theme.font_size_small)
                            .color(theme.warning())
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Mute")
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            )
                            .fill(Color32::from_rgb(60, 50, 30))
                            .min_size(Vec2::new(btn_width, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": "mute",
                                        "target": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }

                        ui.add_space(4.0);

                        if ui.add(
                            egui::Button::new(
                                RichText::new("Kick")
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            )
                            .fill(Color32::from_rgb(70, 40, 30))
                            .min_size(Vec2::new(btn_width, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": "kick",
                                        "target": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    });
                }

                // Admin section (only if viewer is admin)
                if is_admin {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Admin")
                            .size(theme.font_size_small)
                            .color(theme.danger())
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Ban")
                                    .size(theme.font_size_body)
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(120, 30, 30))
                            .min_size(Vec2::new(90.0, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": "ban",
                                        "target": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }

                        ui.add_space(4.0);

                        let target_is_mod = user_role == "moderator" || user_role == "mod";
                        let mod_btn_label = if target_is_mod { "Unmod" } else { "Mod" };
                        if ui.add(
                            egui::Button::new(
                                RichText::new(mod_btn_label)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            )
                            .fill(Color32::from_rgb(50, 50, 70))
                            .min_size(Vec2::new(90.0, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let action = if target_is_mod { "unmod" } else { "mod" };
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": action,
                                        "target": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    });
                }

                ui.add_space(8.0);

                if ui.add(
                    egui::Button::new(
                        RichText::new("Close")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    )
                    .fill(Color32::from_rgb(40, 40, 48))
                    .min_size(Vec2::new(btn_width * 2.0 + 4.0, 28.0)),
                ).clicked() {
                    state.chat_user_modal_open = false;
                }
            });
        });
    state.chat_user_modal_open = open;
}

// ─────────────────────────────── Create Channel Modal ──────────────────────

fn draw_create_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_create_channel_modal;
    egui::Window::new("Create Channel")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Channel Name")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.new_channel_name)
                    .desired_width(300.0)
                    .hint_text("e.g. announcements"),
            );

            ui.add_space(8.0);

            ui.label(
                RichText::new("Description (optional)")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.new_channel_description)
                    .desired_width(300.0)
                    .hint_text("What is this channel about?"),
            );

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let name_valid = !state.new_channel_name.trim().is_empty();
                if ui.add_enabled(
                    name_valid,
                    egui::Button::new(
                        RichText::new("Create")
                            .size(theme.font_size_body)
                            .color(theme.text_on_accent()),
                    )
                    .fill(if name_valid { theme.accent() } else { Color32::from_rgb(60, 60, 70) })
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    // Send channel_create via WebSocket
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "channel_create",
                                "name": state.new_channel_name.trim(),
                                "description": state.new_channel_description.trim(),
                            });
                            client.send(&msg.to_string());
                            log::info!("Channel create requested: {}", state.new_channel_name.trim());
                            crate::debug::push_debug(format!("Channel create: {}", state.new_channel_name.trim()));
                        }
                    }
                    state.show_create_channel_modal = false;
                }

                ui.add_space(8.0);

                if ui.add(
                    egui::Button::new(
                        RichText::new("Cancel")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    )
                    .fill(Color32::from_rgb(40, 40, 48))
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    state.show_create_channel_modal = false;
                }
            });
        });
    state.show_create_channel_modal = open;
}

// ─────────────────────────────── Edit Channel Modal ──────────────────────

fn draw_edit_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_channel_edit_modal;
    egui::Window::new("Edit Channel")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("Editing: #{}", state.edit_channel_id))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(8.0);

            ui.label(
                RichText::new("Channel Name")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.edit_channel_name)
                    .desired_width(300.0),
            );

            ui.add_space(8.0);

            ui.label(
                RichText::new("Description")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.edit_channel_description)
                    .desired_width(300.0),
            );

            ui.add_space(8.0);

            // Voice enabled toggle (Bug 6)
            let mut voice_enabled = state.chat_channels.iter()
                .find(|c| c.id == state.edit_channel_id)
                .map(|c| c.voice_enabled)
                .unwrap_or(true);
            if ui.checkbox(&mut voice_enabled, RichText::new("Voice enabled").size(theme.font_size_small).color(theme.text_secondary())).changed() {
                if let Some(ch) = state.chat_channels.iter_mut().find(|c| c.id == state.edit_channel_id) {
                    ch.voice_enabled = voice_enabled;
                }
            }

            ui.add_space(12.0);

            // Save changes
            ui.horizontal(|ui| {
                let name_valid = !state.edit_channel_name.trim().is_empty();
                if ui.add_enabled(
                    name_valid,
                    egui::Button::new(
                        RichText::new("Save")
                            .size(theme.font_size_body)
                            .color(theme.text_on_accent()),
                    )
                    .fill(if name_valid { theme.accent() } else { Color32::from_rgb(60, 60, 70) })
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "channel_edit",
                                "channel_id": state.edit_channel_id,
                                "name": state.edit_channel_name.trim(),
                                "description": state.edit_channel_description.trim(),
                            });
                            client.send(&msg.to_string());
                            log::info!("Channel edit: {} -> {}", state.edit_channel_id, state.edit_channel_name.trim());
                        }
                    }
                    state.show_channel_edit_modal = false;
                }

                ui.add_space(8.0);

                if ui.add(
                    egui::Button::new(
                        RichText::new("Cancel")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    )
                    .fill(Color32::from_rgb(40, 40, 48))
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    state.show_channel_edit_modal = false;
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Delete channel section
            if !state.edit_channel_confirm_delete {
                if ui.add(
                    egui::Button::new(
                        RichText::new("Delete Channel")
                            .size(theme.font_size_body)
                            .color(theme.danger()),
                    )
                    .fill(Color32::from_rgb(50, 25, 25))
                    .min_size(Vec2::new(300.0, 28.0)),
                ).clicked() {
                    state.edit_channel_confirm_delete = true;
                }
            } else {
                ui.label(
                    RichText::new("Are you sure? This cannot be undone.")
                        .size(theme.font_size_small)
                        .color(theme.danger()),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Button::new(
                            RichText::new("Yes, Delete")
                                .size(theme.font_size_body)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(140, 30, 30))
                        .min_size(Vec2::new(120.0, 28.0)),
                    ).clicked() {
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {
                                let msg = serde_json::json!({
                                    "type": "channel_delete",
                                    "channel_id": state.edit_channel_id,
                                });
                                client.send(&msg.to_string());
                                log::info!("Channel delete: {}", state.edit_channel_id);
                            }
                        }
                        state.show_channel_edit_modal = false;
                    }

                    ui.add_space(8.0);

                    if ui.add(
                        egui::Button::new(
                            RichText::new("No, Keep")
                                .size(theme.font_size_body)
                                .color(theme.text_secondary()),
                        )
                        .fill(Color32::from_rgb(40, 40, 48))
                        .min_size(Vec2::new(120.0, 28.0)),
                    ).clicked() {
                        state.edit_channel_confirm_delete = false;
                    }
                });
            }
        });
    state.show_channel_edit_modal = open;
}

// ─────────────────────────────── Create Group Modal ─────────────────────

fn draw_create_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_create_group_modal;
    egui::Window::new("Create Group")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Group Name")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.new_group_name)
                    .desired_width(300.0)
                    .hint_text("e.g. My Team"),
            );

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let name_valid = !state.new_group_name.trim().is_empty();
                if ui.add_enabled(
                    name_valid,
                    egui::Button::new(
                        RichText::new("Create")
                            .size(theme.font_size_body)
                            .color(theme.text_on_accent()),
                    )
                    .fill(if name_valid { theme.accent() } else { Color32::from_rgb(60, 60, 70) })
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "group_create",
                                "name": state.new_group_name.trim(),
                            });
                            client.send(&msg.to_string());
                            log::info!("Group create requested: {}", state.new_group_name.trim());
                            crate::debug::push_debug(format!("Group create: {}", state.new_group_name.trim()));
                        }
                    }
                    state.show_create_group_modal = false;
                }

                ui.add_space(8.0);

                if ui.add(
                    egui::Button::new(
                        RichText::new("Cancel")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    )
                    .fill(Color32::from_rgb(40, 40, 48))
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    state.show_create_group_modal = false;
                }
            });
        });
    state.show_create_group_modal = open;
}

// ─────────────────────────────── Join Group Modal ──────────────────────

fn draw_join_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_join_group_modal;
    egui::Window::new("Join Group")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Invite Code")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.join_group_invite_code)
                    .desired_width(300.0)
                    .hint_text("Paste invite code here"),
            );

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let code_valid = !state.join_group_invite_code.trim().is_empty();
                if ui.add_enabled(
                    code_valid,
                    egui::Button::new(
                        RichText::new("Join")
                            .size(theme.font_size_body)
                            .color(theme.text_on_accent()),
                    )
                    .fill(if code_valid { theme.accent() } else { Color32::from_rgb(60, 60, 70) })
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "group_join",
                                "invite_code": state.join_group_invite_code.trim(),
                            });
                            client.send(&msg.to_string());
                            log::info!("Group join requested with invite code");
                            crate::debug::push_debug("Group join requested".to_string());
                        }
                    }
                    state.show_join_group_modal = false;
                }

                ui.add_space(8.0);

                if ui.add(
                    egui::Button::new(
                        RichText::new("Cancel")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    )
                    .fill(Color32::from_rgb(40, 40, 48))
                    .min_size(Vec2::new(100.0, 30.0)),
                ).clicked() {
                    state.show_join_group_modal = false;
                }
            });
        });
    state.show_join_group_modal = open;
}

// ─────────────────────────────── UI Helpers ──────────────────────────────

/// Draw a lock/unlock toggle button at the top of a panel.
/// Returns true if the button was clicked (toggle lock state).
fn draw_panel_lock_button(ui: &mut egui::Ui, _theme: &Theme, locked: bool) -> bool {
    let icon = if locked { "\u{1F512}" } else { "\u{1F513}" }; // locked/unlocked padlock
    let tooltip = if locked { "Unlock panel width" } else { "Lock panel width" };
    let response = ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            let btn = ui.add(
                egui::Button::new(
                    RichText::new(icon)
                        .size(11.0)
                        .color(if locked { Color32::from_rgb(200, 180, 100) } else { Color32::from_rgb(140, 140, 140) }),
                )
                .fill(Color32::TRANSPARENT)
                .frame(false),
            ).on_hover_text(tooltip);
            btn.clicked()
        }).inner
    }).inner;
    response
}

/// Draw a collapsible section header with a tinted background.
/// Returns true if the header was clicked (toggle).
fn tinted_section_header(ui: &mut egui::Ui, label: &str, collapsed: bool, bg: Color32) -> bool {
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 28.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let full_rect = ui.max_rect();
                // Slightly brighter header bg
                let header_bg = Color32::from_rgba_premultiplied(
                    bg.r().saturating_add(15),
                    bg.g().saturating_add(15),
                    bg.b().saturating_add(15),
                    bg.a(),
                );
                ui.painter().rect_filled(full_rect, 0.0, header_bg);
                ui.add_space(8.0);

                let arrow = if collapsed { "\u{25B8}" } else { "\u{25BE}" };
                ui.label(
                    RichText::new(arrow)
                        .size(12.0)
                        .color(Color32::from_rgb(180, 180, 180)),
                );
                ui.label(
                    RichText::new(label)
                        .size(12.0)
                        .color(Color32::from_rgb(200, 200, 200))
                        .strong(),
                );
            },
        )
        .response;

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// Draw a plain collapsible section header (for right panel).
fn section_header(ui: &mut egui::Ui, label: &str, collapsed: bool, bg: Color32) -> bool {
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let full_rect = ui.max_rect();
                ui.painter().rect_filled(full_rect, 0.0, bg);
                ui.add_space(10.0);

                let arrow = if collapsed { "\u{25B8}" } else { "\u{25BE}" };
                ui.label(
                    RichText::new(arrow)
                        .size(12.0)
                        .color(Color32::from_rgb(180, 180, 180)),
                );
                ui.label(
                    RichText::new(label)
                        .size(13.0)
                        .color(Color32::from_rgb(200, 200, 200))
                        .strong(),
                );
            },
        )
        .response;

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// Draw colored role badge pills (A=admin red, M=mod orange, V=verified blue).
fn draw_role_badges(ui: &mut egui::Ui, theme: &Theme, role: &str) {
    if role.is_empty() || role == "member" {
        return;
    }

    let (badge_char, badge_color) = match role {
        "admin" => ("A", Theme::c32(&theme.badge_admin)),
        "moderator" | "mod" => ("M", Theme::c32(&theme.badge_mod)),
        "verified" => ("V", Theme::c32(&theme.badge_verified)),
        "donor" => ("D", Theme::c32(&theme.badge_donor)),
        _ => return,
    };

    let text = RichText::new(badge_char)
        .size(theme.font_size_small - 2.0)
        .color(Color32::WHITE)
        .strong();

    let galley = ui.fonts(|f| f.layout_no_wrap(badge_char.to_string(), egui::FontId::proportional(theme.font_size_small - 2.0), Color32::WHITE));
    let badge_width = galley.size().x + 8.0;
    let badge_height = 16.0;

    let (rect, _) = ui.allocate_exact_size(Vec2::new(badge_width, badge_height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, badge_color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        badge_char,
        egui::FontId::proportional(theme.font_size_small - 2.0),
        Color32::WHITE,
    );
    let _ = text; // suppress unused warning
}

/// Get the viewer's own role by matching their public key in the user list.
fn viewer_role(state: &GuiState) -> String {
    if state.profile_public_key.is_empty() {
        return String::new();
    }
    state.chat_users.iter()
        .find(|u| u.public_key == state.profile_public_key)
        .map(|u| u.role.clone())
        .unwrap_or_default()
}

/// Extract a display name from a server URL.
fn server_display_name(url: &str) -> String {
    let cleaned = url
        .replace("https://", "")
        .replace("http://", "")
        .replace("wss://", "")
        .replace("ws://", "")
        .trim_end_matches('/')
        .trim_end_matches("/ws")
        .to_string();
    if cleaned.is_empty() {
        "Server".to_string()
    } else {
        cleaned
    }
}

/// Truncate a string to max chars, adding "..." if truncated.
fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

// ─────────────────────────────── Shared Helpers ──────────────────────────

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

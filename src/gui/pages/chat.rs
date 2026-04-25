//! 3-panel chat page matching the website layout.
//!
//! LEFT:   Collapsible DMs (red), Groups (green), Servers (blue), Connection settings
//! MIDDLE: Channel header, message feed, input bar
//! RIGHT:  Friends list, Server members list

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{ChatMessage, ChatUser, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

// Maximum messages kept in the local chat buffer (was hardcoded, now uses theme.max_messages if needed).

/// Minimum panel width in points.
const MIN_PANEL_WIDTH: f32 = 150.0;
/// Maximum panel width in points.
const MAX_PANEL_WIDTH: f32 = 400.0;

// Section tint colors now come from theme.ron (theme.dm_bg(), theme.group_bg(), theme.server_bg(), etc.)

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // ── LEFT PANEL ──
    let left_panel = egui::SidePanel::left("chat_left_panel")
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0))
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
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0))
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
        .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(0.0))
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

    // ── HELP / COMMANDS MODAL ──
    if state.show_help_modal {
        draw_help_modal(ctx, theme, state);
    }
}

// ─────────────────────────────── LEFT PANEL ───────────────────────────────

fn draw_left_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let is_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    // Show connect UI only when disconnected (no separate connection bar when connected)
    if !is_connected {
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
                    state.user_name = "DesktopUser".to_string();
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

    // ── Scrollable section area ──
    ScrollArea::vertical()
        .id_salt("chat_left_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── Scratchpad (local-only, above everything) ──
            draw_scratchpad_row(ui, theme, state);

            // ── DMs Section (red tint) ──
            draw_dm_section(ui, theme, state);

            // ── Groups Section (green tint) ──
            draw_groups_section(ui, theme, state);

            // ── Servers Section (blue tint) ──
            draw_servers_section(ui, theme, state);
        });
}

/// Local scratchpad channel - not attached to any server/group/DM.
fn draw_scratchpad_row(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let is_active = state.chat_active_channel == "scratchpad";
    let row_height = theme.row_height;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        let bg = if is_active {
            Color32::from_rgb(45, 40, 55)
        } else if resp.hovered() {
            Color32::from_rgb(40, 38, 48)
        } else {
            Color32::from_rgb(32, 32, 38)
        };
        ui.painter().rect_filled(rect, 0.0, bg);

        if is_active {
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            ui.painter().rect_filled(bar, 0.0, Color32::from_rgb(160, 140, 200));
        }

        let text_color = if is_active { theme.text_primary() } else { theme.text_secondary() };
        ui.painter().text(
            egui::pos2(rect.left() + theme.item_padding + 2.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            "# scratchpad",
            egui::FontId::proportional(theme.body_size),
            text_color,
        );
    }

    if resp.clicked() {
        state.chat_active_channel = "scratchpad".to_string();
        state.chat_messages.clear();
        state.history_fetched = false;
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

// ── DMs Section ──

fn draw_dm_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_dm_collapsed;
    let dm_count = state.chat_dms.len();
    let display_limit = state.chat_dm_display_limit;

    let mut dm_cog_clicked = false;
    if tinted_section_header_with_buttons(
        ui,
        &format!("DMs ({})", dm_count),
        collapsed,
        theme.dm_bg(),
        |ui| {
            let (cog_rect, cog_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let cog_color = if cog_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
            crate::gui::widgets::icons::paint_cog(ui.painter(), cog_rect, cog_color);
            if cog_resp.on_hover_text("DM Settings").clicked() {
                dm_cog_clicked = true;
            }
        },
    ) {
        state.chat_dm_collapsed = !state.chat_dm_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // DM settings popup
    if dm_cog_clicked {
        let menu_id = ui.id().with("dm_settings_menu");
        ui.memory_mut(|m| m.toggle_popup(menu_id));
    }
    {
        let menu_id = ui.id().with("dm_settings_menu");
        let dummy_resp = ui.allocate_rect(egui::Rect::from_min_size(ui.cursor().min, Vec2::ZERO), egui::Sense::hover());
        egui::popup_below_widget(ui, menu_id, &dummy_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(140.0);
            ui.label(RichText::new("DM Settings").size(theme.font_size_body).color(theme.text_primary()).strong());
            ui.separator();
            if ui.button("Clear All DMs").clicked() {
                state.chat_dms.clear();
            }
            if ui.button("DM Notifications").clicked() {
                // TODO: toggle DM notifications
            }
        });
    }

    if !collapsed {
        Frame::NONE
            .fill(theme.dm_bg())
            .inner_margin(egui::Margin::symmetric(0, 1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
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
                let visible_dms: Vec<_> = if display_limit > 0 && dms.len() > display_limit {
                    dms.iter().take(display_limit).collect()
                } else {
                    dms.iter().collect()
                };

                for dm in &visible_dms {
                    let dm_channel = format!("dm:{}", dm.user_key);
                    let is_active = state.chat_active_channel == dm_channel;

                    // Channel-row style DM entry
                    let row_height = theme.row_height;
                    let (full_rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    if ui.is_rect_visible(full_rect) {
                        let bg = if is_active || response.hovered() {
                            theme.dm_row_hover()
                        } else {
                            theme.dm_row_bg()
                        };
                        ui.painter().rect_filled(full_rect, 0.0, bg);

                        // Active indicator bar
                        if is_active {
                            let bar = egui::Rect::from_min_size(full_rect.min, Vec2::new(3.0, full_rect.height()));
                            ui.painter().rect_filled(bar, 0.0, Color32::from_rgb(200, 80, 80));
                        }

                        let mut cursor_x = full_rect.left() + theme.item_padding + 2.0;
                        let cy = full_rect.center().y;

                        // Unread dot
                        if dm.unread {
                            let dot_r = theme.status_dot_size * 0.375;
                            ui.painter().circle_filled(egui::pos2(cursor_x + dot_r, cy), dot_r, Color32::from_rgb(200, 80, 80));
                            cursor_x += dot_r * 2.0 + 3.0;
                        }

                        // DM icon prefix + user name (like "@ username")
                        let name_color = if is_active || dm.unread { theme.text_primary() } else { theme.text_secondary() };
                        ui.painter().text(
                            egui::pos2(cursor_x, cy),
                            egui::Align2::LEFT_CENTER,
                            &format!("@ {}", dm.user_name),
                            egui::FontId::proportional(theme.body_size),
                            name_color,
                        );
                    }

                    if response.clicked() {
                        state.chat_active_channel = dm_channel;
                        state.chat_messages.clear();
                        state.history_fetched = false;
                        // Request DM history from server
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {
                                let msg = serde_json::json!({
                                    "type": "dm_open",
                                    "partner": dm.user_key,
                                });
                                client.send(&msg.to_string());
                            }
                        }
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    // Right-click context menu on DMs
                    response.context_menu(|ui| {
                        if ui.button("View Profile").clicked() {
                            state.chat_user_modal_open = true;
                            state.chat_user_modal_key = dm.user_key.clone();
                            ui.close_menu();
                        }
                        if ui.button(RichText::new("Close Conversation").color(Color32::from_rgb(200, 80, 80))).clicked() {
                            state.chat_dms.retain(|d| d.user_key != dm.user_key);
                            if state.chat_active_channel == format!("dm:{}", dm.user_key) {
                                state.chat_active_channel = "general".to_string();
                            }
                            ui.close_menu();
                        }
                    });
                }
            });
    }
}

// ── Groups Section ──

fn draw_groups_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_groups_collapsed;
    let group_count = state.chat_groups.len();

    // Track button clicks from the header
    let mut create_clicked = false;
    let mut join_clicked = false;
    let mut groups_cog_clicked = false;

    if tinted_section_header_with_buttons(
        ui,
        &format!("Groups ({})", group_count),
        collapsed,
        theme.group_bg(),
        |ui| {
            // Cog button (group settings)
            {
                let (cog_rect, cog_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
                let cog_color = if cog_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
                crate::gui::widgets::icons::paint_cog(ui.painter(), cog_rect, cog_color);
                if cog_resp.on_hover_text("Group Settings").clicked() {
                    groups_cog_clicked = true;
                }
            }
            // + button (create group)
            {
                let (plus_rect, plus_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
                let plus_color = if plus_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
                crate::gui::widgets::icons::paint_plus(ui.painter(), plus_rect, plus_color);
                if plus_resp.on_hover_text("Create Group").clicked() {
                    create_clicked = true;
                }
            }
            // Join button (arrow icon)
            {
                let (arrow_rect, arrow_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
                let arrow_color = if arrow_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
                crate::gui::widgets::icons::paint_arrow_right(ui.painter(), arrow_rect, arrow_color);
                if arrow_resp.on_hover_text("Join Group").clicked() {
                    join_clicked = true;
                }
            }
        },
    ) {
        state.chat_groups_collapsed = !state.chat_groups_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // Groups settings popup
    if groups_cog_clicked {
        let menu_id = ui.id().with("groups_settings_menu");
        ui.memory_mut(|m| m.toggle_popup(menu_id));
    }
    {
        let menu_id = ui.id().with("groups_settings_menu");
        let dummy_resp = ui.allocate_rect(egui::Rect::from_min_size(ui.cursor().min, Vec2::ZERO), egui::Sense::hover());
        egui::popup_below_widget(ui, menu_id, &dummy_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(140.0);
            ui.label(RichText::new("Groups Settings").size(theme.font_size_body).color(theme.text_primary()).strong());
            ui.separator();
            if ui.button("Group Notifications").clicked() {
                // TODO: toggle group notifications
            }
            if ui.button("Sort by Activity").clicked() {
                // TODO: sort groups
            }
        });
    }

    if create_clicked {
        state.show_create_group_modal = true;
        state.new_group_name.clear();
    }
    if join_clicked {
        state.show_join_group_modal = true;
        state.join_group_invite_code.clear();
    }

    if !collapsed {
        Frame::NONE
            .fill(theme.group_bg())
            .inner_margin(egui::Margin::symmetric(0, 1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;

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

                // Groups render like servers (expandable header + nested channels)
                // but remain P2P under the hood. Header: <arrow> <cog> <name> <count>.
                // Channels inside route through `group_msg` with the group id. When
                // server-side multi-channel support lands, additional channels appear
                // here automatically from the GroupList data.
                let groups = state.chat_groups.clone();
                let ctx_time = ui.ctx().input(|i| i.time);
                let mut toggle_group_idx: Option<usize> = None;
                let mut leave_group_id: Option<String> = None;
                let mut open_server_settings = false;

                for (gi, group) in groups.iter().enumerate() {
                    if gi > 0 { ui.add_space(4.0); }

                    let hdr_height = 24.0;
                    let full_w = ui.available_width();
                    let (hdr_rect, _) = ui.allocate_exact_size(
                        Vec2::new(full_w, hdr_height),
                        egui::Sense::hover(),
                    );

                    let cog_click_rect;
                    if ui.is_rect_visible(hdr_rect) {
                        let hdr_bg = Color32::from_rgba_premultiplied(
                            theme.group_bg().r().saturating_add(20),
                            theme.group_bg().g().saturating_add(20),
                            theme.group_bg().b().saturating_add(20),
                            theme.group_bg().a(),
                        );
                        ui.painter().rect_filled(hdr_rect, 0.0, hdr_bg);

                        let mut cx = hdr_rect.left() + 8.0;
                        let cy = hdr_rect.center().y;

                        // Collapse arrow
                        let arrow_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        if group.collapsed {
                            crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        } else {
                            crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        }
                        cx += 14.0;

                        // Cog icon with RGB hover (matches nav)
                        cog_click_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, hdr_rect.top()), Vec2::new(14.0, hdr_height));
                        let cog_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default());
                        let on_cog = cog_click_rect.contains(hover_pos);
                        let cog_color = if on_cog { theme.accent() } else { Color32::from_rgb(140, 140, 150) };
                        crate::gui::widgets::icons::paint_cog(ui.painter(), cog_icon_rect, cog_color);
                        if on_cog {
                            let rgb = crate::gui::widgets::row::rgb_from_time(ctx_time);
                            ui.painter().rect_stroke(
                                cog_icon_rect.expand(2.0),
                                Rounding::same(3),
                                Stroke::new(1.0, rgb),
                                egui::StrokeKind::Outside,
                            );
                            ui.ctx().request_repaint();
                        }
                        cx += 14.0;

                        // Group name
                        ui.painter().text(
                            egui::pos2(cx, cy),
                            egui::Align2::LEFT_CENTER,
                            &group.name,
                            egui::FontId::proportional(theme.body_size),
                            theme.text_primary(),
                        );

                        // Member count on right
                        ui.painter().text(
                            egui::pos2(hdr_rect.right() - theme.item_padding, cy),
                            egui::Align2::RIGHT_CENTER,
                            &format!("{}", group.member_count),
                            egui::FontId::proportional(theme.small_size),
                            theme.text_muted(),
                        );
                    } else {
                        cog_click_rect = hdr_rect;
                    }

                    // Interact order: general header first, then specific arrow/cog so those
                    // win hit-test within their rects (egui: last interact wins).
                    let hdr_interact = ui.interact(hdr_rect, ui.id().with("grp_ctx").with(gi), egui::Sense::click());
                    let show_menu_rc = hdr_interact.secondary_clicked();

                    let arrow_click = egui::Rect::from_min_size(hdr_rect.min, Vec2::new(22.0, hdr_height));
                    let arrow_resp = ui.interact(arrow_click, ui.id().with("grp_arrow").with(gi), egui::Sense::click());
                    if arrow_resp.clicked() { toggle_group_idx = Some(gi); }
                    if arrow_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

                    let cog_resp = ui.interact(cog_click_rect, ui.id().with("grp_cog").with(gi), egui::Sense::click());
                    if cog_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    let show_group_menu = cog_resp.clicked() || cog_resp.secondary_clicked();

                    let menu_id = ui.id().with("grp_menu").with(gi);
                    if show_group_menu || show_menu_rc {
                        ui.memory_mut(|m| m.toggle_popup(menu_id));
                    }
                    egui::popup_below_widget(ui, menu_id, &cog_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(180.0);
                        ui.label(RichText::new(&group.name).size(theme.font_size_body).color(theme.text_primary()).strong());
                        ui.separator();
                        if ui.button("Group Settings").clicked() {
                            open_server_settings = true;
                            state.chat_user_modal_key = group.id.clone(); // piggyback context
                        }
                        let invite_url = format!("https://united-humanity.us/chat/group/{}", group.id);
                        if ui.button("Copy Invite Link").clicked() { ui.ctx().copy_text(invite_url); }
                        if ui.button("Copy Group ID").clicked() { ui.ctx().copy_text(group.id.clone()); }
                        ui.separator();
                        if ui.button("What's a group vs a server?").clicked() {
                            state.active_help_topic = Some("groups-vs-servers".to_string());
                        }
                        ui.separator();
                        if ui.button(RichText::new("Leave Group").color(Color32::from_rgb(200, 80, 80))).clicked() {
                            leave_group_id = Some(group.id.clone());
                        }
                    });

                    // Channel rows (only when expanded). Same layout as server
                    // channels: voice-mic icon, settings cog, then the # name.
                    if !group.collapsed {
                        let is_group_admin = true; // TODO: per-group role once server reports it
                        for ch in group.channels.iter() {
                            let is_active = state.chat_active_channel == ch.id;
                            let accent = Color32::from_rgb(80, 200, 80);
                            let base_bg = if is_active {
                                Color32::from_rgb(accent.r() / 5 + 15, accent.g() / 5 + 15, accent.b() / 5 + 15)
                            } else {
                                theme.group_row_bg()
                            };
                            let row_w = ui.available_width();
                            let (row_rect, response) = ui.allocate_exact_size(
                                Vec2::new(row_w, theme.row_height),
                                egui::Sense::click(),
                            );

                            let mut voice_icon_rect = egui::Rect::NOTHING;
                            let mut gear_icon_rect = egui::Rect::NOTHING;

                            if ui.is_rect_visible(row_rect) {
                                let hover = response.hovered();
                                let fill = if hover && !is_active { theme.group_row_hover() } else { base_bg };
                                ui.painter().rect_filled(row_rect, 0.0, fill);
                                if is_active {
                                    let bar = egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_rect.height()));
                                    ui.painter().rect_filled(bar, 0.0, accent);
                                }

                                let text_color = if is_active { theme.text_primary() } else { theme.text_secondary() };
                                let icon_size = 12.0;
                                let cy = row_rect.center().y;
                                let mut cx = row_rect.left() + theme.item_padding + 2.0;
                                let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default());

                                // 1. Voice icon
                                if ch.voice_enabled {
                                    let icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - icon_size * 0.5), Vec2::splat(icon_size));
                                    voice_icon_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, row_rect.top()), Vec2::new(icon_size + 4.0, row_rect.height()));
                                    if ch.voice_joined {
                                        crate::gui::widgets::icons::paint_speaker(ui.painter(), icon_rect, theme.success());
                                    } else {
                                        let on_voice = response.hovered() && voice_icon_rect.contains(hover_pos);
                                        let mic_color = if on_voice { Color32::WHITE } else { Color32::from_rgb(100, 100, 110) };
                                        crate::gui::widgets::icons::paint_mic(ui.painter(), icon_rect, mic_color);
                                    }
                                    cx += icon_size + 2.0;
                                }

                                // 2. Cog icon (admin only)
                                if is_group_admin {
                                    let cog_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - icon_size * 0.5), Vec2::splat(icon_size));
                                    gear_icon_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, row_rect.top()), Vec2::new(icon_size + 4.0, row_rect.height()));
                                    let on_gear = response.hovered() && gear_icon_rect.contains(hover_pos);
                                    let cog_color = if on_gear { theme.accent() } else { Color32::from_rgb(100, 100, 110) };
                                    crate::gui::widgets::icons::paint_cog(ui.painter(), cog_rect, cog_color);
                                    if on_gear {
                                        let rgb = crate::gui::widgets::row::rgb_from_time(ctx_time);
                                        ui.painter().rect_stroke(
                                            cog_rect.expand(2.0),
                                            Rounding::same(2),
                                            Stroke::new(1.0, rgb),
                                            egui::StrokeKind::Outside,
                                        );
                                        ui.ctx().request_repaint();
                                    }
                                    cx += icon_size + 2.0;
                                }

                                // 3. # channel name
                                ui.painter().text(
                                    egui::pos2(cx + 2.0, cy),
                                    egui::Align2::LEFT_CENTER,
                                    &format!("# {}", ch.name),
                                    egui::FontId::proportional(theme.body_size),
                                    text_color,
                                );
                            }

                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if response.clicked() {
                                let click_pos = ui.ctx().input(|i| i.pointer.interact_pos().unwrap_or_default());
                                if voice_icon_rect.contains(click_pos) && ch.voice_enabled {
                                    // TODO: wire group voice join/leave through the relay
                                } else if gear_icon_rect.contains(click_pos) && is_group_admin {
                                    state.show_channel_edit_modal = true;
                                    state.edit_channel_id = ch.id.clone();
                                    state.edit_channel_name = ch.name.clone();
                                    state.edit_channel_description = ch.description.clone();
                                } else {
                                    state.chat_active_channel = ch.id.clone();
                                    state.chat_messages.clear();
                                    state.history_fetched = false;
                                    if let Some(ref client) = state.ws_client {
                                        if client.is_connected() {
                                            let msg = serde_json::json!({
                                                "type": "group_history_request",
                                                "group_id": group.id,
                                            });
                                            client.send(&msg.to_string());
                                        }
                                    }
                                }
                            }
                            response.context_menu(|ui| {
                                let link = format!("https://united-humanity.us/chat/group/{}/{}", group.id, ch.name);
                                if ui.button("Copy Channel Link").clicked() {
                                    ui.ctx().copy_text(link);
                                    ui.close_menu();
                                }
                            });
                        }

                        // Hint for planned multi-channel support
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new("+ Channel (coming soon)")
                                    .size(theme.small_size)
                                    .color(theme.text_muted())
                                    .italics(),
                            );
                        });
                    }
                }

                if let Some(idx) = toggle_group_idx {
                    if let Some(g) = state.chat_groups.get_mut(idx) {
                        g.collapsed = !g.collapsed;
                    }
                }

                // Navigate to server-settings page if a group menu asked for it.
                if open_server_settings {
                    state.active_page = crate::gui::GuiPage::ServerSettings;
                }

                // Apply group leave (send to server so groups don't come back)
                if let Some(gid) = leave_group_id {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "group_leave",
                                "group_id": gid,
                            });
                            client.send(&msg.to_string());
                        }
                    }
                    state.chat_groups.retain(|g| g.id != gid);
                    if state.chat_active_channel.starts_with(&format!("group:{}", gid)) {
                        state.chat_active_channel = "general".to_string();
                    }
                }
            });
    }
}

// ── Servers Section ──

fn draw_servers_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_servers_collapsed;

    // Build a virtual server from the current connection
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    let virtual_server_count = if connected { 1 } else { 0 } + state.chat_servers.len();

    let header_label = format!("Servers ({})", virtual_server_count);

    let mut add_server_clicked = false;
    if tinted_section_header_with_buttons(
        ui,
        &header_label,
        collapsed,
        theme.server_bg(),
        |ui| {
            let (plus_rect, plus_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let plus_color = if plus_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
            crate::gui::widgets::icons::paint_plus(ui.painter(), plus_rect, plus_color);
            if plus_resp.on_hover_text("Add Server").clicked() {
                add_server_clicked = true;
            }
        },
    ) {
        state.chat_servers_collapsed = !state.chat_servers_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // (disconnect is handled inside the server name row)
    if add_server_clicked {
        // TODO: open add-server modal
    }

    if !collapsed {
        Frame::NONE
            .fill(theme.server_bg())
            .inner_margin(egui::Margin::symmetric(0, 1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                // Current connected server (virtual entry)
                if connected {
                    let online_count = state.chat_users.iter().filter(|u| u.status != "offline").count();
                    let svr_hdr_height = 24.0;
                    let svr_full_w = ui.available_width();
                    let svr_name = server_display_name(&state.server_url);
                    let svr_collapsed = state.chat_connected_server_collapsed;
                    let ctx_time = ui.ctx().input(|i| i.time);

                    // Server header: <collapse> <cog> <name (online)> ... <member count>
                    let (svr_rect, _) = ui.allocate_exact_size(Vec2::new(svr_full_w, svr_hdr_height), egui::Sense::hover());
                    let cog_click_rect;
                    let mut svr_disconnect = false;
                    if ui.is_rect_visible(svr_rect) {
                        let svr_bg = Color32::from_rgba_premultiplied(
                            theme.server_bg().r().saturating_add(20),
                            theme.server_bg().g().saturating_add(20),
                            theme.server_bg().b().saturating_add(20),
                            theme.server_bg().a(),
                        );
                        ui.painter().rect_filled(svr_rect, 0.0, svr_bg);

                        let mut cx = svr_rect.left() + 8.0;
                        let cy = svr_rect.center().y;

                        // Collapse arrow
                        let arrow_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        if svr_collapsed {
                            crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        } else {
                            crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        }
                        cx += 14.0;

                        // Cog icon (with RGB hover effect matching nav buttons)
                        cog_click_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, svr_rect.top()), Vec2::new(14.0, svr_hdr_height));
                        let cog_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default());
                        let on_cog = cog_click_rect.contains(hover_pos);
                        let cog_color = if on_cog { theme.accent() } else { Color32::from_rgb(140, 140, 150) };
                        crate::gui::widgets::icons::paint_cog(ui.painter(), cog_icon_rect, cog_color);
                        if on_cog {
                            let rgb = crate::gui::widgets::row::rgb_from_time(ctx_time);
                            ui.painter().rect_stroke(
                                cog_icon_rect.expand(2.0),
                                Rounding::same(3),
                                Stroke::new(1.0, rgb),
                                egui::StrokeKind::Outside,
                            );
                            ui.ctx().request_repaint();
                        }
                        cx += 14.0;

                        // Green dot
                        let dot_r = theme.status_dot_size / 2.0;
                        ui.painter().circle_filled(egui::pos2(cx + dot_r, cy), dot_r, theme.success());
                        cx += theme.status_dot_size + 4.0;

                        // Server name + online count
                        ui.painter().text(
                            egui::pos2(cx, cy),
                            egui::Align2::LEFT_CENTER,
                            &format!("{} ({})", svr_name, online_count),
                            egui::FontId::proportional(theme.body_size),
                            theme.text_primary(),
                        );

                        // Member count on right
                        ui.painter().text(
                            egui::pos2(svr_rect.right() - theme.item_padding, cy),
                            egui::Align2::RIGHT_CENTER,
                            &format!("{}", state.chat_users.len()),
                            egui::FontId::proportional(theme.small_size),
                            theme.text_muted(),
                        );
                    } else {
                        cog_click_rect = svr_rect;
                    }

                    // Full-header interact FIRST so later specific-region interacts (arrow, cog)
                    // take priority. Last-registered wins hit-test for overlapping rects.
                    let hdr_interact = ui.interact(svr_rect, ui.id().with("svr_ctx"), egui::Sense::click());
                    let show_menu_rc = hdr_interact.secondary_clicked();

                    // Collapse arrow click target (overrides hdr_interact in its 22px region)
                    let arrow_click = egui::Rect::from_min_size(svr_rect.min, Vec2::new(22.0, svr_hdr_height));
                    let arrow_resp = ui.interact(arrow_click, ui.id().with("svr_arrow"), egui::Sense::click());
                    if arrow_resp.clicked() {
                        state.chat_connected_server_collapsed = !state.chat_connected_server_collapsed;
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                    if arrow_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Cog click target - opens settings popup (registered last = priority)
                    let cog_resp = ui.interact(cog_click_rect, ui.id().with("svr_cog"), egui::Sense::click());
                    if cog_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let show_svr_menu = cog_resp.clicked() || cog_resp.secondary_clicked();

                    let menu_id = ui.id().with("svr_menu");
                    if show_svr_menu || show_menu_rc {
                        ui.memory_mut(|m| m.toggle_popup(menu_id));
                    }
                    let is_server_admin = {
                        let vr = viewer_role(state);
                        vr == "admin" || vr == "moderator" || vr == "mod"
                    };
                    egui::popup_below_widget(ui, menu_id, &cog_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(160.0);
                        ui.label(RichText::new(&svr_name).size(theme.font_size_body).color(theme.text_primary()).strong());
                        ui.separator();
                        let invite_url = format!("https://united-humanity.us/chat");
                        if ui.button("Copy Invite Link").clicked() {
                            ui.ctx().copy_text(invite_url);
                        }
                        if is_server_admin {
                            ui.separator();
                            if ui.button("Server Settings").clicked() {
                                state.active_page = crate::gui::GuiPage::ServerSettings;
                            }
                        }
                        ui.separator();
                        if ui.button("Mute Server").clicked() {
                            // TODO: implement mute
                        }
                        if ui.button(RichText::new("Disconnect").color(Color32::from_rgb(200, 80, 80))).clicked() {
                            svr_disconnect = true;
                        }
                    });

                    if svr_disconnect {
                        if let Some(ref mut client) = state.ws_client {
                            client.disconnect();
                        }
                        state.ws_client = None;
                        state.ws_status = "Disconnected".to_string();
                        state.ws_manually_disconnected = true;
                        state.chat_users.clear();
                    }

                    // Merged channels (only if not collapsed)
                    if !svr_collapsed {
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
                            theme.server_row_bg()
                        };

                        // Check if the edit modal is open for THIS channel (for RGB border on cog)
                        let edit_modal_for_this = state.show_channel_edit_modal
                            && state.edit_channel_id == ch.id;

                        // Single click target for the whole row; check click position
                        // to distinguish voice/gear/channel clicks without overlapping interact().
                        let row_w = ui.available_width();
                        let (row_rect, response) = ui.allocate_exact_size(
                            Vec2::new(row_w, theme.row_height),
                            egui::Sense::click(),
                        );

                        let mut voice_icon_rect = egui::Rect::NOTHING;
                        let mut gear_icon_rect = egui::Rect::NOTHING;

                        if ui.is_rect_visible(row_rect) {
                            let hover = response.hovered();
                            let fill = if hover && !is_active { theme.server_row_hover() } else { bg };
                            ui.painter().rect_filled(row_rect, 0.0, fill);
                            if is_active {
                                let bar = egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_rect.height()));
                                ui.painter().rect_filled(bar, 0.0, accent);
                            }

                            let text_color = if is_active { theme.text_primary() } else { theme.text_secondary() };
                            let icon_size = 12.0;
                            let cy = row_rect.center().y;
                            let mut cx = row_rect.left() + theme.item_padding + 2.0;

                            // 1. Voice chat icon
                            if ch.voice_enabled {
                                let icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - icon_size * 0.5), Vec2::splat(icon_size));
                                voice_icon_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, row_rect.top()), Vec2::new(icon_size + 4.0, row_rect.height()));
                                if ch.voice_joined {
                                    crate::gui::widgets::icons::paint_speaker(ui.painter(), icon_rect, theme.success());
                                } else {
                                    let on_voice = response.hovered() && voice_icon_rect.contains(ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default()));
                                    let mic_color = if on_voice { Color32::WHITE } else { Color32::from_rgb(100, 100, 110) };
                                    crate::gui::widgets::icons::paint_mic(ui.painter(), icon_rect, mic_color);
                                }
                                cx += icon_size + 2.0;
                            }

                            // 2. Settings/cog icon (admin only)
                            if is_channel_admin {
                                let cog_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - icon_size * 0.5), Vec2::splat(icon_size));
                                gear_icon_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, row_rect.top()), Vec2::new(icon_size + 4.0, row_rect.height()));
                                let on_gear = response.hovered() && gear_icon_rect.contains(ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default()));
                                let gear_color = if edit_modal_for_this || on_gear {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(100, 100, 110)
                                };
                                crate::gui::widgets::icons::paint_cog(ui.painter(), cog_rect, gear_color);
                                if edit_modal_for_this {
                                    let rgb_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
                                    ui.painter().rect_stroke(
                                        cog_rect, Rounding::same(2),
                                        Stroke::new(1.5, rgb_color), egui::StrokeKind::Outside,
                                    );
                                    ui.ctx().request_repaint();
                                }
                                cx += icon_size + 2.0;
                            }

                            // 3. # Channel name
                            ui.painter().text(
                                egui::pos2(cx + 2.0, cy),
                                egui::Align2::LEFT_CENTER,
                                &format!("# {}", ch.name),
                                egui::FontId::proportional(theme.body_size),
                                text_color,
                            );
                        }

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if response.clicked() {
                            let click_pos = ui.ctx().input(|i| i.pointer.interact_pos().unwrap_or_default());
                            if voice_icon_rect.contains(click_pos) && ch.voice_enabled {
                                voice_toggle_idx = Some((idx, !ch.voice_joined));
                            } else if gear_icon_rect.contains(click_pos) && is_channel_admin {
                                gear_click_id = Some(ch.id.clone());
                            } else {
                                state.chat_active_channel = ch.id.clone();
                                state.chat_messages.clear();
                                state.history_fetched = false;
                            }
                        }
                        // Right-click: copy channel link
                        response.context_menu(|ui| {
                            let link = format!("https://united-humanity.us/chat/{}", ch.name);
                            if ui.button("Copy Channel Link").clicked() {
                                ui.ctx().copy_text(link);
                                ui.close_menu();
                            }
                        });
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
                        ui.add_space(1.0);
                        ui.horizontal(|ui| {
                            ui.add_space(theme.item_padding + 2.0);
                            if widgets::Button::ghost("+ Create Channel")
                                .size(widgets::ButtonSize::Small)
                                .show(ui, theme)
                            {
                                state.show_create_channel_modal = true;
                                state.new_channel_name.clear();
                                state.new_channel_description.clear();
                            }
                        });
                    }

                    ui.add_space(2.0);
                    } // end if !svr_collapsed
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

    let row_height = theme.row_height;
    let (full_rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(full_rect) {
        let bg = if response.hovered() {
            Color32::from_rgb(45, 45, 55)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(full_rect, 0.0, bg);

        // RGB border when this user's modal is open
        if is_modal_target {
            let border_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
            ui.painter().rect_stroke(
                full_rect, 2.0,
                egui::Stroke::new(1.5, border_color),
                egui::epaint::StrokeKind::Inside,
            );
            ui.ctx().request_repaint();
        }

        let mut cx = full_rect.left() + theme.item_padding;
        let cy = full_rect.center().y;

        // Online/offline dot
        let dot_color = match status {
            "offline" => Color32::from_rgb(100, 100, 100),
            "away" => theme.warning(),
            "busy" | "dnd" => theme.danger(),
            _ => theme.success(),
        };
        let dot_r = theme.status_dot_size / 2.0;
        ui.painter().circle_filled(egui::pos2(cx + dot_r, cy), dot_r, dot_color);
        cx += theme.status_dot_size + 4.0;

        // Name
        let nc = if status == "offline" { theme.text_muted() } else { theme.text_primary() };
        let name_galley = ui.painter().layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(theme.body_size),
            nc,
        );
        ui.painter().galley(egui::pos2(cx, cy - name_galley.size().y / 2.0), name_galley, nc);
        // Advance past name for role badges
        let name_w = ui.painter().layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(theme.body_size),
            nc,
        ).size().x;
        cx += name_w + 4.0;

        // Role badge (single letter pill)
        if !role.is_empty() && role != "member" {
            let (badge_color, badge_letter) = match role {
                "admin" => (Color32::from_rgb(231, 76, 60), "A"),
                "mod" | "moderator" => (Color32::from_rgb(155, 89, 182), "M"),
                "verified" => (Color32::from_rgb(52, 152, 219), "V"),
                "donor" => (Color32::from_rgb(241, 196, 15), "D"),
                _ => (Color32::from_rgb(100, 100, 100), "?"),
            };
            let badge_rect = egui::Rect::from_min_size(
                egui::pos2(cx, cy - 7.0),
                Vec2::new(14.0, 14.0),
            );
            ui.painter().rect_filled(badge_rect, Rounding::same(3), badge_color);
            ui.painter().text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                badge_letter,
                egui::FontId::proportional(9.0),
                Color32::WHITE,
            );
        }
    }

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
                let ac = state.chat_active_channel.clone();
                if ac.starts_with("dm:") {
                    // DM header: back button + partner name
                    if widgets::Button::ghost("\u{2190} Back").show(ui, theme) {
                        state.chat_active_channel = "general".to_string();
                    }
                    let partner_key = &ac[3..];
                    let partner_name = state.chat_dms.iter()
                        .find(|d| d.user_key == partner_key)
                        .map(|d| d.user_name.clone())
                        .unwrap_or_else(|| partner_key.to_string());
                    ui.label(
                        RichText::new(format!("DM: {}", partner_name))
                            .size(theme.font_size_heading)
                            .color(Color32::from_rgb(220, 120, 120))
                            .strong(),
                    );
                } else if ac.starts_with("group:") {
                    // Group header: back button + group name
                    if widgets::Button::ghost("\u{2190} Back").show(ui, theme) {
                        state.chat_active_channel = "general".to_string();
                    }
                    let group_id = &ac[6..];
                    let group_name = state.chat_groups.iter()
                        .find(|g| g.id == group_id)
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| group_id.to_string());
                    ui.label(
                        RichText::new(format!("Group: {}", group_name))
                            .size(theme.font_size_heading)
                            .color(Color32::from_rgb(120, 220, 120))
                            .strong(),
                    );
                } else {
                    // Normal channel header
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
                    // Extract image URLs from the message so we can render them
                    // as inline thumbnails instead of raw /uploads/... text.
                    let image_urls = crate::gui::widgets::image_cache::extract_image_urls(&msg.content);
                    let display_text = if image_urls.is_empty() {
                        msg.content.clone()
                    } else {
                        crate::gui::widgets::image_cache::strip_image_urls(&msg.content)
                    };

                    // Only call message_row if there's something to show — an
                    // image-only message should not produce an empty text row
                    // unless it needs the sender header.
                    let need_text_row = show_header || !display_text.trim().is_empty();
                    if need_text_row {
                        let row_resp = crate::gui::widgets::row::message_row(
                            ui,
                            theme,
                            icon_letter,
                            icon_color,
                            &msg.sender_name,
                            &msg.timestamp,
                            &display_text,
                            show_header,
                            row_bg,
                            channeling,
                            ctx_time,
                        );
                        if row_resp.userbox_clicked(ui.ctx()) {
                            state.chat_user_modal_open = true;
                            state.chat_user_modal_name = msg.sender_name.clone();
                            state.chat_user_modal_key = msg.sender_key.clone();
                        }
                    }

                    // Render each attached image as a clickable thumbnail
                    // indented under the message text.
                    if !image_urls.is_empty() {
                        let server_url = state.server_url.clone();
                        const THUMB_INDENT: f32 = 40.0;
                        const THUMB_W: f32 = 240.0;
                        for raw_url in image_urls {
                            let url = crate::gui::widgets::image_cache::resolve_url(&raw_url, &server_url);
                            state.image_cache.request(&url);
                            let status = state.image_cache.status(&url);

                            match status {
                                crate::gui::widgets::image_cache::ImageStatus::Ready { width, height } => {
                                    let aspect = width as f32 / height.max(1) as f32;
                                    let thumb_h = (THUMB_W / aspect.max(0.1)).min(360.0).max(60.0);
                                    let row_w = ui.available_width();
                                    let (row_rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(row_w, thumb_h + 4.0),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(row_rect, 0.0, row_bg);
                                    if let Some(tex) = state.image_cache.get_texture(&url) {
                                        let img_rect = egui::Rect::from_min_size(
                                            egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.top() + 2.0),
                                            Vec2::new(THUMB_W, thumb_h),
                                        );
                                        let mut mesh = egui::Mesh::with_texture(tex.id());
                                        mesh.add_rect_with_uv(
                                            img_rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                        ui.painter().add(egui::Shape::mesh(mesh));
                                        // Faint border
                                        ui.painter().rect_stroke(
                                            img_rect,
                                            Rounding::same(3),
                                            Stroke::new(1.0, theme.border()),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        state.image_viewer_url = Some(url.clone());
                                    }
                                }
                                crate::gui::widgets::image_cache::ImageStatus::Fetching
                                | crate::gui::widgets::image_cache::ImageStatus::Idle => {
                                    let row_w = ui.available_width();
                                    let (row_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(row_w, 20.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(row_rect, 0.0, row_bg);
                                    ui.painter().text(
                                        egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        &format!("Loading image… {}", crate::gui::widgets::image_cache::filename_from_url(&url)),
                                        egui::FontId::proportional(theme.font_size_small),
                                        theme.text_muted(),
                                    );
                                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                                }
                                crate::gui::widgets::image_cache::ImageStatus::Failed(err) => {
                                    let row_w = ui.available_width();
                                    let (row_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(row_w, 20.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(row_rect, 0.0, row_bg);
                                    ui.painter().text(
                                        egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        &format!("Image failed: {err}"),
                                        egui::FontId::proportional(theme.font_size_small),
                                        theme.danger(),
                                    );
                                }
                            }
                        }
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
                Frame::NONE
                    .fill(Color32::from_rgb(30, 30, 38))
                    .rounding(Rounding::same(theme.border_radius_lg as u8))
                    .stroke(Stroke::new(1.0, theme.border()))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let hint = if state.chat_active_channel.starts_with("dm:") {
                        let pk = &state.chat_active_channel[3..];
                        let name = state.chat_dms.iter().find(|d| d.user_key == pk)
                            .map(|d| d.user_name.as_str()).unwrap_or("user");
                        format!("Message {}", name)
                    } else if state.chat_active_channel.starts_with("group:") {
                        let gid = &state.chat_active_channel[6..];
                        let name = state.chat_groups.iter().find(|g| g.id == gid)
                            .map(|g| g.name.as_str()).unwrap_or("group");
                        format!("Message {}", name)
                    } else {
                        format!("Message #{}", state.chat_active_channel)
                    };
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut state.chat_input)
                            .desired_width(ui.available_width() - 104.0)
                            .hint_text(hint),
                    );

                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    // Help button (?) - opens slash commands reference
                    if ui.add(
                        egui::Button::new(
                            RichText::new("?")
                                .size(theme.font_size_body)
                                .color(theme.text_muted()),
                        )
                        .fill(Color32::from_rgb(40, 40, 48))
                        .min_size(Vec2::new(28.0, 28.0)),
                    ).clicked() {
                        state.show_help_modal = !state.show_help_modal;
                    }

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

                                let json_str = if channel.starts_with("dm:") {
                                    // DM: send as type "dm" with target partner key.
                                    // Attempt E2E encryption if we know the peer's ECDH public key.
                                    let partner_key = &channel[3..];
                                    let mut dm_obj = serde_json::json!({
                                        "type": "dm",
                                        "from": state.profile_public_key,
                                        "from_name": display_name,
                                        "to": partner_key,
                                        "content": content,
                                        "timestamp": ts,
                                    });
                                    if !state.ecdh_private_hex.is_empty() {
                                        if let Some(peer_ecdh) = state.peer_ecdh_keys.get(partner_key) {
                                            if let Ok(sb) = hex::decode(&state.ecdh_private_hex) {
                                                if sb.len() == 32 {
                                                    let mut bytes = [0u8; 32];
                                                    bytes.copy_from_slice(&sb);
                                                    if let Ok(kp) = crate::net::dm_crypto::DmKeypair::from_secret_bytes(&bytes) {
                                                        match crate::net::dm_crypto::encrypt_dm(&kp, peer_ecdh, &content) {
                                                            Ok(enc) => {
                                                                dm_obj["content"] = serde_json::Value::String(enc.content_b64);
                                                                dm_obj["nonce"] = serde_json::Value::String(enc.nonce_b64);
                                                                dm_obj["encrypted"] = serde_json::Value::Bool(true);
                                                            }
                                                            Err(e) => {
                                                                log::warn!("DM encryption failed for {}: {} (sending plaintext)", partner_key, e);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            log::debug!("No ECDH key for {}, sending plaintext", partner_key);
                                        }
                                    }
                                    dm_obj.to_string()
                                } else if channel.starts_with("group:") {
                                    // Group: send as type "group_msg"
                                    let group_id = &channel[6..];
                                    serde_json::json!({
                                        "type": "group_msg",
                                        "group_id": group_id,
                                        "content": content,
                                    }).to_string()
                                } else {
                                    // Normal channel chat
                                    serde_json::json!({
                                        "type": "chat",
                                        "from": state.profile_public_key,
                                        "from_name": display_name,
                                        "content": content,
                                        "timestamp": ts,
                                        "channel": channel,
                                    }).to_string()
                                };
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

                        while state.chat_messages.len() > 200 {
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
    });
}

// ─────────────────────────────── User Profile Modal ────────────────────────

fn draw_user_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if !state.chat_user_modal_open { return; }

    let mut close_modal = false;

    egui::Window::new("User Profile")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(320.0, 0.0))
        .title_bar(false)
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(20.0).rounding(Rounding::same(8)).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            let name = state.chat_user_modal_name.clone();
            let key = state.chat_user_modal_key.clone();

            // Title bar with close X
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("User Profile")
                        .size(theme.font_size_body)
                        .color(theme.text_muted()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        RichText::new("X").size(theme.font_size_body).color(theme.text_secondary()),
                    ).fill(Color32::TRANSPARENT).frame(false)).clicked() {
                        close_modal = true;
                    }
                });
            });
            ui.add_space(8.0);

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

            // Online/offline status - user is online if they appear in chat_users
            // (peer_list only contains connected users; full_user_list has an "online" field)
            let user_entry = state.chat_users.iter().find(|u| u.public_key == key);
            let user_status = user_entry
                .map(|u| u.status.clone())
                .unwrap_or_else(|| "offline".to_string());
            // If status is empty or unrecognized, and user exists in the list, default to online
            let effective_status = if user_status.is_empty() { "online".to_string() } else { user_status };
            ui.vertical_centered(|ui| {
                let (dot_color, status_text) = match effective_status.as_str() {
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

            // Check if target user is streaming
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
                        // Open DM with this user
                        let dm_channel = format!("dm:{}", key);
                        // Add to DMs list if not already there
                        if !state.chat_dms.iter().any(|d| d.user_key == key) {
                            state.chat_dms.push(crate::gui::ChatDm {
                                user_name: name.clone(),
                                user_key: key.clone(),
                                last_message: String::new(),
                                timestamp: String::new(),
                                unread: false,
                            });
                        }
                        state.chat_active_channel = dm_channel;
                        state.chat_messages.clear();
                        state.history_fetched = false;
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {
                                let msg = serde_json::json!({
                                    "type": "dm_open",
                                    "partner": key,
                                });
                                client.send(&msg.to_string());
                            }
                        }
                        close_modal = true;
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
                    close_modal = true;
                }
            });
        });
    if close_modal {
        state.chat_user_modal_open = false;
    }
}

// ─────────────────────────────── Create Channel Modal ──────────────────────

fn draw_create_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Create Channel")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
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
}

// ─────────────────────────────── Edit Channel Modal ──────────────────────

fn draw_edit_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Edit Channel")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
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
                        // Send delete via slash command (server matches by channel ID)
                        let ch_id = state.edit_channel_id.clone();
                        let ch_name = state.edit_channel_name.clone();
                        send_slash_command(state, &format!("/channel-delete {}", ch_id));
                        // If name differs from ID, also try deleting by name
                        if ch_name.to_lowercase() != ch_id.to_lowercase() {
                            send_slash_command(state, &format!("/channel-delete {}", ch_name));
                        }
                        log::info!("Channel delete: id={}, name={}", ch_id, ch_name);
                        // Switch to general if we just deleted the active channel
                        if state.chat_active_channel == ch_name {
                            state.chat_active_channel = "general".to_string();
                        }
                        state.show_channel_edit_modal = false;
                        state.edit_channel_confirm_delete = false;
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
}

// ─────────────────────────────── Create Group Modal ─────────────────────

fn draw_create_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Create Group")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
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
}

// ─────────────────────────────── Join Group Modal ──────────────────────

fn draw_join_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Join Group")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
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
}

// ─────────────────────────────── UI Helpers ──────────────────────────────

/// Draw a lock/unlock toggle button at the top of a panel.
/// Positioned at the left edge, next to the panel border.
/// Returns true if the button was clicked (toggle lock state).
fn draw_panel_lock_button(ui: &mut egui::Ui, _theme: &Theme, locked: bool) -> bool {
    let tooltip = if locked { "Unlock panel width" } else { "Lock panel width" };
    let color = if locked { Color32::from_rgb(200, 180, 100) } else { Color32::from_rgb(100, 100, 100) };
    let response = ui.horizontal(|ui| {
        ui.add_space(2.0);
        let (rect, resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
        if locked {
            crate::gui::widgets::icons::paint_lock(ui.painter(), rect, color);
        } else {
            crate::gui::widgets::icons::paint_unlock(ui.painter(), rect, color);
        }
        resp.on_hover_text(tooltip).clicked()
    }).inner;
    response
}

/// Draw a collapsible section header with a tinted background.
/// Returns true if the header was clicked (toggle).
fn tinted_section_header(ui: &mut egui::Ui, label: &str, collapsed: bool, bg: Color32) -> bool {
    tinted_section_header_with_buttons(ui, label, collapsed, bg, |_| {})
}

/// Draw a tinted section header with optional right-aligned buttons.
/// The `add_buttons` closure receives the UI in right-to-left layout for adding icon buttons.
/// Returns true if the collapse arrow area was clicked (toggle collapse).
fn tinted_section_header_with_buttons(
    ui: &mut egui::Ui,
    label: &str,
    collapsed: bool,
    bg: Color32,
    add_buttons: impl FnOnce(&mut egui::Ui),
) -> bool {
    let header_height = 28.0;
    let full_width = ui.available_width();

    // Allocate the full header rect for background painting
    let (full_rect, _) = ui.allocate_exact_size(
        Vec2::new(full_width, header_height),
        egui::Sense::hover(),
    );

    // Paint background
    let header_bg = Color32::from_rgba_premultiplied(
        bg.r().saturating_add(15),
        bg.g().saturating_add(15),
        bg.b().saturating_add(15),
        bg.a(),
    );
    ui.painter().rect_filled(full_rect, 0.0, header_bg);

    // Place the collapse arrow as a separate click target
    let arrow_size = 12.0;
    let arrow_left = full_rect.left() + 8.0;
    let cy = full_rect.center().y;
    let arrow_click_rect = egui::Rect::from_min_size(
        egui::pos2(full_rect.left(), full_rect.top()),
        Vec2::new(arrow_size + 16.0, header_height), // wider click target for the collapse area
    );
    let arrow_resp = ui.interact(arrow_click_rect, ui.id().with(label).with("arrow"), egui::Sense::click());

    // Draw the arrow
    let arrow_rect = egui::Rect::from_min_size(
        egui::pos2(arrow_left, cy - arrow_size / 2.0),
        Vec2::splat(arrow_size),
    );
    let arrow_color = Color32::from_rgb(180, 180, 180);
    if collapsed {
        crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_rect, arrow_color);
    } else {
        crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_rect, arrow_color);
    }

    // Draw the label
    let label_x = arrow_left + arrow_size + 4.0;
    ui.painter().text(
        egui::pos2(label_x, cy),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(200, 200, 200),
    );

    // Draw right-aligned buttons using a child UI positioned at the right side
    let btn_rect = egui::Rect::from_min_max(
        egui::pos2(full_rect.right() - 80.0, full_rect.top()),
        full_rect.max,
    );
    let mut btn_ui = ui.new_child(egui::UiBuilder::new().max_rect(btn_rect).layout(Layout::right_to_left(Align::Center)));
    btn_ui.add_space(4.0);
    add_buttons(&mut btn_ui);

    if arrow_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    arrow_resp.clicked()
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

                // Painted collapse/expand triangle
                let arrow_color = Color32::from_rgb(180, 180, 180);
                let (arrow_rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                if collapsed {
                    crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_rect, arrow_color);
                } else {
                    crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_rect, arrow_color);
                }
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

/// Send a slash command as a chat message (server handles moderation via slash commands).
fn send_slash_command(state: &mut GuiState, command: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let msg = serde_json::json!({
                "type": "chat",
                "from": state.profile_public_key,
                "from_name": state.user_name,
                "content": command,
                "timestamp": ts,
                "channel": state.chat_active_channel,
            });
            client.send(&msg.to_string());
        }
    }
}

// ─────────────────────────────── Help Modal ──────────────────────────────

fn draw_help_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Slash Commands")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_size(Vec2::new(460.0, 500.0))
        .frame(Frame::NONE.fill(Color32::from_rgb(26, 26, 32)).inner_margin(20.0).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            // Close button
            ui.horizontal(|ui| {
                ui.label(RichText::new("Slash Commands Reference")
                    .size(theme.font_size_heading).color(theme.text_primary()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        RichText::new("X").size(theme.font_size_body).color(theme.text_muted()),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        state.show_help_modal = false;
                    }
                });
            });
            ui.add_space(8.0);

            ScrollArea::vertical()
                .id_salt("help_modal_scroll")
                .auto_shrink([false, false])
                .max_height(440.0)
                .show(ui, |ui| {
                    let section_color = Color32::from_rgb(100, 180, 255);
                    let cmd_color = theme.text_primary();
                    let desc_color = theme.text_muted();

                    // General
                    ui.label(RichText::new("General").size(theme.font_size_body + 2.0).color(section_color).strong());
                    ui.add_space(4.0);
                    let general_cmds = [
                        ("/help", "Show available commands"),
                        ("/link", "Generate a code to link another device"),
                        ("/revoke <key_prefix>", "Remove a stolen/lost device"),
                        ("/users", "List all registered users"),
                        ("/report <name> [reason]", "Report a user"),
                        ("/dm <name> <message>", "Send a direct message"),
                        ("/dms", "List your DM conversations"),
                        ("/edit <text>", "Edit your last message"),
                        ("/pins", "List pinned messages"),
                        ("/friend-code", "Generate a shareable friend code"),
                        ("/redeem <code>", "Redeem a friend code"),
                        ("/server-list", "List federated servers"),
                    ];
                    for (cmd, desc) in &general_cmds {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(*cmd).size(theme.font_size_body).color(cmd_color).monospace());
                            ui.label(RichText::new(*desc).size(theme.font_size_small).color(desc_color));
                        });
                        ui.add_space(2.0);
                    }

                    ui.add_space(10.0);
                    ui.label(RichText::new("Moderator").size(theme.font_size_body + 2.0).color(Color32::from_rgb(255, 180, 80)).strong());
                    ui.add_space(4.0);
                    let mod_cmds = [
                        ("/kick <name>", "Disconnect a user"),
                        ("/mute <name>", "Mute a user"),
                        ("/unmute <name>", "Unmute a user"),
                        ("/pin", "Pin the last message in the channel"),
                        ("/unpin <N>", "Unpin a message by index"),
                        ("/invite", "Generate an invite code (lockdown bypass)"),
                    ];
                    for (cmd, desc) in &mod_cmds {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(*cmd).size(theme.font_size_body).color(cmd_color).monospace());
                            ui.label(RichText::new(*desc).size(theme.font_size_small).color(desc_color));
                        });
                        ui.add_space(2.0);
                    }

                    ui.add_space(10.0);
                    ui.label(RichText::new("Admin").size(theme.font_size_body + 2.0).color(Color32::from_rgb(255, 100, 100)).strong());
                    ui.add_space(4.0);
                    let admin_cmds = [
                        ("/ban <name>", "Ban a user"),
                        ("/unban <name>", "Unban a user"),
                        ("/mod <name>", "Make a user a moderator"),
                        ("/unmod <name>", "Remove moderator role"),
                        ("/verify <name>", "Mark a user as verified"),
                        ("/donor <name>", "Mark a user as a donor"),
                        ("/unverify <name>", "Remove verified status"),
                        ("/lockdown", "Toggle registration lockdown"),
                        ("/wipe", "Clear current channel's history"),
                        ("/wipe-all", "Clear ALL channels' history"),
                        ("/gc", "Garbage collect inactive names (90 days)"),
                        ("/channel-create <name> [--readonly] [desc]", "Create a channel"),
                        ("/channel-delete <name>", "Delete a channel"),
                        ("/channel-readonly <name>", "Toggle read-only"),
                        ("/channel-reorder <name> <pos>", "Set channel sort order"),
                        ("/name-release <name>", "Release a name (account recovery)"),
                        ("/reports", "View recent reports"),
                        ("/reports-clear", "Clear all reports"),
                    ];
                    for (cmd, desc) in &admin_cmds {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(*cmd).size(theme.font_size_body).color(cmd_color).monospace());
                            ui.label(RichText::new(*desc).size(theme.font_size_small).color(desc_color));
                        });
                        ui.add_space(2.0);
                    }

                    ui.add_space(10.0);
                    ui.label(RichText::new("Federation").size(theme.font_size_body + 2.0).color(Color32::from_rgb(100, 220, 160)).strong());
                    ui.add_space(4.0);
                    let fed_cmds = [
                        ("/server-add <url> [name]", "Add a federated server"),
                        ("/server-remove <id>", "Remove a federated server"),
                        ("/server-trust <id> <0-3>", "Set trust tier"),
                        ("/server-federate <channel>", "Toggle federation for a channel"),
                        ("/server-connect", "Connect to all verified servers"),
                    ];
                    for (cmd, desc) in &fed_cmds {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(*cmd).size(theme.font_size_body).color(cmd_color).monospace());
                            ui.label(RichText::new(*desc).size(theme.font_size_small).color(desc_color));
                        });
                        ui.add_space(2.0);
                    }

                    ui.add_space(10.0);
                    ui.label(RichText::new("Formatting Tips").size(theme.font_size_body + 2.0).color(section_color).strong());
                    ui.add_space(4.0);
                    ui.label(RichText::new("**bold**, *italic*, `code`, ~~strike~~").size(theme.font_size_body).color(desc_color));
                    ui.label(RichText::new("Click the reply arrow on any message to reply").size(theme.font_size_body).color(desc_color));
                });
        });
}

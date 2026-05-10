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
    // Panel lock buttons MOVED to center panel header in v0.188.0.
    // The sidebar panels just render their content; the lock toggle is a
    // small button on the chat header alongside the channel name. This
    // keeps the sidebars clean and puts the controls where the user is
    // most likely already looking.
    let left_response = left_panel.show(ctx, |ui| {
        draw_left_panel(ui, theme, state);
    });
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
    let right_response = right_panel.show(ctx, |ui| {
        draw_right_panel(ui, theme, state);
    });
    if !state.chat_right_panel_locked {
        state.chat_right_panel_width = right_response.response.rect.width();
    }

    // ── CENTER PANEL ──
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_center_panel(ui, theme, state);
        });

    // ── FLOATING LOCK OVERLAYS (v0.190.0) ──
    // Pinned to the EXACT top corners of the center panel with zero
    // inner padding. The button is 14×14 and sits flush against the
    // panel boundary — left button at the left panel's right edge,
    // right button such that its right edge meets the right panel's
    // left edge. The Area gets `Frame::NONE` (no inner margin) and
    // the button helper itself avoids any horizontal wrapper / spacing,
    // so what you see on screen is exactly 14 pixels of icon and
    // nothing else.
    const LOCK_PX: f32 = 14.0;
    let left_panel_right = left_response.response.rect.right();
    let right_panel_left = right_response.response.rect.left();
    let header_top = left_response.response.rect.top();

    egui::Area::new(egui::Id::new("chat_left_lock_overlay"))
        .fixed_pos(egui::pos2(left_panel_right, header_top))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            // Strip the parent style's item_spacing inside this Area
            // so even nested allocations don't introduce stray gaps.
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if draw_panel_lock_button(ui, theme, state.chat_left_panel_locked) {
                state.chat_left_panel_locked = !state.chat_left_panel_locked;
                crate::config::AppConfig::from_gui_state(state).save();
            }
        });
    egui::Area::new(egui::Id::new("chat_right_lock_overlay"))
        .fixed_pos(egui::pos2(right_panel_left - LOCK_PX, header_top))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if draw_panel_lock_button(ui, theme, state.chat_right_panel_locked) {
                state.chat_right_panel_locked = !state.chat_right_panel_locked;
                crate::config::AppConfig::from_gui_state(state).save();
            }
        });

    // ── USER PROFILE MODAL ──
    if state.chat_user_modal_open {
        draw_user_modal(ctx, theme, state);
    }

    // ── UNENCRYPTED-DM CONFIRMATION MODAL (v0.199.0, B3 fix) ──
    // Pops up when the user clicked Send on a DM that we couldn't
    // encrypt (recipient ECDH key missing, etc.). User must explicitly
    // confirm "Send unencrypted" or cancel — no silent plaintext fallback.
    draw_unencrypted_dm_modal(ctx, theme, state);

    // ── CREATE CHANNEL MODAL ──
    if state.show_create_channel_modal {
        draw_create_channel_modal(ctx, theme, state);
    }

    // ── ADD SERVER MODAL (v0.187.0) ──
    if state.show_add_server_modal {
        draw_add_server_modal(ctx, theme, state);
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

    // ── SEARCH MODAL ──
    if state.chat_search_open {
        draw_search_modal(ctx, theme, state);
    }

    // ── PINS MODAL ──
    if state.chat_pins_open {
        draw_pins_modal(ctx, theme, state);
    }
}

fn draw_pins_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.chat_pins_open;
    let channel = state.chat_active_channel.clone();
    let title = format!("Pinned in #{}", channel);
    widgets::dialog(ctx, theme, "chat_pins_dialog", &title, &mut open, |ui| {
        ui.set_min_width(520.0);

        // Snapshot the pin list to avoid borrow conflict if rendering triggers
        // state mutations (e.g. clicking a pin to jump to the message).
        let pins = state.chat_pins.get(&channel).cloned().unwrap_or_default();
        if pins.is_empty() {
            ui.label(
                RichText::new("No pinned messages in this channel.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            return;
        }

        ui.label(
            RichText::new(format!("{} pin(s) — pin/unpin via the 📌 button on each message.", pins.len()))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);

        ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
            for p in &pins {
                widgets::card(ui, theme, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&p.from_name)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format_timestamp(p.original_timestamp))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("pinned by {}", p.pinned_by))
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                        });
                    });
                    ui.label(
                        RichText::new(&p.content)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                });
                ui.add_space(theme.spacing_xs);
            }
        });
    });
    if !open {
        state.chat_pins_open = false;
    }
}

fn draw_search_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.chat_search_open;
    widgets::dialog(ctx, theme, "chat_search_dialog", "Search messages", &mut open, |ui| {
        ui.set_min_width(520.0);
        widgets::form_row(ui, theme, "Query", |ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut state.chat_search_query)
                    .desired_width(360.0)
                    .hint_text("e.g. blueprint   (min 2 chars)"),
            );
            ui.add_space(theme.spacing_sm);
            let can_search = state.chat_search_query.trim().len() >= 2;
            ui.add_enabled_ui(can_search, |ui| {
                if widgets::Button::primary("Search").show(ui, theme)
                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "search",
                                "query": state.chat_search_query.trim(),
                                "limit": 50,
                            });
                            client.send(&msg.to_string());
                            state.chat_search_results.clear();
                        }
                    }
                }
            });
        });

        ui.add_space(theme.spacing_sm);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        if state.chat_search_results.is_empty() {
            ui.label(
                RichText::new("No results yet — type a query and hit Search.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        } else {
            ui.label(
                RichText::new(format!("{} result(s)", state.chat_search_results.len()))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);

            // Snapshot to avoid borrow conflict when clicking jumps the channel.
            let results = state.chat_search_results.clone();
            let mut jump_to: Option<String> = None;
            ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for r in &results {
                    widgets::card(ui, theme, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&r.sender_name)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary())
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!("in #{}", r.channel))
                                    .size(theme.font_size_small)
                                    .color(theme.accent()),
                            );
                            ui.label(
                                RichText::new(format_timestamp(r.timestamp_ms))
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::Button::secondary("Jump").show(ui, theme) {
                                    jump_to = Some(r.channel.clone());
                                }
                            });
                        });
                        ui.label(
                            RichText::new(&r.content)
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    });
                    ui.add_space(theme.spacing_xs);
                }
            });

            if let Some(ch) = jump_to {
                state.chat_active_channel = ch;
                state.chat_search_open = false;
            }
        }
    });
    if !open {
        state.chat_search_open = false;
        state.chat_search_results.clear();
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

                if widgets::Button::primary("Connect").full_width().show(ui, theme) {
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

    if resp.clicked() && state.chat_active_channel != "scratchpad" {
        // Only clear+refetch when actually switching contexts. Re-clicking the
        // active row is a no-op (BUG-035 — used to nuke local-echoed unsent text).
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

    // Capture the cog's rect from inside the closure so the floating
    // popup below knows where to anchor. Cell because Rect: Copy and we
    // need interior mutability across the closure boundary.
    let cog_rect_cell: std::cell::Cell<Option<egui::Rect>> = std::cell::Cell::new(None);
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
            cog_rect_cell.set(Some(cog_rect));
            if cog_resp.on_hover_text("DM Settings").clicked() {
                dm_cog_clicked = true;
            }
        },
    ) {
        state.chat_dm_collapsed = !state.chat_dm_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // Toggle popup open state on cog click.
    if dm_cog_clicked {
        state.dm_settings_popup_open = !state.dm_settings_popup_open;
    }
    // Render the popup as a manual floating Area anchored to the cog.
    // Bypassing egui::popup_below_widget because that machinery uses
    // CloseOnClick which fires on the SAME FRAME as the trigger click,
    // making the popup flicker on for one frame then disappear
    // (operator bug 2026-05-08). Manual Area + close-outside check
    // that explicitly excludes the trigger click frame is reliable.
    if state.dm_settings_popup_open {
        if let Some(cog_rect) = cog_rect_cell.get() {
            let popup_resp = egui::Area::new(egui::Id::new("dm_settings_popup"))
                .fixed_pos(egui::pos2(cog_rect.left() - 100.0, cog_rect.bottom() + 4.0))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            ui.set_min_width(140.0);
                            ui.label(RichText::new("DM Settings").size(theme.font_size_body).color(theme.text_primary()).strong());
                            ui.separator();
                            if ui.button("Clear All DMs").clicked() {
                                state.chat_dms.clear();
                                state.dm_settings_popup_open = false;
                            }
                            if ui.button("DM Notifications").clicked() {
                                // TODO: toggle DM notifications
                                state.dm_settings_popup_open = false;
                            }
                        });
                });
            // Close-on-click-outside — but ignore the trigger-click frame
            // so opening doesn't immediately close. Click is "outside" if
            // it lands neither in the popup's rect nor on the cog.
            if !dm_cog_clicked {
                let click_outside = ui.ctx().input(|i| {
                    i.pointer.any_click() && i.pointer.interact_pos().map_or(false, |pos| {
                        !popup_resp.response.rect.contains(pos) && !cog_rect.contains(pos)
                    })
                });
                if click_outside {
                    state.dm_settings_popup_open = false;
                }
            }
        }
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

                    if response.clicked() && state.chat_active_channel != dm_channel {
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
    // Capture the cog's rect from inside the closure for the floating
    // popup anchor — same pattern as the DM section.
    let groups_cog_rect_cell: std::cell::Cell<Option<egui::Rect>> = std::cell::Cell::new(None);

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
                groups_cog_rect_cell.set(Some(cog_rect));
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
    // Groups settings popup — manual Area, same pattern as DM cog.
    // Fixes the same flicker bug where popup_below_widget + CloseOnClick
    // self-closed on the trigger click frame.
    if groups_cog_clicked {
        state.groups_settings_popup_open = !state.groups_settings_popup_open;
    }
    if state.groups_settings_popup_open {
        if let Some(cog_rect) = groups_cog_rect_cell.get() {
            let popup_resp = egui::Area::new(egui::Id::new("groups_settings_popup"))
                .fixed_pos(egui::pos2(cog_rect.left() - 100.0, cog_rect.bottom() + 4.0))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            ui.set_min_width(140.0);
                            ui.label(RichText::new("Groups Settings").size(theme.font_size_body).color(theme.text_primary()).strong());
                            ui.separator();
                            if ui.button("Group Notifications").clicked() {
                                // TODO: toggle group notifications
                                state.groups_settings_popup_open = false;
                            }
                            if ui.button("Sort by Activity").clicked() {
                                // TODO: sort groups
                                state.groups_settings_popup_open = false;
                            }
                        });
                });
            if !groups_cog_clicked {
                let click_outside = ui.ctx().input(|i| {
                    i.pointer.any_click() && i.pointer.interact_pos().map_or(false, |pos| {
                        !popup_resp.response.rect.contains(pos) && !cog_rect.contains(pos)
                    })
                });
                if click_outside {
                    state.groups_settings_popup_open = false;
                }
            }
        }
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

                                // (Per-channel cog removed in v0.187 — group
                                // channel admin lives in the group settings
                                // cog next to the group name.)

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
                                } else if state.chat_active_channel != ch.id {
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
                // push_nav_to so Esc on ServerSettings returns to Chat
                // instead of jumping to FPS mode.
                if open_server_settings {
                    state.push_nav_to(crate::gui::GuiPage::ServerSettings);
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
        state.show_add_server_modal = true;
        state.add_server_url_draft.clear();
        state.add_server_name_draft.clear();
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
                                // push_nav_to so Esc returns to Chat.
                                state.push_nav_to(crate::gui::GuiPage::ServerSettings);
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

                            // (Per-channel settings cog removed in v0.187 —
                            // channel admin lives in the Server Settings
                            // modal next to the server name. Click that
                            // single cog to manage every channel + role +
                            // member in one place.)

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
                            } else if state.chat_active_channel != ch.id {
                                // Only swap channel context when the click actually changes
                                // channels. Re-clicking the active channel used to clear
                                // chat_messages and re-fetch history, which nuked any
                                // local-echoed unsent reply (BUG-035). Now it's a no-op.
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

                    // (+ Create Channel button removed in v0.187 — channel
                    // creation now lives inside the Server Settings cog so
                    // the sidebar stays clean. Click the cog next to the
                    // server name to manage all channels in one place.)

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
    // Lock buttons moved OUT of the header in v0.189.x — operator wanted
    // them tucked into the actual panel CORNERS, not next to the channel
    // title where they could be mistaken for a UI label. They now paint
    // as floating Areas anchored to the side-panel boundaries (see the
    // overlays at the bottom of `chat::draw`).
    Frame::NONE
        .fill(Color32::from_rgb(25, 25, 30))
        .inner_margin(egui::Margin::symmetric(16, 10))
        .stroke(Stroke::new(1.0, Color32::from_rgb(40, 40, 48)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Force the horizontal layout (and therefore the Frame
                // around it) to fill the full available panel width.
                // Without this, ui.horizontal shrinks to its content's
                // bounding rect, so the dark-gray header background only
                // covered the left portion next to the channel name and
                // left a black void on the right when the description
                // was short. The header should always read as one
                // continuous bar across the panel.
                ui.set_min_width(ui.available_width());
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
    // Input height grows by 22 px when a reply context is active so the
    // "Replying to … [X]" banner has its own row above the text field.
    let reply_banner_h: f32 = if state.chat_reply_to.is_some() { 22.0 } else { 0.0 };
    let input_height = 52.0 + reply_banner_h;

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

                // Reactions clicked during render — applied after the loop ends
                // so we don't try to send WS messages while iterating &state.
                let mut pending_reactions: Vec<(String, u64, String)> = Vec::new();
                // Reply button clicks: defer setting state.chat_reply_to until after the loop.
                let mut pending_reply: Option<crate::gui::ReplyContext> = None;
                // Pin button clicks — defer pin_request WS sends.
                let mut pending_pins: Vec<(String, String, String, u64)> = Vec::new();
                // Edit button clicks — defer setting edit target.
                let mut pending_edit: Option<(u64, String)> = None;
                // Edit-save submissions from the inline editor.
                let mut pending_edit_save: Option<(u64, String)> = None;
                // Cancel flag — Cancel button in the inline editor sets this
                // so we clear chat_edit_target after the loop ends.
                let mut pending_edit_cancel = false;
                // Report button clicks (from context menu) — buffered to send /report slash command.
                let mut pending_reports: Vec<(String, String)> = Vec::new();

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

                    // ── Reply-to context (if this is a reply) ──
                    if let Some(ref reply) = msg.reply_to {
                        let row_w = ui.available_width();
                        let (row_rect, _) = ui.allocate_exact_size(
                            Vec2::new(row_w, 18.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(row_rect, 0.0, row_bg);
                        let preview = if reply.preview.len() > 60 {
                            format!("{}…", &reply.preview[..60])
                        } else {
                            reply.preview.clone()
                        };
                        ui.painter().text(
                            egui::pos2(row_rect.left() + 40.0, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &format!("↩ {}: {}", reply.sender_name, preview),
                            egui::FontId::proportional(theme.font_size_small),
                            theme.text_muted(),
                        );
                    }

                    // Inline editor for this message (only on user's own messages
                    // matching chat_edit_target). Otherwise render the regular row.
                    let is_editing = state.chat_edit_target
                        .as_ref()
                        .map(|(ts, _)| *ts == msg.timestamp_ms && msg.sender_key == state.profile_public_key)
                        .unwrap_or(false);

                    let need_text_row = show_header || !display_text.trim().is_empty();
                    // Track row hover + row rect + pill rect so the expanded
                    // pill popup (rendered later) can anchor to the right
                    // place. The popup is an egui::Area, doesn't allocate
                    // layout space — hovering a message no longer shifts
                    // every message below.
                    let mut row_was_hovered = false;
                    let mut row_rect_opt: Option<egui::Rect> = None;
                    let mut pill_rect_for_msg: egui::Rect = egui::Rect::NOTHING;
                    if is_editing {
                        // Render an editable row in place of the message text.
                        if let Some((_, ref mut draft)) = state.chat_edit_target {
                            ui.horizontal(|ui| {
                                ui.add_space(40.0);
                                let resp = ui.add(
                                    egui::TextEdit::multiline(draft)
                                        .desired_width(ui.available_width() - 130.0)
                                        .desired_rows(2),
                                );
                                ui.add_space(theme.spacing_xs);
                                if widgets::Button::primary("Save").show(ui, theme)
                                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                {
                                    pending_edit_save = Some((msg.timestamp_ms, draft.trim().to_string()));
                                }
                                if widgets::Button::secondary("Cancel").show(ui, theme) {
                                    pending_edit_cancel = true;
                                }
                            });
                        }
                    } else if need_text_row {
                        // Measure exact pill width — must match what
                        // paint_timestamp_pill draws or the reserved space
                        // in message_row will be wrong and content text
                        // overlaps the pill (operator-reported bug).
                        let pill_width = compute_pill_width(ui.ctx(), theme, &msg.timestamp, &msg.reactions);
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
                            pill_width,
                        );
                        row_was_hovered = row_resp.response.hovered();
                        row_rect_opt = Some(row_resp.response.rect);
                        pill_rect_for_msg = row_resp.pill_rect;

                        // Paint the timestamp pill into the rect message_row reserved.
                        if pill_rect_for_msg != egui::Rect::NOTHING {
                            paint_timestamp_pill(
                                ui,
                                theme,
                                pill_rect_for_msg,
                                &msg.timestamp,
                                &msg.reactions,
                                &state.profile_public_key,
                                msg.timestamp_ms,
                                msg.sender_key.clone(),
                                &mut pending_reactions,
                            );
                        }
                        if row_resp.userbox_clicked(ui.ctx()) {
                            state.chat_user_modal_open = true;
                            state.chat_user_modal_name = msg.sender_name.clone();
                            state.chat_user_modal_key = msg.sender_key.clone();
                        }

                        // Right-click context menu — same actions as the inline
                        // pill buttons but quicker to reach.
                        let is_own = msg.sender_key == state.profile_public_key;
                        row_resp.response.context_menu(|ui| {
                            ui.set_min_width(160.0);
                            // Plain text labels — leading emoji glyphs were
                            // unreliable across the loaded font (📋 📌 ✎ all
                            // rendered as tofu in some sessions). The context
                            // menu prioritizes legibility over ornament.
                            if ui.button("Copy text").clicked() {
                                ui.ctx().copy_text(msg.content.clone());
                                ui.close_menu();
                            }
                            // ↩ U+21A9 is in the Arrows block which IS in the
                            // loaded font — safe to keep.
                            if msg.timestamp_ms > 0 && ui.button("↩ Quote / reply").clicked() {
                                let preview = if msg.content.len() > 80 {
                                    format!("{}…", &msg.content[..80])
                                } else {
                                    msg.content.clone()
                                };
                                pending_reply = Some(crate::gui::ReplyContext {
                                    sender_key: msg.sender_key.clone(),
                                    sender_name: msg.sender_name.clone(),
                                    preview,
                                    timestamp_ms: msg.timestamp_ms,
                                });
                                ui.close_menu();
                            }
                            if msg.timestamp_ms > 0 && ui.button("Pin message").clicked() {
                                pending_pins.push((
                                    msg.sender_key.clone(),
                                    msg.sender_name.clone(),
                                    msg.content.clone(),
                                    msg.timestamp_ms,
                                ));
                                ui.close_menu();
                            }
                            if is_own && msg.timestamp_ms > 0 && ui.button("Edit").clicked() {
                                pending_edit = Some((msg.timestamp_ms, msg.content.clone()));
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Report").clicked() {
                                pending_reports.push((msg.sender_name.clone(), msg.content.clone()));
                                ui.close_menu();
                            }
                        });
                    }

                    // (Old inline reaction PILLS row removed — reactions
                    // now live INSIDE the timestamp pill itself. See
                    // paint_timestamp_pill() above for the inline display.)
                    let target_ts = msg.timestamp_ms;
                    let target_from = msg.sender_key.clone();

                    // ── Expanded pill popup (right of Þ, sticky hover) ──
                    // Opens when the cursor is over the pill OR over the popup
                    // itself (sticky combined region), so moving from pill to
                    // popup doesn't dismiss it. Extends RIGHTWARD from the
                    // pill so it reads as the pill "growing" with new
                    // controls — functions on the LEFT (separated from the
                    // existing Þ that's already in the inline pill), reactions
                    // on the RIGHT, ∞ at the far right.
                    if pill_rect_for_msg != egui::Rect::NOTHING && target_ts > 0 {
                        // Reactions list. As of v0.190.0 we install the OS's
                        // emoji font (Windows seguiemj.ttf, macOS Apple Color
                        // Emoji, Linux Noto Color Emoji) as an egui font
                        // fallback at startup (see src/gui/fonts.rs). That
                        // covers basically every emoji in BMP + supplementary
                        // plane, so we can use a real reaction palette here.
                        // The icon_glyph_lint test still catches U+FE0F
                        // variation selectors and known-broken glyphs.
                        const TOP_REACTIONS: &[&str] = &[
                            "❤", "👍", "👎", "😂", "🤣", "😢", "😡", "🔥", "💯", "⭐",
                        ];
                        const ALL_REACTIONS: &[&str] = &[
                            // Hearts (colored) — system font supplies these.
                            "❤", "🧡", "💛", "💚", "💙", "💜", "🤍", "🖤", "🤎",
                            // Faces — laughs, cries, surprises, anger, love.
                            "😂", "🤣", "😢", "😭", "😡", "🤬", "😮", "😱", "🥰", "😍",
                            "🤔", "🙄", "😴", "🤯", "🥳", "😎",
                            // Hands & gestures.
                            "👍", "👎", "👏", "🙌", "🙏", "🤝", "✊", "💪",
                            // Symbols / objects.
                            "🔥", "💯", "⭐", "🎉", "✨", "💡", "🚀", "💀", "👀",
                            // Picker handle.
                            "∞",
                        ];
                        let is_own = msg.sender_key == state.profile_public_key;

                        // Estimated popup geometry — needed for the sticky
                        // hover region. Function buttons are now text labels
                        // (Pin / Edit) which are wider than the prior icon
                        // attempts, so widen the estimate accordingly.
                        let func_w = 26.0      // ↩ reply
                                   + 36.0      // Pin
                                   + (if is_own { 40.0 } else { 0.0 }); // Edit (own only)
                        let est_popup_w =
                            func_w
                            + 18.0                       // Þ separator
                            + TOP_REACTIONS.len() as f32 * 28.0 // top-10 reactions
                            + 30.0                       // ∞ button
                            + 16.0;                      // padding
                        // Popup rect adjacent to the pill (no gap so cursor
                        // sliding rightward stays in a connected hover region).
                        let est_popup_rect = egui::Rect::from_min_size(
                            egui::pos2(pill_rect_for_msg.right(), pill_rect_for_msg.top() - 2.0),
                            Vec2::new(est_popup_w, pill_rect_for_msg.height() + 4.0),
                        );
                        // Sticky hover gate: open if cursor is over THE PILL
                        // or over THE POPUP REGION specifically. Earlier code
                        // used pill.union(popup) which created a bounding
                        // rect that included the message TEXT area between
                        // them — so hovering message body opened the popup.
                        // (Operator-reported bug 2026-05-04.)
                        let pointer = ui.ctx().input(|i| i.pointer.hover_pos());
                        let pill_hovered = pointer.map(|p| pill_rect_for_msg.contains(p)).unwrap_or(false);
                        let popup_hovered = pointer.map(|p| est_popup_rect.contains(p)).unwrap_or(false);
                        let combined_hovered = pill_hovered || popup_hovered;

                        if combined_hovered {
                            let overlay_pos = egui::pos2(
                                pill_rect_for_msg.right(),
                                pill_rect_for_msg.center().y,
                            );
                            // Animated channeling color (RGB cycle / pulse / red on attack).
                            // Used for every glyph + the Þ separator so the popup feels
                            // alive and matches the active-page nav border.
                            let chan_time = ui.ctx().input(|i| i.time) as f32;
                            let chan_attack = state.attack_pulse_active;
                            let chan = crate::gui::pages::escape_menu::channeling_color(
                                theme, chan_time, chan_attack, theme.accent(),
                            );
                            ui.ctx().request_repaint();
                            egui::Area::new(egui::Id::new(("pill_expand", target_ts)))
                                .fixed_pos(overlay_pos)
                                .pivot(egui::Align2::LEFT_CENTER)
                                .order(egui::Order::Foreground)
                                .interactable(true)
                                .show(ui.ctx(), |ui| {
                                    Frame::none()
                                        .fill(theme.bg_card())
                                        .stroke(Stroke::new(1.0, theme.border()))
                                        .rounding(Rounding::same(8))
                                        .inner_margin(4.0)
                                        .shadow(egui::epaint::Shadow {
                                            offset: [1, 2],
                                            blur: 6,
                                            spread: 0,
                                            color: Color32::from_black_alpha(80),
                                        })
                                        .show(ui, |ui| {
                                            ui.spacing_mut().item_spacing.x = 2.0;
                                            ui.horizontal(|ui| {
                                                // Functions on the LEFT of Þ. Use plain
                                                // unicode arrows + ASCII letters so they
                                                // render reliably in the default font
                                                // (emoji glyphs like 📌 ✎ show as squares
                                                // without an emoji font installed).
                                                if ui.add(
                                                    egui::Button::new(RichText::new("↩").size(theme.font_size_body).color(chan))
                                                        .min_size(Vec2::new(26.0, 22.0))
                                                        .rounding(Rounding::same(4))
                                                ).on_hover_text("Reply").clicked() {
                                                    let preview = if msg.content.len() > 80 {
                                                        format!("{}…", &msg.content[..80])
                                                    } else { msg.content.clone() };
                                                    pending_reply = Some(crate::gui::ReplyContext {
                                                        sender_key: msg.sender_key.clone(),
                                                        sender_name: msg.sender_name.clone(),
                                                        preview,
                                                        timestamp_ms: target_ts,
                                                    });
                                                }
                                                if ui.add(
                                                    egui::Button::new(RichText::new("Pin").size(theme.font_size_small).color(chan))
                                                        .min_size(Vec2::new(34.0, 22.0))
                                                        .rounding(Rounding::same(4))
                                                ).on_hover_text("Pin message").clicked() {
                                                    pending_pins.push((msg.sender_key.clone(), msg.sender_name.clone(), msg.content.clone(), target_ts));
                                                }
                                                if is_own {
                                                    if ui.add(
                                                        egui::Button::new(RichText::new("Edit").size(theme.font_size_small).color(chan))
                                                            .min_size(Vec2::new(38.0, 22.0))
                                                            .rounding(Rounding::same(4))
                                                    ).on_hover_text("Edit message").clicked() {
                                                        pending_edit = Some((target_ts, msg.content.clone()));
                                                    }
                                                }

                                                // Þ separator (matches the inline pill).
                                                // Painted in the channeling color too so the
                                                // whole popup pulses together.
                                                ui.add_space(2.0);
                                                ui.label(RichText::new("Þ").size(theme.font_size_body).color(chan).strong());
                                                ui.add_space(2.0);

                                                // Top-10 reactions on the RIGHT of Þ. Strip
                                                // any U+FE0F variation selector — it ends up
                                                // as a tofu square next to ❤ in fonts that
                                                // don't honor the emoji presentation hint.
                                                for emoji in TOP_REACTIONS {
                                                    let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
                                                    if ui.add(
                                                        egui::Button::new(RichText::new(&clean).size(theme.font_size_body))
                                                            .min_size(Vec2::new(26.0, 22.0))
                                                            .rounding(Rounding::same(4))
                                                    ).clicked() {
                                                        pending_reactions.push((target_from.clone(), target_ts, clean.clone()));
                                                    }
                                                }

                                                // ∞ all-emoji picker (popup with full set).
                                                let inf_resp = ui.add(
                                                    egui::Button::new(RichText::new("∞").size(theme.font_size_body).color(chan))
                                                        .min_size(Vec2::new(26.0, 22.0))
                                                        .rounding(Rounding::same(4))
                                                ).on_hover_text("All reactions");
                                                let inf_popup_id = egui::Id::new(("react_inf_popup", target_ts));
                                                if inf_resp.clicked() {
                                                    ui.memory_mut(|m| m.toggle_popup(inf_popup_id));
                                                }
                                                egui::popup::popup_below_widget(
                                                    ui, inf_popup_id, &inf_resp,
                                                    egui::PopupCloseBehavior::CloseOnClickOutside,
                                                    |ui| {
                                                        ui.set_min_width(280.0);
                                                        ui.horizontal_wrapped(|ui| {
                                                            for emoji in ALL_REACTIONS {
                                                                let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
                                                                if ui.button(&clean).clicked() {
                                                                    pending_reactions.push((target_from.clone(), target_ts, clean.clone()));
                                                                    ui.memory_mut(|m| m.close_popup());
                                                                }
                                                            }
                                                        });
                                                    },
                                                );
                                            });
                                        });
                                });
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

                // Apply pending reply selection (set chat composing context).
                if let Some(ctx) = pending_reply.take() {
                    state.chat_reply_to = Some(ctx);
                }

                // Apply pending edit-target click (open the inline editor for this message).
                if let Some((ts, draft)) = pending_edit.take() {
                    state.chat_edit_target = Some((ts, draft));
                }

                // Apply pending edit cancel (clear the inline editor).
                if pending_edit_cancel {
                    state.chat_edit_target = None;
                }

                // Apply pending edit save (send the WS edit message + clear edit target).
                if let Some((ts, new_content)) = pending_edit_save.take() {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "edit",
                                "from": state.profile_public_key,
                                "timestamp": ts,
                                "new_content": new_content.clone(),
                                "channel": state.chat_active_channel,
                            });
                            client.send(&msg.to_string());
                        }
                    }
                    // Optimistic local update so the UI shows the new text immediately.
                    for m in state.chat_messages.iter_mut() {
                        if m.sender_key == state.profile_public_key && m.timestamp_ms == ts {
                            m.content = new_content;
                            break;
                        }
                    }
                    state.chat_edit_target = None;
                }

                // Send pending report slash commands.
                for (sender_name, _content) in pending_reports {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            let report_cmd = format!("/report {}", sender_name);
                            let m = serde_json::json!({
                                "type": "chat",
                                "from": state.profile_public_key,
                                "from_name": state.user_name,
                                "content": report_cmd,
                                "timestamp": ts,
                                "channel": state.chat_active_channel,
                            });
                            client.send(&m.to_string());
                        }
                    }
                }

                // Send pending pin requests via WebSocket.
                for (from_key, from_name, content, ts) in pending_pins {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let r = serde_json::json!({
                                "type": "pin_request",
                                "from_key": from_key,
                                "from_name": from_name,
                                "content": content,
                                "timestamp": ts,
                                "channel": state.chat_active_channel,
                            });
                            client.send(&r.to_string());
                        }
                    }
                }

                // Apply any pending reaction sends collected during render.
                for (target_from, target_ts, emoji) in pending_reactions {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let from_name = if !state.user_name.is_empty() {
                                state.user_name.clone()
                            } else {
                                "Anonymous".to_string()
                            };
                            let r = serde_json::json!({
                                "type": "reaction",
                                "target_from": target_from,
                                "target_timestamp": target_ts,
                                "emoji": emoji,
                                "from": state.profile_public_key,
                                "from_name": from_name,
                                "channel": state.chat_active_channel,
                            });
                            client.send(&r.to_string());
                        }
                    }
                }
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
                // Reply banner — only shown when a reply context is active.
                if let Some(ref reply) = state.chat_reply_to.clone() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "↩ Replying to {}: {}",
                                reply.sender_name,
                                if reply.preview.len() > 60 {
                                    format!("{}…", &reply.preview[..60])
                                } else {
                                    reply.preview.clone()
                                }
                            ))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                        );
                        if widgets::Button::ghost("X").show(ui, theme) {
                            state.chat_reply_to = None;
                        }
                    });
                }

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

                    // ── @mention autocomplete ──
                    // If the input ends with `@partial` (no whitespace after the @),
                    // show a popup of matching users from chat_users.
                    let mention_partial: Option<String> = {
                        let text = &state.chat_input;
                        if let Some(at_pos) = text.rfind('@') {
                            let after_at = &text[at_pos + 1..];
                            // No whitespace after the @ means we're still typing the mention.
                            if !after_at.contains(char::is_whitespace) && after_at.len() < 32 {
                                Some(after_at.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(partial) = mention_partial {
                        let partial_lower = partial.to_lowercase();
                        let matches: Vec<String> = state.chat_users.iter()
                            .filter(|u| u.name.to_lowercase().starts_with(&partial_lower))
                            .take(5)
                            .map(|u| u.name.clone())
                            .collect();

                        if !matches.is_empty() && response.has_focus() {
                            let popup_id = egui::Id::new("mention_autocomplete");
                            ui.memory_mut(|m| m.open_popup(popup_id));
                            egui::popup::popup_above_or_below_widget(
                                ui, popup_id, &response,
                                egui::AboveOrBelow::Above,
                                egui::PopupCloseBehavior::CloseOnClickOutside,
                                |ui| {
                                    ui.set_min_width(200.0);
                                    ui.label(
                                        RichText::new(format!("Mention: @{}", partial))
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                    ui.separator();
                                    for name in matches {
                                        if ui.button(format!("@{}", name)).clicked() {
                                            // Replace the @partial with @name + space.
                                            if let Some(at_pos) = state.chat_input.rfind('@') {
                                                state.chat_input.truncate(at_pos);
                                                state.chat_input.push('@');
                                                state.chat_input.push_str(&name);
                                                state.chat_input.push(' ');
                                            }
                                            ui.memory_mut(|m| m.close_popup());
                                        }
                                    }
                                },
                            );
                        }
                    }

                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    // Search button — opens the message search modal.
                    if widgets::Button::ghost("🔍").show(ui, theme) {
                        state.chat_search_open = true;
                    }

                    // Pins button — shows pin count + opens the pins modal.
                    let pin_count = state.chat_pins.get(&state.chat_active_channel).map(|p| p.len()).unwrap_or(0);
                    let pin_label = if pin_count > 0 { format!("📌 {}", pin_count) } else { "📌".to_string() };
                    if widgets::Button::ghost(&pin_label).show(ui, theme) {
                        state.chat_pins_open = true;
                    }

                    // Help button (?) - opens slash commands reference
                    if widgets::Button::ghost("?").show(ui, theme) {
                        state.show_help_modal = !state.show_help_modal;
                    }

                    let send_clicked = widgets::Button::primary("Send").show(ui, theme);

                    if (enter_pressed || send_clicked) && !state.chat_input.trim().is_empty() {
                        let content = state.chat_input.trim().to_string();
                        let channel = state.chat_active_channel.clone();
                        // Single timestamp used for both the WS send and the local echo
                        // so reaction-targeting (which keys on sender + ts) matches.
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        // B3 fix (v0.199.0): track whether the WS send was
                        // aborted (DM unencryptable, awaiting modal confirm).
                        // If aborted we ALSO skip the local message push so
                        // the user doesn't see their own message echo for a
                        // message that didn't actually send.
                        let mut send_aborted = false;

                        // Send via WebSocket if connected
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {

                                // Resolve display name: prefer user_name, fall back to peer list, then "Anonymous"
                                let display_name = if !state.user_name.is_empty() {
                                    state.user_name.clone()
                                } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                                    if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "Anonymous".to_string() }
                                } else {
                                    "Anonymous".to_string()
                                };

                                // v0.199.0 (B3 fix): build the WS payload as Option<String>
                                // so the DM branch can SKIP the send if encryption isn't
                                // possible. Previously the DM code silently fell back to
                                // plaintext with only a log line — that's a downgrade
                                // attack vector. Now we stash the unencryptable message
                                // in state.dm_unencrypted_confirm and a modal asks the
                                // user to explicitly confirm "Send unencrypted anyway"
                                // or cancel.
                                let json_str_opt: Option<String> = if channel.starts_with("dm:") {
                                    let partner_key = &channel[3..];
                                    // Try to encrypt. Fail-reason strings line up with
                                    // PendingUnencryptedDm::reason values in mod.rs.
                                    let encrypt_outcome: Result<(String, String), &'static str> =
                                        try_encrypt_dm(state, partner_key, &content);
                                    match encrypt_outcome {
                                        Ok((content_b64, nonce_b64)) => {
                                            // Encrypted — build the DM JSON normally.
                                            let dm_obj = serde_json::json!({
                                                "type": "dm",
                                                "from": state.profile_public_key,
                                                "from_name": display_name,
                                                "to": partner_key,
                                                "content": content_b64,
                                                "nonce": nonce_b64,
                                                "encrypted": true,
                                                "timestamp": ts,
                                            });
                                            Some(dm_obj.to_string())
                                        }
                                        Err(reason) => {
                                            // B3: refuse the silent plaintext fallback.
                                            // Stash for the confirmation modal. The
                                            // pending DM holds the original plaintext
                                            // + ts so the user's choice is preserved if
                                            // they confirm later.
                                            let partner_name = state.chat_dms.iter()
                                                .find(|d| d.user_key == partner_key)
                                                .map(|d| d.user_name.clone())
                                                .unwrap_or_else(|| {
                                                    let take = 8.min(partner_key.len());
                                                    partner_key[..take].to_string()
                                                });
                                            log::warn!(
                                                "DM to {} ({}) cannot be encrypted ({}). Asking user to confirm plaintext.",
                                                partner_name, partner_key, reason
                                            );
                                            state.dm_unencrypted_confirm = Some(crate::gui::PendingUnencryptedDm {
                                                partner_key: partner_key.to_string(),
                                                partner_name,
                                                content: content.clone(),
                                                timestamp_ms: ts,
                                                reason: reason.to_string(),
                                            });
                                            None
                                        }
                                    }
                                } else if channel.starts_with("group:") {
                                    // Group: send as type "group_msg"
                                    let group_id = &channel[6..];
                                    Some(serde_json::json!({
                                        "type": "group_msg",
                                        "group_id": group_id,
                                        "content": content,
                                    }).to_string())
                                } else {
                                    // Normal channel chat. Include reply_to if a thread context is active.
                                    let mut chat_obj = serde_json::json!({
                                        "type": "chat",
                                        "from": state.profile_public_key,
                                        "from_name": display_name,
                                        "content": content,
                                        "timestamp": ts,
                                        "channel": channel,
                                    });
                                    if let Some(ref r) = state.chat_reply_to {
                                        chat_obj["reply_to"] = serde_json::json!({
                                            "from": r.sender_key,
                                            "from_name": r.sender_name,
                                            "content": r.preview,
                                            "timestamp": r.timestamp_ms,
                                        });
                                    }
                                    Some(chat_obj.to_string())
                                };
                                if let Some(json_str) = json_str_opt {
                                    crate::debug::push_debug(format!("WS >>> {}", json_str));
                                    client.send(&json_str);

                                    // Track timestamp for dedup when server echoes it back
                                    state.chat_sent_timestamps.push(ts);
                                    // Keep only last 20 timestamps
                                    if state.chat_sent_timestamps.len() > 20 {
                                        state.chat_sent_timestamps.remove(0);
                                    }
                                } else {
                                    // B3: send was aborted because DM is unencryptable.
                                    // Skip the local message push too — modal will
                                    // handle resend after user confirms.
                                    send_aborted = true;
                                }
                            }
                        }

                        if send_aborted {
                            // Don't add the local echo or clear input — the
                            // pending DM is in state.dm_unencrypted_confirm.
                            // Modal will deal with it on the next frame.
                            // Returning the closure (early-out via continue
                            // analog: just skip the rest of the if-block).
                        } else {

                        // Store locally so user sees their own message immediately
                        let now = chrono_now_str();
                        let local_name = if !state.user_name.is_empty() {
                            state.user_name.clone()
                        } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                            if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "You".to_string() }
                        } else {
                            "You".to_string()
                        };
                        let local_reply_to = state.chat_reply_to.clone();
                        state.chat_messages.push(ChatMessage {
                            sender_name: local_name,
                            sender_key: state.profile_public_key.clone(),
                            content,
                            timestamp: now,
                            timestamp_ms: ts,
                            channel,
                            reply_to: local_reply_to,
                            ..Default::default()
                        });
                        // Clear reply context — the reply has been sent.
                        state.chat_reply_to = None;

                        while state.chat_messages.len() > 200 {
                            state.chat_messages.remove(0);
                        }

                        state.chat_input.clear();
                        response.request_focus();
                        } // end of `else` branch added by B3 fix (send_aborted == false)
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
                        if state.chat_active_channel != dm_channel {
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
    let mut open = state.show_create_channel_modal;
    widgets::dialog(ctx, theme, "create_channel_dialog", "Create Channel", &mut open, |ui| {
        ui.set_min_width(300.0);

        widgets::form_row(ui, theme, "Channel name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.new_channel_name)
                    .desired_width(220.0)
                    .hint_text("e.g. announcements"),
            );
        });

        widgets::form_row(ui, theme, "Description", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.new_channel_description)
                    .desired_width(220.0)
                    .hint_text("What is this channel about?"),
            );
        });

        ui.add_space(theme.spacing_md);

        ui.horizontal(|ui| {
            let name_valid = !state.new_channel_name.trim().is_empty();
            ui.add_enabled_ui(name_valid, |ui| {
                if widgets::Button::primary("Create").show(ui, theme) {
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
            });
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                state.show_create_channel_modal = false;
            }
        });
    });
    // Apply X-button close back to state.
    if !open {
        state.show_create_channel_modal = false;
    }
}

// ─────────────────────────────── Add Server Modal ───────────────────────

fn draw_add_server_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_add_server_modal;
    widgets::dialog(ctx, theme, "add_server_dialog", "Add Server", &mut open, |ui| {
        ui.set_min_width(380.0);
        ui.label(
            RichText::new(
                "Connect to another HumanityOS relay. Paste the server's URL \
                 (e.g. https://example.com) and an optional display name. \
                 The server is added to your sidebar; you can switch to it \
                 by clicking its name."
            )
            .size(theme.font_size_small)
            .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_md);

        widgets::form_row(ui, theme, "Server URL", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.add_server_url_draft)
                    .desired_width(280.0)
                    .hint_text("https://example.com"),
            );
        });
        widgets::form_row(ui, theme, "Display name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.add_server_name_draft)
                    .desired_width(280.0)
                    .hint_text("(optional — derived from URL if blank)"),
            );
        });

        ui.add_space(theme.spacing_md);

        // Validation: URL must be non-empty and parse-able with http/https
        // scheme. Doesn't reach the server here — the connect attempt
        // happens later when the user clicks the new server's row.
        let url = state.add_server_url_draft.trim();
        let url_valid = !url.is_empty()
            && (url.starts_with("https://") || url.starts_with("http://"))
            && url.len() > 8;

        ui.horizontal(|ui| {
            ui.add_enabled_ui(url_valid, |ui| {
                if widgets::Button::primary("Add").show(ui, theme) {
                    let normalized = url.trim_end_matches('/').to_string();
                    let derived_name = normalized
                        .trim_start_matches("https://")
                        .trim_start_matches("http://")
                        .split('/').next().unwrap_or("server").to_string();
                    let display_name = if state.add_server_name_draft.trim().is_empty() {
                        derived_name
                    } else {
                        state.add_server_name_draft.trim().to_string()
                    };
                    // Append to chat_servers if not already present.
                    if !state.chat_servers.iter().any(|s| s.url == normalized) {
                        state.chat_servers.push(crate::gui::ChatServer {
                            id: format!("srv_{}", normalized),
                            name: display_name,
                            url: normalized,
                            connected: false,
                            channels: Vec::new(),
                            voice_channels: Vec::new(),
                        });
                    }
                    state.show_add_server_modal = false;
                }
            });
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                state.show_add_server_modal = false;
            }
        });
    });
    if !open {
        state.show_add_server_modal = false;
    }
}

// ─────────────────────────────── Edit Channel Modal ──────────────────────

fn draw_edit_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_channel_edit_modal;
    widgets::dialog(ctx, theme, "edit_channel_dialog", "Edit Channel", &mut open, |ui| {
        ui.set_min_width(300.0);

        ui.label(
            RichText::new(format!("Editing: #{}", state.edit_channel_id))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);

        widgets::form_row(ui, theme, "Channel name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.edit_channel_name).desired_width(220.0));
        });
        widgets::form_row(ui, theme, "Description", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.edit_channel_description).desired_width(220.0));
        });

        // Voice enabled toggle.
        let mut voice_enabled = state.chat_channels.iter()
            .find(|c| c.id == state.edit_channel_id)
            .map(|c| c.voice_enabled)
            .unwrap_or(true);
        if ui.checkbox(&mut voice_enabled, "Voice enabled").changed() {
            if let Some(ch) = state.chat_channels.iter_mut().find(|c| c.id == state.edit_channel_id) {
                ch.voice_enabled = voice_enabled;
            }
        }

        ui.add_space(theme.spacing_md);

        ui.horizontal(|ui| {
            let name_valid = !state.edit_channel_name.trim().is_empty();
            ui.add_enabled_ui(name_valid, |ui| {
                if widgets::Button::primary("Save").show(ui, theme) {
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
            });
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                state.show_channel_edit_modal = false;
            }
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // Delete channel section
        if !state.edit_channel_confirm_delete {
            if widgets::Button::danger("Delete Channel").show(ui, theme) {
                state.edit_channel_confirm_delete = true;
            }
        } else {
            widgets::alert(ui, theme, widgets::AlertKind::Warning,
                "Are you sure? This cannot be undone.");
            ui.add_space(theme.spacing_sm);
            ui.horizontal(|ui| {
                if widgets::Button::danger("Yes, Delete").show(ui, theme) {
                    let ch_id = state.edit_channel_id.clone();
                    let ch_name = state.edit_channel_name.clone();
                    send_slash_command(state, &format!("/channel-delete {}", ch_id));
                    if ch_name.to_lowercase() != ch_id.to_lowercase() {
                        send_slash_command(state, &format!("/channel-delete {}", ch_name));
                    }
                    log::info!("Channel delete: id={}, name={}", ch_id, ch_name);
                    if state.chat_active_channel == ch_name {
                        state.chat_active_channel = "general".to_string();
                    }
                    state.show_channel_edit_modal = false;
                    state.edit_channel_confirm_delete = false;
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("No, Keep").show(ui, theme) {
                    state.edit_channel_confirm_delete = false;
                }
            });
        }
    });
    if !open {
        state.show_channel_edit_modal = false;
    }
}

// ─────────────────────────────── Create Group Modal ─────────────────────

fn draw_create_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_create_group_modal;
    widgets::dialog(ctx, theme, "create_group_dialog", "Create Group", &mut open, |ui| {
        ui.set_min_width(300.0);

        widgets::form_row(ui, theme, "Group name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.new_group_name)
                    .desired_width(220.0)
                    .hint_text("e.g. My Team"),
            );
        });

        ui.add_space(theme.spacing_md);

        ui.horizontal(|ui| {
            let name_valid = !state.new_group_name.trim().is_empty();
            ui.add_enabled_ui(name_valid, |ui| {
                if widgets::Button::primary("Create").show(ui, theme) {
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
            });
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                state.show_create_group_modal = false;
            }
        });
    });
    if !open {
        state.show_create_group_modal = false;
    }
}

// ─────────────────────────────── Join Group Modal ──────────────────────

fn draw_join_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_join_group_modal;
    widgets::dialog(ctx, theme, "join_group_dialog", "Join Group", &mut open, |ui| {
        ui.set_min_width(300.0);

        widgets::form_row(ui, theme, "Invite code", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.join_group_invite_code)
                    .desired_width(220.0)
                    .hint_text("Paste invite code here"),
            );
        });

        ui.add_space(theme.spacing_md);

        ui.horizontal(|ui| {
            let code_valid = !state.join_group_invite_code.trim().is_empty();
            ui.add_enabled_ui(code_valid, |ui| {
                if widgets::Button::primary("Join").show(ui, theme) {
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
            });
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                state.show_join_group_modal = false;
            }
        });
    });
    if !open {
        state.show_join_group_modal = false;
    }
}

// ─────────────────────────────── UI Helpers ──────────────────────────────

/// Draw a lock/unlock toggle button — designed to sit flush in a panel
/// corner with NO surrounding padding. Allocates exactly 14×14 with no
/// horizontal wrapper or item spacing, so when the caller positions an
/// Area at the panel boundary the button paints exactly there.
/// Returns true if the button was clicked (toggle lock state).
fn draw_panel_lock_button(ui: &mut egui::Ui, _theme: &Theme, locked: bool) -> bool {
    let tooltip = if locked { "Unlock panel width" } else { "Lock panel width" };
    let color = if locked { Color32::from_rgb(200, 180, 100) } else { Color32::from_rgb(100, 100, 100) };
    let (rect, resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
    if locked {
        crate::gui::widgets::icons::paint_lock(ui.painter(), rect, color);
    } else {
        crate::gui::widgets::icons::paint_unlock(ui.painter(), rect, color);
    }
    resp.on_hover_text(tooltip).clicked()
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

/// Measure the exact width the timestamp pill needs. Used by chat.rs to
/// pass `pill_width` into message_row so the row reserves the correct
/// amount of horizontal space. paint_timestamp_pill MUST keep its
/// rendering math in sync with this function — if they diverge the
/// reservation will be wrong and message text will overlap the pill.
///
/// Layout (matches paint_timestamp_pill):
///   [ 6px pad | timestamp | 5px gap | Þ | 4px gap | badges (3px gap each) | 6px pad ]
fn compute_pill_width(
    ctx: &egui::Context,
    theme: &Theme,
    timestamp: &str,
    reactions: &std::collections::HashMap<String, Vec<String>>,
) -> f32 {
    let ts_clean = timestamp.trim().trim_end_matches(" UTC").trim().to_string();
    let ts_w = ctx.fonts(|f| {
        f.layout_no_wrap(
            ts_clean,
            egui::FontId::proportional(theme.small_size),
            theme.text_muted(),
        )
    }).size().x;

    let thorn_w = ctx.fonts(|f| {
        f.layout_no_wrap(
            "Þ".to_string(),
            egui::FontId::proportional(theme.font_size_body),
            theme.accent(),
        )
    }).size().x;

    // Base layout (must match paint_timestamp_pill):
    //   left_pad(6) + ts_w + gap(5) + thorn_w + right_pad(6)
    let mut total = 6.0 + ts_w + 5.0 + thorn_w + 6.0;

    if !reactions.is_empty() {
        // When badges are present they REPLACE the right pad with:
        // gap_before_first(4) + sum(badge_body) + sum(gap 3 between)
        // + right_pad(6).
        total -= 6.0;
        let mut first = true;
        for (emoji, keys) in {
            let mut e: Vec<(&String, &Vec<String>)> = reactions.iter().collect();
            e.sort_by(|a, b| a.0.cmp(b.0));
            e.into_iter().take(4)
        } {
            if keys.is_empty() { continue; }
            // Match paint_timestamp_pill — strip FE0F before measuring so
            // the badge width estimate matches the rendered width.
            let display_emoji: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
            let label = format!("{}{}", display_emoji, keys.len());
            let label_w = ctx.fonts(|f| {
                f.layout_no_wrap(
                    label,
                    egui::FontId::proportional(theme.small_size),
                    theme.text_primary(),
                )
            }).size().x;
            total += if first { 4.0 } else { 3.0 };
            first = false;
            total += label_w + 4.0; // badge body = label_w + 2px each side
        }
        total += 6.0; // right pad after last badge
    }
    total.ceil()
}

/// Paint the inline timestamp pill: a small rounded frame containing the
/// timestamp text, a Þ separator, and any existing reaction badges (with
/// counts). Anchored at the rect message_row reserved.
///
/// Clicking an existing reaction toggles your own. Clicking Þ has no
/// dedicated action (the pill expands via row hover; see the
/// `pill_expand` Area in the message render block).
fn paint_timestamp_pill(
    ui: &mut egui::Ui,
    theme: &Theme,
    rect: egui::Rect,
    timestamp: &str,
    reactions: &std::collections::HashMap<String, Vec<String>>,
    my_key: &str,
    msg_ts_ms: u64,
    msg_sender_key: String,
    pending_reactions: &mut Vec<(String, u64, String)>,
) {
    let painter = ui.painter();
    // Pill background — fully OPAQUE so the underlying transparent layout
    // spacer doesn't let message text bleed through. Earlier the alpha
    // was 200 which produced visible text overlap on long pill widths.
    painter.rect_filled(rect, Rounding::same(9), theme.bg_card());
    painter.rect_stroke(
        rect,
        Rounding::same(9),
        Stroke::new(1.0, theme.border()),
        egui::StrokeKind::Inside,
    );

    let ts_clean = timestamp.trim().trim_end_matches(" UTC").trim();
    let cy = rect.center().y;
    let mut x = rect.left() + 6.0; // left pad — must match compute_pill_width

    // Timestamp text
    let ts_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            ts_clean.to_string(),
            egui::FontId::proportional(theme.small_size),
            theme.text_muted(),
        )
    });
    let ts_h = ts_galley.size().y;
    let ts_w = ts_galley.size().x;
    painter.galley(egui::pos2(x, cy - ts_h / 2.0), ts_galley, theme.text_muted());
    x += ts_w + 5.0; // ts width + gap before Þ — must match compute

    // Þ pull-tab marker
    let thorn_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            "Þ".to_string(),
            egui::FontId::proportional(theme.font_size_body),
            theme.accent(),
        )
    });
    let thorn_h = thorn_galley.size().y;
    let thorn_w = thorn_galley.size().x;
    painter.galley(egui::pos2(x, cy - thorn_h / 2.0), thorn_galley, theme.accent());
    x += thorn_w; // advance past Þ; first-badge gap added below

    // Existing reaction badges (right of Þ). Each = [2px-pad emoji+count 2px-pad]
    // followed by 3px gap to the next badge. First badge follows Þ after a 4px gap.
    if !reactions.is_empty() {
        let mut emojis: Vec<(&String, &Vec<String>)> = reactions.iter().collect();
        emojis.sort_by(|a, b| a.0.cmp(b.0));
        let mut first = true;
        for (emoji, keys) in emojis.into_iter().take(4) {
            let count = keys.len();
            if count == 0 { continue; }
            // Strip U+FE0F variation selector from any pre-existing reaction
            // (older clients may have stored "❤️" with the selector — render
            // path now uses bare codepoint to avoid the trailing tofu square).
            let display_emoji: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
            let i_reacted = keys.contains(&my_key.to_string());
            let label = format!("{}{}", display_emoji, count);
            let label_galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    label.clone(),
                    egui::FontId::proportional(theme.small_size),
                    if i_reacted { theme.accent() } else { theme.text_primary() },
                )
            });
            let label_w = label_galley.size().x;
            let badge_w = label_w + 4.0; // 2px internal pad each side
            // Pre-badge gap: 4 for first, 3 for subsequent — must match compute.
            x += if first { 4.0 } else { 3.0 };
            first = false;
            let badge_rect = egui::Rect::from_min_size(
                egui::pos2(x, cy - 8.0),
                Vec2::new(badge_w, 16.0),
            );
            // Stop drawing if we'd overflow the reserved pill width.
            if badge_rect.right() > rect.right() - 2.0 { break; }
            let badge_bg = if i_reacted {
                let a = theme.accent();
                Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 60)
            } else {
                Color32::TRANSPARENT
            };
            painter.rect_filled(badge_rect, Rounding::same(7), badge_bg);
            painter.galley(
                egui::pos2(badge_rect.left() + 2.0, cy - label_galley.size().y / 2.0),
                label_galley,
                if i_reacted { theme.accent() } else { theme.text_primary() },
            );
            // Click badge to toggle this reaction. Send the SAME key that was
            // stored (with or without FE0F) so the relay matches and toggles
            // the correct entry — don't substitute the cleaned display string.
            let resp = ui.interact(
                badge_rect,
                egui::Id::new(("react_pill_inline", msg_ts_ms, emoji.clone())),
                egui::Sense::click(),
            );
            if resp.clicked() {
                pending_reactions.push((msg_sender_key.clone(), msg_ts_ms, emoji.clone()));
            }
            x += badge_w; // advance past the badge body; next-badge gap added next iter
        }
    }
}

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
    let mut open = state.show_help_modal;
    widgets::dialog(ctx, theme, "slash_commands_dialog", "Slash Commands", &mut open, |ui| {
        ui.set_min_width(420.0);
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
    if !open {
        state.show_help_modal = false;
    }
}

// ──────────────────────────────────────────────────────────────────────────
// B3 fix (v0.199.0): DM crypto silent downgrade prevention.
// ──────────────────────────────────────────────────────────────────────────

/// Attempt to encrypt a DM. Returns `Ok((content_b64, nonce_b64))` if
/// encryption succeeds, `Err(reason)` if it can't be encrypted at all.
/// The reason string flows into `PendingUnencryptedDm::reason` for the
/// confirmation modal.
///
/// Failure reasons:
///   - `"no_own_ecdh"`        — this client has no ECDH private key set
///   - `"missing_peer_key"`   — recipient's ECDH public key isn't known
///   - `"bad_own_ecdh_hex"`   — our key is set but malformed (hex error)
///   - `"bad_own_ecdh_len"`   — our key is wrong length (not 32 bytes)
///   - `"bad_own_ecdh_keypair"` — keypair construction failed
///   - `"encryption_failed"`  — encrypt_dm() returned an error
fn try_encrypt_dm(
    state: &GuiState,
    partner_key: &str,
    content: &str,
) -> Result<(String, String), &'static str> {
    if state.ecdh_private_hex.is_empty() {
        return Err("no_own_ecdh");
    }
    let peer_ecdh = state.peer_ecdh_keys.get(partner_key)
        .ok_or("missing_peer_key")?;
    let sb = hex::decode(&state.ecdh_private_hex)
        .map_err(|_| "bad_own_ecdh_hex")?;
    if sb.len() != 32 {
        return Err("bad_own_ecdh_len");
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&sb);
    let kp = crate::net::dm_crypto::DmKeypair::from_secret_bytes(&bytes)
        .map_err(|_| "bad_own_ecdh_keypair")?;
    crate::net::dm_crypto::encrypt_dm(&kp, peer_ecdh, content)
        .map(|enc| (enc.content_b64, enc.nonce_b64))
        .map_err(|_| "encryption_failed")
}

/// Render the unencrypted-DM confirmation modal if one is pending.
/// Pops up when the user clicked Send on a DM that we couldn't encrypt
/// (B3 fix). User must explicitly confirm "Send unencrypted" or cancel
/// — no silent plaintext fallback. Call from chat::draw.
pub(crate) fn draw_unencrypted_dm_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let pending = match state.dm_unencrypted_confirm.clone() {
        Some(p) => p,
        None => return,
    };

    let reason_human = match pending.reason.as_str() {
        "no_own_ecdh" => "Your device has no encryption key set.",
        "missing_peer_key" => "We don't have the recipient's encryption key yet — they may not have come online with a current key, or their key broadcast hasn't reached us.",
        "bad_own_ecdh_hex" | "bad_own_ecdh_len" | "bad_own_ecdh_keypair" =>
            "Your encryption key on this device is malformed. Try Identity → Recover.",
        "encryption_failed" => "Encryption failed unexpectedly.",
        other => other,
    };

    let mut close_modal = false;
    let mut send_anyway = false;
    let mut cancel_clicked = false;

    egui::Window::new("⚠ Unencrypted DM Confirmation")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(egui::Vec2::new(440.0, 0.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new(format!("Sending to: {}", pending.partner_name))
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new("This message CANNOT be end-to-end encrypted.")
                    .size(theme.font_size_body)
                    .color(theme.danger())
                    .strong(),
            );
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(reason_human)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Sending unencrypted means anyone with access to the relay server can read it. The recipient is NOT protected from a hostile or compromised relay. Continue?")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Your message:")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            // Show a preview of the message body, truncated to keep the
            // modal compact even for long drafts.
            let preview = if pending.content.chars().count() > 240 {
                let truncated: String = pending.content.chars().take(240).collect();
                format!("{}…", truncated)
            } else {
                pending.content.clone()
            };
            egui::Frame::none()
                .fill(theme.bg_panel())
                .rounding(egui::Rounding::same(theme.border_radius as u8))
                .inner_margin(theme.card_padding)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(&preview)
                            .size(theme.font_size_small)
                            .color(theme.text_primary())
                            .monospace(),
                    );
                });
            ui.add_space(theme.spacing_md);
            ui.horizontal(|ui| {
                if widgets::Button::secondary("Cancel — keep it private")
                    .tooltip("Don't send. The message is restored to your input box so you can wait for the recipient's encryption key, edit, or copy it elsewhere.")
                    .show(ui, theme)
                {
                    cancel_clicked = true;
                    close_modal = true;
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::danger("Send unencrypted anyway")
                    .tooltip("Send the plaintext message NOW. The relay (and anyone with access to it) will be able to read it.")
                    .show(ui, theme)
                {
                    send_anyway = true;
                    close_modal = true;
                }
            });
            ui.add_space(theme.spacing_sm);
        });

    if cancel_clicked {
        // Restore draft into the input box so the user can decide what to do.
        state.chat_input = pending.content.clone();
    }

    if send_anyway {
        // Build and send the plaintext DM directly. We bypass the normal
        // send path because that path would re-trigger the confirm modal.
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let display_name = if !state.user_name.is_empty() {
                    state.user_name.clone()
                } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                    if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "Anonymous".to_string() }
                } else {
                    "Anonymous".to_string()
                };
                let dm_obj = serde_json::json!({
                    "type": "dm",
                    "from": state.profile_public_key,
                    "from_name": display_name,
                    "to": pending.partner_key,
                    "content": pending.content,
                    "timestamp": pending.timestamp_ms,
                    // Explicit false so the relay + recipient can show
                    // an "unencrypted" indicator if they want.
                    "encrypted": false,
                });
                let json_str = dm_obj.to_string();
                crate::debug::push_debug(format!("WS >>> [user-confirmed plaintext] {}", json_str));
                client.send(&json_str);
                state.chat_sent_timestamps.push(pending.timestamp_ms);
                if state.chat_sent_timestamps.len() > 20 {
                    state.chat_sent_timestamps.remove(0);
                }
                // Local echo so the user sees their own message immediately,
                // matching the normal-send code path's behavior.
                let local_name = if !state.user_name.is_empty() {
                    state.user_name.clone()
                } else {
                    "You".to_string()
                };
                let now = chrono_now_str();
                state.chat_messages.push(ChatMessage {
                    sender_name: local_name,
                    sender_key: state.profile_public_key.clone(),
                    content: pending.content.clone(),
                    timestamp: now,
                    timestamp_ms: pending.timestamp_ms,
                    channel: format!("dm:{}", pending.partner_key),
                    reply_to: None,
                    ..Default::default()
                });
                while state.chat_messages.len() > 200 {
                    state.chat_messages.remove(0);
                }
                state.chat_input.clear();
            }
        }
    }

    if close_modal {
        state.dm_unencrypted_confirm = None;
    }
}

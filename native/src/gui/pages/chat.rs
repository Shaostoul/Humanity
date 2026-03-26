//! 3-panel chat page matching the website layout.
//!
//! LEFT:   Collapsible DMs (red), Groups (green), Servers (blue), Connection settings
//! MIDDLE: Channel header, message feed, input bar
//! RIGHT:  Friends list, Server members list

use egui::{Color32, Frame, RichText, ScrollArea, Stroke, Vec2};
use crate::gui::{ChatMessage, ChatUser, GuiState};
use crate::gui::theme::Theme;

/// Maximum messages kept in the local chat buffer.
const MAX_MESSAGES: usize = 200;

/// Left panel width in points.
const LEFT_PANEL_WIDTH: f32 = 220.0;
/// Right panel width in points.
const RIGHT_PANEL_WIDTH: f32 = 220.0;

// ── Section collapse state (persists across frames via thread_local) ──

use std::cell::RefCell;

#[derive(Debug)]
struct CollapseState {
    connection: bool,
    dms: bool,
    groups: bool,
    servers: bool,
    friends: bool,
    members: bool,
}

impl Default for CollapseState {
    fn default() -> Self {
        Self {
            connection: true, // collapsed by default
            dms: false,
            groups: false,
            servers: false,
            friends: false,
            members: false,
        }
    }
}

thread_local! {
    static COLLAPSE: RefCell<CollapseState> = RefCell::new(CollapseState::default());
}

fn with_collapse<R>(f: impl FnOnce(&mut CollapseState) -> R) -> R {
    COLLAPSE.with(|c| f(&mut c.borrow_mut()))
}

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
    egui::SidePanel::left("chat_left_panel")
        .exact_width(LEFT_PANEL_WIDTH)
        .resizable(false)
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_left_panel(ui, theme, state);
        });

    // ── RIGHT PANEL ──
    egui::SidePanel::right("chat_right_panel")
        .exact_width(RIGHT_PANEL_WIDTH)
        .resizable(false)
        .frame(Frame::NONE.fill(Color32::from_rgb(30, 30, 36)).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_right_panel(ui, theme, state);
        });

    // ── CENTER PANEL ──
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(Color32::from_rgb(20, 20, 25)).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_center_panel(ui, theme, state);
        });
}

// ─────────────────────────────── LEFT PANEL ───────────────────────────────

fn draw_left_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Connection status bar ──
    let is_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    if is_connected {
        // Compact connected indicator
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, theme.success());
            ui.label(
                RichText::new(format!("Connected ({} online)", state.chat_users.iter().filter(|u| u.status != "offline").count()))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
    } else {
        // Prominent connect section when disconnected
        Frame::NONE
            .fill(Color32::from_rgb(40, 30, 30))
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Not Connected")
                        .size(theme.font_size_body)
                        .color(theme.danger())
                        .strong(),
                );
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
                        .desired_width(LEFT_PANEL_WIDTH - 24.0)
                        .font(egui::TextStyle::Small),
                );
                ui.add_space(2.0);
                ui.label(RichText::new("Name:").size(theme.font_size_small).color(theme.text_muted()));
                ui.add(
                    egui::TextEdit::singleline(&mut state.user_name)
                        .desired_width(LEFT_PANEL_WIDTH - 24.0)
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
                        .min_size(Vec2::new(LEFT_PANEL_WIDTH - 32.0, 32.0)),
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

    // Collapsible connection settings (when connected, to allow changing server/disconnect)
    if is_connected {
        let conn_collapsed = with_collapse(|c| c.connection);
        if section_header(ui, "Connection", conn_collapsed, Color32::from_rgb(40, 40, 48)) {
            with_collapse(|c| c.connection = !c.connection);
        }
        if !conn_collapsed {
            Frame::NONE
                .fill(Color32::from_rgb(35, 35, 42))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.label(RichText::new("Server:").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.server_url)
                            .desired_width(LEFT_PANEL_WIDTH - 24.0)
                            .font(egui::TextStyle::Small),
                    );
                    ui.add_space(2.0);
                    ui.label(RichText::new("Name:").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.user_name)
                            .desired_width(LEFT_PANEL_WIDTH - 24.0)
                            .font(egui::TextStyle::Small),
                    );
                    ui.add_space(4.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Disconnect")
                                    .size(theme.font_size_small)
                                    .color(theme.text_primary()),
                            )
                            .fill(Color32::from_rgb(60, 30, 30))
                            .min_size(Vec2::new(LEFT_PANEL_WIDTH - 32.0, 26.0)),
                        )
                        .clicked()
                    {
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
        }
    }

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
    let collapsed = with_collapse(|c| c.dms);
    let dm_count = state.chat_dms.len();

    if tinted_section_header(ui, &format!("DMs ({})", dm_count), collapsed, DM_BG) {
        with_collapse(|c| c.dms = !c.dms);
    }

    if !collapsed {
        Frame::NONE
            .fill(DM_BG)
            .inner_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
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
                            Vec2::new(ui.available_width(), 38.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let full_rect = ui.max_rect();
                                let bg = if ui.rect_contains_pointer(full_rect) {
                                    DM_ROW_HOVER
                                } else {
                                    DM_ROW_BG
                                };
                                ui.painter().rect_filled(full_rect, 0.0, bg);

                                ui.add_space(12.0);

                                // Unread dot
                                if dm.unread {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 3.0, theme.accent());
                                }

                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&dm.user_name)
                                                .size(theme.font_size_body)
                                                .color(if dm.unread { theme.text_primary() } else { theme.text_secondary() })
                                                .strong(),
                                        );
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new(&dm.timestamp)
                                                    .size(theme.font_size_small - 2.0)
                                                    .color(theme.text_muted()),
                                            );
                                        });
                                    });
                                    if !dm.last_message.is_empty() {
                                        ui.label(
                                            RichText::new(truncate_str(&dm.last_message, 30))
                                                .size(theme.font_size_small - 1.0)
                                                .color(theme.text_muted()),
                                        );
                                    }
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
    let collapsed = with_collapse(|c| c.groups);
    let group_count = state.chat_groups.len();

    if tinted_section_header(ui, &format!("Groups ({})", group_count), collapsed, GROUP_BG) {
        with_collapse(|c| c.groups = !c.groups);
    }

    if !collapsed {
        Frame::NONE
            .fill(GROUP_BG)
            .inner_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
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
                            Vec2::new(ui.available_width(), 30.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let full_rect = ui.max_rect();
                                let bg = if ui.rect_contains_pointer(full_rect) {
                                    GROUP_ROW_HOVER
                                } else {
                                    GROUP_ROW_BG
                                };
                                ui.painter().rect_filled(full_rect, 0.0, bg);
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new(&group.name)
                                        .size(theme.font_size_body)
                                        .color(theme.text_primary()),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(format!("{} members", group.member_count))
                                            .size(theme.font_size_small - 1.0)
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
                        RichText::new("Create Group").size(theme.font_size_small).color(theme.text_secondary()),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        // placeholder
                    }
                    if ui.add(egui::Button::new(
                        RichText::new("Join Group").size(theme.font_size_small).color(theme.text_secondary()),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        // placeholder
                    }
                });
                ui.add_space(2.0);
            });
    }
}

// ── Servers Section ──

fn draw_servers_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = with_collapse(|c| c.servers);

    // Build a virtual server from the current connection
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let virtual_server_count = if connected { 1 } else { 0 } + state.chat_servers.len();

    if tinted_section_header(ui, &format!("Servers ({})", virtual_server_count), collapsed, SERVER_BG) {
        with_collapse(|c| c.servers = !c.servers);
    }

    if !collapsed {
        Frame::NONE
            .fill(SERVER_BG)
            .inner_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
                // Current connected server (virtual entry)
                if connected {
                    // Server name header
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, theme.success());
                        ui.label(
                            RichText::new(server_display_name(&state.server_url))
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                    });
                    ui.add_space(4.0);

                    // Text channels
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("TEXT CHANNELS")
                                .size(theme.font_size_small - 2.0)
                                .color(theme.text_muted())
                                .strong(),
                        );
                    });
                    ui.add_space(2.0);

                    let active = state.chat_active_channel.clone();
                    let channels = state.chat_channels.clone();
                    for ch in &channels {
                        if ch.category == "Voice" {
                            continue;
                        }
                        let is_active = ch.id == active;
                        let bg = if is_active {
                            Color32::from_rgba_premultiplied(40, 40, 80, 255)
                        } else {
                            SERVER_ROW_BG
                        };

                        let response = ui
                            .allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), 26.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let full_rect = ui.max_rect();
                                    let hover = ui.rect_contains_pointer(full_rect);
                                    let fill = if hover && !is_active { SERVER_ROW_HOVER } else { bg };
                                    ui.painter().rect_filled(full_rect, 0.0, fill);
                                    ui.add_space(20.0);
                                    let text_color = if is_active {
                                        theme.text_primary()
                                    } else {
                                        theme.text_secondary()
                                    };
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
                            state.chat_messages.clear();
                            state.history_fetched = false;
                        }
                    }

                    ui.add_space(6.0);

                    // Voice channels
                    let voice_channels: Vec<&crate::gui::ChatChannel> = channels.iter().filter(|c| c.category == "Voice").collect();
                    if !voice_channels.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("VOICE CHANNELS")
                                    .size(theme.font_size_small - 2.0)
                                    .color(theme.text_muted())
                                    .strong(),
                            );
                        });
                        ui.add_space(2.0);

                        for vc in &voice_channels {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.label(
                                    RichText::new(format!("  {}", vc.name))
                                        .size(theme.font_size_body)
                                        .color(theme.text_muted()),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(8.0);
                                    if ui.add(egui::Button::new(
                                        RichText::new("Join").size(theme.font_size_small - 1.0).color(theme.text_secondary()),
                                    ).fill(Color32::TRANSPARENT)).clicked() {
                                        // placeholder
                                    }
                                });
                            });
                        }
                    } else {
                        // Default voice channels if none from server
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("VOICE CHANNELS")
                                    .size(theme.font_size_small - 2.0)
                                    .color(theme.text_muted())
                                    .strong(),
                            );
                        });
                        ui.add_space(2.0);
                        for label in &["Lounge", "Dev Talk"] {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.label(
                                    RichText::new(format!("  {}", label))
                                        .size(theme.font_size_body)
                                        .color(theme.text_muted()),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(8.0);
                                    if ui.add(egui::Button::new(
                                        RichText::new("Join").size(theme.font_size_small - 1.0).color(theme.text_secondary()),
                                    ).fill(Color32::TRANSPARENT)).clicked() {
                                        // placeholder
                                    }
                                });
                            });
                        }
                    }

                    ui.add_space(4.0);
                }

                // Additional servers from chat_servers list
                for server in state.chat_servers.clone().iter() {
                    ui.add_space(4.0);
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

                // + Add Server button
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    if ui.add(egui::Button::new(
                        RichText::new("+ Add Server").size(theme.font_size_small).color(theme.text_secondary()),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        // placeholder
                    }
                });
                ui.add_space(4.0);
            });
    }
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

fn draw_friends_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = with_collapse(|c| c.friends);
    let friend_count = state.chat_friends.len();

    if section_header(ui, &format!("Friends ({})", friend_count), collapsed, Color32::from_rgb(35, 35, 42)) {
        with_collapse(|c| c.friends = !c.friends);
    }

    if !collapsed {
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

        for friend in state.chat_friends.clone().iter() {
            ui.horizontal(|ui| {
                ui.add_space(12.0);

                // Online/offline dot
                let dot_color = if friend.status == "offline" {
                    Color32::from_rgb(100, 100, 100)
                } else {
                    theme.success()
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, dot_color);

                // Name
                ui.label(
                    RichText::new(&friend.name)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );

                // Role badges
                draw_role_badges(ui, theme, &friend.role);

                // Action buttons (DM, call)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui.add(egui::Button::new(
                        RichText::new("DM").size(theme.font_size_small - 1.0).color(theme.text_muted()),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        // placeholder
                    }
                });
            });
        }
    }
}

fn draw_members_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = with_collapse(|c| c.members);
    let member_count = state.chat_users.len();
    let server_name = server_display_name(&state.server_url);

    if section_header(ui, &format!("{} ({})", server_name, member_count), collapsed, Color32::from_rgb(35, 35, 42)) {
        with_collapse(|c| c.members = !c.members);
    }

    if !collapsed {
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

        for user in &users {
            ui.horizontal(|ui| {
                ui.add_space(12.0);

                // Online/offline dot
                let dot_color = match user.status.as_str() {
                    "offline" => Color32::from_rgb(100, 100, 100),
                    "away" => theme.warning(),
                    "busy" | "dnd" => theme.danger(),
                    _ => theme.success(),
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, dot_color);

                // Name
                let name_color = if user.status == "offline" {
                    theme.text_muted()
                } else {
                    theme.text_primary()
                };
                ui.label(
                    RichText::new(&user.name)
                        .size(theme.font_size_body)
                        .color(name_color),
                );

                // Role badges
                draw_role_badges(ui, theme, &user.role);
            });
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
                let bg_even = Color32::from_rgb(2, 2, 2);
                let bg_odd = Color32::from_rgb(4, 4, 4);

                for msg in &filtered {
                    let new_group = msg.sender_name != last_sender;
                    if new_group {
                        sender_parity = !sender_parity;
                    }
                    last_sender = msg.sender_name.clone();

                    if new_group {
                        ui.add_space(8.0);
                    }

                    // Paint row background based on sender parity
                    let row_bg = if sender_parity { bg_even } else { bg_odd };
                    let row_response = ui.horizontal(|ui| {
                        let full_rect = egui::Rect::from_min_size(
                            ui.cursor().min,
                            Vec2::new(ui.available_width(), ui.spacing().interact_size.y.max(28.0)),
                        );
                        ui.painter().rect_filled(full_rect, 0.0, row_bg);
                        ui.add_space(16.0);

                        if new_group {
                            // Avatar circle from name hash
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

                                let chat_msg = serde_json::json!({
                                    "type": "chat",
                                    "from": state.profile_public_key,
                                    "from_name": if state.user_name.is_empty() { "Anonymous".to_string() } else { state.user_name.clone() },
                                    "content": content,
                                    "timestamp": ts,
                                    "channel": channel,
                                });
                                client.send(&chat_msg.to_string());
                            }
                        }

                        // Store locally so user sees their own message immediately
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

// ─────────────────────────────── UI Helpers ──────────────────────────────

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

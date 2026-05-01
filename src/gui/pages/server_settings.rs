//! Server and group settings page.
//!
//! Reached from the cog menu on the server row or on a group row in the
//! chat sidebar. Renders three role-tiered sections, color-coded the same way
//! the nav bar groups pages:
//!
//! - **USER** (red) — visible to everyone: identity info, profile shortcut,
//!   notification preferences, disconnect.
//! - **MODERATOR** (green) — visible to mods + admins: kick / mute targeting.
//! - **ADMIN** (blue) — visible to admins only: lockdown toggle, invite
//!   generation, channel management, ban, verify, promote.
//!
//! Action buttons send slash commands through the existing `chat` channel —
//! the relay's slash-command processor (`/kick`, `/ban`, `/lockdown`, etc.)
//! does the actual server-side work, so we don't need new WebSocket message
//! types. This keeps server settings consistent with what works in chat.

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

/// Section identity colors — match the nav bar grouping in escape_menu.rs.
const USER_COLOR:  Color32 = Color32::from_rgb(231, 76, 60);   // RED — identity
const MOD_COLOR:   Color32 = Color32::from_rgb(46, 204, 113);  // GREEN — contextual
const ADMIN_COLOR: Color32 = Color32::from_rgb(52, 152, 219);  // BLUE — system

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    draw_header(ui, theme, state);
                    ui.add_space(theme.spacing_xl);

                    let role = current_user_role(state);
                    let is_mod   = matches!(role.as_str(), "mod" | "admin" | "owner");
                    let is_admin = matches!(role.as_str(), "admin" | "owner");

                    draw_user_section(ui, theme, state, &role);
                    ui.add_space(theme.spacing_lg);

                    if is_mod {
                        draw_mod_section(ui, theme, state);
                        ui.add_space(theme.spacing_lg);
                    }

                    if is_admin {
                        draw_admin_section(ui, theme, state);
                        ui.add_space(theme.spacing_lg);
                    }

                    if !state.server_settings_status.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.set_max_width(720.0);
                            let kind = if state.server_settings_status.starts_with("Error")
                                || state.server_settings_status.starts_with("Failed")
                            {
                                widgets::AlertKind::Error
                            } else {
                                widgets::AlertKind::Success
                            };
                            widgets::alert(ui, theme, kind, &state.server_settings_status);
                        });
                        ui.add_space(theme.spacing_md);
                    }

                    ui.add_space(theme.spacing_xl);
                });
        });
}

// ── Header ──────────────────────────────────────────────────────────────────

fn draw_header(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_lg);
    ui.horizontal(|ui| {
        ui.add_space(theme.spacing_lg);
        if widgets::Button::secondary("< Back to Chat").show(ui, theme) {
            state.active_page = GuiPage::Chat;
            state.server_settings_status.clear();
        }
    });
    ui.add_space(theme.spacing_md);

    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.label(
            RichText::new("SERVER / GROUP SETTINGS")
                .size(theme.font_size_small)
                .color(theme.accent())
                .strong(),
        );
        ui.add_space(theme.spacing_sm);
        let (scope_label, target_id) = resolve_scope(state);
        ui.label(
            RichText::new(scope_label)
                .size(theme.font_size_title)
                .color(theme.text_primary())
                .strong(),
        );
        if !target_id.is_empty() {
            ui.label(
                RichText::new(target_id)
                    .size(theme.font_size_small)
                    .color(theme.text_muted())
                    .monospace(),
            );
        }
    });
}

// ── Sections ────────────────────────────────────────────────────────────────

fn draw_user_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, role: &str) {
    color_section(ui, theme, "USER", USER_COLOR, |ui, theme| {
        kv_row(ui, theme, "Connected server", resolve_server_url(state));
        kv_row(ui, theme, "Your identity", short_key(&state.profile_public_key));
        kv_row(ui, theme, "Network status", state.ws_status.clone());
        kv_row(ui, theme, "Your role", role_label(role));

        ui.add_space(theme.spacing_md);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Open Profile").show(ui, theme) {
                state.active_page = GuiPage::Profile;
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Notification preferences").show(ui, theme) {
                state.active_page = GuiPage::Settings;
            }
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // Invite (everyone can copy server invite)
        let (label, invite_url) = match resolve_group(state) {
            Some((id, _name)) => (
                "Group invite link",
                format!("https://united-humanity.us/chat/group/{}", id),
            ),
            None => ("Server invite link", "https://united-humanity.us/chat".to_string()),
        };
        kv_row(ui, theme, label, invite_url.clone());
        ui.add_space(theme.spacing_sm);
        if widgets::Button::primary("Copy invite").show(ui, theme) {
            ui.ctx().copy_text(invite_url);
            state.server_settings_status = "Invite link copied to clipboard.".into();
        }

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // Disconnect (was the old "danger zone" action — leaves it in user scope
        // because anyone can disconnect themselves).
        let (disconnect_label, group_id) = match resolve_group(state) {
            Some((id, _name)) => ("Leave group", Some(id)),
            None => ("Disconnect from server", None),
        };
        if widgets::Button::danger(disconnect_label).show(ui, theme) {
            do_disconnect(state, group_id);
        }
    });
}

fn draw_mod_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    color_section(ui, theme, "MODERATOR", MOD_COLOR, |ui, theme| {
        ui.label(
            RichText::new("Targets the username typed below. Leave blank to use a slash command in chat instead.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);

        widgets::form_row(ui, theme, "Target user", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.server_settings_target_user)
                    .desired_width(220.0)
                    .hint_text("username"),
            );
        });

        ui.add_space(theme.spacing_sm);

        let target_valid = !state.server_settings_target_user.trim().is_empty();
        ui.add_enabled_ui(target_valid, |ui| {
            ui.horizontal(|ui| {
                if widgets::Button::secondary("Mute").show(ui, theme) {
                    let cmd = format!("/mute {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Unmute").show(ui, theme) {
                    let cmd = format!("/unmute {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::danger("Kick").show(ui, theme) {
                    let cmd = format!("/kick {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
            });
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        ui.label(
            RichText::new("Channel moderation")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Pin last message").show(ui, theme) {
                send_slash(state, "/pin");
                state.server_settings_status = "Sent: /pin".into();
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("View reports").show(ui, theme) {
                send_slash(state, "/reports");
                state.server_settings_status = "Sent: /reports — check the active channel for results.".into();
            }
        });
    });
}

fn draw_admin_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    color_section(ui, theme, "ADMIN", ADMIN_COLOR, |ui, theme| {
        // ── Lockdown + invites ──
        ui.label(
            RichText::new("Registration")
                .size(theme.font_size_body)
                .color(theme.text_primary())
                .strong(),
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Toggle lockdown").show(ui, theme) {
                send_slash(state, "/lockdown");
                state.server_settings_status = "Sent: /lockdown — registration toggle requested.".into();
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::primary("Generate invite code").show(ui, theme) {
                send_slash(state, "/invite");
                state.server_settings_status = "Sent: /invite — code will appear in the active channel.".into();
            }
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Channel management ──
        ui.label(
            RichText::new("Channels")
                .size(theme.font_size_body)
                .color(theme.text_primary())
                .strong(),
        );
        ui.add_space(theme.spacing_xs);
        widgets::form_row(ui, theme, "Channel name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.server_settings_channel_name)
                    .desired_width(220.0)
                    .hint_text("e.g. announcements"),
            );
        });
        let channel_valid = !state.server_settings_channel_name.trim().is_empty();
        ui.add_enabled_ui(channel_valid, |ui| {
            ui.horizontal(|ui| {
                if widgets::Button::primary("Create").show(ui, theme) {
                    let cmd = format!("/channel-create {}", state.server_settings_channel_name.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Toggle read-only").show(ui, theme) {
                    let cmd = format!("/channel-readonly {}", state.server_settings_channel_name.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::danger("Delete").show(ui, theme) {
                    state.server_settings_confirm_action = Some(format!(
                        "/channel-delete {}",
                        state.server_settings_channel_name.trim()
                    ));
                }
            });
        });

        // Confirm-delete prompt for channel deletion.
        if let Some(cmd) = state.server_settings_confirm_action.clone() {
            if cmd.starts_with("/channel-delete") {
                ui.add_space(theme.spacing_sm);
                widgets::alert(ui, theme, widgets::AlertKind::Warning,
                    "Delete this channel? Messages will be lost. This cannot be undone.");
                ui.add_space(theme.spacing_xs);
                ui.horizontal(|ui| {
                    if widgets::Button::danger("Yes, delete").show(ui, theme) {
                        send_slash(state, &cmd);
                        state.server_settings_status = format!("Sent: {}", cmd);
                        state.server_settings_confirm_action = None;
                    }
                    ui.add_space(theme.spacing_sm);
                    if widgets::Button::secondary("Cancel").show(ui, theme) {
                        state.server_settings_confirm_action = None;
                    }
                });
            }
        }

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── User management ──
        ui.label(
            RichText::new("User management")
                .size(theme.font_size_body)
                .color(theme.text_primary())
                .strong(),
        );
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(format!(
                "Targets the username from the Moderator section above (currently: {}).",
                if state.server_settings_target_user.trim().is_empty() {
                    "(empty)".to_string()
                } else {
                    state.server_settings_target_user.trim().to_string()
                }
            ))
            .size(theme.font_size_small)
            .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_xs);
        let user_valid = !state.server_settings_target_user.trim().is_empty();
        ui.add_enabled_ui(user_valid, |ui| {
            ui.horizontal(|ui| {
                if widgets::Button::secondary("Verify").show(ui, theme) {
                    let cmd = format!("/verify {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Promote to mod").show(ui, theme) {
                    let cmd = format!("/mod {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::danger("Ban").show(ui, theme) {
                    let cmd = format!("/ban {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
            });
        });
    });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Card with a colored top-border + accent-colored title. Color encodes the
/// privilege tier (red = user, green = mod, blue = admin).
fn color_section(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    color: Color32,
    contents: impl FnOnce(&mut egui::Ui, &Theme),
) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(720.0);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.label(
                RichText::new(title)
                    .size(theme.font_size_small)
                    .color(color)
                    .strong(),
            );
            ui.add_space(theme.spacing_sm);
            // Tinted background derived from the section color (alpha 18).
            let tint = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18);
            Frame::none()
                .fill(tint)
                .stroke(Stroke::new(1.5, color))
                .rounding(Rounding::same(theme.border_radius as u8))
                .inner_margin(theme.card_padding * 1.5)
                .show(ui, |ui| {
                    contents(ui, theme);
                });
        });
    });
}

fn kv_row(ui: &mut egui::Ui, theme: &Theme, key: &str, value: String) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(160.0, ui.spacing().interact_size.y),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(
                    RichText::new(key)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            },
        );
        ui.label(
            RichText::new(value)
                .size(theme.font_size_body)
                .color(theme.text_primary())
                .monospace(),
        );
    });
    ui.add_space(theme.spacing_xs);
}

/// Send a slash command via the existing chat WebSocket. Server processes
/// `/kick`, `/ban`, `/lockdown`, `/channel-*`, `/mod`, `/verify`, etc.
fn send_slash(state: &mut GuiState, command: &str) {
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

fn do_disconnect(state: &mut GuiState, group_id: Option<String>) {
    match group_id {
        Some(gid) => {
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
            state.chat_active_channel = "general".to_string();
            state.active_page = GuiPage::Chat;
        }
        None => {
            if let Some(ref mut client) = state.ws_client {
                client.disconnect();
            }
            state.ws_client = None;
            state.ws_status = "Disconnected".to_string();
            state.ws_manually_disconnected = true;
            state.active_page = GuiPage::Chat;
        }
    }
}

/// Look up the current user's role from the chat user list.
/// Returns one of "owner", "admin", "mod", "member", or "" (unknown).
fn current_user_role(state: &GuiState) -> String {
    state
        .chat_users
        .iter()
        .find(|u| u.public_key == state.profile_public_key)
        .map(|u| u.role.clone())
        .unwrap_or_default()
}

fn role_label(role: &str) -> String {
    match role {
        "owner" => "Owner".into(),
        "admin" => "Admin".into(),
        "mod"   => "Moderator".into(),
        "member" | "" => "Member".into(),
        other => other.to_string(),
    }
}

/// Figure out whether we are configuring a group (modal context was set) or
/// the currently connected server. Returns (header label, id/key string).
fn resolve_scope(state: &GuiState) -> (String, String) {
    if let Some((id, name)) = resolve_group(state) {
        (format!("Group: {}", name), id)
    } else {
        (resolve_server_url(state), String::new())
    }
}

fn resolve_group(state: &GuiState) -> Option<(String, String)> {
    let ctx_id = state.chat_user_modal_key.clone();
    if ctx_id.is_empty() { return None; }
    state
        .chat_groups
        .iter()
        .find(|g| g.id == ctx_id)
        .map(|g| (g.id.clone(), g.name.clone()))
}

fn resolve_server_url(state: &GuiState) -> String {
    if state.server_url.is_empty() {
        "HumanityOS".to_string()
    } else {
        state.server_url.clone()
    }
}

fn short_key(key: &str) -> String {
    if key.len() <= 16 {
        key.to_string()
    } else {
        format!("{}…{}", &key[..8], &key[key.len() - 4..])
    }
}

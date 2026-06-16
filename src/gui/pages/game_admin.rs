//! Game Admin page (v0.474) -- game-world moderation, kept STRUCTURALLY
//! SEPARATE from chat moderation.
//!
//! Operator directive (2026-06-16): "Bans for characters shouldn't ban users
//! from chat. The comms is the most important aspect of HumanityOS, I want to
//! guarantee free speech. Being able to play video games with each other on the
//! official MMO server is a privilege."
//!
//! So this page issues GAME bans only: they block a player from spawning into
//! the shared 3D world and never touch the chat ban path (`banned_keys`). A
//! game-banned user keeps full access to every channel + DM. The relay enforces
//! the separation in disjoint code (game_banned_keys table + a single check in
//! handle_game_join); this page is just the admin surface for it. Auth is
//! authoritative on the relay (get_role must be admin/owner); the role check
//! here is defense-in-depth so the page renders an honest message to non-admins.

use egui::{RichText, ScrollArea, Frame, Align, Layout};
use crate::gui::{GuiState, GuiPage};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                draw_header(ui, theme, state);
                ui.add_space(theme.spacing_md);
                draw_disclaimer(ui, theme);
                ui.add_space(theme.spacing_md);

                let role = current_game_admin_role(state);
                let is_admin = matches!(role.as_str(), "admin" | "owner");
                if !is_admin {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("Admins only")
                                .size(theme.font_size_heading)
                                .strong()
                                .color(theme.text_primary()),
                        );
                        ui.label(
                            RichText::new(
                                "Game moderation is limited to server admins and owners. You are \
                                 signed in without that role, so the ban controls are hidden. The \
                                 relay enforces this regardless of the client.",
                            )
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                        );
                    });
                    return;
                }

                draw_ban_form(ui, theme, state);
                ui.add_space(theme.spacing_md);
                draw_ban_list(ui, theme, state);

                if !state.game_admin_status.is_empty() {
                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        RichText::new(state.game_admin_status.clone())
                            .size(theme.font_size_small)
                            .color(theme.accent()),
                    );
                }
            });
        });
}

fn draw_header(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_lg);
    ui.vertical_centered(|ui| {
        if widgets::Button::secondary("< Back")
            .tooltip("Return to the previous page. Same as pressing Esc.")
            .show(ui, theme)
        {
            if !state.pop_nav_back() {
                state.active_page = GuiPage::Chat;
            }
            state.game_admin_status.clear();
        }
    });
    ui.add_space(theme.spacing_md);
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.label(
            RichText::new("GAME ADMIN")
                .size(theme.font_size_small)
                .color(theme.accent())
                .strong(),
        );
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new("Game-world moderation")
                .size(theme.font_size_title)
                .color(theme.text_primary())
                .strong(),
        );
    });
}

/// The free-speech guarantee, stated plainly + prominently. This is the whole
/// reason the page exists as a separate surface.
fn draw_disclaimer(ui: &mut egui::Ui, theme: &Theme) {
    widgets::card(ui, theme, |ui| {
        ui.label(
            RichText::new("Game bans do NOT affect chat")
                .size(theme.font_size_heading)
                .strong()
                .color(theme.text_primary()),
        );
        ui.label(
            RichText::new(
                "A game ban blocks a player from the shared 3D world only. Chat is a right: a \
                 game-banned user keeps full access to every channel and every direct message. \
                 Playing on the world is a privilege, and only that privilege is revoked here. \
                 To moderate chat, use Server Settings instead -- it is a separate system.",
            )
            .size(theme.font_size_small)
            .color(theme.text_secondary()),
        );
    });
}

fn draw_ban_form(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, "Ban a player from the game");
    widgets::body_hint(
        ui, theme,
        "Enter the player's public key (their identity). The ban takes effect immediately: if \
         they are in the world they are removed, and their next join is refused. Their chat is \
         untouched.",
    );
    ui.add_space(theme.spacing_sm);

    widgets::form_row(ui, theme, "Public key", |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.game_admin_target_key)
                .desired_width(360.0)
                .hint_text("player public key (hex)"),
        );
    });
    widgets::form_row(ui, theme, "Reason", |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.game_admin_ban_reason)
                .desired_width(360.0)
                .hint_text("why (shown to admins; optional)"),
        );
    });
    ui.add_space(theme.spacing_sm);

    let target_valid = !state.game_admin_target_key.trim().is_empty();
    ui.add_enabled_ui(target_valid, |ui| {
        if widgets::Button::danger("Game-ban player")
            .tooltip("Block this player from the 3D world. Does NOT affect their chat access.")
            .show(ui, theme)
        {
            let target = state.game_admin_target_key.trim().to_string();
            let reason = state.game_admin_ban_reason.trim().to_string();
            send_game_ban(state, &target, &reason);
            state.game_admin_status = format!("Sent a game ban for {target}. Chat is unaffected.");
            state.game_admin_target_key.clear();
            state.game_admin_ban_reason.clear();
        }
    });
}

fn draw_ban_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, "Game-banned players");

    // Auto-request once per session so the list isn't empty on first open.
    // game_bans_requested is reset on disconnect (lib.rs).
    if !state.game_bans_requested {
        send_game_banned_list_request(state);
        state.game_bans_requested = true;
    }

    ui.horizontal(|ui| {
        if widgets::Button::secondary("Refresh")
            .tooltip("Re-fetch the game-ban list from the server.")
            .show(ui, theme)
        {
            send_game_banned_list_request(state);
            state.game_admin_status = "Requested the latest game-ban list.".into();
        }
        ui.add_space(theme.spacing_sm);
        ui.colored_label(
            theme.text_muted(),
            format!("{} game-banned", state.game_bans.len()),
        );
    });
    ui.add_space(theme.spacing_sm);

    if state.game_bans.is_empty() {
        widgets::body_hint(ui, theme, "No one is game-banned. A clean slate.");
        return;
    }

    let bans = state.game_bans.clone();
    let mut unban_key: Option<String> = None;
    for b in &bans {
        widgets::card(ui, theme, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // Short key.
                let short_key = if b.public_key.len() > 20 {
                    format!("{}...", &b.public_key[..20])
                } else {
                    b.public_key.clone()
                };
                ui.add_sized(
                    [200.0, 22.0],
                    egui::Label::new(
                        RichText::new(short_key)
                            .color(theme.text_primary())
                            .size(theme.body_size * 0.95)
                            .monospace(),
                    ),
                );
                // Reason (or a dash).
                let reason = if b.reason.trim().is_empty() {
                    "(no reason given)".to_string()
                } else {
                    b.reason.clone()
                };
                ui.add_sized(
                    [220.0, 22.0],
                    egui::Label::new(
                        RichText::new(reason)
                            .color(theme.text_secondary())
                            .size(theme.body_size * 0.9),
                    )
                    .truncate(),
                );
                // Banned-at date.
                ui.add_sized(
                    [150.0, 22.0],
                    egui::Label::new(
                        RichText::new(format_ban_date(b.banned_at))
                            .color(theme.text_muted())
                            .size(theme.body_size * 0.9),
                    ),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::Button::secondary("Unban")
                        .tooltip("Restore this player's access to the game world.")
                        .show(ui, theme)
                    {
                        unban_key = Some(b.public_key.clone());
                    }
                });
            });
        });
        ui.add_space(2.0);
    }

    if let Some(key) = unban_key {
        send_game_unban(state, &key);
        state.game_admin_status = "Sent a game unban; the list will refresh.".into();
    }
}

/// Current user's role from the chat user list (defense-in-depth; the relay is
/// authoritative). Mirrors server_settings::current_user_role.
fn current_game_admin_role(state: &GuiState) -> String {
    state
        .chat_users
        .iter()
        .find(|u| u.public_key == state.profile_public_key)
        .map(|u| u.role.clone())
        .unwrap_or_default()
}

/// Send a `game_ban` (admin-gated server-side; relay replies privately).
fn send_game_ban(state: &GuiState, target: &str, reason: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "game_ban", "target": target, "reason": reason });
            client.send(&msg.to_string());
        }
    }
}

/// Send a `game_unban` (admin-gated server-side).
fn send_game_unban(state: &GuiState, target: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "game_unban", "target": target });
            client.send(&msg.to_string());
        }
    }
}

/// Request the game-ban list (admin-gated; relay replies privately).
fn send_game_banned_list_request(state: &GuiState) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "game_banned_list_request" });
            client.send(&msg.to_string());
        }
    }
}

/// Format a Unix-ms timestamp as `YYYY-MM-DD HH:MM` (UTC), chrono-free
/// (same Howard Hinnant civil-date math as server_settings::format_ban_date).
fn format_ban_date(ms: i64) -> String {
    if ms <= 0 {
        return "unknown".to_string();
    }
    let secs = ms / 1000;
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (hh, mm) = (tod / 3600, (tod % 3600) / 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02} {hh:02}:{mm:02}")
}

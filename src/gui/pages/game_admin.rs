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

use egui::{RichText, Layout, Align};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Draw the game-world bans admin controls as a SUBSECTION of Server Settings >
/// ADMIN (v0.479: folded in from a dedicated page so the nav has one fewer
/// button -- operator's fold-don't-proliferate preference). It stays a clearly
/// DISTINCT subsection with the free-speech disclaimer, so game bans remain
/// obviously separate from chat moderation. The caller (the ADMIN tinted_section)
/// is already admin-gated, like the sibling "Banned users" (chat) panel.
pub fn draw_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Game world bans");
    draw_disclaimer(ui, theme);
    ui.add_space(theme.spacing_sm);
    draw_ban_form(ui, theme, state);
    ui.add_space(theme.spacing_md);
    draw_ban_list(ui, theme, state);
    ui.add_space(theme.spacing_md);
    draw_plot_release(ui, theme, state);
    ui.add_space(theme.spacing_md);
    // How fast the shared world's clock runs (2026-10-04, real time by default).
    super::world_clock_admin::draw(ui, theme, state);
    ui.add_space(theme.spacing_md);
    // Whether the fleet's stores can run out, and the fleet's totals (2026-10-04).
    super::fleet_ledger::draw_admin(ui, theme, state);
    if !state.game_admin_status.is_empty() {
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new(state.game_admin_status.clone())
                .size(theme.font_size_small)
                .color(theme.accent()),
        );
    }
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

/// Homes on the ship (ship homes increment 1b, docs/design/ship-homes-and-logistics.md): give
/// back a plot, so the next player who joins without one gets it. The in-app control for the
/// relay's plot table (GUI-first: nobody should need a shell for it). The admin names the
/// plot's id ("p1") or the public key of the player who holds it: a plot id still works when
/// the holder's key is gone from every list (an erased account; erasing an account gives its
/// plot back by itself now, but an id is the one name an admin always has). The relay refuses
/// while the holder is in the world (their home stands on the plot) and says whether anyone
/// held it; its reply lands in the status line below. Nothing releases an idle plot by itself:
/// when one should go back is the operator's call (the design's open questions).
fn draw_plot_release(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, "Homes on the ship");
    widgets::body_hint(
        ui, theme,
        "Each player who joins holds one plot of the ship for their home, and keeps it when they \
         leave. To give a plot back (someone left for good, and the ship is full), enter the \
         plot's id (p1, p2, ...) or the public key of the player who holds it. It works only \
         while they are out of the world; their own home and saves are untouched, and they get a \
         free plot, or a guest place, when they come back.",
    );
    ui.add_space(theme.spacing_sm);
    widgets::form_row(ui, theme, "Plot or key", |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.game_admin_plot_key)
                .desired_width(360.0)
                .hint_text("plot id (p1) or player public key (hex)"),
        );
    });
    ui.add_space(theme.spacing_sm);
    let target_valid = !state.game_admin_plot_key.trim().is_empty();
    ui.add_enabled_ui(target_valid, |ui| {
        if widgets::Button::secondary("Release plot")
            .tooltip("Give this plot back for the next player who joins without one.")
            .show(ui, theme)
        {
            let target = state.game_admin_plot_key.trim().to_string();
            // Only a message that went out is reported as asked, and only then is the field
            // cleared (the third review: with the link down this said "Asked the server" and
            // threw the key away, while the web window said "Not connected").
            if send_release_plot(state, &target) {
                state.game_admin_status = "Asked the server to release that plot.".into();
                state.game_admin_plot_key.clear();
            } else {
                state.game_admin_status = "Not connected to the server.".into();
            }
        }
    });
}

/// Send a `game_release_plot` (admin-gated server-side; the relay replies privately with a
/// `game_admin_notice` or a `game_admin_error`). False when there is no open connection to
/// send it on, so the caller says so instead of claiming it asked.
fn send_release_plot(state: &GuiState, target: &str) -> bool {
    match state.ws_client.as_ref() {
        Some(client) if client.is_connected() => {
            let msg = serde_json::json!({ "type": "game_release_plot", "target": target });
            client.send(&msg.to_string());
            true
        }
        _ => false,
    }
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
pub(crate) fn format_ban_date(ms: i64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;

    /// One headless frame of the plot-release form in a plain panel, with `events`.
    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 600.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_plot_release(ui, theme, state));
        })
    }

    /// Release plot with the server link down, clicked the way an admin clicks it: the status
    /// says the server was NOT asked and the plot id stays in the field, as the web Game Admin
    /// window does (the third review of ship homes 1b: the native form said "Asked the server
    /// to release that player's plot." and emptied the field, while nothing was sent).
    ///
    /// Seen red 2026-10-03 with the status set whether or not anything was sent (the a504c5cd9
    /// form's way): "with no connection the form says: Asked the server to release that plot.".
    #[test]
    fn release_plot_with_no_connection_says_so_and_keeps_what_was_typed() {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = GuiState::default();
        assert!(state.ws_client.is_none());
        state.game_admin_plot_key = "p1".into();
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        let pos = find_text_in_shapes(&out.shapes, "Release plot").expect("the button is drawn").rect.center();
        let m = egui::Modifiers::default();
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(pos)]);
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
        assert_eq!(state.game_admin_status, "Not connected to the server.", "with no connection the form says: {}", state.game_admin_status);
        assert_eq!(state.game_admin_plot_key, "p1", "and keeps what was typed");
    }
}

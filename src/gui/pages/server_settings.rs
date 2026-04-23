//! Server and group settings page.
//!
//! Reached from the cog menu on the server row or on a group row in the
//! chat sidebar. For now this is a minimal skeleton that lets admins see
//! basic metadata and perform common actions (copy invite, leave/disconnect,
//! edit name, edit description). Fuller settings (role management, channel
//! permissions, moderation, federation config) land here incrementally.

use egui::{Align, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};

use crate::gui::theme::Theme;
use crate::gui::{GuiPage, GuiState};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    draw_header(ui, theme, state);
                    ui.add_space(theme.spacing_xl);
                    draw_general_section(ui, theme, state);
                    ui.add_space(theme.spacing_xl);
                    draw_invite_section(ui, theme, state);
                    ui.add_space(theme.spacing_xl);
                    draw_danger_section(ui, theme, state);
                    ui.add_space(theme.spacing_xl);
                });
        });
}

fn draw_header(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_lg);
    ui.horizontal(|ui| {
        ui.add_space(theme.spacing_lg);
        let back = egui::Button::new(
            RichText::new("< Back to Chat")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, theme.border()))
        .rounding(Rounding::same(theme.border_radius as u8));
        if ui.add(back).clicked() {
            state.active_page = GuiPage::Chat;
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

fn draw_general_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    section_card(ui, theme, "General", |ui, theme| {
        kv_row(ui, theme, "Connected server", resolve_server_url(state));
        ui.add_space(theme.spacing_sm);
        kv_row(ui, theme, "Your identity", short_key(&state.profile_public_key));
        ui.add_space(theme.spacing_sm);
        kv_row(ui, theme, "Network status", state.ws_status.clone());
    });
}

fn draw_invite_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    section_card(ui, theme, "Invite", |ui, theme| {
        let (label, invite_url) = match resolve_group(state) {
            Some((id, _name)) => (
                "Group invite link",
                format!("https://united-humanity.us/chat/group/{}", id),
            ),
            None => ("Server invite link", "https://united-humanity.us/chat".to_string()),
        };
        kv_row(ui, theme, label, invite_url.clone());
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            let btn = egui::Button::new(
                RichText::new("Copy invite")
                    .color(theme.text_on_accent())
                    .size(theme.font_size_body),
            )
            .fill(theme.accent())
            .rounding(Rounding::same(theme.border_radius as u8));
            if ui.add(btn).clicked() {
                ui.ctx().copy_text(invite_url);
            }
        });
    });
}

fn draw_danger_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    section_card(ui, theme, "Danger zone", |ui, theme| {
        ui.label(
            RichText::new("Irreversible actions. Double-check before clicking.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_md);
        let (label, group_id) = match resolve_group(state) {
            Some((id, _name)) => ("Leave group", Some(id)),
            None => ("Disconnect from server", None),
        };
        let btn = egui::Button::new(
            RichText::new(label)
                .size(theme.font_size_body)
                .color(egui::Color32::WHITE),
        )
        .fill(theme.danger())
        .rounding(Rounding::same(theme.border_radius as u8));
        if ui.add(btn).clicked() {
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
    });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn section_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    contents: impl FnOnce(&mut egui::Ui, &Theme),
) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(720.0);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.label(
                RichText::new(title.to_uppercase())
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .strong(),
            );
            ui.add_space(theme.spacing_sm);
            Frame::none()
                .fill(theme.bg_card())
                .stroke(Stroke::new(1.0, theme.border()))
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
    // The group cog menu sets `chat_user_modal_key` to the group id as a
    // piggy-backed context. If that id matches one of our groups, use it;
    // otherwise fall through to server settings.
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

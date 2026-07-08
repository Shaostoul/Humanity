//! Guilds page - live guild browser against the connected relay's REST guild
//! API (v0.757, closure ladder rung 10). List/search all guilds, join and
//! leave for real, create with your signed-in identity as owner, member list
//! per guild. Replaced the page-local mock where join/create only edited GUI
//! memory. Worker-thread + mpsc for every request (the Server Settings fetch
//! pattern); the relay's response is the only truth.

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiGuild, GuiState};
use egui::{Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use std::collections::HashSet;

/// Fetch every guild plus my memberships, merged into one list.
fn spawn_guilds_fetch(state: &mut GuiState) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let my_key = state.profile_public_key.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    state.guilds_rx = Some(rx);
    state.guilds_loaded = true; // set now so a slow fetch is not re-spawned
    std::thread::spawn(move || {
        let fetch = || -> Result<Vec<GuiGuild>, String> {
            let get_list = |url: &str| -> Result<Vec<serde_json::Value>, String> {
                let body = ureq::get(url)
                    .call()
                    .map_err(|e| format!("guilds: {e}"))?
                    .into_string()
                    .map_err(|e| format!("read: {e}"))?;
                let val: serde_json::Value =
                    serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
                Ok(val.as_array().cloned().unwrap_or_default())
            };
            let mut rows: Vec<GuiGuild> = get_list(&format!("{base}/api/guilds?limit=200"))?
                .iter()
                .map(GuiGuild::from_relay_json)
                .collect();
            if !my_key.is_empty() {
                let mine = get_list(&format!("{base}/api/guilds?user={my_key}"))?;
                let mine_ids: HashSet<String> = mine
                    .iter()
                    .filter_map(|g| g.get("id").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .collect();
                for g in &mut rows {
                    g.is_member = mine_ids.contains(&g.id);
                }
                // A guild of mine beyond the browse limit still shows.
                for m in &mine {
                    let mut g = GuiGuild::from_relay_json(m);
                    if !rows.iter().any(|r| r.id == g.id) {
                        g.is_member = true;
                        rows.push(g);
                    }
                }
            }
            rows.sort_by(|a, b| (!a.is_member, a.name.clone()).cmp(&(!b.is_member, b.name.clone())));
            Ok(rows)
        };
        let _ = tx.send(fetch());
    });
}

/// Fetch a guild's member list: (display name or key prefix, role).
fn spawn_members_fetch(state: &mut GuiState, guild_id: &str) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let id = guild_id.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    state.guild_members_rx = Some(rx);
    state.guild_members_for = guild_id.to_string();
    state.guild_members.clear();
    std::thread::spawn(move || {
        let fetch = || -> Result<Vec<(String, String)>, String> {
            let body = ureq::get(&format!("{base}/api/guilds/{id}/members?limit=200"))
                .call()
                .map_err(|e| format!("members: {e}"))?
                .into_string()
                .map_err(|e| format!("read: {e}"))?;
            let val: serde_json::Value =
                serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
            Ok(val
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|m| {
                    let key = m.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
                    let name = m
                        .get("name")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            if key.len() > 10 { format!("{}...", &key[..10]) } else { key.to_string() }
                        });
                    let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("member").to_string();
                    (name, role)
                })
                .collect())
        };
        let _ = tx.send(fetch());
    });
}

/// Fire a guild action (join/leave/create/delete) in a worker; Ok refetches.
fn spawn_guild_action(
    state: &mut GuiState,
    method: &'static str,
    url: String,
    body: Option<serde_json::Value>,
    ok_msg: &'static str,
) {
    let (tx, rx) = std::sync::mpsc::channel();
    state.guild_action_rx = Some(rx);
    state.guild_status = "Working...".to_string();
    std::thread::spawn(move || {
        let run = || -> Result<String, String> {
            let req = match method {
                "DELETE" => ureq::delete(&url),
                _ => ureq::post(&url),
            };
            let resp = match body {
                Some(b) => req
                    .set("Content-Type", "application/json")
                    .send_string(&b.to_string()),
                None => req.call(),
            };
            match resp {
                Ok(_) => Ok(ok_msg.to_string()),
                Err(ureq::Error::Status(_code, r)) => {
                    Err(r.into_string().unwrap_or_else(|_| "Request refused.".to_string()))
                }
                Err(e) => Err(format!("{e}")),
            }
        };
        let _ = tx.send(run());
    });
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Drain finished workers.
    if let Some(rx) = &state.guilds_rx {
        match rx.try_recv() {
            Ok(Ok(rows)) => {
                state.guilds = rows;
                state.guilds_rx = None;
            }
            Ok(Err(e)) => {
                state.guild_status = e;
                state.guilds_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.guilds_rx = None;
            }
        }
    }
    if let Some(rx) = &state.guild_members_rx {
        match rx.try_recv() {
            Ok(Ok(rows)) => {
                state.guild_members = rows;
                state.guild_members_rx = None;
            }
            Ok(Err(_)) => {
                state.guild_members_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.guild_members_rx = None;
            }
        }
    }
    if let Some(rx) = &state.guild_action_rx {
        match rx.try_recv() {
            Ok(Ok(msg)) => {
                state.guild_status = msg;
                state.guild_action_rx = None;
                // Refetch the list + open member panel so the UI shows what
                // the relay now holds.
                state.guilds_loaded = false;
                state.guild_members_for.clear();
            }
            Ok(Err(e)) => {
                state.guild_status = e;
                state.guild_action_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.guild_action_rx = None;
            }
        }
    }
    // First view (or post-action): pull the live list.
    if !state.guilds_loaded && state.guilds_rx.is_none() {
        spawn_guilds_fetch(state);
    }

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Guilds")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, theme, "+ Create Guild") {
                        state.guild_show_create = true;
                        state.guild_selected = None;
                    }
                    if widgets::secondary_button(ui, theme, "Refresh") {
                        state.guilds_loaded = false;
                    }
                });
            });
            if !state.guild_status.is_empty() {
                ui.label(
                    RichText::new(&state.guild_status)
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                );
            }

            ui.add_space(theme.spacing_sm);
            widgets::search_bar(ui, theme, &mut state.guild_search, "Search guilds by name...");
            ui.add_space(theme.spacing_md);

            if state.guild_show_create {
                draw_create_form(ui, theme, state);
            } else if state.guild_selected.is_some() {
                draw_guild_detail(ui, theme, state);
            } else {
                draw_guild_grid(ui, theme, state);
            }
        });
}

fn draw_guild_grid(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let search = state.guild_search.to_lowercase();
    let rows: Vec<GuiGuild> = state
        .guilds
        .iter()
        .filter(|g| search.is_empty() || g.name.to_lowercase().contains(&search))
        .cloned()
        .collect();

    if rows.is_empty() {
        ui.add_space(theme.spacing_xl);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("No guilds found")
                    .size(theme.font_size_heading)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);
            let hint = if state.guilds_rx.is_some() {
                "Loading from the server..."
            } else {
                "Create one to get started!"
            };
            ui.label(RichText::new(hint).size(theme.font_size_body).color(theme.text_muted()));
        });
        return;
    }

    let my_key = state.profile_public_key.clone();
    let mut select_id: Option<String> = None;
    let mut join_id: Option<String> = None;

    ScrollArea::vertical().id_salt("guilds_grid").show(ui, |ui| {
        let card_width = 260.0;
        let available_width = ui.available_width();
        let cols = ((available_width / (card_width + theme.spacing_sm)).floor() as usize).max(1);
        let mut col = 0;

        egui::Grid::new("guilds_card_grid")
            .spacing(Vec2::new(theme.spacing_sm, theme.spacing_sm))
            .show(ui, |ui| {
                for guild in &rows {
                    let frame = egui::Frame::none()
                        .fill(theme.bg_card())
                        .rounding(Rounding::same(theme.border_radius as u8))
                        .stroke(Stroke::new(1.0, theme.border()))
                        .inner_margin(theme.card_padding);

                    frame.show(ui, |ui| {
                        ui.set_min_width(card_width - theme.card_padding * 2.0);
                        ui.set_max_width(card_width - theme.card_padding * 2.0);

                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let (dot_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(12.0, 12.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(dot_rect.center(), 6.0, guild.color);
                                ui.label(
                                    RichText::new(&guild.name)
                                        .size(theme.font_size_heading)
                                        .color(theme.text_primary()),
                                );
                            });
                            ui.label(
                                RichText::new(format!("{} members", guild.member_count))
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            ui.add_space(theme.spacing_xs);
                            let preview: String = guild.description.chars().take(80).collect();
                            let preview = if guild.description.chars().count() > 80 {
                                format!("{}...", preview)
                            } else {
                                preview
                            };
                            ui.label(
                                RichText::new(preview)
                                    .size(theme.font_size_small)
                                    .color(theme.text_secondary()),
                            );
                            ui.add_space(theme.spacing_sm);
                            ui.horizontal(|ui| {
                                if widgets::secondary_button(ui, theme, "View") {
                                    select_id = Some(guild.id.clone());
                                }
                                if !guild.is_member
                                    && !my_key.is_empty()
                                    && widgets::primary_button(ui, theme, "Join")
                                {
                                    join_id = Some(guild.id.clone());
                                }
                            });
                        });
                    });

                    col += 1;
                    if col >= cols {
                        ui.end_row();
                        col = 0;
                    }
                }
            });
    });

    if let Some(id) = select_id {
        state.guild_selected = Some(id);
    }
    if let Some(id) = join_id {
        let base = state.server_url.trim_end_matches('/').to_string();
        spawn_guild_action(
            state,
            "POST",
            format!("{base}/api/guilds/{id}/members"),
            Some(serde_json::json!({"public_key": my_key})),
            "Joined the guild.",
        );
    }
}

fn draw_guild_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if widgets::secondary_button(ui, theme, "< Back to Guilds") {
        state.guild_selected = None;
        return;
    }
    ui.add_space(theme.spacing_sm);

    let sel_id = state.guild_selected.clone().unwrap_or_default();
    let Some(guild) = state.guilds.iter().find(|g| g.id == sel_id).cloned() else {
        state.guild_selected = None;
        return;
    };
    // Pull this guild's member list on first view.
    if state.guild_members_for != sel_id && state.guild_members_rx.is_none() {
        spawn_members_fetch(state, &sel_id);
    }
    let my_key = state.profile_public_key.clone();
    let base = state.server_url.trim_end_matches('/').to_string();

    let mut action: Option<(&'static str, String, Option<serde_json::Value>, &'static str)> = None;

    ScrollArea::vertical().id_salt("guild_detail").show(ui, |ui| {
        ui.horizontal(|ui| {
            let (dot_rect, _) =
                ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
            ui.painter().circle_filled(dot_rect.center(), 8.0, guild.color);
            ui.label(
                RichText::new(&guild.name)
                    .size(theme.font_size_title)
                    .color(theme.accent()),
            );
        });
        ui.add_space(theme.spacing_sm);

        widgets::card(ui, theme, |ui| {
            ui.label(
                RichText::new("About")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_xs);
            if guild.description.is_empty() {
                ui.label(RichText::new("No description provided").color(theme.text_muted()));
            } else {
                ui.label(RichText::new(&guild.description).color(theme.text_secondary()));
            }
        });

        ui.add_space(theme.spacing_md);

        widgets::card_with_header(
            ui,
            theme,
            &format!("Members ({})", guild.member_count),
            |ui| {
                if state.guild_members.is_empty() {
                    let hint = if state.guild_members_rx.is_some() {
                        "Loading..."
                    } else {
                        "No members listed."
                    };
                    ui.label(RichText::new(hint).color(theme.text_muted()));
                }
                ScrollArea::vertical()
                    .id_salt("guild_members_detail")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (name, role) in &state.guild_members {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(name).color(theme.text_primary()));
                                ui.label(
                                    RichText::new(role)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            });
                        }
                    });
            },
        );

        ui.add_space(theme.spacing_sm);
        // Guild chat: the Chat page's P2P groups are the real conversation
        // surface today; a guild-scoped channel is a follow-up, so say so
        // instead of shipping a fake echo box.
        ui.label(
            RichText::new("Guild chat arrives with guild-scoped channels; use a Chat page group meanwhile.")
                .color(theme.text_muted())
                .size(theme.font_size_small),
        );

        ui.add_space(theme.spacing_md);

        if my_key.is_empty() {
            ui.label(
                RichText::new("Sign in (Chat page) to join guilds.")
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
            );
        } else if guild.owner_key == my_key {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("You own this guild.")
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                );
                if widgets::danger_button(ui, theme, "Delete Guild") {
                    action = Some((
                        "DELETE",
                        format!("{base}/api/guilds/{}?owner_key={my_key}", guild.id),
                        None,
                        "Guild deleted.",
                    ));
                }
            });
        } else if guild.is_member {
            if widgets::danger_button(ui, theme, "Leave Guild") {
                action = Some((
                    "POST",
                    format!("{base}/api/guilds/{}/leave", guild.id),
                    Some(serde_json::json!({"public_key": my_key})),
                    "Left the guild.",
                ));
            }
        } else if widgets::primary_button(ui, theme, "Join Guild") {
            action = Some((
                "POST",
                format!("{base}/api/guilds/{}/members", guild.id),
                Some(serde_json::json!({"public_key": my_key})),
                "Joined the guild.",
            ));
        }
    });

    if let Some((method, url, body, ok_msg)) = action {
        spawn_guild_action(state, method, url, body, ok_msg);
    }
}

fn draw_create_form(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if widgets::secondary_button(ui, theme, "< Back to Guilds") {
        state.guild_show_create = false;
        return;
    }
    ui.add_space(theme.spacing_md);
    ui.label(
        RichText::new("Create Guild")
            .size(theme.font_size_heading)
            .color(theme.accent()),
    );
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, "Name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.guild_new_name)
                    .desired_width(280.0)
                    .hint_text("Guild name"),
            );
        });
        widgets::form_row(ui, theme, "Description", |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut state.guild_new_desc)
                    .desired_width(280.0)
                    .desired_rows(4)
                    .hint_text("What is this guild about?"),
            );
        });
        widgets::form_row(ui, theme, "Color", |ui| {
            ui.color_edit_button_srgba(&mut state.guild_new_color);
        });
    });

    ui.add_space(theme.spacing_md);

    let my_key = state.profile_public_key.clone();
    if my_key.is_empty() {
        ui.label(
            RichText::new("Sign in (Chat page) to create a guild - the owner is your identity key.")
                .color(theme.text_muted())
                .size(theme.font_size_small),
        );
    }
    ui.horizontal(|ui| {
        let can_create = !state.guild_new_name.trim().is_empty() && !my_key.is_empty();
        ui.add_enabled_ui(can_create, |ui| {
            if widgets::primary_button(ui, theme, "Create Guild") {
                let c = state.guild_new_color;
                let base = state.server_url.trim_end_matches('/').to_string();
                let body = serde_json::json!({
                    "name": state.guild_new_name.trim(),
                    "description": state.guild_new_desc.trim(),
                    "icon": "",
                    "color": format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()),
                    "owner_key": my_key,
                });
                state.guild_new_name.clear();
                state.guild_new_desc.clear();
                state.guild_show_create = false;
                spawn_guild_action(
                    state,
                    "POST",
                    format!("{base}/api/guilds"),
                    Some(body),
                    "Guild created.",
                );
            }
        });
        if widgets::secondary_button(ui, theme, "Cancel") {
            state.guild_show_create = false;
        }
    });
}

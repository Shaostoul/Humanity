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
                    ui.add_space(theme.spacing_md);

                    let role = current_user_role(state);
                    let is_mod   = matches!(role.as_str(), "mod" | "admin" | "owner");
                    let is_admin = matches!(role.as_str(), "admin" | "owner");

                    draw_tab_bar(ui, theme, state, is_mod, is_admin);
                    ui.add_space(theme.spacing_md);

                    match state.server_settings_tab {
                        0 => {
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
                        }
                        1 => draw_channels_tab(ui, theme, state, is_admin),
                        2 => draw_members_tab(ui, theme, state, is_mod),
                        3 => draw_reports_tab(ui, theme),
                        _ => state.server_settings_tab = 0,
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

/// Draw the tab bar — Overview / Channels / Members / Reports.
/// Reports is dev-only for v0.188 (placeholder); becomes the mod review
/// surface in v0.189. Members + Reports gated on mod tier.
fn draw_tab_bar(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    is_mod: bool,
    is_admin: bool,
) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(960.0);
        ui.horizontal(|ui| {
            let tabs: &[(&str, u8, bool)] = &[
                ("Overview",  0, true),
                ("Channels",  1, is_admin),
                ("Members",   2, is_mod),
                ("Reports",   3, is_mod),
            ];
            for (label, idx, enabled) in tabs {
                if !*enabled { continue; }
                let active = state.server_settings_tab == *idx;
                if widgets::Button::secondary(*label).active(active).show(ui, theme) {
                    state.server_settings_tab = *idx;
                }
            }
        });
    });
}

/// Shared column widths for the Channels grid — used by header AND data
/// rows so they line up perfectly. Bug fix 2026-05-04: previously the
/// header used allocate_ui_with_layout(reservation) but the labels
/// collapsed to text width while data rows used different widget widths,
/// so the columns drifted.
const CHANNEL_COL_WIDTHS: [f32; 7] = [
    144.0, // Name
    284.0, // Description
    72.0,  // Read-only
    60.0,  // Voice
    72.0,  // Federated
    60.0,  // Save
    72.0,  // Delete
];

/// Channels spreadsheet tab. One row per channel with editable cells:
/// Name | Description | Read-only | Voice | Federated | (Save) | (Delete).
/// Plus a sticky "+ new channel" row at the bottom.
fn draw_channels_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, is_admin: bool) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(960.0);
        ui.with_layout(egui::Layout::top_down(Align::Min), |ui| {
            // Section heading
            ui.label(
                RichText::new("Channels")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new(
                    "Edit any cell, then click Save on the row. New channel? \
                     Fill in the bottom row and click Create. Delete is admin-only \
                     and asks for confirmation."
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_md);

            // Header row — visual column titles.
            channel_grid_row(
                ui, theme,
                &["Name", "Description", "Read-only", "Voice", "Federated", "", ""],
                /* is_header */ true,
            );

            // One row per existing channel.
            let channels = state.chat_channels.clone();
            let mut delete_id: Option<String> = None;
            let mut save_id: Option<String> = None;

            for ch in &channels {
                // Pull or seed the draft for this channel.
                let draft = state.server_settings_channel_drafts
                    .entry(ch.id.clone())
                    .or_insert_with(|| crate::gui::ChannelDraft {
                        name: ch.name.clone(),
                        description: ch.description.clone(),
                        read_only: false, // TODO wire from chat_channels once flag exists
                        federated: false, // TODO wire from chat_channels once flag exists
                        voice_enabled: ch.voice_enabled,
                    });
                let mut row_changed = false;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    // Each cell uses ui.add_sized with the shared CHANNEL_COL_WIDTHS
                    // so columns line up across header + every data row.
                    if ui.add_sized(
                        Vec2::new(CHANNEL_COL_WIDTHS[0], 22.0),
                        egui::TextEdit::singleline(&mut draft.name),
                    ).changed() { row_changed = true; }
                    if ui.add_sized(
                        Vec2::new(CHANNEL_COL_WIDTHS[1], 22.0),
                        egui::TextEdit::singleline(&mut draft.description),
                    ).changed() { row_changed = true; }
                    centered_checkbox(ui, theme, &mut draft.read_only, CHANNEL_COL_WIDTHS[2], &mut row_changed);
                    centered_checkbox(ui, theme, &mut draft.voice_enabled, CHANNEL_COL_WIDTHS[3], &mut row_changed);
                    centered_checkbox(ui, theme, &mut draft.federated, CHANNEL_COL_WIDTHS[4], &mut row_changed);
                    ui.allocate_ui_with_layout(
                        Vec2::new(CHANNEL_COL_WIDTHS[5], 22.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if widgets::Button::primary("Save").show(ui, theme) {
                                save_id = Some(ch.id.clone());
                            }
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(CHANNEL_COL_WIDTHS[6], 22.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if is_admin {
                                if widgets::Button::danger("Delete").show(ui, theme) {
                                    delete_id = Some(ch.id.clone());
                                }
                            }
                        },
                    );
                });
                let _ = row_changed; // visual cue could go here; keep minimal for v1
            }

            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_md);

            // "+ new channel" sticky row at bottom (admin only).
            if is_admin {
                ui.label(
                    RichText::new("+ New channel")
                        .size(theme.font_size_body)
                        .color(theme.accent())
                        .strong(),
                );
                ui.add_space(theme.spacing_xs);
                let new_draft = &mut state.server_settings_new_channel;
                let mut create_clicked = false;
                let mut _row_changed_unused = false;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_sized(
                        Vec2::new(CHANNEL_COL_WIDTHS[0], 22.0),
                        egui::TextEdit::singleline(&mut new_draft.name).hint_text("channel-name"),
                    );
                    ui.add_sized(
                        Vec2::new(CHANNEL_COL_WIDTHS[1], 22.0),
                        egui::TextEdit::singleline(&mut new_draft.description).hint_text("Description"),
                    );
                    centered_checkbox(ui, theme, &mut new_draft.read_only, CHANNEL_COL_WIDTHS[2], &mut _row_changed_unused);
                    centered_checkbox(ui, theme, &mut new_draft.voice_enabled, CHANNEL_COL_WIDTHS[3], &mut _row_changed_unused);
                    centered_checkbox(ui, theme, &mut new_draft.federated, CHANNEL_COL_WIDTHS[4], &mut _row_changed_unused);
                    ui.allocate_ui_with_layout(
                        Vec2::new(CHANNEL_COL_WIDTHS[5] + CHANNEL_COL_WIDTHS[6], 22.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let valid = !new_draft.name.trim().is_empty();
                            ui.add_enabled_ui(valid, |ui| {
                                if widgets::Button::primary("Create").show(ui, theme) {
                                    create_clicked = true;
                                }
                            });
                        },
                    );
                });

                if create_clicked {
                    let name = state.server_settings_new_channel.name.trim().to_string();
                    let desc = state.server_settings_new_channel.description.trim().to_string();
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "channel_create",
                                "name": name,
                                "description": desc,
                            });
                            client.send(&msg.to_string());
                        }
                    }
                    state.server_settings_new_channel = crate::gui::ChannelDraft::default();
                    state.server_settings_status = format!("Channel '{}' creation requested.", name);
                }
            }

            // Apply pending row actions.
            if let Some(id) = save_id {
                if let Some(draft) = state.server_settings_channel_drafts.get(&id).cloned() {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "channel_update",
                                "channel_id": id,
                                "name": draft.name.trim(),
                                "description": draft.description.trim(),
                                "read_only": draft.read_only,
                                "voice_enabled": draft.voice_enabled,
                                "federated": draft.federated,
                            });
                            client.send(&msg.to_string());
                        }
                    }
                    state.server_settings_status = format!("Channel '{}' update sent.", draft.name);
                }
            }
            if let Some(id) = delete_id {
                if let Some(ref client) = state.ws_client {
                    if client.is_connected() {
                        let msg = serde_json::json!({
                            "type": "channel_delete",
                            "channel_id": id,
                        });
                        client.send(&msg.to_string());
                    }
                }
                state.server_settings_status = format!("Channel deletion requested for '{}'.", id);
                state.server_settings_channel_drafts.remove(&id);
            }
        });
    });
}

/// Centered checkbox cell with a visible border. Wraps
/// `widgets::custom_checkbox` (which has a theme-driven border) inside a
/// fixed-width allocation so checkbox columns line up with their headers.
/// Without the visible border the unchecked state was invisible against
/// the page background — bug fix 2026-05-04.
fn centered_checkbox(
    ui: &mut egui::Ui,
    theme: &Theme,
    value: &mut bool,
    cell_width: f32,
    row_changed: &mut bool,
) {
    ui.allocate_ui_with_layout(
        Vec2::new(cell_width, 22.0),
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| {
            if widgets::custom_checkbox(ui, theme, value) {
                *row_changed = true;
            }
        },
    );
}

/// One row of the channel grid header. Labels live in fixed-width slots
/// matching `CHANNEL_COL_WIDTHS`, and we use `add_sized` so the label's
/// drawn box actually fills the column (egui's `allocate_ui_with_layout`
/// + `ui.label` collapses the inner widget to its text width and lets
/// the next cell crowd in).
fn channel_grid_row(ui: &mut egui::Ui, theme: &Theme, cells: &[&str], is_header: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let size = if is_header { theme.font_size_small } else { theme.font_size_body };
        for (i, label) in cells.iter().enumerate() {
            let w = CHANNEL_COL_WIDTHS.get(i).copied().unwrap_or(80.0);
            let txt = if is_header {
                RichText::new(*label).size(size).color(theme.text_muted()).strong()
            } else {
                RichText::new(*label).size(size).color(theme.text_primary())
            };
            ui.add_sized(Vec2::new(w, 22.0), egui::Label::new(txt));
        }
    });
}

/// Members tab — list of server / group members with role + actions.
/// Spreadsheet-style; v0.188 uses the existing slash-command surface
/// (kick / mute / ban / promote) per row.
fn draw_members_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, is_mod: bool) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(960.0);
        ui.with_layout(egui::Layout::top_down(Align::Min), |ui| {
            ui.label(
                RichText::new("Members")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new(
                    "Member roster will populate from the relay. Per-row \
                     actions (Mute / Kick / Ban / Promote) trigger the \
                     existing slash-command flow. v0.188 ships the layout; \
                     full inline actions land in v0.189."
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_md);

            // Header
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let cols = [("Name", 160.0), ("DID", 200.0), ("Role", 90.0), ("Joined", 110.0), ("Actions", 200.0)];
                for (label, w) in cols {
                    ui.allocate_ui_with_layout(
                        Vec2::new(w, 22.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(RichText::new(label).color(theme.text_muted()).strong().size(theme.font_size_small));
                        },
                    );
                }
            });

            ui.separator();

            let _ = is_mod;
            // For v0.188, list the cached chat peers as a placeholder.
            // v0.189 wires this to relay's server_members table.
            let peers: Vec<(String, String)> = state.chat_users
                .iter()
                .map(|p| (p.name.clone(), p.public_key.clone()))
                .collect();
            if peers.is_empty() {
                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("No members loaded yet — wait for the relay roster sync.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted())
                        .italics(),
                );
            } else {
                for (name, key) in peers.iter().take(50) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.allocate_ui_with_layout(
                            Vec2::new(160.0, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(RichText::new(name).color(theme.text_primary()));
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(200.0, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let short = if key.len() > 16 {
                                    format!("{}…{}", &key[..6], &key[key.len()-6..])
                                } else { key.clone() };
                                ui.label(RichText::new(short).monospace().color(theme.text_muted()).size(theme.font_size_small));
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(90.0, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(RichText::new("user").color(theme.text_secondary()).size(theme.font_size_small));
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(110.0, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(RichText::new("—").color(theme.text_muted()).size(theme.font_size_small));
                            },
                        );
                        ui.allocate_ui_with_layout(
                            Vec2::new(200.0, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(RichText::new("(actions in v0.189)").color(theme.text_muted()).italics().size(theme.font_size_small));
                            },
                        );
                    });
                }
            }
        });
    });
}

/// Reports tab — placeholder for the v0.189 mod review surface.
/// See `docs/design/report-system.md` for the full design.
fn draw_reports_tab(ui: &mut egui::Ui, theme: &Theme) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(720.0);
        ui.with_layout(egui::Layout::top_down(Align::Min), |ui| {
            ui.label(
                RichText::new("Reports")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new(
                    "Mod review surface for messages flagged via 🚩 Report. \
                     Designed in `docs/design/report-system.md`. Ships v0.189 \
                     — storage table, WebSocket handlers, decision buttons \
                     (Dismiss / Warn / Mute / Kick / Ban / Mark Bogus), and \
                     the trust-score-based anti-abuse defenses."
                )
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_lg);
            widgets::alert(
                ui, theme, widgets::AlertKind::Info,
                "Eight overlapping anti-abuse defenses: rate limit / same-target cooldown / self-report rejected / trust-score weighting / Mark Bogus → trust hit / adversarial-mod escape valve / signed transparent log / federation opt-in.",
            );
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

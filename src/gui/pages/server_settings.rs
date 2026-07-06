//! Server and group settings page.
//!
//! Reached from the cog menu on the server row or on a group row in the
//! chat sidebar. Two tabs:
//!
//! - **Overview** — three role-tiered sections color-coded the same way
//!   the nav bar groups pages:
//!     - USER (red) — visible to everyone: identity info, profile shortcut,
//!       notification preferences, invite, disconnect.
//!     - MODERATOR (green) — visible to mods + admins: target user actions
//!       (mute/unmute/kick), channel moderation (pin), reports review.
//!     - ADMIN (blue) — visible to admins only: registration / invites,
//!       full channel spreadsheet (create / edit / delete inline), user
//!       management (verify / promote / ban).
//! - **Members** — server / group roster with role + actions.
//!
//! Channels editor + reports surface used to be their own tabs. Operator
//! 2026-05-08: merged into Overview so admins / mods don't have to tab-hop.
//! Channels live in the Admin section. Reports live in the Mod section.
//!
//! Action buttons send WebSocket messages (typed `channel_update`,
//! `channel_delete`, etc.) where supported, otherwise slash commands
//! through the chat channel — the relay's slash-command processor
//! (`/kick`, `/ban`, `/lockdown`, etc.) does the actual server-side work.

use egui::{Align, Color32, Layout, RichText, ScrollArea, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

/// Section identity colors — match the nav bar grouping in escape_menu.rs.
/// theme-exempt: these encode the privilege tier (red/green/blue) and are
/// referenced by both `widgets::tinted_section` calls AND the design
/// language documentation. Editing means a tier semantic change, not a
/// theme change. (Same convention as nav category colors.)
const USER_COLOR:  Color32 = Color32::from_rgb(231, 76, 60);   // RED — identity
const MOD_COLOR:   Color32 = Color32::from_rgb(46, 204, 113);  // GREEN — contextual
const ADMIN_COLOR: Color32 = Color32::from_rgb(52, 152, 219);  // BLUE — system

/// Shared cell height for the channel spreadsheet — used by EVERY cell
/// (text edits, checkboxes, Save/Delete/Create buttons) so the row reads
/// as one consistent line. Operator bug 2026-05-08: Save/Delete buttons
/// were rendering at theme.button_height (36) while text edits sat at 22,
/// making the row visually ragged. Picking 26 gives a tight spreadsheet
/// feel without buttons looking cramped.
const CHANNEL_ROW_H: f32 = 26.0;

/// Shared max width for every tinted section on this page. Picked to
/// fit the channel grid (~788 px including spacing) plus padding so
/// the Admin section doesn't bulge out wider than User and Mod.
/// Operator bug 2026-05-08: width mismatch read as a layout regression.
/// Using one constant guarantees all sections render at IDENTICAL width,
/// matching the tab bar and members tab too.
const SECTION_MAX_WIDTH: f32 = 960.0;

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

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    draw_header(ui, theme, state);
                    ui.add_space(theme.spacing_md);

                    let role = current_user_role(state);
                    let is_mod   = matches!(role.as_str(), "mod" | "admin" | "owner");
                    let is_admin = matches!(role.as_str(), "admin" | "owner");

                    draw_tab_bar(ui, theme, state, is_mod);
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
                        1 => draw_members_tab(ui, theme, state, is_mod),
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

/// Tab bar — Overview + Members. Channels and Reports merged into
/// Overview (admin and mod sections respectively) per operator 2026-05-08
/// to reduce tab-hopping.
fn draw_tab_bar(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    is_mod: bool,
) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(960.0);
        ui.horizontal(|ui| {
            let tabs: &[(&str, u8, bool)] = &[
                ("Overview", 0, true),
                ("Members",  1, is_mod),
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

// ── Header ──────────────────────────────────────────────────────────────────

fn draw_header(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_lg);
    // Back button — centered at the top of the page so users always
    // know where to find it. Operator 2026-05-08: "have the back to chat
    // button at the top middle so the go back button is always in a
    // predictable place." Same UX pattern as Esc, just clickable.
    ui.vertical_centered(|ui| {
        if widgets::Button::secondary("< Back")
            .tooltip("Return to the previous page (or Chat if you opened settings directly). \
                      Same as pressing Esc. Any unsaved row drafts in the channels editor \
                      are preserved if you come back.")
            .show(ui, theme)
        {
            // Pop the nav stack if we have one — that's the "previous
            // page" the user expects. Otherwise fall back to Chat as
            // the canonical home for this page.
            if !state.pop_nav_back() {
                state.active_page = GuiPage::Chat;
            }
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

// ── User Section ────────────────────────────────────────────────────────────

fn draw_user_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, role: &str) {
    widgets::tinted_section(ui, theme, "USER", USER_COLOR, SECTION_MAX_WIDTH, |ui, theme| {
        widgets::body_hint(
            ui, theme,
            "What you see no matter your role. Your connection details, profile shortcuts, \
             a copyable invite link, and the disconnect button.",
        );
        widgets::body_hint(
            ui, theme,
            "Tip: hold Alt and hover any underlined word for its definition (Ed25519, \
             federation, peer-to-peer, etc.).",
        );
        ui.add_space(theme.spacing_sm);

        kv_row(ui, theme, "Connected server", resolve_server_url(state));
        // Identity is your Dilithium3 (ML-DSA-65) public key -- the post-quantum
        // chat identity since the full-PQ cutover (v0.262+). The old "ed25519"
        // label here was crypto doc-drift: the VALUE shown has been the Dilithium
        // hex since Inc3 (fixed 2026-07-04, snapshot sweep tail).
        kv_row_with_definition(ui, theme, "Your identity", "dilithium3", short_key(&state.profile_public_key));
        kv_row(ui, theme, "Network status", state.ws_status.clone());
        kv_row(ui, theme, "Your role", role_label(role));

        ui.add_space(theme.spacing_md);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Open Profile")
                .tooltip("Edit your display name, avatar, bio, and pronouns. Your profile is \
                          signed and replicates across federated servers.")
                .show(ui, theme)
            {
                // push_nav_to so Esc returns to ServerSettings.
                state.push_nav_to(GuiPage::Profile);
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Notification preferences")
                .tooltip("Choose which events ping you (DMs, mentions, task assignments) \
                          and set quiet hours. Stored locally per device.")
                .show(ui, theme)
            {
                state.push_nav_to(GuiPage::Settings);
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
        if widgets::Button::primary("Copy invite")
            .tooltip("Copy the public invite link to your clipboard. Anyone with the link \
                      can join. For an invite-only server, ask an admin to generate a one-time invite code.")
            .show(ui, theme)
        {
            ui.ctx().copy_text(invite_url);
            state.server_settings_status = "Invite link copied to clipboard.".into();
        }

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Device & friend codes (v0.722 commands-to-buttons pass) ──
        // /link, /revoke, /friend-code and /redeem only existed as typed
        // commands. The relay replies privately in the active chat channel.
        widgets::subsection_label(ui, theme, "Device & friend codes");
        widgets::body_hint(
            ui, theme,
            "Link code: one-time code (5 min) to sign this identity in on another \
             device. Friend code: shareable code that makes you and the redeemer \
             follow each other. Results appear as private messages in Chat.",
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Generate device link code")
                .tooltip("Create a one-time code (expires in 5 minutes) to add another \
                          device to this identity. The code appears privately in Chat.")
                .show(ui, theme)
            {
                send_slash(state, "/link");
                state.server_settings_status = "Sent: /link — the code appears privately in Chat.".into();
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Generate friend code")
                .tooltip("Create a shareable code. When someone redeems it, you both \
                          follow each other automatically. Appears privately in Chat.")
                .show(ui, theme)
            {
                send_slash(state, "/friend-code");
                state.server_settings_status = "Sent: /friend-code — the code appears privately in Chat.".into();
            }
        });
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Redeem friend code:")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.redeem_code_draft)
                    .desired_width(120.0)
                    .hint_text("8-char code"),
            );
            let code_ok = !state.redeem_code_draft.trim().is_empty();
            ui.add_enabled_ui(code_ok, |ui| {
                if widgets::Button::secondary("Redeem")
                    .tooltip("Redeem a friend code someone shared with you — you'll \
                              follow each other automatically.")
                    .show(ui, theme)
                {
                    let cmd = format!("/redeem {}", state.redeem_code_draft.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {} — result appears in Chat.", cmd);
                    state.redeem_code_draft.clear();
                }
            });
        });
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Revoke a device:")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.revoke_key_draft)
                    .desired_width(160.0)
                    .hint_text("key prefix (8+ chars)"),
            );
            let key_ok = state.revoke_key_draft.trim().len() >= 4;
            ui.add_enabled_ui(key_ok, |ui| {
                if widgets::Button::danger("Revoke")
                    .tooltip("Remove a stolen or lost device from your name. Paste the \
                              first characters of that device's public key (from /users \
                              or your other device's Settings).")
                    .show(ui, theme)
                {
                    let cmd = format!("/revoke {}", state.revoke_key_draft.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {} — result appears in Chat.", cmd);
                    state.revoke_key_draft.clear();
                }
            });
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // Disconnect (was the old "danger zone" action — leaves it in user scope
        // because anyone can disconnect themselves).
        let (disconnect_label, group_id) = match resolve_group(state) {
            Some((id, _name)) => ("Leave group", Some(id)),
            None => ("Disconnect from server", None),
        };
        let disconnect_tip = if group_id.is_some() {
            "Leave this group. You won't receive new messages here. You can rejoin via \
             the group invite link."
        } else {
            "Stop the WebSocket connection to this server. Your identity stays on your \
             device. Reconnect any time by re-entering the server URL."
        };
        if widgets::Button::danger(disconnect_label).tooltip(disconnect_tip).show(ui, theme) {
            do_disconnect(state, group_id);
        }
    });
}

// ── Mod Section ─────────────────────────────────────────────────────────────

fn draw_mod_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::tinted_section(ui, theme, "MODERATOR", MOD_COLOR, SECTION_MAX_WIDTH, |ui, theme| {
        widgets::body_hint(
            ui, theme,
            "Mods can take action on members and review reported messages. Type a username \
             below, then click an action. Leave the field blank to use a slash command in chat instead.",
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
                if widgets::Button::secondary("Mute")
                    .tooltip("Prevent this user from sending messages. They can still read. \
                              Reversible, click Unmute to restore.")
                    .show(ui, theme)
                {
                    let cmd = format!("/mute {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Unmute")
                    .tooltip("Restore the user's ability to send messages.")
                    .show(ui, theme)
                {
                    let cmd = format!("/unmute {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::danger("Kick")
                    .tooltip("Disconnect the user immediately. They can rejoin (use Ban for a \
                              persistent block, admin-only).")
                    .show(ui, theme)
                {
                    let cmd = format!("/kick {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
            });
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Muted users (v0.246): list + per-row Unmute ──
        draw_muted_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        widgets::subsection_label(ui, theme, "Channel moderation");
        widgets::body_hint(
            ui, theme,
            "Acts on the currently-active channel.",
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Pin last message")
                .tooltip("Pin the most recent message in the channel you have open. Pinned \
                          messages stay accessible from the channel header.")
                .show(ui, theme)
            {
                send_slash(state, "/pin");
                state.server_settings_status = "Sent: /pin".into();
            }
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Reports surface (merged from former Reports tab) ──
        widgets::subsection_label(ui, theme, "Reports");
        widgets::body_hint(
            ui, theme,
            "Queue of messages flagged via the Report button on a chat row. View shows the \
             flagged content + reporter. Decide: Dismiss / Warn / Mute / Kick / Ban / Mark Bogus. \
             Mark Bogus deducts trust from the reporter so abusive flagging gets self-corrected.",
        );
        ui.add_space(theme.spacing_xs);
        widgets::body_hint(
            ui, theme,
            "Anti-abuse defenses: rate limit (max reports per hour), same-target cooldown \
             (no spam-reporting one user), self-reports rejected, trust-score weighting on \
             reporter rep, signed transparent log of all decisions, federation opt-in for \
             cross-server escalation. See docs/design/report-system.md for the full design.",
        );
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("View reports")
                .tooltip("Open the report queue in chat (current implementation uses the \
                          /reports slash command, UI surface lands in v0.194+).")
                .show(ui, theme)
            {
                send_slash(state, "/reports");
                state.server_settings_status = "Sent: /reports, check the active channel for results.".into();
            }
            ui.add_space(theme.spacing_sm);
            // /reports-clear had no button anywhere (v0.722 commands-to-buttons
            // pass). Admin-only server-side; the relay rejects others politely.
            if state.server_settings_confirm_action.as_deref() == Some("/reports-clear") {
                if widgets::Button::danger("Really clear ALL reports?")
                    .tooltip("Click to confirm. This empties the whole report queue.")
                    .show(ui, theme)
                {
                    send_slash(state, "/reports-clear");
                    state.server_settings_status = "Sent: /reports-clear".into();
                    state.server_settings_confirm_action = None;
                }
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    state.server_settings_confirm_action = None;
                }
            } else if widgets::Button::secondary("Clear all reports")
                .tooltip("Empty the report queue (admin only). Asks to confirm.")
                .show(ui, theme)
            {
                state.server_settings_confirm_action = Some("/reports-clear".to_string());
            }
        });
    });
}

// ── Federation panel (v0.722) ───────────────────────────────────────────────

/// Background fetch of GET /api/federation/servers (public read endpoint).
/// Same worker-thread + mpsc pattern as the System health panel.
fn spawn_federation_fetch(state: &mut GuiState) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    state.federation_rx = Some(rx);
    state.federation_status = "Loading...".to_string();
    std::thread::spawn(move || {
        let fetch = || -> Result<Vec<crate::gui::FederationServerRow>, String> {
            let body = ureq::get(&format!("{base}/api/federation/servers"))
                .call()
                .map_err(|e| format!("list failed: {e}"))?
                .into_string()
                .map_err(|e| format!("read: {e}"))?;
            let val: serde_json::Value =
                serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
            let arr = val.as_array().cloned().unwrap_or_default();
            Ok(arr
                .into_iter()
                .map(|s| crate::gui::FederationServerRow {
                    server_id: s.get("server_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    url: s.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    trust_tier: s.get("trust_tier").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                    status: s.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    accord_compliant: s.get("accord_compliant").and_then(|v| v.as_bool()).unwrap_or(false),
                })
                .collect())
        };
        let _ = tx.send(fetch());
    });
}

/// Federation admin panel — the GUI for the /server-* commands, which
/// previously existed ONLY as typed commands (and /server-add was outright
/// unreachable before the v0.716 dot-gate fix). List / add / trust-tier /
/// remove / connect-all. Actions go through the same slash commands the
/// relay already enforces role checks on; the list reads the public REST
/// endpoint. Federation-activation Phase 1 UI (docs/design/federation-activation.md).
fn draw_federation_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Drain a finished fetch.
    if let Some(rx) = &state.federation_rx {
        match rx.try_recv() {
            Ok(Ok(rows)) => {
                state.federation_servers = rows;
                state.federation_status.clear();
                state.federation_rx = None;
            }
            Ok(Err(e)) => {
                state.federation_status = e;
                state.federation_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.federation_status = "Fetch thread died.".to_string();
                state.federation_rx = None;
            }
        }
    }
    // First view while connected: fetch automatically.
    let ws_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if ws_connected
        && state.federation_servers.is_empty()
        && state.federation_rx.is_none()
        && state.federation_status.is_empty()
    {
        spawn_federation_fetch(state);
    }

    widgets::subsection_label(ui, theme, "Federation");
    widgets::body_hint(
        ui, theme,
        "Peer servers this relay federates with. Trust tiers: 0 = untrusted, \
         1 = basic, 2 = trusted, 3 = fully trusted. Command results appear \
         privately in Chat; click Refresh afterwards to see the updated list.",
    );
    ui.add_space(theme.spacing_xs);

    if state.federation_servers.is_empty() && state.federation_status.is_empty() {
        widgets::body_hint(ui, theme, "No federated servers yet — add one below.");
    }
    if !state.federation_status.is_empty() {
        ui.label(
            RichText::new(&state.federation_status)
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_xs);
    }

    let rows = state.federation_servers.clone();
    for row in &rows {
        ui.horizontal(|ui| {
            let status_color = if row.status == "connected" || row.status == "verified" {
                theme.success()
            } else {
                theme.text_muted()
            };
            ui.label(
                RichText::new(&row.name)
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new(&row.url)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.label(
                RichText::new(&row.status)
                    .size(theme.font_size_small)
                    .color(status_color),
            );
            if row.accord_compliant {
                ui.label(
                    RichText::new("accord")
                        .size(theme.font_size_small)
                        .color(theme.success()),
                );
            }
            // Trust tier selector — sends /server-trust on change.
            let mut tier = row.trust_tier;
            egui::ComboBox::from_id_salt(format!("fed_trust_{}", row.server_id))
                .selected_text(format!("trust {}", tier))
                .width(80.0)
                .show_ui(ui, |ui| {
                    for t in 0..=3 {
                        ui.selectable_value(&mut tier, t, format!("{} - {}", t, match t {
                            0 => "untrusted",
                            1 => "basic",
                            2 => "trusted",
                            _ => "full",
                        }));
                    }
                });
            if tier != row.trust_tier {
                let cmd = format!("/server-trust {} {}", row.server_id, tier);
                send_slash(state, &cmd);
                state.server_settings_status = format!("Sent: {} — Refresh to confirm.", cmd);
            }
            // Remove (confirm per row via the shared confirm slot).
            let confirm_key = format!("/server-remove {}", row.server_id);
            if state.server_settings_confirm_action.as_deref() == Some(confirm_key.as_str()) {
                if widgets::Button::danger("Really remove?").show(ui, theme) {
                    send_slash(state, &confirm_key);
                    state.server_settings_status = format!("Sent: {} — Refresh to confirm.", confirm_key);
                    state.server_settings_confirm_action = None;
                }
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    state.server_settings_confirm_action = None;
                }
            } else if widgets::Button::ghost("Remove")
                .tooltip("Stop federating with this server. Asks to confirm.")
                .show(ui, theme)
            {
                state.server_settings_confirm_action = Some(confirm_key);
            }
        });
        ui.add_space(theme.spacing_xs);
    }

    // Add-server row.
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Add:")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        ui.add(
            egui::TextEdit::singleline(&mut state.federation_add_url_draft)
                .desired_width(200.0)
                .hint_text("https://server.example.com"),
        );
        ui.add(
            egui::TextEdit::singleline(&mut state.federation_add_name_draft)
                .desired_width(110.0)
                .hint_text("name (optional)"),
        );
        let url = state.federation_add_url_draft.trim().to_string();
        let url_ok = url.starts_with("http://") || url.starts_with("https://");
        ui.add_enabled_ui(url_ok, |ui| {
            if widgets::Button::primary("Add server")
                .tooltip("Federate with another HumanityOS relay. The relay auto-discovers \
                          its details via /api/server-info.")
                .show(ui, theme)
            {
                let name = state.federation_add_name_draft.trim().to_string();
                let cmd = if name.is_empty() {
                    format!("/server-add {}", url)
                } else {
                    format!("/server-add {} {}", url, name)
                };
                send_slash(state, &cmd);
                state.server_settings_status = format!("Sent: {} — Refresh to see it listed.", cmd);
                state.federation_add_url_draft.clear();
                state.federation_add_name_draft.clear();
            }
        });
    });
    ui.add_space(theme.spacing_xs);
    ui.horizontal(|ui| {
        if widgets::Button::secondary("Refresh")
            .tooltip("Re-fetch the federated-server list.")
            .show(ui, theme)
        {
            spawn_federation_fetch(state);
        }
        ui.add_space(theme.spacing_sm);
        if widgets::Button::secondary("Connect to all verified")
            .tooltip("Open federation connections to every verified peer server \
                      (/server-connect). Result appears privately in Chat.")
            .show(ui, theme)
        {
            send_slash(state, "/server-connect");
            state.server_settings_status = "Sent: /server-connect — result appears in Chat.".into();
        }
    });
}

// ── System health (v0.720) ──────────────────────────────────────────────────

/// Kick off a background fetch of the connected server's /health + /api/stats.
/// Worker thread + mpsc, same pattern as the Files page's shared-files fetch —
/// never blocks the UI thread.
fn spawn_system_health_fetch(state: &mut GuiState) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    state.system_health_rx = Some(rx);
    state.system_health_status = "Loading...".to_string();
    std::thread::spawn(move || {
        let fetch = || -> Result<crate::gui::SystemHealth, String> {
            let h_body = ureq::get(&format!("{base}/health"))
                .call()
                .map_err(|e| format!("/health failed: {e}"))?
                .into_string()
                .map_err(|e| format!("/health read: {e}"))?;
            let h: serde_json::Value = serde_json::from_str(&h_body)
                .map_err(|e| format!("/health parse: {e}"))?;
            let s_body = ureq::get(&format!("{base}/api/stats"))
                .call()
                .map_err(|e| format!("/api/stats failed: {e}"))?
                .into_string()
                .map_err(|e| format!("/api/stats read: {e}"))?;
            let s: serde_json::Value = serde_json::from_str(&s_body)
                .map_err(|e| format!("/api/stats parse: {e}"))?;
            Ok(crate::gui::SystemHealth {
                status: h.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                version: s.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                uptime_seconds: h.get("uptime_seconds").and_then(|v| v.as_u64()).unwrap_or(0),
                total_messages: s.get("total_messages").and_then(|v| v.as_u64()).unwrap_or(0),
                connected_peers: s.get("connected_peers").and_then(|v| v.as_u64()).unwrap_or(0),
            })
        };
        let _ = tx.send(fetch());
    });
}

/// "3d 4h 12m" style humanized uptime.
fn humanize_uptime(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m {}s", secs % 60)
    }
}

/// Read-only live health snapshot of the connected server — in-app ops
/// slice 1 native parity (docs/design/in-app-ops.md: "Health/system view",
/// the lowest-risk slice). Replaces SSHing the VPS to ask "is it up, which
/// build is it running". Auto-fetches on first view; Refresh re-polls.
fn draw_system_health_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Drain a finished fetch.
    if let Some(rx) = &state.system_health_rx {
        match rx.try_recv() {
            Ok(Ok(health)) => {
                state.system_health = Some(health);
                state.system_health_status.clear();
                state.system_health_rx = None;
            }
            Ok(Err(e)) => {
                state.system_health_status = e;
                state.system_health_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Keep frames coming while the worker runs.
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.system_health_status = "Fetch thread died.".to_string();
                state.system_health_rx = None;
            }
        }
    }
    // First view: fetch automatically — but only while actually connected,
    // so opening Settings offline doesn't spawn a doomed request (Refresh
    // still works manually any time).
    let ws_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if ws_connected
        && state.system_health.is_none()
        && state.system_health_rx.is_none()
        && state.system_health_status.is_empty()
    {
        spawn_system_health_fetch(state);
    }

    widgets::subsection_label(ui, theme, "System health");
    widgets::body_hint(
        ui, theme,
        &format!(
            "Live read-only snapshot of {} (its public /health + /api/stats). \
             The version is the deployed build's git commit — compare it against \
             the newest release to spot a stale deploy.",
            state.server_url.trim_end_matches('/')
        ),
    );
    ui.add_space(theme.spacing_xs);

    if let Some(h) = state.system_health.clone() {
        egui::Grid::new("system_health_grid")
            .num_columns(2)
            .spacing([theme.spacing_xl, theme.spacing_xs])
            .show(ui, |ui| {
                let label = |ui: &mut egui::Ui, t: &str| {
                    ui.label(RichText::new(t).size(theme.font_size_small).color(theme.text_secondary()));
                };
                let value = |ui: &mut egui::Ui, t: &str, c: egui::Color32| {
                    ui.label(RichText::new(t).size(theme.font_size_body).color(c));
                };
                label(ui, "Status");
                let (status_txt, status_color) = if h.status == "ok" {
                    ("ok".to_string(), theme.success())
                } else {
                    (h.status.clone(), theme.danger())
                };
                value(ui, &status_txt, status_color);
                ui.end_row();
                label(ui, "Deployed build");
                value(ui, &h.version, theme.text_primary());
                ui.end_row();
                label(ui, "Relay uptime");
                value(ui, &humanize_uptime(h.uptime_seconds), theme.text_primary());
                ui.end_row();
                label(ui, "Messages stored");
                value(ui, &h.total_messages.to_string(), theme.text_primary());
                ui.end_row();
                label(ui, "Connected now");
                value(ui, &h.connected_peers.to_string(), theme.text_primary());
                ui.end_row();
            });
        ui.add_space(theme.spacing_xs);
    }
    if !state.system_health_status.is_empty() {
        ui.label(
            RichText::new(&state.system_health_status)
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_xs);
    }
    if widgets::Button::secondary("Refresh")
        .tooltip("Re-poll the server's /health and /api/stats right now.")
        .show(ui, theme)
    {
        spawn_system_health_fetch(state);
    }
}

// ── Admin Section ───────────────────────────────────────────────────────────

fn draw_admin_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::tinted_section(ui, theme, "ADMIN", ADMIN_COLOR, SECTION_MAX_WIDTH, |ui, theme| {
        widgets::body_hint(
            ui, theme,
            "Admin-only controls: who can join, what channels exist, and which users get \
             elevated roles or banned.",
        );
        ui.add_space(theme.spacing_sm);

        // ── System health (v0.720): read-only live snapshot ──
        draw_system_health_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Registration ──
        widgets::subsection_label(ui, theme, "Registration");
        widgets::body_hint(
            ui, theme,
            "Lockdown blocks NEW registrations server-wide. Existing members keep full \
             access, useful during a spam wave or when switching to invite-only mode. \
             Generate Invite produces a one-time code that bypasses lockdown for one new user.",
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Toggle lockdown")
                .tooltip("Flip the registration gate on/off. Existing members are unaffected. \
                          Status appears in chat after the toggle.")
                .show(ui, theme)
            {
                send_slash(state, "/lockdown");
                state.server_settings_status = "Sent: /lockdown, registration toggle requested.".into();
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::primary("Generate invite code")
                .tooltip("Create a single-use invite code. Code appears in the chat channel. \
                          Share it with one person, they can register even during lockdown.")
                .show(ui, theme)
            {
                send_slash(state, "/invite");
                state.server_settings_status = "Sent: /invite, code will appear in the active channel.".into();
            }
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Channels (full spreadsheet, merged from former Channels tab) ──
        draw_channels_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Server policy (v0.200.0): per-role limits, sharing toggles ──
        draw_server_policy_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Roles (v0.242, Phase R3): create/edit/delete custom roles ──
        draw_roles_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Services (v0.262.16): soft feature gate + OS-daemon control ──
        draw_services_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── User management ──
        widgets::subsection_label(ui, theme, "User management");
        widgets::body_hint(
            ui, theme,
            &format!(
                "Acts on the username typed in the Moderator section above (currently: {}). \
                 Verify gives a green check next to their name. Promote to mod grants \
                 moderator-tier permissions. Ban is permanent, they can't rejoin without \
                 admin reversal.",
                if state.server_settings_target_user.trim().is_empty() {
                    "(empty)".to_string()
                } else {
                    state.server_settings_target_user.trim().to_string()
                }
            ),
        );
        ui.add_space(theme.spacing_xs);
        let user_valid = !state.server_settings_target_user.trim().is_empty();
        ui.add_enabled_ui(user_valid, |ui| {
            ui.horizontal(|ui| {
                if widgets::Button::secondary("Verify")
                    .tooltip("Add a verified badge next to the user's name. Use for known-good \
                              identities to help others trust them at a glance.")
                    .show(ui, theme)
                {
                    let cmd = format!("/verify {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Promote to mod")
                    .tooltip("Grant moderator role: the user can now mute, kick, pin messages, \
                              and view the report queue. Reversible, promote to admin to allow \
                              them to demote others.")
                    .show(ui, theme)
                {
                    let cmd = format!("/mod {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::danger("Ban")
                    .tooltip("Permanently block this user from the server. Their public key is \
                              added to the ban list. Reversible only by another admin.")
                    .show(ui, theme)
                {
                    let cmd = format!("/ban {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
            });
            // Second row (v0.722 commands-to-buttons pass): the reversals +
            // donor mark had slash commands but no buttons.
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                if widgets::Button::secondary("Remove verified")
                    .tooltip("Take the verified badge away (resets the user to the plain \
                              member role).")
                    .show(ui, theme)
                {
                    let cmd = format!("/unverify {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Remove mod")
                    .tooltip("Demote the user from moderator back to a regular member.")
                    .show(ui, theme)
                {
                    let cmd = format!("/unmod {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Mark as donor")
                    .tooltip("Give the user the donor badge as a thank-you for supporting \
                              the server.")
                    .show(ui, theme)
                {
                    let cmd = format!("/donor {}", state.server_settings_target_user.trim());
                    send_slash(state, &cmd);
                    state.server_settings_status = format!("Sent: {}", cmd);
                }
                ui.add_space(theme.spacing_sm);
                // /name-release deletes EVERY key association for the name —
                // account recovery / squatter cleanup. Destructive, so confirm.
                if state.server_settings_confirm_action.as_deref() == Some("/name-release") {
                    if widgets::Button::danger("Really release the name?")
                        .tooltip("Click to confirm. Every device key registered to this name \
                                  is unlinked; the name becomes claimable again.")
                        .show(ui, theme)
                    {
                        let cmd = format!("/name-release {}", state.server_settings_target_user.trim());
                        send_slash(state, &cmd);
                        state.server_settings_status = format!("Sent: {}", cmd);
                        state.server_settings_confirm_action = None;
                    }
                    if widgets::Button::secondary("Cancel").show(ui, theme) {
                        state.server_settings_confirm_action = None;
                    }
                } else if widgets::Button::danger("Release name")
                    .tooltip("Unlink every device key from this name so it can be registered \
                              fresh (account recovery / squatted-name cleanup). Asks to confirm.")
                    .show(ui, theme)
                {
                    state.server_settings_confirm_action = Some("/name-release".to_string());
                }
            });
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Banned users (v0.245): list + per-row Unban ──
        draw_banned_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Server maintenance (v0.722 commands-to-buttons pass): /wipe,
        // /wipe-all and /gc had no clickable path. All three echo their
        // result privately in chat; the destructive two confirm first.
        widgets::subsection_label(ui, theme, "Server maintenance");
        widgets::body_hint(
            ui, theme,
            &format!(
                "History wipes are permanent. \"Wipe active channel\" clears the channel \
                 you currently have open in Chat ({}); \"Wipe ALL channels\" clears every \
                 channel's history. \"Clean up names\" releases names inactive for 90+ days.",
                if state.chat_active_channel.is_empty() { "general" } else { state.chat_active_channel.as_str() }
            ),
        );
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            if state.server_settings_confirm_action.as_deref() == Some("/wipe") {
                if widgets::Button::danger("Really wipe this channel?")
                    .tooltip("Click to confirm. The active channel's message history is \
                              permanently deleted.")
                    .show(ui, theme)
                {
                    send_slash(state, "/wipe");
                    state.server_settings_status = "Sent: /wipe (active channel history cleared).".into();
                    state.server_settings_confirm_action = None;
                }
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    state.server_settings_confirm_action = None;
                }
            } else if widgets::Button::danger("Wipe active channel")
                .tooltip("Permanently clear the open channel's message history. Asks to confirm.")
                .show(ui, theme)
            {
                state.server_settings_confirm_action = Some("/wipe".to_string());
            }
            ui.add_space(theme.spacing_sm);
            if state.server_settings_confirm_action.as_deref() == Some("/wipe-all") {
                if widgets::Button::danger("Really wipe EVERY channel?")
                    .tooltip("Click to confirm. ALL channels' message history is permanently \
                              deleted server-wide.")
                    .show(ui, theme)
                {
                    send_slash(state, "/wipe-all");
                    state.server_settings_status = "Sent: /wipe-all (all channel history cleared).".into();
                    state.server_settings_confirm_action = None;
                }
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    state.server_settings_confirm_action = None;
                }
            } else if widgets::Button::danger("Wipe ALL channels")
                .tooltip("Permanently clear every channel's history server-wide. Asks to confirm.")
                .show(ui, theme)
            {
                state.server_settings_confirm_action = Some("/wipe-all".to_string());
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Clean up names")
                .tooltip("Garbage-collect registered names that have been inactive for \
                          90+ days, freeing them for new users. Non-destructive to \
                          active members.")
                .show(ui, theme)
            {
                send_slash(state, "/gc");
                state.server_settings_status = "Sent: /gc (inactive-name cleanup requested).".into();
            }
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Federation (v0.722): the /server-* commands had no GUI at all ──
        draw_federation_admin(ui, theme, state);

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Game world bans (v0.479): folded in from the former dedicated Game
        // Admin page. STRUCTURALLY SEPARATE from the chat "Banned users" panel
        // above (its own table + disclaimer) -- chat is a right, MMO play is a
        // privilege. Placed adjacent so an admin manages all bans in one place,
        // but clearly distinct.
        super::game_admin::draw_section(ui, theme, state);
    });
}

/// Banned-users admin panel. Lists every key in the relay's
/// `banned_keys` table (name captured at ban time) with a per-row
/// Unban button. The list is admin-only — the relay targets the
/// `banned_list` message at the requesting admin so it never leaks.
///
/// The panel auto-requests the list once per session the first time an
/// admin opens this section; a manual Refresh re-fetches it.
fn draw_banned_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Banned users");
    widgets::body_hint(
        ui, theme,
        "Everyone currently blocked from this server. A ban removes them from the \
         member list and rejects the key at connect, so the only record of who they \
         were is captured here. Click Unban to restore access, the change takes \
         effect on their next connection attempt.",
    );
    ui.add_space(theme.spacing_xs);

    // Auto-request once per session so the panel isn't empty on first
    // open. `chat_banned_requested` is reset on disconnect (lib.rs).
    if !state.chat_banned_requested {
        send_banned_list_request(state);
        state.chat_banned_requested = true;
    }

    ui.horizontal(|ui| {
        if widgets::Button::secondary("Refresh")
            .tooltip("Re-fetch the ban list from the server.")
            .show(ui, theme)
        {
            send_banned_list_request(state);
            state.server_settings_status = "Requested the latest ban list.".into();
        }
        ui.add_space(theme.spacing_sm);
        ui.colored_label(
            theme.text_muted(),
            format!("{} banned", state.chat_banned_users.len()),
        );
    });
    ui.add_space(theme.spacing_sm);

    if state.chat_banned_users.is_empty() {
        widgets::body_hint(ui, theme, "No one is banned. A clean slate.");
        return;
    }

    // Collect the unban target outside the loop (can't mutate `state`
    // while iterating a clone of its field — same pattern as roles).
    let banned = state.chat_banned_users.clone();
    let mut unban_key: Option<String> = None;
    for b in &banned {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // Name (or a placeholder for pre-v0.245 bans with no name).
            let shown_name = if b.name.trim().is_empty() {
                "(unknown, banned before name capture)".to_string()
            } else {
                b.name.clone()
            };
            ui.add_sized(
                [180.0, 22.0],
                egui::Label::new(
                    egui::RichText::new(shown_name)
                        .color(theme.text_primary())
                        .size(theme.body_size),
                )
                .truncate(),
            );
            // Short key.
            let short_key = if b.public_key.len() > 16 {
                format!("{}…", &b.public_key[..16])
            } else {
                b.public_key.clone()
            };
            ui.add_sized(
                [150.0, 22.0],
                egui::Label::new(
                    egui::RichText::new(short_key)
                        .color(theme.text_muted())
                        .size(theme.body_size * 0.9)
                        .monospace(),
                ),
            );
            // Banned-at date.
            ui.add_sized(
                [160.0, 22.0],
                egui::Label::new(
                    egui::RichText::new(format_ban_date(b.banned_at))
                        .color(theme.text_muted())
                        .size(theme.body_size * 0.9),
                ),
            );
            if widgets::Button::secondary("Unban")
                .tooltip("Lift this ban. The user can reconnect immediately.")
                .show(ui, theme)
            {
                unban_key = Some(b.public_key.clone());
            }
        });
        ui.add_space(2.0);
    }

    if let Some(key) = unban_key {
        send_unban(state, &key);
        state.server_settings_status = "Sent unban, the ban list will refresh.".into();
    }
}

/// Send a `banned_list_request` (admin-gated; relay replies privately).
fn send_banned_list_request(state: &GuiState) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "banned_list_request" });
            client.send(&msg.to_string());
        }
    }
}

/// Send an `unban` for the given public key (admin-gated server-side).
fn send_unban(state: &GuiState, public_key: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "unban", "target": public_key });
            client.send(&msg.to_string());
        }
    }
}

/// Format a Unix-ms timestamp as `YYYY-MM-DD HH:MM` (UTC). Used for
/// both the ban and mute panels. Uses the same chrono-free civil-date
/// math as chat.rs::format_full_timestamp (Howard Hinnant's algorithm)
/// so we don't add a dependency.
fn format_ban_date(ms: i64) -> String {
    if ms <= 0 {
        return ", ".to_string();
    }
    let secs = ms / 1000;
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (hh, mm) = (tod / 3600, (tod % 3600) / 60);
    // Civil date from days since 1970-01-01 (Hinnant).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02} {hh:02}:{mm:02}")
}

/// Muted-users mod panel. Lists every key in `muted_members` (name
/// captured at mute time) with a per-row Unmute. Mirrors the
/// Banned-users panel but lives in the MOD section because mute is a
/// mod-level action. The list is mod-only — the relay targets the
/// `muted_list` message at the requesting mod so it never leaks.
fn draw_muted_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Muted users");
    widgets::body_hint(
        ui, theme,
        "Everyone currently muted. A muted user can still read every channel but \
         can't post until unmuted. Mute keeps their role intact (it does NOT demote \
         them), so unmute restores them exactly as they were. Click Unmute to lift \
         it, effective on their next message attempt.",
    );
    ui.add_space(theme.spacing_xs);

    if !state.chat_muted_requested {
        send_muted_list_request(state);
        state.chat_muted_requested = true;
    }

    ui.horizontal(|ui| {
        if widgets::Button::secondary("Refresh")
            .tooltip("Re-fetch the mute list from the server.")
            .show(ui, theme)
        {
            send_muted_list_request(state);
            state.server_settings_status = "Requested the latest mute list.".into();
        }
        ui.add_space(theme.spacing_sm);
        ui.colored_label(
            theme.text_muted(),
            format!("{} muted", state.chat_muted_users.len()),
        );
    });
    ui.add_space(theme.spacing_sm);

    if state.chat_muted_users.is_empty() {
        widgets::body_hint(ui, theme, "No one is muted.");
        return;
    }

    let muted = state.chat_muted_users.clone();
    let mut unmute_key: Option<String> = None;
    for m in &muted {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let shown_name = if m.name.trim().is_empty() {
                "(unknown)".to_string()
            } else {
                m.name.clone()
            };
            ui.add_sized(
                [180.0, 22.0],
                egui::Label::new(
                    egui::RichText::new(shown_name)
                        .color(theme.text_primary())
                        .size(theme.body_size),
                )
                .truncate(),
            );
            let short_key = if m.public_key.len() > 16 {
                format!("{}…", &m.public_key[..16])
            } else {
                m.public_key.clone()
            };
            ui.add_sized(
                [150.0, 22.0],
                egui::Label::new(
                    egui::RichText::new(short_key)
                        .color(theme.text_muted())
                        .size(theme.body_size * 0.9)
                        .monospace(),
                ),
            );
            ui.add_sized(
                [160.0, 22.0],
                egui::Label::new(
                    egui::RichText::new(format_ban_date(m.muted_at))
                        .color(theme.text_muted())
                        .size(theme.body_size * 0.9),
                ),
            );
            if widgets::Button::secondary("Unmute")
                .tooltip("Lift this mute. The user can post again immediately.")
                .show(ui, theme)
            {
                unmute_key = Some(m.public_key.clone());
            }
        });
        ui.add_space(2.0);
    }

    if let Some(key) = unmute_key {
        send_unmute(state, &key);
        state.server_settings_status = "Sent unmute, the mute list will refresh.".into();
    }
}

/// Send a `muted_list_request` (mod-gated; relay replies privately).
fn send_muted_list_request(state: &GuiState) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "muted_list_request" });
            client.send(&msg.to_string());
        }
    }
}

/// Send an `unmute` for the given public key (mod-gated server-side).
fn send_unmute(state: &GuiState, public_key: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "unmute", "target": public_key });
            client.send(&msg.to_string());
        }
    }
}

/// Channels spreadsheet (admin only, lives inside the ADMIN tinted
/// section). One row per channel with editable cells: Name | Description
/// | Read-only | Voice | Federated | (Save) | (Delete). Plus a sticky
/// "+ new channel" row at the bottom. Every cell uses CHANNEL_ROW_H so
/// the row is one consistent visual line.
fn draw_channels_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Channels");
    widgets::body_hint(
        ui, theme,
        "Each row is a channel. Edit a cell then click Save. Hover any column header for \
         what that flag does. Use the bottom row to create a new channel. Delete is \
         permanent, messages are kept but the channel goes away.",
    );
    ui.add_space(theme.spacing_xs);
    widgets::body_hint(
        ui, theme,
        "Read-only (eye icon): only mods + admins can post; everyone can read. \
         Voice: enables the voice-call icon next to the channel, disable to make \
         it text-only.",
    );
    ui.add_space(theme.spacing_xs);
    widgets::body_hint(
        ui, theme,
        "Federated (node icon): messages here gossip to peer servers in the \
         federation network. Beyond cross-server read/reply, this gives you: \
         (1) Reach, your community's posts are visible to people who never \
         joined THIS server; (2) Resilience, the conversation + history is \
         mirrored on peer servers, so if this VPS goes down or is seized the \
         thread survives; (3) Censorship-resistance, no single operator (not \
         even you) can silently erase a federated channel everywhere; \
         (4) Discovery, members on other servers can find and join the \
         discussion. Off = local-only: faster, fully private to this server, \
         but a single point of failure. Choose per-channel: keep #private-ops \
         local, federate #general.",
    );
    ui.add_space(theme.spacing_sm);

    // Header row — visual column titles with hover tooltips so admins
    // get the per-column explanation without scrolling back to the
    // intro paragraph.
    channel_grid_header(ui, theme);

    // One row per existing channel.
    let channels = state.chat_channels.clone();
    let mut delete_id: Option<String> = None;
    let mut save_id: Option<String> = None;
    // (name, target_position) for /channel-reorder — the up/down buttons
    // send the row's display index ± 1 as the absolute position. (v0.722)
    let mut reorder: Option<(String, i32)> = None;

    for (ci, ch) in channels.iter().enumerate() {
        // Pull or seed the draft for this channel.
        let draft = state.server_settings_channel_drafts
            .entry(ch.id.clone())
            .or_insert_with(|| crate::gui::ChannelDraft {
                name: ch.name.clone(),
                description: ch.description.clone(),
                read_only: ch.read_only,
                federated: ch.federated,
                voice_enabled: ch.voice_enabled,
            });
        let mut row_changed = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Each cell uses ui.add_sized with shared CHANNEL_COL_WIDTHS
            // so columns line up across header + every data row, AND
            // CHANNEL_ROW_H so vertical alignment is identical from cell
            // to cell.
            if ui.add_sized(
                Vec2::new(CHANNEL_COL_WIDTHS[0], CHANNEL_ROW_H),
                egui::TextEdit::singleline(&mut draft.name),
            ).changed() { row_changed = true; }
            if ui.add_sized(
                Vec2::new(CHANNEL_COL_WIDTHS[1], CHANNEL_ROW_H),
                egui::TextEdit::singleline(&mut draft.description),
            ).changed() { row_changed = true; }
            centered_checkbox(ui, theme, &mut draft.read_only, CHANNEL_COL_WIDTHS[2], &mut row_changed);
            centered_checkbox(ui, theme, &mut draft.voice_enabled, CHANNEL_COL_WIDTHS[3], &mut row_changed);
            centered_checkbox(ui, theme, &mut draft.federated, CHANNEL_COL_WIDTHS[4], &mut row_changed);
            row_button(
                ui, theme, CHANNEL_COL_WIDTHS[5],
                widgets::Button::primary("Save").tooltip(
                    "Apply this row's changes. Updates name / description / read-only / \
                     voice / federated flags on the relay and broadcasts to all clients."
                ),
                || { save_id = Some(ch.id.clone()); },
            );
            row_button(
                ui, theme, CHANNEL_COL_WIDTHS[6],
                widgets::Button::danger("Delete").tooltip(
                    "Permanently delete this channel. Past messages stay in the database \
                     but the channel disappears from sidebars. Cannot be undone."
                ),
                || { delete_id = Some(ch.id.clone()); },
            );
            // Reorder (v0.722 commands-to-buttons pass — /channel-reorder had
            // no GUI). Up/down sends the row's index ± 1 as the new position.
            if ci > 0 {
                if widgets::Button::ghost("Up")
                    .tooltip("Move this channel up in the sidebar order.")
                    .show(ui, theme)
                {
                    reorder = Some((ch.name.clone(), ci as i32 - 1));
                }
            }
            if ci + 1 < channels.len() {
                if widgets::Button::ghost("Down")
                    .tooltip("Move this channel down in the sidebar order.")
                    .show(ui, theme)
                {
                    reorder = Some((ch.name.clone(), ci as i32 + 1));
                }
            }
        });
        let _ = row_changed; // visual cue could go here; keep minimal for v1
    }
    if let Some((name, pos)) = reorder {
        let cmd = format!("/channel-reorder {} {}", name, pos);
        send_slash(state, &cmd);
        state.server_settings_status = format!("Sent: {}", cmd);
    }

    ui.add_space(theme.spacing_sm);
    ui.separator();
    ui.add_space(theme.spacing_sm);

    // "+ new channel" sticky row at bottom.
    widgets::subsection_label(ui, theme, "+ New channel");
    ui.add_space(theme.spacing_xs);
    let new_draft = &mut state.server_settings_new_channel;
    let mut create_clicked = false;
    let mut _row_changed_unused = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_sized(
            Vec2::new(CHANNEL_COL_WIDTHS[0], CHANNEL_ROW_H),
            egui::TextEdit::singleline(&mut new_draft.name).hint_text("channel-name"),
        );
        ui.add_sized(
            Vec2::new(CHANNEL_COL_WIDTHS[1], CHANNEL_ROW_H),
            egui::TextEdit::singleline(&mut new_draft.description).hint_text("Description"),
        );
        centered_checkbox(ui, theme, &mut new_draft.read_only, CHANNEL_COL_WIDTHS[2], &mut _row_changed_unused);
        centered_checkbox(ui, theme, &mut new_draft.voice_enabled, CHANNEL_COL_WIDTHS[3], &mut _row_changed_unused);
        centered_checkbox(ui, theme, &mut new_draft.federated, CHANNEL_COL_WIDTHS[4], &mut _row_changed_unused);
        // Create button spans the Save+Delete columns since there's only
        // one action on the new-channel row.
        let create_w = CHANNEL_COL_WIDTHS[5] + CHANNEL_COL_WIDTHS[6] + 4.0;
        let valid = !new_draft.name.trim().is_empty();
        ui.allocate_ui_with_layout(
            Vec2::new(create_w, CHANNEL_ROW_H),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_enabled_ui(valid, |ui| {
                    if widgets::Button::primary("Create")
                        .min_height(CHANNEL_ROW_H)
                        .tooltip(
                            "Create a new channel with the values typed above. All flags \
                             default off (read-only, federated). Voice defaults on. You can \
                             change any flag after creation by editing the row."
                        )
                        .show(ui, theme)
                    {
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

    // Apply pending row actions.
    if let Some(id) = save_id {
        if let Some(draft) = state.server_settings_channel_drafts.get(&id).cloned() {
            // 1. Apply locally so the chat UI updates immediately.
            if let Some(ch) = state.chat_channels.iter_mut().find(|c| c.id == id) {
                ch.name = draft.name.trim().to_string();
                ch.description = draft.description.trim().to_string();
                ch.read_only = draft.read_only;
                ch.voice_enabled = draft.voice_enabled;
                ch.federated = draft.federated;
            }
            // 2. Send to relay (v0.192.0 channel_update handler persists
            //    these flags in the channels table and rebroadcasts the
            //    new channel_list to all clients).
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
            state.server_settings_status = format!("Channel '{}' update applied.", draft.name);
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
}

/// Render a button inside a fixed-width spreadsheet cell with the
/// shared CHANNEL_ROW_H height. Centralizes the alignment so Save and
/// Server-wide policy editor (admin only, lives inside the ADMIN tinted
/// section). Per-role chat character limits + image/file/voice/streaming
/// toggles + max upload + allowed extensions. v0.200.0.
///
/// Editing pattern: the displayed values come from
/// `state.server_settings_draft` if set, otherwise from
/// `state.server_settings` (the cached relay-broadcast state). Click
/// "Edit" to copy the cache into the draft. "Save" sends a
/// `server_settings_update` WS message; the relay broadcasts the new
/// state which clears the draft on next receive. "Cancel" drops the draft.
fn draw_server_policy_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Server policy");
    widgets::body_hint(
        ui, theme,
        "True server-wide settings: the post-quantum-signature requirement, \
         the total upload disk cap, and the allowed file extensions. The \
         master sharing / voice / streaming kill-switches now live as the \
         'Server master' row at the top of the Roles table below, same \
         columns as every role, so it's one cohesive table instead of a \
         second detached set of toggles. Per-role limits (chars / upload \
         MB / uploads-kept) are per-role in that table too. Changes \
         broadcast to all connected clients on Save.",
    );
    ui.add_space(theme.spacing_sm);

    // Always-editable (operator 2026-05-15: "make it so the policy
    // settings are individually editable without clicking the edit
    // policy button. Seems like unnecessary friction."). The draft is
    // the live working copy; it's seeded from the cached relay-broadcast
    // state and only pushed to the relay when the operator clicks Save
    // Changes (server-wide settings must NOT broadcast on every keystroke).
    let cached: crate::relay::storage::ServerSettings =
        state.server_settings.clone().unwrap_or_default();
    // Seed / re-seed the draft from cache when there's no draft yet, OR
    // when it exactly equals the cache (i.e. no unsaved edits) so an
    // external admin's broadcast update flows in. If the draft differs
    // (operator has unsaved edits) we preserve it. Edge case — two
    // admins editing simultaneously — the "Revert to server state"
    // button is the escape hatch.
    if state.server_settings_draft.is_none() {
        state.server_settings_draft = Some(cached.clone());
    }
    let effective = state.server_settings_draft.clone().unwrap_or_else(|| cached.clone());

    // Last-updated badge (informational).
    if let Some(ref s) = state.server_settings {
        let updated_text = if s.updated_at == 0 {
            "Never modified, defaults active.".to_string()
        } else {
            let by = if s.updated_by.is_empty() { "?".to_string() }
                     else if s.updated_by.len() > 16 { format!("{}…{}", &s.updated_by[..6], &s.updated_by[s.updated_by.len()-6..]) }
                     else { s.updated_by.clone() };
            format!("Last updated: {} ms (by {})", s.updated_at, by)
        };
        widgets::body_hint(ui, theme, &updated_text);
        ui.add_space(theme.spacing_sm);
    }

    {
        // ── ALWAYS-EDITABLE ──
        // No mode toggle. Borrow-checker pattern: clone the draft into a
        // local, build the UI against it, write back at the end; Save
        // sends the WS update, Revert resets the draft from cache.
        let _ = effective; // draft is now the single source for the inputs
        let mut draft = state.server_settings_draft.clone().unwrap_or_else(|| cached.clone());
        // v0.262.4: the obsolete "Per-role limits" header + the dead
        // pointer hint were trimmed (operator: "trimmed of obsolete
        // options now"). Per-role chars/upload-MB/uploads-kept moved to
        // the Roles table in R4; the legacy server_settings.max_*_<tier>
        // columns remain only as inert back-compat shadows. What's left
        // here is genuinely server-wide.
        // Server name + description (v0.478.1 / v0.480) — the server's public
        // identity, shown in the launcher's server browser + chat. Edited HERE
        // (the admin's home for their server).
        widgets::subsection_label(ui, theme, "Server name");
        widgets::body_hint(
            ui, theme,
            "The display name for this server. Blank keeps the configured default.",
        );
        widgets::form_row(ui, theme, "Name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut draft.server_name)
                    .desired_width(280.0)
                    .hint_text("My Server"),
            );
        });
        ui.add_space(theme.spacing_md);

        widgets::subsection_label(ui, theme, "Server description");
        widgets::body_hint(
            ui, theme,
            "A short blurb shown to anyone who views this server in the launcher's server \
             list. Plain text. Saves with the other policy changes below.",
        );
        ui.add(
            egui::TextEdit::multiline(&mut draft.server_description)
                .desired_rows(3)
                .desired_width(360.0)
                .hint_text("Describe your server in a sentence or two."),
        );
        ui.add_space(theme.spacing_md);

        widgets::form_row(ui, theme, "Total upload disk cap (MB, server-wide)", |ui| {
            int_input(ui, &mut draft.max_total_upload_mb, 1, 1_000_000);
        });

        ui.add_space(theme.spacing_sm);
        widgets::subsection_label(ui, theme, "Security policy");
        // Full-PQ: the "Require post-quantum signatures" toggle was removed.
        // It only existed to gate the Ed25519→Dilithium migration. The
        // identity now IS Dilithium3 for every client, the relay always
        // verifies the Dilithium `pq_signature`, and there is no Ed25519
        // chat path to fall back to — so a per-server enforcement switch
        // is meaningless. (The struct field is left inert; nothing reads
        // it. Master sharing/voice/streaming live in the Roles grid's
        // "Server master" row.)
        widgets::form_row(ui, theme, "Allowed extensions (csv, blank = any)", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut draft.allowed_file_extensions)
                    .desired_width(280.0)
                    .hint_text("png,jpg,pdf,…"),
            );
        });

        ui.add_space(theme.spacing_sm);
        // Dirty = the working draft differs from the cached relay state.
        // Save is only enabled when dirty (server-wide settings must not
        // re-broadcast a no-op to every client).
        let dirty = draft != cached;
        if dirty {
            widgets::body_hint(ui, theme, "Unsaved changes, click Save Changes to apply server-wide.");
        } else {
            widgets::body_hint(ui, theme, "No unsaved changes. Edit any field above; it applies on Save.");
        }
        ui.add_space(theme.spacing_xs);
        let mut save_clicked = false;
        let mut cancel_clicked = false;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(dirty, |ui| {
                save_clicked = widgets::Button::primary("Save Changes")
                    .tooltip("Send the policy update to the relay. All connected clients \
                              will see the new state immediately.")
                    .show(ui, theme);
            });
            ui.add_space(theme.spacing_sm);
            ui.add_enabled_ui(dirty, |ui| {
                cancel_clicked = widgets::Button::secondary("Revert to server state")
                    .tooltip("Discard your unsaved edits and reload the live server policy.")
                    .show(ui, theme);
            });
        });

        // Persist the (possibly-edited) draft back to state. Without this
        // every frame would overwrite the user's typing with the prior frame's
        // value because the local `draft` is dropped at end of this block.
        state.server_settings_draft = Some(draft.clone());

        if save_clicked {
            // Same payload builder the Roles-grid "Server master" row uses
            // (the 4 master sharing/voice/streaming switches live there
            // now). ONE builder so the two entry points can never drift.
            send_server_settings_update(state, &draft);
            state.server_settings_draft = None;
        }
        if cancel_clicked {
            state.server_settings_draft = None;
        }
    }
}

/// Roles editor (v0.242, Phase R3 — docs/design/roles-system.md).
/// Lists every role; lets an admin edit capabilities/label/color,
/// create custom roles, and delete custom ones. All changes go to the
/// relay via `role_upsert` / `role_delete`; the relay re-broadcasts
/// `role_list` so the UI + badges + assignment dropdown update live.
fn draw_roles_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Roles");
    widgets::body_hint(
        ui, theme,
        "One cohesive table. The top 'Server master' row is the set of \
         server-wide kill-switches; every row below is a role. Effective \
         permission = the Server-master switch for that column AND the \
         role's own checkbox, a user can do X only if BOTH are on. e.g. \
         to let family livestream WITHOUT making them moderators: tick \
         Stream on the Server-master row, tick Stream on a 'family' role, \
         then assign that role to them (click their name in chat → Role \
         dropdown). Turning a Server-master switch OFF instantly disables \
         that capability for everyone regardless of role, your abuse / \
         emergency panic switch. Built-in roles can't be deleted and \
         their id / trust are locked, but every capability and numeric \
         limit (chars / upload MB / uploads kept) is editable per-role.",
    );
    ui.add_space(theme.spacing_sm);

    if state.chat_roles.is_empty() {
        widgets::body_hint(ui, theme, "Waiting for the relay's role list… (connect to a server)");
        return;
    }

    let roles = state.chat_roles.clone();
    let mut pending_save: Option<crate::relay::storage::RoleDef> = None;
    let mut pending_delete: Option<String> = None;
    // Set when the "Server master" row's Save is clicked — applied after
    // the grid closure (same deferred pattern as pending_save/_delete so
    // we don't send mid-borrow). Carries the full edited ServerSettings.
    let mut pending_master_save: Option<crate::relay::storage::ServerSettings> = None;

    // Aligned columns via egui::Grid (operator #1 — was a per-row
    // ui.horizontal inline flow, so variable-width name/tier segments
    // made every Save/Delete button land at a different x; the rows
    // "stairstepped". A fixed-column Grid aligns every cell by
    // construction. 2026-05-16.
    egui::Grid::new("server_roles")
        .num_columns(12)
        .spacing([theme.spacing_xl, theme.spacing_md])
        .striped(true)
        .show(ui, |ui| {
            let hdr = |ui: &mut egui::Ui, t: &str| {
                ui.label(
                    RichText::new(t)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary())
                        .strong(),
                );
            };
            hdr(ui, "Role");
            hdr(ui, "Color");
            hdr(ui, "Stream");
            hdr(ui, "Upload");
            hdr(ui, "Voice");
            hdr(ui, "Image");
            hdr(ui, "File");
            hdr(ui, "Max chars");
            hdr(ui, "Max upMB");
            hdr(ui, "Up kept");
            hdr(ui, "");
            hdr(ui, "");
            ui.end_row();

            // ── Server master row (v0.262.6) ──────────────────────────
            // The 4 server-wide kill-switches used to be a separate
            // "Sharing & policy toggles" checkbox group up in Server
            // policy. The operator (2026-05-16) read that as a DUPLICATE
            // of the per-role Stream/Voice/Image/File columns: "I still
            // see duplicate file sharing areas. One in the old area and
            // another in the new area." They're NOT duplicates (effective
            // = master AND role) but two detached checkbox groups looked
            // redundant. Fix: fold them into THIS table as the top row —
            // same columns as every role — so the master∧capability
            // relationship is visually self-evident. Edits mutate the
            // shared `state.server_settings_draft` (draw_server_policy_admin
            // seeded it earlier this frame); the row's own Save sends the
            // same `server_settings_update` payload the Server-policy Save
            // uses. `Upload` has no server-wide master (legacy general
            // can_upload is per-role only) so that cell is a dash.
            {
                let ss_cached = state.server_settings.clone().unwrap_or_default();
                let mut ss_draft = state.server_settings_draft
                    .clone()
                    .unwrap_or_else(|| ss_cached.clone());
                let dash = |ui: &mut egui::Ui| {
                    ui.label(
                        RichText::new(", ")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                };
                // Col 1 — distinct accent label, no swatch.
                ui.label(
                    RichText::new("Server master")
                        .size(theme.font_size_body)
                        .color(theme.accent())
                        .strong(),
                );
                // Col 2 — no color.
                dash(ui);
                // Cols 3-7 — the global kill-switches (Upload = dash).
                ui.checkbox(&mut ss_draft.video_streaming_enabled, "");
                dash(ui);
                ui.checkbox(&mut ss_draft.voice_channels_enabled, "");
                ui.checkbox(&mut ss_draft.image_sharing_enabled, "");
                ui.checkbox(&mut ss_draft.file_sharing_enabled, "");
                // Cols 8-10 — master has no numeric limits.
                dash(ui);
                dash(ui);
                dash(ui);
                // Col 11 — Save (only when the toggles differ from the
                // live server state, to avoid a no-op broadcast).
                let ss_dirty = ss_draft != ss_cached;
                ui.add_enabled_ui(ss_dirty, |ui| {
                    if widgets::Button::primary("Save").show(ui, theme) {
                        pending_master_save = Some(ss_draft.clone());
                    }
                });
                // Col 12 — cannot delete the master.
                dash(ui);
                ui.end_row();
                // Persist edits so next frame — and the Server-policy
                // "Save Changes" button — see them (shared draft).
                state.server_settings_draft = Some(ss_draft);
            }

            for role in &roles {
                let draft = state.roles_drafts
                    .entry(role.id.clone())
                    .or_insert_with(|| role.clone());
                let is_built_in = draft.built_in;

                // Col 1 — swatch + label.
                ui.horizontal(|ui| {
                    let (sw, _) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                    ui.painter().rect_filled(sw, 3.0, parse_role_color_ss(&draft.color, theme));
                    ui.add_space(theme.spacing_xs);
                    if is_built_in {
                        ui.label(
                            RichText::new(&draft.label)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                        ui.label(
                            RichText::new("(built-in)")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    } else {
                        ui.add_sized(
                            Vec2::new(120.0, 22.0),
                            egui::TextEdit::singleline(&mut draft.label),
                        );
                    }
                });
                // Col 2 — color hex.
                ui.add_sized(
                    Vec2::new(84.0, 22.0),
                    egui::TextEdit::singleline(&mut draft.color).hint_text("#RRGGBB"),
                );
                // Cols 3-7 — capabilities (header names them, no inline labels).
                ui.checkbox(&mut draft.can_stream, "");
                ui.checkbox(&mut draft.can_upload, "");
                ui.checkbox(&mut draft.can_voice, "");
                ui.checkbox(&mut draft.can_image_share, "");
                ui.checkbox(&mut draft.can_file_share, "");
                // Cols 8-10 — per-role numeric limits (R4: owned by the
                // role, editable on EVERY role incl. built-ins; the old
                // base_tier column is gone — it's now just a prefill
                // convenience in the add-role form).
                int_input(ui, &mut draft.max_chars, 1, 1_000_000);
                int_input(ui, &mut draft.max_upload_mb, 1, 10_000);
                int_input(ui, &mut draft.max_uploads_kept, 1, 1_000);
                // Col 11 — Save.
                if widgets::Button::primary("Save").show(ui, theme) {
                    pending_save = Some(draft.clone());
                }
                // Col 8 — Delete (custom only; built-ins emit a placeholder
                // so the striped rows stay rectangular).
                if !is_built_in {
                    if widgets::Button::danger("Delete").show(ui, theme) {
                        pending_delete = Some(draft.id.clone());
                    }
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });

    // ── Add a custom role ──
    ui.add_space(theme.spacing_md);
    widgets::body_hint(ui, theme, "Add a custom role:");
    ui.add_space(theme.spacing_xs);
    let mut create_clicked = false;
    {
        let nr = &mut state.new_role_draft;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_sized(Vec2::new(90.0, 22.0),
                egui::TextEdit::singleline(&mut nr.id).hint_text("id (e.g. family)"));
            ui.add_sized(Vec2::new(110.0, 22.0),
                egui::TextEdit::singleline(&mut nr.label).hint_text("Label"));
            ui.add_sized(Vec2::new(80.0, 22.0),
                egui::TextEdit::singleline(&mut nr.color).hint_text("#RRGGBB"));
            ui.checkbox(&mut nr.can_stream, "stream");
            ui.checkbox(&mut nr.can_upload, "upload");
            ui.checkbox(&mut nr.can_voice, "voice");
            ui.checkbox(&mut nr.can_image_share, "image");
            ui.checkbox(&mut nr.can_file_share, "file");
            // Per-role numeric limits (R4) — directly editable.
            ui.label(RichText::new("chars").size(theme.font_size_small).color(theme.text_muted()));
            int_input(ui, &mut nr.max_chars, 1, 1_000_000);
            ui.label(RichText::new("MB").size(theme.font_size_small).color(theme.text_muted()));
            int_input(ui, &mut nr.max_upload_mb, 1, 10_000);
            ui.label(RichText::new("kept").size(theme.font_size_small).color(theme.text_muted()));
            int_input(ui, &mut nr.max_uploads_kept, 1, 1_000);
            // base_tier is no longer a runtime indirection (R4); this
            // dropdown is a one-tap PREFILL of the historical preset
            // numbers into the three fields above (then fine-tune).
            egui::ComboBox::from_id_salt("new_role_prefill")
                .selected_text("Prefill…")
                .show_ui(ui, |ui| {
                    for (t, c, mb, k) in [
                        ("unverified", 280, 5, 4),
                        ("verified", 1000, 25, 20),
                        ("mod", 4000, 100, 100),
                        ("admin", 10000, 500, 500),
                    ] {
                        if ui.selectable_label(false, t).clicked() {
                            nr.base_tier = t.to_string();
                            nr.max_chars = c;
                            nr.max_upload_mb = mb;
                            nr.max_uploads_kept = k;
                        }
                    }
                });
            if widgets::Button::primary("Create").show(ui, theme) {
                create_clicked = true;
            }
        });
    }

    // Apply pending actions after the immutable `roles` borrow ends.
    if let Some(role) = pending_save {
        send_role_upsert(state, &role);
        state.roles_drafts.remove(&role.id); // re-seed from fresh role_list
    }
    if let Some(ss) = pending_master_save {
        // Same payload + behavior as the Server-policy "Save Changes"
        // button (clear the draft so it re-seeds from the relay's echo).
        send_server_settings_update(state, &ss);
        state.server_settings_draft = None;
    }
    if let Some(id) = pending_delete {
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                client.send(&serde_json::json!({ "type": "role_delete", "id": id }).to_string());
            }
        }
        state.roles_drafts.remove(&id);
    }
    if create_clicked {
        let nr = state.new_role_draft.clone();
        let id_ok = !nr.id.trim().is_empty()
            && nr.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if id_ok && !nr.label.trim().is_empty() {
            send_role_upsert(state, &nr);
            // Reset the form (keep sensible defaults).
            state.new_role_draft = {
                let mut r = crate::relay::storage::RoleDef::default();
                r.id = String::new();
                r.label = String::new();
                r.color = "#7E57C2".to_string();
                r.trust_level = 1;
                r.built_in = false;
                r.can_upload = true;
                r.can_voice = true;
                r.base_tier = "verified".to_string();
                r.sort_order = 50;
                r
            };
        } else {
            state.server_settings_status =
                "Role id must be non-empty alphanumeric (a-z 0-9 _ -) and have a label.".into();
        }
    }
}

/// Server Settings → Services (v0.262.16, docs/design/services-toggles.md).
///
/// One cohesive table per backing OS daemon: the SOFT feature gate (a
/// `server_settings` bool the relay reads at runtime — instant, no
/// restart) plus the OPTIONAL hard daemon Start/Stop so the operator
/// reclaims resources without SSHing the VPS. Soft toggles edit the
/// SAME shared `server_settings_draft` + `send_server_settings_update`
/// helper the Server-policy / Server-master row use (one builder, can't
/// drift). Daemon control goes through the tightly-allowlisted relay
/// bridge (`crate::relay::services`) via `service_control`.
fn draw_services_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Services");
    widgets::body_hint(
        ui, theme,
        "Each feature has a SOFT gate (the relay instantly stops offering \
         it, no restart, persists) AND an optional backing OS daemon you \
         can Start/Stop here to reclaim RAM instead of SSHing the VPS. \
         Effective = soft gate ON. Stopping a daemon takes effect now but \
         does NOT survive a server reboot (v1), the soft gate is what \
         persists the decision (a stopped-then-rebooted daemon is harmless: \
         the soft gate keeps the feature off). The Voice soft gate is the \
         same setting as the Server-master Voice column.",
    );
    ui.add_space(theme.spacing_sm);

    // Shared draft — identical source the Server-policy Save + the
    // Server-master row use, so soft toggles never drift.
    let cached: crate::relay::storage::ServerSettings =
        state.server_settings.clone().unwrap_or_default();
    if state.server_settings_draft.is_none() {
        state.server_settings_draft = Some(cached.clone());
    }
    let mut draft = state.server_settings_draft.clone().unwrap_or_else(|| cached.clone());

    let services = state.service_state.clone();
    if services.is_empty() {
        widgets::body_hint(
            ui, theme,
            "No daemon status yet, click \"Refresh status\" (admin only; \
             the relay replies privately).",
        );
    }

    let mut pending: Option<(String, &'static str)> = None;
    let mut refresh = false;

    egui::Grid::new("server_services")
        .num_columns(5)
        .spacing([theme.spacing_xl, theme.spacing_md])
        .striped(true)
        .show(ui, |ui| {
            let hdr = |ui: &mut egui::Ui, t: &str| {
                ui.label(
                    RichText::new(t)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary())
                        .strong(),
                );
            };
            hdr(ui, "Service");
            hdr(ui, "Feature (soft)");
            hdr(ui, "Daemon");
            hdr(ui, "");
            hdr(ui, "");
            ui.end_row();

            for svc in &services {
                ui.label(
                    RichText::new(&svc.label)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
                // Soft gate → the shared draft field for this service.
                match svc.id.as_str() {
                    "voice" => { ui.checkbox(&mut draft.voice_channels_enabled, ""); }
                    "p2p" => { ui.checkbox(&mut draft.p2p_distribution_enabled, ""); }
                    _ => {
                        ui.label(
                            RichText::new(", ")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                }
                // Live daemon status (from the relay's snapshot).
                let (txt, col) = if svc.daemon_active {
                    ("running", theme.success())
                } else {
                    ("stopped", theme.text_muted())
                };
                ui.label(
                    RichText::new(format!(
                        "{txt} (boots: {})",
                        if svc.daemon_enabled { "yes" } else { "no" }
                    ))
                    .size(theme.font_size_small)
                    .color(col),
                );
                if widgets::Button::secondary("Start").show(ui, theme) {
                    pending = Some((svc.id.clone(), "start"));
                }
                if widgets::Button::danger("Stop").show(ui, theme) {
                    pending = Some((svc.id.clone(), "stop"));
                }
                ui.end_row();
            }
        });

    // Persist the (possibly-edited) draft back to state so the soft
    // checkbox isn't reverted next frame.
    state.server_settings_draft = Some(draft.clone());

    ui.add_space(theme.spacing_sm);
    let dirty = draft != cached;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(dirty, |ui| {
            if widgets::Button::primary("Save feature toggles")
                .tooltip("Persist the soft gates server-wide (same path as \
                          the Server-policy Save). Broadcasts to all clients.")
                .show(ui, theme)
            {
                send_server_settings_update(state, &draft);
                state.server_settings_draft = None;
            }
        });
        ui.add_space(theme.spacing_sm);
        if widgets::Button::secondary("Refresh status")
            .tooltip("Ask the relay for the current daemon state.")
            .show(ui, theme)
        {
            refresh = true;
        }
    });

    if let Some((svc, action)) = pending {
        send_service_control(state, &svc, action);
    }
    if refresh {
        send_service_control(state, "", "refresh");
    }
}

/// Send a `service_control` WS message (admin-only on the relay). The
/// relay re-checks admin + allowlists service/action; these strings are
/// never used as shell/unit args server-side (see crate::relay::services).
fn send_service_control(state: &GuiState, service: &str, action: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({
                "type": "service_control",
                "service": service,
                "action": action,
            });
            client.send(&msg.to_string());
        }
    }
}

/// Build & send the full `server_settings_update` WS payload from a draft.
///
/// Called from TWO entry points that edit the SAME `ServerSettings`
/// object: the Server-policy "Save Changes" button (disk cap / PQ gate /
/// extensions) and the Roles-grid "Server master" row's Save (the 4
/// master sharing/voice/streaming kill-switches). One builder so the
/// payloads can never silently drift apart. Sets `server_settings_status`
/// (different `state` field than `ws_client`, so the disjoint borrow is
/// fine — same pattern the call sites used inline before extraction).
fn send_server_settings_update(
    state: &mut GuiState,
    draft: &crate::relay::storage::ServerSettings,
) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({
                "type": "server_settings_update",
                "max_chars_unverified": draft.max_chars_unverified,
                "max_chars_verified":   draft.max_chars_verified,
                "max_chars_mod":        draft.max_chars_mod,
                "max_chars_admin":      draft.max_chars_admin,
                "image_sharing_enabled": draft.image_sharing_enabled,
                "file_sharing_enabled":  draft.file_sharing_enabled,
                // v0.201: per-role upload caps. Legacy single
                // max_upload_mb omitted — server's v0.201 handler
                // applies the per-role values directly.
                "max_upload_mb_unverified": draft.max_upload_mb_unverified,
                "max_upload_mb_verified":   draft.max_upload_mb_verified,
                "max_upload_mb_mod":        draft.max_upload_mb_mod,
                "max_upload_mb_admin":      draft.max_upload_mb_admin,
                "voice_channels_enabled":  draft.voice_channels_enabled,
                "video_streaming_enabled": draft.video_streaming_enabled,
                "allowed_file_extensions": draft.allowed_file_extensions,
                // v0.237 server-wide disk cap + v0.238 per-role FIFO.
                "max_total_upload_mb":     draft.max_total_upload_mb,
                "max_uploads_per_user_unverified": draft.max_uploads_per_user_unverified,
                "max_uploads_per_user_verified":   draft.max_uploads_per_user_verified,
                "max_uploads_per_user_mod":        draft.max_uploads_per_user_mod,
                "max_uploads_per_user_admin":      draft.max_uploads_per_user_admin,
                // PQ Increment 3: gated hard-enforcement toggle.
                "require_pq_signatures":           draft.require_pq_signatures,
                // Server→Services (v0.262.16): P2P-distribution soft gate.
                "p2p_distribution_enabled":        draft.p2p_distribution_enabled,
                // Server description shown in the launcher (v0.478.1).
                "server_description":              draft.server_description,
                // Server display name (v0.480).
                "server_name":                    draft.server_name,
            });
            client.send(&msg.to_string());
            state.server_settings_status = "Server policy update sent.".into();
        } else {
            state.server_settings_status = "Not connected, can't save.".into();
        }
    }
}

/// Send a `role_upsert` WS message for the given role definition.
fn send_role_upsert(state: &GuiState, role: &crate::relay::storage::RoleDef) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            // Serialize the RoleDef (its serde shape matches the relay's).
            if let Ok(role_json) = serde_json::to_value(role) {
                let msg = serde_json::json!({ "type": "role_upsert", "role": role_json });
                client.send(&msg.to_string());
            }
        }
    }
}

/// Local hex→Color32 for the roles editor swatch (server_settings.rs has
/// no access to chat.rs::parse_role_color). #RRGGBB, theme fallback.
fn parse_role_color_ss(hex: &str, theme: &Theme) -> Color32 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b); // theme-exempt: data-driven role color
        }
    }
    theme.text_muted()
}

/// Compact int input with min/max bounds. Used for char-limit + upload-MB rows.
fn int_input(ui: &mut egui::Ui, value: &mut i64, min: i64, max: i64) {
    let mut text = value.to_string();
    let resp = ui.add(
        egui::TextEdit::singleline(&mut text)
            .desired_width(120.0)
            .char_limit(10),
    );
    if resp.changed() {
        // Permissive parse — empty / non-numeric leaves the value unchanged
        // so the user can keep typing. Clamp on commit.
        if let Ok(n) = text.parse::<i64>() {
            *value = n.clamp(min, max);
        }
    }
}

fn row_button<F: FnOnce()>(
    ui: &mut egui::Ui,
    theme: &Theme,
    cell_width: f32,
    btn: widgets::Button,
    on_click: F,
) {
    ui.allocate_ui_with_layout(
        Vec2::new(cell_width, CHANNEL_ROW_H),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            // Force the button to match the row height exactly.
            // Button::min_height overrides the size-based default,
            // so a 26-tall button slots cleanly into a 26-tall cell.
            if btn.size(widgets::ButtonSize::Small).min_height(CHANNEL_ROW_H).show(ui, theme) {
                on_click();
            }
        },
    );
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
        Vec2::new(cell_width, CHANNEL_ROW_H),
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| {
            if widgets::custom_checkbox(ui, theme, value) {
                *row_changed = true;
            }
        },
    );
}

/// Channel grid header. Labels live in fixed-width slots matching
/// `CHANNEL_COL_WIDTHS`, and we use `add_sized` so the label's drawn box
/// actually fills the column (egui's `allocate_ui_with_layout` +
/// `ui.label` collapses the inner widget to its text width and lets the
/// next cell crowd in).
///
/// Each header label gets a hover tooltip explaining what the column
/// does — operator 2026-05-08: "we should provide more information about
/// what each option does." Hovering Read-only / Voice / Federated tells
/// the admin exactly what flipping that checkbox will do.
fn channel_grid_header(ui: &mut egui::Ui, theme: &Theme) {
    // Each cell is (label, tooltip). Save and Delete columns are blank
    // headers since the buttons themselves carry their own tooltips.
    let cells: [(&str, &str); 7] = [
        ("Name",        "The channel id used in chat (#name). Renaming updates the display \
                         label only, the underlying id stays the same so existing message \
                         references keep working."),
        ("Description", "One-line summary shown next to the channel name in the chat header."),
        ("Read-only",   "On: only mods + admins can post here. Everyone can still read. \
                         Useful for #announcements style channels."),
        ("Voice",       "On: voice icon appears next to the channel so members can join a \
                         voice call. Off: text-only, voice icon hidden."),
        ("Federated",   "On: messages gossip to peer servers, reach beyond this server, \
                         history mirrored on peers (survives this VPS going down), \
                         censorship-resistant, discoverable by other servers' members. \
                         Off: local-only, faster + fully private, but a single point \
                         of failure."),
        ("",            ""),
        ("",            ""),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, (label, tip)) in cells.iter().enumerate() {
            let w = CHANNEL_COL_WIDTHS[i];
            // Columns 2 (Read-only) and 4 (Federated) get a painted icon
            // before the label so the flag is recognizable at a glance
            // (operator request 2026-05-15 — eye for read-only, node-graph
            // for federated). Other columns: plain label as before.
            let icon_fn: Option<fn(&egui::Painter, egui::Rect, Color32)> = match i {
                2 => Some(crate::gui::widgets::icons::paint_eye),
                4 => Some(crate::gui::widgets::icons::paint_federation),
                _ => None,
            };
            if let Some(paint) = icon_fn {
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(w, CHANNEL_ROW_H),
                    egui::Sense::hover(),
                );
                let icon_sz = (CHANNEL_ROW_H * 0.72).min(16.0);
                let icon_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 2.0, rect.center().y - icon_sz / 2.0),
                    Vec2::splat(icon_sz),
                );
                paint(ui.painter(), icon_rect, theme.text_muted());
                ui.painter().text(
                    egui::pos2(icon_rect.right() + 4.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    *label,
                    egui::FontId::proportional(theme.font_size_small),
                    theme.text_muted(),
                );
                if !tip.is_empty() {
                    resp.on_hover_text(*tip);
                }
            } else {
                let txt = RichText::new(*label)
                    .size(theme.font_size_small)
                    .color(theme.text_muted())
                    .strong();
                let resp = ui.add_sized(Vec2::new(w, CHANNEL_ROW_H), egui::Label::new(txt));
                if !tip.is_empty() {
                    resp.on_hover_text(*tip);
                }
            }
        }
    });
}

// ── Members Tab ─────────────────────────────────────────────────────────────

/// Members tab — list of server / group members with role + actions.
/// Spreadsheet-style; v0.188 uses the existing slash-command surface
/// (kick / mute / ban / promote) per row.
fn draw_members_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, is_mod: bool) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(960.0);
        ui.with_layout(egui::Layout::top_down(Align::Min), |ui| {
            widgets::subsection_label(ui, theme, "Members");
            widgets::body_hint(
                ui, theme,
                "Member roster will populate from the relay. Per-row actions \
                 (Mute / Kick / Ban / Promote) trigger the existing slash-command \
                 flow. v0.188 ships the layout; full inline actions land in v0.189.",
            );
            ui.add_space(theme.spacing_md);

            // Header
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let cols = [("Name", 160.0), ("DID", 200.0), ("Role", 90.0), ("Joined", 110.0), ("Actions", 200.0)];
                for (label, w) in cols {
                    ui.add_sized(
                        Vec2::new(w, CHANNEL_ROW_H),
                        egui::Label::new(RichText::new(label).color(theme.text_muted()).strong().size(theme.font_size_small)),
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
                    RichText::new("No members loaded yet, wait for the relay roster sync.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted())
                        .italics(),
                );
            } else {
                for (name, key) in peers.iter().take(50) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_sized(
                            Vec2::new(160.0, CHANNEL_ROW_H),
                            egui::Label::new(RichText::new(name).color(theme.text_primary())),
                        );
                        let short = if key.len() > 16 {
                            format!("{}…{}", &key[..6], &key[key.len()-6..])
                        } else { key.clone() };
                        ui.add_sized(
                            Vec2::new(200.0, CHANNEL_ROW_H),
                            egui::Label::new(RichText::new(short).monospace().color(theme.text_muted()).size(theme.font_size_small)),
                        );
                        ui.add_sized(
                            Vec2::new(90.0, CHANNEL_ROW_H),
                            egui::Label::new(RichText::new("user").color(theme.text_secondary()).size(theme.font_size_small)),
                        );
                        ui.add_sized(
                            Vec2::new(110.0, CHANNEL_ROW_H),
                            egui::Label::new(RichText::new(", ").color(theme.text_muted()).size(theme.font_size_small)),
                        );
                        ui.add_sized(
                            Vec2::new(200.0, CHANNEL_ROW_H),
                            egui::Label::new(RichText::new("(actions in v0.189)").color(theme.text_muted()).italics().size(theme.font_size_small)),
                        );
                    });
                }
            }
        });
    });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn kv_row(ui: &mut egui::Ui, theme: &Theme, key: &str, value: String) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(theme.settings_label_width, ui.spacing().interact_size.y),
            Layout::left_to_right(Align::Center),
            |ui| {
                // set_min_width PINS the label column. Without it
                // allocate_ui_with_layout shrinks to the label's text
                // width, so the value crowds the label and adjacent
                // rows don't align (operator bug 2026-05-16). Token,
                // not a magic 160 (settings_label_width = ui-system.md).
                ui.set_min_width(theme.settings_label_width);
                ui.label(
                    RichText::new(key)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            },
        );
        ui.add_space(theme.spacing_md); // gutter so label/value never collide
        ui.label(
            RichText::new(value)
                .size(theme.font_size_body)
                .color(theme.text_primary())
                .monospace(),
        );
    });
    ui.add_space(theme.spacing_xs);
}

/// Like `kv_row`, but the KEY label gets an Alt+hover dictionary
/// tooltip. `glossary_term` is looked up case-insensitively in
/// `data/glossary.json`. If the term isn't in the glossary the row
/// renders identically to kv_row — no warning, no breakage.
/// Foundation for the in-app docs system (v0.195.0); incremental
/// adoption across the app follows.
fn kv_row_with_definition(ui: &mut egui::Ui, theme: &Theme, key: &str, glossary_term: &str, value: String) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(theme.settings_label_width, ui.spacing().interact_size.y),
            Layout::left_to_right(Align::Center),
            |ui| {
                // Pin the column (see kv_row) so glossary rows align
                // with plain ones and the value never crowds the label.
                ui.set_min_width(theme.settings_label_width);
                // We render the key text inline so it picks up the
                // standard kv_row styling, then call definition_text
                // separately — the widget handles the Alt+hover tooltip.
                let label_resp = ui.label(
                    RichText::new(key)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
                // Manual Alt+hover (mirrors widgets::definition_text but
                // applied to an existing Response so we keep kv_row's
                // exact font/size/color).
                let alt = ui.ctx().input(|i| i.modifiers.alt);
                if alt {
                    if let Some(entry) = crate::gui::glossary::glossary().lookup(glossary_term) {
                        let entry_term = entry.term.clone();
                        let entry_def = entry.definition.clone();
                        let entry_link = entry.link.clone();
                        label_resp.on_hover_ui(move |ui| {
                            ui.set_max_width(360.0);
                            ui.label(RichText::new(&entry_term).strong());
                            ui.add_space(4.0);
                            ui.label(&entry_def);
                            if !entry_link.is_empty() {
                                ui.add_space(4.0);
                                ui.label(RichText::new(format!("More: {}", &entry_link)).italics().small());
                            }
                        });
                    }
                }
            },
        );
        ui.add_space(theme.spacing_md); // gutter, matches kv_row
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

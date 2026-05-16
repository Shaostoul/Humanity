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
        // Identity is your Ed25519 public key. Alt+hover "Ed25519" or
        // "identity" to see what those mean.
        kv_row_with_definition(ui, theme, "Your identity", "ed25519", short_key(&state.profile_public_key));
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
                              Reversible — click Unmute to restore.")
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
                              persistent block — admin-only).")
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
                          /reports slash command — UI surface lands in v0.194+).")
                .show(ui, theme)
            {
                send_slash(state, "/reports");
                state.server_settings_status = "Sent: /reports — check the active channel for results.".into();
            }
        });
    });
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

        // ── Registration ──
        widgets::subsection_label(ui, theme, "Registration");
        widgets::body_hint(
            ui, theme,
            "Lockdown blocks NEW registrations server-wide. Existing members keep full \
             access — useful during a spam wave or when switching to invite-only mode. \
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
                state.server_settings_status = "Sent: /lockdown — registration toggle requested.".into();
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::primary("Generate invite code")
                .tooltip("Create a single-use invite code. Code appears in the chat channel. \
                          Share it with one person — they can register even during lockdown.")
                .show(ui, theme)
            {
                send_slash(state, "/invite");
                state.server_settings_status = "Sent: /invite — code will appear in the active channel.".into();
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

        // ── User management ──
        widgets::subsection_label(ui, theme, "User management");
        widgets::body_hint(
            ui, theme,
            &format!(
                "Acts on the username typed in the Moderator section above (currently: {}). \
                 Verify gives a green check next to their name. Promote to mod grants \
                 moderator-tier permissions. Ban is permanent — they can't rejoin without \
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
                              and view the report queue. Reversible — promote to admin to allow \
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
        });

        ui.add_space(theme.spacing_md);
        ui.separator();
        ui.add_space(theme.spacing_sm);

        // ── Banned users (v0.245): list + per-row Unban ──
        draw_banned_admin(ui, theme, state);
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
         were is captured here. Click Unban to restore access — the change takes \
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
                "(unknown — banned before name capture)".to_string()
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
        state.server_settings_status = "Sent unban — the ban list will refresh.".into();
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

/// Format a Unix-ms ban timestamp as `YYYY-MM-DD HH:MM` (UTC). Uses
/// the same chrono-free civil-date math as chat.rs::format_full_timestamp
/// (Howard Hinnant's algorithm) so we don't add a dependency.
fn format_ban_date(ms: i64) -> String {
    if ms <= 0 {
        return "—".to_string();
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
         permanent — messages are kept but the channel goes away.",
    );
    ui.add_space(theme.spacing_xs);
    widgets::body_hint(
        ui, theme,
        "Read-only (eye icon): only mods + admins can post; everyone can read. \
         Voice: enables the voice-call icon next to the channel — disable to make \
         it text-only.",
    );
    ui.add_space(theme.spacing_xs);
    widgets::body_hint(
        ui, theme,
        "Federated (node icon): messages here gossip to peer servers in the \
         federation network. Beyond cross-server read/reply, this gives you: \
         (1) Reach — your community's posts are visible to people who never \
         joined THIS server; (2) Resilience — the conversation + history is \
         mirrored on peer servers, so if this VPS goes down or is seized the \
         thread survives; (3) Censorship-resistance — no single operator (not \
         even you) can silently erase a federated channel everywhere; \
         (4) Discovery — members on other servers can find and join the \
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

    for ch in &channels {
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
        });
        let _ = row_changed; // visual cue could go here; keep minimal for v1
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
        "Per-role message length limits, file / image / voice / streaming toggles, and \
         upload size cap. Apply to every member of this server. Changes broadcast to \
         all connected clients on Save.",
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
            "Never modified — defaults active.".to_string()
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
        widgets::form_row(ui, theme, "Max chars — unverified (default 280)", |ui| {
            int_input(ui, &mut draft.max_chars_unverified, 1, 1_000_000);
        });
        widgets::form_row(ui, theme, "Max chars — verified (default 1000)", |ui| {
            int_input(ui, &mut draft.max_chars_verified, 1, 1_000_000);
        });
        widgets::form_row(ui, theme, "Max chars — moderator (default 4000)", |ui| {
            int_input(ui, &mut draft.max_chars_mod, 1, 1_000_000);
        });
        widgets::form_row(ui, theme, "Max chars — admin (default 10000)", |ui| {
            int_input(ui, &mut draft.max_chars_admin, 1, 1_000_000);
        });
        widgets::form_row(ui, theme, "Max upload MB — unverified (default 5)", |ui| {
            int_input(ui, &mut draft.max_upload_mb_unverified, 1, 10_000);
        });
        widgets::form_row(ui, theme, "Max upload MB — verified (default 25)", |ui| {
            int_input(ui, &mut draft.max_upload_mb_verified, 1, 10_000);
        });
        widgets::form_row(ui, theme, "Max upload MB — moderator (default 100)", |ui| {
            int_input(ui, &mut draft.max_upload_mb_mod, 1, 10_000);
        });
        widgets::form_row(ui, theme, "Max upload MB — admin (default 500)", |ui| {
            int_input(ui, &mut draft.max_upload_mb_admin, 1, 10_000);
        });
        widgets::form_row(ui, theme, "Image sharing", |ui| {
            ui.checkbox(&mut draft.image_sharing_enabled, "");
        });
        widgets::form_row(ui, theme, "File sharing", |ui| {
            ui.checkbox(&mut draft.file_sharing_enabled, "");
        });
        widgets::form_row(ui, theme, "Voice channels", |ui| {
            ui.checkbox(&mut draft.voice_channels_enabled, "");
        });
        widgets::form_row(ui, theme, "Video streaming", |ui| {
            ui.checkbox(&mut draft.video_streaming_enabled, "");
        });
        widgets::form_row(ui, theme, "Allowed extensions (csv, blank=any)", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut draft.allowed_file_extensions)
                    .desired_width(280.0)
                    .hint_text("png,jpg,pdf,…"),
            );
        });
        widgets::form_row(ui, theme, "Uploads kept — unverified, FIFO (default 4)", |ui| {
            int_input(ui, &mut draft.max_uploads_per_user_unverified, 1, 1_000);
        });
        widgets::form_row(ui, theme, "Uploads kept — verified, FIFO (default 20)", |ui| {
            int_input(ui, &mut draft.max_uploads_per_user_verified, 1, 1_000);
        });
        widgets::form_row(ui, theme, "Uploads kept — moderator, FIFO (default 100)", |ui| {
            int_input(ui, &mut draft.max_uploads_per_user_mod, 1, 1_000);
        });
        widgets::form_row(ui, theme, "Uploads kept — admin, FIFO (default 500)", |ui| {
            int_input(ui, &mut draft.max_uploads_per_user_admin, 1, 1_000);
        });
        widgets::form_row(ui, theme, "Total upload disk cap MB, server-wide (default 500)", |ui| {
            int_input(ui, &mut draft.max_total_upload_mb, 1, 1_000_000);
        });

        ui.add_space(theme.spacing_sm);
        // Dirty = the working draft differs from the cached relay state.
        // Save is only enabled when dirty (server-wide settings must not
        // re-broadcast a no-op to every client).
        let dirty = draft != cached;
        if dirty {
            widgets::body_hint(ui, theme, "Unsaved changes — click Save Changes to apply server-wide.");
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
                    });
                    client.send(&msg.to_string());
                    state.server_settings_status = "Server policy update sent.".into();
                } else {
                    state.server_settings_status = "Not connected — can't save.".into();
                }
            }
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
        "Roles carry capabilities. Effective permission = the server-wide \
         master toggle (in Server policy above) AND the role's capability. \
         e.g. to let family livestream WITHOUT making them moderators: turn \
         ON 'Video streaming' in Server policy, then give a role can-stream \
         here and assign it to them (click their name in chat → Role \
         dropdown). Built-in roles can't be deleted and their id / trust / \
         limit-tier are locked, but their capabilities are editable. Custom \
         roles are fully editable. A role's numeric limits (chars / upload \
         MB / uploads kept) follow its base tier.",
    );
    ui.add_space(theme.spacing_sm);

    if state.chat_roles.is_empty() {
        widgets::body_hint(ui, theme, "Waiting for the relay's role list… (connect to a server)");
        return;
    }

    let roles = state.chat_roles.clone();
    let mut pending_save: Option<crate::relay::storage::RoleDef> = None;
    let mut pending_delete: Option<String> = None;

    for role in &roles {
        let draft = state.roles_drafts
            .entry(role.id.clone())
            .or_insert_with(|| role.clone());
        let is_built_in = draft.built_in;
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // Color swatch.
            let (sw, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
            ui.painter().rect_filled(sw, 3.0, parse_role_color_ss(&draft.color, theme));
            // Label (custom roles editable; built-ins fixed label shown).
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
            // Color hex.
            ui.add_sized(
                Vec2::new(80.0, 22.0),
                egui::TextEdit::singleline(&mut draft.color).hint_text("#RRGGBB"),
            );
            // Capabilities (always editable, even on built-ins).
            ui.checkbox(&mut draft.can_stream, "stream");
            ui.checkbox(&mut draft.can_upload, "upload");
            ui.checkbox(&mut draft.can_voice, "voice");
            // base_tier (custom only — built-in is locked server-side).
            if !is_built_in {
                egui::ComboBox::from_id_salt(("role_tier", draft.id.as_str()))
                    .selected_text(format!("tier: {}", draft.base_tier))
                    .show_ui(ui, |ui| {
                        for t in ["unverified", "verified", "mod", "admin"] {
                            if ui.selectable_label(draft.base_tier == t, t).clicked() {
                                draft.base_tier = t.to_string();
                            }
                        }
                    });
            } else {
                ui.label(
                    RichText::new(format!("tier: {}", draft.base_tier))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
            if widgets::Button::primary("Save").show(ui, theme) {
                pending_save = Some(draft.clone());
            }
            if !is_built_in && widgets::Button::danger("Delete").show(ui, theme) {
                pending_delete = Some(draft.id.clone());
            }
        });
    }

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
            egui::ComboBox::from_id_salt("new_role_tier")
                .selected_text(format!("tier: {}", nr.base_tier))
                .show_ui(ui, |ui| {
                    for t in ["unverified", "verified", "mod", "admin"] {
                        if ui.selectable_label(nr.base_tier == t, t).clicked() {
                            nr.base_tier = t.to_string();
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
                         label only — the underlying id stays the same so existing message \
                         references keep working."),
        ("Description", "One-line summary shown next to the channel name in the chat header."),
        ("Read-only",   "On: only mods + admins can post here. Everyone can still read. \
                         Useful for #announcements style channels."),
        ("Voice",       "On: voice icon appears next to the channel so members can join a \
                         voice call. Off: text-only — voice icon hidden."),
        ("Federated",   "On: messages gossip to peer servers — reach beyond this server, \
                         history mirrored on peers (survives this VPS going down), \
                         censorship-resistant, discoverable by other servers' members. \
                         Off: local-only — faster + fully private, but a single point \
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
                    RichText::new("No members loaded yet — wait for the relay roster sync.")
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
                            egui::Label::new(RichText::new("—").color(theme.text_muted()).size(theme.font_size_small)),
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

/// Like `kv_row`, but the KEY label gets an Alt+hover dictionary
/// tooltip. `glossary_term` is looked up case-insensitively in
/// `data/glossary.json`. If the term isn't in the glossary the row
/// renders identically to kv_row — no warning, no breakage.
/// Foundation for the in-app docs system (v0.195.0); incremental
/// adoption across the app follows.
fn kv_row_with_definition(ui: &mut egui::Ui, theme: &Theme, key: &str, glossary_term: &str, value: String) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(160.0, ui.spacing().interact_size.y),
            Layout::left_to_right(Align::Center),
            |ui| {
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

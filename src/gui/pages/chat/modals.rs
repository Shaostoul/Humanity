//! The chat page's OVERLAYS: everything drawn ON TOP of the three panels.
//!
//! One cluster because they share a shape, not a subject. Each of these is a
//! whole surface that appears when a `state.show_*` / `*_open` flag is set,
//! takes over the pointer, does one job, and closes itself; none of them is
//! part of the rail-centre-rail layout, and `draw` calls them in a single block
//! after the panels are painted. Keeping them together is what makes that block
//! readable as "and then, the overlays".
//!
//! Extracted VERBATIM from `gui/pages/chat.rs` (file-size ratchet), which stood
//! at 9,053 lines against an 8,000 budget. Fourth and last of the clusters out
//! in that pass. The only delta beyond the move is seven `pub(super)` prefixes,
//! each commented where it is declared, on overlays whose callers stayed behind.
//!
//! WHAT IS IN HERE:
//!
//!   * `draw_pins_modal`, `draw_search_modal` - reading the channel: what is
//!     pinned, and finding an old message.
//!   * `draw_user_modal` and its `send_mod_action` - one person: their profile,
//!     the follow control, and the moderation and admin buttons. The action
//!     helper is here because it exists only to give those six buttons one
//!     well-formed `mod_action` message instead of six copies of the JSON.
//!   * `draw_add_server_modal`, `draw_edit_channel_modal` - changing where you
//!     are connected and what a channel is called.
//!   * `draw_create_group_modal`, `draw_join_group_modal` - the two ways into a
//!     P2P group. The work they trigger lives in `p2p_groups.rs`; these are the
//!     forms.
//!   * `draw_commons_info_modal` - the "what is the Commons?" explainer, opened
//!     from the ? on the Commons header. Inline-first: the explanation lives on
//!     the thing it explains.
//!   * `draw_help_modal` - the slash-command reference.
//!   * `draw_incoming_call_modal` and `draw_call_bar` - the voice call, ringing
//!     and then in progress. The bar is not a modal but it is an overlay by the
//!     same rule: `draw` paints it over the panels, and lib.rs paints it over
//!     the world so a call stays visible outside this page.
//!
//! Takes `use super::*` like the page's other children, so the dialog widget,
//! the timestamp helpers, `refresh_p2p_groups` and the page's imports all
//! arrive without any of them being widened.

use super::*;

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`; it was private when the two lived in one file.
pub(super) fn draw_pins_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
            RichText::new(format!("{} pin(s), pin/unpin via the 📌 button on each message.", pins.len()))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);

        // Mods/admins get a per-pin Unpin button (v0.722 commands-to-buttons
        // pass — /unpin <N> previously had no clickable path).
        let can_unpin = {
            let vr = viewer_role(state);
            vr == "admin" || vr == "moderator" || vr == "mod"
        };
        let mut unpin_index: Option<usize> = None;
        ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
            for (pi, p) in pins.iter().enumerate() {
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
                            if can_unpin {
                                if widgets::Button::ghost("Unpin").show(ui, theme) {
                                    // The relay's /unpin is 1-based.
                                    unpin_index = Some(pi + 1);
                                }
                            }
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
        if let Some(n) = unpin_index {
            send_slash_command(state, &format!("/unpin {}", n));
        }
    });
    if !open {
        state.chat_pins_open = false;
    }
}

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`.
pub(super) fn draw_search_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
                RichText::new("No results yet, type a query and hit Search.")
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

// ─────────────────────────────── User Profile Modal ────────────────────────

pub(crate) fn draw_user_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if !state.chat_user_modal_open { return; }

    let name = state.chat_user_modal_name.clone();
    let key = state.chat_user_modal_key.clone();

    // ── Derive everything read-only up front (one borrow of state), so the
    // dialog closure only needs state for the action mutations. ──
    let user_entry = state.chat_users.iter().find(|u| u.public_key == key);
    let user_role = user_entry.map(|u| u.role.clone()).unwrap_or_default();
    let user_status = user_entry.map(|u| u.status.clone()).unwrap_or_else(|| "offline".to_string());
    // Empty status but present in the roster ⇒ treat as online.
    let effective_status = if user_status.is_empty() { "online".to_string() } else { user_status };

    // Relationship (both directions, v0.721 — mirrors the web profile's
    // Friends / Following / Follows you).
    let is_following = state.chat_following_keys.contains(&key)
        || state.chat_friends.iter().any(|f| f.public_key == key);
    let follows_me = state.chat_followers.contains(&key);

    // Voice call (v0.705). Disabled when we're already in any call state or
    // calling ourselves; the ring goes out and the callee (web or native)
    // sees the incoming-call prompt.
    let can_call = key != state.profile_public_key
        && state.call_active.is_none()
        && state.call_outgoing.is_none()
        && state.call_incoming.is_none()
        && state.voice_active_room.is_none();

    let my_role = viewer_role(state);
    let is_mod = my_role == "moderator" || my_role == "mod" || my_role == "admin";
    let is_admin = my_role == "admin";
    let roles = state.chat_roles.clone();

    let mut open = true;
    let mut close_after = false;

    widgets::dialog(ctx, theme, "user_profile_dialog", "Profile", &mut open, |ui| {
        ui.set_min_width(320.0);

        // ── Identity header (avatar, name, status), all centered ──
        ui.vertical_centered(|ui| {
            // Avatar: filled disc in the name's hash color, wrapped in a soft
            // ring of the same hue so it reads as an intentional badge.
            let icon_color = name_color(&name);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(80.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 38.0, icon_color.gamma_multiply(0.30));
            ui.painter().circle_filled(rect.center(), 32.0, icon_color);
            let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &initial,
                egui::FontId::proportional(32.0),
                Color32::WHITE,
            );

            ui.add_space(theme.spacing_sm);

            // Name + role badge
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&name)
                        .size(theme.font_size_heading)
                        .color(theme.text_primary())
                        .strong(),
                );
                if !user_role.is_empty() && user_role != "member" {
                    draw_role_badges(ui, theme, &user_role, &roles);
                }
            });

            ui.add_space(4.0);

            // Status dot + label
            let (dot_color, status_text) = match effective_status.as_str() {
                "offline" => (theme.text_muted(), "Offline"),
                "away" => (theme.warning(), "Away"),
                "busy" | "dnd" => (theme.danger(), "Do Not Disturb"),
                "streaming" => (theme.accent(), "Live"),
                _ => (theme.success(), "Online"),
            };
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, dot_color);
                ui.label(
                    RichText::new(status_text)
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });

            // Relationship line (plain words — U+2190 "←" is tofu in the app font).
            let relationship = match (is_following, follows_me) {
                (true, true) => Some(("Friends", theme.success())),
                (true, false) => Some(("You follow them", theme.text_secondary())),
                (false, true) => Some(("Follows you", theme.warning())),
                (false, false) => None,
            };
            if let Some((txt, color)) = relationship {
                ui.add_space(2.0);
                ui.label(RichText::new(txt).size(theme.font_size_small).color(color));
            }
        });

        ui.add_space(theme.spacing_md);

        // ── Public key, in a subtle framed row with an inline Copy ──
        let display_key = if key.len() > 16 {
            format!("{}...{}", &key[..8], &key[key.len()-8..])
        } else {
            key.clone()
        };
        Frame::none()
            .fill(theme.bg_secondary())
            .rounding(Rounding::same(theme.border_radius as u8))
            .inner_margin(theme.spacing_sm)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Key")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.add_space(6.0);
                    // Monospace: a member key is compared character by
                    // character against another screen.
                    ui.label(
                        RichText::new(&display_key)
                            .monospace()
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::Button::ghost("Copy").show(ui, theme) {
                            ui.ctx().copy_text(key.clone());
                        }
                    });
                });
            });

        ui.add_space(theme.spacing_md);

        // ── Primary actions ──
        // Send DM — the most common action, full width.
        if widgets::Button::primary("Send DM").full_width().show(ui, theme) {
            let dm_channel = format!("dm:{}", key);
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
                // Local-store open (sealed-sender: no server history call).
                open_dm_conversation(state, &key);
            }
            close_after = true;
        }

        ui.add_space(theme.spacing_sm);

        // Call + Follow/Unfollow, equal width side by side.
        ui.columns(2, |cols| {
            // Voice call
            if widgets::Button::secondary("Call")
                .disabled(!can_call)
                .full_width()
                .tooltip(if can_call { "Start a voice call" } else { "Unavailable during another call" })
                .show(&mut cols[0], theme)
            {
                if let Some(ref client) = state.ws_client {
                    let _ = client.send(&serde_json::json!({
                        "type": "voice_call",
                        "from": state.profile_public_key,
                        "to": key,
                        "action": "ring",
                    }).to_string());
                }
                state.call_outgoing = Some((key.clone(), name.clone()));
                state.call_outgoing_deadline =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
                state.call_muted = false;
                close_after = true;
            }

            // Follow / Unfollow (Follow reads as a positive confirm ⇒ success green)
            if is_following {
                if widgets::Button::secondary("Unfollow").full_width().show(&mut cols[1], theme) {
                    // Follows removal (2026-08-24): a sealed control message
                    // to the peer; the server stores no edges.
                    crate::engine::dm::set_follow(state, &key, false);
                }
            } else {
                // "Follow back" when they already follow you (v0.721).
                let follow_label = if follows_me { "Follow back" } else { "Follow" };
                if widgets::Button::success(follow_label).full_width().show(&mut cols[1], theme) {
                    // Follows removal (2026-08-24): sealed control message +
                    // certificate exchange when it becomes mutual.
                    crate::engine::dm::set_follow(state, &key, true);
                    state.chat_following_keys.insert(key.clone());
                    if !state.chat_friends.iter().any(|f| f.public_key == key) {
                        if let Some(u) = state.chat_users.iter().find(|u| u.public_key == key).cloned() {
                            state.chat_friends.push(u);
                        }
                    }
                }
            }
        });

        // (v0.845: the old "Watch Stream" button was a dead no-op — the native
        // roster carries no per-user stream URL and there's no native stream
        // viewer, so it was a false affordance. The "Live" status dot above
        // already signals that a user is streaming; the button is removed until
        // a real in-app viewer exists. Tracked in docs/UI-AUDIT.md.)

        // ── Moderation (mods + admins) ──
        if is_mod {
            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Moderation")
                    .size(theme.font_size_small)
                    .color(theme.warning())
                    .strong(),
            );
            ui.add_space(4.0);
            ui.columns(2, |cols| {
                if widgets::Button::secondary("Mute").full_width().show(&mut cols[0], theme) {
                    send_mod_action(state, "mute", &key, &name);
                }
                if widgets::Button::secondary("Kick").full_width().show(&mut cols[1], theme) {
                    send_mod_action(state, "kick", &key, &name);
                }
            });
        }

        // ── Admin ──
        if is_admin {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Admin")
                    .size(theme.font_size_small)
                    .color(theme.danger())
                    .strong(),
            );
            ui.add_space(4.0);
            ui.columns(2, |cols| {
                if widgets::Button::danger("Ban").full_width().show(&mut cols[0], theme) {
                    send_mod_action(state, "ban", &key, &name);
                }
                let target_is_mod = user_role == "moderator" || user_role == "mod";
                let mod_btn_label = if target_is_mod { "Unmod" } else { "Mod" };
                if widgets::Button::secondary(mod_btn_label).full_width().show(&mut cols[1], theme) {
                    send_mod_action(state, if target_is_mod { "unmod" } else { "mod" }, &key, &name);
                }
            });

            // Verify quick action (v0.687). Only plain members can be verified
            // (the relay refuses elevated targets — v0.692 an unguarded click
            // silently demoted mods/admins via role overwrite).
            let target_is_verified = user_role == "verified";
            let verify_applicable = target_is_verified
                || matches!(user_role.as_str(), "" | "member" | "user" | "unverified");
            if verify_applicable {
                ui.add_space(4.0);
                let verify_label = if target_is_verified { "Unverify" } else { "Verify" };
                if widgets::Button::secondary(verify_label).full_width().show(ui, theme) {
                    send_mod_action(state, if target_is_verified { "unverify" } else { "verify" }, &key, &name);
                }
            }

            // ── Role assignment dropdown (roles Phase R2) ──
            // Lists every role from the relay's role_list; picking one sends
            // set_user_role. The operator's chosen path for custom roles (e.g.
            // give dad a "Family" role with can_stream). Mod/Unmod stay as
            // quick shortcuts above.
            if !roles.is_empty() {
                ui.add_space(theme.spacing_sm);
                let current_label = roles
                    .iter()
                    .find(|r| r.id == user_role)
                    .map(|r| r.label.clone())
                    .unwrap_or_else(|| {
                        if user_role.is_empty() { "Unverified".to_string() } else { user_role.clone() }
                    });
                let mut picked: Option<String> = None;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Role")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    egui::ComboBox::from_id_salt(("user_role_combo", key.as_str()))
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for r in &roles {
                                // Each label in its own badge color so custom
                                // roles read distinctly; selectable_label's
                                // highlight marks the current selection (no
                                // glyph needed — keeps icon_glyph_lint happy).
                                let sel = r.id == user_role;
                                if ui.selectable_label(
                                    sel,
                                    RichText::new(&r.label).color(parse_role_color(&r.color, theme)),
                                ).clicked() {
                                    picked = Some(r.id.clone());
                                }
                            }
                        });
                });
                if let Some(rid) = picked {
                    if rid != user_role {
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {
                                let msg = serde_json::json!({
                                    "type": "set_user_role",
                                    "target": key,
                                    "role_id": rid,
                                });
                                client.send(&msg.to_string());
                            }
                        }
                    }
                }
            }
        }

        ui.add_space(theme.spacing_md);
        if widgets::Button::ghost("Close").full_width().show(ui, theme) {
            close_after = true;
        }
    });

    if !open || close_after {
        state.chat_user_modal_open = false;
    }
}

/// Send a `mod_action` WS message (mute/kick/ban/mod/unmod/verify/unverify).
/// Extracted so the moderation + admin buttons in the user modal share one
/// well-formed message shape instead of copy-pasting the JSON six times.
fn send_mod_action(state: &GuiState, action: &str, target_key: &str, target_name: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({
                "type": "mod_action",
                "action": action,
                "target": target_key,
                "target_name": target_name,
            });
            client.send(&msg.to_string());
        }
    }
}


// ─────────────────────────────── Add Server Modal ───────────────────────

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`.
pub(super) fn draw_add_server_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
                    .hint_text("(optional, derived from URL if blank)"),
            );
        });

        ui.add_space(theme.spacing_md);

        // Validation: URL must be non-empty and parse-able with http/https
        // scheme. Doesn't reach the server here — the connect attempt
        // happens later when the user clicks the new server's row.
        // Be forgiving: if the user typed a bare host with no scheme (e.g.
        // "server1.example.com"), assume https:// so the Add button doesn't
        // just grey out with no explanation. (v0.714)
        let raw = state.add_server_url_draft.trim();
        let url_owned = if !raw.is_empty()
            && !raw.starts_with("http://")
            && !raw.starts_with("https://")
            && raw.contains('.')
            && !raw.contains(' ')
        {
            format!("https://{raw}")
        } else {
            raw.to_string()
        };
        let url = url_owned.as_str();
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
                        // Persist: saved servers used to vanish on relaunch
                        // because nothing wrote them to AppConfig (2026-08-14).
                        state.settings_dirty = true;
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

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`.
pub(super) fn draw_edit_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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

        // Channel flag toggles. These mutate the local ChatChannel for instant
        // feedback; the Save button persists them to the relay below. Read the
        // current values out of chat_channels (the modal's source of truth).
        let (mut voice_enabled, mut read_only) = state.chat_channels.iter()
            .find(|c| c.id == state.edit_channel_id)
            .map(|c| (c.voice_enabled, c.read_only))
            .unwrap_or((true, false));
        if ui.checkbox(&mut voice_enabled, "Voice enabled").changed() {
            if let Some(ch) = state.chat_channels.iter_mut().find(|c| c.id == state.edit_channel_id) {
                ch.voice_enabled = voice_enabled;
            }
        }
        if ui.checkbox(&mut read_only, "Read-only (only admins/mods can post)").changed() {
            if let Some(ch) = state.chat_channels.iter_mut().find(|c| c.id == state.edit_channel_id) {
                ch.read_only = read_only;
            }
        }

        ui.add_space(theme.spacing_md);

        ui.horizontal(|ui| {
            let name_valid = !state.edit_channel_name.trim().is_empty();
            ui.add_enabled_ui(name_valid, |ui| {
                if widgets::Button::primary("Save").show(ui, theme) {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            // `channel_update` — NOT the old `channel_edit`, which
                            // the relay never had a handler for (so name, desc AND
                            // the flag toggles were all silently dropped). The
                            // relay's channel_update handler is admin-gated, applies
                            // each provided field, and rebroadcasts channel_list;
                            // omitted fields (e.g. federated) are left unchanged.
                            // Mirrors server_settings.rs's Channels-page Save.
                            let msg = serde_json::json!({
                                "type": "channel_update",
                                "channel_id": state.edit_channel_id,
                                "name": state.edit_channel_name.trim(),
                                "description": state.edit_channel_description.trim(),
                                "read_only": read_only,
                                "voice_enabled": voice_enabled,
                            });
                            client.send(&msg.to_string());
                            log::info!("Channel update: {} (read_only={read_only}, voice_enabled={voice_enabled})", state.edit_channel_id);
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

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`.
pub(super) fn draw_create_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_create_group_modal;
    widgets::dialog(ctx, theme, "create_group_dialog", "Create Group", &mut open, |ui| {
        ui.set_min_width(300.0);

        // After a successful create, this modal flips to a "share the ticket"
        // view so the user can copy/share it immediately (the most common
        // next step after creating a group).
        if let Some(ticket) = state.create_group_ticket.clone() {
            ui.label(RichText::new("✅ Group created").strong());
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new("Share this invite ticket (valid 7 days). It's signed by you, so members can join even when you're offline. Anyone with this string can join, keep it private to the people you mean to invite.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);
            let mut display = ticket.clone();
            // MONOSPACE, and of every string in this product this is the one
            // that most needs it: base64 is the only alphabet here containing
            // 0, O, I and l at once. base58 (DIDs, the Solana address) omits
            // all four by construction, public keys are lowercase hex, and
            // BIP39 is lowercase a-z, so this ticket is the sole surface where
            // those characters can be confused. Hack, already compiled in, has
            // a dotted zero and a serifed I in its default instance.
            ui.add(
                egui::TextEdit::multiline(&mut display)
                    .font(egui::FontId::monospace(theme.font_size_small))
                    .desired_width(360.0)
                    .desired_rows(3)
                    .interactive(false),
            );
            ui.add_space(theme.spacing_sm);
            ui.horizontal(|ui| {
                if widgets::Button::primary("📋 Copy ticket").show(ui, theme) {
                    ui.ctx().copy_text(ticket.clone());
                    state.create_group_status = "Ticket copied to clipboard.".to_string();
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Done").show(ui, theme) {
                    state.show_create_group_modal = false;
                    state.create_group_ticket = None;
                    state.create_group_status.clear();
                    state.new_group_name.clear();
                }
            });
            if !state.create_group_status.is_empty() {
                ui.add_space(theme.spacing_xs);
                ui.label(
                    RichText::new(&state.create_group_status)
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
        } else {
            // Initial state: name input + Create.
            widgets::form_row(ui, theme, "Group name", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_group_name)
                        .desired_width(240.0)
                        .hint_text("e.g. My Team"),
                );
            });

            // History policy for members who join later (operator-requested).
            // Plain-ASCII +/- markers keep the glyph lint happy.
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Message history for people who join later")
                    .strong()
                    .size(theme.font_size_small),
            );
            ui.add_space(theme.spacing_xs);
            ui.radio_value(&mut state.new_group_share_history, false, "Private (default)");
            ui.label(
                RichText::new(
                    "New members only see messages sent after they join.\n\
                     + Past conversations stay between who was there\n\
                     + Stronger forward secrecy, re-keys on each join\n\
                     - Newcomers start with no context",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            ui.radio_value(&mut state.new_group_share_history, true, "Shared history");
            ui.label(
                RichText::new(
                    "New members can read the full history from before they joined.\n\
                     + Newcomers get full context, good for onboarding\n\
                     - Anyone invited later can read earlier messages\n\
                     - Weaker forward secrecy, the key is not rotated on join",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            if !state.create_group_status.is_empty() {
                ui.add_space(theme.spacing_xs);
                ui.label(
                    RichText::new(&state.create_group_status)
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
            ui.add_space(theme.spacing_md);
            let name_valid = !state.new_group_name.trim().is_empty();
            // Pressing Enter on the name field also triggers Create — the
            // expected keyboard shortcut for a single-field "name + Create"
            // form (operator feedback 2026-05-28).
            let enter_pressed = name_valid && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut do_create = false;
            ui.horizontal(|ui| {
                ui.add_enabled_ui(name_valid, |ui| {
                    if widgets::Button::primary("Create").show(ui, theme) {
                        do_create = true;
                    }
                });
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    state.show_create_group_modal = false;
                    state.new_group_name.clear();
                    state.new_group_share_history = false;
                    state.create_group_status.clear();
                }
            });
            if do_create || enter_pressed {
                // P2P signed-object create: build group_v1 + an initial 7-day
                // creator-signed invite_v1, all via POST /api/v2/objects.
                // Replaces the legacy WS group_create path (which never
                // produced a working invite URL).
                let server_url = state.server_url.clone();
                let name = state.new_group_name.trim().to_string();
                let seed_opt = state.private_key_bytes.clone();
                match seed_opt {
                    Some(seed) => {
                        match crate::net::api_v2::create_group_and_first_invite(&server_url, &seed, &name, state.new_group_share_history) {
                            Ok((group_id, ticket)) => {
                                state.create_group_ticket = Some(ticket);
                                state.create_group_status.clear();
                                log::info!("P2P group created ({}), first invite minted", group_id);
                                crate::debug::push_debug(format!("P2P group create: {} ({})", name, group_id));
                                // Refresh the projection cache so the new group
                                // appears in the left panel when the modal closes.
                                refresh_p2p_groups(state);
                                // Auto-switch into the new group so the creator
                                // lands in it immediately and its epoch key is
                                // live right away (the keygen happens on create;
                                // entering also runs the rekey path). The ticket
                                // modal stays open over it for copying.
                                state.chat_active_channel = format!("p2pgroup:{}", group_id);
                                state.chat_messages.retain(|m| !m.channel.starts_with("p2pgroup:"));
                                spawn_group_load(state, &group_id, true);
                            }
                            Err(e) => {
                                state.create_group_status = format!("Create failed: {e}");
                                log::error!("create P2P group failed: {e}");
                            }
                        }
                    }
                    None => {
                        state.create_group_status = "No identity loaded. Connect first.".to_string();
                    }
                }
            }
        }
    });
    if !open {
        state.show_create_group_modal = false;
        state.create_group_ticket = None;
        state.create_group_status.clear();
        state.new_group_name.clear();
    }
}

// ─────────────────────────────── Join Group Modal ──────────────────────

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`.
pub(super) fn draw_join_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_join_group_modal;
    widgets::dialog(ctx, theme, "join_group_dialog", "Join Group", &mut open, |ui| {
        ui.set_min_width(360.0);

        // After a successful join, the modal flips into a visible "✅ Joined"
        // confirmation so the user gets clear feedback (instead of the modal
        // closing silently — operator feedback 2026-05-28).
        if let Some(joined_name) = state.join_group_result.clone() {
            ui.label(RichText::new(format!("✅ Joined group \"{}\"", joined_name)).strong());
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new("You're now an active member. The group will appear in your Groups list.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_md);
            if widgets::Button::primary("Done").show(ui, theme) {
                state.show_join_group_modal = false;
                state.join_group_result = None;
                state.join_group_invite_code.clear();
                state.join_group_status.clear();
            }
        } else {
            ui.label(
                RichText::new("Paste an invite ticket, the long base64 string from a group creator. The creator's signature inside lets you join even when they're offline.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);
            widgets::form_row(ui, theme, "Invite ticket", |ui| {
                // Monospace for the same reason as the ticket display above:
                // base64 is the one alphabet here where 0, O, I and l coexist,
                // and this field is where a mistyped character silently fails
                // a join.
                ui.add(
                    egui::TextEdit::multiline(&mut state.join_group_invite_code)
                        .font(egui::FontId::monospace(theme.font_size_small))
                        .desired_width(360.0)
                        .desired_rows(3)
                        .hint_text("paste base64 ticket here"),
                );
            });

            if !state.join_group_status.is_empty() {
                ui.add_space(theme.spacing_xs);
                ui.label(
                    RichText::new(&state.join_group_status)
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }

            ui.add_space(theme.spacing_md);

            let code_valid = !state.join_group_invite_code.trim().is_empty();
            let mut do_join = false;
            ui.horizontal(|ui| {
                ui.add_enabled_ui(code_valid, |ui| {
                    if widgets::Button::primary("Join").show(ui, theme) {
                        do_join = true;
                    }
                });
                ui.add_space(theme.spacing_sm);
                if widgets::Button::secondary("Cancel").show(ui, theme) {
                    state.show_join_group_modal = false;
                    state.join_group_status.clear();
                }
            });
            if do_join {
                // P2P signed-object join: decode the ticket + POST a
                // group_join_v1 revealing the secret. The relay's roster fold
                // admits us iff BLAKE3(secret) matches the creator-signed
                // invite and it hasn't expired.
                let server_url = state.server_url.clone();
                let ticket = state.join_group_invite_code.trim().to_string();
                let seed_opt = state.private_key_bytes.clone();
                match seed_opt {
                    Some(seed) => {
                        match crate::net::api_v2::join_group_by_ticket(&server_url, &seed, &ticket) {
                            Ok((group_id, name)) => {
                                log::info!("Joined P2P group: {} ({})", name, group_id);
                                crate::debug::push_debug(format!("Joined P2P group '{}'", name));
                                state.join_group_status.clear();
                                state.join_group_result = Some(if name.is_empty() {
                                    "(unnamed)".to_string()
                                } else {
                                    name
                                });
                                // Refresh so the joined group appears in the
                                // left-panel list once the user clicks Done.
                                refresh_p2p_groups(state);
                            }
                            Err(e) => {
                                state.join_group_status = format!("Join failed: {e}");
                                log::error!("join P2P group failed: {e}");
                            }
                        }
                    }
                    None => {
                        state.join_group_status = "No identity loaded. Connect first.".to_string();
                    }
                }
            }
        }
    });
    if !open {
        state.show_join_group_modal = false;
        state.join_group_status.clear();
        state.join_group_result = None;
        state.join_group_invite_code.clear();
    }
}

/// The "what is the Commons?" explainer, opened from the ? on the COMMONS
/// section header. Plain language, whole story: bridged rooms, how a
/// message travels, what stays local, and how servers come to trust each
/// other -- the inline-first rule (explanations live ON the thing).
// `pub(super)` only because its caller (`draw_commons_section`, which stayed
// with the commons helpers in `chat.rs`) is outside this file.
pub(super) fn draw_commons_info_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if !state.show_commons_info {
        return;
    }
    let mut open = true;
    egui::Window::new("The Commons")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .max_width(440.0)
        .show(ctx, |ui| {
            let body = |ui: &mut egui::Ui, text: &str| {
                ui.label(
                    RichText::new(text)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
                ui.add_space(theme.spacing_sm);
            };
            body(ui,
                "The Commons is where bridged rooms live. When two or more of \
                 your servers carry the same federated room (like #general), it \
                 appears here ONCE, and what you read is the merged conversation \
                 from every server that bridges it.");
            body(ui,
                "How a message travels: when you post in a Commons room, it is \
                 sent through one of your connected servers that carries the \
                 room (the box under the message says which). That server \
                 stores it and passes a signed copy to each server it \
                 federates with, so the conversation survives even if one \
                 server goes down. Everyone connected to any of those servers \
                 sees it.");
            body(ui,
                "How servers come to trust each other: federation is by \
                 explicit operator choice, never automatic. A server's \
                 identity is its cryptographic key; operators pair servers by \
                 URL or by key and set a trust level. Only rooms an admin \
                 marks as Federated are bridged, and only with trusted peers.");
            body(ui,
                "What stays local: each server's #local room is guaranteed to \
                 never bridge anywhere (click the server's name to open it), \
                 and any channel not marked Federated stays on its own \
                 server. Direct messages are end-to-end encrypted and are \
                 never part of federation.");
            body(ui,
                "The sidebar in one line: COMMONS = shared rooms, merged; \
                 SERVERS = each server's own rooms; a server labeled \"(all \
                 shared)\" keeps everything in the Commons.");
            ui.add_space(theme.spacing_xs);
            ui.vertical_centered(|ui| {
                if widgets::Button::secondary("Close").show(ui, theme) {
                    state.show_commons_info = false;
                }
            });
        });
    if !open {
        state.show_commons_info = false;
    }
}

// ─────────────────────────────── Help Modal ──────────────────────────────

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`.
pub(super) fn draw_help_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_help_modal;
    widgets::dialog(ctx, theme, "slash_commands_dialog", "Slash Commands", &mut open, |ui| {
        ui.set_min_width(420.0);
        ScrollArea::vertical()
            .id_salt("help_modal_scroll")
            .auto_shrink([false, false])
            .max_height(440.0)
            .show(ui, |ui| {
                    let section_color = theme.info();
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
                        // NOTE: /dm was removed server-side in v0.279 (it stored
                        // plaintext) — DMs go through the DM panel, which encrypts.
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
                    ui.label(RichText::new("Moderator").size(theme.font_size_body + 2.0).color(theme.warning()).strong());
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
                    ui.label(RichText::new("Admin").size(theme.font_size_body + 2.0).color(theme.danger()).strong());
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
                    ui.label(RichText::new("Federation").size(theme.font_size_body + 2.0).color(theme.success()).strong());
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
                    ui.label(RichText::new("Start a line with \"- \" for a bullet list").size(theme.font_size_body).color(desc_color));
                    ui.label(RichText::new("Enter sends; Shift+Enter starts a new line").size(theme.font_size_body).color(desc_color));
                    ui.label(RichText::new("Click the reply arrow on any message to reply").size(theme.font_size_body).color(desc_color));
                });
    });
    if !open {
        state.show_help_modal = false;
    }
}

/// Render the unencrypted-DM confirmation modal if one is pending.
/// Pops up when the user clicked Send on a DM that we couldn't encrypt
/// (B3 fix). User must explicitly confirm "Send unencrypted" or cancel
/// — no silent plaintext fallback. Call from chat::draw.
/// Incoming 1:1 voice call — Accept / Decline (v0.703). Mirrors the web's
/// incoming-call overlay; the control protocol is chat-voice-calls.js's
/// `voice_call` ring/accept/reject/hangup. Before this, native silently
/// discarded the ring and a web caller rang forever.
pub(crate) fn draw_incoming_call_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let (peer_key, peer_name) = match state.call_incoming.clone() {
        Some(p) => p,
        None => return,
    };
    let mut accept = false;
    let mut decline = false;
    egui::Window::new("Incoming call")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(egui::Vec2::new(340.0, 0.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new(&peer_name)
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new("is calling you (voice)")
                    .size(theme.font_size_body)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);
            ui.horizontal(|ui| {
                if widgets::Button::primary("Accept").show(ui, theme) {
                    accept = true;
                }
                if widgets::Button::danger("Decline").show(ui, theme) {
                    decline = true;
                }
            });
            ui.add_space(theme.spacing_xs);
        });
    if accept || decline {
        let action = if accept { "accept" } else { "reject" };
        if let Some(ref client) = state.ws_client {
            let _ = client.send(&serde_json::json!({
                "type": "voice_call",
                "from": state.profile_public_key,
                "to": peer_key,
                "action": action,
            }).to_string());
        }
        state.call_incoming = None;
        if accept {
            // The web caller now creates the WebRTC offer; it arrives as a
            // bare `webrtc_signal offer` which lib.rs routes into the voice
            // path (gated on this exact peer). The shared voice session +
            // Opus pump start from `call_active` on the next frame.
            state.call_active = Some((peer_key, peer_name));
            state.call_muted = false;
        }
    }
}

/// The in-call bar for an active 1:1 call (v0.703): who, connection state,
/// Mute, Hang up. Also shows a "Calling ... / Cancel" bar while an outgoing
/// call rings (v0.705). Anchored top-center so it stays visible over the chat.
pub(crate) fn draw_call_bar(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Outgoing ring (waiting for the callee to accept).
    if let Some((peer_key, peer_name)) = state.call_outgoing.clone() {
        let mut cancel = false;
        egui::Window::new("call_bar_ringing")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 8.0])
            .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Calling {}...", peer_name))
                            .size(theme.font_size_body)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    if widgets::Button::danger("Cancel").show(ui, theme) {
                        cancel = true;
                    }
                });
            });
        if cancel {
            if let Some(ref client) = state.ws_client {
                let _ = client.send(&serde_json::json!({
                    "type": "voice_call",
                    "from": state.profile_public_key,
                    "to": peer_key,
                    "action": "hangup",
                }).to_string());
            }
            state.call_outgoing = None;
            state.call_outgoing_deadline = None;
        }
        return;
    }

    let (peer_key, peer_name) = match state.call_active.clone() {
        Some(p) => p,
        None => return,
    };
    let connected = state.voice_connected_peers.contains(&peer_key);
    let mut hangup = false;
    let mut toggle_mute = false;
    let muted = state.call_muted;
    egui::Window::new("call_bar")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 8.0])
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("In call with {}", peer_name))
                        .size(theme.font_size_body)
                        .color(theme.text_primary())
                        .strong(),
                );
                let (status, color) = if connected {
                    ("connected", theme.success())
                } else {
                    ("connecting...", theme.warning())
                };
                ui.label(RichText::new(status).size(theme.font_size_small).color(color));
                let mute_label = if muted { "Unmute" } else { "Mute" };
                if widgets::Button::secondary(mute_label).show(ui, theme) {
                    toggle_mute = true;
                }
                if widgets::Button::danger("Hang up").show(ui, theme) {
                    hangup = true;
                }
            });
            if muted {
                ui.label(
                    RichText::new("Your mic is muted")
                        .size(theme.font_size_small)
                        .color(theme.warning()),
                );
            }
        });
    if toggle_mute {
        state.call_muted = !state.call_muted;
    }
    if hangup {
        if let Some(ref client) = state.ws_client {
            let _ = client.send(&serde_json::json!({
                "type": "voice_call",
                "from": state.profile_public_key,
                "to": peer_key,
                "action": "hangup",
            }).to_string());
        }
        state.call_active = None;
        state.call_muted = false;
        state.voice_connected_peers.remove(&peer_key);
        // Drop the str0m connection NOW; leaving it to ICE timeout would
        // make on_voice_offer's already-connected guard refuse this peer's
        // next call for ~30s.
        if let Some(ref webrtc) = state.webrtc {
            webrtc.close_peer(peer_key);
        }
    }
}


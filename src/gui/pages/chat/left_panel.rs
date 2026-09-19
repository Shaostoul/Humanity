//! The chat page's LEFT RAIL: the column of collapsible sections that decides
//! what the centre panel is showing.
//!
//! Top to bottom it is the connect box (only while disconnected), the
//! scratchpad row, Direct Messages, Groups, Commons, and Servers, and under an
//! expanded server its channel list with the add/edit/pin affordances. Picking
//! any row in here is the page's one way of setting `chat_active_channel`,
//! which is why the whole rail is one cluster rather than several: every
//! section is a different shape of the same act.
//!
//! Extracted VERBATIM from `gui/pages/chat.rs` (file-size ratchet), which stood
//! at 9,053 lines against an 8,000 budget. Second of four clusters out in that
//! pass, and the largest. The only delta beyond the move is one `pub(super)`
//! on `draw_left_panel`, whose caller (`draw`, the page frame) stayed behind.
//!
//! WHAT IS IN HERE, and why each piece belongs with the others:
//!
//!   * `draw_left_panel` is the frame: it fixes the rail's width against the
//!     lock button, then calls the sections in order.
//!   * `draw_scratchpad_row` is the always-present private note channel.
//!   * `draw_dm_section` lists conversations newest first and opens one from
//!     the LOCAL encrypted store (sealed sender: the relay keeps no history).
//!   * `draw_groups_section` lists the P2P groups. The network and crypto work
//!     behind those rows lives in the sibling `p2p_groups.rs`; this file only
//!     draws them and asks.
//!   * `draw_servers_section`, `ServerRowSpec`, `ServerRowOut` and
//!     `draw_server_row` are the saved-server list. The spec/out pair exists so
//!     one row can be drawn identically whether it is the active server or a
//!     saved one, with the row reporting back what the user did instead of
//!     mutating state mid-draw.
//!   * `draw_active_server_entry` is the expanded server: its channel list,
//!     unread marks, and the per-channel menus.
//!
//! `draw_commons_section` deliberately stayed in `chat.rs`: it is drawn here
//! but it belongs to the commons helpers (`commons_rooms`, `open_commons_room`,
//! `commons_room_read_only`) that the centre panel reads too. This file reaches
//! it through `use super::*`, which also brings in the page's imports and the
//! shared section-header and colour helpers, so nothing had to be widened.

use super::*;

// ─────────────────────────────── LEFT PANEL ───────────────────────────────

// `pub(super)` only because its caller (`draw`, the page frame) stayed in
// `chat.rs`; it was private when the two lived in one file.
pub(super) fn draw_left_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let is_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    // Show connect UI only when disconnected (no separate connection bar when connected)
    if !is_connected {
        Frame::NONE
            .fill(theme.bg_card())
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let dot_sz = theme.status_dot_size;
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(dot_sz), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), dot_sz / 2.0, theme.danger());
                    ui.label(
                        RichText::new("Not connected")
                            .size(theme.font_size_body)
                            .color(theme.danger())
                            .strong(),
                    );
                });
                ui.add_space(4.0);

                // Full-PQ: if the seed is locked (encrypted on disk, not in
                // memory), surface a prominent unlock affordance and SHORT-
                // CIRCUIT the rest of the connect panel. Without this the
                // user sees a Connect button that silently no-ops because
                // the limited-mode connect guard refuses without a seed.
                let identity_locked = state.private_key_bytes.is_none()
                    && !state.encrypted_private_key.is_empty();
                if identity_locked {
                    ui.label(
                        RichText::new("Identity locked")
                            .size(theme.font_size_body)
                            .color(theme.warning())
                            .strong(),
                    );
                    ui.label(
                        RichText::new("Your seed is encrypted. Unlock it to connect (DMs need it for the post-quantum key).")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.add_space(6.0);
                    // v0.278.0: route to PIN modal if the user has set up
                    // KeychainPin mode — otherwise it's the classic
                    // passphrase modal. Keychain mode auto-unlocked at
                    // startup; if it failed (keychain gone), this button
                    // falls back to the passphrase modal which is the
                    // recovery path.
                    let (btn_label, target_mode) = match state.auto_unlock_mode {
                        crate::auto_unlock::AutoUnlockMode::KeychainPin if !state.pin_encrypted_seed.is_empty() => {
                            ("Unlock with PIN", crate::gui::PassphraseMode::PinUnlock)
                        }
                        _ => ("Unlock with Passphrase", crate::gui::PassphraseMode::Unlock),
                    };
                    if widgets::Button::primary(btn_label).full_width().show(ui, theme) {
                        state.passphrase_needed = true;
                        state.passphrase_mode = target_mode;
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Don't have the passphrase? Open Settings → Identity & Seed Phrase → Recover from Seed Phrase, and enter your 24-word backup.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    // Skip the server/name/Connect form below — it can't help.
                    return;
                }

                if state.server_url.is_empty() {
                    state.server_url = "https://united-humanity.us".to_string();
                }
                if state.user_name.is_empty() {
                    state.user_name = "DesktopUser".to_string();
                }

                ui.label(RichText::new("Server:").size(theme.font_size_small).color(theme.text_muted()));
                ui.add(
                    egui::TextEdit::singleline(&mut state.server_url)
                        .desired_width(ui.available_width() - 24.0)
                        .font(egui::TextStyle::Small),
                );
                ui.add_space(2.0);
                ui.label(RichText::new("Name:").size(theme.font_size_small).color(theme.text_muted()));
                ui.add(
                    egui::TextEdit::singleline(&mut state.user_name)
                        .desired_width(ui.available_width() - 24.0)
                        .font(egui::TextStyle::Small),
                );
                ui.add_space(4.0);

                if widgets::Button::primary("Connect").full_width().show(ui, theme) {
                    // Full-PQ guard: refuse to connect with a locked seed —
                    // it would register a keyless name-squatter on the relay.
                    if state.private_key_bytes.is_none() {
                        state.ws_status = "Unlock your identity first (Settings → Security → Unlock, or Recover from seed). Connecting locked would squat your name with no encryption key.".to_string();
                    } else {
                        let ws_url = derive_ws_url(&state.server_url);
                        let name = state.user_name.clone();
                        let pubkey = if state.profile_public_key.is_empty() {
                            generate_random_hex_key()
                        } else {
                            state.profile_public_key.clone()
                        };
                        log::info!("Connecting to {} as {} (key: {})", ws_url, name, &pubkey[..8]);
                        // If another server's live state is still resident
                        // (dropped socket, message buffer), park it under
                        // the URL it was dialed for before dialing the new
                        // one, so nothing is lost or mis-filed.
                        if norm_server_url(&state.connected_server_url)
                            != norm_server_url(&state.server_url)
                        {
                            let target = state.server_url.clone();
                            state.park_active_connection();
                            state.server_url = target;
                        }
                        // Full-PQ: manual Connect must advertise the Kyber key too.
                        state.ws_client = Some(crate::net::ws_client::WsClient::connect_with_kyber(
                            &ws_url, &name, &pubkey, &state.kyber_public_b64,
                        ));
                        state.connected_server_url = state.server_url.clone();
                        // Fresh socket: identify handshake not yet complete (v0.794).
                        state.ws_identified = false;
                        state.dm_fetch_sent = false;
                        state.ws_status = "Connecting...".to_string();
                        state.ws_manually_disconnected = false;
                        state.ws_reconnect_timer = 0.0;
                        state.ws_reconnect_delay = 5.0;
                        state.ws_reconnect_attempts = 0;
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                }
            });
    }

    // ── Scrollable section area ──
    // Thin floating scrollbar, matching the right rail (operator field
    // test 4: the left bar sat permanently wide because the full-width
    // interactive rows underneath kept its hover-expansion latched).
    ui.spacing_mut().scroll = egui::style::ScrollStyle::thin();
    ScrollArea::vertical()
        .id_salt("chat_left_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── Scratchpad (local-only, above everything) ──
            draw_scratchpad_row(ui, theme, state);

            // ── DMs Section (red tint) ──
            draw_dm_section(ui, theme, state);

            // ── Groups Section (green tint) ──
            draw_groups_section(ui, theme, state);

            // ── Commons Section: federated rooms carried by 2+ of my
            //    servers, each rendered once (federation-ux.md) ──
            draw_commons_section(ui, theme, state);

            // ── Servers Section (blue tint) ──
            draw_servers_section(ui, theme, state);
        });
}

/// Local scratchpad channel - not attached to any server/group/DM.
fn draw_scratchpad_row(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let is_active = state.chat_active_channel == "scratchpad";
    let row_height = theme.row_height;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        // Active row = a wash of the scratchpad's own identity accent; hover /
        // rest fall back to the shared surface tokens. All three follow the
        // theme, so a restyle takes the scratchpad with it.
        let sp = theme.scratchpad_accent();
        let bg = if is_active {
            Color32::from_rgba_unmultiplied(sp.r(), sp.g(), sp.b(), 70)
        } else if resp.hovered() {
            theme.bg_tertiary()
        } else {
            theme.bg_card()
        };
        ui.painter().rect_filled(rect, 0.0, bg);

        if is_active {
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            ui.painter().rect_filled(bar, 0.0, sp);
        }

        let text_color = if is_active { theme.text_primary() } else { theme.text_secondary() };
        ui.painter().text(
            egui::pos2(rect.left() + theme.item_padding + 2.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            "# scratchpad",
            egui::FontId::proportional(theme.body_size),
            text_color,
        );
    }

    if resp.clicked() && state.chat_active_channel != "scratchpad" {
        // Only clear+refetch when actually switching contexts. Re-clicking the
        // active row is a no-op (BUG-035 — used to nuke local-echoed unsent text).
        state.chat_active_channel = "scratchpad".to_string();
        state.chat_messages.clear();
        state.history_fetched = false;
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

// ── DMs Section ──

fn draw_dm_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Sort DMs alphabetically (case-insensitive) by partner name. Operator
    // feedback 2026-05-15 — "alphabetical first, drag-and-drop later if
    // needed." Done in-place at the top of each render fn so the order is
    // stable across updates without each WS handler having to re-sort.
    // O(n log n) on a small Vec; negligible cost.
    state.chat_dms.sort_by(|a, b| a.user_name.to_lowercase().cmp(&b.user_name.to_lowercase()));

    let collapsed = state.chat_dm_collapsed;
    let dm_count = state.chat_dms.len();
    let display_limit = state.chat_dm_display_limit;

    // Capture the cog's rect from inside the closure so the floating
    // popup below knows where to anchor. Cell because Rect: Copy and we
    // need interior mutability across the closure boundary.
    let cog_rect_cell: std::cell::Cell<Option<egui::Rect>> = std::cell::Cell::new(None);
    let mut dm_cog_clicked = false;
    if tinted_section_header_with_buttons(
        ui,
        theme,
        &format!("DMs ({})", dm_count),
        collapsed,
        theme.dm_bg(),
        |ui| {
            let (cog_rect, cog_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let cog_color = if cog_resp.hovered() { Color32::WHITE } else { theme.text_muted() };
            crate::gui::widgets::icons::paint_cog(ui.painter(), cog_rect, cog_color);
            cog_rect_cell.set(Some(cog_rect));
            if cog_resp.on_hover_text("DM Settings").clicked() {
                dm_cog_clicked = true;
            }
        },
    ) {
        state.chat_dm_collapsed = !state.chat_dm_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // Toggle popup open state on cog click.
    if dm_cog_clicked {
        state.dm_settings_popup_open = !state.dm_settings_popup_open;
        // First time this popup opens this session, fetch the real prefs from the server
        // (relay + web already support this fully -- see GuiState::notif_dm_enabled doc
        // comment) so the toggle below reflects reality instead of the client-side default.
        if state.dm_settings_popup_open && !state.notif_prefs_loaded {
            if let Some(ref client) = state.ws_client {
                if client.is_connected() {
                    let msg = serde_json::json!({ "type": "get_notification_prefs" });
                    client.send(&msg.to_string());
                }
            }
        }
    }
    // Render the popup as a manual floating Area anchored to the cog.
    // Bypassing egui::popup_below_widget because that machinery uses
    // CloseOnClick which fires on the SAME FRAME as the trigger click,
    // making the popup flicker on for one frame then disappear
    // (operator bug 2026-05-08). Manual Area + close-outside check
    // that explicitly excludes the trigger click frame is reliable.
    if state.dm_settings_popup_open {
        if let Some(cog_rect) = cog_rect_cell.get() {
            let popup_resp = egui::Area::new(egui::Id::new("dm_settings_popup"))
                .fixed_pos(egui::pos2(cog_rect.left() - 100.0, cog_rect.bottom() + 4.0))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            ui.set_min_width(140.0);
                            ui.label(RichText::new("DM Settings").size(theme.font_size_body).color(theme.text_primary()).strong());
                            ui.separator();
                            // Press-and-HOLD (5s): wipes local DM history on THIS
                            // device; unrecoverable unless another device has it.
                            if hold_to_confirm_button(
                                ui,
                                theme,
                                egui::Id::new("dm_clear_all_hold"),
                                "Clear all DMs (hold)",
                                "Hold to clear all DMs…",
                                5.0,
                                "Press and HOLD to erase your local DM history on THIS device. It cannot be recovered unless another device still has a copy.",
                            ) {
                                state.chat_dms.clear();
                                state.dm_settings_popup_open = false;
                            }
                            // Sealed-sender privacy control: delete every
                            // envelope currently queued for us server-side.
                            // (They auto-expire after the server's TTL
                            // anyway; this is the immediate scrub. Local
                            // history on this device is untouched.)
                            // Press-and-HOLD (5s) to fire: this scrubs the SERVER
                            // mailbox for every device you have not synced yet, so
                            // a single click is far too easy to fat-finger.
                            if hold_to_confirm_button(
                                ui,
                                theme,
                                egui::Id::new("dm_purge_hold"),
                                "Delete my server mailbox (hold)",
                                "Hold to delete mailbox…",
                                5.0,
                                "Press and HOLD to delete all encrypted DM envelopes stored for you on the server. Messages already saved on your devices stay. Holding prevents an accidental misclick.",
                            ) {
                                if let Some(ref client) = state.ws_client {
                                    if client.is_connected() {
                                        client.send(&serde_json::json!({ "type": "dm_purge" }).to_string());
                                    }
                                }
                                state.dm_settings_popup_open = false;
                            }
                            let dm_notif_label = if state.notif_dm_enabled {
                                "DM Notifications: On"
                            } else {
                                "DM Notifications: Off"
                            };
                            if ui.button(dm_notif_label).clicked() {
                                state.notif_dm_enabled = !state.notif_dm_enabled;
                                if let Some(ref client) = state.ws_client {
                                    if client.is_connected() {
                                        let msg = serde_json::json!({
                                            "type": "update_notification_prefs",
                                            "dm": state.notif_dm_enabled,
                                            "mentions": state.notif_mentions_enabled,
                                            "tasks": state.notif_tasks_enabled,
                                            "dnd_start": state.notif_dnd_start,
                                            "dnd_end": state.notif_dnd_end,
                                        });
                                        client.send(&msg.to_string());
                                    }
                                }
                                state.dm_settings_popup_open = false;
                            }
                        });
                });
            // Close-on-click-outside — but ignore the trigger-click frame
            // so opening doesn't immediately close. Click is "outside" if
            // it lands neither in the popup's rect nor on the cog.
            if !dm_cog_clicked {
                let click_outside = ui.ctx().input(|i| {
                    i.pointer.any_click() && i.pointer.interact_pos().map_or(false, |pos| {
                        !popup_resp.response.rect.contains(pos) && !cog_rect.contains(pos)
                    })
                });
                if click_outside {
                    state.dm_settings_popup_open = false;
                }
            }
        }
    }

    if !collapsed {
        Frame::NONE
            .fill(theme.dm_bg())
            .inner_margin(egui::Margin::symmetric(0, 1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
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
                let visible_dms: Vec<_> = if display_limit > 0 && dms.len() > display_limit {
                    dms.iter().take(display_limit).collect()
                } else {
                    dms.iter().collect()
                };

                for dm in &visible_dms {
                    let dm_channel = format!("dm:{}", dm.user_key);
                    let is_active = state.chat_active_channel == dm_channel;

                    // Channel-row style DM entry. When we have a last-message
                    // preview the row grows a second line for it (v0.715).
                    let has_preview = !dm.last_message.trim().is_empty();
                    let row_height = if has_preview {
                        theme.row_height + theme.font_size_small + 4.0
                    } else {
                        theme.row_height
                    };
                    let (full_rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    if ui.is_rect_visible(full_rect) {
                        let bg = if is_active || response.hovered() {
                            theme.dm_row_hover()
                        } else {
                            theme.dm_row_bg()
                        };
                        ui.painter().rect_filled(full_rect, 0.0, bg);

                        // Active indicator bar, in the DM lane's identity accent.
                        if is_active {
                            let bar = egui::Rect::from_min_size(full_rect.min, Vec2::new(3.0, full_rect.height()));
                            ui.painter().rect_filled(bar, 0.0, theme.dm_accent());
                        }

                        let mut cursor_x = full_rect.left() + theme.item_padding + 2.0;
                        // With a preview, the name sits on the upper line and
                        // the preview on the lower; without, name is centered.
                        let cy = if has_preview {
                            full_rect.top() + theme.row_height * 0.5
                        } else {
                            full_rect.center().y
                        };

                        // Unread dot
                        if dm.unread {
                            let dot_r = theme.status_dot_size * 0.375;
                            ui.painter().circle_filled(egui::pos2(cursor_x + dot_r, cy), dot_r, theme.danger());
                            cursor_x += dot_r * 2.0 + 3.0;
                        }

                        // DM icon prefix + user name (like "@ username")
                        let name_color = if is_active || dm.unread { theme.text_primary() } else { theme.text_secondary() };
                        ui.painter().text(
                            egui::pos2(cursor_x, cy),
                            egui::Align2::LEFT_CENTER,
                            &format!("@ {}", dm.user_name),
                            egui::FontId::proportional(theme.body_size),
                            name_color,
                        );

                        // Last-message preview line: muted, single line,
                        // elided to fit the row width. Newlines flattened.
                        if has_preview {
                            let avail_w = full_rect.width() - (theme.item_padding + 2.0) - 10.0;
                            // Cheap width heuristic: proportional glyphs
                            // average ~0.55em; egui has no single-line elide.
                            let max_chars = ((avail_w / (theme.font_size_small * 0.55)).max(4.0)) as usize;
                            let flat: String = dm
                                .last_message
                                .chars()
                                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                                .collect();
                            let flat = flat.trim();
                            let preview: String = if flat.chars().count() > max_chars {
                                let cut: String = flat.chars().take(max_chars.saturating_sub(3)).collect();
                                format!("{}...", cut.trim_end())
                            } else {
                                flat.to_string()
                            };
                            let preview_color = if dm.unread { theme.text_secondary() } else { theme.text_muted() };
                            ui.painter().text(
                                egui::pos2(full_rect.left() + theme.item_padding + 2.0, full_rect.top() + theme.row_height + theme.font_size_small * 0.5),
                                egui::Align2::LEFT_CENTER,
                                &preview,
                                egui::FontId::proportional(theme.font_size_small),
                                preview_color,
                            );
                        }
                    }

                    if response.clicked() && state.chat_active_channel != dm_channel {
                        // History loads from the LOCAL encrypted store —
                        // the relay keeps no DM history (sealed-sender).
                        open_dm_conversation(state, &dm.user_key);
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    // Right-click context menu on DMs
                    response.context_menu(|ui| {
                        if ui.button("View Profile").clicked() {
                            state.chat_user_modal_open = true;
                            state.chat_user_modal_key = dm.user_key.clone();
                            ui.close_menu();
                        }
                        if ui.button(RichText::new("Close Conversation").color(theme.danger())).clicked() {
                            state.chat_dms.retain(|d| d.user_key != dm.user_key);
                            // Also delete from the local store — the sidebar
                            // is rebuilt from it, so a retain() alone would
                            // resurrect the row on the next DM event.
                            if let Some(store) = state.dm_store.as_mut() {
                                store.delete_conversation(&dm.user_key);
                                store.save();
                            }
                            if state.chat_active_channel == format!("dm:{}", dm.user_key) {
                                state.chat_active_channel = "general".to_string();
                            }
                            ui.close_menu();
                        }
                    });
                }
            });
    }
}

// ── Groups Section ──

fn draw_groups_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // NOTE: first-render population is handled by the BACKGROUND list refresh at
    // the top of draw() (spawn_groups_list_refresh, which fires when
    // p2p_groups_last_fetch is None). We deliberately do NOT do a synchronous
    // fetch here — that blocked the UI thread on the first open of the Groups
    // section. The background path runs before this panel each frame, so the
    // list is requested without ever freezing the render loop.

    // Sort groups alphabetically by name (see draw_dm_section for rationale).
    state.p2p_groups.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let collapsed = state.chat_groups_collapsed;
    let group_count = state.p2p_groups.len();

    // Track button clicks from the header.
    // Note: the Groups-level "settings" cog was REMOVED in v0.223
    // (operator feedback 2026-05-12 — "The settings button does nothing
    // ... Group notifications are handled per group. Rearranging groups
    // can be done by simply dragging and dropping..."). Only the
    // Create + Join buttons remain.
    let mut create_clicked = false;
    let mut join_clicked = false;

    if tinted_section_header_with_buttons(
        ui,
        theme,
        &format!("Groups ({})", group_count),
        collapsed,
        theme.group_bg(),
        |ui| {
            // + button (create group)
            {
                let (plus_rect, plus_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
                let plus_color = if plus_resp.hovered() { Color32::WHITE } else { theme.text_secondary() };
                crate::gui::widgets::icons::paint_plus(ui.painter(), plus_rect, plus_color);
                if plus_resp.on_hover_text("Create Group").clicked() {
                    create_clicked = true;
                }
            }
            // Join button (arrow icon)
            {
                let (arrow_rect, arrow_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
                let arrow_color = if arrow_resp.hovered() { Color32::WHITE } else { theme.text_muted() };
                crate::gui::widgets::icons::paint_arrow_right(ui.painter(), arrow_rect, arrow_color);
                if arrow_resp.on_hover_text("Join Group").clicked() {
                    join_clicked = true;
                }
            }
        },
    ) {
        state.chat_groups_collapsed = !state.chat_groups_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if create_clicked {
        state.show_create_group_modal = true;
        state.new_group_name.clear();
        state.new_group_share_history = false; // default to private each open
    }
    if join_clicked {
        state.show_join_group_modal = true;
        state.join_group_invite_code.clear();
    }

    if !collapsed {
        Frame::NONE
            .fill(theme.group_bg())
            .inner_margin(egui::Margin::symmetric(0, 1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;

                if state.p2p_groups.is_empty() {
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

                // P2P (signed-object) groups — rendered above legacy ones.
                // Left-click switches the main chat to the group, exactly like
                // clicking a channel (no modal): the active channel becomes
                // "p2pgroup:<id>" and the decrypted message log renders in the
                // center panel. The active row is highlighted; clicking it runs
                // the full enter (rekey-if-creator + epoch fetch + decrypt).
                let p2p_clone = state.p2p_groups.clone();
                let mut open_p2p_id: Option<String> = None;
                // Deferred popup actions (applied after the loop to avoid
                // borrowing `state` while iterating the cloned list).
                let mut p2p_copy_invite: Option<(String, String)> = None; // (gid, name)
                let mut p2p_leave_gid: Option<String> = None;
                let mut p2p_disband_gid: Option<String> = None;
                let p2p_ctx_time = ui.ctx().input(|i| i.time);
                for p in p2p_clone.iter() {
                    let is_active =
                        state.chat_active_channel == format!("p2pgroup:{}", p.group_id);
                    let hdr_height = 24.0;
                    let full_w = ui.available_width();
                    let (row_rect, row_resp) = ui.allocate_exact_size(
                        Vec2::new(full_w, hdr_height),
                        egui::Sense::click(),
                    );
                    // Cog (settings) sits at the right edge — opens the group
                    // menu (Copy invite / Leave / Disband). Computed here so both
                    // the paint pass and the interact below share the rect.
                    let cog_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.right() - 14.0, row_rect.center().y),
                        Vec2::splat(16.0),
                    );
                    if ui.is_rect_visible(row_rect) {
                        let bump: u8 = if is_active { 40 } else if row_resp.hovered() { 28 } else { 0 };
                        let bg = if bump > 0 {
                            Color32::from_rgba_premultiplied(
                                theme.group_bg().r().saturating_add(bump),
                                theme.group_bg().g().saturating_add(bump),
                                theme.group_bg().b().saturating_add(bump),
                                theme.group_bg().a(),
                            )
                        } else { theme.group_bg() };
                        ui.painter().rect_filled(row_rect, 0.0, bg);
                        let cy = row_rect.center().y;
                        // Crown (gold) marks a group I created/own, just left of
                        // the name. Joined groups have no crown; their name keeps
                        // the default indent. Painted shape — the egui font has
                        // no crown glyph (emoji tofu).
                        let name_x = if p.is_creator {
                            let crown_rect = egui::Rect::from_center_size(
                                egui::pos2(row_rect.left() + 12.0, cy),
                                Vec2::splat(13.0),
                            );
                            // Gold via the theme's `warning` token (an
                            // amber/gold) — keeps the crown themeable, no
                            // hardcoded literal.
                            crate::gui::widgets::icons::paint_crown(
                                ui.painter(),
                                crown_rect,
                                theme.warning(),
                            );
                            row_rect.left() + 24.0
                        } else {
                            row_rect.left() + 12.0
                        };
                        // Group name
                        ui.painter().text(
                            egui::pos2(name_x, cy),
                            egui::Align2::LEFT_CENTER,
                            &p.name,
                            egui::FontId::proportional(theme.body_size),
                            if is_active { theme.accent() } else { theme.text_primary() },
                        );
                        // Member count, right-aligned (left of the cog).
                        ui.painter().text(
                            egui::pos2(row_rect.right() - 28.0, cy),
                            egui::Align2::RIGHT_CENTER,
                            format!("{}", p.members.len()),
                            egui::FontId::proportional(theme.font_size_small),
                            theme.text_muted(),
                        );
                        // Cog icon (accent on hover, matching the legacy groups).
                        let on_cog = cog_rect.contains(
                            ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default()),
                        );
                        let cog_color = if on_cog { theme.accent() } else { theme.text_muted() };
                        crate::gui::widgets::icons::paint_cog(
                            ui.painter(),
                            egui::Rect::from_center_size(cog_rect.center(), Vec2::splat(11.0)),
                            cog_color,
                        );
                        if on_cog {
                            let rgb = crate::gui::widgets::row::rgb_from_time(p2p_ctx_time);
                            ui.painter().rect_stroke(
                                cog_rect.shrink(1.0),
                                Rounding::same(3),
                                Stroke::new(1.0, rgb),
                                egui::StrokeKind::Outside,
                            );
                            ui.ctx().request_repaint();
                        }
                    }
                    // Cog interact (after row_resp so it wins its sub-rect:
                    // egui's last-interact-wins keeps a cog click from also
                    // opening the group).
                    let cog_resp = ui.interact(
                        cog_rect,
                        ui.id().with("p2pcog").with(&p.group_id),
                        egui::Sense::click(),
                    );
                    if cog_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let menu_id = ui.id().with("p2pmenu").with(&p.group_id);
                    if cog_resp.clicked() || row_resp.secondary_clicked() {
                        ui.memory_mut(|m| m.toggle_popup(menu_id));
                    }
                    egui::popup_below_widget(
                        ui,
                        menu_id,
                        &cog_resp,
                        egui::PopupCloseBehavior::CloseOnClick,
                        |ui| {
                            ui.set_min_width(180.0);
                            ui.label(
                                RichText::new(&p.name)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary())
                                    .strong(),
                            );
                            ui.separator();
                            if ui.button("Copy invite ticket").clicked() {
                                p2p_copy_invite = Some((p.group_id.clone(), p.name.clone()));
                            }
                            // 3s hold: a short gate so a stray tap doesn't drop
                            // you from a group; rejoining needs a new invite.
                            if hold_to_confirm_button(
                                ui,
                                theme,
                                egui::Id::new(("p2p_leave_hold", p.group_id.as_str())),
                                "Leave group (hold)",
                                "Hold to leave…",
                                3.0,
                                "Press and HOLD to leave this group. You can rejoin later with a new invite ticket.",
                            ) {
                                p2p_leave_gid = Some(p.group_id.clone());
                            }
                            // Disband: creator only. Press-and-HOLD (5s): this
                            // destroys the group for EVERY member, irreversibly.
                            if p.is_creator
                                && hold_to_confirm_button(
                                    ui,
                                    theme,
                                    egui::Id::new(("p2p_disband_hold", p.group_id.as_str())),
                                    "Disband for everyone (hold)",
                                    "Hold to disband…",
                                    5.0,
                                    "Press and HOLD to permanently disband this group for ALL members. This cannot be undone.",
                                )
                            {
                                p2p_disband_gid = Some(p.group_id.clone());
                            }
                        },
                    );
                    if row_resp.clicked() {
                        open_p2p_id = Some(p.group_id.clone());
                    }
                    if row_resp.hovered() && !cog_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    ui.add_space(2.0);
                }
                if let Some(gid) = open_p2p_id {
                    // Switch the active channel INSTANTLY (the header + "Loading…"
                    // render this frame), then load the group on a background
                    // thread — no UI freeze. apply_group_load fills in the
                    // messages when the worker returns.
                    state.chat_active_channel = format!("p2pgroup:{}", gid);
                    state.p2p_group_invite_status.clear();
                    state.chat_reply_to = None;
                    // Drop the previous group's rows immediately so we don't
                    // briefly show stale history under the new header.
                    state.chat_messages.retain(|m| !m.channel.starts_with("p2pgroup:"));
                    spawn_group_load(state, &gid, true);
                }
                // Apply deferred popup actions.
                if let Some((gid, name)) = p2p_copy_invite {
                    mint_and_copy_p2p_invite(ui.ctx(), state, &gid, &name);
                }
                if let Some(gid) = p2p_leave_gid {
                    leave_p2p_group(state, &gid);
                }
                if let Some(gid) = p2p_disband_gid {
                    disband_p2p_group(state, &gid);
                }

                // (Legacy relay-group rendering removed 2026-08-23: the
                // plaintext group system died server-side; groups are the E2EE
                // P2P list above.)
            });
    }
}

// ── Servers Section ──

fn draw_servers_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // NO sorting: the list keeps the user's saved order, stable across
    // switches (operator field test 3: "have them maintain their place
    // when I switch"). Reordering is drag-and-drop on the rows.

    let collapsed = state.chat_servers_collapsed;

    // Build a virtual server from the current connection
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    let virtual_server_count = if connected { 1 } else { 0 } + state.chat_servers.len();

    let header_label = format!("Servers ({})", virtual_server_count);

    let mut add_server_clicked = false;
    let mut manage_servers_clicked = false;
    if tinted_section_header_with_buttons(
        ui,
        theme,
        &header_label,
        collapsed,
        theme.server_bg(),
        |ui| {
            let (plus_rect, plus_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let plus_color = if plus_resp.hovered() { Color32::WHITE } else { theme.text_secondary() };
            crate::gui::widgets::icons::paint_plus(ui.painter(), plus_rect, plus_color);
            if plus_resp.on_hover_text("Add Server").clicked() {
                add_server_clicked = true;
            }
            // Manage servers: the whole Server Control Center (health,
            // control, console, config, host-a-node) opens from HERE now --
            // the top-nav "Servers" tab was folded into this header at the
            // operator's request (one servers concept, one place).
            let (srv_rect, srv_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let srv_color = if srv_resp.hovered() { Color32::WHITE } else { theme.text_secondary() };
            crate::gui::widgets::icons::paint_server(ui.painter(), srv_rect, srv_color);
            if srv_resp
                .on_hover_text("Manage servers: health, control, config, host a node on this PC")
                .clicked()
            {
                manage_servers_clicked = true;
            }
        },
    ) {
        state.chat_servers_collapsed = !state.chat_servers_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    if manage_servers_clicked {
        state.push_nav_to(crate::gui::GuiPage::RelayControl);
    }
    // (disconnect is handled inside the server name row)
    if add_server_clicked {
        // Fresh form every time: the drafts kept the PREVIOUS entry's
        // URL, so adding a second server opened pre-filled with the
        // first one (report 2026-08-14).
        state.add_server_url_draft.clear();
        state.add_server_name_draft.clear();
        state.show_add_server_modal = true;
        state.add_server_url_draft.clear();
        state.add_server_name_draft.clear();
    }

    if !collapsed {
        Frame::NONE
            .fill(theme.server_bg())
            .inner_margin(egui::Margin::symmetric(0, 1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                // Every server renders IN ITS SAVED ORDER: the active one
                // expands in place (draw_active_server_entry below) instead
                // of being pulled out to the top, so the list never
                // reshuffles on switch (operator field test 3). A connected
                // server that is not saved yet still shows, first.
                let current_in_list = {
                    let cur = norm_server_url(&state.server_url);
                    state.chat_servers.iter().any(|s| norm_server_url(&s.url) == cur)
                };
                if connected && !current_in_list {
                    draw_active_server_entry(ui, theme, state);
                    ui.add_space(6.0);
                }

                // Additional saved servers. Clicking a name SWITCHES to that
                // server (points server_url there and reconnects with the same
                // identity) -- the Add Server modal promises this, and before
                // v0.712 the names were plain labels that did nothing.
                let current = state.server_url.trim_end_matches('/').to_string();
                // (url, top_y, bottom_y) of each server's rendered block --
                // the drop targets for drag-reordering.
                let mut row_spans: Vec<(String, f32, f32)> = Vec::new();
                for server in state.chat_servers.clone().iter() {
                    let is_current = server.url.trim_end_matches('/') == current;
                    if is_current && connected {
                        // The active server, rendered IN PLACE at its saved
                        // position (full entry: header, cog, channels, voice).
                        let span_top = ui.cursor().min.y;
                        draw_active_server_entry(ui, theme, state);
                        row_spans.push((server.url.clone(), span_top, ui.cursor().min.y));
                        ui.add_space(6.0);
                        continue;
                    }
                    let span_top = ui.cursor().min.y;
                    let sec_collapsed = state
                        .chat_server_sections_collapsed
                        .contains(&norm_server_url(&server.url));
                    // This server's VISIBLE channels, computed up front so the
                    // header knows whether to draw a collapse triangle at all.
                    // Bridged rooms live only in COMMONS and #local is the
                    // server name itself (field test 4), so most servers list
                    // nothing here -- which is the point: the list stays
                    // short at dozens of servers.
                    let bg_channels: Vec<(String, String, bool, bool)> = {
                        let commons: std::collections::HashSet<String> =
                            commons_rooms(state).into_iter().map(|r| r.name).collect();
                        let filter_map = |ch: &crate::gui::ChatChannel| {
                            if ch.local_only || (ch.federated && commons.contains(&ch.id)) {
                                None
                            } else {
                                Some((
                                    ch.id.clone(),
                                    ch.name.clone(),
                                    ch.unread,
                                    false,
                                ))
                            }
                        };
                        if is_current {
                            state.chat_channels.iter().filter_map(filter_map).collect()
                        } else {
                            state
                                .connections
                                .iter()
                                .find(|c| c.url == norm_server_url(&server.url))
                                .map(|c| c.channels.iter().filter_map(filter_map).collect())
                                .unwrap_or_default()
                        }
                    };
                    // Bridged rooms this server carries (federation badge).
                    let bridges: Vec<String> = {
                        let commons: std::collections::HashSet<String> =
                            commons_rooms(state).into_iter().map(|r| r.name).collect();
                        let pick = |chs: &Vec<crate::gui::ChatChannel>| {
                            chs.iter()
                                .filter(|c| c.federated && commons.contains(&c.id))
                                .map(|c| c.id.clone())
                                .collect::<Vec<String>>()
                        };
                        if is_current {
                            pick(&state.chat_channels)
                        } else {
                            state
                                .connections
                                .iter()
                                .find(|c| c.url == norm_server_url(&server.url))
                                .map(|c| pick(&c.channels))
                                .unwrap_or_default()
                        }
                    };
                    // Live background link state (multi-connection stage 3):
                    // every saved server holds its own connection now, so a
                    // non-current row can still be online, with unread mail.
                    let bg = state
                        .connections
                        .iter()
                        .find(|c| c.url == norm_server_url(&server.url));
                    let bg_online = bg.map_or(false, |c| c.identified);
                    let bg_unread = bg.map_or(false, |c| {
                        c.channels.iter().any(|ch| ch.unread) || c.dms.iter().any(|d| d.unread)
                    });
                    // "All shared": every room this server has is bridged, so
                    // its whole conversation already lives under COMMONS --
                    // say so BEFORE the user clicks in looking for more
                    // (operator field test 3).
                    let bg_all_shared = bg.map_or(false, |c| {
                        c.identified
                            && !c.channels.is_empty()
                            && c.channels.iter().all(|ch| ch.federated)
                    });
                    // NO leading add_space here: the active path has none, and
                    // any asymmetry between the two callers moves a row when
                    // it changes state (field test 7, the 2 px shuffle). The
                    // inter-server gap is the single add_space(6.0) at the
                    // bottom of the loop, identical for both paths.
                    let spec = ServerRowSpec {
                        name: server.name.clone(),
                        url: server.url.clone(),
                        id: server.id.clone(),
                        is_current,
                        link_up: bg_online || (is_current && connected),
                        online_count: None,
                        member_count: None,
                        all_shared: bg_all_shared,
                        unread: bg_unread,
                        has_channels: !bg_channels.is_empty(),
                        collapsed: sec_collapsed,
                        bridges,
                        can_forget: !is_current,
                    };
                    let out = draw_server_row(ui, theme, state, &spec);
                    if out.collapse_clicked {
                        let key = norm_server_url(&server.url);
                        if !state.chat_server_sections_collapsed.remove(&key) {
                            state.chat_server_sections_collapsed.insert(key);
                        }
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                    if out.forget_fired {
                        let rid = server.id.clone();
                        state.chat_servers.retain(|s| s.id != rid);
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                    if out.cog.clicked() && state.private_key_bytes.is_some() {
                        // Settings for THIS server: switch (instant when its
                        // background link is live), then open the page.
                        if !is_current {
                            state.park_active_connection();
                            if !state.unpark_connection(&server.url) {
                                state.server_url = server.url.clone();
                                let ws_url = derive_ws_url(&server.url);
                                let uname = state.user_name.clone();
                                let pubkey = state.profile_public_key.clone();
                                state.ws_client = Some(
                                    crate::net::ws_client::WsClient::connect_with_kyber(
                                        &ws_url,
                                        &uname,
                                        &pubkey,
                                        &state.kyber_public_b64,
                                    ),
                                );
                                state.connected_server_url = server.url.clone();
                                state.ws_identified = false;
                        state.dm_fetch_sent = false;
                                state.ws_status = format!("Switching to {}...", server.name);
                                state.ws_manually_disconnected = false;
                                state.ws_reconnect_timer = 0.0;
                                state.ws_reconnect_delay = 5.0;
                                state.ws_reconnect_attempts = 0;
                                state.chat_active_channel = "general".to_string();
                                state.history_fetched = false;
                            }
                            crate::config::AppConfig::from_gui_state(state).save();
                        }
                        state.push_nav_to(crate::gui::GuiPage::ServerSettings);
                    }
                    if out.clicked && !is_current {
                        if state.private_key_bytes.is_some() {
                            state.park_active_connection();
                            if state.unpark_connection(&server.url) {
                                // Land in this server's own room: #local.
                                if let Some(local_id) = state
                                    .chat_channels
                                    .iter()
                                    .find(|c| c.local_only)
                                    .map(|c| c.id.clone())
                                {
                                    if state.chat_active_channel != local_id {
                                        state.chat_active_channel = local_id;
                                        state.chat_messages.clear();
                                        state.history_fetched = false;
                                    }
                                }
                            } else {
                                state.server_url = server.url.clone();
                                let ws_url = derive_ws_url(&server.url);
                                let uname = state.user_name.clone();
                                let pubkey = if state.profile_public_key.is_empty() {
                                    generate_random_hex_key()
                                } else {
                                    state.profile_public_key.clone()
                                };
                                state.ws_client = Some(
                                    crate::net::ws_client::WsClient::connect_with_kyber(
                                        &ws_url,
                                        &uname,
                                        &pubkey,
                                        &state.kyber_public_b64,
                                    ),
                                );
                                state.connected_server_url = server.url.clone();
                                state.ws_identified = false;
                        state.dm_fetch_sent = false;
                                state.ws_status = format!("Switching to {}...", server.name);
                                state.ws_manually_disconnected = false;
                                state.ws_reconnect_timer = 0.0;
                                state.ws_reconnect_delay = 5.0;
                                state.ws_reconnect_attempts = 0;
                                state.chat_active_channel = "general".to_string();
                                state.history_fetched = false;
                            }
                            crate::config::AppConfig::from_gui_state(state).save();
                        } else {
                            state.ws_status =
                                "Unlock your identity first to connect (Settings).".to_string();
                        }
                    }
                    // This server's channels, live from its background
                    // connection (multi-connection stage 4). ALL channels are
                    // listed; a bridged one carries the federation badge and
                    // opens the merged Commons view WITHOUT switching servers
                    // (one room, one view). A plain channel switches to this
                    // server and lands in that room. A CURRENT but offline
                    // server (the operator's stopped self-relay) lists its
                    // last-known channels from the legacy fields, so it never
                    // looks channel-less.
                    if !sec_collapsed {
                        for (ch_id, ch_name, ch_unread, ch_commons) in bg_channels.clone() {
                            ui.horizontal(|ui| {
                                // Exactly the ACTIVE channel-row height: any
                                // per-path height difference shuffles the
                                // list on switch (field test 7 discipline).
                                ui.set_min_height(theme.row_height);
                                ui.add_space(24.0);
                                let label = ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(format!("# {}", ch_name))
                                                .size(theme.font_size_body)
                                                .color(theme.text_secondary()),
                                        )
                                        .sense(egui::Sense::click()),
                                    )
                                    .on_hover_text(if ch_commons {
                                        format!(
                                            "Open the Commons view of #{} (bridged room)",
                                            ch_name
                                        )
                                    } else {
                                        format!("Open #{} on {}", ch_name, server.name)
                                    });
                                if ch_commons {
                                    let (br, _) = ui.allocate_exact_size(
                                        egui::vec2(13.0, 11.0),
                                        egui::Sense::hover(),
                                    );
                                    crate::gui::widgets::icons::paint_federation(
                                        ui.painter(),
                                        br,
                                        theme.text_muted(),
                                    );
                                }
                                if ch_unread {
                                    let (ur, _) = ui.allocate_exact_size(
                                        egui::vec2(8.0, 8.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(
                                        ur.center(),
                                        3.0,
                                        theme.accent(),
                                    );
                                }
                                if label.clicked() && state.private_key_bytes.is_some() {
                                    if ch_commons {
                                        open_commons_room(state, &ch_id);
                                    } else {
                                        // Same switch as clicking the server name,
                                        // then land directly in the clicked room.
                                        state.park_active_connection();
                                        if !state.unpark_connection(&server.url) {
                                            state.server_url = server.url.clone();
                                            let ws_url = derive_ws_url(&server.url);
                                            let uname = state.user_name.clone();
                                            let pubkey = state.profile_public_key.clone();
                                            state.ws_client = Some(
                                                crate::net::ws_client::WsClient::connect_with_kyber(
                                                    &ws_url,
                                                    &uname,
                                                    &pubkey,
                                                    &state.kyber_public_b64,
                                                ),
                                            );
                                            state.connected_server_url = server.url.clone();
                                            state.ws_identified = false;
                        state.dm_fetch_sent = false;
                                            state.ws_status =
                                                format!("Switching to {}...", server.name);
                                            state.ws_manually_disconnected = false;
                                            state.ws_reconnect_timer = 0.0;
                                            state.ws_reconnect_delay = 5.0;
                                            state.ws_reconnect_attempts = 0;
                                        }
                                        state.chat_active_channel = ch_id.clone();
                                        state.chat_messages.clear();
                                        state.history_fetched = false;
                                        if let Some(c) = state
                                            .chat_channels
                                            .iter_mut()
                                            .find(|c| c.id == ch_id)
                                        {
                                            c.unread = false;
                                        }
                                        crate::config::AppConfig::from_gui_state(state).save();
                                    }
                                }
                            });
                        }
                    }
                    row_spans.push((server.url.clone(), span_top, ui.cursor().min.y));
                    // Gap between server sections (operator field test 3):
                    // each server is its own block, visually separated.
                    ui.add_space(6.0);
                }

                // Drag-reorder resolution: while a row is held, draw the
                // insertion line at the nearest slot; on release, move the
                // server there and persist the new order.
                if let Some(drag_url) = state.server_drag.clone() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    let pointer = ui.input(|i| i.pointer.interact_pos());
                    let released = ui.input(|i| i.pointer.any_released());
                    if let Some(pos) = pointer {
                        let mut insert_at = row_spans.len();
                        for (i, (_, top, bottom)) in row_spans.iter().enumerate() {
                            if pos.y < (top + bottom) * 0.5 {
                                insert_at = i;
                                break;
                            }
                        }
                        let line_y = if insert_at == 0 {
                            row_spans.first().map(|s| s.1)
                        } else {
                            row_spans.get(insert_at - 1).map(|s| s.2)
                        };
                        if let Some(y) = line_y {
                            ui.painter().line_segment(
                                [
                                    egui::pos2(ui.max_rect().min.x, y),
                                    egui::pos2(ui.max_rect().max.x, y),
                                ],
                                egui::Stroke::new(2.0, theme.accent()),
                            );
                        }
                        if released {
                            state.server_drag = None;
                            if let Some(from) =
                                state.chat_servers.iter().position(|s| s.url == drag_url)
                            {
                                let entry = state.chat_servers.remove(from);
                                let mut to = insert_at;
                                if from < insert_at {
                                    to = to.saturating_sub(1);
                                }
                                let to = to.min(state.chat_servers.len());
                                state.chat_servers.insert(to, entry);
                                crate::config::AppConfig::from_gui_state(state).save();
                            }
                        }
                    } else if released {
                        state.server_drag = None;
                    }
                }
            });
    }

    ui.add_space(2.0);
}

// ─────────────────────────────── RIGHT PANEL ──────────────────────────────

/// Everything one server header row needs, prepared by the caller.
struct ServerRowSpec {
    name: String,
    /// The row's identity for actions + the identity-color chip.
    url: String,
    /// Saved-entry id ("" when the active server isn't saved).
    id: String,
    is_current: bool,
    link_up: bool,
    /// Active server: online user count shown after the name.
    online_count: Option<usize>,
    /// Active server: total member count at the far right (the X slot,
    /// which the current server doesn't use -- you can't forget it).
    member_count: Option<usize>,
    all_shared: bool,
    unread: bool,
    has_channels: bool,
    collapsed: bool,
    /// Bridged room names this server carries (federation badge + tooltip).
    bridges: Vec<String>,
    /// Offer the hold-to-forget X (saved, non-current rows).
    can_forget: bool,
}

/// What the row reported back; the caller applies the effects.
struct ServerRowOut {
    /// Left-click anywhere on the row (not on a control).
    clicked: bool,
    /// Right-click anywhere on the row.
    secondary: bool,
    collapse_clicked: bool,
    cog: egui::Response,
    forget_fired: bool,
}

/// THE server header row -- one painter-based template with FIXED slots,
/// used for the active server and every saved server alike, so nothing
/// shifts, resizes, or reorders when the active server changes (operator
/// field tests 3-6; this replaced two divergent implementations).
///
/// Layout, left to right: collapse triangle (or blank), identity color
/// chip, link dot, name (+online count), then right-aligned at fixed
/// offsets: "(current)"/"(all shared)", unread dot, federation badge,
/// cog, drag grip, and the X-or-member-count slot.
fn draw_server_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    spec: &ServerRowSpec,
) -> ServerRowOut {
    const ROW_H: f32 = 24.0;
    let full_w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(full_w, ROW_H), egui::Sense::hover());
    let cy = rect.center().y;
    let row_id = ui.id().with(("srv_row", &spec.url));

    // Background strip: every server reads as its own section.
    let svr_bg = Color32::from_rgba_premultiplied(
        theme.server_bg().r().saturating_add(20),
        theme.server_bg().g().saturating_add(20),
        theme.server_bg().b().saturating_add(20),
        theme.server_bg().a(),
    );
    ui.painter().rect_filled(rect, 0.0, svr_bg);

    // Whole-row interact registered FIRST: egui gives overlapping hits to
    // the LAST-registered widget, so every specific control below (triangle,
    // cog, grip, X) must come after this to win its own region. Getting
    // this backwards is exactly how the cogs went dead in field test 7.
    let row_resp = ui.interact(rect, row_id.with("ctx"), egui::Sense::click());
    if row_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // ── Right cluster, fixed offsets from the right edge ──
    let slot_x = rect.right() - 12.0; // X / member count
    let grip_c = egui::pos2(rect.right() - 30.0, cy);
    let cog_c = egui::pos2(rect.right() - 48.0, cy);
    let badge_c = egui::pos2(rect.right() - 65.0, cy);
    let tag_right = rect.right() - 74.0;

    // ── Left slots ──
    let mut cx = rect.left() + 8.0;
    // Collapse triangle (blank keeps alignment when there is nothing to expand).
    let tri_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
    let mut collapse_clicked = false;
    if spec.has_channels {
        if spec.collapsed {
            crate::gui::widgets::icons::paint_triangle_right(ui.painter(), tri_rect, theme.text_secondary());
        } else {
            crate::gui::widgets::icons::paint_triangle_down(ui.painter(), tri_rect, theme.text_secondary());
        }
        // Registered after the row interact, so it wins its region.
        let tri_click = egui::Rect::from_min_size(rect.min, Vec2::new(22.0, ROW_H));
        let tri = ui.interact(tri_click, row_id.with("tri"), egui::Sense::click());
        if tri.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        collapse_clicked = tri.clicked();
    }
    cx += 14.0;
    // Identity color chip (matches this server's dots on Commons rooms).
    let chip = egui::Rect::from_center_size(egui::pos2(cx + 2.0, cy), egui::vec2(4.0, 14.0));
    ui.painter().rect_filled(chip, 2.0, server_color(&spec.url));
    cx += 10.0;
    // Link dot: green only when genuinely up.
    let dot_color = if spec.link_up { theme.success() } else { theme.text_muted() };
    ui.painter().circle_filled(egui::pos2(cx + 4.0, cy), 4.0, dot_color);
    cx += 14.0;
    // Name (+ online count for the active server), RGB-animated when current.
    let name_color = if spec.is_current {
        let t = ui.ctx().input(|i| i.time);
        match theme.nav_active_border_animation {
            crate::gui::theme::anim::RGB_CYCLE => {
                let speed = theme.nav_separator_animation_speed.max(0.0);
                let hue = ((t * 0.3 * speed as f64) % 1.0) as f32;
                crate::gui::pages::escape_menu::hsv_to_rgb(hue.rem_euclid(1.0), 0.9, 1.0)
            }
            crate::gui::theme::anim::PULSE => {
                let speed = theme.nav_separator_animation_speed.max(0.0);
                let p = ((t * 2.0 * speed as f64).sin() * 0.5 + 0.5) as f32;
                let a = theme.accent();
                let b = theme.text_primary();
                Color32::from_rgb( // theme-exempt: animated blend of two theme tokens
                    (a.r() as f32 * p + b.r() as f32 * (1.0 - p)) as u8,
                    (a.g() as f32 * p + b.g() as f32 * (1.0 - p)) as u8,
                    (a.b() as f32 * p + b.b() as f32 * (1.0 - p)) as u8,
                )
            }
            _ => theme.success(),
        }
    } else {
        theme.text_primary()
    };
    let label = match spec.online_count {
        Some(n) => format!("{} ({})", spec.name, n),
        None => spec.name.clone(),
    };
    // Elide the name against the RESERVED right cluster. The tag is
    // right-ALIGNED at tag_right, extending LEFT by its width, so the name
    // must stop before the widest tag's left edge -- reserved constantly
    // (whether a tag is shown or not) so a name never changes length when
    // "(current)" appears on switch.
    let name_font = egui::FontId::proportional(theme.body_size);
    let tag_reserve = ui
        .fonts(|f| {
            f.layout_no_wrap(
                "(all shared)".to_string(),
                egui::FontId::proportional(theme.small_size),
                theme.text_muted(),
            )
        })
        .size()
        .x;
    let max_name_w = (tag_right - tag_reserve - 8.0 - cx).max(24.0);
    let measure = |ui: &egui::Ui, s: &str| {
        ui.fonts(|f| f.layout_no_wrap(s.to_string(), name_font.clone(), name_color))
            .size()
            .x
    };
    let mut shown = label.clone();
    if measure(ui, &shown) > max_name_w {
        while !shown.is_empty() && measure(ui, &format!("{}...", shown)) > max_name_w {
            shown.pop();
        }
        shown = format!("{}...", shown.trim_end());
    }
    ui.painter().text(
        egui::pos2(cx, cy),
        egui::Align2::LEFT_CENTER,
        &shown,
        name_font.clone(),
        name_color,
    );

    // ── Right cluster contents ──
    // Tag: (current) / (all shared).
    let tag = if spec.is_current {
        Some("(current)")
    } else if spec.all_shared {
        Some("(all shared)")
    } else {
        None
    };
    if let Some(t) = tag {
        ui.painter().text(
            egui::pos2(tag_right, cy),
            egui::Align2::RIGHT_CENTER,
            t,
            egui::FontId::proportional(theme.small_size),
            theme.text_muted(),
        );
    }
    // Unread dot.
    if spec.unread && !spec.is_current {
        ui.painter().circle_filled(egui::pos2(badge_c.x + 14.0, cy), 3.0, theme.accent());
    }
    // Federation badge: this server bridges rooms into the Commons.
    if !spec.bridges.is_empty() {
        let br = egui::Rect::from_center_size(badge_c, Vec2::splat(11.0));
        crate::gui::widgets::icons::paint_federation(ui.painter(), br, theme.text_muted());
        let badge_resp = ui.interact(
            br.expand(2.0),
            row_id.with("badge"),
            egui::Sense::hover(),
        );
        badge_resp.on_hover_text(format!(
            "Bridges into the Commons: #{}",
            spec.bridges.join(", #")
        ));
    }
    // Cog.
    let cog_rect = egui::Rect::from_center_size(cog_c, Vec2::splat(12.0));
    let cog = ui.interact(cog_rect.expand(3.0), row_id.with("cog"), egui::Sense::click());
    let cog_color = if cog.hovered() { theme.accent() } else { theme.text_muted() };
    crate::gui::widgets::icons::paint_cog(ui.painter(), cog_rect, cog_color);
    if cog.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // Drag grip.
    let grip_rect = egui::Rect::from_center_size(grip_c, Vec2::splat(14.0));
    let grip = ui.interact(grip_rect, row_id.with("grip"), egui::Sense::drag());
    let grip_color = if grip.hovered() || grip.dragged() {
        theme.text_primary()
    } else {
        theme.text_muted()
    };
    for k in 0..3 {
        let y = grip_rect.top() + 4.0 + k as f32 * 3.0;
        ui.painter().line_segment(
            [
                egui::pos2(grip_rect.left() + 2.0, y),
                egui::pos2(grip_rect.right() - 2.0, y),
            ],
            egui::Stroke::new(1.5, grip_color),
        );
    }
    if grip.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if grip.drag_started() {
        state.server_drag = Some(spec.url.clone());
    }
    grip.on_hover_text("Drag to reorder");
    // X (hold to forget) or member count.
    let mut forget_fired = false;
    if spec.can_forget {
        let x_rect = egui::Rect::from_center_size(egui::pos2(slot_x, cy), Vec2::splat(18.0));
        forget_fired = widgets::hold_to_confirm_at(
            ui,
            theme,
            row_id.with("forget"),
            "X",
            x_rect,
            3.0,
            "HOLD 3 seconds to forget this server (Add Server re-adds it)",
        );
    } else if let Some(n) = spec.member_count {
        ui.painter().text(
            egui::pos2(rect.right() - theme.item_padding, cy),
            egui::Align2::RIGHT_CENTER,
            &format!("{}", n),
            egui::FontId::proportional(theme.small_size),
            theme.text_muted(),
        );
    }

    ServerRowOut {
        clicked: row_resp.clicked(),
        secondary: row_resp.secondary_clicked(),
        collapse_clicked,
        cog,
        forget_fired,
    }
}

/// The ACTIVE server's full sidebar entry (header + cog + channels +
/// voice rosters). Extracted verbatim from the old virtual-entry block so
/// the servers loop can render it IN PLACE at the server's saved position.
fn draw_active_server_entry(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !state.ws_client.as_ref().map_or(false, |c| c.is_connected()) {
        return;
    }
                    let online_count = state.chat_users.iter().filter(|u| u.status != "offline").count();
                    let svr_hdr_height = 24.0;
                    let svr_full_w = ui.available_width();
                    // Prefer the SAVED name (field test 5: switching to the
                    // self-relay renamed it to its IP because this header
                    // derived the label from the URL instead of the saved
                    // entry the user named).
                    let svr_name = {
                        let cur = norm_server_url(&state.server_url);
                        state
                            .chat_servers
                            .iter()
                            .find(|s| norm_server_url(&s.url) == cur && !s.name.trim().is_empty())
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| server_display_name(&state.server_url))
                    };
                    // Per-server collapse set, same as saved rows (the old
                    // separate chat_connected_server_collapsed flag is
                    // retired from this path).
                    let svr_collapsed = state
                        .chat_server_sections_collapsed
                        .contains(&norm_server_url(&state.server_url));
                    let commons_set: std::collections::HashSet<String> =
                        commons_rooms(state).into_iter().map(|r| r.name).collect();
                    let bridges: Vec<String> = state
                        .chat_channels
                        .iter()
                        .filter(|c| c.federated && commons_set.contains(&c.id))
                        .map(|c| c.id.clone())
                        .collect();
                    let has_channels = state
                        .chat_channels
                        .iter()
                        .any(|c| !c.local_only && !(c.federated && commons_set.contains(&c.id)));
                    let _ = (svr_hdr_height, svr_full_w);
                    let spec = ServerRowSpec {
                        name: svr_name.clone(),
                        url: state.server_url.clone(),
                        id: String::new(),
                        is_current: true,
                        link_up: true,
                        online_count: Some(online_count),
                        member_count: Some(state.chat_users.len()),
                        all_shared: false,
                        unread: false,
                        has_channels,
                        collapsed: svr_collapsed,
                        bridges,
                        can_forget: false,
                    };
                    let out = draw_server_row(ui, theme, state, &spec);
                    let mut svr_disconnect = false;
                    if out.collapse_clicked {
                        let key = norm_server_url(&state.server_url);
                        if !state.chat_server_sections_collapsed.remove(&key) {
                            state.chat_server_sections_collapsed.insert(key);
                        }
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                    // Clicking the row opens the server's OWN room: #local.
                    if out.clicked {
                        if let Some(local_id) = state
                            .chat_channels
                            .iter()
                            .find(|c| c.local_only)
                            .map(|c| c.id.clone())
                        {
                            if state.chat_active_channel != local_id {
                                state.chat_active_channel = local_id;
                                state.chat_messages.clear();
                                state.history_fetched = false;
                            }
                        }
                    }
                    let cog_resp = out.cog;
                    let menu_id = ui.id().with("svr_menu");
                    if cog_resp.clicked() || cog_resp.secondary_clicked() || out.secondary {
                        ui.memory_mut(|m| m.toggle_popup(menu_id));
                    }
                    let is_server_admin = {
                        let vr = viewer_role(state);
                        vr == "admin" || vr == "moderator" || vr == "mod"
                    };
                    egui::popup_below_widget(ui, menu_id, &cog_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(160.0);
                        ui.label(RichText::new(&svr_name).size(theme.font_size_body).color(theme.text_primary()).strong());
                        ui.separator();
                        let invite_url = format!("https://united-humanity.us/chat");
                        if ui.button("Copy Invite Link").clicked() {
                            ui.ctx().copy_text(invite_url);
                        }
                        if is_server_admin {
                            ui.separator();
                            if ui.button("Server Settings").clicked() {
                                // push_nav_to so Esc returns to Chat.
                                state.push_nav_to(crate::gui::GuiPage::ServerSettings);
                            }
                        }
                        ui.separator();
                        // (v0.847.x: "Mute Server" removed — native has no
                        // notification/desktop-alert system yet, so there was
                        // nothing to mute. Re-add wired when notifications land.)
                        if ui.button(RichText::new("Disconnect").color(theme.danger())).clicked() {
                            svr_disconnect = true;
                        }
                    });

                    if svr_disconnect {
                        if let Some(ref mut client) = state.ws_client {
                            client.disconnect();
                        }
                        state.ws_client = None;
                        state.ws_status = "Disconnected".to_string();
                        state.ws_manually_disconnected = true;
                        state.chat_users.clear();
                    }

                    // Merged channels (only if not collapsed)
                    if !svr_collapsed {
                    let active = state.chat_active_channel.clone();
                    let channels = state.chat_channels.clone();
                    // Rooms that render ONCE in the COMMONS section above are
                    // hidden from every per-server list (federation-ux.md).
                    let commons_names: std::collections::HashSet<String> =
                        commons_rooms(state).into_iter().map(|r| r.name).collect();
                    let is_channel_admin = {
                        let vr = viewer_role(state);
                        vr == "admin" || vr == "moderator" || vr == "mod"
                    };

                    // Track which channel index had a voice toggle click
                    let mut voice_toggle_idx: Option<(usize, bool)> = None;
                    // Track which channel had a gear icon click
                    let mut gear_click_id: Option<String> = None;
                    // Voice roster (v0.484): a clicked participant (name, public_key)
                    // opens the per-user control modal; local key marks our own row.
                    let mut voice_user_click: Option<(String, String)> = None;
                    let my_voice_key = state.profile_public_key.clone();

                    let ctx_time = ui.ctx().input(|i| i.time);
                    for (idx, ch) in channels.iter().enumerate() {
                        // Field test 4 (final call after trying both ways):
                        // bridged rooms live ONLY in COMMONS, and #local is
                        // the server name itself -- neither repeats here.
                        // This is what keeps the list short at many servers.
                        if (ch.federated && commons_names.contains(&ch.id)) || ch.local_only {
                            continue;
                        }
                        let is_commons = false;
                        let is_active = ch.id == active;
                        let accent = theme.accent();
                        let bg = if is_active {
                            Color32::from_rgb(
                                accent.r() / 5 + 15,
                                accent.g() / 5 + 15,
                                accent.b() / 5 + 15,
                            )
                        } else {
                            theme.server_row_bg()
                        };

                        // Check if the edit modal is open for THIS channel (for RGB border on cog)
                        let edit_modal_for_this = state.show_channel_edit_modal
                            && state.edit_channel_id == ch.id;

                        // Single click target for the whole row; check click position
                        // to distinguish voice/gear/channel clicks without overlapping interact().
                        let row_w = ui.available_width();
                        let (row_rect, response) = ui.allocate_exact_size(
                            Vec2::new(row_w, theme.row_height),
                            egui::Sense::click(),
                        );

                        let mut voice_icon_rect = egui::Rect::NOTHING;
                        let mut gear_icon_rect = egui::Rect::NOTHING;

                        if ui.is_rect_visible(row_rect) {
                            let hover = response.hovered();
                            let fill = if hover && !is_active { theme.server_row_hover() } else { bg };
                            ui.painter().rect_filled(row_rect, 0.0, fill);
                            if is_active {
                                let bar = egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_rect.height()));
                                ui.painter().rect_filled(bar, 0.0, accent);
                            }

                            let text_color = if is_active { theme.text_primary() } else { theme.text_secondary() };
                            let icon_size = 12.0;
                            let cy = row_rect.center().y;
                            let mut cx = row_rect.left() + theme.item_padding + 2.0;

                            // 1. Voice chat icon
                            if ch.voice_enabled {
                                let icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - icon_size * 0.5), Vec2::splat(icon_size));
                                voice_icon_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, row_rect.top()), Vec2::new(icon_size + 4.0, row_rect.height()));
                                if ch.voice_joined {
                                    crate::gui::widgets::icons::paint_speaker(ui.painter(), icon_rect, theme.success());
                                } else {
                                    let on_voice = response.hovered() && voice_icon_rect.contains(ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default()));
                                    let mic_color = if on_voice { Color32::WHITE } else { theme.text_muted() };
                                    crate::gui::widgets::icons::paint_mic(ui.painter(), icon_rect, mic_color);
                                }
                                cx += icon_size + 2.0;
                            }

                            // (Per-channel settings cog removed in v0.187 —
                            // channel admin lives in the Server Settings
                            // modal next to the server name. Click that
                            // single cog to manage every channel + role +
                            // member in one place.)

                            // 2. Unread dot (same look as the DM/group rows). (v0.718)
                            if ch.unread {
                                let dot_r = theme.status_dot_size * 0.375;
                                ui.painter().circle_filled(egui::pos2(cx + dot_r, cy), dot_r, theme.danger());
                                cx += dot_r * 2.0 + 3.0;
                            }

                            // 3. # Channel name
                            let name_str = format!("# {}", ch.name);
                            let name_color = if ch.unread && !is_active { theme.text_primary() } else { text_color };
                            ui.painter().text(
                                egui::pos2(cx + 2.0, cy),
                                egui::Align2::LEFT_CENTER,
                                &name_str,
                                egui::FontId::proportional(theme.body_size),
                                name_color,
                            );
                            // Status icons after the name: eye = read-only,
                            // node-graph = federated (v0.244; uses the
                            // paint_eye/paint_federation icons added v0.240).
                            if ch.read_only || ch.federated {
                                let name_w = ui.fonts(|f| f.layout_no_wrap(
                                    name_str.clone(),
                                    egui::FontId::proportional(theme.body_size),
                                    text_color,
                                )).size().x;
                                let isz = (theme.body_size * 0.9).min(14.0);
                                let mut ix = cx + 2.0 + name_w + 6.0;
                                if ch.read_only {
                                    let r = egui::Rect::from_min_size(
                                        egui::pos2(ix, cy - isz / 2.0),
                                        Vec2::splat(isz),
                                    );
                                    crate::gui::widgets::icons::paint_eye(
                                        ui.painter(), r, theme.text_muted());
                                    ix += isz + 3.0;
                                }
                                if ch.federated {
                                    let r = egui::Rect::from_min_size(
                                        egui::pos2(ix, cy - isz / 2.0),
                                        Vec2::splat(isz),
                                    );
                                    crate::gui::widgets::icons::paint_federation(
                                        ui.painter(), r, theme.text_muted());
                                }
                            }
                        }

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if response.clicked() {
                            let click_pos = ui.ctx().input(|i| i.pointer.interact_pos().unwrap_or_default());
                            if voice_icon_rect.contains(click_pos) && ch.voice_enabled {
                                voice_toggle_idx = Some((idx, !ch.voice_joined));
                            } else if gear_icon_rect.contains(click_pos) && is_channel_admin {
                                gear_click_id = Some(ch.id.clone());
                            } else if is_commons {
                                // Bridged channel: open the merged Commons
                                // view of this room (same one the COMMONS
                                // section opens -- one room, one view).
                                if !is_active {
                                    open_commons_room(state, &ch.id);
                                }
                            } else if state.chat_active_channel != ch.id {
                                // Only swap channel context when the click actually changes
                                // channels. Re-clicking the active channel used to clear
                                // chat_messages and re-fetch history, which nuked any
                                // local-echoed unsent reply (BUG-035). Now it's a no-op.
                                state.chat_active_channel = ch.id.clone();
                                state.chat_messages.clear();
                                state.history_fetched = false;
                                // Opening the channel clears its unread dot. (v0.718)
                                if let Some(c) = state.chat_channels.iter_mut().find(|c| c.id == ch.id) {
                                    c.unread = false;
                                }
                            }
                        }
                        // Right-click: copy channel link
                        response.context_menu(|ui| {
                            let link = format!("https://united-humanity.us/chat/{}", ch.name);
                            if ui.button("Copy Channel Link").clicked() {
                                ui.ctx().copy_text(link);
                                ui.close_menu();
                            }
                        });

                        // Voice roster (v0.481): who is currently connected to this
                        // channel's voice, indented under the row. Populated from the
                        // relay's voice_channel_list broadcast (the relay tracks the
                        // authoritative roster; this is purely a display).
                        for (pk, pname) in &ch.voice_participants {
                            let is_me = pk == &my_voice_key;
                            let pw = ui.available_width();
                            let (prect, presp) = ui.allocate_exact_size(
                                Vec2::new(pw, theme.row_height * 0.78),
                                egui::Sense::click(),
                            );
                            if presp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if presp.clicked() {
                                let nm = if pname.trim().is_empty() { "Player".to_string() } else { pname.clone() };
                                voice_user_click = Some((nm, pk.clone()));
                            }
                            if ui.is_rect_visible(prect) {
                                let isz = 10.0;
                                let pcy = prect.center().y;
                                let pix = prect.left() + theme.item_padding + 18.0; // indent under the name
                                // Your own entry is accent + bold so you can tell at a
                                // glance you are in this channel's voice. (v0.484)
                                let col = if is_me { theme.accent() } else { theme.text_secondary() };
                                let irect = egui::Rect::from_min_size(
                                    egui::pos2(pix, pcy - isz * 0.5), Vec2::splat(isz),
                                );
                                crate::gui::widgets::icons::paint_person(ui.painter(), irect, col);
                                let mut dn = if pname.trim().is_empty() {
                                    "(in voice)".to_string()
                                } else {
                                    pname.clone()
                                };
                                if is_me {
                                    dn.push_str("  (you)");
                                }
                                ui.painter().text(
                                    egui::pos2(pix + isz + 6.0, pcy),
                                    egui::Align2::LEFT_CENTER,
                                    &dn,
                                    egui::FontId::proportional(theme.body_size * 0.9),
                                    col,
                                );
                            }
                        }
                    }

                    // Apply gear click (open edit modal for that channel)
                    if let Some(ch_id) = gear_click_id {
                        if let Some(ch) = channels.iter().find(|c| c.id == ch_id) {
                            state.show_channel_edit_modal = true;
                            state.edit_channel_id = ch.id.clone();
                            state.edit_channel_name = ch.name.clone();
                            state.edit_channel_description = ch.description.clone();
                            state.edit_channel_confirm_delete = false;
                        }
                    }

                    // Apply a voice-roster click: open the per-user control modal
                    // (the same modal reached from the member list). (v0.484)
                    if let Some((name, key)) = voice_user_click {
                        state.chat_user_modal_open = true;
                        state.chat_user_modal_name = name;
                        state.chat_user_modal_key = key;
                    }

                    // Apply voice toggle after the loop. The relay tracks voice
                    // rooms by NUMERIC channel id and expects a `voice_room` message
                    // with action join/leave (the old `voice_join` was ignored by
                    // the relay, so native join never registered). Phase C, v0.491.
                    if let Some((idx, joining)) = voice_toggle_idx {
                        // Voice is per-channel (v0.493): the room id IS this text
                        // channel's own id. Join/leave that directly.
                        let (ch_name, ch_id) = match state.chat_channels.get_mut(idx) {
                            Some(ch) => {
                                ch.voice_joined = joining;
                                (ch.name.clone(), ch.id.clone())
                            }
                            None => (String::new(), String::new()),
                        };
                        if !ch_id.is_empty() {
                            let action = if joining { "join" } else { "leave" };
                            log::info!("Voice {} requested: {} (room_id {})", action, ch_name, ch_id);
                            crate::debug::push_debug(format!("Voice: {} for '{}' (id {})", action, ch_name, ch_id));
                            // Phase C: track the active room so the roster handler
                            // dials the incumbents (newcomer-offers rule). Reset the
                            // incumbent-capture flag on each join.
                            if joining {
                                state.voice_active_room = Some(ch_id.clone());
                                state.voice_incumbents_captured = false;
                            } else {
                                state.voice_active_room = None;
                            }
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "voice_room",
                                        "action": action,
                                        "room_id": ch_id,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    }

                    // (+ Create Channel button removed in v0.187 — channel
                    // creation now lives inside the Server Settings cog so
                    // the sidebar stays clean. Click the cog next to the
                    // server name to manage all channels in one place.)

                    // (The "all rooms shared" hint row was removed in field
                    // test 5, and the trailing add_space in field test 7:
                    // ANYTHING the active body emits that the saved path
                    // does not is a height delta that shuffles rows on
                    // switch. The active entry must emit EXACTLY a 24 px
                    // header + its channel rows, like every other server.)
                    } // end if !svr_collapsed
}

